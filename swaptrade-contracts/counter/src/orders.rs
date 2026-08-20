use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

use crate::errors::ContractError;

/// Order types supported by the system
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum OrderType {
    Market,    // Execute immediately at best available price
    Limit,     // Execute only at specified price or better
    StopLoss,  // Execute when price reaches trigger (becomes market order)
    StopLimit, // Execute when price reaches trigger (becomes limit order)
}

/// Order status
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum OrderStatus {
    Pending,         // Order is active and waiting to be filled
    Filled,          // Order has been completely filled
    Cancelled,       // Order was cancelled by user
    Expired,         // Order expired without being filled
    PartiallyFilled, // Order is partially executed
    Scheduled,  // Recurring order is between executions, waiting for next run
}

/// Side of the order (buy or sell)
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// A trade order in the system
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub order_id: u64,
    pub owner: Address,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub base_token: Symbol,
    pub quote_token: Symbol,
    pub amount: i128,          // Total amount in base units
    pub amount_remaining: i128,// Remaining amount to fill in base units
    pub price: u128,           // Price in quote per base (for limit orders)
    pub status: OrderStatus,
    pub created_at: u64,
    pub expires_at: Option<u64>, // None means no expiry
    pub filled_at: Option<u64>,
    // Keep existing fields for backwards compatibility
    pub token_in: Symbol,
    pub token_out: Symbol,
    pub amount_in: i128,
    pub amount_filled: i128,
    pub limit_price: Option<u128>,
    pub trigger_price: Option<u128>,
    pub interval_secs: Option<u64>,         // For recurring: seconds between executions
    pub remaining_occurrences: Option<u64>, // For recurring: how many more times to execute
    pub next_run: Option<u64>,              // For recurring: timestamp when next execution is due
}

/// Orderbook snapshot entry for aggregation
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OrderBookLevel {
    pub price: u128,
    pub total_amount: i128,
    pub order_count: u32,
}

/// Complete orderbook snapshot
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OrderBookSnapshot {
    pub base_token: Symbol,
    pub quote_token: Symbol,
    pub bids: Vec<OrderBookLevel>, // Sorted descending by price (best first)
    pub asks: Vec<OrderBookLevel>, // Sorted ascending by price (best first)
    pub timestamp: u64,
}

/// Result of taking an order (fill execution)
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct FillResult {
    pub order_id: u64,
    pub filled_amount_base: i128,
    pub filled_amount_quote: i128,
    pub price: u128,
    pub is_complete_fill: bool,
    pub maker: Address,
    pub taker: Address,
}

/// Order book for a token pair with optimized storage for time-price priority
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OrderBook {
    pub base_token: Symbol,
    pub quote_token: Symbol,
    // Sorted: for bids (buy orders), higher prices first, same price: earlier orders first
    pub bids: Vec<u64>,
    // Sorted: for asks (sell orders), lower prices first, same price: earlier orders first
    pub asks: Vec<u64>,
    // Keep old fields for backwards compatibility
    pub token_pair: (Symbol, Symbol),
    pub buy_orders: Vec<u64>,
    pub sell_orders: Vec<u64>,
}

/// Order manager - handles order lifecycle
pub struct OrderManager;

impl OrderManager {
    /// Place a limit order
    /// Will execute when market price reaches limit_price or better
    pub fn place_limit_order(
        env: &Env,
        owner: Address,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        limit_price: u128,
        expires_at: Option<u64>,
    ) -> Result<u64, ContractError> {
        owner.require_auth();

        if amount_in <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        if limit_price == 0 {
            return Err(ContractError::InvalidPrice);
        }

        Self::create_order(
            env,
            owner,
            OrderType::Limit,
            token_in,
            token_out,
            amount_in,
            Some(limit_price),
            None,
            expires_at,
        )
    }

