#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, Address, Env};
use soroban_sdk::testutils::{Address as _, Ledger as _};

const PRECISION: u128 = 1_000_000_000_000_000_000;

#[test]
fn test_oracle_set_and_get() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");
    let pair = (xlm.clone(), usdc.clone());

    // 1 XLM = 0.5 USDC (fixed point)
    let price = 500_000_000_000_000_000; // 0.5 * 10^18
    client.set_price(&pair, &price);

    let stored_price = client.get_current_price(&pair);
    assert_eq!(stored_price, price);
}

#[test]
fn test_slippage_calculation() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Set Price 1:1
    let price = PRECISION;
    client.set_price(&(xlm.clone(), usdc.clone()), &price);

    // Mint XLM to user
    client.mint(&xlm, &user, &1000);

    // Set Pool Liquidity for USDC (Target Token)
    // If pool has 1000 USDC.
    // Swap 100 XLM.
    // Theoretical out = 100 * 1.0 = 100 USDC.
    // Impact = 100 / 1000 = 10%.
    // Slippage = 100 * 10% = 10 USDC.
    // Actual out = 90 USDC.
    
    client.set_pool_liquidity(&usdc, &1000);
    
    // Perform Swap
    let out = client.swap(&xlm, &usdc, &100, &user);
    
    assert_eq!(out, 90);
}

#[test]
#[should_panic(expected = "Slippage exceeded")]
fn test_max_slippage_enforcement() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    client.set_price(&(xlm.clone(), usdc.clone()), &PRECISION);
    client.mint(&xlm, &user, &1000);
    client.set_pool_liquidity(&usdc, &1000);
    
    // Set Max Slippage to 5% (500 bps)
    client.set_max_slippage_bps(&500);
    
    // Swap 100 XLM -> 10% slippage -> Should Fail
    client.swap(&xlm, &usdc, &100, &user);
}

#[test]
#[should_panic(expected = "Oracle price is stale")]
fn test_stale_price() {
    let env = Env::default();
    
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");
    
    // Set price at t=0
    env.ledger().set_timestamp(0);
    client.set_price(&(xlm.clone(), usdc.clone()), &PRECISION);
    
    // Advance time beyond threshold (600s)
    env.ledger().set_timestamp(601);
    
    let user = Address::generate(&env);
    client.mint(&xlm, &user, &100);
    
    // Swap should fail due to stale price
    client.swap(&xlm, &usdc, &10, &user);
}

#[test]
fn test_price_impact_on_pool() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    client.set_price(&(xlm.clone(), usdc.clone()), &PRECISION);
    client.mint(&xlm, &user, &2000);
    
    // Reset pool
    client.set_pool_liquidity(&usdc, &1000);
    
    // Swap 1: 200 XLM -> 160 USDC (20% slippage)
    // Impact = 200/1000 = 20%. Slip = 40. Out = 160.
    let out_a = client.swap(&xlm, &usdc, &200, &user);
    assert_eq!(out_a, 160);
    
    // Pool USDC remaining: 1000 - 160 = 840.
    
    // Swap 2: 200 XLM.
    // Impact = 200/840 = 23.8% -> 2380 bps.
    // Theoretical = 200.
    // Slip = 200 * 0.238 = 47.6 -> 47.
    // Out = 200 - 47 = 153.
    let out_b = client.swap(&xlm, &usdc, &200, &user);
    assert_eq!(out_b, 153); // Confirms slippage increases as pool depletes
}
