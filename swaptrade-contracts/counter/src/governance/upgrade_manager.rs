use soroban_sdk::{contracttype, symbol_short, Address, Bytes, Env, Symbol, Vec};

use crate::errors::SwapTradeError;

/// Storage keys for the upgrade manager
const UPGRADE_CONFIG_KEY: Symbol = symbol_short!("upg_cfg");
const UPGRADE_HISTORY_KEY: Symbol = symbol_short!("upg_hist");
const PENDING_UPGRADE_KEY: Symbol = symbol_short!("pupg");

/// Configuration for the upgrade manager
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeConfig {
    /// The multisig signers authorized to approve upgrades
    pub authorized_upgrade_signers: Vec<Address>,
    /// Number of signatures required to approve an upgrade
    pub threshold: u32,
    /// Minimum delay (in seconds) between upgrade approval and execution
    pub timelock_delay: u64,
}

impl Default for UpgradeConfig {
    fn default() -> Self {
        Self {
            authorized_upgrade_signers: soroban_sdk::Vec::default(),
            threshold: 0,
            // Default 48-hour timelock for upgrades
            timelock_delay: 172_800,
        }
    }
}

/// Record of a past upgrade
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeRecord {
    /// Version before upgrade
    pub from_version: u32,
    /// Version after upgrade
    pub to_version: u32,
    /// WASM hash of the new implementation
    pub new_wasm_hash: Bytes,
    /// Timestamp of upgrade execution
    pub executed_at: u64,
    /// Who executed the upgrade
    pub executed_by: Address,
}

/// Pending upgrade proposal
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingUpgrade {
    /// WASM hash of the proposed new implementation
    pub new_wasm_hash: Bytes,
    /// New contract version
    pub new_version: u32,
    /// Addresses that have approved this upgrade
    pub approved_by: Vec<Address>,
    /// When this upgrade was proposed
    pub proposed_at: u64,
    /// Earliest time this upgrade can be executed (after timelock)
    pub executable_at: u64,
    /// Whether this upgrade has been executed
    pub executed: bool,
}

/// Upgrade manager providing controlled upgradeability with multisig + timelock
pub struct UpgradeManager;

impl UpgradeManager {
    // ── Configuration ────────────────────────────────────────────────────────

    /// Initialize the upgrade manager with authorized signers and threshold.
    /// Must be called during contract setup.
    pub fn initialize(
        env: &Env,
        admin: &Address,
        signers: Vec<Address>,
        threshold: u32,
        timelock_delay: u64,
    ) -> Result<(), SwapTradeError> {
        admin.require_auth();
        crate::admin::require_admin(env, admin)?;

        if threshold == 0 || threshold > signers.len() {
            return Err(SwapTradeError::InvalidMultiSigConfig);
        }

        let config = UpgradeConfig {
            authorized_upgrade_signers: signers,
            threshold,
            timelock_delay,
        };
        env.storage().persistent().set(&UPGRADE_CONFIG_KEY, &config);
        Ok(())
    }

    /// Get the current upgrade configuration
    pub fn get_config(env: &Env) -> UpgradeConfig {
        env.storage()
            .persistent()
            .get(&UPGRADE_CONFIG_KEY)
            .unwrap_or_default()
    }

    // ── Propose Upgrade ──────────────────────────────────────────────────────

    /// Propose a contract upgrade. The proposer must be an authorized upgrade signer.
    /// Returns the proposed new WASM hash for tracking.
    pub fn propose_upgrade(
        env: &Env,
        proposer: &Address,
        new_wasm_hash: Bytes,
        new_version: u32,
    ) -> Result<(), SwapTradeError> {
        proposer.require_auth();

        let config = Self::get_config(env);
        if !config.authorized_upgrade_signers.contains(proposer) {
            return Err(SwapTradeError::NotAuthorized);
        }

        let now = env.ledger().timestamp();
        let mut approved_by = Vec::new(env);
        approved_by.push_back(proposer.clone());

        let pending = PendingUpgrade {
            new_wasm_hash,
            new_version,
            approved_by,
            proposed_at: now,
            executable_at: now.saturating_add(config.timelock_delay),
            executed: false,
        };

        env.storage().persistent().set(&PENDING_UPGRADE_KEY, &pending);

        env.events().publish(
            (symbol_short!("upg_prop"), proposer),
            new_version,
        );

        Ok(())
    }

