#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec, symbol_short};

// Bring in modules from parent directory
mod portfolio { include!("../portfolio.rs"); }
mod trading { include!("../trading.rs"); }
pub mod oracle;

use portfolio::{Portfolio, Asset};
pub use portfolio::Badge;
pub use portfolio::Metrics;
use trading::perform_swap;
use oracle::{set_stored_price, get_price_safe};

#[contract]
pub struct CounterContract;

#[contractimpl]
impl CounterContract {
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

    /// Alias to match external API
    pub fn get_balance(env: Env, token: Symbol, owner: Address) -> i128 {
        Self::balance_of(env, token, owner)
    }

    /// Swap tokens using simplified AMM (1:1 XLM <-> USDCSIM)
    pub fn swap(env: Env, from: Symbol, to: Symbol, amount: i128, user: Address) -> i128 {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let out_amount = perform_swap(&env, &mut portfolio, from, to, amount, user.clone());

        portfolio.record_trade(&env, user);
        env.storage().instance().set(&(), &portfolio);

        // Optional structured logging for successful swap
        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events().publish(
                (symbol_short!("swap")),
                (amount, out_amount),
            );
        }

        out_amount
    }

    /// Non-panicking swap that counts failed orders and returns 0 on failure
    pub fn safe_swap(env: Env, from: Symbol, to: Symbol, amount: i128, user: Address) -> i128 {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let tokens_ok = (from == symbol_short!("XLM") || from == symbol_short!("USDCSIM"))
            && (to == symbol_short!("XLM") || to == symbol_short!("USDCSIM"));
        let pair_ok = from != to;
        let amount_ok = amount > 0;

        if !(tokens_ok && pair_ok && amount_ok) {
            // Count failed order
            portfolio.inc_failed_order();
            env.storage().instance().set(&(), &portfolio);

            #[cfg(feature = "logging")]
            {
                use soroban_sdk::symbol_short;
                env.events().publish(
                    (symbol_short!("swap_failed"), user.clone()),
                    (from, to, amount),
                );
            }
            return 0;
        }

    let out_amount = perform_swap(&env, &mut portfolio, from, to, amount, user.clone());
    portfolio.record_trade(&env, user);
    env.storage().instance().set(&(), &portfolio);

        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events().publish(
                (symbol_short!("swap")),
                (amount, out_amount),
            );
        }

        out_amount
    }

    /// Record a swap execution for a user
    pub fn record_trade(env: Env, user: Address) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.record_trade(&env, user);

        env.storage().instance().set(&(), &portfolio);
    }

    /// Get portfolio stats for a user (trade count, pnl)
    pub fn get_portfolio(env: Env, user: Address) -> (u32, i128) {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_portfolio(&env, user)
    }

    /// Get aggregate metrics
    pub fn get_metrics(env: Env) -> Metrics {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_metrics()
    }

    /// Check if a user has earned a specific badge
    pub fn has_badge(env: Env, user: Address, badge: Badge) -> bool {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.has_badge(&env, user, badge)
    }

    /// Get all badges earned by a user
    pub fn get_user_badges(env: Env, user: Address) -> Vec<Badge> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_user_badges(&env, user)
    }
}

#[cfg(test)]
mod balance_test;
#[cfg(test)]
mod oracle_tests;

// trading tests are provided as integration/unit tests in the repository tests/ folder
