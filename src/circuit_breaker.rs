
#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol, Val};

#[contract]
pub struct CircuitBreaker;

#[contractimpl]
impl CircuitBreaker {
    /// Checks if the circuit breaker is open.
    pub fn is_open(env: &Env) -> bool {
        env.storage().instance().get(&Symbol::new(&env, "is_open")).unwrap_or(false)
    }

    /// Trips the circuit breaker.
    pub fn trip(env: &Env) {
        env.storage().instance().set(&Symbol::new(&env, "is_open"), &true);
    }

    /// Resets the circuit breaker.
    pub fn reset(env: &Env) {
        env.storage().instance().set(&Symbol::new(&env, "is_open"), &false);
    }
}