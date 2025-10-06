#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, Address, Env, Symbol};

// 1) Happy path: simple swap XLM -> USDC-SIM
#[test]
fn test_swap_happy_path() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");

    // Mint XLM and perform swap
    client.mint(&xlm, &user, &1000);
    let out = client.swap(&xlm, &usdc, &500, &user);
    assert_eq!(out, 500);

    // Balances updated
    assert_eq!(client.get_balance(&xlm, &user), 500);
    assert_eq!(client.get_balance(&usdc, &user), 500);
}

// 2) Edge: insufficient balance should panic in swap (perform_swap uses assert)
#[test]
#[should_panic(expected = "Insufficient funds")]
fn test_swap_insufficient_balance_panics() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");

    // No minting, attempt to swap should panic due to insufficient funds
    client.swap(&xlm, &usdc, &100, &user);
}

// 3) try_swap should not panic and should count failed orders
#[test]
fn test_try_swap_handles_invalid_inputs_and_counts_failed() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");

    // invalid pair (same token) -> returns 0
    let out = client.try_swap(&xlm, &xlm, &100, &user);
    assert_eq!(out, 0);

    // negative amount -> returns 0
    let usdc = symbol_short!("USDC-SIM");
    let out2 = client.try_swap(&xlm, &usdc, &-10, &user);
    assert_eq!(out2, 0);

    // metrics reflect failed orders
    let m = client.get_metrics();
    assert!(m.failed_orders >= 2);
}

// 4) Rounding / precision: ensure integer arithmetic truncates as expected
#[test]
fn test_swap_precision_truncation() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");

    // Mint a small amount and swap
    client.mint(&xlm, &user, &3); // small odd amount
    let out = client.swap(&xlm, &usdc, &1, &user);
    assert_eq!(out, 1);

    // After swapping 1, remaining xlm should be 2, usdc 1
    assert_eq!(client.get_balance(&xlm, &user), 2);
    assert_eq!(client.get_balance(&usdc, &user), 1);
}

// 5) AMM round trip: swap XLM->USDC and back -> end balances equal original
#[test]
fn test_amm_round_trip_identity() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");

    client.mint(&xlm, &user, &1000);
    let out1 = client.swap(&xlm, &usdc, &250, &user);
    assert_eq!(out1, 250);

    let out2 = client.swap(&usdc, &xlm, &250, &user);
    assert_eq!(out2, 250);

    // Balances return to original
    assert_eq!(client.get_balance(&xlm, &user), 1000);
    assert_eq!(client.get_balance(&usdc, &user), 0);
}

// 6) Simulated concurrent placements: sequentially perform swaps from two users to ensure isolation
#[test]
fn test_concurrent_like_swaps_isolation() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");

    client.mint(&xlm, &user1, &500);
    client.mint(&xlm, &user2, &300);

    // User1 swaps 200
    let u1_out = client.swap(&xlm, &usdc, &200, &user1);
    assert_eq!(u1_out, 200);

    // User2 swaps 300
    let u2_out = client.swap(&xlm, &usdc, &300, &user2);
    assert_eq!(u2_out, 300);

    // Ensure balances are isolated and correct
    assert_eq!(client.get_balance(&xlm, &user1), 300);
    assert_eq!(client.get_balance(&usdc, &user1), 200);

    assert_eq!(client.get_balance(&xlm, &user2), 0);
    assert_eq!(client.get_balance(&usdc, &user2), 300);
}

// 7) Edge: zero amount swap should panic due to assert in perform_swap
#[test]
#[should_panic(expected = "Amount must be positive")]
fn test_swap_zero_amount_panics() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");

    client.mint(&xlm, &user, &100);
    client.swap(&xlm, &usdc, &0, &user);
}
