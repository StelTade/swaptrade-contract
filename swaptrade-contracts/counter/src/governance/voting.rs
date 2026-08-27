// Governance Voting
//
// Simple on-chain proposal and vote tracking for the DAO phase.
// Each proposal collects `votes_for` and `votes_against` from unique voters.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalStatus {
    Active,
    Passed,
    Rejected,
    Expired,
    Executed,
}

#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: u64,
    pub description: String,
    pub proposer: String,
    pub votes_for: u64,
    pub votes_against: u64,
    pub quorum: u64,
    pub deadline_secs: u64,
    pub created_at: u64,
    pub status: ProposalStatus,
    voters: HashSet<String>,
}

impl Proposal {
    pub fn new(
        id: u64,
        description: String,
        proposer: String,
        quorum: u64,
        deadline_secs: u64,
        created_at: u64,
    ) -> Self {
        Self {
            id,
            description,
            proposer,
            votes_for: 0,
            votes_against: 0,
            quorum,
            deadline_secs,
            created_at,
            status: ProposalStatus::Active,
            voters: HashSet::new(),
        }
    }

    pub fn cast_vote(&mut self, voter: &str, in_favor: bool, now: u64) -> Result<(), String> {
        if self.status != ProposalStatus::Active {
            return Err("proposal is not active".to_string());
        }
        if now > self.created_at + self.deadline_secs {
            self.status = ProposalStatus::Expired;
            return Err("voting period has ended".to_string());
        }
        if !self.voters.insert(voter.to_string()) {
            return Err("voter has already voted".to_string());
        }
        if in_favor {
            self.votes_for += 1;
        } else {
            self.votes_against += 1;
        }
        Ok(())
    }

    pub fn finalize(&mut self, now: u64) {
        if self.status != ProposalStatus::Active {
            return;
        }
        let total = self.votes_for + self.votes_against;
        if total < self.quorum {
            if now > self.created_at + self.deadline_secs {
                self.status = ProposalStatus::Expired;
            }
            return;
        }
        if self.votes_for > self.votes_against {
            self.status = ProposalStatus::Passed;
        } else {
            self.status = ProposalStatus::Rejected;
        }
    }

    /// Execute a passed proposal. Prevents double execution.
    pub fn execute(&mut self) -> Result<(), String> {
        if self.status == ProposalStatus::Executed {
            return Err("Proposal has already been executed".to_string());
        }
        if self.status != ProposalStatus::Passed {
            return Err(format!(
                "Proposal cannot be executed, current status: {:?}",
                self.status
            ));
        }
        self.status = ProposalStatus::Executed;
        Ok(())
    }
}

pub struct GovernanceVoting {
    proposals: HashMap<u64, Proposal>,
    next_id: u64,
}

impl GovernanceVoting {
    pub fn new() -> Self {
        Self {
            proposals: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn create_proposal(
        &mut self,
        description: String,
        proposer: String,
        quorum: u64,
        deadline_secs: u64,
        now: u64,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let proposal = Proposal::new(id, description, proposer, quorum, deadline_secs, now);
        self.proposals.insert(id, proposal);
        id
    }

    pub fn vote(
        &mut self,
        proposal_id: u64,
        voter: &str,
        in_favor: bool,
        now: u64,
    ) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or("proposal not found")?;
        proposal.cast_vote(voter, in_favor, now)
    }

    pub fn finalize(&mut self, proposal_id: u64, now: u64) -> Result<ProposalStatus, String> {
        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or("proposal not found")?;
        proposal.finalize(now);
        Ok(proposal.status.clone())
    }

    pub fn execute_proposal(&mut self, proposal_id: u64) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or("proposal not found")?;
        proposal.execute()
    }

    pub fn get(&self, proposal_id: u64) -> Option<&Proposal> {
        self.proposals.get(&proposal_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vote_and_pass() {
        let mut gov = GovernanceVoting::new();
        let id = gov.create_proposal(
            "Enable fee discount".to_string(),
            "alice".to_string(),
            2,
            3600,
            0,
        );
        gov.vote(id, "alice", true, 100).unwrap();
        gov.vote(id, "bob", true, 200).unwrap();
        let status = gov.finalize(id, 300).unwrap();
        assert_eq!(status, ProposalStatus::Passed);
    }

    #[test]
    fn test_duplicate_vote_rejected() {
        let mut gov = GovernanceVoting::new();
        let id = gov.create_proposal("Change quorum".to_string(), "alice".to_string(), 1, 3600, 0);
        gov.vote(id, "alice", true, 100).unwrap();
        assert!(gov.vote(id, "alice", true, 200).is_err());
    }

    #[test]
    fn test_expired_proposal() {
        let mut gov = GovernanceVoting::new();
        let id = gov.create_proposal("Expired prop".to_string(), "alice".to_string(), 5, 60, 0);
        assert!(gov.vote(id, "alice", true, 120).is_err());
    }
}
