// governance/rewards.rs
//
// Governance Reward Distribution
//
// Distributes governance rewards to active participants to incentivize
// community engagement and responsible governance.
//
// Capabilities:
//   - Time-weighted reward calculation based on participation
//   - Proposal creation bonuses
//   - Voting participation rewards
//   - Delegation rewards
//   - Epoch-based reward distribution
//   - Anti-gaming measures (cooldowns, diminishing returns)
//   - Reward claiming with verification

use std::collections::HashMap;

// ─── Types ────────────────────────────────────────────────────────────────────

/// Unique identifier for reward epochs.
pub type EpochId = u64;

/// Participant address.
pub type ParticipantId = String;

/// Reward token amount.
pub type RewardAmount = u128;

/// Participation record for a user in an epoch.
#[derive(Debug, Clone, Default)]
pub struct ParticipationRecord {
    /// Number of proposals created.
    pub proposals_created: u32,
    /// Number of votes cast.
    pub votes_cast: u32,
    /// Number of votes where the participant was a delegate.
    pub delegated_votes_received: u32,
    /// Total voting power used.
    pub total_voting_power_used: u128,
    /// Whether the participant voted on all eligible proposals.
    pub full_participation: bool,
    /// Timestamp of first action in epoch.
    pub first_action_at: u64,
    /// Timestamp of last action in epoch.
    pub last_action_at: u64,
}

impl ParticipationRecord {
    /// Compute a participation score (0-100).
    pub fn score(&self) -> u32 {
        let mut score: u32 = 0;

        // Voting participation (up to 50 points)
        score = score.saturating_add(self.votes_cast.saturating_mul(10).min(50));

        // Proposal creation (up to 30 points)
        score = score.saturating_add(self.proposals_created.saturating_mul(15).min(30));

        // Delegation activity (up to 10 points)
        score = score.saturating_add(self.delegated_votes_received.saturating_mul(5).min(10));

        // Full participation bonus (10 points)
        if self.full_participation {
            score = score.saturating_add(10);
        }

        score.min(100)
    }
}

/// Configuration for the reward system.
#[derive(Debug, Clone)]
pub struct RewardConfig {
    /// Total reward pool per epoch.
    pub epoch_reward_pool: RewardAmount,
    /// Duration of each epoch in seconds.
    pub epoch_duration_secs: u64,
    /// Cooldown between claim attempts per participant.
    pub claim_cooldown_secs: u64,
    /// Maximum reward per participant per epoch (caps gaming).
    pub max_reward_per_participant: RewardAmount,
    /// Base reward for creating a proposal.
    pub proposal_creation_bonus: RewardAmount,
    /// Base reward for casting a vote.
    pub vote_cast_reward: RewardAmount,
    /// Multiplier for voting with higher power (basis points, 100 = 1x).
    pub voting_power_multiplier_bps: u32,
    /// Minimum participation score to qualify for rewards.
    pub min_qualification_score: u32,
    /// Diminishing returns factor (basis points, 100 = no diminishing).
    pub diminishing_returns_bps: u32,
}

impl Default for RewardConfig {
    fn default() -> Self {
        Self {
            epoch_reward_pool: 100_000,
            epoch_duration_secs: 604800, // 7 days
            claim_cooldown_secs: 3600,
            max_reward_per_participant: 10_000,
            proposal_creation_bonus: 500,
            vote_cast_reward: 100,
            voting_power_multiplier_bps: 100,
            min_qualification_score: 10,
            diminishing_returns_bps: 80, // 80% efficiency after first epoch
        }
    }
}

/// An epoch containing participation data and reward distribution.
#[derive(Debug, Clone)]
pub struct Epoch {
    pub id: EpochId,
    pub start_time: u64,
    pub end_time: u64,
    /// Total reward pool allocated.
    pub reward_pool: RewardAmount,
    /// Rewards claimed so far.
    pub rewards_distributed: RewardAmount,
    /// Participation records per participant.
    pub participation: HashMap<ParticipantId, ParticipationRecord>,
    /// Whether the epoch is finalized.
    pub finalized: bool,
}

