use soroban_sdk::{contracterror, contracttype};

/// Errors specific to the fee & incentives module.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FeeError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    InvalidFeeConfig = 5,
    FeeTooHigh = 6,
    NoRewardsToClaim = 7,
    InsufficientBalance = 8,
    ReplayedClaim = 9,
    OperationNotAllowed = 10,
    PairNotFound = 11,
    ZeroAddress = 12,
}

/// Describes the type of operation that triggered fee collection.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeeOperation {
    Swap,       // Orderbook match
    PoolSwap,   // AMM/fallback pool swap
    OrderFill,  // Individual order fill
}

/// Where a fee portion is routed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeeDestination {
    Treasury,
    LpPool,
    Relayer,
}
