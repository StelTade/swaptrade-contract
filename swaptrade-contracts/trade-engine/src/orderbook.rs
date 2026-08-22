use soroban_sdk::{contracttype, Address, Env, Map, Vec};

use crate::storage::StorageKey;
use crate::types::{Order, OrderBookLevel, OrderBookSummary, OrderSide, OrderStatus};

/// OrderBook state for a specific trading pair (base_asset, quote_asset)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairOrderBook {
    pub base_asset: Address,
    pub quote_asset: Address,
    pub bid_order_ids: Vec<u64>, // Buy orders
    pub ask_order_ids: Vec<u64>, // Sell orders
}

impl PairOrderBook {
    pub fn new(env: &Env, base_asset: Address, quote_asset: Address) -> Self {
        Self {
            base_asset,
            quote_asset,
            bid_order_ids: Vec::new(env),
            ask_order_ids: Vec::new(env),
        }
    }
}

pub struct OrderBookManager;

impl OrderBookManager {
    /// Get canonical pair key: sorts asset addresses deterministically so (A, B) and (B, A) match the same orderbook
    pub fn get_pair_key(env: &Env, asset_1: &Address, asset_2: &Address) -> (StorageKey, bool) {
        if asset_1 < asset_2 {
            (StorageKey::OrderBook(asset_1.clone(), asset_2.clone()), true)
        } else {
            (StorageKey::OrderBook(asset_2.clone(), asset_1.clone()), false)
        }
    }

