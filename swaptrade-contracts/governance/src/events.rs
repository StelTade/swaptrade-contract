use soroban_sdk::{Address, Env, Symbol, symbol_short};

#[contracttype]
#[derive(Clone)]
pub struct GovernanceEvent {
    pub action: Symbol,
    pub proposal_id: u64,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct TimelockEvent {
    pub proposal_id: u64,
    pub delay: u64,
    pub execute_at: u64,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct UpgradeEvent {
    pub proposal_id: u64,
    pub old_implementation: Address,
    pub new_implementation: Address,
    pub timestamp: u64,
}

pub fn proposal_created(env: &Env, proposal_id: u64, proposer: Address, action: Symbol) {
    env.events().publish(
        (symbol_short!("prop_create"), proposal_id),
        (proposer, action, env.ledger().timestamp()),
    );
}

pub fn proposal_signed(env: &Env, proposal_id: u64, signer: Address, signatures: u32) {
    env.events().publish(
        (symbol_short!("prop_sign"), proposal_id),
        (signer, signatures, env.ledger().timestamp()),
    );
}

pub fn proposal_executed(env: &Env, proposal_id: u64, executor: Address) {
    env.events().publish(
        (symbol_short!("prop_exec"), proposal_id),
        (executor, env.ledger().timestamp()),
    );
}

pub fn proposal_canceled(env: &Env, proposal_id: u64, canceler: Address) {
    env.events().publish(
        (symbol_short!("prop_cancel"), proposal_id),
        (canceler, env.ledger().timestamp()),
    );
}

pub fn timelock_scheduled(env: &Env, proposal_id: u64, delay: u64, execute_at: u64) {
    env.events().publish(
        (symbol_short!("tl_sched"), proposal_id),
        (delay, execute_at, env.ledger().timestamp()),
    );
}

pub fn paused(env: &Env, caller: Address) {
    env.events()
        .publish((symbol_short!("paused"),), (caller, env.ledger().timestamp()));
}

pub fn unpaused(env: &Env, caller: Address) {
    env.events()
        .publish((symbol_short!("unpaused"),), (caller, env.ledger().timestamp()));
}

pub fn upgraded(env: &Env, proposal_id: u64, old_impl: Address, new_impl: Address) {
    env.events().publish(
        (symbol_short!("upgraded"), proposal_id),
        (old_impl, new_impl, env.ledger().timestamp()),
    );
}

pub fn signer_added(env: &Env, signer: Address) {
    env.events()
        .publish((symbol_short!("signer_add"),), (signer, env.ledger().timestamp()));
}

pub fn signer_removed(env: &Env, signer: Address) {
    env.events()
        .publish((symbol_short!("signer_rm"),), (signer, env.ledger().timestamp()));
}

pub fn threshold_updated(env: &Env, old_threshold: u32, new_threshold: u32) {
    env.events().publish(
        (symbol_short!("thresh_upd"),),
        (old_threshold, new_threshold, env.ledger().timestamp()),
    );
}
