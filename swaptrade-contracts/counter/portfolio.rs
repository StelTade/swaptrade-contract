extern crate alloc;
use soroban_sdk::{contracttype, Address, Env, Symbol, Map, Vec, symbol_short};
#[cfg(test)]
use soroban_sdk::testutils::Address as _;
use crate::events::*;
// use soroban_sdk::symbol_short;
use crate::tiers::{UserTier, calculate_user_tier};
use crate::emergency;


#[derive(Clone)]
#[contracttype]
pub enum Asset {
    XLM,
    Custom(Symbol),
}

#[derive(Clone, PartialEq, Debug)]
#[contracttype]
pub enum Badge {
    /// Complete your first swap - achieved at 1+ trades
    FirstTrade,
    
    /// Become an experienced trader - achieved at 10+ trades
    Trader,
    
    /// Build significant wealth - achieved when balance ≥ 10x starting amount
    WealthBuilder,
    
    /// Provide liquidity to the ecosystem - achieved at 1+ LP deposits
    LiquidityProvider,
    
    /// Explore diverse trading pairs - achieved when trading with 5+ different token pairs
    Diversifier,
    
    /// Trade consistently across blocks - achieved when trading on 7+ different ledger heights
    Consistency,
}

#[derive(Clone)]
#[contracttype]
pub struct Portfolio {
    balances: Map<(Address, Asset), i128>,
    trades: Map<Address, u32>,       // number of trades per user
    user_volumes: Map<Address, i128>, // total trading volume per user
    pnl: Map<Address, i128>,         // cumulative balance change placeholder
    badges: Map<(Address, Badge), bool>, // tracks which badges each user has earned
    tiers: Map<Address, UserTier>,   // user tier storage
    metrics: Metrics,                 // lightweight aggregate metrics

    // Admin Dashboard Aggregate Stats
    total_users: u32,                 // unique traders/LPs
    total_trading_volume: i128,       // sum of all swap amounts
    active_users: Vec<Address>,       // users with activity (limited to last N blocks)
    top_traders: Vec<(Address, i128)>, // top 100 traders by PnL
    xlm_in_pool: i128,               // liquidity pool XLM
    usdc_in_pool: i128,              // liquidity pool USDC
    total_fees_collected: i128,       // accumulated fees

    // Badge & Achievement Tracking
    initial_balances: Map<Address, i128>,  // starting balance for WealthBuilder tracking
    token_pairs_traded: Map<Address, Vec<Symbol>>, // unique token pairs per user
    ledger_heights_traded: Map<Address, Vec<u64>>, // ledger heights where user traded
    lp_deposits_count: Map<Address, u32>,  // number of LP deposits per user
    transactions: Map<Address, Vec<Transaction>>, // transaction history

    // LP Position Tracking
    lp_positions: Map<Address, LPPosition>, // LP positions per user
    total_lp_tokens: i128,                 // total LP tokens minted (for share calculations)
    lp_fees_accumulated: i128,            // accumulated fees for LP distribution
}

#[derive(Clone, Debug, PartialEq)] // Added derives for testing
#[contracttype]
pub struct Transaction {
    pub timestamp: u64,
    pub from_token: Symbol,
    pub to_token: Symbol,
    pub from_amount: i128,
    pub to_amount: i128,
    pub rate_achieved: u128, // Represented with 7 decimals precision (units of 10^-7)
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct LPPosition {
    pub lp_address: Address,
    pub xlm_deposited: i128,
    pub usdc_deposited: i128,
    pub lp_tokens_minted: i128,
}

impl Portfolio {
    pub fn new(env: &Env) -> Self {
        Self {
            balances: Map::new(env),
            trades: Map::new(env),
            user_volumes: Map::new(env),
            pnl: Map::new(env),
            badges: Map::new(env),
            tiers: Map::new(env),
            metrics: Metrics::default(),
            total_users: 0,
            total_trading_volume: 0,
            active_users: Vec::new(env),
            top_traders: Vec::new(env),
            xlm_in_pool: 0,
            usdc_in_pool: 0,
            total_fees_collected: 0,
            initial_balances: Map::new(env),
            token_pairs_traded: Map::new(env),
            ledger_heights_traded: Map::new(env),
            lp_deposits_count: Map::new(env),
            transactions: Map::new(env),
            lp_positions: Map::new(env),
            total_lp_tokens: 0,
            lp_fees_accumulated: 0,
        }
    }

