use soroban_sdk::{contracttype, Address, Env, Map, Vec};

use crate::storage::StorageKey;
use crate::types::{Order, OrderBookLevel, OrderBookSummary, OrderSide, OrderStatus};

/// Aggregated data for a single price level in the order book.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceLevel {
    /// Total remaining (unfilled) base-asset amount across all active orders at this price.
    pub total_amount: i128,
    /// Number of active (pending / partially-filled, non-expired) orders at this price.
    pub order_count: u32,
}

/// OrderBook state for a specific trading pair (base_asset, quote_asset).
///
/// Orders are bucketed by price so that:
///  - `add_order` is O(1) instead of O(n) (no sorted-insertion scan)
///  - `remove_order_id` touches only the affected price level
///  - `get_summary` uses pre-computed level aggregates instead of reading every order
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairOrderBook {
    pub base_asset: Address,
    pub quote_asset: Address,
    /// Bids (buy orders): price -> Vec<order_id>, ordered by creation time (FIFO).
    pub bids: Map<u128, Vec<u64>>,
    /// Asks (sell orders): price -> Vec<order_id>, ordered by creation time (FIFO).
    pub asks: Map<u128, Vec<u64>>,
    /// Running aggregate per bid price level.
    pub bid_levels: Map<u128, PriceLevel>,
    /// Running aggregate per ask price level.
    pub ask_levels: Map<u128, PriceLevel>,
}

impl PairOrderBook {
    pub fn new(env: &Env, base_asset: Address, quote_asset: Address) -> Self {
        Self {
            base_asset,
            quote_asset,
            bids: Map::new(env),
            asks: Map::new(env),
            bid_levels: Map::new(env),
            ask_levels: Map::new(env),
        }
    }
}

pub struct OrderBookManager;

impl OrderBookManager {
    /// Get canonical pair key: sorts asset addresses deterministically so (A, B) and (B, A) match the same orderbook.
    pub fn get_pair_key(_env: &Env, asset_1: &Address, asset_2: &Address) -> (StorageKey, bool) {
        if asset_1 < asset_2 {
            (
                StorageKey::OrderBook(asset_1.clone(), asset_2.clone()),
                true,
            )
        } else {
            (
                StorageKey::OrderBook(asset_2.clone(), asset_1.clone()),
                false,
            )
        }
    }

