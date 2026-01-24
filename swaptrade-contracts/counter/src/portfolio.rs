// counter/src/portfolio.rs
use soroban_sdk::{Env, Address, Symbol, Result as SorobanResult};
use crate::errors::ContractError;

// Mock functions for portfolio
pub fn get_balance(_env: &Env, _user: &Address, _token: Symbol) -> Option<u128> {
    Some(1000)
}

pub fn deposit(_env: &Env, _user: &Address, _token: Symbol, _amount: u128) -> SorobanResult<()> {
    if _amount == 0 {
        return Err(ContractError::ZeroAmountSwap);
    }
    Ok(())
}

pub fn withdraw(_env: &Env, _user: &Address, _token: Symbol, amount: u128) -> SorobanResult<()> {
    let balance = get_balance(_env, _user, _token).ok_or(ContractError::InsufficientBalance)?;
    if balance < amount {
        return Err(ContractError::InsufficientBalance);
    }
    Ok(())
}
