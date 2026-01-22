#![allow(non_snake_case)]
use soroban_sdk::{contracttype, contracterror, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OracleError {
    PriceNotFound = 1,
    StalePrice = 2,
    SlippageExceeded = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: u128,      // 18 decimals fixed point
    pub timestamp: u64,
    pub liquidity: u128,  // Virtual liquidity for slippage calc
}

#[contracttype]
pub enum DataKey {
    Price((Symbol, Symbol)),
    MaxSlippage,
}

// 18 decimals
pub const PRECISION: u128 = 1_000_000_000_000_000_000;

pub trait PriceFeed {
    fn get_price(env: &Env, pair: (Symbol, Symbol)) -> Result<PriceData, OracleError>;
    fn set_price(env: &Env, pair: (Symbol, Symbol), price: u128, liquidity: u128);
    fn last_update_time(env: &Env, pair: (Symbol, Symbol)) -> u64;
}

pub struct Oracle;

impl Oracle {
    pub fn get_price(env: &Env, pair: (Symbol, Symbol)) -> Result<PriceData, OracleError> {
        let key = DataKey::Price(pair);
        if let Some(data) = env.storage().persistent().get::<_, PriceData>(&key) {
             Ok(data)
        } else {
             Err(OracleError::PriceNotFound)
        }
    }

    pub fn set_price(env: &Env, pair: (Symbol, Symbol), price: u128, liquidity: u128) {
        let key = DataKey::Price(pair);
        let data = PriceData {
            price,
            timestamp: env.ledger().timestamp(),
            liquidity
        };
        env.storage().persistent().set(&key, &data);
    }
    
    pub fn last_update_time(env: &Env, pair: (Symbol, Symbol)) -> u64 {
        match Self::get_price(env, pair) {
            Ok(data) => data.timestamp,
            Err(_) => 0,
        }
    }
}

pub fn set_max_slippage(env: &Env, bps: u32) {
    env.storage().persistent().set(&DataKey::MaxSlippage, &bps);
}

pub fn get_max_slippage(env: &Env) -> u32 {
    env.storage().persistent().get(&DataKey::MaxSlippage).unwrap_or(10000) // Default to 100% (10000 bps) if not set? Or strict? 
    // Requirement: "Add max_slippage_bps (basis points) setting, return error if exceeded"
    // If not set, maybe no limit? or 100%?
    // Let's default to max (no slippage protection unless set) or maybe a reasonable default.
    // The user says "Add max_slippage_bps setting", implying it's a constraint.
    // I'll default to 0 (which would block everything) or a high number.
    // Let's check if "get" returns option. `get` returns Option.
    // I'll handle defaults in logic or return 10000 (100%).
}
