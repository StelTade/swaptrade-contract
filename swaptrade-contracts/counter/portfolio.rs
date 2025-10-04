use soroban_sdk::{contracttype, Address, Env, Symbol, Map, Vec};

#[derive(Clone)]
#[contracttype]
pub enum Asset {
    XLM,
    Custom(Symbol),
}

#[derive(Clone, PartialEq, Debug)]
#[contracttype]
pub enum Badge {
    FirstTrade,
}

#[derive(Clone)]
#[contracttype]
pub struct Portfolio {
    balances: Map<(Address, Asset), i128>,
    trades: Map<Address, u32>,       // number of trades per user
    pnl: Map<Address, i128>,         // cumulative balance change placeholder
    badges: Map<(Address, Badge), bool>, // tracks which badges each user has earned
    metrics: Metrics,                 // lightweight aggregate metrics
}

impl Portfolio {
    pub fn new() -> Self {
        Self {
            balances: Map::new(),
            trades: Map::new(),
            pnl: Map::new(),
            badges: Map::new(),
            metrics: Metrics::default(),
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
        let from_key = (user.clone(), from_token);
        let from_balance = self.balances.get(env, &from_key).unwrap_or(0);
        assert!(from_balance >= amount, "Insufficient funds");
        let new_from = from_balance - amount;
        self.balances.set(env, &from_key, &new_from);

        // Credit to destination asset
        let to_key = (user.clone(), to_token);
        let to_balance = self.balances.get(env, &to_key).unwrap_or(0);
        let new_to = to_balance + amount;
        self.balances.set(env, &to_key, &new_to);

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


    /// Mint tokens (XLM or a custom token) to a user’s balance.
    pub fn mint(&mut self, env: &Env, token: Asset, to: Address, amount: i128) {
        assert!(amount >= 0, "Amount must be non-negative");

        let key = (to.clone(), token.clone());
        let current = self.balances.get(env, &key).unwrap_or(0);
        let new_balance = current + amount;

        self.balances.set(env, &key, &new_balance);

        // Update PnL placeholder
        let current_pnl = self.pnl.get(env, &to).unwrap_or(0);
        self.pnl.set(env, &to, &(current_pnl + amount));

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
        let count = self.trades.get(env, &user).unwrap_or(0);
        self.trades.set(env, &user, &(count + 1));

        // Metrics: successful trade executed
        self.metrics.trades_executed = self.metrics.trades_executed.saturating_add(1);

        // Award "First Trade" badge if this is the first trade
        if count == 0 {
            self.award_badge(env, user, Badge::FirstTrade);
        }
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
        self.badges.set(env, &key, &true);
        true
    }

    /// Check if a user has earned a specific badge.
    pub fn has_badge(&self, env: &Env, user: Address, badge: Badge) -> bool {
        let key = (user, badge);
        self.badges.get(env, &key).unwrap_or(false)
    }

    /// Get all badges earned by a user.
    pub fn get_user_badges(&self, env: &Env, user: Address) -> Vec<Badge> {
        let mut badges = Vec::new(env);

        // Check for FirstTrade badge
        if self.has_badge(env, user.clone(), Badge::FirstTrade) {
            badges.push_back(Badge::FirstTrade);
        }

        badges
    }

    /// Get balance of a token for a given user.
    /// Returns 0 if no balance exists for the requested token/address.
    pub fn balance_of(&self, env: &Env, token: Asset, user: Address) -> i128 {
        let key = (user, token);
        self.balances.get(env, &key).unwrap_or(0)
    }

    /// Get portfolio statistics for a user
    /// Returns (trade_count, pnl)
    pub fn get_portfolio(&self, env: &Env, user: Address) -> (u32, i128) {
        let trades = self.trades.get(env, &user).unwrap_or(0);
        let pnl = self.pnl.get(env, &user).unwrap_or(0);
        (trades, pnl)
    }

    /// Read aggregate metrics
    pub fn get_metrics(&self) -> Metrics {
        self.metrics.clone()
    }

    /// Increment failed order counter
    pub fn inc_failed_order(&mut self) {
        self.metrics.failed_orders = self.metrics.failed_orders.saturating_add(1);
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
#[should_panic(expected = "Amount must be positive")] 
fn test_mint_negative_should_panic() {
    let env = Env::default(); 
    let user = Address::generate(&env); 
    let mut portfolio = Portfolio::new(); 

    // This should panic 
    portfolio.mint(&env, Asset::XLM, user.clone(), -100);
}

#[test]
fn test_balance_of_returns_zero_for_new_user() {
    let env = Env::default();
    let user = Address::generate(&env);
    let portfolio = Portfolio::new();
    
    // Should return 0 for a user with no balance
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), 0);
}

#[test]
fn test_balance_of_returns_correct_balance_after_mint() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new();
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
    let mut portfolio = Portfolio::new();
    
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
    let mut portfolio = Portfolio::new();
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
    let mut portfolio = Portfolio::new();
    
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
    let mut portfolio = Portfolio::new();
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
    let mut portfolio = Portfolio::new();
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
    let mut portfolio = Portfolio::new();
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
    let mut portfolio = Portfolio::new();
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
    let portfolio = Portfolio::new();
    let user = Address::generate(&env);

    // New user should have no badges
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), false);
    assert_eq!(portfolio.get_user_badges(&env, user).len(), 0);
}

/// Test reward logic integration with trade counting
#[test]
fn test_rewards_integrate_with_trade_counting() {
    let env = Env::default();
    let mut portfolio = Portfolio::new();
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