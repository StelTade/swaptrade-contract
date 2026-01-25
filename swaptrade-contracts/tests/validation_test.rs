use crate::validation::*;
use soroban_sdk::{Env, Address, symbol_short};

#[test]
fn rejects_zero_amount() {
    assert!(validate_amount(0).is_err());
}

#[test]
fn rejects_negative_amount() {
    assert!(validate_amount(-10).is_err());
}

#[test]
fn accepts_valid_tokens() {
    assert!(validate_token_symbol(symbol_short!("XLM")).is_ok());
}

#[test]
fn rejects_same_token_swap() {
    assert!(validate_swap_pair(
        symbol_short!("XLM"),
        symbol_short!("XLM")
    ).is_err());
}

#[test]
fn accepts_valid_user_address() {
    let env = Env::default();
    let user = Address::generate(&env);
    assert!(validate_user_address(&user).is_ok());
}
