#![cfg_attr(all(not(test), target_family = "wasm"), no_std)]

mod types;

pub use types::{Swap, SwapState};

use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct AtomicSwapContract;

#[contractimpl]
impl AtomicSwapContract {
    /// Placeholder — will be replaced in subsequent commits.
    pub fn initialize(_env: Env) {}
}