    // ── Approve Upgrade ──────────────────────────────────────────────────────

    /// Approve a pending upgrade. Caller must be an authorized upgrade signer
    /// and must not have already approved.
    pub fn approve_upgrade(env: &Env, approver: &Address) -> Result<u32, SwapTradeError> {
        approver.require_auth();

        let config = Self::get_config(env);
        if !config.authorized_upgrade_signers.contains(approver) {
            return Err(SwapTradeError::NotAuthorized);
        }

        let mut pending: PendingUpgrade = env
            .storage()
            .persistent()
            .get(&PENDING_UPGRADE_KEY)
            .ok_or(SwapTradeError::ProposalNotFound)?;

        if pending.executed {
            return Err(SwapTradeError::ProposalAlreadyExecuted);
        }

        if pending.approved_by.contains(approver) {
            return Err(SwapTradeError::AlreadyApproved);
        }

        pending.approved_by.push_back(approver.clone());
        let approval_count = pending.approved_by.len();
        env.storage().persistent().set(&PENDING_UPGRADE_KEY, &pending);

        env.events().publish(
            (symbol_short!("upg_appr"), approver),
            approval_count,
        );

        Ok(approval_count)
    }

    // ── Execute Upgrade ──────────────────────────────────────────────────────

    /// Execute an approved upgrade. Requires:
    /// 1. Sufficient multisig approvals (threshold met)
    /// 2. Timelock has elapsed
    /// 3. Upgrade hasn't already been executed
    pub fn execute_upgrade(
        env: &Env,
        executor: &Address,
    ) -> Result<u32, SwapTradeError> {
        executor.require_auth();

        let config = Self::get_config(env);
        let mut pending: PendingUpgrade = env
            .storage()
            .persistent()
            .get(&PENDING_UPGRADE_KEY)
            .ok_or(SwapTradeError::ProposalNotFound)?;

        if pending.executed {
            return Err(SwapTradeError::ProposalAlreadyExecuted);
        }

        // Check threshold
        if pending.approved_by.len() < config.threshold {
            return Err(SwapTradeError::InsufficientSignatures);
        }

        // Check timelock
        let now = env.ledger().timestamp();
        if now < pending.executable_at {
            return Err(SwapTradeError::TimelockNotElapsed);
        }

        // Record the upgrade in history
        let current_version: u32 = env
            .storage()
            .instance()
            .get(&Symbol::short("v_code"))
            .unwrap_or(1);

        let record = UpgradeRecord {
            from_version: current_version,
            to_version: pending.new_version,
            new_wasm_hash: pending.new_wasm_hash.clone(),
            executed_at: now,
            executed_by: executor.clone(),
        };

        let mut history: Vec<UpgradeRecord> = env
            .storage()
            .persistent()
            .get(&UPGRADE_HISTORY_KEY)
            .unwrap_or_else(|| Vec::new(env));
        history.push_back(record);
        env.storage()
            .persistent()
            .set(&UPGRADE_HISTORY_KEY, &history);

        // Update the version marker
        env.storage()
            .instance()
            .set(&Symbol::short("v_code"), &pending.new_version);

        // Mark as executed
        pending.executed = true;
        env.storage().persistent().set(&PENDING_UPGRADE_KEY, &pending);

        env.events().publish(
            (symbol_short!("upg_done"), executor),
            pending.new_version,
        );

        Ok(pending.new_version)
    }

    // ── Query ────────────────────────────────────────────────────────────────

