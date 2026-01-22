#![cfg(test)]

use super::*;
use soroban_sdk::{Env, Address, symbol_short, testutils::Address as _};
use crate::oracle::{PRECISION};

#[test]
fn test_set_and_get_price() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let xlm = symbol_short!("XLM");
    let usdc = Symbol::new(&env, "USDCSIM");

    // Price 1:1, Liquidity 1000
    let price = PRECISION; // 1.0 * 1e18
    let liquidity = 1000;
    
    client.set_price(&xlm, &usdc, &price, &liquidity);
    
    let stored_price = client.get_current_price(&xlm, &usdc);
    assert_eq!(stored_price, price);
}

#[test]
fn test_swap_slippage() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc_sim = Symbol::new(&env, "USDCSIM");

    // Mint XLM to user
    client.mint(&xlm, &user, &1000);
    
    // Set price 1:1, Liquidity 1000 (of output token)
    let price = PRECISION;
    let liquidity = 1000;
    client.set_price(&xlm, &usdc_sim, &price, &liquidity);
    
    // Swap 100 XLM
    // Theoretical output: 100 * 1 = 100.
    // Impact: 100 / 1000 = 0.1 (10%).
    // Slippage: 100 * (1 - 0.1) = 90.
    
    let received = client.swap(&xlm, &usdc_sim, &100, &user);
    assert_eq!(received, 90);
    
    // Check balances
    assert_eq!(client.get_balance(&xlm, &user), 900);
    assert_eq!(client.get_balance(&usdc_sim, &user), 90);
}

#[test]
#[should_panic(expected = "Slippage exceeded")]
fn test_max_slippage_exceeded() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc_sim = Symbol::new(&env, "USDCSIM");

    client.mint(&xlm, &user, &1000);
    
    // Set price 1:1, Liquidity 1000
    let price = PRECISION;
    let liquidity = 1000;
    client.set_price(&xlm, &usdc_sim, &price, &liquidity);
    
    // Set max slippage to 5% (500 bps)
    client.set_max_slippage(&500);
    
    // Swap 100 XLM (10% impact)
    client.swap(&xlm, &usdc_sim, &100, &user);
}

#[test]
fn test_price_fluctuation() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc_sim = Symbol::new(&env, "USDCSIM");

    client.mint(&xlm, &user, &2000);
    
    // Scenario 1: Price 1:1, Liquidity 10000
    // Swap 100. Impact 100/10000 = 1%.
    // Output 100 * 0.99 = 99.
    client.set_price(&xlm, &usdc_sim, &PRECISION, &10000);
    let received1 = client.swap(&xlm, &usdc_sim, &100, &user);
    assert_eq!(received1, 99);
    
    // Scenario 2: Price drops to 0.5 USDC/XLM. Liquidity 10000.
    // Swap 100 XLM.
    // Theoretical: 100 * 0.5 = 50.
    // Impact: 50 / 10000 = 0.5%. (50 bps)
    // Output: 50 * (1 - 0.005) = 50 * 0.995 = 49.75 -> 49 (integer math).
    
    let price_half = PRECISION / 2;
    client.set_price(&xlm, &usdc_sim, &price_half, &10000);
    
    let received2 = client.swap(&xlm, &usdc_sim, &100, &user);
    // 50 * 9950 / 10000 = 497500 / 10000 = 49.
    assert_eq!(received2, 49);
}
