// rewards_stub.rs
//
// Stub rewards module for seasons integration.

use soroban_sdk::{Address, Env};

pub use crate::portfolio::Badge;

/// Award a badge to a user (stub - actual logic is in portfolio).
pub fn award_badge(_env: &Env, _user: &Address, _badge: Badge) {
    // Badge awarding is handled by the portfolio system.
    // This stub exists for seasons.rs compatibility.
}
