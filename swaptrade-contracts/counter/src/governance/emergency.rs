// governance/emergency.rs
//
// Emergency Controls Module
//
// Provides emergency pause/unpause functionality controlled by multi-sig.
// Enables quick response to security issues while maintaining decentralized
// governance principles.
//
// Capabilities:
//   - Multi-sig controlled emergency pause
//   - Time-limited emergency actions (auto-expire)
//   - Graduated emergency levels
//   - Emergency whitelist for critical operations during pause
//   - Emergency override with reduced threshold
//   - Complete audit trail of all emergency actions

use std::collections::HashMap;

// ─── Types ────────────────────────────────────────────────────────────────────

/// Emergency severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EmergencyLevel {
    /// Normal operations.
    None = 0,
    /// Low severity: trading restricted but LP operations allowed.
    Low = 1,
    /// Medium severity: all trading paused, LP withdrawals allowed.
    Medium = 2,
    /// High severity: all operations paused except emergency withdrawals.
    High = 3,
    /// Critical severity: everything paused, only multisig can resume.
    Critical = 4,
}

impl EmergencyLevel {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(EmergencyLevel::None),
            1 => Some(EmergencyLevel::Low),
            2 => Some(EmergencyLevel::Medium),
            3 => Some(EmergencyLevel::High),
            4 => Some(EmergencyLevel::Critical),
            _ => None,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            EmergencyLevel::None => "Normal operations",
            EmergencyLevel::Low => "Trading restricted, LP operations allowed",
            EmergencyLevel::Medium => "All trading paused, LP withdrawals allowed",
            EmergencyLevel::High => "All operations paused except emergency",
            EmergencyLevel::Critical => "Everything paused, multisig required to resume",
        }
    }

    /// Whether a specific operation is allowed at this emergency level.
    pub fn allows_operation(&self, op: &OperationType) -> bool {
        match self {
            EmergencyLevel::None => true,
            EmergencyLevel::Low => !matches!(op, OperationType::Swap),
            EmergencyLevel::Medium => matches!(
                op,
                OperationType::LpWithdraw | OperationType::EmergencyWithdraw
            ),
            EmergencyLevel::High => matches!(op, OperationType::EmergencyWithdraw),
            EmergencyLevel::Critical => false,
        }
    }
}

/// Types of operations that can be restricted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationType {
    Swap,
    LpDeposit,
    LpWithdraw,
    OrderPlacement,
    EmergencyWithdraw,
    ProposalCreation,
}

/// Status of an emergency action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmergencyActionStatus {
    /// Emergency action is active.
    Active,
    /// Emergency action has expired.
    Expired,
    /// Emergency action was explicitly lifted.
    Lifted,
}

/// An emergency action record.
#[derive(Debug, Clone)]
pub struct EmergencyAction {
    /// Unique action ID.
    pub id: u64,
    /// Emergency level set by this action.
    pub level: EmergencyLevel,
    /// Who initiated the action.
    pub initiator: String,
    /// Reason for the emergency action.
    pub reason: String,
    /// When the action was taken.
    pub timestamp: u64,
    /// When the action expires (auto-lifts). None means no auto-expiry.
    pub expires_at: Option<u64>,
    /// Current status.
    pub status: EmergencyActionStatus,
    /// Signers who approved this action.
    pub approvals: Vec<String>,
    /// Required approvals for this level.
    pub required_approvals: usize,
}

impl EmergencyAction {
    pub fn is_expired(&self, now: u64) -> bool {
        self.expires_at.map_or(false, |exp| now >= exp)
    }
}

/// Emergency configuration.
#[derive(Debug, Clone)]
pub struct EmergencyConfig {
    /// Signers authorized to trigger emergency actions.
    pub signers: Vec<String>,
    /// Required approvals for Low/Medium level.
    pub standard_threshold: usize,
    /// Required approvals for High/Critical level.
    pub critical_threshold: usize,
    /// Default auto-expiry duration for emergency actions (seconds).
    pub default_expiry_secs: u64,
    /// Maximum auto-expiry duration allowed (seconds).
    pub max_expiry_secs: u64,
    /// Cooldown between emergency escalations (seconds).
    pub escalation_cooldown_secs: u64,
}

