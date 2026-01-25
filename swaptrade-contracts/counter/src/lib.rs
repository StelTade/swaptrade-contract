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

use portfolio::{Portfolio, Asset, LPPosition};
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
        if let Err(_limit_status) = RateLimiter::check_swap_limit(&env, &user, &user_tier) {
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

    // ===== LIQUIDITY PROVIDER (LP) FUNCTIONS =====

    /// Add liquidity to the pool and mint LP tokens
    /// Returns the number of LP tokens minted
    pub fn add_liquidity(env: Env, xlm_amount: i128, usdc_amount: i128, user: Address) -> i128 {
        assert!(xlm_amount > 0, "XLM amount must be positive");
        assert!(usdc_amount > 0, "USDC amount must be positive");

        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        // Check rate limit for LP operations
        let user_tier = portfolio.get_user_tier(&env, user.clone());
        if let Err(_) = RateLimiter::check_lp_limit(&env, &user, &user_tier) {
            panic!("RATELIMIT");
        }

        // Get current pool state
        let current_xlm = portfolio.get_liquidity(Asset::XLM);
        let current_usdc = portfolio.get_liquidity(Asset::Custom(symbol_short!("USDCSIM")));
        let total_lp_tokens = portfolio.get_total_lp_tokens();

        // Check user has sufficient balance
        let user_xlm_balance = portfolio.balance_of(&env, Asset::XLM, user.clone());
        let user_usdc_balance = portfolio.balance_of(&env, Asset::Custom(symbol_short!("USDCSIM")), user.clone());
        
        assert!(user_xlm_balance >= xlm_amount, "Insufficient XLM balance");
        assert!(user_usdc_balance >= usdc_amount, "Insufficient USDC balance");

        // Calculate LP tokens to mint using constant product AMM formula
        // If pool is empty, LP tokens = sqrt(xlm * usdc)
        // Otherwise, LP tokens = (deposit / pool_size) * total_lp_tokens
        let lp_tokens_minted = if total_lp_tokens == 0 {
            // First liquidity provider: LP tokens = sqrt(xlm * usdc)
            // Use integer square root (Babylonian method)
            let product = (xlm_amount as u128).saturating_mul(usdc_amount as u128);
            if product == 0 {
                panic!("Product must be positive");
            }
            // Integer square root using Babylonian method
            let mut guess = product;
            let mut prev_guess = 0u128;
            // Limit iterations to prevent infinite loop
            let mut iterations = 0;
            while guess != prev_guess && iterations < 100 {
                prev_guess = guess;
                let quotient = product / guess;
                guess = (guess + quotient) / 2;
                if guess == 0 {
                    guess = 1;
                    break;
                }
                iterations += 1;
            }
            guess as i128
        } else {
            // Calculate proportional share
            // LP tokens = min((xlm_amount / current_xlm) * total_lp_tokens, (usdc_amount / current_usdc) * total_lp_tokens)
            // This ensures the ratio is maintained
            let xlm_share = if current_xlm > 0 {
                (xlm_amount as u128).saturating_mul(total_lp_tokens as u128) / (current_xlm as u128)
            } else {
                0
            };
            let usdc_share = if current_usdc > 0 {
                (usdc_amount as u128).saturating_mul(total_lp_tokens as u128) / (current_usdc as u128)
            } else {
                0
            };
            
            // Take minimum to maintain ratio
            core::cmp::min(xlm_share as i128, usdc_share as i128)
        };

        assert!(lp_tokens_minted > 0, "LP tokens minted must be positive");

        // Debit assets from user (transfer to pool)
        portfolio.debit(&env, Asset::XLM, user.clone(), xlm_amount);
        portfolio.debit(&env, Asset::Custom(symbol_short!("USDCSIM")), user.clone(), usdc_amount);

        // Update pool liquidity
        portfolio.add_pool_liquidity(xlm_amount, usdc_amount);

        // Update or create LP position
        let existing_position = portfolio.get_lp_position(user.clone());
        let new_position = if let Some(mut pos) = existing_position {
            // Update existing position
            pos.xlm_deposited = pos.xlm_deposited.saturating_add(xlm_amount);
            pos.usdc_deposited = pos.usdc_deposited.saturating_add(usdc_amount);
            pos.lp_tokens_minted = pos.lp_tokens_minted.saturating_add(lp_tokens_minted);
            pos
        } else {
            // Create new position
            LPPosition {
                lp_address: user.clone(),
                xlm_deposited: xlm_amount,
                usdc_deposited: usdc_amount,
                lp_tokens_minted,
            }
        };

        portfolio.set_lp_position(user.clone(), new_position);
        portfolio.add_total_lp_tokens(lp_tokens_minted);

        // Record LP deposit for badge tracking
        portfolio.record_lp_deposit(user.clone());
        portfolio.check_and_award_badges(&env, user.clone());

        // Record rate limit usage
        RateLimiter::record_lp_op(&env, &user, env.ledger().timestamp());

        env.storage().instance().set(&(), &portfolio);

        lp_tokens_minted
    }

    /// Remove liquidity from the pool by burning LP tokens
    /// Returns (xlm_amount, usdc_amount) returned to user
    pub fn remove_liquidity(env: Env, lp_tokens: i128, user: Address) -> (i128, i128) {
        assert!(lp_tokens > 0, "LP tokens must be positive");

        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        // Get user's LP position
        let position = portfolio.get_lp_position(user.clone());
        assert!(position.is_some(), "User has no LP position");
        let mut pos = position.unwrap();

        // Verify user has enough LP tokens
        assert!(pos.lp_tokens_minted >= lp_tokens, "Insufficient LP tokens");

        // Get current pool state
        let current_xlm = portfolio.get_liquidity(Asset::XLM);
        let current_usdc = portfolio.get_liquidity(Asset::Custom(symbol_short!("USDCSIM")));
        let total_lp_tokens = portfolio.get_total_lp_tokens();

        assert!(total_lp_tokens > 0, "No LP tokens in pool");

        // Calculate proportional share of pool
        // xlm_amount = (lp_tokens / total_lp_tokens) * current_xlm
        // usdc_amount = (lp_tokens / total_lp_tokens) * current_usdc
        let xlm_amount = ((lp_tokens as u128).saturating_mul(current_xlm as u128) / (total_lp_tokens as u128)) as i128;
        let usdc_amount = ((lp_tokens as u128).saturating_mul(current_usdc as u128) / (total_lp_tokens as u128)) as i128;

        assert!(xlm_amount > 0 && usdc_amount > 0, "Amounts must be positive");

        // Verify we're not removing more than deposited (with rounding tolerance)
        // Allow small rounding differences
        let max_xlm = pos.xlm_deposited;
        let max_usdc = pos.usdc_deposited;
        
        // Check if removing more than deposited (with 1% tolerance for rounding)
        if xlm_amount > max_xlm.saturating_mul(101) / 100 || usdc_amount > max_usdc.saturating_mul(101) / 100 {
            panic!("Cannot remove more than deposited");
        }

        // Update pool liquidity (subtract)
        portfolio.set_liquidity(Asset::XLM, current_xlm.saturating_sub(xlm_amount));
        portfolio.set_liquidity(Asset::Custom(symbol_short!("USDCSIM")), current_usdc.saturating_sub(usdc_amount));

        // Transfer assets from pool to user
        portfolio.mint(&env, Asset::XLM, user.clone(), xlm_amount);
        portfolio.mint(&env, Asset::Custom(symbol_short!("USDCSIM")), user.clone(), usdc_amount);

        // Update LP position
        pos.lp_tokens_minted = pos.lp_tokens_minted.saturating_sub(lp_tokens);
        pos.xlm_deposited = pos.xlm_deposited.saturating_sub(xlm_amount);
        pos.usdc_deposited = pos.usdc_deposited.saturating_sub(usdc_amount);

        if pos.lp_tokens_minted == 0 {
            // Remove position if all tokens burned
            // Note: Map doesn't have remove, so we set to a zero position or track separately
            // For now, we'll keep it with zero values
        }
        portfolio.set_lp_position(user.clone(), pos);
        portfolio.subtract_total_lp_tokens(lp_tokens);

        // Record rate limit usage
        RateLimiter::record_lp_op(&env, &user, env.ledger().timestamp());

        env.storage().instance().set(&(), &portfolio);

        (xlm_amount, usdc_amount)
    }

    /// Get LP positions for a user
    /// Returns a Vec containing the user's position if it exists
    pub fn get_lp_positions(env: Env, user: Address) -> Vec<LPPosition> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let mut result = Vec::new(&env);
        if let Some(position) = portfolio.get_lp_position(user) {
            result.push_back(position);
        }
        result
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
#[cfg(test)]
mod lp_tests;
