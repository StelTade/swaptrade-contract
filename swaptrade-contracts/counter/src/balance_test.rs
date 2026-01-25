#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, Address, Env};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_get_balance_returns_zero_for_new_user() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let token = symbol_short!("XLM");
    
    // Should return 0 for a user with no balance
    assert_eq!(client.get_balance(&token, &user), 0);
}

#[test]
fn test_get_balance_returns_zero_for_custom_token() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let token = symbol_short!("USDC");
    
    // Should return 0 for a user with no balance for custom token
    assert_eq!(client.get_balance(&token, &user), 0);
}

#[test]
fn test_get_balance_returns_correct_balance_after_mint() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let token = symbol_short!("XLM");
    let amount = 1000;
    
    // Mint some tokens
    client.mint(&token, &user, &amount);
    
    // Should return the minted amount
    assert_eq!(client.get_balance(&token, &user), amount);
}

#[test]
fn test_get_balance_returns_updated_balance_after_multiple_mints() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let token = symbol_short!("XLM");
    
    // First mint
    client.mint(&token, &user, &500);
    assert_eq!(client.get_balance(&token, &user), 500);
    
    // Second mint
    client.mint(&token, &user, &300);
    assert_eq!(client.get_balance(&token, &user), 800);
    
    // Third mint
    client.mint(&token, &user, &200);
    assert_eq!(client.get_balance(&token, &user), 1000);
}

#[test]
fn test_get_balance_works_with_custom_tokens() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let usdc_token = symbol_short!("USDC");
    let btc_token = symbol_short!("BTC");
    
    // Mint different amounts to different tokens
    client.mint(&usdc_token, &user, &1000);
    client.mint(&btc_token, &user, &5);
    
    // Should return correct balances for each token
    assert_eq!(client.get_balance(&usdc_token, &user), 1000);
    assert_eq!(client.get_balance(&btc_token, &user), 5);
}

#[test]
fn test_get_balance_isolates_different_users() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let token = symbol_short!("XLM");
    
    // Mint to user1
    client.mint(&token, &user1, &1000);
    
    // user1 should have balance, user2 should have 0
    assert_eq!(client.get_balance(&token, &user1), 1000);
    assert_eq!(client.get_balance(&token, &user2), 0);
    
    // Mint to user2
    client.mint(&token, &user2, &500);
    
    // Both users should have their respective balances
    assert_eq!(client.get_balance(&token, &user1), 1000);
    assert_eq!(client.get_balance(&token, &user2), 500);
}

#[test]
fn test_get_balance_isolates_different_tokens_for_same_user() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm_token = symbol_short!("XLM");
    let usdc_token = symbol_short!("USDC");
    let btc_token = symbol_short!("BTC");
    
    // Mint different amounts to different tokens for the same user
    client.mint(&xlm_token, &user, &1000);
    client.mint(&usdc_token, &user, &2000);
    // Don't mint BTC
    
    // Should return correct balances for each token
    assert_eq!(client.get_balance(&xlm_token, &user), 1000);
    assert_eq!(client.get_balance(&usdc_token, &user), 2000);
    assert_eq!(client.get_balance(&btc_token, &user), 0);
}

#[test]
fn test_get_balance_consistency_with_balance_of() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let token = symbol_short!("XLM");
    let amount = 1500;
    
    // Mint some tokens
    client.mint(&token, &user, &amount);
    
    // get_balance and balance_of should return the same value
    assert_eq!(client.get_balance(&token, &user), client.balance_of(&token, &user));
    assert_eq!(client.get_balance(&token, &user), amount);
}

#[test]
fn test_get_balance_handles_large_amounts() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let token = symbol_short!("XLM");
    let large_amount = i128::MAX / 2; // Use a large but safe amount
    
    // Mint large amount
    client.mint(&token, &user, &large_amount);
    
    // Should handle large amounts correctly
    assert_eq!(client.get_balance(&token, &user), large_amount);
}

#[test]
fn test_get_balance_handles_zero_mint() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let token = symbol_short!("XLM");
    
    // Mint zero amount (this should work)
    client.mint(&token, &user, &0);
    
    // Should still return 0
    assert_eq!(client.get_balance(&token, &user), 0);
}

#[test]
fn test_get_balance_persistence_across_calls() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let token = symbol_short!("XLM");
    
    // Mint some tokens
    client.mint(&token, &user, &1000);
    
    // Multiple calls should return the same value
    assert_eq!(client.get_balance(&token, &user), 1000);
    assert_eq!(client.get_balance(&token, &user), 1000);
    assert_eq!(client.get_balance(&token, &user), 1000);
}

#[test]
fn test_metrics_increment_on_mint_and_swap() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Mint and check balances
    client.mint(&xlm, &user, &1000);
    assert_eq!(client.get_balance(&xlm, &user), 1000);

    // Swap XLM -> USDCSIM
    let out = client.swap(&xlm, &usdc, &500, &user);
    assert_eq!(out, 500);

    // Check metrics
    let m = client.get_metrics();
    assert_eq!(m.trades_executed, 1);
    assert!(m.balances_updated >= 3); // 1 mint + 2 transfer updates
}

#[test]
fn test_try_swap_counts_failed_orders_without_panic() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Fail: same token pair
    let out_same = client.safe_swap(&xlm, &xlm, &100, &user);
    assert_eq!(out_same, 0);

    // Fail: invalid token
    let btc = symbol_short!("BTC");
    let out_bad_token = client.safe_swap(&xlm, &btc, &100, &user);
    assert_eq!(out_bad_token, 0);

    // Fail: negative amount
    let out_neg = client.safe_swap(&xlm, &usdc, &-10, &user);
    assert_eq!(out_neg, 0);

    // Metrics reflect failed orders
    let m = client.get_metrics();
    assert_eq!(m.failed_orders, 3);
    assert_eq!(m.trades_executed, 0);
}
