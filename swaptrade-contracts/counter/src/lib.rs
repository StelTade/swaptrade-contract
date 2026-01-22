use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec, symbol_short};

// Bring in modules from parent directory
mod portfolio { include!("../portfolio.rs"); }
mod trading { include!("../trading.rs"); }
mod batch { include!("../batch.rs"); }
pub mod oracle;

use portfolio::{Portfolio, Asset};
pub use portfolio::{Badge, Metrics};
use trading::perform_swap;

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

    // ===== SWAPS =====

    pub fn swap(env: Env, from: Symbol, to: Symbol, amount: i128, user: Address) -> i128 {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let out_amount = perform_swap(&env, &mut portfolio, from, to, amount, user.clone());
        portfolio.record_trade(&env, user);
        env.storage().instance().set(&(), &portfolio);

        out_amount
    }

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

        let out_amount = perform_swap(&env, &mut portfolio, from, to, amount, user.clone());
        portfolio.record_trade(&env, user);
        env.storage().instance().set(&(), &portfolio);

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

    // ===== BATCH OPERATIONS =====

    pub fn execute_batch_atomic(env: Env, operations: Vec<BatchOperation>) -> BatchResult {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(Portfolio::new);

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
            .unwrap_or_else(Portfolio::new);

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