impl Epoch {
    pub fn total_participants(&self) -> usize {
        self.participation.len()
    }

    pub fn total_votes_cast(&self) -> u32 {
        self.participation.values().map(|r| r.votes_cast).sum()
    }

    pub fn total_proposals_created(&self) -> u32 {
        self.participation
            .values()
            .map(|r| r.proposals_created)
            .sum()
    }
}

/// Claim record for audit trail.
#[derive(Debug, Clone)]
pub struct ClaimRecord {
    pub participant: ParticipantId,
    pub epoch_id: EpochId,
    pub amount: RewardAmount,
    pub claimed_at: u64,
    pub participation_score: u32,
}

// ─── Reward Manager ───────────────────────────────────────────────────────────

pub struct RewardManager {
    /// Reward configuration.
    config: RewardConfig,
    /// Historical and current epochs.
    epochs: HashMap<EpochId, Epoch>,
    /// Current epoch ID.
    current_epoch_id: EpochId,
    /// When the current epoch started.
    current_epoch_start: u64,
    /// Claim history per participant (epoch_id -> last claim timestamp).
    last_claim: HashMap<ParticipantId, u64>,
    /// Total rewards distributed across all epochs.
    total_rewards_distributed: RewardAmount,
    /// Historical claim records.
    claim_history: Vec<ClaimRecord>,
    /// All-time participation counts per participant.
    all_time_participation: HashMap<ParticipantId, AllTimeStats>,
}

#[derive(Debug, Clone, Default)]
pub struct AllTimeStats {
    pub epochs_participated: u32,
    pub total_rewards_claimed: RewardAmount,
    pub total_proposals_created: u32,
    pub total_votes_cast: u32,
}

impl RewardManager {
    pub fn new(config: RewardConfig, start_time: u64) -> Self {
        let mut epochs = HashMap::new();
        let first_epoch = Epoch {
            id: 1,
            start_time,
            end_time: start_time + config.epoch_duration_secs,
            reward_pool: config.epoch_reward_pool,
            rewards_distributed: 0,
            participation: HashMap::new(),
            finalized: false,
        };
        epochs.insert(1, first_epoch);

        Self {
            config,
            epochs,
            current_epoch_id: 1,
            current_epoch_start: start_time,
            last_claim: HashMap::new(),
            total_rewards_distributed: 0,
            claim_history: Vec::new(),
            all_time_participation: HashMap::new(),
        }
    }

    // ── Participation Tracking ───────────────────────────────────────────────

    /// Record a proposal creation.
    pub fn record_proposal_created(&mut self, participant: &str, now: u64) -> Result<(), String> {
        self.ensure_current_epoch(now)?;

        let epoch = self
            .epochs
            .get_mut(&self.current_epoch_id)
            .ok_or("epoch not found")?;

        let record = epoch
            .participation
            .entry(participant.to_string())
            .or_insert_with(ParticipationRecord::default);

        record.proposals_created = record.proposals_created.saturating_add(1);
        if record.first_action_at == 0 {
            record.first_action_at = now;
        }
        record.last_action_at = now;

        // Update all-time stats
        let stats = self
            .all_time_participation
            .entry(participant.to_string())
            .or_insert_with(AllTimeStats::default);
        stats.total_proposals_created = stats.total_proposals_created.saturating_add(1);

        Ok(())
    }

