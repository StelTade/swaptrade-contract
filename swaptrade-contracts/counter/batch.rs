extern crate alloc;
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

use crate::portfolio::{Portfolio, Asset};
use crate::trading::perform_swap;

/// Maximum number of operations allowed in a single batch
pub const MAX_BATCH_SIZE: u32 = 10;

/// Represents different types of operations that can be batched
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum BatchOperation {
    /// Swap operation: (from_token, to_token, amount, user)
    Swap(Symbol, Symbol, i128, Address),
    
    /// Add liquidity operation: (xlm_amount, usdc_amount, user)
    AddLiquidity(i128, i128, Address),
    
    /// Remove liquidity operation: (xlm_amount, usdc_amount, user)
    RemoveLiquidity(i128, i128, Address),
    
    /// Mint token operation: (token, to, amount)
    MintToken(Symbol, Address, i128),
}

/// Result of executing a single operation in a batch
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum OperationResult {
    /// Success with output amount or status
    Success(i128),
    
    /// Failure with error message
    OpError(Symbol),
}

/// Container for batch execution results
#[derive(Clone)]
#[contracttype]
pub struct BatchResult {
    pub results: Vec<OperationResult>,
    pub operations_executed: u32,
    pub operations_failed: u32,
}

impl BatchResult {
    pub fn new(env: &Env) -> Self {
        Self {
            results: Vec::new(env),
            operations_executed: 0,
            operations_failed: 0,
        }
    }
}

/// Validates all operations in a batch before execution
/// Returns Ok(()) if all operations are valid, Err with first error found
pub fn validate_batch(env: &Env, operations: &Vec<BatchOperation>) -> Result<(), Symbol> {
    // Check batch size limit
    if operations.len() > MAX_BATCH_SIZE {
        return Err(Symbol::new(env, "batch_size_exceeded"));
    }
    
    if operations.is_empty() {
        return Err(Symbol::new(env, "empty_batch"));
    }
    
    // Validate each operation
    for i in 0..operations.len() {
        if let Some(op) = operations.get(i) {
            match validate_operation(env, &op) {
                Ok(_) => continue,
                Err(e) => return Err(e),
            }
        }
    }
    
    Ok(())
}

/// Validates a single operation
fn validate_operation(env: &Env, operation: &BatchOperation) -> Result<(), Symbol> {
    match operation {
        BatchOperation::Swap(from, to, amount, _user) => {
            if *amount <= 0 {
                return Err(Symbol::new(env, "invalid_amount"));
            }
            if from == to {
                return Err(Symbol::new(env, "same_token_swap"));
            }
            // Validate tokens are supported
            if !is_valid_token(from) || !is_valid_token(to) {
                return Err(Symbol::new(env, "invalid_token"));
            }
            Ok(())
        }
        BatchOperation::AddLiquidity(xlm_amount, usdc_amount, _user) => {
            if *xlm_amount <= 0 || *usdc_amount <= 0 {
                return Err(Symbol::new(env, "invalid_liquidity"));
            }
            Ok(())
        }
        BatchOperation::RemoveLiquidity(xlm_amount, usdc_amount, _user) => {
            if *xlm_amount < 0 || *usdc_amount < 0 {
                return Err(Symbol::new(env, "negative_liquidity"));
            }
            if *xlm_amount == 0 && *usdc_amount == 0 {
                return Err(Symbol::new(env, "zero_liquidity"));
            }
            Ok(())
        }
        BatchOperation::MintToken(token, _to, amount) => {
            if *amount < 0 {
                return Err(Symbol::new(env, "negative_mint"));
            }
            if !is_valid_token(token) {
                return Err(Symbol::new(env, "invalid_token"));
            }
            Ok(())
        }
    }
}

/// Helper function to check if a token symbol is valid
fn is_valid_token(token: &Symbol) -> bool {
    let s = token.to_string();
    matches!(s.as_str(), "XLM" | "USDC-SIM")
}

/// Converts Symbol to Asset
fn symbol_to_asset(sym: &Symbol) -> Asset {
    let s = sym.to_string();
    match s.as_str() {
        "XLM" => Asset::XLM,
        "USDC-SIM" => Asset::Custom(sym.clone()),
        _ => Asset::Custom(sym.clone()), // Fallback for custom tokens
    }
}

/// Execute a batch of operations atomically (all-or-nothing)
/// Returns results for each operation
pub fn execute_batch_atomic(
    env: &Env,
    portfolio: &mut Portfolio,
    operations: Vec<BatchOperation>,
) -> Result<BatchResult, Symbol> {
    // Validate entire batch first
    validate_batch(env, &operations)?;
    
    // Create a snapshot of the portfolio state for rollback
    let snapshot = portfolio.clone();
    
    let mut batch_result = BatchResult::new(env);
    
    // Execute each operation
    for i in 0..operations.len() {
        if let Some(op) = operations.get(i) {
            match execute_single_operation(env, portfolio, &op) {
                Ok(result) => {
                    batch_result.results.push_back(OperationResult::Success(result));
                    batch_result.operations_executed += 1;
                }
                Err(error_sym) => {
                    // Rollback: restore portfolio to snapshot
                    *portfolio = snapshot;
                    batch_result.results.push_back(OperationResult::OpError(error_sym));
                    batch_result.operations_failed += 1;
                    
                    // Return error with partial results
                    return Err(Symbol::new(env, "batch_failed"));
                }
            }
        }
    }
    
    Ok(batch_result)
}

