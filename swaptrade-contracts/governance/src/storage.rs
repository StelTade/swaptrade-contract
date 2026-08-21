use soroban_sdk::{symbol_short, Address, Env, Map, Symbol, Vec};

pub const ADMIN_KEY: Symbol = symbol_short!("admin");
pub const SIGNERS_KEY: Symbol = symbol_short!("signers");
pub const THRESHOLD_KEY: Symbol = symbol_short!("threshold");
pub const PROPOSALS_KEY: Symbol = symbol_short!("proposals");
pub const PROPOSAL_COUNT_KEY: Symbol = symbol_short!("prop_cnt");
pub const PAUSED_KEY: Symbol = symbol_short!("paused");
pub const INITIALIZED_KEY: Symbol = symbol_short!("init");
pub const TIMELOCK_DELAY_KEY: Symbol = symbol_short!("tl_delay");
pub const IMPLEMENTATION_KEY: Symbol = symbol_short!("impl");
pub const UPGRADE_SCHEDULED_KEY: Symbol = symbol_short!("upg_sched");
pub const NONCE_KEY: Symbol = symbol_short!("nonce");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub action: ProposalAction,
    pub signatures: Vec<Address>,
    pub threshold: u32,
    pub created_at: u64,
    pub execute_after: u64,
    pub executed: bool,
    pub canceled: bool,
    pub nonce: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalAction {
    Pause,
    Unpause,
    Upgrade(Address),
    AddSigner(Address),
    RemoveSigner(Address),
    SetThreshold(u32),
    SetTimelockDelay(u64),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelockSchedule {
    pub proposal_id: u64,
    pub execute_at: u64,
    pub action: ProposalAction,
}

pub fn load_signers(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&SIGNERS_KEY)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn store_signers(env: &Env, signers: &Vec<Address>) {
    env.storage().persistent().set(&SIGNERS_KEY, signers);
}

pub fn load_threshold(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&THRESHOLD_KEY)
        .unwrap_or(0)
}

pub fn store_threshold(env: &Env, threshold: u32) {
    env.storage().persistent().set(&THRESHOLD_KEY, &threshold);
}

pub fn load_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&PAUSED_KEY)
        .unwrap_or(false)
}

pub fn store_paused(env: &Env, paused: bool) {
    env.storage().persistent().set(&PAUSED_KEY, &paused);
}

pub fn load_initialized(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&INITIALIZED_KEY)
        .unwrap_or(false)
}

pub fn store_initialized(env: &Env, initialized: bool) {
    env.storage().persistent().set(&INITIALIZED_KEY, &initialized);
}

pub fn load_timelock_delay(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&TIMELOCK_DELAY_KEY)
        .unwrap_or(0)
}

pub fn store_timelock_delay(env: &Env, delay: u64) {
    env.storage().persistent().set(&TIMELOCK_DELAY_KEY, &delay);
}

pub fn load_implementation(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get(&IMPLEMENTATION_KEY)
        .expect("Implementation not set")
}

pub fn store_implementation(env: &Env, impl_addr: &Address) {
    env.storage().persistent().set(&IMPLEMENTATION_KEY, impl_addr);
}

pub fn load_proposals(env: &Env) -> Map<u64, Proposal> {
    env.storage()
        .persistent()
        .get(&PROPOSALS_KEY)
        .unwrap_or_else(|| Map::new(env))
}

pub fn store_proposals(env: &Env, proposals: &Map<u64, Proposal>) {
    env.storage().persistent().set(&PROPOSALS_KEY, proposals);
}

pub fn load_proposal_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&PROPOSAL_COUNT_KEY)
        .unwrap_or(0)
}

pub fn store_proposal_count(env: &Env, count: u64) {
    env.storage().persistent().set(&PROPOSAL_COUNT_KEY, &count);
}

pub fn load_nonce(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&NONCE_KEY)
        .unwrap_or(0)
}

pub fn store_nonce(env: &Env, nonce: u64) {
    env.storage().persistent().set(&NONCE_KEY, &nonce);
}

pub fn load_upgrade_scheduled(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&UPGRADE_SCHEDULED_KEY)
}

pub fn store_upgrade_scheduled(env: &Env, impl_addr: Option<&Address>) {
    if let Some(addr) = impl_addr {
        env.storage().persistent().set(&UPGRADE_SCHEDULED_KEY, addr);
    } else {
        env.storage().persistent().remove(&UPGRADE_SCHEDULED_KEY);
    }
}