    /// Record a vote cast.
    pub fn record_vote_cast(
        &mut self,
        participant: &str,
        voting_power: u128,
        now: u64,
    ) -> Result<(), String> {
        self.ensure_current_epoch(now)?;

        let epoch = self
            .epochs
            .get_mut(&self.current_epoch_id)
            .ok_or("epoch not found")?;

        let record = epoch
            .participation
            .entry(participant.to_string())
            .or_insert_with(ParticipationRecord::default);

        record.votes_cast = record.votes_cast.saturating_add(1);
        record.total_voting_power_used =
            record.total_voting_power_used.saturating_add(voting_power);
        if record.first_action_at == 0 {
            record.first_action_at = now;
        }
        record.last_action_at = now;

        // Update all-time stats
        let stats = self
            .all_time_participation
            .entry(participant.to_string())
            .or_insert_with(AllTimeStats::default);
        stats.total_votes_cast = stats.total_votes_cast.saturating_add(1);

        Ok(())
    }

    /// Record delegation activity.
    pub fn record_delegation_received(&mut self, delegate: &str, now: u64) -> Result<(), String> {
        self.ensure_current_epoch(now)?;

        let epoch = self
            .epochs
            .get_mut(&self.current_epoch_id)
            .ok_or("epoch not found")?;

        let record = epoch
            .participation
            .entry(delegate.to_string())
            .or_insert_with(ParticipationRecord::default);

        record.delegated_votes_received = record.delegated_votes_received.saturating_add(1);
        if record.first_action_at == 0 {
            record.first_action_at = now;
        }
        record.last_action_at = now;

        Ok(())
    }

    // ── Epoch Management ─────────────────────────────────────────────────────

    fn ensure_current_epoch(&mut self, now: u64) -> Result<(), String> {
        let epoch = self
            .epochs
            .get(&self.current_epoch_id)
            .ok_or("epoch not found")?;

        if now >= epoch.end_time {
            // Finalize current epoch and start new one
            self.finalize_epoch(self.current_epoch_id, now)?;
        }
        Ok(())
    }

    /// Finalize an epoch, computing final scores.
    pub fn finalize_epoch(&mut self, epoch_id: EpochId, now: u64) -> Result<(), String> {
        let epoch = self.epochs.get_mut(&epoch_id).ok_or("epoch not found")?;

        if epoch.finalized {
            return Err("epoch already finalized".to_string());
        }

        // Mark full participation for users who voted on everything
        // (simplified: check if they cast at least 1 vote)
        for record in epoch.participation.values_mut() {
            if record.votes_cast > 0 && record.proposals_created > 0 {
                record.full_participation = true;
            }
        }

        epoch.finalized = true;

        // Start new epoch
        let new_id = epoch_id + 1;
        let new_epoch = Epoch {
            id: new_id,
            start_time: now,
            end_time: now + self.config.epoch_duration_secs,
            reward_pool: self.config.epoch_reward_pool,
            rewards_distributed: 0,
            participation: HashMap::new(),
            finalized: false,
        };
        self.epochs.insert(new_id, new_epoch);
        self.current_epoch_id = new_id;
        self.current_epoch_start = now;

        Ok(())
    }

    // ── Reward Calculation ───────────────────────────────────────────────────

    /// Calculate the reward for a participant in a given epoch.
    pub fn calculate_reward(&self, participant: &str, epoch_id: EpochId) -> RewardAmount {
        let epoch = match self.epochs.get(&epoch_id) {
            Some(e) => e,
            None => return 0,
        };

        let record = match epoch.participation.get(participant) {
            Some(r) => r,
            None => return 0,
        };

        let score = record.score();
        if score < self.config.min_qualification_score {
            return 0;
        }

        // Base reward proportional to score
        let base_reward = (epoch.reward_pool * score as u128) / 100;

        // Apply participation bonuses
        let proposal_bonus = record.proposals_created as u128 * self.config.proposal_creation_bonus;
        let vote_bonus = record.votes_cast as u128 * self.config.vote_cast_reward;

        // Apply voting power multiplier
        let power_multiplier = (self.config.voting_power_multiplier_bps as u128).min(200); // Cap at 2x
        let power_bonus = (record.total_voting_power_used * power_multiplier) / 10_000;

        let mut total = base_reward
            .saturating_add(proposal_bonus)
            .saturating_add(vote_bonus)
            .saturating_add(power_bonus);

        // Apply diminishing returns for repeat participants
        let stats = self.all_time_participation.get(participant);
        if let Some(stats) = stats {
            if stats.epochs_participated > 0 {
                let efficiency = self.config.diminishing_returns_bps as u128;
                let factor = efficiency.pow(stats.epochs_participated.min(5) as u32);
                let divisor = 100u128.pow(stats.epochs_participated.min(5) as u32);
                total = (total * factor) / divisor;
            }
        }

        // Cap at maximum
        total.min(self.config.max_reward_per_participant)
    }

