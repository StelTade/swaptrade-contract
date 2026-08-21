// governance/treasury.rs
//
// Treasury Management Module
//
// Secure fund storage with multi-sig controls for the SwapTrade DAO.
// All critical treasury operations require multi-signature approval.
//
// Capabilities:
//   - Multi-asset fund storage with per-asset accounting
//   - Multi-sig withdrawal proposals requiring threshold approval
//   - Governance-controlled deposit allocation
//   - Spending limits with per-period caps
//   - Complete audit trail of all treasury movements
//   - Budget proposals for recurring allocations

use std::collections::HashMap;

// ─── Types ────────────────────────────────────────────────────────────────────

/// Identifier for a treasury asset (simplified as string for non-Soroban context).
pub type AssetId = String;

/// A unique identifier for treasury operations.
pub type OperationId = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreasuryOperationStatus {
    /// Proposal created, collecting signatures.
    Pending,
    /// Threshold reached, ready for execution.
    Approved,
    /// Operation executed successfully.
    Executed,
    /// Operation rejected or expired.
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalType {
    /// Standard withdrawal request.
    Withdrawal,
    /// Budget allocation for recurring payments.
    BudgetAllocation,
    /// Emergency withdrawal (reduced threshold).
    EmergencyWithdrawal,
    /// Grant or bounty payout.
    Grant,
}

#[derive(Debug, Clone)]
pub struct TreasuryProposal {
    pub id: OperationId,
    pub proposal_type: ProposalType,
    pub proposer: String,
    pub asset: AssetId,
    pub amount: u128,
    pub recipient: String,
    pub description: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub approvals: Vec<String>,
    pub required_approvals: usize,
    pub status: TreasuryOperationStatus,
    pub executed_at: Option<u64>,
}

impl TreasuryProposal {
    pub fn approval_count(&self) -> usize {
        self.approvals.len()
    }

    pub fn is_approved(&self) -> bool {
        self.approvals.len() >= self.required_approvals
    }

    pub fn is_expired(&self, now: u64) -> bool {
        now > self.expires_at
    }
}

/// Per-asset balance tracking in the treasury.
#[derive(Debug, Clone, Default)]
pub struct AssetBalance {
    pub balance: u128,
    pub total_deposited: u128,
    pub total_withdrawn: u128,
    pub last_deposit_at: u64,
    pub last_withdrawal_at: u64,
}

/// Spending limits per time period.
#[derive(Debug, Clone)]
pub struct SpendingLimit {
    /// Maximum amount that can be withdrawn within the period.
    pub max_per_period: u128,
    /// Duration of the spending period in seconds.
    pub period_duration_secs: u64,
    /// Amount already spent in the current period.
    pub spent_in_period: u128,
    /// Timestamp when the current period started.
    pub period_start: u64,
}

impl SpendingLimit {
    pub fn remaining(&self, now: u64) -> u128 {
        if now > self.period_start + self.period_duration_secs {
            // Period expired, full amount available
            self.max_per_period
        } else {
            self.max_per_period.saturating_sub(self.spent_in_period)
        }
    }

    pub fn record_spend(&mut self, amount: u128, now: u64) {
        if now > self.period_start + self.period_duration_secs {
            // Start new period
            self.period_start = now;
            self.spent_in_period = amount;
        } else {
            self.spent_in_period = self.spent_in_period.saturating_add(amount);
        }
    }
}

/// Audit log entry for treasury operations.
#[derive(Debug, Clone)]
pub struct TreasuryAuditEntry {
    pub seq: u64,
    pub timestamp: u64,
    pub operation: TreasuryAuditOperation,
    pub actor: String,
    pub entry_hash: [u8; 32],
}

#[derive(Debug, Clone)]
pub enum TreasuryAuditOperation {
    Deposit {
        asset: AssetId,
        amount: u128,
        from: String,
    },
    Withdrawal {
        asset: AssetId,
        amount: u128,
        to: String,
        proposal_id: String,
    },
    ProposalCreated {
        proposal_id: String,
        proposal_type: ProposalType,
    },
    ProposalApproved {
        proposal_id: String,
        approver: String,
        approval_count: usize,
    },
    ProposalRejected {
        proposal_id: String,
        reason: String,
    },
    SpendingLimitSet {
        asset: AssetId,
        max_per_period: u128,
    },
    MultiSigUpdated {
        old_threshold: usize,
        new_threshold: usize,
    },
    BalanceSnapshot {
        total_assets: usize,
    },
}

// ─── Treasury Contract ────────────────────────────────────────────────────────

pub struct Treasury {
    /// Per-asset balances.
    balances: HashMap<AssetId, AssetBalance>,
    /// Pending and historical proposals.
    proposals: HashMap<OperationId, TreasuryProposal>,
    /// Multi-sig signers.
    signers: Vec<String>,
    /// Required threshold for treasury operations.
    threshold: usize,
    /// Emergency threshold (reduced for critical operations).
    emergency_threshold: usize,
    /// Per-asset spending limits.
    spending_limits: HashMap<AssetId, SpendingLimit>,
    /// Complete audit trail.
    audit_log: Vec<TreasuryAuditEntry>,
    /// Monotonic sequence counter.
    seq: u64,
}

impl Treasury {
    /// Create a new treasury with the given signers and threshold.
    pub fn new(signers: Vec<String>, threshold: usize, emergency_threshold: usize) -> Self {
        assert!(
            threshold > 0 && threshold <= signers.len(),
            "threshold must be between 1 and signers.len()"
        );
        assert!(
            emergency_threshold > 0 && emergency_threshold <= threshold,
            "emergency threshold must be between 1 and normal threshold"
        );
        Self {
            balances: HashMap::new(),
            proposals: HashMap::new(),
            signers,
            threshold,
            emergency_threshold,
            spending_limits: HashMap::new(),
            audit_log: Vec::new(),
            seq: 0,
        }
    }

    // ── Signer Management ────────────────────────────────────────────────────

    pub fn add_signer(&mut self, signer: String) -> Result<(), String> {
        if self.signers.contains(&signer) {
            return Err("signer already exists".to_string());
        }
        self.signers.push(signer);
        Ok(())
    }

    pub fn remove_signer(&mut self, signer: &str) -> Result<(), String> {
        if self.signers.len() <= self.threshold {
            return Err("cannot remove signer: would drop below threshold".to_string());
        }
        self.signers.retain(|s| s != signer);
        Ok(())
    }

    pub fn update_threshold(&mut self, new_threshold: usize, now: u64) -> Result<(), String> {
        if new_threshold == 0 || new_threshold > self.signers.len() {
            return Err("invalid threshold".to_string());
        }
        let old = self.threshold;
        self.threshold = new_threshold;
        self.log_operation(
            TreasuryAuditOperation::MultiSigUpdated {
                old_threshold: old,
                new_threshold,
            },
            "system".to_string(),
            now,
        );
        Ok(())
    }

    pub fn is_signer(&self, signer: &str) -> bool {
        self.signers.contains(&signer.to_string())
    }

    // ── Balance Management ───────────────────────────────────────────────────

    /// Deposit funds into the treasury.
    pub fn deposit(
        &mut self,
        asset: &str,
        amount: u128,
        from: &str,
        now: u64,
    ) -> Result<(), String> {
        if amount == 0 {
            return Err("deposit amount must be greater than zero".to_string());
        }

        let entry = self
            .balances
            .entry(asset.to_string())
            .or_insert_with(AssetBalance::default);

        entry.balance = entry.balance.checked_add(amount).ok_or("overflow")?;
        entry.total_deposited = entry.total_deposited.saturating_add(amount);
        entry.last_deposit_at = now;

        self.log_operation(
            TreasuryAuditOperation::Deposit {
                asset: asset.to_string(),
                amount,
                from: from.to_string(),
            },
            from.to_string(),
            now,
        );

        Ok(())
    }

    /// Get the balance of a specific asset.
    pub fn balance_of(&self, asset: &str) -> u128 {
        self.balances.get(asset).map(|b| b.balance).unwrap_or(0)
    }

    /// Get the total value of all assets (simplified - counts unique assets).
    pub fn total_assets(&self) -> usize {
        self.balances.len()
    }

    // ── Spending Limits ──────────────────────────────────────────────────────

    /// Set a spending limit for an asset.
    pub fn set_spending_limit(
        &mut self,
        asset: &str,
        max_per_period: u128,
        period_duration_secs: u64,
        now: u64,
    ) {
        self.spending_limits.insert(
            asset.to_string(),
            SpendingLimit {
                max_per_period,
                period_duration_secs,
                spent_in_period: 0,
                period_start: now,
            },
        );
        self.log_operation(
            TreasuryAuditOperation::SpendingLimitSet {
                asset: asset.to_string(),
                max_per_period,
            },
            "admin".to_string(),
            now,
        );
    }

    /// Check if a withdrawal would exceed the spending limit.
    pub fn check_spending_limit(&self, asset: &str, amount: u128, now: u64) -> bool {
        match self.spending_limits.get(asset) {
            Some(limit) => amount <= limit.remaining(now),
            None => true, // No limit set
        }
    }

    // ── Proposal Management ──────────────────────────────────────────────────

    /// Create a treasury withdrawal proposal.
    pub fn create_proposal(
        &mut self,
        proposer: &str,
        proposal_type: ProposalType,
        asset: &str,
        amount: u128,
        recipient: &str,
        description: &str,
        now: u64,
        ttl_secs: u64,
    ) -> Result<OperationId, String> {
        if !self.is_signer(proposer) {
            return Err("only signers can create treasury proposals".to_string());
        }

        if amount == 0 {
            return Err("amount must be greater than zero".to_string());
        }

        let balance = self.balance_of(asset);
        if balance < amount {
            return Err(format!(
                "insufficient treasury balance: {} < {}",
                balance, amount
            ));
        }

        let required = match &proposal_type {
            ProposalType::EmergencyWithdrawal => self.emergency_threshold,
            _ => self.threshold,
        };

        // Compute proposal ID from content
        let mut id_bytes = [0u8; 32];
        let desc_bytes = description.as_bytes();
        let prop_bytes = proposer.as_bytes();
        for (i, b) in desc_bytes.iter().enumerate() {
            id_bytes[i % 32] ^= b;
        }
        for (i, b) in prop_bytes.iter().enumerate() {
            id_bytes[(i + desc_bytes.len()) % 32] ^= b;
        }
        id_bytes[0] ^= (amount & 0xFF) as u8;
        id_bytes[1] ^= ((amount >> 8) & 0xFF) as u8;
        id_bytes[2] ^= (now & 0xFF) as u8;

        let proposal = TreasuryProposal {
            id: id_bytes,
            proposal_type: proposal_type.clone(),
            proposer: proposer.to_string(),
            asset: asset.to_string(),
            amount,
            recipient: recipient.to_string(),
            description: description.to_string(),
            created_at: now,
            expires_at: now + ttl_secs,
            approvals: vec![proposer.to_string()], // Proposer auto-approves
            required_approvals: required,
            status: TreasuryOperationStatus::Pending,
            executed_at: None,
        };

        self.log_operation(
            TreasuryAuditOperation::ProposalCreated {
                proposal_id: hex_encode(&id_bytes),
                proposal_type,
            },
            proposer.to_string(),
            now,
        );

        self.proposals.insert(id_bytes, proposal);

        Ok(id_bytes)
    }

    /// Approve a treasury proposal.
    pub fn approve_proposal(
        &mut self,
        proposal_id: &OperationId,
        signer: &str,
        now: u64,
    ) -> Result<usize, String> {
        if !self.is_signer(signer) {
            return Err("only signers can approve treasury proposals".to_string());
        }

        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or("proposal not found")?;

        if proposal.status == TreasuryOperationStatus::Executed {
            return Err("proposal already executed".to_string());
        }

        if proposal.status == TreasuryOperationStatus::Rejected {
            return Err("proposal already rejected".to_string());
        }

        if proposal.is_expired(now) {
            proposal.status = TreasuryOperationStatus::Rejected;
            return Err("proposal has expired".to_string());
        }

        if proposal.approvals.contains(&signer.to_string()) {
            return Err("signer has already approved this proposal".to_string());
        }

        proposal.approvals.push(signer.to_string());
        let count = proposal.approval_count();

        if proposal.is_approved() {
            proposal.status = TreasuryOperationStatus::Approved;
        }

        self.log_operation(
            TreasuryAuditOperation::ProposalApproved {
                proposal_id: hex_encode(proposal_id),
                approver: signer.to_string(),
                approval_count: count,
            },
            signer.to_string(),
            now,
        );

        Ok(count)
    }

    /// Execute an approved treasury proposal.
    pub fn execute_proposal(&mut self, proposal_id: &OperationId, now: u64) -> Result<(), String> {
        // Validate and extract needed data first to avoid borrow conflicts
        let (asset, amount, recipient) = {
            let proposal = self
                .proposals
                .get(proposal_id)
                .ok_or("proposal not found")?;

            if proposal.status != TreasuryOperationStatus::Approved {
                return Err(format!(
                    "proposal not in approved state: {:?}",
                    proposal.status
                ));
            }

            if proposal.is_expired(now) {
                return Err("proposal has expired".to_string());
            }

            // Check spending limit
            if !self.check_spending_limit(&proposal.asset, proposal.amount, now) {
                return Err("withdrawal would exceed spending limit".to_string());
            }

            // Check balance
            let balance = self.balance_of(&proposal.asset);
            if balance < proposal.amount {
                return Err(format!(
                    "insufficient treasury balance: {} < {}",
                    balance, proposal.amount
                ));
            }

            (
                proposal.asset.clone(),
                proposal.amount,
                proposal.recipient.clone(),
            )
        };

        // Execute the withdrawal
        let entry = self
            .balances
            .get_mut(&asset)
            .ok_or("asset not found in treasury")?;

        entry.balance = entry.balance.saturating_sub(amount);
        entry.total_withdrawn = entry.total_withdrawn.saturating_add(amount);
        entry.last_withdrawal_at = now;

        // Record spending in limit tracker
        if let Some(limit) = self.spending_limits.get_mut(&asset) {
            limit.record_spend(amount, now);
        }

        // Update proposal status
        if let Some(proposal) = self.proposals.get_mut(proposal_id) {
            proposal.status = TreasuryOperationStatus::Executed;
            proposal.executed_at = Some(now);
        }

        self.log_operation(
            TreasuryAuditOperation::Withdrawal {
                asset,
                amount,
                to: recipient,
                proposal_id: hex_encode(proposal_id),
            },
            "system".to_string(),
            now,
        );

        Ok(())
    }

    /// Reject a treasury proposal.
    pub fn reject_proposal(
        &mut self,
        proposal_id: &OperationId,
        reason: &str,
        now: u64,
    ) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or("proposal not found")?;

        if proposal.status == TreasuryOperationStatus::Executed {
            return Err("cannot reject an executed proposal".to_string());
        }

        proposal.status = TreasuryOperationStatus::Rejected;

        self.log_operation(
            TreasuryAuditOperation::ProposalRejected {
                proposal_id: hex_encode(proposal_id),
                reason: reason.to_string(),
            },
            "system".to_string(),
            now,
        );

        Ok(())
    }

    // ── Audit Trail ──────────────────────────────────────────────────────────

    fn log_operation(
        &mut self,
        operation: TreasuryAuditOperation,
        actor: String,
        now: u64,
    ) -> [u8; 32] {
        self.seq += 1;
        let mut entry = TreasuryAuditEntry {
            seq: self.seq,
            timestamp: now,
            operation,
            actor,
            entry_hash: [0u8; 32],
        };

        // Compute a simple hash for chain integrity
        let mut hash_input = Vec::new();
        hash_input.extend_from_slice(&entry.seq.to_le_bytes());
        hash_input.extend_from_slice(&entry.timestamp.to_le_bytes());
        hash_input.extend_from_slice(entry.actor.as_bytes());
        if let Some(prev) = self.audit_log.last() {
            hash_input.extend_from_slice(&prev.entry_hash);
        }
        entry.entry_hash = simple_hash(&hash_input);

        let hash = entry.entry_hash;
        self.audit_log.push(entry);
        hash
    }

    /// Verify the integrity of the audit trail.
    pub fn verify_audit_trail(&self) -> bool {
        for (i, entry) in self.audit_log.iter().enumerate() {
            let mut hash_input = Vec::new();
            hash_input.extend_from_slice(&entry.seq.to_le_bytes());
            hash_input.extend_from_slice(&entry.timestamp.to_le_bytes());
            hash_input.extend_from_slice(entry.actor.as_bytes());
            if i > 0 {
                hash_input.extend_from_slice(&self.audit_log[i - 1].entry_hash);
            }
            if entry.entry_hash != simple_hash(&hash_input) {
                return false;
            }
        }
        true
    }

    /// Get the number of audit log entries.
    pub fn audit_log_length(&self) -> usize {
        self.audit_log.len()
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    /// Get a proposal by ID.
    pub fn get_proposal(&self, proposal_id: &OperationId) -> Option<&TreasuryProposal> {
        self.proposals.get(proposal_id)
    }

    /// Get all pending proposals.
    pub fn pending_proposals(&self, now: u64) -> Vec<&TreasuryProposal> {
        self.proposals
            .values()
            .filter(|p| {
                p.status != TreasuryOperationStatus::Executed
                    && p.status != TreasuryOperationStatus::Rejected
                    && !p.is_expired(now)
            })
            .collect()
    }

    /// Get the number of signers.
    pub fn signer_count(&self) -> usize {
        self.signers.len()
    }

    /// Get the current threshold.
    pub fn get_threshold(&self) -> usize {
        self.threshold
    }

    /// Get the emergency threshold.
    pub fn get_emergency_threshold(&self) -> usize {
        self.emergency_threshold
    }
}

// ─── Utility ──────────────────────────────────────────────────────────────────

fn simple_hash(data: &[u8]) -> [u8; 32] {
    // FNV-1a inspired simple hash for audit trail integrity
    let mut hash: [u8; 32] = [0x6b; 32]; // Init with a non-zero constant
    for (i, &byte) in data.iter().enumerate() {
        hash[i % 32] = hash[i % 32].wrapping_mul(31).wrapping_add(byte);
        hash[(i + 13) % 32] = hash[(i + 13) % 32]
            .wrapping_mul(17)
            .wrapping_add(byte ^ 0xAA);
    }
    hash
}

fn hex_encode(bytes: &[u8; 32]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Treasury {
        let signers = vec![
            "alice".to_string(),
            "bob".to_string(),
            "carol".to_string(),
            "dave".to_string(),
            "eve".to_string(),
        ];
        Treasury::new(signers, 3, 2)
    }

    #[test]
    fn test_treasury_creation() {
        let treasury = setup();
        assert_eq!(treasury.signer_count(), 5);
        assert_eq!(treasury.get_threshold(), 3);
        assert_eq!(treasury.get_emergency_threshold(), 2);
    }

    #[test]
    fn test_deposit() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();
        assert_eq!(treasury.balance_of("XLM"), 1_000_000);
    }

    #[test]
    fn test_zero_deposit_fails() {
        let mut treasury = setup();
        assert!(treasury.deposit("XLM", 0, "foundation", 100).is_err());
    }

    #[test]
    fn test_multiple_deposits() {
        let mut treasury = setup();
        treasury.deposit("XLM", 500_000, "foundation", 100).unwrap();
        treasury.deposit("XLM", 300_000, "donor", 200).unwrap();
        treasury
            .deposit("USDC", 100_000, "foundation", 300)
            .unwrap();

        assert_eq!(treasury.balance_of("XLM"), 800_000);
        assert_eq!(treasury.balance_of("USDC"), 100_000);
        assert_eq!(treasury.total_assets(), 2);
    }

    #[test]
    fn test_create_proposal() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recipient1",
                "Grant for community project",
                200,
                86400 * 7,
            )
            .unwrap();

        let proposal = treasury.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.proposal_type, ProposalType::Withdrawal);
        assert_eq!(proposal.amount, 100_000);
        assert_eq!(proposal.required_approvals, 3);
        assert_eq!(proposal.approval_count(), 1); // proposer auto-approves
    }

    #[test]
    fn test_non_signer_cannot_propose() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let result = treasury.create_proposal(
            "frank",
            ProposalType::Withdrawal,
            "XLM",
            100_000,
            "recipient1",
            "Test",
            200,
            86400 * 7,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("only signers"));
    }

    #[test]
    fn test_insufficient_balance_for_proposal() {
        let mut treasury = setup();
        treasury.deposit("XLM", 50_000, "foundation", 100).unwrap();

        let result = treasury.create_proposal(
            "alice",
            ProposalType::Withdrawal,
            "XLM",
            100_000,
            "recipient1",
            "Too much",
            200,
            86400 * 7,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("insufficient"));
    }

    #[test]
    fn test_approve_proposal() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recipient1",
                "Grant",
                200,
                86400 * 7,
            )
            .unwrap();

        // Bob approves
        let count = treasury.approve_proposal(&proposal_id, "bob", 300).unwrap();
        assert_eq!(count, 2);

        // Carol approves - reaches threshold
        let count = treasury
            .approve_proposal(&proposal_id, "carol", 300)
            .unwrap();
        assert_eq!(count, 3);

        // Proposal should now be approved
        let proposal = treasury.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.status, TreasuryOperationStatus::Approved);
    }

    #[test]
    fn test_duplicate_approval_fails() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recipient1",
                "Grant",
                200,
                86400 * 7,
            )
            .unwrap();

        // Alice (proposer) tries to approve again
        let result = treasury.approve_proposal(&proposal_id, "alice", 300);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already approved"));
    }

    #[test]
    fn test_execute_proposal() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recipient1",
                "Grant",
                200,
                86400 * 7,
            )
            .unwrap();

        treasury.approve_proposal(&proposal_id, "bob", 300).unwrap();
        treasury
            .approve_proposal(&proposal_id, "carol", 300)
            .unwrap();

        treasury.execute_proposal(&proposal_id, 400).unwrap();

        assert_eq!(treasury.balance_of("XLM"), 900_000);
        let proposal = treasury.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.status, TreasuryOperationStatus::Executed);
        assert_eq!(proposal.executed_at, Some(400));
    }

    #[test]
    fn test_execute_unapproved_proposal_fails() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recipient1",
                "Grant",
                200,
                86400 * 7,
            )
            .unwrap();

        // Only 1 approval (proposer), not enough for threshold
        let result = treasury.execute_proposal(&proposal_id, 300);
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_proposal_rejected() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recipient1",
                "Grant",
                200,
                60, // 60 second TTL
            )
            .unwrap();

        // Try to approve after expiration
        let result = treasury.approve_proposal(&proposal_id, "bob", 200 + 61);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[test]
    fn test_reject_proposal() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recipient1",
                "Grant",
                200,
                86400 * 7,
            )
            .unwrap();

        treasury
            .reject_proposal(&proposal_id, "not in budget", 300)
            .unwrap();

        let proposal = treasury.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.status, TreasuryOperationStatus::Rejected);
    }

    #[test]
    fn test_emergency_withdrawal_lower_threshold() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::EmergencyWithdrawal,
                "XLM",
                50_000,
                "security_team",
                "Emergency: vulnerability patch",
                200,
                86400,
            )
            .unwrap();

        let proposal = treasury.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.required_approvals, 2); // emergency threshold

        treasury.approve_proposal(&proposal_id, "bob", 300).unwrap();
        let proposal = treasury.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.status, TreasuryOperationStatus::Approved);

        treasury.execute_proposal(&proposal_id, 400).unwrap();
        assert_eq!(treasury.balance_of("XLM"), 950_000);
    }

    #[test]
    fn test_spending_limit() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 10_000_000, "foundation", 100)
            .unwrap();

        // Set limit: 500,000 XLM per day
        treasury.set_spending_limit("XLM", 500_000, 86400, 200);

        assert!(treasury.check_spending_limit("XLM", 500_000, 300));
        assert!(!treasury.check_spending_limit("XLM", 500_001, 300));

        // Execute a proposal that uses some of the limit
        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                300_000,
                "recipient",
                "Grant 1",
                200,
                86400 * 7,
            )
            .unwrap();
        treasury.approve_proposal(&proposal_id, "bob", 200).unwrap();
        treasury
            .approve_proposal(&proposal_id, "carol", 200)
            .unwrap();
        treasury.execute_proposal(&proposal_id, 250).unwrap();

        // Now limit should have 200,000 remaining
        assert!(treasury.check_spending_limit("XLM", 200_000, 300));
        assert!(!treasury.check_spending_limit("XLM", 200_001, 300));
    }

    #[test]
    fn test_spending_limit_resets_after_period() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 10_000_000, "foundation", 100)
            .unwrap();

        treasury.set_spending_limit("XLM", 500_000, 86400, 100);

        // Spend full limit
        let id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                500_000,
                "recip",
                "Spend",
                100,
                86400,
            )
            .unwrap();
        treasury.approve_proposal(&id, "bob", 100).unwrap();
        treasury.approve_proposal(&id, "carol", 100).unwrap();
        treasury.execute_proposal(&id, 200).unwrap();

        // Over limit now
        assert!(!treasury.check_spending_limit("XLM", 1, 300));

        // After period expires, full limit restored
        assert!(treasury.check_spending_limit("XLM", 500_000, 100 + 86401));
    }

    #[test]
    fn test_add_remove_signer() {
        let mut treasury = setup();
        treasury.add_signer("frank".to_string()).unwrap();
        assert_eq!(treasury.signer_count(), 6);
        assert!(treasury.is_signer("frank"));

        treasury.remove_signer("frank").unwrap();
        assert_eq!(treasury.signer_count(), 5);
        assert!(!treasury.is_signer("frank"));
    }

    #[test]
    fn test_cannot_remove_below_threshold() {
        let signers = vec!["alice".to_string(), "bob".to_string(), "carol".to_string()];
        let mut treasury = Treasury::new(signers, 3, 1);
        let result = treasury.remove_signer("carol");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("below threshold"));
    }

    #[test]
    fn test_update_threshold() {
        let mut treasury = setup();
        treasury.update_threshold(4, 100).unwrap();
        assert_eq!(treasury.get_threshold(), 4);
    }

    #[test]
    fn test_invalid_threshold() {
        let mut treasury = setup();
        assert!(treasury.update_threshold(0, 100).is_err());
        assert!(treasury.update_threshold(10, 100).is_err()); // > signers.len()
    }

    #[test]
    fn test_audit_trail_integrity() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recip",
                "Grant",
                200,
                86400 * 7,
            )
            .unwrap();
        treasury.approve_proposal(&id, "bob", 300).unwrap();
        treasury.approve_proposal(&id, "carol", 300).unwrap();
        treasury.execute_proposal(&id, 400).unwrap();

        assert!(treasury.verify_audit_trail());
        assert_eq!(treasury.audit_log_length(), 5); // deposit + create + 2 approves + execute
    }

    #[test]
    fn test_pending_proposals() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let id1 = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "r1",
                "Grant 1",
                200,
                86400 * 7,
            )
            .unwrap();
        let _id2 = treasury
            .create_proposal(
                "bob",
                ProposalType::Grant,
                "XLM",
                50_000,
                "r2",
                "Grant 2",
                200,
                86400 * 7,
            )
            .unwrap();

        assert_eq!(treasury.pending_proposals(300).len(), 2);

        // Execute one
        treasury.approve_proposal(&id1, "bob", 300).unwrap();
        treasury.approve_proposal(&id1, "carol", 300).unwrap();
        treasury.execute_proposal(&id1, 400).unwrap();

        assert_eq!(treasury.pending_proposals(500).len(), 1);
    }

    #[test]
    fn test_non_signer_cannot_approve() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "r1",
                "Grant",
                200,
                86400 * 7,
            )
            .unwrap();

        let result = treasury.approve_proposal(&id, "frank", 300);
        assert!(result.is_err());
    }

    #[test]
    fn test_double_execution_fails() {
        let mut treasury = setup();
        treasury
            .deposit("XLM", 1_000_000, "foundation", 100)
            .unwrap();

        let id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recip",
                "Grant",
                200,
                86400 * 7,
            )
            .unwrap();
        treasury.approve_proposal(&id, "bob", 300).unwrap();
        treasury.approve_proposal(&id, "carol", 300).unwrap();
        treasury.execute_proposal(&id, 400).unwrap();

        let result = treasury.execute_proposal(&id, 500);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in approved state"));
    }
}
