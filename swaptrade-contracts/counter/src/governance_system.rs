use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec, Map};

use crate::errors::SwapTradeError;
use crate::storage::{PROPOSALS_KEY, PROPOSAL_STATE_KEY, TOTAL_SUPPLY_KEY, BALANCES_KEY, GOV_COUNCIL_KEY};
use crate::governance::quadratic_voting::{self, Vote};
use crate::governance::multi_sig::MultiSig;
use crate::governance_params::{GovernanceParams, ParamKey};
use crate::governance::delegation;
use crate::governance_types;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalAction {
    PauseTrading,
    ResumeTrading,
    SetAdmin(Address),
    SetTreasury(Address),
    UpdatePoolFeeTier(u64, u32),
    UpdateGovParam(ParamKey, i128),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub action: ProposalAction,
    pub created_at: u64,
    pub created_by: Address,
    pub executed: bool,
    pub canceled: bool,
    pub executable_at: u64,
    pub multi_sig: MultiSig,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalState {
    pub votes: Vec<Vote>,
}

pub fn create_proposal(
    env: &Env,
    caller: Address,
    action: ProposalAction,
) -> Result<u64, SwapTradeError> {
    caller.require_auth();

    let total_supply: u64 = env.storage().persistent().get(&TOTAL_SUPPLY_KEY).unwrap_or(0);
    let balance = get_token_balance(env, &caller);

    if balance < total_supply / 100 { // 1% of total supply
        return Err(SwapTradeError::InsufficientBalance);
    }

    let mut proposals: Map<u64, Proposal> = env
        .storage()
        .persistent()
        .get(&PROPOSALS_KEY)
        .unwrap_or_else(|| Map::new(env));

    let signers: Vec<Address> = Vec::new(env);
    let multi_sig = MultiSig {
        signers,
        threshold: 5,
    };

    let proposal_id = proposals.len() as u64;
    let proposal = Proposal {
        id: proposal_id,
        action,
        created_at: env.ledger().timestamp(),
        created_by: caller,
        executed: false,
        canceled: false,
        executable_at: 0, // Set to 0 initially, updated when the proposal passes
        multi_sig,
    };

    proposals.set(proposal_id, proposal.clone());
    env.storage().persistent().set(&PROPOSALS_KEY, &proposals);

    let mut proposal_state: Map<u64, ProposalState> = env
        .storage()
        .persistent()
        .get(&PROPOSAL_STATE_KEY)
        .unwrap_or_else(|| Map::new(env));

    proposal_state.set(
        proposal_id,
        ProposalState {
            votes: Vec::new(env),
        },
    );
    env.storage()
        .persistent()
        .set(&PROPOSAL_STATE_KEY, &proposal_state);

    env.events().publish(
        (symbol_short!("prop_new"), proposal_id),
        proposal,
    );

    Ok(proposal_id)
}

pub fn cast_vote(
    env: &Env,
    caller: Address,
    proposal_id: u64,
    in_favor: bool,
) -> Result<(), SwapTradeError> {
    caller.require_auth();

    let mut proposals: Map<u64, Proposal> =
        env.storage().persistent().get(&PROPOSALS_KEY).unwrap();
    let proposal = proposals
        .get(proposal_id)
        .ok_or(SwapTradeError::ProposalNotFound)?;

    if proposal.executed {
        return Err(SwapTradeError::ProposalAlreadyExecuted);
    }

    if proposal.executable_at > 0 {
        return Err(SwapTradeError::VotingEnded);
    }

    if proposal.canceled {
        return Err(SwapTradeError::ProposalCanceled);
    }

    let mut proposal_state: Map<u64, ProposalState> =
        env.storage().persistent().get(&PROPOSAL_STATE_KEY).unwrap();
    let mut state = proposal_state.get(proposal_id).unwrap();

    if state.votes.iter().any(|v| v.voter == caller) {
        return Err(SwapTradeError::AlreadyVoted);
    }

    let effective_voter = delegation::get_delegate(env, &caller).unwrap_or(caller.clone());

    let balance = get_token_balance(env, &effective_voter);
    let vote_weight = quadratic_voting::calculate_voting_power(balance);

    let vote = Vote {
        voter: caller.clone(),
        vote_weight: if in_favor { vote_weight as i64 } else { -(vote_weight as i64) },
    };

    state.votes.push_back(vote);
    proposal_state.set(proposal_id, state.clone());
    env.storage()
        .persistent()
        .set(&PROPOSAL_STATE_KEY, &proposal_state);

    let (votes_for, votes_against) = quadratic_voting::tally_votes(&state.votes);
    let total_supply: u64 = env.storage().persistent().get(&TOTAL_SUPPLY_KEY).unwrap_or(0);
    let total_votes = votes_for + votes_against;

    if total_votes >= total_supply * 30 / 100 && votes_for * 100 >= (votes_for + votes_against) * 60 {
        let mut proposals: Map<u64, Proposal> = env.storage().persistent().get(&PROPOSALS_KEY).unwrap();
        let mut proposal = proposals.get(proposal_id).unwrap();
        proposal.executable_at = env.ledger().timestamp() + 172800; // 48 hours
        proposals.set(proposal_id, proposal.clone());
        env.storage().persistent().set(&PROPOSALS_KEY, &proposals);
    }

    env.events()
        .publish((symbol_short!("prop_vote"), proposal_id), (caller, in_favor));

    Ok(())
}

pub fn sign_proposal(
    env: &Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), SwapTradeError> {
    caller.require_auth();

    let mut proposals: Map<u64, Proposal> =
        env.storage().persistent().get(&PROPOSALS_KEY).unwrap();
    let mut proposal = proposals
        .get(proposal_id)
        .ok_or(SwapTradeError::ProposalNotFound)?;

    if proposal.executed {
        return Err(SwapTradeError::ProposalAlreadyExecuted);
    }

    let gov_council: Vec<Address> = env.storage().persistent().get(&GOV_COUNCIL_KEY).unwrap();
    if !gov_council.contains(&caller) {
        return Err(SwapTradeError::NotInCouncil);
    }

    proposal.multi_sig.signers.push_back(caller.clone());
    proposals.set(proposal_id, proposal.clone());
    env.storage().persistent().set(&PROPOSALS_KEY, &proposals);

    env.events()
        .publish((symbol_short!("prop_sign"), proposal_id), caller);

    Ok(())
}

/// Multi-sig approval for a proposal (alias for sign_proposal).
pub fn approve_proposal(
    env: &Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), SwapTradeError> {
    sign_proposal(env, caller, proposal_id)
}

pub fn cancel_proposal(
    env: &Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), SwapTradeError> {
    caller.require_auth();

    let mut proposals: Map<u64, Proposal> =
        env.storage().persistent().get(&PROPOSALS_KEY).unwrap();
    let mut proposal = proposals
        .get(proposal_id)
        .ok_or(SwapTradeError::ProposalNotFound)?;

    if proposal.executed {
        return Err(SwapTradeError::ProposalAlreadyExecuted);
    }

    let gov_council: Vec<Address> = env.storage().persistent().get(&GOV_COUNCIL_KEY).unwrap();
    if !gov_council.contains(&caller) {
        return Err(SwapTradeError::NotInCouncil);
    }

    proposal.canceled = true;
    proposals.set(proposal_id, proposal.clone());
    env.storage().persistent().set(&PROPOSALS_KEY, &proposals);

    env.events()
        .publish((symbol_short!("prop_cncl"), proposal_id), caller);

    Ok(())
}

pub fn approve_proposal(
    env: &Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), SwapTradeError> {
    caller.require_auth();

    let mut proposals: Map<u64, Proposal> =
        env.storage().persistent().get(&PROPOSALS_KEY).unwrap();
    let mut proposal = proposals
        .get(proposal_id)
        .ok_or(SwapTradeError::ProposalNotFound)?;

    if proposal.executed {
        return Err(SwapTradeError::ProposalAlreadyExecuted);
    }

    // Only authorized multisig signers may approve
    let config = crate::admin::get_multi_sig_config(env)?;
    if !config.signers.contains(&caller) {
        return Err(SwapTradeError::NotAuthorized);
    }

    // Prevent duplicate approvals
    if proposal.multi_sig.signers.contains(&caller) {
        return Err(SwapTradeError::AlreadyApproved);
    }

    proposal.multi_sig.signers.push_back(caller.clone());
    proposals.set(proposal_id, proposal.clone());
    env.storage().persistent().set(&PROPOSALS_KEY, &proposals);

    env.events()
        .publish((symbol_short!("prop_approve"), proposal_id), caller);

    Ok(())
}

pub fn execute_proposal(
    env: &Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), SwapTradeError> {
    caller.require_auth();

    let mut proposals: Map<u64, Proposal> =
        env.storage().persistent().get(&PROPOSALS_KEY).unwrap();
    let mut proposal = proposals
        .get(proposal_id)
        .ok_or(SwapTradeError::ProposalNotFound)?;

    if proposal.executed {
        return Err(SwapTradeError::ProposalAlreadyExecuted);
    }

    if env.ledger().timestamp() < proposal.executable_at {
        return Err(SwapTradeError::TimelockNotElapsed);
    }

    if proposal.multi_sig.signers.len() < proposal.multi_sig.threshold {
        return Err(SwapTradeError::InsufficientSignatures);
    }

    let proposal_state: Map<u64, ProposalState> =
        env.storage().persistent().get(&PROPOSAL_STATE_KEY).unwrap();
    let state = proposal_state.get(proposal_id).unwrap();

    let (votes_for, votes_against) = quadratic_voting::tally_votes(&state.votes);

    let total_votes = votes_for + votes_against;
    let total_supply: u64 = env.storage().persistent().get(&TOTAL_SUPPLY_KEY).unwrap_or(0);

    if total_votes < total_supply * 30 / 100 { // 30% quorum
        return Err(SwapTradeError::QuorumNotReached);
    }

    if votes_for * 100 < (votes_for + votes_against) * 60 { // 60% approval
        return Err(SwapTradeError::ProposalFailed);
    }

    match proposal.action {
        ProposalAction::PauseTrading => {
            env.storage().persistent().set(&crate::storage::PAUSED_KEY, &true);
        }
        ProposalAction::ResumeTrading => {
            env.storage().persistent().set(&crate::storage::PAUSED_KEY, &false);
        }
        ProposalAction::SetAdmin(ref new_admin) => {
            env.storage().persistent().set(&crate::storage::ADMIN_KEY, &new_admin);
        }
        ProposalAction::SetTreasury(ref new_treasury) => {
            env.storage().persistent().set(&crate::storage::DEFAULT_TREASURY_KEY, &new_treasury);
        }
        ProposalAction::UpdatePoolFeeTier(pool_id, new_fee_tier) => {
            crate::update_pool_fee_tier(env.clone(), caller.clone(), pool_id, new_fee_tier)?
        }
        ProposalAction::UpdateGovParam(ref param, new_value) => {
            GovernanceParams::apply_param_update(env, param.clone(), new_value)?
        }
    };

    proposal.executed = true;
    proposals.set(proposal_id, proposal);
    env.storage().persistent().set(&PROPOSALS_KEY, &proposals);

    env.events()
        .publish((symbol_short!("prop_exec"), proposal_id), caller);

    Ok(())
}

fn get_token_balance(env: &Env, user: &Address) -> u64 {
    let balances: Map<Address, u64> = env.storage().persistent().get(&BALANCES_KEY).unwrap_or_else(|| Map::new(env));
    balances.get(user.clone()).unwrap_or(0)
}

// ── GovernanceSystem wrapper for lib.rs contract interface ─────────────────

pub struct GovernanceSystem;

impl GovernanceSystem {
    pub fn create_proposal(
        env: &Env,
        proposer: &Address,
        _proposal_type: governance_types::ProposalType,
        _description: Symbol,
        _voting_period: u64,
    ) -> Result<u64, SwapTradeError> {
        // Map governance_types::ProposalType to a ProposalAction
        let action = ProposalAction::PauseTrading; // default; real impl maps per variant
        create_proposal(env, proposer.clone(), action)
    }

    pub fn cast_vote(
        env: &Env,
        voter: &Address,
        proposal_id: u64,
        support: governance_types::VoteOption,
    ) -> Result<(), SwapTradeError> {
        let in_favor = matches!(support, governance_types::VoteOption::For);
        cast_vote(env, voter.clone(), proposal_id, in_favor)
    }

    pub fn execute_proposal(
        env: &Env,
        executor: &Address,
        proposal_id: u64,
    ) -> Result<(), SwapTradeError> {
        execute_proposal(env, executor.clone(), proposal_id)
    }

    pub fn get_proposal(
        env: &Env,
        proposal_id: u64,
    ) -> Result<governance_types::Proposal, SwapTradeError> {
        let proposals: Map<u64, Proposal> =
            env.storage().persistent().get(&PROPOSALS_KEY)
                .ok_or(SwapTradeError::ProposalNotFound)?;
        let p = proposals.get(proposal_id)
            .ok_or(SwapTradeError::ProposalNotFound)?;

        // Convert internal Proposal to governance_types::Proposal
        let status = if p.executed {
            governance_types::ProposalStatus::Executed
        } else if p.canceled {
            governance_types::ProposalStatus::Cancelled
        } else {
            governance_types::ProposalStatus::Active
        };

        Ok(governance_types::Proposal {
            id: p.id,
            proposer: p.created_by,
            proposal_type: governance_types::ProposalType::Custom(
                Symbol::new(env, "action"),
                Symbol::new(env, "details"),
            ),
            description: Symbol::new(env, "proposal"),
            start_time: p.created_at,
            end_time: p.executable_at,
            execution_time: if p.executed { Some(p.executable_at) } else { None },
            status,
            votes_for: 0,
            votes_against: 0,
            votes_abstain: 0,
            total_voting_power: 0,
            quorum_required: 0,
            approval_threshold: 5000,
            executed: p.executed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin::set_admin;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    fn setup() -> (Env, Address, Address, Vec<Address>) {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, crate::CounterContract);
        let admin = Address::generate(&env);

        let mut users: Vec<Address> = Vec::new(&env);
        let mut balances: Map<Address, u64> = Map::new(&env);
        let mut total_supply: u64 = 0;

        for i in 0..10 {
            let user = Address::generate(&env);
            let balance = 100 * (i + 1);
            balances.set(user.clone(), balance);
            total_supply += balance;
            users.push_back(user);
        }

        let mut gov_council: Vec<Address> = Vec::new(&env);
        for _ in 0..7 {
            gov_council.push_back(Address::generate(&env));
        }

        env.as_contract(&contract_id, || {
            set_admin(&env, &admin);
            env.storage().persistent().set(&BALANCES_KEY, &balances);
            env.storage().persistent().set(&TOTAL_SUPPLY_KEY, &total_supply);
            env.storage().persistent().set(&GOV_COUNCIL_KEY, &gov_council);
        });

        (env, contract_id, admin, users)
    }

    #[test]
    fn test_create_proposal() {
        let (env, contract_id, admin, users) = setup();
        env.as_contract(&contract_id, || {
            let id = create_proposal(&env, users.get(9).unwrap(), ProposalAction::PauseTrading).unwrap();
            assert_eq!(id, 0);
        });
    }

    #[test]
    fn test_cast_vote() {
        let (env, contract_id, admin, users) = setup();
        env.as_contract(&contract_id, || {
            let id = create_proposal(&env, users.get(9).unwrap(), ProposalAction::PauseTrading).unwrap();
            cast_vote(&env, users.get(0).unwrap(), id, true).unwrap();
        });
    }

    #[test]
    fn test_execute_proposal() {
        let (env, contract_id, admin, users) = setup();
        env.as_contract(&contract_id, || {
            let id = create_proposal(&env, users.get(9).unwrap(), ProposalAction::PauseTrading).unwrap();

            for i in 0..7 {
                cast_vote(&env, users.get(i).unwrap(), id, true).unwrap();
            }

            for i in 7..10 {
                cast_vote(&env, users.get(i).unwrap(), id, false).unwrap();
            }

            let mut proposals: Map<u64, Proposal> = env.storage().persistent().get(&PROPOSALS_KEY).unwrap();
            let mut proposal = proposals.get(id).unwrap();

            for i in 0..5 {
                proposal.multi_sig.signers.push_back(users.get(i).unwrap());
            }
            proposals.set(id, proposal.clone());
            env.storage().persistent().set(&PROPOSALS_KEY, &proposals);

            env.ledger().with_mut(|l| {
                l.timestamp = env.ledger().timestamp() + 172800 + 1;
            });

            execute_proposal(&env, admin.clone(), id).unwrap();
        });
    }
}