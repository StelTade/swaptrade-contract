use soroban_sdk::{symbol_short, Address, Env, IntoVal, Map, Symbol, Vec};

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
    let id: u64 = env.storage().persistent().get(&NEXT_ID_KEY).unwrap_or(1u64);
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

// ── Trustline check ──────────────────────────────────────────

/// Returns `true` if `address` holds a trustline for `asset`.
///
/// Invokes the Stellar Asset Contract's `balance(address) → i128`.
/// If the call succeeds the trustline exists; if it traps, it doesn't.
pub fn has_trustline(env: &Env, address: &Address, asset: &Address) -> bool {
    // `try_invoke_contract` returns Result<Result<T, E>, InvokeError>.
    // We only care whether the outer + inner are both Ok.
    let args: Vec<soroban_sdk::Val> = Vec::from_array(env, [address.to_val()]);
    let result =
        env.try_invoke_contract::<i128, SwapError>(asset, &Symbol::new(env, "balance"), args);
    matches!(result, Ok(Ok(_)))
}

// ── Token transfer ───────────────────────────────────────────

/// Internal transfer helper.  Does NOT call `require_auth` — the caller
/// is responsible for ensuring authorization has been obtained.
fn invoke_transfer(
    env: &Env,
    asset: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<i128, SwapError> {
    let args: Vec<soroban_sdk::Val> =
        soroban_sdk::vec![env, from.to_val(), to.to_val(), amount.into_val(env),];
    let result =
        env.try_invoke_contract::<i128, SwapError>(asset, &Symbol::new(env, "transfer"), args);
    match result {
        Ok(Ok(transferred)) => Ok(transferred),
        Ok(Err(_swap_err)) => Err(SwapError::TransferMismatch),
        Err(_invoke_err) => Err(SwapError::TransferMismatch),
    }
}

/// Transfer tokens via the Stellar Asset Contract's `transfer`.
/// Requires auth on `from`.
pub fn transfer_token(
    env: &Env,
    asset: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<i128, SwapError> {
    from.require_auth();
    invoke_transfer(env, asset, from, to, amount)
}

/// Transfer tokens without requiring auth on `from` (for internal
/// bookkeeping when auth is satisfied at the entry point).
pub fn transfer_token_no_auth(
    env: &Env,
    asset: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<i128, SwapError> {
    invoke_transfer(env, asset, from, to, amount)
}