    // ── Claiming ─────────────────────────────────────────────────────────────

    /// Claim rewards for a finalized epoch.
    pub fn claim_rewards(
        &mut self,
        participant: &str,
        epoch_id: EpochId,
        now: u64,
    ) -> Result<RewardAmount, String> {
        let epoch = self.epochs.get(&epoch_id).ok_or("epoch not found")?;

        if !epoch.finalized {
            return Err("epoch is not yet finalized".to_string());
        }

        // Check cooldown
        if let Some(last) = self.last_claim.get(participant) {
            if now < last + self.config.claim_cooldown_secs {
                return Err("claim cooldown not elapsed".to_string());
            }
        }

        let amount = self.calculate_reward(participant, epoch_id);
        if amount == 0 {
            return Err("no rewards to claim".to_string());
        }

        // Check remaining pool
        let epoch = self.epochs.get_mut(&epoch_id).ok_or("epoch not found")?;
        if epoch.rewards_distributed + amount > epoch.reward_pool {
            return Err("insufficient reward pool".to_string());
        }

        epoch.rewards_distributed = epoch.rewards_distributed.saturating_add(amount);
        self.total_rewards_distributed = self.total_rewards_distributed.saturating_add(amount);
        self.last_claim.insert(participant.to_string(), now);

        // Update all-time stats
        let stats = self
            .all_time_participation
            .entry(participant.to_string())
            .or_insert_with(AllTimeStats::default);
        stats.epochs_participated = stats.epochs_participated.saturating_add(1);
        stats.total_rewards_claimed = stats.total_rewards_claimed.saturating_add(amount);

        let record = epoch.participation.get(participant);
        let score = record.map(|r| r.score()).unwrap_or(0);

        self.claim_history.push(ClaimRecord {
            participant: participant.to_string(),
            epoch_id,
            amount,
            claimed_at: now,
            participation_score: score,
        });

        Ok(amount)
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    pub fn current_epoch_id(&self) -> EpochId {
        self.current_epoch_id
    }

    pub fn get_epoch(&self, epoch_id: EpochId) -> Option<&Epoch> {
        self.epochs.get(&epoch_id)
    }

    pub fn total_distributed(&self) -> RewardAmount {
        self.total_rewards_distributed
    }

    pub fn participation_record(
        &self,
        participant: &str,
        epoch_id: EpochId,
    ) -> Option<&ParticipationRecord> {
        self.epochs.get(&epoch_id)?.participation.get(participant)
    }

    pub fn all_time_stats(&self, participant: &str) -> Option<&AllTimeStats> {
        self.all_time_participation.get(participant)
    }

    pub fn claim_history_length(&self) -> usize {
        self.claim_history.len()
    }

    pub fn pending_reward(&self, participant: &str, epoch_id: EpochId) -> RewardAmount {
        self.calculate_reward(participant, epoch_id)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> RewardManager {
        let config = RewardConfig {
            epoch_reward_pool: 100_000,
            epoch_duration_secs: 604800,
            claim_cooldown_secs: 60,
            max_reward_per_participant: 10_000,
            proposal_creation_bonus: 500,
            vote_cast_reward: 100,
            voting_power_multiplier_bps: 100,
            min_qualification_score: 10,
            diminishing_returns_bps: 100, // No diminishing for tests
        };
        RewardManager::new(config, 0)
    }

    #[test]
    fn test_initial_epoch() {
        let mgr = setup();
        assert_eq!(mgr.current_epoch_id(), 1);
        let epoch = mgr.get_epoch(1).unwrap();
        assert_eq!(epoch.reward_pool, 100_000);
        assert!(!epoch.finalized);
    }

    #[test]
    fn test_record_proposal_created() {
        let mut mgr = setup();
        mgr.record_proposal_created("alice", 100).unwrap();

        let record = mgr.participation_record("alice", 1).unwrap();
        assert_eq!(record.proposals_created, 1);
        assert_eq!(record.first_action_at, 100);
    }

    #[test]
    fn test_record_vote_cast() {
        let mut mgr = setup();
        mgr.record_vote_cast("bob", 500, 200).unwrap();

        let record = mgr.participation_record("bob", 1).unwrap();
        assert_eq!(record.votes_cast, 1);
        assert_eq!(record.total_voting_power_used, 500);
    }

    #[test]
    fn test_participation_score() {
        let mut record = ParticipationRecord::default();
        record.votes_cast = 5;
        record.proposals_created = 2;
        record.delegated_votes_received = 1;
        record.full_participation = true;

        // 50 (votes) + 30 (proposals) + 5 (delegation) + 10 (full) = 95
        assert_eq!(record.score(), 95);
    }

    #[test]
    fn test_participation_score_low() {
        let mut record = ParticipationRecord::default();
        record.votes_cast = 1;

        // 10 (vote) = 10
        assert_eq!(record.score(), 10);
    }

    #[test]
    fn test_calculate_reward() {
        let mut mgr = setup();

        // Alice: 5 votes, 2 proposals = high participation
        for i in 0..5 {
            mgr.record_vote_cast("alice", 100, 100 + i).unwrap();
        }
        mgr.record_proposal_created("alice", 100).unwrap();
        mgr.record_proposal_created("alice", 110).unwrap();

        // Finalize epoch
        mgr.finalize_epoch(1, 700000).unwrap();

        let reward = mgr.calculate_reward("alice", 1);
        assert!(reward > 0);
        assert!(reward <= 10_000); // Within cap
    }

    #[test]
    fn test_claim_rewards() {
        let mut mgr = setup();

        mgr.record_vote_cast("alice", 200, 100).unwrap();
        mgr.record_proposal_created("alice", 100).unwrap();
        mgr.finalize_epoch(1, 700000).unwrap();

        let amount = mgr.claim_rewards("alice", 1, 700001).unwrap();
        assert!(amount > 0);
        assert_eq!(mgr.total_distributed(), amount);
    }

    #[test]
    fn test_claim_before_finalization_fails() {
        let mut mgr = setup();
        mgr.record_vote_cast("alice", 200, 100).unwrap();

        let result = mgr.claim_rewards("alice", 1, 200);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not yet finalized"));
    }

    #[test]
    fn test_claim_cooldown() {
        let mut mgr = setup();
        mgr.record_vote_cast("alice", 200, 100).unwrap();
        mgr.finalize_epoch(1, 700000).unwrap();

        mgr.claim_rewards("alice", 1, 700001).unwrap();

        // Try again immediately
        let result = mgr.claim_rewards("alice", 1, 700002);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cooldown"));

        // After cooldown
        let amount = mgr.claim_rewards("alice", 1, 700001 + 61).unwrap();
        assert!(amount > 0);
    }

    #[test]
    fn test_no_rewards_without_participation() {
        let mut mgr = setup();
        mgr.finalize_epoch(1, 700000).unwrap();

        let result = mgr.claim_rewards("nobody", 1, 700001);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no rewards"));
    }

    #[test]
    fn test_epoch_transition() {
        let mut mgr = setup();

        mgr.record_vote_cast("alice", 100, 100).unwrap();

        // Force epoch transition by ticking past end
        let epoch = mgr.get_epoch(1).unwrap();
        let end_time = epoch.end_time;

        mgr.ensure_current_epoch(end_time + 1).unwrap();
        assert_eq!(mgr.current_epoch_id(), 2);

        // Previous epoch finalized
        assert!(mgr.get_epoch(1).unwrap().finalized);
    }

    #[test]
    fn test_all_time_stats() {
        let mut mgr = setup();

        mgr.record_proposal_created("alice", 100).unwrap();
        mgr.record_vote_cast("alice", 100, 100).unwrap();
        mgr.finalize_epoch(1, 700000).unwrap();
        mgr.claim_rewards("alice", 1, 700001).unwrap();

        let stats = mgr.all_time_stats("alice").unwrap();
        assert_eq!(stats.epochs_participated, 1);
        assert!(stats.total_rewards_claimed > 0);
        assert_eq!(stats.total_proposals_created, 1);
        assert_eq!(stats.total_votes_cast, 1);
    }

    #[test]
    fn test_pending_reward() {
        let mut mgr = setup();

        assert_eq!(mgr.pending_reward("alice", 1), 0);

        mgr.record_vote_cast("alice", 200, 100).unwrap();
        let pending = mgr.pending_reward("alice", 1);
        assert!(pending > 0);
    }

    #[test]
    fn test_delegation_recorded() {
        let mut mgr = setup();

        mgr.record_delegation_received("bob_delegate", 100).unwrap();
        let record = mgr.participation_record("bob_delegate", 1).unwrap();
        assert_eq!(record.delegated_votes_received, 1);
    }

    #[test]
    fn test_reward_cap() {
        let config = RewardConfig {
            max_reward_per_participant: 500,
            epoch_reward_pool: 1_000_000,
            ..RewardConfig::default()
        };
        let mut mgr = RewardManager::new(config, 0);

        // Maximum participation
        for i in 0..20 {
            mgr.record_vote_cast("alice", 10_000, 100 + i).unwrap();
        }
        for i in 0..10 {
            mgr.record_proposal_created("alice", 100 + i).unwrap();
        }
        mgr.finalize_epoch(1, 700000).unwrap();

        let reward = mgr.calculate_reward("alice", 1);
        assert!(reward <= 500);
    }

    #[test]
    fn test_claim_history() {
        let mut mgr = setup();

        mgr.record_vote_cast("alice", 200, 100).unwrap();
        mgr.finalize_epoch(1, 700000).unwrap();
        mgr.claim_rewards("alice", 1, 700001).unwrap();

        assert_eq!(mgr.claim_history_length(), 1);
    }

    #[test]
    fn test_full_participation_bonus() {
        let mut record = ParticipationRecord::default();
        record.votes_cast = 3;
        record.proposals_created = 1;
        record.full_participation = true;

        let score_with = record.score();

        record.full_participation = false;
        let score_without = record.score();

        assert_eq!(score_with, score_without + 10);
    }

    #[test]
    fn test_multiple_participants() {
        let config = RewardConfig {
            epoch_reward_pool: 10_000,
            max_reward_per_participant: 5_000,
            diminishing_returns_bps: 100,
            ..RewardConfig::default()
        };
        let mut mgr = RewardManager::new(config, 0);

        mgr.record_vote_cast("alice", 500, 100).unwrap();
        mgr.record_proposal_created("alice", 100).unwrap();
        mgr.record_vote_cast("bob", 300, 200).unwrap();
        mgr.record_vote_cast("carol", 100, 300).unwrap();

        mgr.finalize_epoch(1, 700000).unwrap();

        let alice_reward = mgr.claim_rewards("alice", 1, 700001).unwrap();
        let bob_reward = mgr.claim_rewards("bob", 1, 700002).unwrap();

        // Alice should get more due to proposal creation and higher voting power
        assert!(alice_reward > bob_reward);
    }
}