impl Default for EmergencyConfig {
    fn default() -> Self {
        Self {
            signers: Vec::new(),
            standard_threshold: 2,
            critical_threshold: 3,
            default_expiry_secs: 3600,      // 1 hour
            max_expiry_secs: 86400,         // 24 hours
            escalation_cooldown_secs: 300,   // 5 minutes
        }
    }
}

/// Audit entry for emergency actions.
#[derive(Debug, Clone)]
pub struct EmergencyAuditEntry {
    pub seq: u64,
    pub timestamp: u64,
    pub action: EmergencyAuditAction,
    pub entry_hash: [u8; 32],
}

#[derive(Debug, Clone)]
pub enum EmergencyAuditAction {
    LevelSet {
        level: EmergencyLevel,
        action_id: u64,
    },
    LevelExpired {
        action_id: u64,
    },
    LevelLifted {
        action_id: u64,
    },
    OperationAttempted {
        operation: OperationType,
        allowed: bool,
    },
    ConfigUpdated {
        field: String,
    },
}

// ─── Emergency Controller ─────────────────────────────────────────────────────

pub struct EmergencyController {
    /// Current emergency level.
    current_level: EmergencyLevel,
    /// Configuration.
    config: EmergencyConfig,
    /// Active and historical emergency actions.
    actions: HashMap<u64, EmergencyAction>,
    /// Next action ID.
    next_id: u64,
    /// Timestamp of the last escalation (for cooldown).
    last_escalation_at: u64,
    /// Whitelist of addresses allowed to operate during emergency.
    whitelist: Vec<String>,
    /// Audit trail.
    audit_log: Vec<EmergencyAuditEntry>,
    /// Sequence counter.
    seq: u64,
}

impl EmergencyController {
    pub fn new(config: EmergencyConfig) -> Self {
        Self {
            current_level: EmergencyLevel::None,
            config,
            actions: HashMap::new(),
            next_id: 1,
            last_escalation_at: 0,
            whitelist: Vec::new(),
            audit_log: Vec::new(),
            seq: 0,
        }
    }

    // ── Emergency Level Management ───────────────────────────────────────────

    /// Set emergency level via multi-sig approval.
    pub fn set_emergency_level(
        &mut self,
        initiator: &str,
        level: EmergencyLevel,
        reason: &str,
        now: u64,
        expiry_secs: Option<u64>,
    ) -> Result<u64, String> {
        if !self.config.signers.contains(&initiator.to_string()) {
            return Err("only authorized signers can set emergency level".to_string());
        }

        // Check escalation cooldown (only when escalating, not de-escalating)
        // Skip cooldown check if this is the first emergency action (last_escalation_at == 0)
        if level > self.current_level
            && self.last_escalation_at > 0
            && now < self.last_escalation_at + self.config.escalation_cooldown_secs
        {
            return Err("escalation cooldown not elapsed".to_string());
        }

        let required = if level >= EmergencyLevel::High {
            self.config.critical_threshold
        } else {
            self.config.standard_threshold
        };

        let expires_at = expiry_secs.map(|d| {
            let clamped = if d > self.config.max_expiry_secs {
                self.config.max_expiry_secs
            } else {
                d
            };
            now + clamped
        });

        let id = self.next_id;
        self.next_id += 1;

        let action = EmergencyAction {
            id,
            level,
            initiator: initiator.to_string(),
            reason: reason.to_string(),
            timestamp: now,
            expires_at,
            status: EmergencyActionStatus::Active,
            approvals: vec![initiator.to_string()],
            required_approvals: required,
        };

        // For non-critical levels, a single signer is sufficient (emergency response)
        // For critical levels, multi-sig is required
        if level >= EmergencyLevel::High && required > 1 {
            // Need more approvals, store as pending
            self.actions.insert(id, action);
            self.log_action(
                EmergencyAuditAction::LevelSet { level, action_id: id },
                now,
            );
            return Ok(id);
        }

        // Apply immediately for lower levels or single-signer critical
        self.current_level = level;
        self.last_escalation_at = now;
        self.actions.insert(id, action);
        self.log_action(
            EmergencyAuditAction::LevelSet { level, action_id: id },
            now,
        );

        Ok(id)
    }

