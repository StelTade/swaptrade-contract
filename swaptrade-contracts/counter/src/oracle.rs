
use soroban_sdk::{contracttype, Env, Symbol};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    InvalidPrice = 1,
    StalePrice = 2,
    SlippageExceeded = 3,
    PriceNotSet = 4,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PriceData {
    pub price: u128,
    pub timestamp: u64,
}

pub trait PriceFeed {
    fn get_price(env: &Env, token_pair: (Symbol, Symbol)) -> Result<u128, ContractError>;
    fn last_update_time(env: &Env, token_pair: (Symbol, Symbol)) -> u64;
    fn set_price(env: &Env, token_pair: (Symbol, Symbol), price: u128);
}

// Helper functions for storage management
pub fn get_stored_price(env: &Env, pair: (Symbol, Symbol)) -> Option<PriceData> {
    env.storage().instance().get(&pair)
}

pub fn set_stored_price(env: &Env, pair: (Symbol, Symbol), price: u128) {
    let timestamp = env.ledger().timestamp();
    let data = PriceData { price, timestamp };
    env.storage().instance().set(&pair, &data);
}

pub fn get_price_safe(env: &Env, pair: (Symbol, Symbol)) -> Result<u128, ContractError> {
    match get_stored_price(env, pair) {
        Some(data) => Ok(data.price),
        None => Err(ContractError::PriceNotSet),
    }
}
