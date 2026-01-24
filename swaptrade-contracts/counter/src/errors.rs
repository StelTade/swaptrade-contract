// counter/src/errors.rs
use soroban_sdk::{contracterror};

#[contracterror]
#[derive(Debug, Clone)]
pub enum ContractError {
    InvalidTokenSymbol,      // Token symbol not recognized
    InsufficientBalance,     // User has insufficient balance
    InvalidSwapPair,         // Swap pair not supported
    ZeroAmountSwap,          // Swap attempted with zero amount
    UnauthorizedAccess,      // Caller lacks permission
}
