use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TradeError {
    InvalidAmount = 1,
    InvalidPrice = 2,
    OrderNotFound = 3,
    Unauthorized = 4,
    Expired = 5,
    SlippageExceeded = 6,
    InsufficientLiquidity = 7,
    InvalidState = 8,
    SameAsset = 9,
    ExecutionFailed = 10,
    InvalidLegs = 11,
    AlreadyExists = 12,
}
