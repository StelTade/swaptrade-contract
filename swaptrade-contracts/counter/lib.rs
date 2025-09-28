use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

mod portfolio;
use portfolio::{Portfolio, Asset};

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

    /// Get portfolio stats for a user (trade count, pnl, balances)
    pub fn get_portfolio(env: Env, user: Address) -> (u32, i128, Vec<(Symbol, i128)>) {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get()
            .unwrap_or_else(Portfolio::new);

        let (trades, pnl, balances) = portfolio.get_portfolio(&env, user.clone());

        // Convert Asset back into Symbol for easier querying
        let mut out: Vec<(Symbol, i128)> = Vec::new(&env);
        for (asset, bal) in balances.iter() {
            let sym = match asset {
                portfolio::Asset::XLM => Symbol::new(&env, "XLM"),
                portfolio::Asset::Custom(s) => s,
            };
            out.push_back((sym, bal));
        }

        (trades, pnl, out)
    }
}