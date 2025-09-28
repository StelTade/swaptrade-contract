use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};
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
}