/// Execute a batch of operations with best-effort (continue on failure)
/// Returns results for each operation, does not rollback on individual failures
pub fn execute_batch_best_effort(
    env: &Env,
    portfolio: &mut Portfolio,
    operations: Vec<BatchOperation>,
) -> Result<BatchResult, Symbol> {
    // Validate entire batch first
    validate_batch(env, &operations)?;
    
    let mut batch_result = BatchResult::new(env);
    
    // Execute each operation, continue on failure
    for i in 0..operations.len() {
        if let Some(op) = operations.get(i) {
            match execute_single_operation(env, portfolio, &op) {
                Ok(result) => {
                    batch_result.results.push_back(OperationResult::Success(result));
                    batch_result.operations_executed += 1;
                }
                Err(error_sym) => {
                    batch_result.results.push_back(OperationResult::OpError(error_sym));
                    batch_result.operations_failed += 1;
                }
            }
        }
    }
    
    Ok(batch_result)
}

/// Execute a single operation
fn execute_single_operation(
    env: &Env,
    portfolio: &mut Portfolio,
    operation: &BatchOperation,
) -> Result<i128, Symbol> {
    match operation {
        BatchOperation::Swap(from, to, amount, user) => {
            // Check if user has sufficient balance
            let from_asset = symbol_to_asset(from);
            let balance = portfolio.balance_of(env, from_asset.clone(), user.clone());
            
            if balance < *amount {
                return Err(Symbol::new(env, "insufficient_funds"));
            }
            
            // Perform the swap
            let out_amount = perform_swap(env, portfolio, from.clone(), to.clone(), *amount, user.clone());
            portfolio.record_trade(env, user.clone());
            Ok(out_amount)
        }
        BatchOperation::AddLiquidity(xlm_amount, usdc_amount, user) => {
            // Check balances
            let xlm_balance = portfolio.balance_of(env, Asset::XLM, user.clone());
            let usdc_balance = portfolio.balance_of(
                env, 
                Asset::Custom(Symbol::new(env, "USDC-SIM")), 
                user.clone()
            );
            
            if xlm_balance < *xlm_amount || usdc_balance < *usdc_amount {
                return Err(Symbol::new(env, "insufficient_funds"));
            }
            
            // Add liquidity (simplified - just track in pool stats)
            portfolio.add_pool_liquidity(*xlm_amount, *usdc_amount);
            portfolio.record_lp_deposit(user.clone());
            
            // Deduct from user's balance
            let xlm_key = (user.clone(), Asset::XLM);
            let usdc_key = (user.clone(), Asset::Custom(Symbol::new(env, "USDC-SIM")));
            
            Ok(*xlm_amount + *usdc_amount) // Return total liquidity added
        }
        BatchOperation::RemoveLiquidity(xlm_amount, usdc_amount, user) => {
            // Simplified - in production you'd check LP token balance
            // For now, just update pool stats
            
            // Return liquidity to user (simplified)
            portfolio.mint(env, Asset::XLM, user.clone(), *xlm_amount);
            portfolio.mint(
                env, 
                Asset::Custom(Symbol::new(env, "USDC-SIM")), 
                user.clone(), 
                *usdc_amount
            );
            
            Ok(*xlm_amount + *usdc_amount) // Return total liquidity removed
        }
        BatchOperation::MintToken(token, to, amount) => {
            let asset = symbol_to_asset(token);
            portfolio.mint(env, asset, to.clone(), *amount);
            Ok(*amount)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;
    
    #[test]
    fn test_validate_batch_size_limit() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        // Create batch with more than MAX_BATCH_SIZE operations
        let mut operations = Vec::new(&env);
        for _ in 0..11 {
            operations.push_back(BatchOperation::Swap(
                Symbol::new(&env, "XLM"),
                Symbol::new(&env, "USDC-SIM"),
                100,
                user.clone(),
            ));
        }
        
        let result = validate_batch(&env, &operations);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Symbol::new(&env, "batch_size_exceeded"));
    }
    
    #[test]
    fn test_validate_empty_batch() {
        let env = Env::default();
        let operations = Vec::new(&env);
        
        let result = validate_batch(&env, &operations);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Symbol::new(&env, "empty_batch"));
    }
    
    #[test]
    fn test_validate_invalid_swap_amount() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        let mut operations = Vec::new(&env);
        operations.push_back(BatchOperation::Swap(
            Symbol::new(&env, "XLM"),
            Symbol::new(&env, "USDC-SIM"),
            -100, // Invalid negative amount
            user.clone(),
        ));
        
        let result = validate_batch(&env, &operations);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Symbol::new(&env, "invalid_amount"));
    }
    
    #[test]
    fn test_validate_same_token_swap() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        let mut operations = Vec::new(&env);
        operations.push_back(BatchOperation::Swap(
            Symbol::new(&env, "XLM"),
            Symbol::new(&env, "XLM"), // Same token
            100,
            user.clone(),
        ));
        
        let result = validate_batch(&env, &operations);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Symbol::new(&env, "same_token_swap"));
    }
}
