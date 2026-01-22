#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, testutils::Address as TestAddress, Env, Symbol, Vec};

// ===== BASIC BATCH OPERATION TESTS =====

/// Test single-leg batch swap works identically to direct swap call
#[test]
fn test_single_leg_batch_identical_to_direct() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Mint tokens for both tests
    client.mint(&xlm, &user, &2000);
    
    // Direct swap
    let direct_result = client.swap(&xlm, &usdc, &500, &user);
    
    // Batch swap with 1 operation
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 500, user.clone()));
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify batch executed successfully
    assert_eq!(batch_result.operations_executed, 1);
    assert_eq!(batch_result.operations_failed, 0);
    
    // Verify results match
    if let Some(OperationResult::Success(amount)) = batch_result.results.get(0) {
        assert_eq!(amount, direct_result);
    } else {
        panic!("Expected success result");
    }
    
    // Verify final balances
    assert_eq!(client.get_balance(&xlm, &user), 1000);
    assert_eq!(client.get_balance(&usdc, &user), 1000);
}

/// Test 3-leg trading strategy in one batch
#[test]
fn test_three_leg_batch_strategy() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Setup: Mint initial tokens
    client.mint(&xlm, &user, &2000);
    
    // Create 3-leg strategy: XLM->USDC, USDC->XLM, XLM->USDC
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 500, user.clone()));
    batch_ops.push_back(BatchOperation::Swap(usdc.clone(), xlm.clone(), 200, user.clone()));
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 300, user.clone()));
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify all operations executed
    assert_eq!(batch_result.operations_executed, 3);
    assert_eq!(batch_result.operations_failed, 0);
    assert_eq!(batch_result.results.len(), 3);
    
    // Verify each operation succeeded
    for i in 0..3 {
        if let Some(result) = batch_result.results.get(i) {
            match result {
                OperationResult::Success(_) => continue,
                OperationResult::Error(_) => panic!("Operation {} failed", i),
            }
        }
    }
    
    // Final balances: started with 2000 XLM
    // Swap 500 XLM -> 500 USDC (1500 XLM, 500 USDC)
    // Swap 200 USDC -> 200 XLM (1700 XLM, 300 USDC)
    // Swap 300 XLM -> 300 USDC (1400 XLM, 600 USDC)
    assert_eq!(client.get_balance(&xlm, &user), 1400);
    assert_eq!(client.get_balance(&usdc, &user), 600);
}

/// Test batch with AddLiquidity and Swap operations
#[test]
fn test_batch_with_add_liquidity_and_swap() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Setup: Mint initial tokens
    client.mint(&xlm, &user, &1000);
    client.mint(&usdc, &user, &1000);
    
    // Create batch: Mint more, Add liquidity, Swap
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::MintToken(xlm.clone(), user.clone(), 500));
    batch_ops.push_back(BatchOperation::AddLiquidity(300, 300, user.clone()));
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 200, user.clone()));
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify all operations executed
    assert_eq!(batch_result.operations_executed, 3);
    assert_eq!(batch_result.operations_failed, 0);
}

/// Test batch with RemoveLiquidity operations
#[test]
fn test_batch_with_remove_liquidity() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Setup: Mint initial tokens and add liquidity
    client.mint(&xlm, &user, &1000);
    client.mint(&usdc, &user, &1000);
    
    let mut add_liq_ops = Vec::new(&env);
    add_liq_ops.push_back(BatchOperation::AddLiquidity(500, 500, user.clone()));
    client.execute_batch(&add_liq_ops);
    
    // Create batch: Swap, then remove liquidity
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 200, user.clone()));
    batch_ops.push_back(BatchOperation::RemoveLiquidity(100, 100, user.clone()));
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify operations executed
    assert_eq!(batch_result.operations_executed, 2);
    assert_eq!(batch_result.operations_failed, 0);
}

// ===== ATOMICITY & ROLLBACK TESTS =====

/// Test partial batch failure rolls back earlier operations (atomic mode)
#[test]
fn test_atomic_batch_rollback_on_failure() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Setup: Mint only 500 XLM
    client.mint(&xlm, &user, &500);
    
    let initial_xlm = client.get_balance(&xlm, &user);
    let initial_usdc = client.get_balance(&usdc, &user);
    
    // Create batch with operations that will fail on the second one
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 200, user.clone())); // Should succeed
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 500, user.clone())); // Should fail (insufficient after first)
    
    let batch_result = client.execute_batch_atomic(&batch_ops);
    
    // Verify batch failed
    assert!(batch_result.operations_failed > 0);
    
    // Verify rollback: balances should be unchanged
    assert_eq!(client.get_balance(&xlm, &user), initial_xlm);
    assert_eq!(client.get_balance(&usdc, &user), initial_usdc);
}

/// Test best-effort mode continues on failure
#[test]
fn test_best_effort_continues_on_failure() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Setup: Mint tokens
    client.mint(&xlm, &user, &1000);
    
    // Create batch with mixed valid/invalid operations
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 200, user.clone())); // Valid
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 5000, user.clone())); // Invalid - insufficient
    batch_ops.push_back(BatchOperation::Swap(usdc.clone(), xlm.clone(), 100, user.clone())); // Valid
    
    let batch_result = client.execute_batch_best_effort(&batch_ops);
    
    // Verify mixed results
    assert_eq!(batch_result.results.len(), 3);
    assert!(batch_result.operations_executed > 0); // At least some succeeded
    assert!(batch_result.operations_failed > 0); // At least one failed
    
    // Verify first operation succeeded
    if let Some(OperationResult::Success(_)) = batch_result.results.get(0) {
        // Good
    } else {
        panic!("First operation should succeed");
    }
    
    // Verify second operation failed
    if let Some(OperationResult::Error(_)) = batch_result.results.get(1) {
        // Good
    } else {
        panic!("Second operation should fail");
    }
}

