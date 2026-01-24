#![cfg(test)]

use crate::portfolio::{Portfolio, Asset, Transaction};
use soroban_sdk::{Env, Symbol, symbol_short, testutils::{Address as _, Ledger}};

#[test]
fn test_record_transaction_stores_data_correctly() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = soroban_sdk::Address::generate(&env);
    
    // Set timestamp
    env.ledger().set_timestamp(1234567890);
    
    let from_token = symbol_short!("XLM");
    let to_token = symbol_short!("USDC");
    let amount_in = 100_000_000;
    let amount_out = 98_000_000; // 0.98 rate
    
    portfolio.record_transaction(
        &env,
        user.clone(),
        from_token.clone(),
        to_token.clone(),
        amount_in,
        amount_out
    );
    
    let transactions = portfolio.get_user_transactions(&env, user.clone(), 10);
    assert_eq!(transactions.len(), 1);
    
    let tx = transactions.get(0).unwrap();
    assert_eq!(tx.timestamp, 1234567890);
    assert_eq!(tx.from_token, from_token);
    assert_eq!(tx.to_token, to_token);
    assert_eq!(tx.from_amount, amount_in);
    assert_eq!(tx.to_amount, amount_out);
    
    // Rate check: (98m * 10m) / 100m = 9,800,000
    assert_eq!(tx.rate_achieved, 9_800_000);
}

#[test]
fn test_transaction_limit_capped_at_100() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = soroban_sdk::Address::generate(&env);
    
    // Record 110 transactions
    for i in 0..110 {
        portfolio.record_transaction(
            &env,
            user.clone(),
            symbol_short!("A"),
            symbol_short!("B"),
            100 + i,
            100 + i
        );
    }
    
    let transactions = portfolio.get_user_transactions(&env, user.clone(), 200);
    
    // Should be capped at 100
    assert_eq!(transactions.len(), 100);
    
    // Should contain the last 100 transactions (10 to 109)
    // The first 10 (0 to 9) should be dropped
    let first_stored = transactions.get(0).unwrap();
    assert_eq!(first_stored.from_amount, 110); // 100 + 10 = 110
    
    let last_stored = transactions.get(99).unwrap();
    assert_eq!(last_stored.from_amount, 209); // 100 + 109 = 209
}

#[test]
fn test_get_user_transactions_limit_works() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = soroban_sdk::Address::generate(&env);
    
    for _ in 0..10 {
        portfolio.record_transaction(
            &env,
            user.clone(),
            symbol_short!("A"),
            symbol_short!("B"),
            100,
            100
        );
    }
    
    let limited = portfolio.get_user_transactions(&env, user.clone(), 5);
    assert_eq!(limited.len(), 5);
}
