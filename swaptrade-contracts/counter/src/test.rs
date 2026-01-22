#![cfg(test)]

use super::*;
use soroban_sdk::{Env, testutils::Address as _, Address, symbol_short, Symbol};

#[test]
fn test_swap() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    
    // 1. Mint 1000 XLM to user
    client.mint(&symbol_short!("XLM"), &user, &1000);
    
    assert_eq!(client.balance_of(&symbol_short!("XLM"), &user), 1000);
    
    // 2. Set Price XLM -> USDC-SIM
    // 1 XLM = 0.5 USDC
    // Price in oracle is u128 fixed point 18 decimals
    // 0.5 * 1e18 = 500_000_000_000_000_000
    let usdc_sim = Symbol::new(&env, "USDCSIM");
    let price: u128 = 500_000_000_000_000_000;
    let liquidity: u128 = 1_000_000_000_000_000_000_000; // lots of liquidity
    
    client.set_price(&symbol_short!("XLM"), &usdc_sim, &price, &liquidity);
    
    // 3. Swap 100 XLM for USDC-SIM
    // Expected output: 100 * 0.5 = 50 USDC-SIM
    let amount_in = 100;
    let amount_out = client.swap(&symbol_short!("XLM"), &usdc_sim, &amount_in, &user);
    
    assert_eq!(amount_out, 50);
    
    // 4. Check balances
    // XLM: 1000 - 100 = 900
    // USDC-SIM: 0 + 50 = 50
    assert_eq!(client.balance_of(&symbol_short!("XLM"), &user), 900);
    assert_eq!(client.balance_of(&usdc_sim, &user), 50);
    
    // 5. Check portfolio stats
    // Trade count should be 1
    let (trades, _pnl) = client.get_portfolio(&user);
    assert_eq!(trades, 1);
}
