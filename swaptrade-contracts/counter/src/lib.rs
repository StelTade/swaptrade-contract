use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec, symbol_short};

// Bring in modules from parent directory
mod events;
mod rewards;
mod rate_limit;
mod emergency;
mod validation;
pub mod errors;

// defined in events mod
mod portfolio { include!("../portfolio.rs"); }
mod trading { include!("../trading.rs"); }
mod batch { include!("../batch.rs"); }
mod tiers { include!("../tiers.rs"); }
pub mod oracle;

use portfolio::{Portfolio, Asset};
pub use portfolio::{Badge, Metrics, Transaction};
pub use tiers::UserTier;
pub use rate_limit::{RateLimiter, RateLimitStatus};
use trading::perform_swap;
// tiers import removed (unused)
use crate::events::Events;
use validation::*;



// Batch imports
use batch::{
    BatchOperation,
    BatchResult,
    OperationResult,
    execute_batch_atomic,
    execute_batch_best_effort,
};

// Oracle imports
use oracle::{set_stored_price, get_price_safe};

#[contract]
pub struct CounterContract;

#[contractimpl]
impl CounterContract {
    // ===== ORACLE =====

    pub fn set_price(env: Env, pair: (Symbol, Symbol), price: u128) {
        set_stored_price(&env, pair, price);
    }

    pub fn get_current_price(env: Env, pair: (Symbol, Symbol)) -> u128 {
        get_price_safe(&env, pair).unwrap_or(0)
    }

    pub fn set_max_slippage_bps(env: Env, bps: u32) {
        env.storage().instance().set(&symbol_short!("MAX_SLIP"), &bps);
    }

    pub fn get_max_slippage_bps(env: Env) -> u32 {
        env.storage().instance().get(&symbol_short!("MAX_SLIP")).unwrap_or(10000)
    }

    // ===== PORTFOLIO / TOKEN MGMT =====

    pub fn set_pool_liquidity(env: Env, token: Symbol, amount: i128) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let asset = if token == symbol_short!("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(token.clone())
        };

