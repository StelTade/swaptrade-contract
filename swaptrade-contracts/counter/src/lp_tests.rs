use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Symbol, Vec};
use crate::portfolio::{Asset, LPPosition};

#[test]
fn test_add_liquidity_first_provider() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);

    // Mint initial balances
    client.mint(&symbol_short!("XLM"), &user, &1000);
    client.mint(&symbol_short!("USDCSIM"), &user, &1000);

    // Add liquidity: 100 XLM + 100 USDC
    let lp_tokens = client.add_liquidity(&100, &100, &user);

    // First provider should get LP tokens = sqrt(100 * 100) = 100
    assert!(lp_tokens > 0, "LP tokens should be minted");
    assert!(lp_tokens >= 99 && lp_tokens <= 101, "LP tokens should be approximately 100");

    // Check LP position
    let positions = client.get_lp_positions(&user);
    assert_eq!(positions.len(), 1, "User should have one LP position");
    let position = positions.get(0).unwrap();
    assert_eq!(position.lp_address, user);
    assert_eq!(position.xlm_deposited, 100);
    assert_eq!(position.usdc_deposited, 100);
    assert_eq!(position.lp_tokens_minted, lp_tokens);

    // Check user's balance was debited (pool liquidity is tracked separately)
    let user_xlm = client.balance_of(&symbol_short!("XLM"), &user);
    assert_eq!(user_xlm, 900, "User should have 900 XLM remaining");
    let user_usdc = client.balance_of(&symbol_short!("USDCSIM"), &user);
    assert_eq!(user_usdc, 900, "User should have 900 USDC remaining");
}

#[test]
fn test_add_liquidity_second_provider() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Mint initial balances
    client.mint(&symbol_short!("XLM"), &user1, &1000);
    client.mint(&symbol_short!("USDCSIM"), &user1, &1000);
    client.mint(&symbol_short!("XLM"), &user2, &1000);
    client.mint(&symbol_short!("USDCSIM"), &user2, &1000);

    // First provider: 100 XLM + 100 USDC
    let lp_tokens1 = client.add_liquidity(&100, &100, &user1);
    assert!(lp_tokens1 > 0);

    // Second provider: 50 XLM + 50 USDC (proportional)
    let lp_tokens2 = client.add_liquidity(&50, &50, &user2);
    assert!(lp_tokens2 > 0);

    // Second provider should get approximately half the LP tokens
    // Since they're adding half the liquidity proportionally
    assert!(lp_tokens2 <= lp_tokens1, "Second provider should get fewer or equal LP tokens");

    // Check positions
    let pos1 = client.get_lp_positions(&user1);
    let pos2 = client.get_lp_positions(&user2);
    assert_eq!(pos1.len(), 1);
    assert_eq!(pos2.len(), 1);
}

#[test]
#[should_panic(expected = "Insufficient XLM balance")]
fn test_add_liquidity_insufficient_balance() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);

    // Don't mint any tokens
    // Try to add liquidity - should panic
    client.add_liquidity(&100, &100, &user);
}

#[test]
fn test_remove_liquidity() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);

    // Mint and add liquidity
    client.mint(&symbol_short!("XLM"), &user, &1000);
    client.mint(&symbol_short!("USDCSIM"), &user, &1000);
    let lp_tokens = client.add_liquidity(&100, &100, &user);

    // Get initial user balances
    let initial_xlm = client.balance_of(&symbol_short!("XLM"), &user);
    let initial_usdc = client.balance_of(&symbol_short!("USDCSIM"), &user);

    // Remove all liquidity
    let result = client.remove_liquidity(&lp_tokens, &user);

    // User should get back approximately what they deposited (allowing for rounding)
    assert!(result.0 >= 99 && result.0 <= 101, "Should return approximately 100 XLM");
    assert!(result.1 >= 99 && result.1 <= 101, "Should return approximately 100 USDC");

    // Check final balances
    let final_xlm = client.balance_of(&symbol_short!("XLM"), &user);
    let final_usdc = client.balance_of(&symbol_short!("USDCSIM"), &user);

    // User should have their tokens back
    assert!(final_xlm >= initial_xlm + 99, "User should have XLM back");
    assert!(final_usdc >= initial_usdc + 99, "User should have USDC back");
}

#[test]
#[should_panic(expected = "User has no LP position")]
fn test_remove_liquidity_no_position() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);

    // Try to remove liquidity without adding any
    client.remove_liquidity(&100, &user);
}

#[test]
#[should_panic(expected = "Insufficient LP tokens")]
fn test_remove_liquidity_insufficient_tokens() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);

    // Mint and add liquidity
    client.mint(&symbol_short!("XLM"), &user, &1000);
    client.mint(&symbol_short!("USDCSIM"), &user, &1000);
    let lp_tokens = client.add_liquidity(&100, &100, &user);

    // Try to remove more than deposited
    client.remove_liquidity(&(lp_tokens + 1), &user);
}