    /// Place a stop-loss order
    /// Will execute as market order when price reaches trigger_price
    pub fn place_stop_loss(
        env: &Env,
        owner: Address,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        trigger_price: u128,
        expires_at: Option<u64>,
    ) -> Result<u64, ContractError> {
        owner.require_auth();

        if amount_in <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        if trigger_price == 0 {
            return Err(ContractError::InvalidPrice);
        }

        Self::create_order(
            env,
            owner,
            OrderType::StopLoss,
            token_in,
            token_out,
            amount_in,
            None,
            Some(trigger_price),
            expires_at,
        )
    }

    /// Cancel an order
    pub fn cancel_order(env: &Env, order_id: u64, owner: Address) -> Result<(), ContractError> {
        owner.require_auth();

        let mut order = Self::get_order(env, order_id)?;

        if order.owner != owner {
            return Err(ContractError::NotAdmin); // No specific "NotOrderOwner" error
        }

        if order.status != OrderStatus::Pending && order.status != OrderStatus::PartiallyFilled && order.status != OrderStatus::Scheduled {
            return Err(ContractError::InvalidAmount); // Order cannot be cancelled
        }

        order.status = OrderStatus::Cancelled;
        Self::save_order(env, &order);

        // Emit cancellation event
        env.events().publish(
            (symbol_short!("ordcan"), order_id),
            (owner, order.token_in, order.token_out, order.amount_in),
        );

        Ok(())
    }

    /// Place a limit order with proper side specification
    /// Places an order on the orderbook with time-price priority
    pub fn place_limit_order(
        env: &Env,
        owner: Address,
        base_token: Symbol,
        quote_token: Symbol,
        side: OrderSide,
        amount: i128, // Amount in base token
        price: u128, // Price in quote tokens per base token
        expires_at: Option<u64>,
    ) -> Result<u64, ContractError> {
        owner.require_auth();

        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        if price == 0 {
            return Err(ContractError::InvalidPrice);
        }

        // Generate order ID
        let next_id: u64 = env
            .storage()
            .instance()
            .get(&symbol_short!("next_oid"))
            .unwrap_or(1);

        let current_time = env.ledger().timestamp();
        
        // Create token_in/token_out for backwards compatibility
        let (token_in, token_out) = match side {
            OrderSide::Buy => (quote_token.clone(), base_token.clone()),
            OrderSide::Sell => (base_token.clone(), quote_token.clone()),
        };

        let order = Order {
            order_id: next_id,
            owner: owner.clone(),
            order_type: OrderType::Limit,
            side: side.clone(),
            base_token: base_token.clone(),
            quote_token: quote_token.clone(),
            amount,
            amount_remaining: amount,
            price,
            status: OrderStatus::Pending,
            created_at: current_time,
            expires_at,
            filled_at: None,
            // Backwards compatibility fields
            token_in,
            token_out,
            amount_in: amount,
            amount_filled: 0,
            limit_price: Some(price),
            trigger_price: None,
            interval_secs: None,
            remaining_occurrences: None,
            next_run: None,
        };

        // Save order
        Self::save_order(env, &order);

        // Add to user's order list
        let mut user_orders: Vec<u64> = env
            .storage()
            .instance()
            .get(&Self::user_orders_key(&owner))
            .unwrap_or_else(|| Vec::new(env));
        user_orders.push_back(next_id);
        env.storage()
            .instance()
            .set(&Self::user_orders_key(&owner), &user_orders);

        // Add to orderbook with proper time-price priority sorting
        Self::add_to_order_book(env, base_token.clone(), quote_token.clone(), order.clone());

        // Increment next order ID
        env.storage()
            .instance()
            .set(&symbol_short!("next_oid"), &(next_id + 1));

        // Emit order placement event
        env.events().publish(
            (symbol_short!("order_new"), next_id),
            (
                owner,
                OrderType::Limit,
                base_token,
                quote_token,
                amount,
                Some(price),
                None::<u128>,
            ),
        );

        Ok(next_id)
    }