        portfolio.set_liquidity(asset, amount);
        env.storage().instance().set(&(), &portfolio);
    }

    pub fn mint(env: Env, token: Symbol, to: Address, amount: i128) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let asset = if token == symbol_short!("XLM") {
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
            .unwrap_or_else(|| Portfolio::new(&env));

        let asset = if token == symbol_short!("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(token.clone())
        };

        portfolio.balance_of(&env, asset, user)
    }

    pub fn get_balance(env: Env, token: Symbol, owner: Address) -> i128 {
        Self::balance_of(env, token, owner)
    }

    /// Swap tokens using simplified AMM (1:1 XLM <-> USDCSIM)
    /// Applies tier-based fee discounts and checks rate limits
    pub fn swap(env: Env, from: Symbol, to: Symbol, amount: i128, user: Address) -> i128 {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        // Get user's current tier for fee calculation and rate limiting
        let user_tier = portfolio.get_user_tier(&env, user.clone());
        
        // Check rate limit before executing swap
        if let Err(limit_status) = RateLimiter::check_swap_limit(&env, &user, &user_tier) {
            panic!("RATELIMIT");
        }

        let fee_bps = user_tier.effective_fee_bps();

        // Calculate fee amount (fee is collected on input amount)
        let fee_amount = (amount * fee_bps as i128) / 10000;
        let swap_amount = amount - fee_amount;

        // Collect the fee
        if fee_amount > 0 {
            // Deduct from user
            let fee_asset = if from == symbol_short!("XLM") {
                Asset::XLM
            } else {
                Asset::Custom(from.clone())
            };
            
            // We need to use a mutable borrow of portfolio which we already have
            portfolio.debit(&env, fee_asset, user.clone(), fee_amount);
            portfolio.collect_fee(fee_amount);
        }

        let out_amount = perform_swap(&env, &mut portfolio, from.clone(), to.clone(), swap_amount, user.clone());

        // Record trade with full transaction details
        portfolio.record_transaction(&env, user.clone(), from, to, amount, out_amount);

        // Update user's tier after trade
        let (_new_tier, _tier_changed) = portfolio.update_tier(&env, user.clone());

        // Record rate limit usage
        RateLimiter::record_swap(&env, &user, env.ledger().timestamp());

        // --- REWARDS LOGIC START ---
        // Award the "First Trade" badge. 
        // The internal check in rewards.rs handles duplicate prevention.
        crate::rewards::award_first_trade(&env, user.clone());
        // --- REWARDS LOGIC END ---

        env.storage().instance().set(&(), &portfolio);

        // Optional structured logging for successful swap
        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events().publish(
                (symbol_short!("swap")),
                (amount, out_amount, fee_amount, user_tier),
            );
        }

    out_amount
    }

    /// Non-panicking swap that counts failed orders and returns 0 on failure
    /// Applies tier-based fee discounts and checks rate limits
    pub fn safe_swap(env: Env, from: Symbol, to: Symbol, amount: i128, user: Address) -> i128 {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let valid = amount > 0 && from != to;

        if !valid {
            portfolio.inc_failed_order();
            env.storage().instance().set(&(), &portfolio);
            return 0;
        }

        // Get user's current tier for fee calculation and rate limiting
        let user_tier = portfolio.get_user_tier(&env, user.clone());
        
        // Check rate limit before executing swap
        if let Err(_) = RateLimiter::check_swap_limit(&env, &user, &user_tier) {
            portfolio.inc_failed_order();
            env.storage().instance().set(&(), &portfolio);
            return 0;
        }

        let fee_bps = user_tier.effective_fee_bps();

        // Calculate fee amount (fee is collected on input amount)
        let fee_amount = (amount * fee_bps as i128) / 10000;
        let swap_amount = amount - fee_amount;

        // Collect the fee
        if fee_amount > 0 {
            portfolio.collect_fee(fee_amount);
        }

        let out_amount = perform_swap(&env, &mut portfolio, from.clone(), to.clone(), swap_amount, user.clone());
        portfolio.record_transaction(&env, user.clone(), from, to, amount, out_amount);

        // Update user's tier after trade
        let (_new_tier, _tier_changed) = portfolio.update_tier(&env, user.clone());

        // Record rate limit usage
        RateLimiter::record_swap(&env, &user, env.ledger().timestamp());
        env.storage().instance().set(&(), &portfolio);

        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events().publish(
                (symbol_short!("swap")),
                (amount, out_amount, fee_amount, user_tier),
            );
        }

        out_amount
    }

    // ===== METRICS & BADGES =====

    pub fn record_trade(env: Env, user: Address) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.record_trade(&env, user);
        env.storage().instance().set(&(), &portfolio);
    }

    pub fn get_portfolio(env: Env, user: Address) -> (u32, i128) {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_portfolio(&env, user)
    }

    pub fn get_metrics(env: Env) -> Metrics {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_metrics()
    }

    pub fn has_badge(env: Env, user: Address, badge: Badge) -> bool {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.has_badge(&env, user, badge)
    }

    pub fn get_user_badges(env: Env, user: Address) -> Vec<Badge> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_user_badges(&env, user)
    }

    pub fn get_user_transactions(env: Env, user: Address, limit: u32) -> Vec<Transaction> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_user_transactions(&env, user, limit)
    }

    /// Get the current tier for a user
    pub fn get_user_tier(env: Env, user: Address) -> UserTier {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_user_tier(&env, user)
    }

    // ===== RATE LIMITING =====

    /// Get rate limit status for swap operations
    pub fn get_swap_rate_limit(env: Env, user: Address) -> RateLimitStatus {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let user_tier = portfolio.get_user_tier(&env, user.clone());
        RateLimiter::get_swap_status(&env, &user, &user_tier)
    }

    /// Get rate limit status for LP operations
    pub fn get_lp_rate_limit(env: Env, user: Address) -> RateLimitStatus {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let user_tier = portfolio.get_user_tier(&env, user.clone());
        RateLimiter::get_lp_status(&env, &user, &user_tier)
    }

    // ===== BATCH OPERATIONS =====

    pub fn execute_batch_atomic(env: Env, operations: Vec<BatchOperation>) -> BatchResult {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let result = execute_batch_atomic(&env, &mut portfolio, operations);

        match result {
            Ok(res) => {
                env.storage().instance().set(&(), &portfolio);
                res
            }
            Err(_) => {
                let mut err = BatchResult::new(&env);
                err.operations_failed = 1;
                err
            }
        }
    }

    pub fn execute_batch_best_effort(env: Env, operations: Vec<BatchOperation>) -> BatchResult {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let result = execute_batch_best_effort(&env, &mut portfolio, operations);

        match result {
            Ok(res) => {
                env.storage().instance().set(&(), &portfolio);
                res
            }
            Err(_) => {
                let mut err = BatchResult::new(&env);
                err.operations_failed = 1;
                err
            }
        }
    }

    pub fn execute_batch(env: Env, operations: Vec<BatchOperation>) -> BatchResult {
        Self::execute_batch_atomic(env, operations)
    }
}

#[cfg(test)]
mod balance_test;
#[cfg(test)]
mod oracle_tests;
#[cfg(test)]
mod batch_tests;
#[cfg(test)]
mod rate_limit_tests;
#[cfg(test)]
mod transaction_tests;
