// emergency_stub.rs
//
// Stub emergency module for trading integration.
// This provides the basic emergency state management that trading.rs and swap.rs need.

use soroban_sdk::{symbol_short, Address, Env, Symbol};

const PAUSED_KEY: Symbol = symbol_short!("em_paused");
const FROZEN_KEY: Symbol = symbol_short!("em_frozen");

/// Check if the system is emergency-paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage().persistent().get(&PAUSED_KEY).unwrap_or(false)
}

/// Set the emergency pause state.
pub fn set_paused(env: &Env, paused: bool) {
    env.storage().persistent().set(&PAUSED_KEY, &paused);
}

/// Check if a specific user is frozen.
pub fn is_frozen(env: &Env, user: Address) -> bool {
    let key = (FROZEN_KEY, user);
    env.storage().persistent().get(&key).unwrap_or(false)
}

/// Freeze a specific user.
pub fn set_frozen(env: &Env, user: Address, frozen: bool) {
    let key = (FROZEN_KEY, user);
    env.storage().persistent().set(&key, &frozen);
}

/// Check if a trade amount would trip the circuit breaker.
pub fn would_trip_circuit_breaker(_env: &Env, _amount: i128, _normal_volume: i128) -> bool {
    // Stub: never trip
    false
}

/// Record a trade volume for circuit breaker tracking.
pub fn record_volume(_env: &Env, _amount: i128) {
    // Stub: no-op
}
