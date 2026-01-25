// counter/src/errors.rs
use soroban_sdk::{contracterror};

#[contracterror]
#[derive(Debug, Clone)]
pub enum ContractError {
    InvalidTokenSymbol = 1,      // Token symbol not recognized
    InsufficientBalance = 2,     // User has insufficient balance
    InvalidSwapPair = 3,         // Swap pair not supported
    ZeroAmountSwap = 4,          // Swap attempted with zero amount
    UnauthorizedAccess = 5,      // Caller lacks permission
    InvalidAmount = 6,
    AmountOverflow = 7,
    InvalidUserAddress = 8,
    PriceNotSet = 9,
    StalePrice = 10,
    InvalidPrice = 11,
}
