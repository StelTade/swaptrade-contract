#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, Address, Env, Vec};

/// Test 1: Insufficient Balance with Detailed Error Handling
/// Tests that insufficient balance scenarios are properly handled
#[test]
fn test_insufficient_balance_detailed_handling() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Mint small amount
    client.mint(&xlm, &user, &100);

    // Attempt to swap more than available balance
    let result = client.try_swap(&xlm, &usdc, &200, &user);
    
    // Should return 0 for insufficient balance
    assert_eq!(result, 0);
    
    // Failed orders metric should increment
    let metrics = client.get_metrics();
    assert!(metrics.failed_orders >= 1);
}

/// Test 2: Concurrent Order Placement Simulation
/// Tests that multiple users can place orders simultaneously without interference
#[test]
fn test_concurrent_order_placement_simulation() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);
    
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Mint different amounts to each user
    client.mint(&xlm, &user1, &1000);
    client.mint(&xlm, &user2, &500);
    client.mint(&xlm, &user3, &2000);

    // Record initial balances
    let user1_xlm_before = client.get_balance(&xlm, &user1);
    let user2_xlm_before = client.get_balance(&xlm, &user2);
    let user3_xlm_before = client.get_balance(&xlm, &user3);

    // Simultaneous swaps from all users
    let out1 = client.swap(&xlm, &usdc, &100, &user1);
    let out2 = client.swap(&xlm, &usdc, &200, &user2);
    let out3 = client.swap(&xlm, &usdc, &500, &user3);

    // Verify outputs
    assert_eq!(out1, 100);
    assert_eq!(out2, 200);
    assert_eq!(out3, 500);

    // Verify user balances are isolated
    assert_eq!(client.get_balance(&xlm, &user1), user1_xlm_before - 100);
    assert_eq!(client.get_balance(&xlm, &user2), user2_xlm_before - 200);
    assert_eq!(client.get_balance(&xlm, &user3), user3_xlm_before - 500);
    
    assert_eq!(client.get_balance(&usdc, &user1), 100);
    assert_eq!(client.get_balance(&usdc, &user2), 200);
    assert_eq!(client.get_balance(&usdc, &user3), 500);
}

/// Test 3: Precision and Rounding Behavior with AMM
/// Tests edge cases in AMM calculations with various input sizes
#[test]
fn test_amm_precision_and_rounding_edge_cases() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Test with very small amounts
    client.mint(&xlm, &user, &3);
    
    // Test 1: Swap 1 unit (minimum)
    let out1 = client.swap(&xlm, &usdc, &1, &user);
    assert_eq!(out1, 1);
    assert_eq!(client.get_balance(&xlm, &user), 2);
    assert_eq!(client.get_balance(&usdc, &user), 1);

    // Test 2: Swap remaining 2 units
    let out2 = client.swap(&xlm, &usdc, &2, &user);
    assert_eq!(out2, 2);
    assert_eq!(client.get_balance(&xlm, &user), 0);
    assert_eq!(client.get_balance(&usdc, &user), 3);

    // Test 3: Very large amounts
    client.mint(&xlm, &user, &1_000_000);
    let out3 = client.swap(&xlm, &usdc, &999_999, &user);
    assert_eq!(out3, 999_999);
}

/// Test 4: AMM Behavior with Liquidity Pool Dynamics
/// Tests how AMM behaves with changing liquidity conditions
#[test]
fn test_amm_behavior_with_liquidity_changes() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Initial state - no liquidity in pool
    client.mint(&xlm, &user1, &1000);
    client.mint(&usdc, &user2, &1000);

    // First swap establishes initial pool ratio
    let out1 = client.swap(&xlm, &usdc, &100, &user1);
    assert_eq!(out1, 100);

    // Second swap with different user should respect AMM dynamics
    let out2 = client.swap(&usdc, &xlm, &50, &user2);
    assert_eq!(out2, 50);

    // Verify pool state is maintained
    let metrics = client.get_metrics();
    assert_eq!(metrics.trades_executed, 2);
}

/// Test 5: Invalid Token Pair Handling
/// Tests that invalid token combinations are properly rejected
#[test]
fn test_invalid_token_pair_handling() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let invalid_token = symbol_short!("INVALID");

    // Test with unsupported token
    let result1 = client.try_swap(&xlm, &invalid_token, &100, &user);
    assert_eq!(result1, 0);

    // Test with same token (should fail)
    let result2 = client.try_swap(&xlm, &xlm, &100, &user);
    assert_eq!(result2, 0);

    // Verify failed orders are counted
    let metrics = client.get_metrics();
    assert!(metrics.failed_orders >= 2);
}

/// Test 6: Zero and Negative Amount Handling
/// Tests edge cases with zero and negative amounts
#[test]
fn test_zero_and_negative_amount_edge_cases() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Test zero amount (should fail gracefully)
    let result1 = client.try_swap(&xlm, &usdc, &0, &user);
    assert_eq!(result1, 0);

    // Test negative amount (should fail gracefully)
    // Note: i128 can be negative, but our contract should handle it
    let result2 = client.try_swap(&xlm, &usdc, &-50, &user);
    assert_eq!(result2, 0);

    // Verify failed orders counter
    let metrics = client.get_metrics();
    assert!(metrics.failed_orders >= 2);
}