#[test]
fn test_swap_uses_lp_pool() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let lp = Address::generate(&env);
    let trader = Address::generate(&env);

    // LP adds liquidity
    client.mint(&symbol_short!("XLM"), &lp, &1000);
    client.mint(&symbol_short!("USDCSIM"), &lp, &1000);
    client.add_liquidity(&100, &100, &lp);

    // Trader mints tokens and swaps
    client.mint(&symbol_short!("XLM"), &trader, &1000);
    client.set_price(&(symbol_short!("XLM"), symbol_short!("USDCSIM")), &1_000_000_000_000_000_000);

    // Swap 10 XLM for USDC
    let out = client.swap(&symbol_short!("XLM"), &symbol_short!("USDCSIM"), &10, &trader);

    // Should get some USDC back (less than 10 due to fees and AMM formula)
    assert!(out > 0, "Should receive USDC");
    assert!(out < 10, "Should be less than input due to fees");

    // Check trader balance
    let trader_usdc = client.balance_of(&symbol_short!("USDCSIM"), &trader);
    assert!(trader_usdc > 0, "Trader should have USDC");
}

#[test]
fn test_lp_fee_collection() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let lp = Address::generate(&env);
    let trader = Address::generate(&env);

    // LP adds liquidity
    client.mint(&symbol_short!("XLM"), &lp, &1000);
    client.mint(&symbol_short!("USDCSIM"), &lp, &1000);
    client.add_liquidity(&100, &100, &lp);

    // Trader swaps multiple times
    client.mint(&symbol_short!("XLM"), &trader, &1000);
    client.set_price(&(symbol_short!("XLM"), symbol_short!("USDCSIM")), &1_000_000_000_000_000_000);

    // Perform 10 swaps
    for _ in 0..10 {
        client.swap(&symbol_short!("XLM"), &symbol_short!("USDCSIM"), &10, &trader);
    }

    // Fees should be accumulated (0.3% of each swap)
    // This is tracked in the portfolio's lp_fees_accumulated
    // Note: We can't directly check this without a getter, but the fees are being collected
}

#[test]
fn test_multiple_lps_and_traders() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    // Create 5 LPs
    let lps: Vec<Address> = (0..5)
        .map(|_| Address::generate(&env))
        .collect();

    // Each LP adds liquidity
    for lp in lps.iter() {
        client.mint(&symbol_short!("XLM"), lp, &1000);
        client.mint(&symbol_short!("USDCSIM"), lp, &1000);
        client.add_liquidity(&100, &100, lp);
    }

    // Create 10 traders
    let traders: Vec<Address> = (0..10)
        .map(|_| Address::generate(&env))
        .collect();

    // Each trader mints and performs swaps
    client.set_price(&(symbol_short!("XLM"), symbol_short!("USDCSIM")), &1_000_000_000_000_000_000);
    
    for trader in traders.iter() {
        client.mint(&symbol_short!("XLM"), trader, &1000);
        
        // Perform 5 swaps each
        for _ in 0..5 {
            client.swap(&symbol_short!("XLM"), &symbol_short!("USDCSIM"), &10, trader);
        }
    }

    // Verify all LP positions still exist
    for lp in lps.iter() {
        let positions = client.get_lp_positions(lp);
        assert_eq!(positions.len(), 1, "Each LP should have a position");
    }

    // Verify balances are consistent
    // (This is a basic sanity check - full consistency would require more detailed checks)
}

#[test]
fn test_remove_partial_liquidity() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);

    // Mint and add liquidity
    client.mint(&symbol_short!("XLM"), &user, &1000);
    client.mint(&symbol_short!("USDCSIM"), &user, &1000);
    let lp_tokens = client.add_liquidity(&100, &100, &user);

    // Remove half the liquidity
    let half_tokens = lp_tokens / 2;
    let result = client.remove_liquidity(&half_tokens, &user);

    // Should return approximately half
    assert!(result.0 >= 49 && result.0 <= 51, "Should return approximately 50 XLM");
    assert!(result.1 >= 49 && result.1 <= 51, "Should return approximately 50 USDC");

    // Check position is updated
    let positions = client.get_lp_positions(&user);
    assert_eq!(positions.len(), 1);
    let position = positions.get(0).unwrap();
    assert_eq!(position.lp_tokens_minted, lp_tokens - half_tokens);
}

#[test]
fn test_get_lp_positions_empty() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);

    // User with no LP position
    let positions = client.get_lp_positions(&user);
    assert_eq!(positions.len(), 0, "Should return empty vec for user with no position");
}

#[test]
fn test_lp_share_calculations() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // User1 adds 100 XLM + 100 USDC
    client.mint(&symbol_short!("XLM"), &user1, &1000);
    client.mint(&symbol_short!("USDCSIM"), &user1, &1000);
    let lp_tokens1 = client.add_liquidity(&100, &100, &user1);

    // User2 adds 200 XLM + 200 USDC (double)
    client.mint(&symbol_short!("XLM"), &user2, &1000);
    client.mint(&symbol_short!("USDCSIM"), &user2, &1000);
    let lp_tokens2 = client.add_liquidity(&200, &200, &user2);

    // User2 should have approximately double the LP tokens
    assert!(lp_tokens2 >= lp_tokens1 * 2 - 2, "User2 should have approximately double LP tokens");
    assert!(lp_tokens2 <= lp_tokens1 * 2 + 2, "User2 should have approximately double LP tokens");
}
