use soroban_sdk::{Address, Symbol, symbol_short};
use crate::errors::ContractError;

/// Safe upper bound (prevents overflow abuse)
const MAX_AMOUNT: i128 = 1_000_000_000_000_000_000;

/// Supported tokens (project-specific)
const XLM: Symbol = symbol_short!("XLM");
const USDC_SIM: Symbol = symbol_short!("USDC-SIM");

pub fn validate_amount(amount: i128) -> Result<(), ContractError> {
    if amount <= 0 {
        return Err(ContractError::InvalidAmount);
    }

    if amount > MAX_AMOUNT {
        return Err(ContractError::AmountOverflow);
    }

    Ok(())
}

pub fn validate_token_symbol(token: Symbol) -> Result<(), ContractError> {
    if token != XLM && token != USDC_SIM {
        return Err(ContractError::InvalidTokenSymbol);
    }
    Ok(())
}

pub fn validate_user_address(address: &Address) -> Result<(), ContractError> {
    let zero = Address::from_contract_id([0; 32]);
    if address == &zero {
        return Err(ContractError::InvalidUserAddress);
    }
    Ok(())
}

pub fn validate_swap_pair(from: Symbol, to: Symbol) -> Result<(), ContractError> {
    if from == to {
        return Err(ContractError::InvalidSwapPair);
    }

    validate_token_symbol(from)?;
    validate_token_symbol(to)?;

    Ok(())
}