    /// Transfer a user's balance from one asset to another.
    /// Fails if amount <= 0 or if the user has insufficient funds in the source asset.
    pub fn transfer_asset(
        &mut self,
        env: &Env,
        from_token: Asset,
        to_token: Asset,
        user: Address,
        amount: i128,
    ) {
        assert!(amount > 0, "Amount must be positive");

        // Debit from source asset
    let from_key = (user.clone(), from_token.clone());
    let from_balance = self.balances.get(from_key.clone()).unwrap_or(0);
        assert!(from_balance >= amount, "Insufficient funds");
    let new_from = from_balance - amount;
    self.balances.set(from_key, new_from);

        // Credit to destination asset
    let to_key = (user.clone(), to_token.clone());
    let to_balance = self.balances.get(to_key.clone()).unwrap_or(0);
    let new_to = to_balance + amount;
    self.balances.set(to_key, new_to);

        // Metrics: two balance updates (debit and credit)
        self.metrics.balances_updated = self.metrics.balances_updated.saturating_add(2);

        // Optional structured logging
        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events().publish(
                (symbol_short!("transfer_asset"), user.clone()),
                (from_token, to_key.1, amount),
            );
        }
    }


    /// Debit tokens from a user's balance (for LP deposits, etc.)
    pub fn debit(&mut self, env: &Env, token: Asset, from: Address, amount: i128) {
        assert!(amount > 0, "Amount must be positive");
        let key = (from.clone(), token.clone());
        let current = self.balances.get(key.clone()).unwrap_or(0);
        assert!(current >= amount, "Insufficient funds");
        let new_balance = current - amount;
        self.balances.set(key, new_balance);
        
        // Update PnL
        let current_pnl = self.pnl.get(from.clone()).unwrap_or(0);
        let new_pnl = current_pnl.saturating_sub(amount);
        self.pnl.set(from.clone(), new_pnl);
        
        // Metrics
        self.metrics.balances_updated = self.metrics.balances_updated.saturating_add(1);
    }