    /// Load pair order book from storage or create new
    pub fn load_book(env: &Env, base_asset: &Address, quote_asset: &Address) -> PairOrderBook {
        let (key, _) = Self::get_pair_key(env, base_asset, quote_asset);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| PairOrderBook::new(env, base_asset.clone(), quote_asset.clone()))
    }

    /// Save pair order book to storage
    pub fn save_book(env: &Env, book: &PairOrderBook) {
        let (key, _) = Self::get_pair_key(env, &book.base_asset, &book.quote_asset);
        env.storage().persistent().set(&key, book);
    }

    /// Add an order to the order book maintaining time-price priority
    /// Bids: Highest price first, then earlier creation time
    /// Asks: Lowest price first, then earlier creation time
    pub fn add_order(env: &Env, order: &Order) {
        let mut book = Self::load_book(env, &order.base_asset, &order.quote_asset);

        match order.side {
            OrderSide::Buy => {
                let mut inserted = false;
                let mut new_bids = Vec::new(env);
                for i in 0..book.bid_order_ids.len() {
                    let existing_id = book.bid_order_ids.get(i).unwrap();
                    if let Some(existing_order) = Self::get_order(env, existing_id) {
                        // Higher price takes priority
                        if !inserted && order.price > existing_order.price {
                            new_bids.push_back(order.order_id);
                            inserted = true;
                        }
                    }
                    new_bids.push_back(existing_id);
                }
                if !inserted {
                    new_bids.push_back(order.order_id);
                }
                book.bid_order_ids = new_bids;
            }
            OrderSide::Sell => {
                let mut inserted = false;
                let mut new_asks = Vec::new(env);
                for i in 0..book.ask_order_ids.len() {
                    let existing_id = book.ask_order_ids.get(i).unwrap();
                    if let Some(existing_order) = Self::get_order(env, existing_id) {
                        // Lower price takes priority
                        if !inserted && order.price < existing_order.price {
                            new_asks.push_back(order.order_id);
                            inserted = true;
                        }
                    }
                    new_asks.push_back(existing_id);
                }
                if !inserted {
                    new_asks.push_back(order.order_id);
                }
                book.ask_order_ids = new_asks;
            }
        }

        Self::save_book(env, &book);
    }

    /// Remove an order ID from order book bids or asks
    pub fn remove_order_id(env: &Env, base_asset: &Address, quote_asset: &Address, side: OrderSide, order_id: u64) {
        let mut book = Self::load_book(env, base_asset, quote_asset);

        match side {
            OrderSide::Buy => {
                let mut new_bids = Vec::new(env);
                for i in 0..book.bid_order_ids.len() {
                    let id = book.bid_order_ids.get(i).unwrap();
                    if id != order_id {
                        new_bids.push_back(id);
                    }
                }
                book.bid_order_ids = new_bids;
            }
            OrderSide::Sell => {
                let mut new_asks = Vec::new(env);
                for i in 0..book.ask_order_ids.len() {
                    let id = book.ask_order_ids.get(i).unwrap();
                    if id != order_id {
                        new_asks.push_back(id);
                    }
                }
                book.ask_order_ids = new_asks;
            }
        }

        Self::save_book(env, &book);
    }

    /// Save individual order
    pub fn save_order(env: &Env, order: &Order) {
        let key = StorageKey::Order(order.order_id);
        env.storage().persistent().set(&key, order);
    }

    /// Load individual order
    pub fn get_order(env: &Env, order_id: u64) -> Option<Order> {
        let key = StorageKey::Order(order_id);
        env.storage().persistent().get(&key)
    }

    /// Get user active order IDs
    pub fn get_user_orders(env: &Env, user: &Address) -> Vec<u64> {
        let key = StorageKey::UserOrders(user.clone());
        env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env))
    }

    /// Add order to user's order list
    pub fn add_user_order(env: &Env, user: &Address, order_id: u64) {
        let key = StorageKey::UserOrders(user.clone());
        let mut user_orders: Vec<u64> = env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env));
        user_orders.push_back(order_id);
        env.storage().persistent().set(&key, &user_orders);
    }

    /// Get aggregated OrderBook summary snapshot
    pub fn get_summary(env: &Env, base_asset: Address, quote_asset: Address, max_levels: u32) -> OrderBookSummary {
        let book = Self::load_book(env, &base_asset, &quote_asset);
        let now = env.ledger().timestamp();

        let mut bid_map: Map<u128, (i128, u32)> = Map::new(env);
        let mut ask_map: Map<u128, (i128, u32)> = Map::new(env);

        // Aggregate bids
        for i in 0..book.bid_order_ids.len() {
            let id = book.bid_order_ids.get(i).unwrap();
            if let Some(order) = Self::get_order(env, id) {
                if (order.status == OrderStatus::Pending || order.status == OrderStatus::PartiallyFilled)
                    && (order.expires_at == 0 || order.expires_at > now)
                {
                    let remaining = order.amount.saturating_sub(order.filled_amount);
                    if remaining > 0 {
                        let (amount, count) = bid_map.get(order.price).unwrap_or((0, 0));
                        bid_map.set(order.price, (amount.saturating_add(remaining), count + 1));
                    }
                }
            }
        }

        // Aggregate asks
        for i in 0..book.ask_order_ids.len() {
            let id = book.ask_order_ids.get(i).unwrap();
            if let Some(order) = Self::get_order(env, id) {
                if (order.status == OrderStatus::Pending || order.status == OrderStatus::PartiallyFilled)
                    && (order.expires_at == 0 || order.expires_at > now)
                {
                    let remaining = order.amount.saturating_sub(order.filled_amount);
                    if remaining > 0 {
                        let (amount, count) = ask_map.get(order.price).unwrap_or((0, 0));
                        ask_map.set(order.price, (amount.saturating_add(remaining), count + 1));
                    }
                }
            }
        }

        let mut bids = Vec::new(env);
        let bid_keys = bid_map.keys();
        // Take top max_levels
        for i in 0..bid_keys.len() {
            if (i as u32) >= max_levels {
                break;
            }
            let price = bid_keys.get(i).unwrap();
            let (total_amount, order_count) = bid_map.get(price).unwrap();
            bids.push_back(OrderBookLevel {
                price,
                total_amount,
                order_count,
            });
        }

        let mut asks = Vec::new(env);
        let ask_keys = ask_map.keys();
        for i in 0..ask_keys.len() {
            if (i as u32) >= max_levels {
                break;
            }
            let price = ask_keys.get(i).unwrap();
            let (total_amount, order_count) = ask_map.get(price).unwrap();
            asks.push_back(OrderBookLevel {
                price,
                total_amount,
                order_count,
            });
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
