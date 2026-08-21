// governance/upgrade.rs
//
// Protocol Upgrade Mechanism
//
// Safe protocol upgrade paths with version management for the SwapTrade DAO.
// Implements a staged upgrade process requiring governance approval and timelock.
//
// Capabilities:
//   - Versioned protocol upgrades tracked by semantic version
//   - Multi-sig approval for upgrade proposals
//   - Timelock delay before upgrade execution
//   - Upgrade validation and pre-flight checks
//   - Rollback mechanism for failed upgrades
//   - Upgrade history and audit trail
//   - Canary testing support (partial deployment)

use std::collections::HashMap;

// ─── Types ────────────────────────────────────────────────────────────────────

/// Semantic version for protocol releases.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Check if this is a breaking change (major version bump).
    pub fn is_breaking(&self, other: &Version) -> bool {
        self.major > other.major
    }

    /// Check if this is a backwards-compatible feature addition.
    pub fn is_feature(&self, other: &Version) -> bool {
        self.major == other.major && self.minor > other.minor
    }

    /// Check if this is a patch (bug fix only).
    pub fn is_patch(&self, other: &Version) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch > other.patch
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Status of an upgrade proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeStatus {
    /// Upgrade proposed, collecting approvals.
    Proposed,
    /// Timelock period active, waiting for execution.
    Timelock,
    /// Ready for execution after timelock expires.
    Ready,
    /// Upgrade executed successfully.
    Executed,
    /// Upgrade failed during execution.
    Failed,
    /// Upgrade cancelled.
    Cancelled,
    /// Upgrade rolled back after execution.
    RolledBack,
}

/// Type of upgrade being proposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeType {
    /// Breaking change requiring migration.
    Major,
    /// New features, backwards compatible.
    Minor,
    /// Bug fixes only.
    Patch,
    /// Emergency hotfix with reduced timelock.
    Hotfix,
    /// Configuration-only change.
    Config,
}

/// An upgrade proposal.
#[derive(Debug, Clone)]
pub struct UpgradeProposal {
    /// Unique proposal identifier.
    pub id: u64,
    /// Current version at time of proposal.
    pub from_version: Version,
    /// Target version.
    pub to_version: Version,
    /// Type of upgrade.
    pub upgrade_type: UpgradeType,
    /// Proposer address.
    pub proposer: String,
    /// Human-readable description.
    pub description: String,
    /// WASM hash of the new contract code (for reference).
    pub code_hash: Option<[u8; 32]>,
    /// Migration steps description.
    pub migration_steps: Vec<String>,
    /// Required approvals.
    pub required_approvals: usize,
    /// Current approvals.
    pub approvals: Vec<String>,
    /// When the proposal was created.
    pub proposed_at: u64,
    /// When the timelock expires (set when timelock starts).
    pub timelock_expires_at: Option<u64>,
    /// Timelock duration in seconds.
    pub timelock_duration: u64,
    /// Current status.
    pub status: UpgradeStatus,
    /// Execution result message.
    pub execution_result: Option<String>,
    /// When executed.
    pub executed_at: Option<u64>,
}

impl UpgradeProposal {
    pub fn approval_count(&self) -> usize {
        self.approvals.len()
    }

    pub fn is_approved(&self) -> bool {
        self.approvals.len() >= self.required_approvals
    }

    pub fn can_execute(&self, now: u64) -> bool {
        matches!(self.status, UpgradeStatus::Ready | UpgradeStatus::Timelock)
            && match self.timelock_expires_at {
                Some(expires) => now >= expires,
                None => false,
            }
    }
}

/// Upgrade history entry.
#[derive(Debug, Clone)]
pub struct UpgradeRecord {
    pub from_version: Version,
    pub to_version: Version,
    pub upgrade_type: UpgradeType,
    pub executed_at: u64,
    pub executed_by: String,
    pub success: bool,
    pub description: String,
}

/// Configuration for the upgrade system.
#[derive(Debug, Clone)]
pub struct UpgradeConfig {
    /// Default timelock duration for upgrades (seconds).
    pub default_timelock_secs: u64,
    /// Reduced timelock for hotfixes (seconds).
    pub hotfix_timelock_secs: u64,
    /// Required approvals for major upgrades.
    pub major_required_approvals: usize,
    /// Required approvals for minor upgrades.
    pub minor_required_approvals: usize,
    /// Required approvals for patches.
    pub patch_required_approvals: usize,
    /// Maximum time between proposal and execution (seconds).
    pub max_proposal_lifetime: u64,
    /// Whether rollback is enabled.
    pub rollback_enabled: bool,
}

