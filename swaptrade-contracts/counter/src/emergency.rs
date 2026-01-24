// src/emergency.rs
extern crate alloc;
use soroban_sdk::{contracttype, Address, Env, Map, Symbol, Vec, symbol_short};

#[contracttype]
pub struct StateSnapshot {
    pub balances: Vec<((Address, Symbol), i128)>,
    pub pool_xlm: i128,
    pub pool_usdc: i128,
    pub total_fees: i128,
    pub badges: Vec<((Address, Symbol), bool)>,
    pub tiers: Vec<(Address, Symbol)>,
    pub paused: bool,
    pub frozen_users: Vec<Address>,
    pub block_volume: Vec<(u64, i128)>,
}

#[contracttype]
pub enum EmergencyKey {
    Paused,
    FrozenUsers,
    BlockVolume,
    ThresholdBps,
    Admin,
}

pub fn set_admin(env: &Env, admin: Address) {
    env.storage().instance().set(&EmergencyKey::Admin, &admin);
}

pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&EmergencyKey::Admin)
}

pub fn is_admin(env: &Env, addr: Address) -> bool {
    match get_admin(env) {
        Some(a) => a == addr,
        None => false,
    }
}

pub fn pause(env: &Env, admin: Address) -> bool {
    assert!(is_admin(env, admin), "Not authorized");
    env.storage().instance().set(&EmergencyKey::Paused, &true);
    true
}

pub fn unpause(env: &Env, admin: Address) -> bool {
    assert!(is_admin(env, admin), "Not authorized");
    env.storage().instance().set(&EmergencyKey::Paused, &false);
    true
}

pub fn is_paused(env: &Env) -> bool {
    env.storage().instance().get(&EmergencyKey::Paused).unwrap_or(false)
}

pub fn freeze_user(env: &Env, admin: Address, user: Address) -> bool {
    assert!(is_admin(env, admin), "Not authorized");

    let mut frozen: Vec<Address> =
        env.storage().instance().get(&EmergencyKey::FrozenUsers).unwrap_or(Vec::new(env));

    if !frozen.contains(&user) {
        frozen.push_back(user.clone());
        env.storage().instance().set(&EmergencyKey::FrozenUsers, &frozen);
    }
    true
}

pub fn unfreeze_user(env: &Env, admin: Address, user: Address) -> bool {
    assert!(is_admin(env, admin), "Not authorized");

    let mut frozen: Vec<Address> =
        env.storage().instance().get(&EmergencyKey::FrozenUsers).unwrap_or(Vec::new(env));

    let mut new_frozen = Vec::new(env);
    for i in 0..frozen.len() {
        if let Some(u) = frozen.get(i) {
            if u != user {
                new_frozen.push_back(u);
            }
        }
    }

    env.storage().instance().set(&EmergencyKey::FrozenUsers, &new_frozen);
    true
}

pub fn is_frozen(env: &Env, user: Address) -> bool {
    let frozen: Vec<Address> =
        env.storage().instance().get(&EmergencyKey::FrozenUsers).unwrap_or(Vec::new(env));
    frozen.contains(&user)
}

// Circuit breaker settings
pub fn set_threshold_bps(env: &Env, admin: Address, bps: u32) {
    assert!(is_admin(env, admin), "Not authorized");
    env.storage().instance().set(&EmergencyKey::ThresholdBps, &bps);
}

pub fn get_threshold_bps(env: &Env) -> u32 {
    env.storage().instance().get(&EmergencyKey::ThresholdBps).unwrap_or(10000u32)
}

// Circuit breaker check
pub fn record_volume(env: &Env, amount: i128) {
    let height = env.ledger().sequence();
    let mut volume_map: Map<u64, i128> =
        env.storage().instance().get(&EmergencyKey::BlockVolume).unwrap_or(Map::new(env));

    let current = volume_map.get(height.into()).unwrap_or(0);
    volume_map.set(height.into(), current.saturating_add(amount));
    env.storage().instance().set(&EmergencyKey::BlockVolume, &volume_map);
}

pub fn get_block_volume(env: &Env, height: u64) -> i128 {
    let volume_map: Map<u64, i128> =
        env.storage().instance().get(&EmergencyKey::BlockVolume).unwrap_or(Map::new(env));
    volume_map.get(height).unwrap_or(0)
}

pub fn circuit_breaker_check(env: &Env, amount: i128, normal_volume: i128) {
    let threshold = get_threshold_bps(env);
    let height = env.ledger().sequence();
    let current = get_block_volume(env, height.into());
    let projected = current.saturating_add(amount);

    // If current > normal * threshold, pause
    if normal_volume > 0 {
        let ratio_bps = (projected * 10000) / normal_volume;
        if ratio_bps > threshold as i128 {
            env.storage().instance().set(&EmergencyKey::Paused, &true);
        }
    }
}

pub fn snapshot(env: &Env, portfolio: &crate::portfolio::Portfolio) -> StateSnapshot {
    // Snapshot balances
    let mut balances: Vec<((Address, Symbol), i128)> = Vec::new(env);
    // NOTE: In production, you'd iterate over a stored list of addresses.
    // For this example, we only snapshot what is easily retrievable.
    // (You may add an "addresses" list in Portfolio later.)

    // Snapshot pool
    let (xlm, usdc, fees) = portfolio.get_pool_stats();

    // Snapshot badges and tiers
    let mut badges: Vec<((Address, Symbol), bool)> = Vec::new(env);
    let mut tiers: Vec<(Address, Symbol)> = Vec::new(env);

    let frozen: Vec<Address> =
        env.storage().instance().get(&EmergencyKey::FrozenUsers).unwrap_or(Vec::new(env));

    let mut block_volume: Vec<(u64, i128)> = Vec::new(env);
    let volume_map: Map<u64, i128> =
        env.storage().instance().get(&EmergencyKey::BlockVolume).unwrap_or(Map::new(env));

    let keys = volume_map.keys();
    for key in keys.iter() {
        if let Some(v) = volume_map.get(key.clone()) {
            block_volume.push_back((key, v));
        }
    }

    StateSnapshot {
        balances,
        pool_xlm: xlm,
        pool_usdc: usdc,
        total_fees: fees,
        badges,
        tiers,
        paused: is_paused(env),
        frozen_users: frozen,
        block_volume,
    }
}