    /// Mint tokens (XLM or a custom token) to a user's balance.
    pub fn mint(&mut self, env: &Env, token: Asset, to: Address, amount: i128) {
        assert!(amount >= 0, "Amount must be non-negative");

    let key = (to.clone(), token.clone());
    let current = self.balances.get(key.clone()).unwrap_or(0);
    let new_balance = current + amount;

    self.balances.set(key, new_balance);

        // Update PnL placeholder
    let current_pnl = self.pnl.get(to.clone()).unwrap_or(0);
    let new_pnl = current_pnl + amount;
    self.pnl.set(to.clone(), new_pnl);

        // Update top traders leaderboard
        self.update_top_traders(env, to.clone());

        // Metrics: one balance updated
        self.metrics.balances_updated = self.metrics.balances_updated.saturating_add(1);

        // Optional structured logging
        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events().publish(
                (symbol_short!("mint"), to.clone()),
                (token, amount),
            );
        }
    }

    /// Record a swap execution (increase trade count).
    /// Automatically awards "First Trade" badge if this is the user's first trade.
    pub fn record_trade(&mut self, env: &Env, user: Address) {
    let count = self.trades.get(user.clone()).unwrap_or(0);
    self.trades.set(user.clone(), count + 1);

        // Metrics: successful trade executed
        self.metrics.trades_executed = self.metrics.trades_executed.saturating_add(1);

        // Award "First Trade" badge if this is the first trade
        if count == 0 {
            self.award_badge(env, user, Badge::FirstTrade);
        }
    }

    /// Record a swap execution with full transaction details
    pub fn record_transaction(
        &mut self, 
        env: &Env, 
        user: Address, 
        from_token: Symbol, 
        to_token: Symbol, 
        from_amount: i128, 
        to_amount: i128
    ) {
        // calculate rate (avoid division by zero)
        let rate = if from_amount > 0 {
            (to_amount as u128 * 10_000_000) / (from_amount as u128)
        } else {
            0
        };

        let tx = Transaction {
            timestamp: env.ledger().timestamp(),
            from_token: from_token.clone(),
            to_token: to_token.clone(),
            from_amount,
            to_amount,
            rate_achieved: rate,
        };

        // Store transaction
        let mut user_txs = self.transactions.get(user.clone()).unwrap_or(Vec::new(env));
        user_txs.push_back(tx);

        // Cap at 100 transactions (remove oldest)
        if user_txs.len() > 100 {
            user_txs.remove(0); // Remove oldest
        }
        self.transactions.set(user.clone(), user_txs);

        // Existing stats updates
        self.record_trade(env, user.clone());
        self.update_stats_on_trade(env, user.clone(), from_amount);
        
        // Update user tier after trade
        self.update_tier(env, user.clone());
        
        // Track for badges
        self.track_trade_for_badges(env, user, from_token, to_token, env.ledger().sequence() as u64);
    }

    /// Helper for tests/backward compat: record trade with amount only (assumes 1:1 XLM swap for simplicity)
    pub fn record_trade_with_amount(&mut self, env: &Env, user: Address, swap_amount: i128) {
        self.record_transaction(
            env, 
            user, 
            symbol_short!("XLM"), 
            symbol_short!("USDC"), 
            swap_amount, 
            swap_amount
        );
    }

    /// Award a badge to a user if they don't already have it.
    /// Returns true if badge was awarded, false if user already had it.
    pub fn award_badge(&mut self, env: &Env, user: Address, badge: Badge) -> bool {
        let key = (user, badge);

        // Check if user already has this badge
        if self.has_badge(env, key.0.clone(), key.1.clone()) {
            return false; // Badge already awarded, prevent duplicate
        }

        // Award the badge
        self.badges.set(key, true);
        true
    }

    /// Check if a user has earned a specific badge.
    pub fn has_badge(&self, _env: &Env, user: Address, badge: Badge) -> bool {
        let key = (user, badge);
        self.badges.get(key).unwrap_or(false)
    }



    /// Get balance of a token for a given user.
    /// Returns 0 if no balance exists for the requested token/address.
    pub fn balance_of(&self, _env: &Env, token: Asset, user: Address) -> i128 {
        let key = (user, token);
        self.balances.get(key).unwrap_or(0)
    }

    /// Get portfolio statistics for a user
    /// Returns (trade_count, pnl)
    pub fn get_portfolio(&self, _env: &Env, user: Address) -> (u32, i128) {
        let trades = self.trades.get(user.clone()).unwrap_or(0);
        let pnl = self.pnl.get(user).unwrap_or(0);
        (trades, pnl)
    }
    
    /// Get transaction history for a user
    pub fn get_user_transactions(&self, env: &Env, user: Address, limit: u32) -> Vec<Transaction> {
        let transactions = self.transactions.get(user).unwrap_or(Vec::new(env));
        if transactions.len() <= limit {
            transactions
        } else {
            // Return last 'limit' transactions
            let start = transactions.len() - limit;
            let mut result = Vec::new(env);
            for i in start..transactions.len() {
               if let Some(tx) = transactions.get(i) {
                   result.push_back(tx);
               }
            }
            result
        }
    }

    /// Read aggregate metrics
    pub fn get_metrics(&self) -> Metrics {
        self.metrics.clone()
    }

    /// Increment failed order counter
    pub fn inc_failed_order(&mut self) {
        self.metrics.failed_orders = self.metrics.failed_orders.saturating_add(1);
    }

    // ===== BADGE & ACHIEVEMENT SYSTEM =====

    /// Update badge tracking when a trade occurs
    /// Tracks token pairs and ledger heights for badge conditions
    pub fn track_trade_for_badges(&mut self, env: &Env, user: Address, from_token: Symbol, to_token: Symbol, ledger_height: u64) {
        // Track token pair diversity
        let mut pairs = self.token_pairs_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        
        // Check if this token pair combo is new
        let pair_key = Self::format_pair_helper(from_token.clone(), to_token.clone());
        let mut is_new_pair = true;
        for i in 0..pairs.len() {
            if let Some(existing) = pairs.get(i) {
                if existing == pair_key {
                    is_new_pair = false;
                    break;
                }
            }
        }
        
        if is_new_pair {
            pairs.push_back(pair_key);
            self.token_pairs_traded.set(user.clone(), pairs);
        }
        
        // Track ledger heights for consistency badge
        let mut heights = self.ledger_heights_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        
        // Check if this ledger height is new
        let mut is_new_height = true;
        for i in 0..heights.len() {
            if let Some(existing) = heights.get(i) {
                if existing == ledger_height {
                    is_new_height = false;
                    break;
                }
            }
        }
        
        if is_new_height {
            heights.push_back(ledger_height);
            self.ledger_heights_traded.set(user, heights);
        }
    }

    /// Check and automatically award all applicable badges to a user
    /// Call this after each trade or LP action
    pub fn check_and_award_badges(&mut self, env: &Env, user: Address) {
        // FirstTrade: Complete 1 swap (already handled in record_trade)
        // We keep it for consistency
        
        // Trader: Complete 10 swaps
        let trades = self.trades.get(user.clone()).unwrap_or(0);
        if trades >= 10 {
            self.award_badge(env, user.clone(), Badge::Trader);
        }
        
        // WealthBuilder: Achieve 10x starting balance
        let current_balance = self.get_total_user_balance(env, user.clone());
        let initial_balance = self.initial_balances.get(user.clone()).unwrap_or(0);
        
        if initial_balance > 0 && current_balance >= initial_balance * 10 {
            self.award_badge(env, user.clone(), Badge::WealthBuilder);
        }
        
        // LiquidityProvider: Deposit liquidity once
        let lp_deposits = self.lp_deposits_count.get(user.clone()).unwrap_or(0);
        if lp_deposits >= 1 {
            self.award_badge(env, user.clone(), Badge::LiquidityProvider);
        }
        
        // Diversifier: Trade with 5+ different token pairs
        let pairs = self.token_pairs_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        if pairs.len() >= 5 {
            self.award_badge(env, user.clone(), Badge::Diversifier);
        }
        
        // Consistency: Make trades on 7+ different ledger heights
        let heights = self.ledger_heights_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        if heights.len() >= 7 {
            self.award_badge(env, user.clone(), Badge::Consistency);
        }
    }

    /// Record an LP deposit for the user
    pub fn record_lp_deposit(&mut self, user: Address) {
        let count = self.lp_deposits_count.get(user.clone()).unwrap_or(0);
        self.lp_deposits_count.set(user, count.saturating_add(1));
    }

    /// Record initial balance for WealthBuilder tracking
    pub fn record_initial_balance(&mut self, user: Address, amount: i128) {
        // Only set if not already recorded
        if self.initial_balances.get(user.clone()).is_none() && amount > 0 {
            self.initial_balances.set(user, amount);
        }
    }

    /// Get total balance across all assets for a user
    fn get_total_user_balance(&self, _env: &Env, user: Address) -> i128 {
        // Sum balances across all assets (simplified - just returns PnL as proxy)
        self.pnl.get(user).unwrap_or(0)
    }

    /// Get badge progress for a user showing progress toward each badge
    /// Returns progress as a string representation (e.g., "3/10 trades toward Trader")
    pub fn get_badge_progress(&self, env: &Env, user: Address) -> Vec<(Badge, u32, u32)> {
        let mut progress = Vec::new(env);
        
        // FirstTrade: 1+ trades
        let trades = self.trades.get(user.clone()).unwrap_or(0);
        progress.push_back((Badge::FirstTrade, trades, 1));
        
        // Trader: 10+ trades
        progress.push_back((Badge::Trader, trades, 10));
        
        // WealthBuilder: 10x starting balance
        let current_balance = self.get_total_user_balance(env, user.clone());
        let initial_balance = self.initial_balances.get(user.clone()).unwrap_or(1); // Avoid division by 0
        let wealth_multiplier = if initial_balance > 0 {
            (current_balance / initial_balance) as u32
        } else {
            0
        };
        progress.push_back((Badge::WealthBuilder, wealth_multiplier, 10));
        
        // LiquidityProvider: 1+ LP deposits
        let lp_deposits = self.lp_deposits_count.get(user.clone()).unwrap_or(0);
        progress.push_back((Badge::LiquidityProvider, lp_deposits, 1));
        
        // Diversifier: 5+ different token pairs
        let pairs = self.token_pairs_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        progress.push_back((Badge::Diversifier, pairs.len() as u32, 5));
        
        // Consistency: 7+ different ledger heights
        let heights = self.ledger_heights_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        progress.push_back((Badge::Consistency, heights.len() as u32, 7));
        
        progress
    }

    /// Update get_user_badges to include all earned badges
    pub fn get_user_badges(&self, env: &Env, user: Address) -> Vec<Badge> {
    let mut badges = Vec::new(env);

        // Check all badge types
        let badge_types = [
            Badge::FirstTrade,
            Badge::Trader,
            Badge::WealthBuilder,
            Badge::LiquidityProvider,
            Badge::Diversifier,
            Badge::Consistency,
        ];
        
        for badge in badge_types.iter() {
            if self.has_badge(env, user.clone(), badge.clone()) {
                badges.push_back(badge.clone());
            }
        }

        badges
    }

    // ===== USER TIER SYSTEM =====

    /// Get the current tier for a user
    /// Calculates tier on-the-fly based on current trade count and volume
    pub fn get_user_tier(&self, _env: &Env, user: Address) -> UserTier {
        let trade_count = self.trades.get(user.clone()).unwrap_or(0);
        let volume = self.user_volumes.get(user).unwrap_or(0);
        calculate_user_tier(trade_count, volume)
    }

    /// Update and store a user's tier after a trade
    /// Returns the new tier and whether it changed
    pub fn update_tier(&mut self, env: &Env, user: Address) -> (UserTier, bool) {
        let new_tier = self.get_user_tier(env, user.clone());
        let old_tier = self.tiers.get(user.clone()).unwrap_or(UserTier::Novice);

        let changed = new_tier != old_tier;
        if changed {
            self.tiers.set(user.clone(), new_tier.clone());

            // Emit tier change event
            #[cfg(feature = "logging")]
            {
                use soroban_sdk::symbol_short;
                env.events().publish(
                    (symbol_short!("tier_changed"), user),
                    (old_tier, new_tier.clone()),
                );
            }
        }

        (new_tier, changed)
    }

    // ===== HELPER FUNCTION FOR TOKEN PAIR FORMATTING =====
    
    /// Format a token pair for tracking (handles ordering)
    fn format_pair_helper(from: Symbol, _to: Symbol) -> Symbol {
        // Simple pair identifier (in production, you might use a hash)
        from
    }

    // ===== ADMIN DASHBOARD QUERY FUNCTIONS =====

    /// Get the total number of unique traders and LPs
    /// Returns u32: unique traders + LPs count
    /// Time complexity: O(1)
    pub fn get_total_users(&self) -> u32 {
        self.total_users
    }

    /// Get the total trading volume (sum of all swap amounts)
    /// Returns i128: sum of all swap amounts
    /// Time complexity: O(1)
    pub fn get_total_trading_volume(&self) -> i128 {
        self.total_trading_volume
    }

    /// Get the count of active users (users with recorded trades)
    /// Returns u32: count of users in active_users list
    /// Time complexity: O(1)
    pub fn get_active_users_count(&self) -> u32 {
        self.active_users.len()
    }

    /// Get the top N traders by PnL (leaderboard)
    /// Capped at top 100 for safety
    /// Returns Vec<(Address, i128)>: list of (user, pnl) pairs sorted by PnL descending
    /// Time complexity: O(1) - precomputed top 100
    pub fn get_top_traders(&self, env: &Env, limit: u32) -> Vec<(Address, i128)> {
        let max_limit = 100u32;
        let actual_limit = if limit > max_limit { max_limit } else { limit };
        
        let mut result = Vec::new(env);
        let len = self.top_traders.len();
        let cap = if len < actual_limit { len } else { actual_limit };
        
        for i in 0..cap {
            if let Some(trader) = self.top_traders.get(i) {
                result.push_back(trader);
            }
        }
        result
    }

    /// Get pool statistics (liquidity and fees)
    /// Returns (i128, i128, i128): (xlm_in_pool, usdc_in_pool, total_fees_collected)
    /// Time complexity: O(1)
    pub fn get_pool_stats(&self) -> (i128, i128, i128) {
        (self.xlm_in_pool, self.usdc_in_pool, self.total_fees_collected)
    }

    /// Helper: Update aggregate stats when a trade is recorded
    /// Called lazily during trade operations
    fn update_stats_on_trade(&mut self, _env: &Env, user: Address, swap_amount: i128) {
        // Check if user is new (not in trades map)
        let trade_count = self.trades.get(user.clone()).unwrap_or(0);
        if trade_count == 0 {
            self.total_users = self.total_users.saturating_add(1);

            // Add to active_users if not already there
            let mut is_active = false;
            for i in 0..self.active_users.len() {
                if let Some(addr) = self.active_users.get(i) {
                    if addr == user {
                        is_active = true;
                        break;
                    }
                }
            }
            if !is_active {
                self.active_users.push_back(user.clone());
            }
        }

        // Update total trading volume
        self.total_trading_volume = self.total_trading_volume.saturating_add(swap_amount);

        // Update per-user volume
        let current_user_volume = self.user_volumes.get(user.clone()).unwrap_or(0);
        self.user_volumes.set(user, current_user_volume.saturating_add(swap_amount));
    }

    /// Helper: Update top traders leaderboard after PnL changes
    /// Maintains top 100 traders sorted by PnL descending
    fn update_top_traders(&mut self, _env: &Env, user: Address) {
        let user_pnl = self.pnl.get(user.clone()).unwrap_or(0);
        
        // Check if user is already in top_traders
        let mut found_index = None;
        for i in 0..self.top_traders.len() {
            if let Some((addr, _)) = self.top_traders.get(i) {
                if addr == user {
                    found_index = Some(i);
                    break;
                }
            }
        }
        
        if let Some(idx) = found_index {
            // Update existing entry
            self.top_traders.set(idx, (user.clone(), user_pnl));
        } else if self.top_traders.len() < 100 {
            // Add new entry if under limit
            self.top_traders.push_back((user.clone(), user_pnl));
        } else {
            // Check if new PnL beats the lowest in top 100
            if let Some((_, lowest_pnl)) = self.top_traders.get(99) {
                if user_pnl > lowest_pnl {
                    self.top_traders.set(99, (user.clone(), user_pnl));
                }
            }
        }
        
        // Sort by PnL descending (simple bubble sort for small list)
        self.sort_top_traders();
    }

    /// Helper: Sort top_traders by PnL in descending order
    fn sort_top_traders(&mut self) {
        let len = self.top_traders.len();
        for i in 0..len {
            for j in 0..(len - 1 - i) {
                if let (Some((_, pnl1)), Some((_, pnl2))) = (self.top_traders.get(j), self.top_traders.get(j + 1)) {
                    if pnl1 < pnl2 {
                        // Swap
                        let temp1 = self.top_traders.get(j).unwrap();
                        let temp2 = self.top_traders.get(j + 1).unwrap();
                        self.top_traders.set(j, temp2);
                        self.top_traders.set(j + 1, temp1);
                    }
                }
            }
        }
    }

    /// Helper: Add liquidity to pool
    pub fn add_pool_liquidity(&mut self, xlm_amount: i128, usdc_amount: i128) {
        self.xlm_in_pool = self.xlm_in_pool.saturating_add(xlm_amount);
        self.usdc_in_pool = self.usdc_in_pool.saturating_add(usdc_amount);
    }

    /// Helper: Collect fees
    pub fn collect_fee(&mut self, fee_amount: i128) {
        self.total_fees_collected = self.total_fees_collected.saturating_add(fee_amount);
    }

    pub fn set_liquidity(&mut self, asset: Asset, amount: i128) {
        match asset {
            Asset::XLM => self.xlm_in_pool = amount,
            Asset::Custom(sym) => {
                if sym == symbol_short!("USDCSIM") {
                    self.usdc_in_pool = amount;
                }
            }
        }
    }

    pub fn get_liquidity(&self, asset: Asset) -> i128 {
        match asset {
            Asset::XLM => self.xlm_in_pool,
            Asset::Custom(sym) => {
                if sym == symbol_short!("USDCSIM") {
                    self.usdc_in_pool
                } else {
                    0
                }
            }
        }
    }

    // ===== LP POSITION MANAGEMENT =====

    /// Get LP position for a user
    pub fn get_lp_position(&self, user: Address) -> Option<LPPosition> {
        self.lp_positions.get(user)
    }

    /// Set or update LP position for a user
    pub fn set_lp_position(&mut self, user: Address, position: LPPosition) {
        self.lp_positions.set(user, position);
    }

    /// Get total LP tokens minted
    pub fn get_total_lp_tokens(&self) -> i128 {
        self.total_lp_tokens
    }

    /// Add to total LP tokens (when minting)
    pub fn add_total_lp_tokens(&mut self, amount: i128) {
        self.total_lp_tokens = self.total_lp_tokens.saturating_add(amount);
    }

    /// Subtract from total LP tokens (when burning)
    pub fn subtract_total_lp_tokens(&mut self, amount: i128) {
        self.total_lp_tokens = self.total_lp_tokens.saturating_sub(amount);
        if self.total_lp_tokens < 0 {
            self.total_lp_tokens = 0;
        }
    }

    /// Add accumulated fees for LP distribution
    pub fn add_lp_fees(&mut self, amount: i128) {
        self.lp_fees_accumulated = self.lp_fees_accumulated.saturating_add(amount);
    }

    /// Get accumulated LP fees
    pub fn get_lp_fees_accumulated(&self) -> i128 {
        self.lp_fees_accumulated
    }

    /// Get all LP positions (for get_lp_positions function)
    pub fn get_all_lp_positions(&self, env: &Env) -> Vec<LPPosition> {
        // Note: Map iteration is limited in Soroban, so we'll need to track LP users separately
        // For now, return empty vec - we'll handle this differently in the contract
        Vec::new(env)
    }
}

