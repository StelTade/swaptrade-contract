use soroban_sdk::{symbol_short, Address, Env, Map, Symbol};

use crate::errors::SwapError;
use crate::types::Swap;

// ── Storage keys ─────────────────────────────────────────────
const SWAPS_KEY: Symbol = symbol_short!("swaps");
const NONCE_KEY: Symbol = symbol_short!("nonce");
const NEXT_ID_KEY: Symbol = symbol_short!("nxid");
const MIN_EXPIRY_KEY: Symbol = symbol_short!("minexp");

/// Default minimum expiry: 5 minutes.
const DEFAULT_MIN_EXPIRY: u64 = 300;

// ── Internal helpers ─────────────────────────────────────────

fn swap_map(env: &Env) -> Map<u64, Swap> {
    env.storage()
        .persistent()
        .get(&SWAPS_KEY)
        .unwrap_or_else(|| Map::new(env))
}

fn save_swap_map(env: &Env, map: &Map<u64, Swap>) {
    env.storage().persistent().set(&SWAPS_KEY, map);
}

fn nonce_index(env: &Env) -> Map<(Address, u64), u64> {
    env.storage()
        .persistent()
        .get(&NONCE_KEY)
        .unwrap_or_else(|| Map::new(env))
}

fn save_nonce_index(env: &Env, map: &Map<(Address, u64), u64>) {
    env.storage().persistent().set(&NONCE_KEY, map);
}

// ── ID & config ──────────────────────────────────────────────

pub fn next_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&NEXT_ID_KEY)
        .unwrap_or(1u64);
    env.storage().persistent().set(&NEXT_ID_KEY, &(id + 1));
    id
}

pub fn min_expiry(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&MIN_EXPIRY_KEY)
        .unwrap_or(DEFAULT_MIN_EXPIRY)
}

pub fn set_min_expiry(env: &Env, seconds: u64) {
    env.storage().persistent().set(&MIN_EXPIRY_KEY, &seconds);
}

// ── Nonce index ──────────────────────────────────────────────

pub fn find_by_nonce(env: &Env, creator: &Address, nonce: u64) -> Option<u64> {
    let idx = nonce_index(env);
    idx.get((creator.clone(), nonce))
}

// ── Swap CRUD ────────────────────────────────────────────────

pub fn save_swap(env: &Env, swap: &Swap) {
    let mut map = swap_map(env);
    map.set(swap.id, swap.clone());
    save_swap_map(env, &map);

    let mut idx = nonce_index(env);
    idx.set((swap.creator.clone(), swap.nonce), swap.id);
    save_nonce_index(env, &idx);
}

pub fn load_swap(env: &Env, id: u64) -> Result<Swap, SwapError> {
    let map = swap_map(env);
    map.get(id).ok_or(SwapError::SwapNotFound)
}

pub fn update_swap(env: &Env, swap: &Swap) {
    let mut map = swap_map(env);
    map.set(swap.id, swap.clone());
    save_swap_map(env, &map);
}
