use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

mod portfolio;
use portfolio::{Portfolio, Asset};
pub use portfolio::Badge;
mod trading;
use trading::perform_swap;
mod referral;
mod rewards;
use referral::ReferralSystem;

#[contract]
pub struct CounterContract;

#[contractimpl]
impl CounterContract {
    pub fn mint(env: Env, token: Symbol, to: Address, amount: i128) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        let asset = if token == Symbol::short("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(token.clone())
        };

        portfolio.mint(&env, asset, to, amount);

        env.storage().instance().set(&(), &portfolio);
    }

    pub fn balance_of(env: Env, token: Symbol, user: Address) -> i128 {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        let asset = if token == Symbol::short("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(token.clone())
        };

        portfolio.balance_of(&env, asset, user)
    }

    /// Swap tokens for a user using a simplified AMM (1:1 XLM <-> USDC-SIM)
    /// - Validates input tokens
    /// - Checks sufficient funds
    /// - Debits from `from` and credits to `to`
    /// - Records the trade
    /// Returns amount received
    pub fn swap(env: Env, from: Symbol, to: Symbol, amount: i128, user: Address) -> i128 {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        // perform swap (validates tokens and amount internally)
        let out_amount = perform_swap(&env, &mut portfolio, from, to, amount, user.clone());

        // record trade and persist state
        portfolio.record_trade_with_amount(&env, user, amount);
        env.storage().instance().set(&(), &portfolio);

        out_amount
    }

    /// Record a swap execution for a user
    pub fn record_trade(env: Env, user: Address) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        portfolio.record_trade(&env, user);

        env.storage().instance().set(&(), &portfolio);
    }

    /// Get portfolio stats for a user (trade count, pnl)
    pub fn get_portfolio(env: Env, user: Address) -> (u32, i128) {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        portfolio.get_portfolio(&env, user)
    }

    /// Get balance for a given token and address
    /// This is an alias for balance_of to match the requirements
    pub fn get_balance(env: Env, token: Symbol, owner: Address) -> i128 {
        Self::balance_of(env, token, owner)
    }

    /// Check if a user has earned a specific badge
    pub fn has_badge(env: Env, user: Address, badge: Badge) -> bool {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        portfolio.has_badge(&env, user, badge)
    }

    /// Get all badges earned by a user
    pub fn get_user_badges(env: Env, user: Address) -> Vec<Badge> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        portfolio.get_user_badges(&env, user)
    }

    // ===== ADMIN DASHBOARD QUERY FUNCTIONS (Read-only) =====

    /// Get the total number of unique traders and LPs on the platform
    /// 
    /// Returns: u32 representing the count of unique traders and liquidity providers
    /// 
    /// Time Complexity: O(1) - stored as aggregate statistic
    /// 
    /// Purpose: Provides educators and maintainers with overall platform user count
    pub fn get_total_users(env: Env) -> u32 {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        portfolio.get_total_users()
    }

    /// Get the total trading volume across all users
    /// 
    /// Returns: i128 representing the sum of all swap amounts executed
    /// 
    /// Time Complexity: O(1) - stored as aggregate statistic
    /// 
    /// Purpose: Helps educators track ecosystem activity and trading momentum
    pub fn get_total_trading_volume(env: Env) -> i128 {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        portfolio.get_total_trading_volume()
    }

    /// Get the count of active users (users with recorded trades)
    /// 
    /// Returns: u32 representing the number of users with at least one trade
    /// 
    /// Time Complexity: O(1) - length of active users vector
    /// 
    /// Purpose: Identifies engagement and helps spot inactive users
    pub fn get_active_users_count(env: Env) -> u32 {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        portfolio.get_active_users_count()
    }

    /// Get the top N traders by PnL (leaderboard)
    /// 
    /// Arguments:
    ///   limit: u32 - Maximum number of traders to return (capped at 100 for safety)
    /// 
    /// Returns: Vec<(Address, i128)> - List of (user_address, pnl) sorted by PnL descending
    /// 
    /// Time Complexity: O(1) - precomputed top 100 list, limited copy operation
    /// 
    /// Safety: Automatically capped at top 100 traders to prevent excessive data retrieval
    /// 
    /// Purpose: Enables leaderboards for educators and helps identify top performers
    pub fn get_top_traders(env: Env, limit: u32) -> Vec<(Address, i128)> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        portfolio.get_top_traders(limit)
    }

    /// Get pool statistics including liquidity and accumulated fees
    /// 
    /// Returns: (i128, i128, i128) tuple containing:
    ///   - xlm_in_pool: XLM liquidity currently in the pool
    ///   - usdc_in_pool: USDC liquidity currently in the pool
    ///   - total_fees_collected: Cumulative fees collected by the platform
    /// 
    /// Time Complexity: O(1) - simple tuple return of stored values
    /// 
    /// Purpose: Provides operational visibility into pool health and fee accumulation
    pub fn get_pool_stats(env: Env) -> (i128, i128, i128) {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

        portfolio.get_pool_stats()
    }

    // ===== REFERRAL SYSTEM FUNCTIONS =====
    
    /// Generate a unique referral code for a user
    pub fn generate_referral_code(env: Env, user: Address) -> Symbol {
        let mut referral_system: ReferralSystem = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "referral_system"))
            .unwrap_or_else(|| ReferralSystem::new(&env));

        let code = referral_system.generate_referral_code(&env, user);
        
        env.storage().instance().set(&Symbol::new(&env, "referral_system"), &referral_system);
        
        code
    }

    /// Register a new user with a referral code
    pub fn register_with_referral(env: Env, referral_code: Symbol, new_user: Address) -> Result<(), &'static str> {
        let mut referral_system: ReferralSystem = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "referral_system"))
            .unwrap_or_else(|| ReferralSystem::new(&env));

        let result = referral_system.register_with_referral(&env, referral_code, new_user);
        
        if result.is_ok() {
            env.storage().instance().set(&Symbol::new(&env, "referral_system"), &referral_system);
        }
        
        result
    }

    /// Get referral code for a user
    pub fn get_referral_code(env: Env, user: Address) -> Symbol {
        let referral_system: ReferralSystem = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "referral_system"))
            .unwrap_or_else(|| ReferralSystem::new(&env));

        referral_system.get_referral_code(&env, user)
    }

    /// Get list of referrals for a user
    pub fn get_referrals(env: Env, user: Address) -> Vec<Address> {
        let referral_system: ReferralSystem = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "referral_system"))
            .unwrap_or_else(|| ReferralSystem::new(&env));

        referral_system.get_referrals(&env, user)
    }

    /// Get referral rewards for a user
    pub fn get_referral_rewards(env: Env, user: Address) -> i128 {
        let referral_system: ReferralSystem = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "referral_system"))
            .unwrap_or_else(|| ReferralSystem::new(&env));

        referral_system.get_referral_rewards(&env, user)
    }

    /// Claim referral rewards for a user
    pub fn claim_referral_rewards(env: Env, user: Address) -> i128 {
        let mut referral_system: ReferralSystem = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "referral_system"))
            .unwrap_or_else(|| ReferralSystem::new(&env));

        let rewards = referral_system.claim_referral_rewards(&env, user);
        
        env.storage().instance().set(&Symbol::new(&env, "referral_system"), &referral_system);
        
        rewards
    }
}

// counter/src/lib.rs
mod trading;
mod portfolio;
mod errors;

pub use trading::swap_tokens;
pub use portfolio::{get_balance, deposit, withdraw};
pub use errors::ContractError;


#[cfg(test)]
mod balance_test;

#[cfg(test)]
mod rewards_test;

#[cfg(test)]
mod dashboard_tests;

#[cfg(test)]
mod achievements_tests;

#[cfg(test)]
mod referral_tests;

#[cfg(test)]
mod trading_tests;