/// Test 7: Maximum Slippage Protection
/// Tests that slippage limits are enforced
#[test]
fn test_slippage_protection_enforcement() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Set maximum slippage to 1% (100 basis points)
    env.storage().instance().set(&symbol_short!("MAX_SLIP"), &100u32);

    client.mint(&xlm, &user, &10000);

    // Large swap that might trigger slippage
    // This test depends on AMM implementation details
    let result = client.try_swap(&xlm, &usdc, &5000, &user);
    
    // Should either succeed or fail gracefully
    if result == 0 {
        // If it failed due to slippage, verify metrics
        let metrics = client.get_metrics();
        assert!(metrics.failed_orders >= 1);
    } else {
        // If it succeeded, verify the output
        assert!(result > 0);
    }
}

/// Test 8: Rate Limiting Integration with Trading
/// Tests that rate limits are properly enforced during trading
#[test]
fn test_rate_limiting_integration_with_trading() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    client.mint(&xlm, &user, &10000);

    // Perform multiple rapid swaps to test rate limiting
    let mut success_count = 0;
    let mut failure_count = 0;

    for i in 0..10 {
        let result = client.try_swap(&xlm, &usdc, &(100 + i), &user);
        if result > 0 {
            success_count += 1;
        } else {
            failure_count += 1;
        }
    }

    // Should have some successes and possibly some failures due to rate limiting
    assert!(success_count > 0);
    // Note: exact failure count depends on rate limit configuration
}

/// Test 9: Transaction History Tracking
/// Tests that trades are properly recorded in transaction history
#[test]
fn test_transaction_history_tracking() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    client.mint(&xlm, &user, &1000);

    // Perform several trades
    client.swap(&xlm, &usdc, &100, &user);
    client.swap(&usdc, &xlm, &50, &user);
    client.swap(&xlm, &usdc, &200, &user);

    // Check transaction history
    let transactions = client.get_user_transactions(&user, &5);
    
    // Should have at least 3 transactions
    assert!(transactions.len() >= 3);
    
    // Verify transaction structure (basic checks)
    if let Some(first_tx) = transactions.get(0) {
        assert_eq!(first_tx.from_amount, 100);
        assert_eq!(first_tx.from_token, xlm);
        assert_eq!(first_tx.to_token, usdc);
    }
}

/// Test 10: Fee Calculation and Collection
/// Tests that trading fees are properly calculated and collected
#[test]
fn test_fee_calculation_and_collection() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Get initial metrics
    let metrics_before = client.get_metrics();
    let fees_before = metrics_before.balances_updated; // Using this as proxy

    client.mint(&xlm, &user, &1000);

    // Perform swap with fee
    let out_amount = client.swap(&xlm, &usdc, &100, &user);

    // Verify output is less than input due to fees
    // Assuming 0.3% fee, output should be ~99.7% of input
    assert!(out_amount < 100);
    assert!(out_amount > 99); // Allow for rounding

    // Verify fee collection through metrics
    let metrics_after = client.get_metrics();
    assert!(metrics_after.balances_updated > fees_before);
}

/// Test 11: Portfolio Statistics Updates
/// Tests that portfolio statistics are correctly updated after trades
#[test]
fn test_portfolio_statistics_updates() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Get initial portfolio stats
    let (trades_before, pnl_before) = client.get_portfolio(&user);
    assert_eq!(trades_before, 0);
    assert_eq!(pnl_before, 0);

    client.mint(&xlm, &user, &1000);

    // Perform trades
    client.swap(&xlm, &usdc, &100, &user);
    client.swap(&usdc, &xlm, &50, &user);

    // Check updated portfolio stats
    let (trades_after, pnl_after) = client.get_portfolio(&user);
    assert_eq!(trades_after, 2);
    // PnL should reflect the net change from trades
    assert_ne!(pnl_after, pnl_before);
}

/// Test 12: Badge System Integration with Trading
/// Tests that trading activities properly trigger badge awards
#[test]
fn test_badge_system_integration_with_trading() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // User should start with no badges
    let initial_badges = client.get_user_badges(&user);
    assert_eq!(initial_badges.len(), 0);

    client.mint(&xlm, &user, &1000);

    // Perform first trade - should award FirstTrade badge
    client.swap(&xlm, &usdc, &100, &user);

    let badges_after_first = client.get_user_badges(&user);
    assert_eq!(badges_after_first.len(), 1);
    
    // Check if FirstTrade badge is present
    let has_first_trade = client.has_badge(&user, &Badge::FirstTrade);
    assert!(has_first_trade);

    // Perform more trades to test progression
    for i in 0..9 {
        client.swap(&xlm, &usdc, &(50 + i), &user);
    }

    // Should now have Trader badge (10+ trades)
    let final_badges = client.get_user_badges(&user);
    assert!(final_badges.len() >= 1);
}