#[derive(Clone, Default)]
#[contracttype]
pub struct Metrics {
    pub trades_executed: u32,
    pub failed_orders: u32,
    pub balances_updated: u32,
}


#[test]
#[should_panic(expected = "Amount must be non-negative")] 
fn test_mint_negative_should_panic() {
    let env = Env::default(); 
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new(&env); 

    // This should panic 
    portfolio.mint(&env, Asset::XLM, user.clone(), -100);
}

#[test]
fn test_balance_of_returns_zero_for_new_user() {
    let env = Env::default();
    let user = Address::generate(&env);
    let portfolio = Portfolio::new(&env);
    
    // Should return 0 for a user with no balance
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), 0);
}

#[test]
fn test_balance_of_returns_correct_balance_after_mint() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new(&env);
    let amount = 1000;
    
    // Mint some tokens
    portfolio.mint(&env, Asset::XLM, user.clone(), amount);
    
    // Should return the minted amount
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), amount);
}

#[test]
fn test_balance_of_returns_updated_balance_after_multiple_mints() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new(&env);
    
    // First mint
    portfolio.mint(&env, Asset::XLM, user.clone(), 500);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user.clone()), 500);
    
    // Second mint
    portfolio.mint(&env, Asset::XLM, user.clone(), 300);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user.clone()), 800);
    
    // Third mint
    portfolio.mint(&env, Asset::XLM, user.clone(), 200);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), 1000);
}