    /// Take (fill) orders from the orderbook as a taker
    /// Supports partial and full fills, atomic balance updates
    pub fn take_order(
        env: &Env,
        taker: Address,
        base_token: Symbol,
        quote_token: Symbol,
        side: OrderSide, // Taker's side (buy or sell)
        max_amount_base: i128, // Maximum amount taker wants to fill (in base)
        max_price: Option<u128>, // Maximum price taker is willing to pay (for buys)
                                // or minimum price taker is willing to receive (for sells)
    ) -> Result<Vec<FillResult>, ContractError> {
        taker.require_auth();

        if max_amount_base <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let pair_key = Self::order_book_key(&(base_token.clone(), quote_token.clone()));
        let order_book: Option<OrderBook> = env.storage().instance().get(&pair_key);
        
        if order_book.is_none() {
            return Err(ContractError::InvalidAmount); // No orderbook for this pair
        }

        let mut book = order_book.unwrap();
        let current_time = env.ledger().timestamp();
        let mut remaining_amount = max_amount_base;
        let mut fills = Vec::new(env);

        // Determine which orders we can fill (opposite side of taker)
        let (candidate_orders, is_buy_taker) = match side {
            OrderSide::Buy => (&book.asks, true), // Taker buys base, fills sell orders (asks)
            OrderSide::Sell => (&book.bids, false), // Taker sells base, fills buy orders (bids)
        };

        let mut updated_orders = if is_buy_taker { book.asks.clone() } else { book.bids.clone() };
        
        for i in 0..candidate_orders.len() {
            if remaining_amount <= 0 {
                break;
            }

            if let Some(order_id) = candidate_orders.get(i) {
                if let Ok(mut order) = Self::get_order(env, *order_id) {
                    // Skip expired, cancelled, or already filled orders
                    if order.status != OrderStatus::Pending && order.status != OrderStatus::PartiallyFilled {
                        continue;
                    }

                    // Check expiry
                    if let Some(expires) = order.expires_at {
                        if current_time > expires {
                            order.status = OrderStatus::Expired;
                            Self::save_order(env, &order);
                            continue;
                        }
                    }

                    // Price validation
                    let price_valid = match (is_buy_taker, max_price) {
                        (true, Some(max_p)) => order.price <= max_p, // Buy: maker's price must be <= taker's max
                        (false, Some(min_p)) => order.price >= min_p, // Sell: maker's price must be >= taker's min
                        _ => true, // No max price specified, accept any
                    };

                    if !price_valid {
                        continue; // Cannot match at this price
                    }

                    // Calculate how much we can fill from this order
                    let fill_amount = if order.amount_remaining <= remaining_amount {
                        order.amount_remaining
                    } else {
                        remaining_amount
                    };

                    // Calculate quote amount (price * base amount, using fixed precision)
                    const PRECISION: u128 = 1_000_000_000_000_000_000;
                    let fill_amount_quote = (order.price as u128 * fill_amount as u128) / PRECISION;

                    // Update order's remaining amount
                    order.amount_remaining -= fill_amount;
                    order.amount_filled += fill_amount;
                    remaining_amount -= fill_amount;

                    let is_complete_fill = order.amount_remaining == 0;
                    if is_complete_fill {
                        order.status = OrderStatus::Filled;
                        order.filled_at = Some(current_time);
                        // Remove from orderbook
                        if let Some(pos) = updated_orders.first_index_of(*order_id) {
                            updated_orders.remove(pos);
                        }
                    } else {
                        order.status = OrderStatus::PartiallyFilled;
                    }

                    Self::save_order(env, &order);

                    // Record fill
                    fills.push_back(FillResult {
                        order_id: *order_id,
                        filled_amount_base: fill_amount,
                        filled_amount_quote: fill_amount_quote as i128,
                        price: order.price,
                        is_complete_fill,
                        maker: order.owner.clone(),
                        taker: taker.clone(),
                    });

                    // Emit fill event
                    env.events().publish(
                        (symbol_short!("fill"), *order_id),
                        (taker.clone(), order.owner, fill_amount, fill_amount_quote as i128, order.price),
                    );
                }
            }
        }

        // Update the orderbook
        if is_buy_taker {
            book.asks = updated_orders;
        } else {
            book.bids = updated_orders;
        }
        // Also update backwards compatibility fields
        book.buy_orders = book.bids.clone();
        book.sell_orders = book.asks.clone();
        env.storage().instance().set(&pair_key, &book);

        Ok(fills)
    }

