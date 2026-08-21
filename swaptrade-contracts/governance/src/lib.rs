#![cfg_attr(all(not(test), target_family = "wasm"), no_std)]

extern crate alloc;

mod errors;
mod events;
mod governance_controller;
mod storage;
mod upgradeable_proxy;

pub use errors::GovernanceError;
pub use governance_controller::{
    approve, cancel, execute, get_proposal, is_signer, propose,
};
pub use storage::{Proposal, ProposalAction};
pub use upgradeable_proxy::{
    execute_scheduled_upgrade, get_implementation, initialize_proxy, is_paused, pause, schedule_upgrade,
    unpause,
};

use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

#[contract]
pub struct GovernanceContract;

#[contractimpl]
impl GovernanceContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        signers: Vec<Address>,
        threshold: u32,
        timelock_delay: u64,
        initial_implementation: Address,
    ) -> Result<(), GovernanceError> {
        admin.require_auth();

        if crate::storage::load_initialized(&env) {
            return Err(GovernanceError::AlreadyInitialized);
        }

        if threshold == 0 {
            return Err(GovernanceError::ZeroThreshold);
        }

        if threshold > signers.len() as u32 {
            return Err(GovernanceError::ThresholdExceeded);
        }

        if signers.is_empty() {
            return Err(GovernanceError::EmptySigners);
        }

        crate::storage::store_signers(&env, &signers);
        crate::storage::store_threshold(&env, threshold);
        crate::storage::store_timelock_delay(&env, timelock_delay);
        crate::storage::store_initialized(&env, true);
        crate::storage::store_paused(&env, false);
        crate::storage::store_proposal_count(&env, 0);
        crate::storage::store_nonce(&env, 0);
        crate::storage::store_implementation(&env, &initial_implementation);

        Ok(())
    }

    pub fn propose(
        env: Env,
        proposer: Address,
        action: ProposalAction,
    ) -> Result<u64, GovernanceError> {
        governance_controller::propose(&env, proposer, action)
    }

    pub fn approve(
        env: Env,
        signer: Address,
        proposal_id: u64,
    ) -> Result<(), GovernanceError> {
        governance_controller::approve(&env, signer, proposal_id)
    }

    pub fn revoke(
        env: Env,
        signer: Address,
        proposal_id: u64,
    ) -> Result<(), GovernanceError> {
        governance_controller::revoke(&env, signer, proposal_id)
    }

    pub fn execute(
        env: Env,
        executor: Address,
        proposal_id: u64,
    ) -> Result<(), GovernanceError> {
        governance_controller::execute(&env, executor, proposal_id)
    }

    pub fn cancel(env: Env, caller: Address, proposal_id: u64) -> Result<(), GovernanceError> {
        governance_controller::cancel(&env, caller, proposal_id)
    }

    pub fn get_proposal(env: Env, proposal_id: u64) -> Result<Proposal, GovernanceError> {
        governance_controller::get_proposal(&env, proposal_id)
    }

    pub fn is_signer(env: Env, address: Address) -> bool {
        governance_controller::is_signer(&env, &address)
    }

    pub fn get_signers(env: Env) -> Vec<Address> {
        crate::storage::load_signers(&env)
    }

    pub fn get_threshold(env: Env) -> u32 {
        crate::storage::load_threshold(&env)
    }

    pub fn get_timelock_delay(env: Env) -> u64 {
        crate::storage::load_timelock_delay(&env)
    }

    pub fn get_implementation(env: Env) -> Address {
        upgradeable_proxy::get_implementation(&env)
    }

    pub fn schedule_upgrade(env: Env, new_implementation: Address) -> Result<(), GovernanceError> {
        upgradeable_proxy::schedule_upgrade(&env, new_implementation)
    }

    pub fn execute_upgrade(env: Env) -> Result<Address, GovernanceError> {
        upgradeable_proxy::execute_scheduled_upgrade(&env)
    }

    pub fn pause(env: Env) -> Result<(), GovernanceError> {
        upgradeable_proxy::pause(&env)
    }

    pub fn unpause(env: Env) -> Result<(), GovernanceError> {
        upgradeable_proxy::unpause(&env)
    }

    pub fn is_paused(env: Env) -> bool {
        upgradeable_proxy::is_paused(&env)
    }
}