#[test]
fn test_balance_of_works_with_custom_assets() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new(&env);
    let custom_asset = Asset::Custom(soroban_sdk::symbol_short!("USDC"));
    
    // Mint to custom asset
    portfolio.mint(&env, custom_asset.clone(), user.clone(), 2000);
    
    // Should return correct balance for custom asset
    assert_eq!(portfolio.balance_of(&env, custom_asset, user), 2000);
}

#[test]
fn test_balance_of_isolates_different_users() {
    let env = Env::default();
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let mut portfolio = Portfolio::new(&env);
    
    // Mint to user1
    portfolio.mint(&env, Asset::XLM, user1.clone(), 1000);
    
    // user1 should have balance, user2 should have 0
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user1), 1000);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user2), 0);
}

// ===== REWARDS TESTS =====

/// Test that the "First Trade" badge is awarded when a user completes their first trade
#[test]
fn test_award_first_trade_badge() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // User should not have any badges initially
    let badges_before = portfolio.get_user_badges(&env, user.clone());
    assert_eq!(badges_before.len(), 0);

    // User should not have FirstTrade badge
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), false);

    // Record the user's first trade
    portfolio.record_trade(&env, user.clone());

    // User should now have the FirstTrade badge
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);

    // Verify badge appears in user's badge list
    let badges_after = portfolio.get_user_badges(&env, user);
    assert_eq!(badges_after.len(), 1);
}

