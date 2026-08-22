use soroban_sdk::{contracttype, Address, Vec};

/// Side of the order (Buy or Sell)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderSide {
    Buy,  // Buy base asset with quote asset
    Sell, // Sell base asset for quote asset
}

/// Type of order
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderType {
    Limit,              // Execute at limit price or better
    Market,             // Execute at best available price
    ImmediateOrCancel,  // Fill immediately whatever possible, cancel rest
    FillOrKill,         // Fill entirely immediately or cancel entire order
}

/// Status of an order
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
}

/// Order structure representing a single bid or ask in the order book
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Order {
    pub order_id: u64,
    pub owner: Address,
    pub base_asset: Address,
    pub quote_asset: Address,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: u128,              // Price scaled by PRICE_PRECISION (1_000_000_000)
    pub amount: i128,             // Total amount of base asset
    pub filled_amount: i128,      // Amount of base asset filled so far
    pub status: OrderStatus,
    pub created_at: u64,
    pub expires_at: u64,          // 0 means no expiration
}

/// Summary of orders at a specific price level in the order book
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderBookLevel {
    pub price: u128,
    pub total_amount: i128,
    pub order_count: u32,
}

/// Full snapshot of the order book for an asset pair
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderBookSummary {
    pub base_asset: Address,
    pub quote_asset: Address,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: u64,
}

/// Single leg specification in a multi-pair atomic trade
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeLeg {
    pub base_asset: Address,
    pub quote_asset: Address,
    pub side: OrderSide,
    pub amount: i128,             // Desired base asset amount to trade
    pub limit_price: u128,        // Max price for Buy, Min price for Sell (0 = no limit)
    pub min_output_amount: i128,  // Slippage protection: min quote received for Sell, min base received for Buy
}

/// Detail of a single fill execution (order book or fallback pool match)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FillResult {
    pub order_id: u64,
    pub maker: Address,
    pub taker: Address,
    pub base_asset: Address,
    pub quote_asset: Address,
    pub price: u128,
    pub filled_base: i128,
    pub filled_quote: i128,
    pub filled_via_pool: bool,
    pub pool_id: u64,
}

/// Result summary of an atomic multi-pair trade execution
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeExecutionResult {
    pub success: bool,
    pub legs_executed: u32,
    pub fills: Vec<FillResult>,
}

/// Liquidity Pool structure for fallback liquidity and price discovery
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityPool {
    pub pool_id: u64,
    pub asset_a: Address,
    pub asset_b: Address,
    pub reserve_a: i128,
    pub reserve_b: i128,
    pub fee_bps: u32,             // Basis points fee (e.g. 30 = 0.3%)
}

/// Precision scale for order book prices (10^7 = 10,000,000)
pub const PRICE_PRECISION: u128 = 10_000_000;
