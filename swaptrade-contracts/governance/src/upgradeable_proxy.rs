use soroban_sdk::{Address, Env, Symbol, symbol_short};

use crate::errors::GovernanceError;
use crate::storage::{load_implementation, load_paused, store_paused, store_upgrade_scheduled};
use crate::events::upgraded;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyConfig {
    pub implementation: Address,
    pub admin: Address,
    pub paused: bool,
}

pub fn initialize_proxy(env: &Env, implementation: Address, admin: Address) {
    let _ = admin;
    crate::storage::store_implementation(env, &implementation);
    crate::storage::store_signers(env, &Vec::new(env));
    crate::storage::store_threshold(env, 0);
    crate::storage::store_initialized(env, true);
    crate::storage::store_paused(env, false);
    crate::storage::store_timelock_delay(env, 0);
    crate::storage::store_proposal_count(env, 0);
    crate::storage::store_nonce(env, 0);
}

pub fn schedule_upgrade(env: &Env, new_implementation: Address) -> Result<(), GovernanceError> {
    if load_paused(env) {
        return Err(GovernanceError::ContractPaused);
    }

    let current = load_implementation(env);
    if current == new_implementation {
        return Err(GovernanceError::ImplementationUnchanged);
    }

    store_upgrade_scheduled(env, Some(&new_implementation));
    Ok(())
}

pub fn execute_scheduled_upgrade(env: &Env) -> Result<Address, GovernanceError> {
    let scheduled = crate::storage::load_upgrade_scheduled(env)
        .ok_or(GovernanceError::UpgradeNotScheduled)?;

    let old_impl = load_implementation(env);
    store_implementation(env, &scheduled);
    store_upgrade_scheduled(env, None);
    store_paused(env, false);

    upgraded(env, 0, old_impl, scheduled.clone());
    Ok(scheduled)
}

pub fn get_implementation(env: &Env) -> Address {
    load_implementation(env)
}

pub fn is_paused(env: &Env) -> bool {
    load_paused(env)
}

pub fn pause(env: &Env) -> Result<(), GovernanceError> {
    if load_paused(env) {
        return Err(GovernanceError::ContractPaused);
    }
    store_paused(env, true);
    Ok(())
}

pub fn unpause(env: &Env) -> Result<(), GovernanceError> {
    if !load_paused(env) {
        return Err(GovernanceError::ContractNotPaused);
    }
    store_paused(env, false);
    Ok(())
}
