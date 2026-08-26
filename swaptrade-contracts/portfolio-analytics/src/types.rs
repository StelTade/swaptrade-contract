use soroban_sdk::{contracttype, Address, Vec};

/// Cost basis accounting method for position tracking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CostBasisMethod {
    /// First-In-First-Out: earliest acquired lots are sold first
    Fifo,
    /// Last-In-First-Out: most recently acquired lots are sold first
    Lifo,
    /// Weighted Average: single blended cost basis across all lots
    WeightedAverage,
}

/// Type of transaction that affects a position
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransactionType {
    /// Buy/opening a new position or adding to an existing one
    Buy,
    /// Sell/partially or fully closing a position
    Sell,
    /// Transfer in from external source (e.g., airdrop, bridge)
    TransferIn,
    /// Transfer out to external destination
    TransferOut,
}

/// Individual cost lot for FIFO/LIFO position tracking.
/// Each lot records the cost at time of acquisition, allowing
/// precise realized P&L calculation when lots are disposed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CostLot {
    /// Timestamp when this lot was acquired
    pub timestamp: u64,
    /// Quantity of units in this lot
    pub quantity: i128,
    /// Cost per unit at time of acquisition (in quote terms)
    pub cost_per_unit: i128,
    /// Total cost for this lot = quantity * cost_per_unit
    pub total_cost: i128,
}

/// Represents a user's open position in a single asset.
/// Tracks quantity, cost basis, realized P&L, and holds
/// cost lots for FIFO/LIFO accounting.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    /// Owner of this position
    pub user: Address,
    /// Asset held in this position
    pub asset: Address,
    /// Quote asset used for pricing (e.g., USDC)
    pub quote_asset: Address,
    /// Current quantity held
    pub quantity: i128,
    /// Average cost basis per unit (in quote terms, scaled by PRICE_PRECISION)
    pub avg_cost_basis: i128,
    /// Total cost basis of current holdings
    pub total_cost_basis: i128,
    /// Cumulative realized P&L from all closed portions
    pub realized_pnl: i128,
    /// Total quote spent on buys
    pub total_invested: i128,
    /// Total quote received from sells
    pub total_divested: i128,
    /// Number of buy transactions on this position
    pub buy_count: u64,
    /// Number of sell transactions on this position
    pub sell_count: u64,
    /// Cost basis accounting method
    pub cost_method: CostBasisMethod,
    /// Cost lots for FIFO/LIFO tracking
    pub cost_lots: Vec<CostLot>,
    /// Timestamp of last update
    pub last_updated: u64,
    /// Timestamp when position was first opened
    pub open_time: u64,
}

/// Summary of a position for snapshot reporting (excludes cost lots for efficiency)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionSummary {
    pub asset: Address,
    pub quote_asset: Address,
    pub quantity: i128,
    pub avg_cost_basis: i128,
    pub total_cost_basis: i128,
    pub realized_pnl: i128,
    pub unrealized_pnl: i128,
    pub market_value: i128,
    pub allocation_pct: u32, // Basis points (100 = 1%)
}

/// Immutable transaction record for audit trail and historical analysis.
/// Every position change creates a new record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionRecord {
    /// Sequential transaction ID
    pub tx_id: u64,
    /// User who initiated the transaction
    pub user: Address,
    /// Asset traded
    pub asset: Address,
    /// Quote asset used for pricing
    pub quote_asset: Address,
    /// Type of transaction (Buy, Sell, TransferIn, TransferOut)
    pub tx_type: TransactionType,
    /// Quantity of base asset transacted
    pub quantity: i128,
    /// Execution price per unit (scaled by PRICE_PRECISION)
    pub price: i128,
    /// Total value in quote terms
    pub total_value: i128,
    /// Realized P&L from this transaction (0 for buys)
    pub realized_pnl: i128,
    /// Remaining position quantity after this transaction
    pub remaining_quantity: i128,
    /// Timestamp of the transaction
    pub timestamp: u64,
}

/// Point-in-time snapshot of a user's complete portfolio state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioSnapshot {
    /// Sequential snapshot ID
    pub snapshot_id: u64,
    /// Portfolio owner
    pub user: Address,
    /// Timestamp when snapshot was taken
    pub timestamp: u64,
    /// Individual position summaries
    pub positions: Vec<PositionSummary>,
    /// Total portfolio market value in quote terms
    pub total_value: i128,
    /// Total cost basis across all positions
    pub total_cost_basis: i128,
    /// Total cumulative realized P&L
    pub total_realized_pnl: i128,
    /// Total unrealized P&L across all positions
    pub total_unrealized_pnl: i128,
    /// Total invested across all positions
    pub total_invested: i128,
    /// Total returned from all sells
    pub total_divested: i128,
}

/// Portfolio-level performance metrics.
/// Computed from historical transaction data and current prices.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceMetrics {
    /// User these metrics belong to
    pub user: Address,
    /// Total return on investment in basis points (100 = 1%)
    /// Calculated as: (total_value - total_cost_basis) / total_cost_basis * 10000
    pub roi_bps: i128,
    /// Annualized Sharpe ratio (scaled by 10_000 for 4 decimal places)
    /// Calculated as: (avg_return - risk_free_rate) / std_deviation
    pub sharpe_ratio: i128,
    /// Win/loss ratio in basis points (10000 = 1.0)
    pub win_loss_ratio: i128,
    /// Total number of closed trade pairs (buy+sell cycles)
    pub total_trades: u64,
    /// Number of profitable closed trades
    pub winning_trades: u64,
    /// Number of losing closed trades
    pub losing_trades: u64,
    /// Maximum drawdown in basis points
    pub max_drawdown_bps: i128,
    /// Portfolio volatility in basis points (annualized)
    pub volatility_bps: i128,
    /// Current total portfolio value in quote terms
    pub total_value: i128,
    /// Total cost basis
    pub total_cost_basis: i128,
    /// Cumulative realized P&L
    pub realized_pnl: i128,
    /// Current unrealized P&L
    pub unrealized_pnl: i128,
}

/// Current market price for an asset (from oracle)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPrice {
    /// The asset
    pub asset: Address,
    /// Price in quote terms (scaled by PRICE_PRECISION)
    pub price: u128,
    /// Timestamp of price update
    pub timestamp: u64,
}

/// Historical return data point for Sharpe/volatility calculations
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnPeriod {
    /// Start timestamp of the period
    pub start_timestamp: u64,
    /// End timestamp of the period
    pub end_timestamp: u64,
    /// Return during this period in basis points
    pub return_bps: i128,
    /// Portfolio value at start
    pub start_value: i128,
    /// Portfolio value at end
    pub end_value: i128,
}

/// Precision scale for prices (10^7 = 10,000,000)
pub const PRICE_PRECISION: i128 = 10_000_000;

/// Basis point scale (100 bps = 1%)
pub const BPS_PRECISION: i128 = 10_000;