    /// Get orderbook snapshot with aggregated top-of-book levels
    pub fn get_orderbook_snapshot(
        env: &Env,
        base_token: Symbol,
        quote_token: Symbol,
        max_levels: u32, // Maximum number of price levels to return
    ) -> Result<OrderBookSnapshot, ContractError> {
        let pair_key = Self::order_book_key(&(base_token.clone(), quote_token.clone()));
        let order_book: Option<OrderBook> = env.storage().instance().get(&pair_key);
        
        if order_book.is_none() {
            return Err(ContractError::InvalidAmount);
        }

        let book = order_book.unwrap();
        let current_time = env.ledger().timestamp();
        let mut bid_levels: Map<u128, i128> = Map::new(env);
        let mut ask_levels: Map<u128, i128> = Map::new(env);
        let mut bid_counts: Map<u128, u32> = Map::new(env);
        let mut ask_counts: Map<u128, u32> = Map::new(env);

        // Process bid orders (buy orders) - aggregate by price
        for &order_id in book.bids.iter() {
            if let Ok(order) = Self::get_order(env, order_id) {
                if order.status == OrderStatus::Pending || order.status == OrderStatus::PartiallyFilled {
                    let current = bid_levels.get(order.price).unwrap_or(0);
                    bid_levels.set(order.price, current + order.amount_remaining);
                    let count = bid_counts.get(order.price).unwrap_or(0);
                    bid_counts.set(order.price, count + 1);
                }
            }
        }

        // Process ask orders (sell orders) - aggregate by price
        for &order_id in book.asks.iter() {
            if let Ok(order) = Self::get_order(env, order_id) {
                if order.status == OrderStatus::Pending || order.status == OrderStatus::PartiallyFilled {
                    let current = ask_levels.get(order.price).unwrap_or(0);
                    ask_levels.set(order.price, current + order.amount_remaining);
                    let count = ask_counts.get(order.price).unwrap_or(0);
                    ask_counts.set(order.price, count + 1);
                }
            }
        }

        // Convert to sorted vectors
        let mut bids = Vec::new(env);
        let mut ask_prices = Vec::new(env);
        
        // Collect bid prices, sort descending (highest first)
        let mut bid_prices: Vec<u128> = bid_levels.keys().into_iter().collect();
        bid_prices.sort_by(|a, b| b.cmp(a));
        
        for price in bid_prices.into_iter().take(max_levels as usize) {
            bids.push_back(OrderBookLevel {
                price,
                total_amount: bid_levels.get(price).unwrap(),
                order_count: bid_counts.get(price).unwrap(),
            });
        }

        // Collect ask prices, sort ascending (lowest first)
        let mut ask_prices: Vec<u128> = ask_levels.keys().into_iter().collect();
        ask_prices.sort();
        
        for price in ask_prices.into_iter().take(max_levels as usize) {
            ask_levels.remove(price);
            ask_counts.remove(price);
        }
        
        // Collect prices again after filtering to get sorted list
        let mut ask_prices: Vec<u128> = ask_levels.keys().into_iter().collect();
        ask_prices.sort();
        
        let mut asks = Vec::new(env);
        for price in ask_prices.into_iter().take(max_levels as usize) {
            asks.push_back(OrderBookLevel {
                price,
                total_amount: ask_levels.get(price).unwrap(),
                order_count: ask_counts.get(price).unwrap(),
            });
        }

        Ok(OrderBookSnapshot {
            base_token,
            quote_token,
            bids,
            asks,
            timestamp: current_time,
        })
    }