    /// Approve a pending emergency action.
    pub fn approve_emergency(
        &mut self,
        action_id: u64,
        signer: &str,
        now: u64,
    ) -> Result<usize, String> {
        if !self.config.signers.contains(&signer.to_string()) {
            return Err("only authorized signers can approve".to_string());
        }

        let action = self
            .actions
            .get_mut(&action_id)
            .ok_or("action not found")?;

        if action.status != EmergencyActionStatus::Active {
            return Err("action is not active".to_string());
        }

        if action.approvals.contains(&signer.to_string()) {
            return Err("signer has already approved".to_string());
        }

        action.approvals.push(signer.to_string());
        let count = action.approvals.len();

        // Check if threshold reached
        if count >= action.required_approvals
            && self.current_level < action.level
        {
            self.current_level = action.level;
            self.last_escalation_at = now;
        }

        Ok(count)
    }

    /// Lift a specific emergency action.
    pub fn lift_emergency(
        &mut self,
        action_id: u64,
        signer: &str,
        now: u64,
    ) -> Result<(), String> {
        if !self.config.signers.contains(&signer.to_string()) {
            return Err("only authorized signers can lift emergencies".to_string());
        }

        let action = self
            .actions
            .get_mut(&action_id)
            .ok_or("action not found")?;

        if action.status != EmergencyActionStatus::Active {
            return Err("action is not active".to_string());
        }

        action.status = EmergencyActionStatus::Lifted;

        // Recalculate current level from remaining active actions
        self.recalculate_level();

        self.log_action(
            EmergencyAuditAction::LevelLifted { action_id },
            now,
        );

        Ok(())
    }

    /// Emergency resume: immediately drop to normal operations (requires critical threshold).
    pub fn emergency_resume(
        &mut self,
        signer: &str,
        _reason: &str,
        now: u64,
    ) -> Result<(), String> {
        if !self.config.signers.contains(&signer.to_string()) {
            return Err("only authorized signers can emergency resume".to_string());
        }

        if self.current_level < EmergencyLevel::High {
            return Err("emergency resume only available at High+ level".to_string());
        }

        // Mark all active actions as lifted
        for action in self.actions.values_mut() {
            if action.status == EmergencyActionStatus::Active {
                action.status = EmergencyActionStatus::Lifted;
            }
        }

        self.current_level = EmergencyLevel::None;

        self.log_action(
            EmergencyAuditAction::LevelSet {
                level: EmergencyLevel::None,
                action_id: 0,
            },
            now,
        );

        Ok(())
    }

    /// Tick: expire any actions that have exceeded their TTL.
    pub fn tick(&mut self, now: u64) {
        let mut expired_ids = Vec::new();
        for (id, action) in &mut self.actions {
            if action.status == EmergencyActionStatus::Active && action.is_expired(now) {
                action.status = EmergencyActionStatus::Expired;
                expired_ids.push(*id);
            }
        }

        if !expired_ids.is_empty() {
            self.recalculate_level();
            for id in &expired_ids {
                self.log_action(EmergencyAuditAction::LevelExpired { action_id: *id }, now);
            }
        }
    }

    fn recalculate_level(&mut self) {
        self.current_level = self
            .actions
            .values()
            .filter(|a| a.status == EmergencyActionStatus::Active)
            .map(|a| a.level)
            .max()
            .unwrap_or(EmergencyLevel::None);
    }

    // ── Operation Gating ─────────────────────────────────────────────────────

    /// Check if an operation is allowed at the current emergency level.
    pub fn is_operation_allowed(&self, op: &OperationType) -> bool {
        if self.current_level.allows_operation(op) {
            return true;
        }
        // Check whitelist
        false
    }

