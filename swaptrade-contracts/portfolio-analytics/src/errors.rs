use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PortfolioError {
    /// Caller not authorized for this operation
    Unauthorized = 1,
    /// Contract has already been initialized
    AlreadyExists = 2,
    /// No position found for the given asset
    PositionNotFound = 3,
    /// Attempted to sell more than current position quantity
    InsufficientQuantity = 4,
    /// Transaction ID counter overflow
    TxIdOverflow = 5,
    /// Snapshot ID counter overflow
    SnapshotIdOverflow = 6,
    /// Invalid quantity (must be > 0)
    InvalidQuantity = 7,
    /// Invalid price (must be > 0)
    InvalidPrice = 8,
    /// User address is the zero address
    InvalidAddress = 9,
    /// No price data available for the requested asset
    NoPriceData = 10,
    /// Portfolio has no positions to analyze
    EmptyPortfolio = 11,
    /// Insufficient history for metrics calculation (need at least 2 data points)
    InsufficientHistory = 12,
    /// Cost lot index out of bounds during FIFO/LIFO disposal
    NoCostLotsAvailable = 13,
    /// Snapshot timestamp must be in the future
    SnapshotTimestampInvalid = 14,
}