    /// Check and execute pending orders that can be filled
    /// Called during trading operations to match orders
    pub fn match_pending_orders(
        env: &Env,
        token_in: Symbol,
        token_out: Symbol,
        current_price: u128,
    ) -> Result<Vec<u64>, ContractError> {
        let mut executed_orders = Vec::new(env);
        let pair_key = Self::order_book_key(&(token_in.clone(), token_out.clone()));

        // Get order book for this pair
        let order_book: Option<OrderBook> = env.storage().instance().get(&pair_key);

        if order_book.is_none() {
            return Ok(executed_orders);
        }

        let book = order_book.unwrap();
        let current_time = env.ledger().timestamp();

        // Check buy orders: execute if current_price <= limit_price
        for i in 0..book.buy_orders.len() {
            if let Some(order_id) = book.buy_orders.get(i) {
                if let Ok(mut order) = Self::get_order(env, order_id) {
                    // Check expiry
                    if let Some(expires) = order.expires_at {
                        if current_time > expires {
                            order.status = OrderStatus::Expired;
                            Self::save_order(env, &order);
                            continue;
                        }
                    }

                    // Check if order can be executed
                    if order.status == OrderStatus::Pending
                        || order.status == OrderStatus::PartiallyFilled
                    {
                        let should_execute = match order.order_type {
                            OrderType::Limit => {
                                // Execute if current price is at or below limit
                                if let Some(limit) = order.limit_price {
                                    current_price <= limit
                                } else {
                                    false
                                }
                            }
                            OrderType::StopLoss => {
                                // Execute if current price is at or below trigger
                                if let Some(trigger) = order.trigger_price {
                                    current_price <= trigger
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        };

                        if should_execute {
                            // Mark as filled (actual execution happens in trading module)
                            order.status = OrderStatus::Filled;
                            order.filled_at = Some(current_time);
                            Self::save_order(env, &order);
                            executed_orders.push_back(order_id);

                            // Emit execution event
                            env.events().publish(
                                (symbol_short!("ofill"), order_id),
                                (
                                    order.owner,
                                    token_in.clone(),
                                    token_out.clone(),
                                    current_price,
                                ),
                            );
                        }
                    }
                }
            }
        }

        Ok(executed_orders)
    }

    /// Get order details
    pub fn get_order(env: &Env, order_id: u64) -> Result<Order, ContractError> {
        let order_key = Self::order_key(order_id);
        env.storage()
            .instance()
            .get(&order_key)
            .ok_or(ContractError::InvalidAmount) // Order not found
    }

    /// Get user's active orders
    pub fn get_user_orders(env: &Env, user: Address) -> Vec<Order> {
        let mut orders = Vec::new(env);
        let user_order_ids: Option<Vec<u64>> =
            env.storage().instance().get(&Self::user_orders_key(&user));

        if let Some(order_ids) = user_order_ids {
            for i in 0..order_ids.len() {
                if let Some(order_id) = order_ids.get(i) {
                    if let Ok(order) = Self::get_order(env, order_id) {
                        if order.status == OrderStatus::Pending
                            || order.status == OrderStatus::PartiallyFilled
                        {
                            orders.push_back(order);
                        }
                    }
                }
            }
        }

        orders
    }

    /// Create a new order
    fn create_order(
        env: &Env,
        owner: Address,
        order_type: OrderType,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        limit_price: Option<u128>,
        trigger_price: Option<u128>,
        expires_at: Option<u64>,
    ) -> Result<u64, ContractError> {
        // Generate order ID
        let next_id: u64 = env
            .storage()
            .instance()
            .get(&symbol_short!("next_oid"))
            .unwrap_or(1);

        let order = Order {
            order_id: next_id,
            owner: owner.clone(),
            order_type: order_type.clone(),
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in,
            amount_filled: 0,
            limit_price,
            trigger_price,
            status: OrderStatus::Pending,
            created_at: env.ledger().timestamp(),
            expires_at,
            filled_at: None,
            interval_secs: None,
            remaining_occurrences: None,
            next_run: None,
        };

        // Save order
        Self::save_order(env, &order);

        // Add to user's order list
        let mut user_orders: Vec<u64> = env
            .storage()
            .instance()
            .get(&Self::user_orders_key(&owner))
            .unwrap_or_else(|| Vec::new(env));
        user_orders.push_back(next_id);
        env.storage()
            .instance()
            .set(&Self::user_orders_key(&owner), &user_orders);

        // Add to order book
        Self::add_to_order_book(env, token_in.clone(), token_out.clone(), next_id);

        // Increment next order ID
        env.storage()
            .instance()
            .set(&symbol_short!("next_oid"), &(next_id + 1));

        // Emit order placement event
        env.events().publish(
            (symbol_short!("order_new"), next_id),
            (
                owner,
                order_type,
                token_in,
                token_out,
                amount_in,
                limit_price,
                trigger_price,
            ),
        );

        Ok(next_id)
    }

    /// Place a recurring (DCA) order that executes on a fixed schedule
    pub fn place_recurring_order(
        env: &Env,
        owner: Address,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        interval_secs: u64,
        occurrences: u64,
        expires_at: Option<u64>,
    ) -> Result<u64, ContractError> {
        owner.require_auth();

        if amount_in <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        if interval_secs == 0 {
            return Err(ContractError::InvalidAmount);
        }

        if occurrences == 0 {
            return Err(ContractError::InvalidAmount);
        }

        let current_time = env.ledger().timestamp();
        let next_run = current_time + interval_secs;

        // Generate order ID
        let next_id: u64 = env.storage().instance().get(&symbol_short!("next_oid")).unwrap_or(1);

        let order = Order {
            order_id: next_id,
            owner: owner.clone(),
            order_type: OrderType::Recurring,
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in,
            amount_filled: 0,
            limit_price: None,
            trigger_price: None,
            status: OrderStatus::Pending,
            created_at: current_time,
            expires_at,
            filled_at: None,
            interval_secs: Some(interval_secs),
            remaining_occurrences: Some(occurrences),
            next_run: Some(next_run),
        };

        Self::save_order(env, &order);

        // Add to user's order list
        let mut user_orders: Vec<u64> = env.storage()
            .instance()
            .get(&Self::user_orders_key(&owner))
            .unwrap_or_else(|| Vec::new(env));
        user_orders.push_back(next_id);
        env.storage().instance().set(&Self::user_orders_key(&owner), &user_orders);

        // Add to order book
        Self::add_to_order_book(env, token_in.clone(), token_out.clone(), next_id);

        // Increment next order ID
        env.storage().instance().set(&symbol_short!("next_oid"), &(next_id + 1));

        // Register in global recurring orders list
        let mut recurring_ids: Vec<u64> = env.storage()
            .instance()
            .get(&Self::recurring_orders_key())
            .unwrap_or_else(|| Vec::new(env));
        recurring_ids.push_back(next_id);
        env.storage().instance().set(&Self::recurring_orders_key(), &recurring_ids);

        // Emit order placement event
        env.events().publish(
            (symbol_short!("order_new"), next_id),
            (owner, OrderType::Recurring, token_in, token_out, amount_in, None::<u128>, None::<u128>),
        );

        Ok(next_id)
    }

    /// Execute all due recurring orders (Pending or Scheduled with now >= next_run)
    /// Returns list of executed order IDs
    pub fn execute_due_orders(env: &Env) -> Result<Vec<u64>, ContractError> {
        let mut executed_orders = Vec::new(env);
        let current_time = env.ledger().timestamp();

        // Iterate through all orders stored by scanning user orders lists
        // We need to find recurring orders. We'll scan a global list of recurring order IDs.
        let recurring_ids: Vec<u64> = env.storage()
            .instance()
            .get(&Self::recurring_orders_key())
            .unwrap_or_else(|| Vec::new(env));

        for i in 0..recurring_ids.len() {
            if let Some(order_id) = recurring_ids.get(i) {
                if let Ok(mut order) = Self::get_order(env, order_id) {
                    // Only process Recurring orders
                    if order.order_type != OrderType::Recurring {
                        continue;
                    }

                    // Check expiry
                    if let Some(expires) = order.expires_at {
                        if current_time > expires {
                            order.status = OrderStatus::Expired;
                            Self::save_order(env, &order);
                            continue;
                        }
                    }

                    // Only execute if status is Pending or Scheduled and next_run has arrived
                    if order.status != OrderStatus::Pending && order.status != OrderStatus::Scheduled {
                        continue;
                    }

                    if let Some(next_run) = order.next_run {
                        if current_time < next_run {
                            continue;
                        }
                    } else {
                        continue;
                    }

                    // Execute the swap
                    let amount_executed = order.amount_in;
                    order.amount_filled = order.amount_filled + amount_executed;

                    // Decrement remaining occurrences
                    let mut remaining = order.remaining_occurrences.unwrap_or(0);
                    if remaining > 0 {
                        remaining -= 1;
                    }
                    order.remaining_occurrences = Some(remaining);

                    if remaining == 0 {
                        // All executions done
                        order.status = OrderStatus::Filled;
                        order.filled_at = Some(current_time);
                        order.next_run = None;
                    } else {
                        // Schedule next execution
                        order.status = OrderStatus::Scheduled;
                        let interval = order.interval_secs.unwrap_or(0);
                        order.next_run = Some(current_time + interval);
                    }

                    Self::save_order(env, &order);
                    executed_orders.push_back(order_id);

                    // Emit execution event
                    env.events().publish(
                        (symbol_short!("recur"), order_id),
                        (order.owner, amount_executed),
                    );
                }
            }
        }

        Ok(executed_orders)
    }

    /// Save order to storage
    fn save_order(env: &Env, order: &Order) {
        let order_key = Self::order_key(order.order_id);
        env.storage().instance().set(&order_key, order);
    }

    /// Add order to order book with proper time-price priority sorting
    fn add_to_order_book(env: &Env, base_token: Symbol, quote_token: Symbol, order: Order) {
        let pair = (base_token.clone(), quote_token.clone());
        let pair_key = Self::order_book_key(&pair);

        let mut book: OrderBook = env
            .storage()
            .instance()
            .get(&pair_key)
            .unwrap_or(OrderBook {
                base_token,
                quote_token,
                bids: Vec::new(env),
                asks: Vec::new(env),
                token_pair: pair.clone(),
                buy_orders: Vec::new(env),
                sell_orders: Vec::new(env),
            });

        // Add to appropriate list based on side, maintaining sorted order
        match order.side {
            OrderSide::Buy => {
                // Bids (buy orders) are sorted: higher prices first, same price: earlier orders first
                let mut insert_idx = book.bids.len();
                for (i, &existing_id) in book.bids.iter().enumerate() {
                    if let Ok(existing_order) = Self::get_order(env, existing_id) {
                        // If current order's price is higher, insert before this one
                        if order.price > existing_order.price {
                            insert_idx = i;
                            break;
                        } else if order.price == existing_order.price {
                            // Same price, maintain time priority (add after older orders)
                            continue;
                        }
                    }
                }
                book.bids.insert(insert_idx as u32, order.order_id);
            },
            OrderSide::Sell => {
                // Asks (sell orders) are sorted: lower prices first, same price: earlier orders first
                let mut insert_idx = book.asks.len();
                for (i, &existing_id) in book.asks.iter().enumerate() {
                    if let Ok(existing_order) = Self::get_order(env, existing_id) {
                        // If current order's price is lower, insert before this one
                        if order.price < existing_order.price {
                            insert_idx = i;
                            break;
                        } else if order.price == existing_order.price {
                            // Same price, maintain time priority (add after older orders)
                            continue;
                        }
                    }
                }
                book.asks.insert(insert_idx as u32, order.order_id);
            }
        }

        // Update backwards compatibility fields
        book.buy_orders = book.bids.clone();
        book.sell_orders = book.asks.clone();
        
        env.storage().instance().set(&pair_key, &book);
    }

    fn order_key(order_id: u64) -> (Symbol, u64) {
        (symbol_short!("order"), order_id)
    }

    fn user_orders_key(user: &Address) -> (Symbol, Address) {
        (symbol_short!("uorders"), user.clone())
    }

    fn order_book_key(pair: &(Symbol, Symbol)) -> (Symbol, Symbol, Symbol) {
        (symbol_short!("obook"), pair.0.clone(), pair.1.clone())
    }

    fn recurring_orders_key() -> Symbol {
        symbol_short!("recur")
    }
}