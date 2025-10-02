use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

mod portfolio;
use portfolio::{Portfolio, Asset};
pub use portfolio::Badge;

#[contract]
pub struct CounterContract;

#[contractimpl]
impl CounterContract {
    pub fn mint(env: Env, token: Symbol, to: Address, amount: i128) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get()
            .unwrap_or_else(Portfolio::new);

        let asset = match token.to_string().as_str() {
            "XLM" => Asset::XLM,
            _ => Asset::Custom(token.clone()),
        };

        portfolio.mint(&env, asset, to, amount);

        env.storage().instance().set(&portfolio);
    }

    pub fn balance_of(env: Env, token: Symbol, user: Address) -> i128 {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get()
            .unwrap_or_else(Portfolio::new);

        let asset = match token.to_string().as_str() {
            "XLM" => Asset::XLM,
            _ => Asset::Custom(token.clone()),
        };

        portfolio.balance_of(&env, asset, user)
    }

    /// Record a swap execution for a user
    pub fn record_trade(env: Env, user: Address) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get()
            .unwrap_or_else(Portfolio::new);

        portfolio.record_trade(&env, user);

        env.storage().instance().set(&portfolio);
    }

    /// Get portfolio stats for a user (trade count, pnl)
    pub fn get_portfolio(env: Env, user: Address) -> (u32, i128) {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get()
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
            .get()
            .unwrap_or_else(Portfolio::new);

        portfolio.has_badge(&env, user, badge)
    }

    /// Get all badges earned by a user
    pub fn get_user_badges(env: Env, user: Address) -> Vec<Badge> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get()
            .unwrap_or_else(Portfolio::new);

        portfolio.get_user_badges(&env, user)
    }
}

#[cfg(test)]
mod balance_test;

#[cfg(test)]
mod rewards_test;