    /// Load pair order book from storage or create new.
    pub fn load_book(env: &Env, base_asset: &Address, quote_asset: &Address) -> PairOrderBook {
        let (key, _) = Self::get_pair_key(env, base_asset, quote_asset);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| PairOrderBook::new(env, base_asset.clone(), quote_asset.clone()))
    }

    /// Save pair order book to storage.
    pub fn save_book(env: &Env, book: &PairOrderBook) {
        let (key, _) = Self::get_pair_key(env, &book.base_asset, &book.quote_asset);
        env.storage().persistent().set(&key, book);
    }

    /// Add an order to the order book.
    ///
    /// With price-level bucketing this is O(1): we look up the price-level
    /// Vec in the Map and push the new order ID — no need to read existing
    /// orders or do a sorted-insertion scan.
    pub fn add_order(env: &Env, order: &Order) {
        let mut book = Self::load_book(env, &order.base_asset, &order.quote_asset);
        let remaining = order.amount.saturating_sub(order.filled_amount);

        match order.side {
            OrderSide::Buy => {
                let mut ids = book.bids.get(order.price).unwrap_or_else(|| Vec::new(env));
                ids.push_back(order.order_id);
                book.bids.set(order.price, ids);

                let mut level = book.bid_levels.get(order.price).unwrap_or(PriceLevel {
                    total_amount: 0,
                    order_count: 0,
                });
                level.total_amount = level.total_amount.saturating_add(remaining);
                level.order_count = level.order_count.saturating_add(1);
                book.bid_levels.set(order.price, level);
            }
            OrderSide::Sell => {
                let mut ids = book.asks.get(order.price).unwrap_or_else(|| Vec::new(env));
                ids.push_back(order.order_id);
                book.asks.set(order.price, ids);

                let mut level = book.ask_levels.get(order.price).unwrap_or(PriceLevel {
                    total_amount: 0,
                    order_count: 0,
                });
                level.total_amount = level.total_amount.saturating_add(remaining);
                level.order_count = level.order_count.saturating_add(1);
                book.ask_levels.set(order.price, level);
            }
        }

        Self::save_book(env, &book);
    }

    /// Remove an order ID from the book and update level aggregates.
    ///
    /// The caller passes `price` and `remaining` to avoid extra storage reads.
    pub fn remove_order_id(
        env: &Env,
        base_asset: &Address,
        quote_asset: &Address,
        side: OrderSide,
        order_id: u64,
        price: u128,
        remaining: i128,
    ) {
        let mut book = Self::load_book(env, base_asset, quote_asset);

        match side {
            OrderSide::Buy => {
                if let Some(ids) = book.bids.get(price) {
                    let mut new_ids = Vec::new(env);
                    for i in 0..ids.len() {
                        let id = ids.get(i).unwrap();
                        if id != order_id {
                            new_ids.push_back(id);
                        }
                    }
                    book.bids.set(price, new_ids);

                    if let Some(mut level) = book.bid_levels.get(price) {
                        level.total_amount = level.total_amount.saturating_sub(remaining);
                        level.order_count = level.order_count.saturating_sub(1);
                        if level.order_count == 0 {
                            book.bid_levels.remove(price);
                        } else {
                            book.bid_levels.set(price, level);
                        }
                    }
                }
            }
            OrderSide::Sell => {
                if let Some(ids) = book.asks.get(price) {
                    let mut new_ids = Vec::new(env);
                    for i in 0..ids.len() {
                        let id = ids.get(i).unwrap();
                        if id != order_id {
                            new_ids.push_back(id);
                        }
                    }
                    book.asks.set(price, new_ids);

                    if let Some(mut level) = book.ask_levels.get(price) {
                        level.total_amount = level.total_amount.saturating_sub(remaining);
                        level.order_count = level.order_count.saturating_sub(1);
                        if level.order_count == 0 {
                            book.ask_levels.remove(price);
                        } else {
                            book.ask_levels.set(price, level);
                        }
                    }
                }
            }
        }

        Self::save_book(env, &book);
    }

    /// Save individual order.
    pub fn save_order(env: &Env, order: &Order) {
        let key = StorageKey::Order(order.order_id);
        env.storage().persistent().set(&key, order);
    }

    /// Load individual order.
    pub fn get_order(env: &Env, order_id: u64) -> Option<Order> {
        let key = StorageKey::Order(order_id);
        env.storage().persistent().get(&key)
    }

    /// Get user active order IDs.
    pub fn get_user_orders(env: &Env, user: &Address) -> Vec<u64> {
        let key = StorageKey::UserOrders(user.clone());
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Add order to user's order list.
    pub fn add_user_order(env: &Env, user: &Address, order_id: u64) {
        let key = StorageKey::UserOrders(user.clone());
        let mut user_orders: Vec<u64> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        user_orders.push_back(order_id);
        env.storage().persistent().set(&key, &user_orders);
    }

    /// Update a single order's aggregate contribution at its price level.
    /// Called after a fill changes `filled_amount` or `status`.
    pub fn update_level_after_fill(
        _env: &Env,
        book: &mut PairOrderBook,
        order: &Order,
        filled: i128,
    ) {
        match order.side {
            OrderSide::Buy => {
                if let Some(mut level) = book.bid_levels.get(order.price) {
                    level.total_amount = level.total_amount.saturating_sub(filled);
                    if order.status == OrderStatus::Filled {
                        level.order_count = level.order_count.saturating_sub(1);
                    }
                    if level.order_count == 0 {
                        book.bid_levels.remove(order.price);
                    } else {
                        book.bid_levels.set(order.price, level);
                    }
                }
            }
            OrderSide::Sell => {
                if let Some(mut level) = book.ask_levels.get(order.price) {
                    level.total_amount = level.total_amount.saturating_sub(filled);
                    if order.status == OrderStatus::Filled {
                        level.order_count = level.order_count.saturating_sub(1);
                    }
                    if level.order_count == 0 {
                        book.ask_levels.remove(order.price);
                    } else {
                        book.ask_levels.set(order.price, level);
                    }
                }
            }
        }
    }

    /// Get aggregated OrderBook summary snapshot.
    ///
    /// Uses pre-computed level aggregates (total_amount, order_count) instead
    /// of reading every individual order from storage.
    pub fn get_summary(
        env: &Env,
        base_asset: Address,
        quote_asset: Address,
        max_levels: u32,
    ) -> OrderBookSummary {
        let book = Self::load_book(env, &base_asset, &quote_asset);
        let now = env.ledger().timestamp();

        let mut bids = Vec::new(env);
        let bid_keys = book.bid_levels.keys();
        let mut bid_count = 0u32;
        for i in 0..bid_keys.len() {
            if bid_count >= max_levels {
                break;
            }
            let price = bid_keys.get(i).unwrap();
            if let Some(level) = book.bid_levels.get(price) {
                if level.order_count > 0 && level.total_amount > 0 {
                    bids.push_back(OrderBookLevel {
                        price,
                        total_amount: level.total_amount,
                        order_count: level.order_count,
                    });
                    bid_count += 1;
                }
            }
        }

        let mut asks = Vec::new(env);
        let ask_keys = book.ask_levels.keys();
        let mut ask_count = 0u32;
        for i in 0..ask_keys.len() {
            if ask_count >= max_levels {
                break;
            }
            let price = ask_keys.get(i).unwrap();
            if let Some(level) = book.ask_levels.get(price) {
                if level.order_count > 0 && level.total_amount > 0 {
                    asks.push_back(OrderBookLevel {
                        price,
                        total_amount: level.total_amount,
                        order_count: level.order_count,
                    });
                    ask_count += 1;
                }
            }
        }

        OrderBookSummary {
            base_asset,
            quote_asset,
            bids,
            asks,
            timestamp: now,
        }
    }
}
