#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

const COUNTER: Symbol = symbol_short!("COUNTER");

#[contract]
pub struct SwapTradeContract;

#[contractimpl]
impl SwapTradeContract {
    /// Initialize the contract
    pub fn init(env: Env) -> u32 {
        env.storage().instance().set(&COUNTER, &0u32);
        0
    }

    /// Increment the counter
    pub fn increment(env: Env) -> u32 {
        let mut count: u32 = env.storage().instance().get(&COUNTER).unwrap_or(0);
        count += 1;
        env.storage().instance().set(&COUNTER, &count);
        count
    }

    /// Get the current count
    pub fn get_count(env: Env) -> u32 {
        env.storage().instance().get(&COUNTER).unwrap_or(0)
    }
}

mod test;