    /// Get the pending upgrade proposal (if any)
    pub fn get_pending_upgrade(env: &Env) -> Option<PendingUpgrade> {
        env.storage().persistent().get(&PENDING_UPGRADE_KEY)
    }

    /// Get upgrade history
    pub fn get_upgrade_history(env: &Env) -> Vec<UpgradeRecord> {
        env.storage()
            .persistent()
            .get(&UPGRADE_HISTORY_KEY)
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Check if a given address is an authorized upgrade signer
    pub fn is_upgrade_signer(env: &Env, address: &Address) -> bool {
        let config = Self::get_config(env);
        config.authorized_upgrade_signers.contains(address)
    }

    // ── Emergency Cancel ─────────────────────────────────────────────────────

    /// Cancel a pending upgrade. Only the original proposer or an admin can cancel.
    pub fn cancel_upgrade(
        env: &Env,
        caller: &Address,
    ) -> Result<(), SwapTradeError> {
        caller.require_auth();

        let mut pending: PendingUpgrade = env
            .storage()
            .persistent()
            .get(&PENDING_UPGRADE_KEY)
            .ok_or(SwapTradeError::ProposalNotFound)?;

        if pending.executed {
            return Err(SwapTradeError::ProposalAlreadyExecuted);
        }

        // Only admin or original proposer can cancel
        if !crate::admin::is_admin(env, caller) && !pending.approved_by.contains(caller) {
            return Err(SwapTradeError::NotAuthorized);
        }

        // Remove the pending upgrade by replacing with empty
        env.storage().persistent().remove(&PENDING_UPGRADE_KEY);

        env.events()
            .publish((symbol_short!("upg_cancel"), caller), ());

        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin::set_admin;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Bytes, Env,
    };

    fn setup() -> (Env, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, crate::CounterContract);
        let admin = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);
        let signer3 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            set_admin(&env, &admin);
        });

        (env, contract_id, admin, signer1, signer2)
    }

    #[test]
    fn test_initialize_upgrade_manager() {
        let (env, contract_id, admin, signer1, signer2) = setup();
        let signer3 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let signers = soroban_sdk::vec![&env, signer1.clone(), signer2.clone(), signer3.clone()];
            UpgradeManager::initialize(&env, &admin, signers, 2, 3600).unwrap();

            let config = UpgradeManager::get_config(&env);
            assert_eq!(config.threshold, 2);
            assert_eq!(config.timelock_delay, 3600);
            assert_eq!(config.authorized_upgrade_signers.len(), 3);
        });
    }

    #[test]
    fn test_propose_and_approve_upgrade() {
        let (env, contract_id, admin, signer1, signer2) = setup();
        let signer3 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let signers = soroban_sdk::vec![&env, signer1.clone(), signer2.clone(), signer3.clone()];
            UpgradeManager::initialize(&env, &admin, signers, 2, 3600).unwrap();

            // Set current version
            env.storage().instance().set(&Symbol::short("v_code"), &1u32);

            let wasm_hash = Bytes::from_array(&env, &[0u8; 32]);
            UpgradeManager::propose_upgrade(&env, &signer1, wasm_hash, 2).unwrap();

            let pending = UpgradeManager::get_pending_upgrade(&env).unwrap();
            assert_eq!(pending.new_version, 2);
            assert_eq!(pending.approved_by.len(), 1);
            assert!(!pending.executed);

            // First approval (from proposer auto-approval)
            // Second approval needed
            UpgradeManager::approve_upgrade(&env, &signer2).unwrap();

            let pending = UpgradeManager::get_pending_upgrade(&env).unwrap();
            assert_eq!(pending.approved_by.len(), 2);
        });
    }

    #[test]
    fn test_execute_upgrade_after_timelock() {
        let (env, contract_id, admin, signer1, signer2) = setup();
        let signer3 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let signers = soroban_sdk::vec![&env, signer1.clone(), signer2.clone(), signer3.clone()];
            UpgradeManager::initialize(&env, &admin, signers, 2, 3600).unwrap();

            env.storage().instance().set(&Symbol::short("v_code"), &1u32);

            let wasm_hash = Bytes::from_array(&env, &[0u8; 32]);
            UpgradeManager::propose_upgrade(&env, &signer1, wasm_hash, 2).unwrap();
            UpgradeManager::approve_upgrade(&env, &signer2).unwrap();

            // Cannot execute before timelock
            assert_eq!(
                UpgradeManager::execute_upgrade(&env, &signer1),
                Err(SwapTradeError::TimelockNotElapsed)
            );

            // Advance time past timelock
            env.ledger().with_mut(|l| {
                l.timestamp = l.timestamp + 3601;
            });

            // Now can execute
            let new_version = UpgradeManager::execute_upgrade(&env, &signer1).unwrap();
            assert_eq!(new_version, 2);

            // Verify version updated
            let version: u32 = env.storage().instance().get(&Symbol::short("v_code")).unwrap();
            assert_eq!(version, 2);

            // Verify upgrade history
            let history = UpgradeManager::get_upgrade_history(&env);
            assert_eq!(history.len(), 1);
        });
    }

    #[test]
    fn test_non_signer_cannot_propose() {
        let (env, contract_id, admin, signer1, _signer2) = setup();
        let non_signer = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let signers = soroban_sdk::vec![&env, signer1.clone()];
            UpgradeManager::initialize(&env, &admin, signers, 1, 3600).unwrap();

            let wasm_hash = Bytes::from_array(&env, &[0u8; 32]);
            assert_eq!(
                UpgradeManager::propose_upgrade(&env, &non_signer, wasm_hash, 2),
                Err(SwapTradeError::NotAuthorized)
            );
        });
    }

    #[test]
    fn test_duplicate_approval_rejected() {
        let (env, contract_id, admin, signer1, _signer2) = setup();

        env.as_contract(&contract_id, || {
            let signers = soroban_sdk::vec![&env, signer1.clone()];
            UpgradeManager::initialize(&env, &admin, signers, 1, 3600).unwrap();

            let wasm_hash = Bytes::from_array(&env, &[0u8; 32]);
            UpgradeManager::propose_upgrade(&env, &signer1, wasm_hash, 2).unwrap();

            // Proposer already auto-approved
            assert_eq!(
                UpgradeManager::approve_upgrade(&env, &signer1),
                Err(SwapTradeError::AlreadyApproved)
            );
        });
    }

    #[test]
    fn test_cancel_upgrade() {
        let (env, contract_id, admin, signer1, _signer2) = setup();

        env.as_contract(&contract_id, || {
            let signers = soroban_sdk::vec![&env, signer1.clone()];
            UpgradeManager::initialize(&env, &admin, signers, 1, 3600).unwrap();

            let wasm_hash = Bytes::from_array(&env, &[0u8; 32]);
            UpgradeManager::propose_upgrade(&env, &signer1, wasm_hash, 2).unwrap();

            // Cancel
            UpgradeManager::cancel_upgrade(&env, &admin).unwrap();

            // Pending upgrade should be gone
            assert!(UpgradeManager::get_pending_upgrade(&env).is_none());
        });
    }

    #[test]
    fn test_cannot_execute_without_sufficient_approvals() {
        let (env, contract_id, admin, signer1, signer2) = setup();
        let signer3 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let signers = soroban_sdk::vec![&env, signer1.clone(), signer2.clone(), signer3.clone()];
            UpgradeManager::initialize(&env, &admin, signers, 3, 0).unwrap();

            env.storage().instance().set(&Symbol::short("v_code"), &1u32);

            let wasm_hash = Bytes::from_array(&env, &[0u8; 32]);
            UpgradeManager::propose_upgrade(&env, &signer1, wasm_hash, 2).unwrap();

            // Only 1 approval (proposer), need 3
            assert_eq!(
                UpgradeManager::execute_upgrade(&env, &signer1),
                Err(SwapTradeError::InsufficientSignatures)
            );
        });
    }
}