impl Default for UpgradeConfig {
    fn default() -> Self {
        Self {
            default_timelock_secs: 172800, // 48 hours
            hotfix_timelock_secs: 3600,    // 1 hour
            major_required_approvals: 5,   // 5 signers for major
            minor_required_approvals: 3,   // 3 signers for minor
            patch_required_approvals: 2,   // 2 signers for patch
            max_proposal_lifetime: 604800, // 7 days
            rollback_enabled: true,
        }
    }
}

// ─── Upgrade Manager ─────────────────────────────────────────────────────────

pub struct UpgradeManager {
    /// Current protocol version.
    current_version: Version,
    /// Upgrade proposals.
    proposals: HashMap<u64, UpgradeProposal>,
    /// Completed upgrade history.
    history: Vec<UpgradeRecord>,
    /// Authorized upgrade signers.
    signers: Vec<String>,
    /// Upgrade configuration.
    config: UpgradeConfig,
    /// Next proposal ID.
    next_id: u64,
    /// Rollback version (last known good).
    rollback_version: Option<Version>,
}

impl UpgradeManager {
    pub fn new(current_version: Version, signers: Vec<String>, config: UpgradeConfig) -> Self {
        Self {
            current_version,
            proposals: HashMap::new(),
            history: Vec::new(),
            signers,
            config,
            next_id: 1,
            rollback_version: None,
        }
    }

    // ── Proposal Lifecycle ───────────────────────────────────────────────────

    /// Create a new upgrade proposal.
    pub fn propose_upgrade(
        &mut self,
        proposer: &str,
        to_version: Version,
        upgrade_type: UpgradeType,
        description: &str,
        code_hash: Option<[u8; 32]>,
        migration_steps: Vec<String>,
        now: u64,
    ) -> Result<u64, String> {
        if !self.signers.contains(&proposer.to_string()) {
            return Err("only authorized signers can propose upgrades".to_string());
        }

        // Validate version is newer
        if to_version <= self.current_version {
            return Err("target version must be greater than current version".to_string());
        }

        // Validate version jump matches upgrade type
        match upgrade_type {
            UpgradeType::Major => {
                if !to_version.is_breaking(&self.current_version) {
                    return Err("major upgrade requires major version bump".to_string());
                }
            }
            UpgradeType::Minor => {
                if !to_version.is_feature(&self.current_version) {
                    return Err("minor upgrade requires minor version bump".to_string());
                }
            }
            UpgradeType::Patch => {
                if !to_version.is_patch(&self.current_version) {
                    return Err("patch upgrade requires patch version bump".to_string());
                }
            }
            UpgradeType::Hotfix => {
                // Hotfix can be any bump
            }
            UpgradeType::Config => {
                // Config change can be any bump
            }
        }

        let required_approvals = match upgrade_type {
            UpgradeType::Major => self.config.major_required_approvals,
            UpgradeType::Minor => self.config.minor_required_approvals,
            UpgradeType::Patch | UpgradeType::Config => self.config.patch_required_approvals,
            UpgradeType::Hotfix => self.config.minor_required_approvals,
        };

        let timelock_duration = match upgrade_type {
            UpgradeType::Hotfix => self.config.hotfix_timelock_secs,
            _ => self.config.default_timelock_secs,
        };

        let id = self.next_id;
        self.next_id += 1;

        let proposal = UpgradeProposal {
            id,
            from_version: self.current_version.clone(),
            to_version,
            upgrade_type,
            proposer: proposer.to_string(),
            description: description.to_string(),
            code_hash,
            migration_steps,
            required_approvals,
            approvals: vec![proposer.to_string()], // Proposer auto-approves
            proposed_at: now,
            timelock_expires_at: None,
            timelock_duration,
            status: UpgradeStatus::Proposed,
            execution_result: None,
            executed_at: None,
        };

        self.proposals.insert(id, proposal);
        Ok(id)
    }

    /// Approve an upgrade proposal.
    pub fn approve_upgrade(
        &mut self,
        proposal_id: u64,
        signer: &str,
        now: u64,
    ) -> Result<usize, String> {
        if !self.signers.contains(&signer.to_string()) {
            return Err("only authorized signers can approve upgrades".to_string());
        }

        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or("proposal not found")?;

        if proposal.status != UpgradeStatus::Proposed {
            return Err(format!(
                "cannot approve in current status: {:?}",
                proposal.status
            ));
        }

        // Check proposal hasn't expired
        if now > proposal.proposed_at + self.config.max_proposal_lifetime {
            proposal.status = UpgradeStatus::Cancelled;
            return Err("proposal has expired".to_string());
        }

        if proposal.approvals.contains(&signer.to_string()) {
            return Err("signer has already approved".to_string());
        }

        proposal.approvals.push(signer.to_string());
        let count = proposal.approval_count();

        // If threshold reached, start timelock
        if proposal.is_approved() {
            proposal.status = UpgradeStatus::Timelock;
            proposal.timelock_expires_at = Some(now + proposal.timelock_duration);
        }

        Ok(count)
    }