/// Test that the "First Trade" badge is only awarded once (no duplicates)
#[test]
fn test_prevent_duplicate_badge_assignment() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Record first trade - should award badge
    portfolio.record_trade(&env, user.clone());
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    let badges_after_first = portfolio.get_user_badges(&env, user.clone());
    assert_eq!(badges_after_first.len(), 1);

    // Record second trade - should NOT duplicate the badge
    portfolio.record_trade(&env, user.clone());
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    let badges_after_second = portfolio.get_user_badges(&env, user.clone());
    assert_eq!(badges_after_second.len(), 1); // Still only 1 badge

    // Record third trade - should still NOT duplicate the badge
    portfolio.record_trade(&env, user.clone());
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    let badges_after_third = portfolio.get_user_badges(&env, user);
    assert_eq!(badges_after_third.len(), 1); // Still only 1 badge
}

/// Test that different users receive badges independently
#[test]
fn test_badges_are_user_specific() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // User1 completes a trade
    portfolio.record_trade(&env, user1.clone());
    assert_eq!(portfolio.has_badge(&env, user1.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.has_badge(&env, user2.clone(), Badge::FirstTrade), false);

    // User2 completes a trade
    portfolio.record_trade(&env, user2.clone());
    assert_eq!(portfolio.has_badge(&env, user1.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.has_badge(&env, user2.clone(), Badge::FirstTrade), true);

    // Both users should have exactly 1 badge each
    assert_eq!(portfolio.get_user_badges(&env, user1).len(), 1);
    assert_eq!(portfolio.get_user_badges(&env, user2).len(), 1);
}

/// Test that badge state persists correctly
#[test]
fn test_badge_persistence() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Award badge via trade
    portfolio.record_trade(&env, user.clone());

    // Check multiple times - should always return true
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);

    // Badge count should remain consistent
    assert_eq!(portfolio.get_user_badges(&env, user).len(), 1);
}