/// Test atomicity with complex 3-operation batch where operation 2 fails
#[test]
fn test_atomicity_three_operations_middle_fails() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Setup: Mint limited tokens
    client.mint(&xlm, &user, &400);
    
    let initial_xlm = client.get_balance(&xlm, &user);
    
    // Create batch: op1 succeeds, op2 fails, op3 would succeed
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 100, user.clone())); // OK
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 500, user.clone())); // FAIL - insufficient
    batch_ops.push_back(BatchOperation::Swap(usdc.clone(), xlm.clone(), 50, user.clone()));  // Would be OK
    
    let batch_result = client.execute_batch_atomic(&batch_ops);
    
    // Verify entire batch rolled back
    assert!(batch_result.operations_failed > 0);
    assert_eq!(client.get_balance(&xlm, &user), initial_xlm); // Unchanged
}

// ===== VALIDATION TESTS =====

/// Test validation catches invalid operations before execution
#[test]
fn test_validation_catches_invalid_amount() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Create batch with negative amount (invalid)
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm, usdc, -100, user.clone()));
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify batch failed validation
    assert!(batch_result.operations_failed > 0);
}

/// Test validation catches same-token swap
#[test]
fn test_validation_catches_same_token_swap() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    
    // Create batch with same-token swap (invalid)
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), xlm.clone(), 100, user.clone()));
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify batch failed validation
    assert!(batch_result.operations_failed > 0);
}

/// Test batch size limit enforcement
#[test]
fn test_batch_size_limit_enforced() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Create batch with 11 operations (exceeds MAX_BATCH_SIZE of 10)
    let mut batch_ops = Vec::new(&env);
    for _ in 0..11 {
        batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 10, user.clone()));
    }
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify batch failed due to size limit
    assert!(batch_result.operations_failed > 0);
}

/// Test empty batch is rejected
#[test]
fn test_empty_batch_rejected() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    // Create empty batch
    let batch_ops = Vec::new(&env);
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify batch failed validation
    assert!(batch_result.operations_failed > 0);
}

// ===== PERFORMANCE & INTEGRATION TESTS =====

/// Test complex strategy in one batch call
#[test]
fn test_complex_multi_operation_strategy() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Setup: Mint initial capital
    client.mint(&xlm, &user, &2000);
    client.mint(&usdc, &user, &2000);
    
    // Complex strategy: mint, add liquidity, multiple swaps, remove liquidity
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::MintToken(xlm.clone(), user.clone(), 500));
    batch_ops.push_back(BatchOperation::AddLiquidity(400, 400, user.clone()));
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 300, user.clone()));
    batch_ops.push_back(BatchOperation::Swap(usdc.clone(), xlm.clone(), 200, user.clone()));
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 100, user.clone()));
    batch_ops.push_back(BatchOperation::RemoveLiquidity(200, 200, user.clone()));
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify all operations executed successfully
    assert_eq!(batch_result.operations_executed, 6);
    assert_eq!(batch_result.operations_failed, 0);
    assert_eq!(batch_result.results.len(), 6);
}

/// Test batch execution updates portfolio stats correctly
#[test]
fn test_batch_updates_portfolio_stats() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Setup
    client.mint(&xlm, &user, &1000);
    
    let (initial_trades, _) = client.get_portfolio(&user);
    
    // Execute batch with 3 swaps
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 100, user.clone()));
    batch_ops.push_back(BatchOperation::Swap(usdc.clone(), xlm.clone(), 50, user.clone()));
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 50, user.clone()));
    
    client.execute_batch(&batch_ops);
    
    // Verify trade count increased
    let (final_trades, _) = client.get_portfolio(&user);
    assert_eq!(final_trades, initial_trades + 3);
}

/// Test batch with multiple users (isolation)
#[test]
fn test_batch_multi_user_isolation() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Setup
    client.mint(&xlm, &user1, &1000);
    client.mint(&xlm, &user2, &1000);
    
    // Create batch with operations for different users
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 200, user1.clone()));
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 300, user2.clone()));
    
    let batch_result = client.execute_batch(&batch_ops);
    
    // Verify both operations succeeded
    assert_eq!(batch_result.operations_executed, 2);
    assert_eq!(batch_result.operations_failed, 0);
    
    // Verify user balances are isolated
    assert_eq!(client.get_balance(&xlm, &user1), 800);
    assert_eq!(client.get_balance(&usdc, &user1), 200);
    
    assert_eq!(client.get_balance(&xlm, &user2), 700);
    assert_eq!(client.get_balance(&usdc, &user2), 300);
}

/// Test error messages are clear for each failed operation
#[test]
fn test_clear_error_messages() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);
    
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC-SIM");
    
    // Create batch with various invalid operations
    let mut batch_ops = Vec::new(&env);
    batch_ops.push_back(BatchOperation::Swap(xlm.clone(), xlm.clone(), 100, user.clone())); // Same token
    
    let batch_result = client.execute_batch_best_effort(&batch_ops);
    
    // Verify error result is returned
    assert!(batch_result.operations_failed > 0);
    if let Some(OperationResult::Error(err_sym)) = batch_result.results.get(0) {
        // Error symbol should be meaningful
        assert!(!err_sym.to_string().is_empty());
    }
}