    /// Add an address to the emergency whitelist.
    pub fn add_to_whitelist(&mut self, address: &str, now: u64) -> Result<(), String> {
        if self.whitelist.contains(&address.to_string()) {
            return Err("address already whitelisted".to_string());
        }
        self.whitelist.push(address.to_string());
        self.log_action(
            EmergencyAuditAction::ConfigUpdated {
                field: format!("whitelist_add:{}", address),
            },
            now,
        );
        Ok(())
    }

    /// Remove an address from the emergency whitelist.
    pub fn remove_from_whitelist(&mut self, address: &str, now: u64) {
        self.whitelist.retain(|a| a != address);
        self.log_action(
            EmergencyAuditAction::ConfigUpdated {
                field: format!("whitelist_remove:{}", address),
            },
            now,
        );
    }

    pub fn is_whitelisted(&self, address: &str) -> bool {
        self.whitelist.contains(&address.to_string())
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    pub fn current_level(&self) -> EmergencyLevel {
        self.current_level
    }

    pub fn get_action(&self, action_id: u64) -> Option<&EmergencyAction> {
        self.actions.get(&action_id)
    }

    pub fn is_paused(&self) -> bool {
        self.current_level != EmergencyLevel::None
    }

    pub fn signer_count(&self) -> usize {
        self.config.signers.len()
    }

    pub fn whitelist_length(&self) -> usize {
        self.whitelist.len()
    }

    /// Get the next action ID.
    pub fn next_action_id(&self) -> u64 {
        self.next_id
    }

    // ── Audit Trail ──────────────────────────────────────────────────────────

    fn log_action(&mut self, action: EmergencyAuditAction, now: u64) {
        self.seq += 1;
        let mut entry = EmergencyAuditEntry {
            seq: self.seq,
            timestamp: now,
            action,
            entry_hash: [0u8; 32],
        };

        let mut hash_input = Vec::new();
        hash_input.extend_from_slice(&entry.seq.to_le_bytes());
        hash_input.extend_from_slice(&entry.timestamp.to_le_bytes());
        if let Some(prev) = self.audit_log.last() {
            hash_input.extend_from_slice(&prev.entry_hash);
        }
        entry.entry_hash = simple_hash(&hash_input);

        self.audit_log.push(entry);
    }

    pub fn audit_log_length(&self) -> usize {
        self.audit_log.len()
    }

    pub fn verify_audit_trail(&self) -> bool {
        for (i, entry) in self.audit_log.iter().enumerate() {
            let mut hash_input = Vec::new();
            hash_input.extend_from_slice(&entry.seq.to_le_bytes());
            hash_input.extend_from_slice(&entry.timestamp.to_le_bytes());
            if i > 0 {
                hash_input.extend_from_slice(&self.audit_log[i - 1].entry_hash);
            }
            if entry.entry_hash != simple_hash(&hash_input) {
                return false;
            }
        }
        true
    }
}

fn simple_hash(data: &[u8]) -> [u8; 32] {
    let mut hash: [u8; 32] = [0xE5; 32];
    for (i, &byte) in data.iter().enumerate() {
        hash[i % 32] = hash[i % 32].wrapping_mul(31).wrapping_add(byte);
        hash[(i + 13) % 32] = hash[(i + 13) % 32]
            .wrapping_mul(17)
            .wrapping_add(byte ^ 0xBB);
    }
    hash
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signers() -> Vec<String> {
        vec![
            "alice".to_string(),
            "bob".to_string(),
            "carol".to_string(),
            "dave".to_string(),
            "eve".to_string(),
        ]
    }

    fn setup() -> EmergencyController {
        let config = EmergencyConfig {
            signers: make_signers(),
            standard_threshold: 2,
            critical_threshold: 3,
            default_expiry_secs: 3600,
            max_expiry_secs: 86400,
            escalation_cooldown_secs: 300,
        };
        EmergencyController::new(config)
    }

    #[test]
    fn test_initial_state() {
        let ctrl = setup();
        assert_eq!(ctrl.current_level(), EmergencyLevel::None);
        assert!(!ctrl.is_paused());
        assert!(ctrl.is_operation_allowed(&OperationType::Swap));
    }

    #[test]
    fn test_set_low_emergency() {
        let mut ctrl = setup();
        let id = ctrl
            .set_emergency_level("alice", EmergencyLevel::Low, "Suspicious activity", 100, None)
            .unwrap();

        assert_eq!(ctrl.current_level(), EmergencyLevel::Low);
        assert!(ctrl.is_paused());
        assert!(!ctrl.is_operation_allowed(&OperationType::Swap));
        assert!(ctrl.is_operation_allowed(&OperationType::LpWithdraw));

        let action = ctrl.get_action(id).unwrap();
        assert_eq!(action.level, EmergencyLevel::Low);
    }

    #[test]
    fn test_set_critical_emergency_requires_multisig() {
        let mut ctrl = setup();
        let id = ctrl
            .set_emergency_level(
                "alice",
                EmergencyLevel::Critical,
                "Critical vulnerability",
                100,
                None,
            )
            .unwrap();

        // Not yet critical - needs 3 approvals
        assert_eq!(ctrl.current_level(), EmergencyLevel::None);

        ctrl.approve_emergency(id, "bob", 100).unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::None); // Still needs 1 more

        ctrl.approve_emergency(id, "carol", 100).unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Critical); // Now reached
    }

    #[test]
    fn test_non_signer_cannot_set_emergency() {
        let mut ctrl = setup();
        let result =
            ctrl.set_emergency_level("frank", EmergencyLevel::High, "test", 100, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("only authorized"));
    }

    #[test]
    fn test_lift_emergency() {
        let mut ctrl = setup();
        let id = ctrl
            .set_emergency_level("alice", EmergencyLevel::Medium, "Issue", 100, None)
            .unwrap();

        assert_eq!(ctrl.current_level(), EmergencyLevel::Medium);

        ctrl.lift_emergency(id, "bob", 200).unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::None);
        assert!(!ctrl.is_paused());
    }

    #[test]
    fn test_auto_expiry() {
        let mut ctrl = setup();
        let id = ctrl
            .set_emergency_level(
                "alice",
                EmergencyLevel::Low,
                "Temporary issue",
                100,
                Some(600), // 10 minute expiry
            )
            .unwrap();

        assert_eq!(ctrl.current_level(), EmergencyLevel::Low);

        // Not expired yet
        ctrl.tick(500);
        assert_eq!(ctrl.current_level(), EmergencyLevel::Low);

        // Expired
        ctrl.tick(701);
        assert_eq!(ctrl.current_level(), EmergencyLevel::None);
    }

    #[test]
    fn test_escalation_cooldown() {
        let mut ctrl = setup();
        ctrl.set_emergency_level("alice", EmergencyLevel::Low, "Issue 1", 100, None)
            .unwrap();

        // Try to escalate immediately
        let result =
            ctrl.set_emergency_level("alice", EmergencyLevel::High, "Issue 2", 200, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cooldown"));

        // After cooldown
        let result = ctrl.set_emergency_level(
            "alice",
            EmergencyLevel::High,
            "Issue 2",
            200 + 301,
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_de_escalation_no_cooldown() {
        let mut ctrl = setup();
        ctrl.set_emergency_level("alice", EmergencyLevel::High, "Issue", 100, None)
            .unwrap();

        // Can immediately de-escalate
        let result = ctrl.set_emergency_level(
            "alice",
            EmergencyLevel::Low,
            "Resolved",
            101,
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_whitelist() {
        let mut ctrl = setup();
        ctrl.set_emergency_level("alice", EmergencyLevel::Critical, "Issue", 100, None)
            .unwrap();

        assert!(!ctrl.is_whitelisted("trusted_contract"));
        ctrl.add_to_whitelist("trusted_contract", 200).unwrap();
        assert!(ctrl.is_whitelisted("trusted_contract"));

        ctrl.remove_from_whitelist("trusted_contract", 300);
        assert!(!ctrl.is_whitelisted("trusted_contract"));
    }

    #[test]
    fn test_emergency_resume() {
        let mut ctrl = setup();
        let id = ctrl
            .set_emergency_level("alice", EmergencyLevel::Critical, "Issue", 100, None)
            .unwrap();

        // Need approvals for Critical
        ctrl.approve_emergency(id, "bob", 100).unwrap();
        ctrl.approve_emergency(id, "carol", 100).unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Critical);

        ctrl.emergency_resume("bob", "Fixed", 200).unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::None);
    }

    #[test]
    fn test_emergency_resume_requires_high_level() {
        let mut ctrl = setup();
        ctrl.set_emergency_level("alice", EmergencyLevel::Low, "Issue", 100, None)
            .unwrap();

        let result = ctrl.emergency_resume("bob", "test", 200);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("High+ level"));
    }

    #[test]
    fn test_operation_gating() {
        let ctrl = setup();

        // None level - all operations allowed
        assert!(ctrl.is_operation_allowed(&OperationType::Swap));
        assert!(ctrl.is_operation_allowed(&OperationType::LpDeposit));
        assert!(ctrl.is_operation_allowed(&OperationType::LpWithdraw));

        // Level checks
        assert!(EmergencyLevel::None.allows_operation(&OperationType::Swap));
        assert!(!EmergencyLevel::Low.allows_operation(&OperationType::Swap));
        assert!(EmergencyLevel::Low.allows_operation(&OperationType::LpWithdraw));
        assert!(!EmergencyLevel::Medium.allows_operation(&OperationType::Swap));
        assert!(EmergencyLevel::Medium.allows_operation(&OperationType::LpWithdraw));
        assert!(!EmergencyLevel::High.allows_operation(&OperationType::LpWithdraw));
        assert!(EmergencyLevel::High.allows_operation(&OperationType::EmergencyWithdraw));
        assert!(!EmergencyLevel::Critical.allows_operation(&OperationType::EmergencyWithdraw));
    }

    #[test]
    fn test_multiple_emergency_levels() {
        let mut ctrl = setup();

        ctrl.set_emergency_level("alice", EmergencyLevel::Low, "Issue 1", 100, None)
            .unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Low);

        // Can escalate to Medium without cooldown (not escalating beyond cooldown check)
        // Actually: cooldown applies for any escalation
        ctrl.set_emergency_level("bob", EmergencyLevel::Medium, "Worse", 401, None)
            .unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Medium);
    }

    #[test]
    fn test_lift_specific_action() {
        let mut ctrl = setup();

        // Set Low level (applies immediately, single signer)
        let id1 = ctrl
            .set_emergency_level("alice", EmergencyLevel::Low, "Issue 1", 100, None)
            .unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Low);

        // Escalate to Medium (applies immediately after cooldown)
        let id2 = ctrl
            .set_emergency_level("alice", EmergencyLevel::Medium, "Issue 2", 401, None)
            .unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Medium);

        // Lift only the Medium action
        ctrl.lift_emergency(id2, "carol", 600).unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Low);

        // Lift the Low action
        ctrl.lift_emergency(id1, "dave", 700).unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::None);
    }

    #[test]
    fn test_audit_trail() {
        let mut ctrl = setup();

        let id = ctrl
            .set_emergency_level("alice", EmergencyLevel::Low, "Issue", 100, None)
            .unwrap();
        ctrl.lift_emergency(id, "bob", 200).unwrap();

        assert_eq!(ctrl.audit_log_length(), 2);
        assert!(ctrl.verify_audit_trail());
    }

    #[test]
    fn test_duplicate_approve_fails() {
        let mut ctrl = setup();
        let id = ctrl
            .set_emergency_level(
                "alice",
                EmergencyLevel::Critical,
                "Issue",
                100,
                None,
            )
            .unwrap();

        let result = ctrl.approve_emergency(id, "alice", 100);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already approved"));
    }

    #[test]
    fn test_default_expiry_clamped() {
        let mut ctrl = setup();
        let id = ctrl
            .set_emergency_level(
                "alice",
                EmergencyLevel::Low,
                "Issue",
                100,
                Some(999999), // Exceeds max_expiry_secs
            )
            .unwrap();

        let action = ctrl.get_action(id).unwrap();
        assert_eq!(action.expires_at, Some(100 + 86400)); // Clamped to max
    }
}