/// Test that new users start with no badges
#[test]
fn test_new_user_has_no_badges() {
    let env = Env::default();
    let portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // New user should have no badges
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), false);
    assert_eq!(portfolio.get_user_badges(&env, user).len(), 0);
}

/// Test reward logic integration with trade counting
#[test]
fn test_rewards_integrate_with_trade_counting() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Get initial portfolio stats
    let (trades_before, _) = portfolio.get_portfolio(&env, user.clone());
    assert_eq!(trades_before, 0);

    // Record first trade
    portfolio.record_trade(&env, user.clone());
    let (trades_after_first, _) = portfolio.get_portfolio(&env, user.clone());
    assert_eq!(trades_after_first, 1);
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);

    // Record additional trades
    portfolio.record_trade(&env, user.clone());
    portfolio.record_trade(&env, user.clone());
    let (trades_after_multiple, _) = portfolio.get_portfolio(&env, user.clone());
    assert_eq!(trades_after_multiple, 3);

    // Badge should still be there, but not duplicated
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.get_user_badges(&env, user).len(), 1);
}

// ===== USER TIER SYSTEM TESTS =====

/// Test that new users start as Novice tier
#[test]
fn test_new_user_starts_as_novice() {
    let env = Env::default();
    let portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    let tier = portfolio.get_user_tier(&env, user);
    assert_eq!(tier, UserTier::Novice);
}