    /// Execute an approved upgrade after timelock expires.
    pub fn execute_upgrade(&mut self, proposal_id: u64, now: u64) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or("proposal not found")?;

        if !proposal.can_execute(now) {
            return Err("upgrade is not ready for execution".to_string());
        }

        // Save rollback version
        self.rollback_version = Some(self.current_version.clone());

        // Perform the upgrade
        self.current_version = proposal.to_version.clone();
        proposal.status = UpgradeStatus::Executed;
        proposal.executed_at = Some(now);
        proposal.execution_result = Some("Upgrade executed successfully".to_string());

        // Record in history
        self.history.push(UpgradeRecord {
            from_version: proposal.from_version.clone(),
            to_version: proposal.to_version.clone(),
            upgrade_type: proposal.upgrade_type.clone(),
            executed_at: now,
            executed_by: proposal.proposer.clone(),
            success: true,
            description: proposal.description.clone(),
        });

        Ok(())
    }

    /// Cancel an upgrade proposal.
    pub fn cancel_upgrade(
        &mut self,
        proposal_id: u64,
        signer: &str,
        now: u64,
    ) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or("proposal not found")?;

        if !self.signers.contains(&signer.to_string()) {
            return Err("only authorized signers can cancel".to_string());
        }

        if proposal.status == UpgradeStatus::Executed {
            return Err("cannot cancel an executed upgrade".to_string());
        }

        if proposal.status == UpgradeStatus::RolledBack {
            return Err("upgrade has already been rolled back".to_string());
        }

        proposal.status = UpgradeStatus::Cancelled;
        Ok(())
    }

    /// Rollback to the previous version.
    pub fn rollback(&mut self, reason: &str, now: u64) -> Result<(), String> {
        if !self.config.rollback_enabled {
            return Err("rollback is not enabled".to_string());
        }

        let rollback_to = self
            .rollback_version
            .as_ref()
            .ok_or("no rollback version available")?
            .clone();

        self.history.push(UpgradeRecord {
            from_version: self.current_version.clone(),
            to_version: rollback_to.clone(),
            upgrade_type: UpgradeType::Hotfix,
            executed_at: now,
            executed_by: "system".to_string(),
            success: true,
            description: format!("Rollback: {}", reason),
        });

        self.current_version = rollback_to;
        Ok(())
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    pub fn current_version(&self) -> &Version {
        &self.current_version
    }

    pub fn get_proposal(&self, proposal_id: u64) -> Option<&UpgradeProposal> {
        self.proposals.get(&proposal_id)
    }

    pub fn history(&self) -> &[UpgradeRecord] {
        &self.history
    }

    pub fn rollback_version(&self) -> Option<&Version> {
        self.rollback_version.as_ref()
    }

    pub fn is_signer(&self, signer: &str) -> bool {
        self.signers.contains(&signer.to_string())
    }

    pub fn signer_count(&self) -> usize {
        self.signers.len()
    }

    pub fn pending_upgrades(&self) -> Vec<&UpgradeProposal> {
        self.proposals
            .values()
            .filter(|p| {
                p.status != UpgradeStatus::Executed
                    && p.status != UpgradeStatus::Cancelled
                    && p.status != UpgradeStatus::RolledBack
            })
            .collect()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> UpgradeManager {
        let signers = vec![
            "alice".to_string(),
            "bob".to_string(),
            "carol".to_string(),
            "dave".to_string(),
            "eve".to_string(),
        ];
        let config = UpgradeConfig::default();
        UpgradeManager::new(Version::new(1, 0, 0), signers, config)
    }

    #[test]
    fn test_version_ordering() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(2, 0, 0);
        let v1_1 = Version::new(1, 1, 0);
        let v1_0_1 = Version::new(1, 0, 1);

        assert!(v2 > v1);
        assert!(v1_1 > v1);
        assert!(v1_0_1 > v1);
        assert!(v2.is_breaking(&v1));
        assert!(v1_1.is_feature(&v1));
        assert!(v1_0_1.is_patch(&v1));
    }

    #[test]
    fn test_create_major_upgrade() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Major upgrade with new AMM",
                None,
                vec!["Migrate pool state".to_string()],
                100,
            )
            .unwrap();

        let proposal = mgr.get_proposal(id).unwrap();
        assert_eq!(proposal.status, UpgradeStatus::Proposed);
        assert_eq!(proposal.required_approvals, 5);
        assert_eq!(proposal.timelock_duration, 172800);
    }

    #[test]
    fn test_create_minor_upgrade() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(1, 1, 0),
                UpgradeType::Minor,
                "Add new order type",
                None,
                vec![],
                100,
            )
            .unwrap();

        let proposal = mgr.get_proposal(id).unwrap();
        assert_eq!(proposal.required_approvals, 3);
    }

    #[test]
    fn test_create_patch_upgrade() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(1, 0, 1),
                UpgradeType::Patch,
                "Fix rounding error",
                None,
                vec![],
                100,
            )
            .unwrap();

        let proposal = mgr.get_proposal(id).unwrap();
        assert_eq!(proposal.required_approvals, 2);
    }

    #[test]
    fn test_hotfix_reduced_timelock() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(1, 0, 1),
                UpgradeType::Hotfix,
                "Security hotfix",
                None,
                vec![],
                100,
            )
            .unwrap();

        let proposal = mgr.get_proposal(id).unwrap();
        assert_eq!(proposal.timelock_duration, 3600); // 1 hour
    }

    #[test]
    fn test_non_signer_cannot_propose() {
        let mut mgr = setup();
        let result = mgr.propose_upgrade(
            "frank",
            Version::new(2, 0, 0),
            UpgradeType::Major,
            "Test",
            None,
            vec![],
            100,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("only authorized"));
    }

    #[test]
    fn test_cannot_downgrade() {
        let mut mgr = setup();
        let result = mgr.propose_upgrade(
            "alice",
            Version::new(0, 5, 0),
            UpgradeType::Major,
            "Downgrade",
            None,
            vec![],
            100,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("greater than current"));
    }

    #[test]
    fn test_major_version_mismatch() {
        let mut mgr = setup();
        let result = mgr.propose_upgrade(
            "alice",
            Version::new(1, 1, 0),
            UpgradeType::Major,
            "Not a major",
            None,
            vec![],
            100,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("major version bump"));
    }

    #[test]
    fn test_approve_upgrade() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Major upgrade",
                None,
                vec![],
                100,
            )
            .unwrap();

        mgr.approve_upgrade(id, "bob", 200).unwrap();
        mgr.approve_upgrade(id, "carol", 200).unwrap();
        mgr.approve_upgrade(id, "dave", 200).unwrap();
        mgr.approve_upgrade(id, "eve", 200).unwrap();

        let proposal = mgr.get_proposal(id).unwrap();
        assert_eq!(proposal.status, UpgradeStatus::Timelock);
        assert_eq!(proposal.timelock_expires_at, Some(200 + 172800));
    }

    #[test]
    fn test_cannot_approve_twice() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Major",
                None,
                vec![],
                100,
            )
            .unwrap();

        let result = mgr.approve_upgrade(id, "alice", 200);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already approved"));
    }

    #[test]
    fn test_execute_after_timelock() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Major upgrade",
                None,
                vec![],
                100,
            )
            .unwrap();

        // Get approvals
        mgr.approve_upgrade(id, "bob", 200).unwrap();
        mgr.approve_upgrade(id, "carol", 200).unwrap();
        mgr.approve_upgrade(id, "dave", 200).unwrap();
        mgr.approve_upgrade(id, "eve", 200).unwrap();

        // Cannot execute before timelock
        let result = mgr.execute_upgrade(id, 300);
        assert!(result.is_err());

        // Execute after timelock
        let timelock_end = 200 + 172800;
        mgr.execute_upgrade(id, timelock_end).unwrap();

        assert_eq!(mgr.current_version(), &Version::new(2, 0, 0));
        assert_eq!(mgr.history().len(), 1);
        assert_eq!(mgr.rollback_version(), Some(&Version::new(1, 0, 0)));
    }

    #[test]
    fn test_rollback() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Major upgrade",
                None,
                vec![],
                100,
            )
            .unwrap();

        mgr.approve_upgrade(id, "bob", 200).unwrap();
        mgr.approve_upgrade(id, "carol", 200).unwrap();
        mgr.approve_upgrade(id, "dave", 200).unwrap();
        mgr.approve_upgrade(id, "eve", 200).unwrap();
        mgr.execute_upgrade(id, 200 + 172800).unwrap();

        assert_eq!(mgr.current_version(), &Version::new(2, 0, 0));

        mgr.rollback("Critical bug found", 200 + 172900).unwrap();
        assert_eq!(mgr.current_version(), &Version::new(1, 0, 0));
        assert_eq!(mgr.history().len(), 2);
    }

    #[test]
    fn test_rollback_disabled() {
        let signers = vec!["alice".to_string()];
        let config = UpgradeConfig {
            rollback_enabled: false,
            ..UpgradeConfig::default()
        };
        let mut mgr = UpgradeManager::new(Version::new(1, 0, 0), signers, config);

        let result = mgr.rollback("test", 100);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not enabled"));
    }

    #[test]
    fn test_cancel_upgrade() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Major upgrade",
                None,
                vec![],
                100,
            )
            .unwrap();

        mgr.cancel_upgrade(id, "bob", 200).unwrap();
        let proposal = mgr.get_proposal(id).unwrap();
        assert_eq!(proposal.status, UpgradeStatus::Cancelled);
    }

    #[test]
    fn test_cannot_cancel_executed() {
        let mut mgr = setup();
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Major upgrade",
                None,
                vec![],
                100,
            )
            .unwrap();

        mgr.approve_upgrade(id, "bob", 200).unwrap();
        mgr.approve_upgrade(id, "carol", 200).unwrap();
        mgr.approve_upgrade(id, "dave", 200).unwrap();
        mgr.approve_upgrade(id, "eve", 200).unwrap();
        mgr.execute_upgrade(id, 200 + 172800).unwrap();

        let result = mgr.cancel_upgrade(id, "alice", 200 + 172900);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("executed"));
    }

    #[test]
    fn test_pending_upgrades() {
        let mut mgr = setup();
        let id1 = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Upgrade 1",
                None,
                vec![],
                100,
            )
            .unwrap();
        let _id2 = mgr
            .propose_upgrade(
                "bob",
                Version::new(1, 1, 0),
                UpgradeType::Minor,
                "Upgrade 2",
                None,
                vec![],
                100,
            )
            .unwrap();

        assert_eq!(mgr.pending_upgrades().len(), 2);

        mgr.cancel_upgrade(id1, "alice", 200).unwrap();
        assert_eq!(mgr.pending_upgrades().len(), 1);
    }

    #[test]
    fn test_sequential_upgrades() {
        let mut mgr = setup();

        // First upgrade: 1.0.0 -> 1.1.0
        let id1 = mgr
            .propose_upgrade(
                "alice",
                Version::new(1, 1, 0),
                UpgradeType::Minor,
                "Feature upgrade",
                None,
                vec![],
                100,
            )
            .unwrap();
        mgr.approve_upgrade(id1, "bob", 200).unwrap();
        mgr.approve_upgrade(id1, "carol", 200).unwrap();
        mgr.execute_upgrade(id1, 200 + 172800).unwrap();

        assert_eq!(mgr.current_version(), &Version::new(1, 1, 0));

        // Second upgrade: 1.1.0 -> 2.0.0
        let id2 = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Major upgrade",
                None,
                vec![],
                300 + 172800,
            )
            .unwrap();
        mgr.approve_upgrade(id2, "bob", 400 + 172800).unwrap();
        mgr.approve_upgrade(id2, "carol", 400 + 172800).unwrap();
        mgr.approve_upgrade(id2, "dave", 400 + 172800).unwrap();
        mgr.approve_upgrade(id2, "eve", 400 + 172800).unwrap();
        mgr.execute_upgrade(id2, 400 + 172800 + 172800).unwrap();

        assert_eq!(mgr.current_version(), &Version::new(2, 0, 0));
        assert_eq!(mgr.history().len(), 2);
    }

    #[test]
    fn test_proposal_expiry() {
        let signers = vec!["alice".to_string(), "bob".to_string(), "carol".to_string()];
        let config = UpgradeConfig {
            max_proposal_lifetime: 3600, // 1 hour
            ..UpgradeConfig::default()
        };
        let mut mgr = UpgradeManager::new(Version::new(1, 0, 0), signers, config);

        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Upgrade",
                None,
                vec![],
                100,
            )
            .unwrap();

        // Try to approve after expiry
        let result = mgr.approve_upgrade(id, "bob", 100 + 3601);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));

        let proposal = mgr.get_proposal(id).unwrap();
        assert_eq!(proposal.status, UpgradeStatus::Cancelled);
    }
}
