// counter/tests/error_tests.rs
use soroban_sdk::Env;
use counter::{swap_tokens, get_balance, deposit, withdraw, errors::ContractError};
use soroban_sdk::Symbol;

#[test]
fn test_zero_amount_swap() {
    let env = Env::default();
    let user = env.accounts().generate();

    let result = swap_tokens(&env, &user, Symbol::from_str("USD"), Symbol::from_str("BTC"), 0);
    assert_eq!(result, Err(ContractError::ZeroAmountSwap));
}

#[test]
fn test_invalid_token() {
    let env = Env::default();
    let user = env.accounts().generate();

    let result = swap_tokens(&env, &user, Symbol::from_str("INVALID"), Symbol::from_str("BTC"), 100);
    assert_eq!(result, Err(ContractError::InvalidTokenSymbol));
}

#[test]
fn test_insufficient_balance() {
    let env = Env::default();
    let user = env.accounts().generate();

    // Simulate withdraw more than balance
    let result = withdraw(&env, &user, Symbol::from_str("USD"), 2000);
    assert_eq!(result, Err(ContractError::InsufficientBalance));
}

#[test]
fn test_invalid_swap_pair() {
    let env = Env::default();
    let user = env.accounts().generate();

    let result = swap_tokens(&env, &user, Symbol::from_str("USD"), Symbol::from_str("ETH"), 100);
    assert_eq!(result, Err(ContractError::InvalidSwapPair));
}