/// Test tier progression based on trade count
#[test]
fn test_tier_progression_by_trades() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Start as Novice
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Novice);

    // Record 9 trades - still Novice
    for _ in 0..9 {
        portfolio.record_trade(&env, user.clone());
    }
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Novice);

    // Record 10th trade - becomes Trader
    portfolio.record_trade(&env, user.clone());
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Trader);

    // Record 40 more trades - still Trader (need 50 total for Expert)
    for _ in 0..40 {
        portfolio.record_trade(&env, user.clone());
    }
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Trader);

    // Record 10 more trades - becomes Expert (50 total)
    for _ in 0..10 {
        portfolio.record_trade(&env, user.clone());
    }
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Expert);

    // Record 150 more trades - becomes Whale (200 total)
    for _ in 0..150 {
        portfolio.record_trade(&env, user.clone());
    }
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Whale);
}

/// Test tier progression based on volume
#[test]
fn test_tier_progression_by_volume() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Start as Novice
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Novice);

    // Add 99 volume - still Novice
    portfolio.record_trade_with_amount(&env, user.clone(), 99);
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Novice);

    // Add 1 more volume - becomes Trader (100 total)
    portfolio.record_trade_with_amount(&env, user.clone(), 1);
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Trader);

    // Add 899 more volume - still Trader (need 1000 total for Expert)
    portfolio.record_trade_with_amount(&env, user.clone(), 899);
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Trader);

    // Add 101 more volume - becomes Expert (1000 total)
    portfolio.record_trade_with_amount(&env, user.clone(), 101);
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Expert);

    // Add 9000 more volume - becomes Whale (10000 total)
    portfolio.record_trade_with_amount(&env, user.clone(), 9000);
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Whale);
}

/// Test tier progression with both trades and volume
#[test]
fn test_tier_progression_combined() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Start as Novice
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Novice);

    // Add 5 trades and 50 volume - still Novice
    for _ in 0..5 {
        portfolio.record_trade_with_amount(&env, user.clone(), 10);
    }
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Novice);

    // Add 5 more trades (10 total) - becomes Trader
    portfolio.record_trade(&env, user.clone());
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Trader);

    // Add 40 more trades but only 400 volume - still Trader (can't be Expert without 1000 volume)
    for _ in 0..40 {
        portfolio.record_trade_with_amount(&env, user.clone(), 10);
    }
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Trader);

    // Add volume to reach 1000 total
    portfolio.record_trade_with_amount(&env, user.clone(), 100);
    assert_eq!(portfolio.get_user_tier(&env, user.clone()), UserTier::Expert);
}

/// Test tier update functionality
#[test]
fn test_tier_update_functionality() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Start as Novice
    let (initial_tier, changed) = portfolio.update_tier(&env, user.clone());
    assert_eq!(initial_tier, UserTier::Novice);
    assert_eq!(changed, false); // No change since it's the same

    // Record trades to become Trader
    for _ in 0..10 {
        portfolio.record_trade(&env, user.clone());
    }

    let (new_tier, changed) = portfolio.update_tier(&env, user.clone());
    assert_eq!(new_tier, UserTier::Trader);
    assert_eq!(changed, true); // Tier changed

    // Update again - should not change
    let (same_tier, changed) = portfolio.update_tier(&env, user.clone());
    assert_eq!(same_tier, UserTier::Trader);
    assert_eq!(changed, false); // No change
}

/// Test that different users have independent tiers
#[test]
fn test_user_tier_isolation() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Both start as Novice
    assert_eq!(portfolio.get_user_tier(&env, user1.clone()), UserTier::Novice);
    assert_eq!(portfolio.get_user_tier(&env, user2.clone()), UserTier::Novice);

    // User1 becomes Trader
    for _ in 0..10 {
        portfolio.record_trade(&env, user1.clone());
    }
    assert_eq!(portfolio.get_user_tier(&env, user1.clone()), UserTier::Trader);
    assert_eq!(portfolio.get_user_tier(&env, user2.clone()), UserTier::Novice); // User2 unchanged

    // User2 becomes Expert
    for _ in 0..50 {
        portfolio.record_trade_with_amount(&env, user2.clone(), 20);
    }
    assert_eq!(portfolio.get_user_tier(&env, user1.clone()), UserTier::Trader);
    assert_eq!(portfolio.get_user_tier(&env, user2.clone()), UserTier::Expert);
}
