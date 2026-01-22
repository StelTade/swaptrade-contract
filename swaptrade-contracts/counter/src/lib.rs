#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec, symbol_short};

// Bring in modules from parent directory
mod portfolio { include!("../portfolio.rs"); }
mod trading { include!("../trading.rs"); }
mod oracle;

use portfolio::{Portfolio, Asset};
pub use portfolio::Badge;
use trading::perform_swap;
use oracle::{Oracle};

#[contract]
pub struct CounterContract;

#[contractimpl]
impl CounterContract {
    pub fn set_price(env: Env, from: Symbol, to: Symbol, price: u128, liquidity: u128) {
        Oracle::set_price(&env, (from, to), price, liquidity);
    }

    pub fn get_current_price(env: Env, from: Symbol, to: Symbol) -> u128 {
         match Oracle::get_price(&env, (from, to)) {
             Ok(data) => data.price,
             Err(_) => 0,
         }
    }

    pub fn set_max_slippage(env: Env, bps: u32) {
        oracle::set_max_slippage(&env, bps);
    }

    pub fn mint(env: Env, token: Symbol, to: Address, amount: i128) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&symbol_short!("PORTFOLIO"))
            .unwrap_or_else(|| Portfolio::new(&env));

        let asset = if token == symbol_short!("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(token.clone())
        };

        portfolio.mint(&env, asset, to, amount);

        env.storage().instance().set(&symbol_short!("PORTFOLIO"), &portfolio);
    }

    pub fn balance_of(env: Env, token: Symbol, user: Address) -> i128 {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&symbol_short!("PORTFOLIO"))
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

    /// Swap tokens using simplified AMM (1:1 XLM <-> USDC-SIM)
    pub fn swap(env: Env, from: Symbol, to: Symbol, amount: i128, user: Address) -> i128 {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&symbol_short!("PORTFOLIO"))
            .unwrap_or_else(|| Portfolio::new(&env));

        let out_amount = perform_swap(&env, &mut portfolio, from, to, amount, user.clone());

        portfolio.record_trade(&env, user);
        env.storage().instance().set(&symbol_short!("PORTFOLIO"), &portfolio);

        out_amount
    }

    /// Record a swap execution for a user
    pub fn record_trade(env: Env, user: Address) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&symbol_short!("PORTFOLIO"))
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.record_trade(&env, user);

        env.storage().instance().set(&symbol_short!("PORTFOLIO"), &portfolio);
    }

    /// Get portfolio stats for a user (trade count, pnl)
    pub fn get_portfolio(env: Env, user: Address) -> (u32, i128) {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&symbol_short!("PORTFOLIO"))
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_portfolio(&env, user)
    }

    /// Check if a user has earned a specific badge
    pub fn has_badge(env: Env, user: Address, badge: Badge) -> bool {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&symbol_short!("PORTFOLIO"))
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.has_badge(&env, user, badge)
    }

    /// Get all badges earned by a user
    pub fn get_user_badges(env: Env, user: Address) -> Vec<Badge> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&symbol_short!("PORTFOLIO"))
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_user_badges(&env, user)
    }
}

mod test;

#[cfg(test)]
mod oracle_test;
