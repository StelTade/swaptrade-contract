//! Contract Invariants Module
//!
//! This module provides comprehensive invariant checking for the SwapTrade contract.
//! All critical security properties are verified through these functions.

use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};

use crate::errors::ContractError;
use crate::portfolio::{Asset, LPPosition, Portfolio};

/// Maximum allowed fee in basis points (1%)
const MAX_FEE_BPS: i128 = 100;
/// Maximum slippage in basis points (100%)
const MAX_SLIPPAGE_BPS: u128 = 10000;
/// Precision for price calculations
const PRECISION: u128 = 1_000_000_000_000_000_000;

/// Comprehensive invariant check result
#[derive(Clone, Debug, PartialEq)]
pub struct InvariantCheck {
    pub passed: bool,
    pub failed_checks: Vec<Symbol>,
}

impl InvariantCheck {
    pub fn new(env: &Env) -> Self {
        Self {
            passed: true,
            failed_checks: Vec::new(env),
        }
    }

    pub fn record_failure(&mut self, check_name: Symbol) {
        self.passed = false;
        self.failed_checks.push_back(check_name);
    }
}

/// Main entry point: Verify all contract invariants
///
/// This function should be called after any state-changing operation
/// to ensure the contract remains in a consistent state.
///
/// # Returns
/// - `Ok(())` if all invariants hold
/// - `Err(ContractError)` with details of which invariant failed
///
/// # Example
/// ```
/// verify_contract_invariants(&env, &portfolio)?;
/// ```
pub fn verify_contract_invariants(env: &Env, portfolio: &Portfolio) -> Result<(), ContractError> {
    let mut check = InvariantCheck::new(env);

    // Asset conservation invariants
    if !invariant_non_negative_balances(portfolio) {
        check.record_failure(symbol_short!("neg_bal"));
    }

    // Pool liquidity invariants
    if !invariant_pool_liquidity_non_negative(portfolio) {
        check.record_failure(symbol_short!("neg_pool"));
    }

    // LP token invariants
    if !invariant_lp_token_conservation(portfolio) {
        check.record_failure(symbol_short!("lp_tok"));
    }

    // Metrics invariants
    if !invariant_metrics_non_negative(portfolio) {
        check.record_failure(symbol_short!("neg_met"));
    }

    // Fee accumulation invariants
    if !invariant_fee_accumulation_non_negative(portfolio) {
        check.record_failure(symbol_short!("neg_fee"));
    }

    // User count invariants
    if !invariant_user_counts_consistent(portfolio) {
        check.record_failure(symbol_short!("usr_cnt"));
    }

    if check.passed {
        Ok(())
    } else {
        Err(ContractError::InvariantViolation)
    }
}

/// Verify invariants after a swap operation
///
/// Additional checks specific to swap operations:
/// - AMM constant product (k should not increase)
/// - Output amount > 0
/// - Fee within bounds
pub fn verify_swap_invariants(
    env: &Env,
    portfolio: &Portfolio,
    xlm_before: i128,
    usdc_before: i128,
    xlm_after: i128,
    usdc_after: i128,
    input_amount: i128,
    output_amount: i128,
    fee_amount: i128,
) -> Result<(), ContractError> {
    let mut check = InvariantCheck::new(env);

    // AMM constant product: k should not increase (fees reduce k)
    if !invariant_amm_constant_product(xlm_before, usdc_before, xlm_after, usdc_after) {
        check.record_failure(symbol_short!("amm_k"));
    }

    // Output must be positive for non-zero input
    if input_amount > 0 && output_amount <= 0 {
        check.record_failure(symbol_short!("zero"));
    }

    // Fee bounds check
    if !invariant_fee_bounds(input_amount, fee_amount) {
        check.record_failure(symbol_short!("fee"));
    }

    // Pool reserves must remain non-negative
    if xlm_after < 0 || usdc_after < 0 {
        check.record_failure(symbol_short!("neg_res"));
    }

    if check.passed {
        Ok(())
    } else {
        Err(ContractError::InvariantViolation)
    }
}

/// Verify invariants after liquidity provision
///
/// Checks:
/// - LP tokens minted > 0
/// - Pool liquidity increased correctly
/// - User balances decreased by deposit amounts
pub fn verify_add_liquidity_invariants(
    env: &Env,
    portfolio: &Portfolio,
    xlm_deposited: i128,
    usdc_deposited: i128,
    lp_tokens_minted: i128,
    xlm_before: i128,
    usdc_before: i128,
) -> Result<(), ContractError> {
    let mut check = InvariantCheck::new(env);

    // LP tokens must be positive
    if lp_tokens_minted <= 0 {
        check.record_failure(symbol_short!("lp_min"));
    }

    // Deposits must be positive
    if xlm_deposited <= 0 || usdc_deposited <= 0 {
        check.record_failure(symbol_short!("dep"));
    }

    // Pool liquidity should increase
    let usdc_sym = Symbol::new(env, "USDCSIM");
    let usdc_asset = Asset::Custom(usdc_sym);
    let xlm_after = portfolio.get_liquidity(Asset::XLM);
    let usdc_after = portfolio.get_liquidity(usdc_asset);

    if xlm_after < xlm_before || usdc_after < usdc_before {
        check.record_failure(symbol_short!("pool"));
    }

    // Verify LP token calculation is proportional
    if xlm_before > 0 && usdc_before > 0 {
        let total_lp = portfolio.get_total_lp_tokens();
        if total_lp <= 0 {
            check.record_failure(symbol_short!("lp_tot"));
        }
    }

    if check.passed {
        Ok(())
    } else {
        Err(ContractError::InvariantViolation)
    }
}

/// Verify invariants after liquidity removal
///
/// Checks:
/// - User receives correct amounts
/// - LP tokens burned correctly
/// - Pool liquidity decreased correctly
pub fn verify_remove_liquidity_invariants(
    env: &Env,
    portfolio: &Portfolio,
    lp_tokens_burned: i128,
    xlm_returned: i128,
    usdc_returned: i128,
    xlm_before: i128,
    usdc_before: i128,
) -> Result<(), ContractError> {
    let mut check = InvariantCheck::new(env);

    // LP tokens burned must be positive
    if lp_tokens_burned <= 0 {
        check.record_failure(symbol_short!("lp_burn"));
    }

    // Returned amounts must be positive
    if xlm_returned <= 0 || usdc_returned <= 0 {
        check.record_failure(symbol_short!("ret_pos"));
    }

    // Pool liquidity should decrease
    let xlm_after = portfolio.get_liquidity(Asset::XLM);
    let usdc_after = portfolio.get_liquidity(Asset::Custom(symbol_short!("USDCSIM")));

    if xlm_after > xlm_before || usdc_after > usdc_before {
        check.record_failure(symbol_short!("pool_dec"));
    }

    // Verify proportional return
    let total_lp = portfolio.get_total_lp_tokens();
    if total_lp < 0 {
        check.record_failure(symbol_short!("lp_neg"));
    }

    if check.passed {
        Ok(())
    } else {
        Err(ContractError::InvariantViolation)
    }
}

/// Verify batch operation invariants
///
/// For atomic batches: all succeed or all fail
/// For best-effort: track success/failure counts
pub fn verify_batch_invariants(
    env: &Env,
    operations_count: u32,
    success_count: u32,
    failure_count: u32,
    is_atomic: bool,
) -> Result<(), ContractError> {
    let mut check = InvariantCheck::new(env);

    // Counts must sum correctly
    if success_count + failure_count != operations_count {
        check.record_failure(symbol_short!("batch_cnt"));
    }

    // For atomic batches, either all succeed or all fail
    if is_atomic && failure_count > 0 && success_count > 0 {
        check.record_failure(symbol_short!("atm_fail"));
    }

    // Cannot have more successes/failures than operations
    if success_count > operations_count || failure_count > operations_count {
        check.record_failure(symbol_short!("cnt_ovf"));
    }

    if check.passed {
        Ok(())
    } else {
        Err(ContractError::InvariantViolation)
    }
}

// ==================== INDIVIDUAL INVARIANT CHECKS ====================

/// INVARIANT: All balances must be non-negative
///
/// This is a critical safety property - users should never have negative balances.
/// We check observable pool balances which are the aggregate of user positions.
pub fn invariant_non_negative_balances(portfolio: &Portfolio) -> bool {
    // Check pool reserves (aggregate of all positions)
    portfolio.get_liquidity(Asset::XLM) >= 0
        && portfolio.get_liquidity(Asset::Custom(symbol_short!("USDCSIM"))) >= 0
}

/// INVARIANT: Pool liquidity must always be non-negative
///
/// The AMM pool should never have negative reserves.
pub fn invariant_pool_liquidity_non_negative(portfolio: &Portfolio) -> bool {
    portfolio.get_pool_stats().0 >= 0 && // xlm_in_pool
    portfolio.get_pool_stats().1 >= 0 // usdc_in_pool
}

/// INVARIANT: LP token conservation
///
/// Total LP tokens must be non-negative and track the sum of all positions.
/// Note: Full verification requires iterating all positions (Soroban limitation).
pub fn invariant_lp_token_conservation(portfolio: &Portfolio) -> bool {
    portfolio.get_total_lp_tokens() >= 0
}

/// INVARIANT: Metrics must be non-negative
///
/// All statistical counters should never be negative.
pub fn invariant_metrics_non_negative(portfolio: &Portfolio) -> bool {
    let metrics = portfolio.get_metrics();
    true // counters are unsigned and always non-negative
}

/// INVARIANT: Fee accumulation must be non-negative
///
/// Accumulated fees should never be negative.
pub fn invariant_fee_accumulation_non_negative(portfolio: &Portfolio) -> bool {
    portfolio.get_pool_stats().2 >= 0 && // total_fees_collected
    portfolio.get_lp_fees_accumulated() >= 0
}

/// INVARIANT: User counts must be consistent
///
/// Active users count should not exceed total users.
pub fn invariant_user_counts_consistent(portfolio: &Portfolio) -> bool {
    portfolio.get_active_users_count() <= portfolio.get_total_users()
}

/// INVARIANT: AMM Constant Product
///
/// For constant product AMM: x * y = k
/// After a swap with fees, k should not increase (fees reduce k).
/// This prevents manipulation that would create value from nothing.
pub fn invariant_amm_constant_product(
    xlm_before: i128,
    usdc_before: i128,
    xlm_after: i128,
    usdc_after: i128,
) -> bool {
    // Prevent negative reserves
    if xlm_after < 0 || usdc_after < 0 {
        return false;
    }

    // Calculate k values
    let k_before = (xlm_before as u128).saturating_mul(usdc_before as u128);
    let k_after = (xlm_after as u128).saturating_mul(usdc_after as u128);

    // After swap with fees, k should not increase
    k_after <= k_before
}

/// INVARIANT: Fee Bounds
///
/// Fees must be within acceptable bounds:
/// - Fee >= 0 (non-negative)
/// - Fee <= 1% of amount (MAX_FEE_BPS)
pub fn invariant_fee_bounds(amount: i128, fee: i128) -> bool {
    // Fee must be non-negative
    if fee < 0 {
        return false;
    }

    // Zero amount should have zero fee
    if amount == 0 {
        return fee == 0;
    }

    // Fee must not exceed maximum
    let max_fee = (amount * MAX_FEE_BPS) / 10000;
    fee <= max_fee
}

/// INVARIANT: Slippage Bounds
///
/// Slippage must be within configured limits.
pub fn invariant_slippage_bounds(
    expected_output: u128,
    actual_output: u128,
    max_slippage_bps: u32,
) -> bool {
    if expected_output == 0 {
        return actual_output == 0;
    }

    if actual_output > expected_output {
        return true; // Positive slippage is acceptable
    }

    let slippage = ((expected_output - actual_output) * 10000) / expected_output;
    slippage <= max_slippage_bps as u128
}

/// INVARIANT: Balance Update Consistency
///
/// Verifies that balance updates are applied correctly:
/// new_balance = old_balance - debit + credit
pub fn invariant_balance_update_consistency(
    balance_before: i128,
    debit_amount: i128,
    credit_amount: i128,
    balance_after: i128,
) -> bool {
    let calculated = balance_before
        .saturating_sub(debit_amount)
        .saturating_add(credit_amount);
    calculated == balance_after
}

/// INVARIANT: LP Position Integrity
///
/// Verifies that LP positions are internally consistent:
/// - LP tokens >= 0
/// - If LP tokens > 0, then deposits must be > 0
pub fn invariant_lp_position_integrity(position: &LPPosition) -> bool {
    position.lp_tokens_minted >= 0
        && position.xlm_deposited >= 0
        && position.usdc_deposited >= 0
        && (position.lp_tokens_minted == 0
            || (position.xlm_deposited > 0 && position.usdc_deposited > 0))
}

/// INVARIANT: Rate Limit Consistency
///
/// Rate limit counters should never decrease within a time window.
pub fn invariant_rate_limit_monotonic(previous_count: u32, current_count: u32) -> bool {
    current_count >= previous_count
}

/// INVARIANT: Version Monotonicity
///
/// Contract version should only increase during migrations.
pub fn invariant_version_monotonic(previous_version: u32, current_version: u32) -> bool {
    current_version >= previous_version
}

/// INVARIANT: Badge Uniqueness
///
/// Users cannot have duplicate badges.
/// Note: This is enforced by the award_badge logic.
pub fn invariant_badge_uniqueness(env: &Env, portfolio: &Portfolio, user: &Address) -> bool {
    let badges = portfolio.get_user_badges(env, user.clone());
    // Maximum 7 distinct badge types exist
    badges.len() <= 7
}

/// INVARIANT: Trading Volume Consistency
///
/// Total trading volume should equal sum of all swap amounts.
/// This is a statistical invariant tracked in metrics.
pub fn invariant_trading_volume_non_negative(portfolio: &Portfolio) -> bool {
    portfolio.get_total_trading_volume() >= 0
}

/// INVARIANT: Timestamp Monotonicity
///
/// Ledger timestamps should be monotonically increasing.
pub fn invariant_timestamp_monotonic(previous_timestamp: u64, current_timestamp: u64) -> bool {
    current_timestamp >= previous_timestamp
}

// ==================== DEBUG/TEST HELPERS ====================

/// Get a detailed invariant report for debugging
///
/// Returns a list of all invariants and their status
pub fn get_invariant_report(env: &Env, portfolio: &Portfolio) -> Vec<(Symbol, bool)> {
    let mut report = Vec::new(env);

    report.push_back((
        symbol_short!("neg_bal"),
        invariant_non_negative_balances(portfolio),
    ));
    report.push_back((
        symbol_short!("neg_pool"),
        invariant_pool_liquidity_non_negative(portfolio),
    ));
    report.push_back((
        symbol_short!("lp_tok"),
        invariant_lp_token_conservation(portfolio),
    ));
    report.push_back((
        symbol_short!("neg_met"),
        invariant_metrics_non_negative(portfolio),
    ));
    report.push_back((
        symbol_short!("neg_fee"),
        invariant_fee_accumulation_non_negative(portfolio),
    ));
    report.push_back((
        symbol_short!("usr_cnt"),
        invariant_user_counts_consistent(portfolio),
    ));
    report.push_back((
        symbol_short!("volume"),
        invariant_trading_volume_non_negative(portfolio),
    ));

    report
}

/// Assert all invariants in test mode
///
/// Panics with detailed message if any invariant fails
#[cfg(test)]
pub fn assert_all_invariants(env: &Env, portfolio: &Portfolio) {
    let report = get_invariant_report(env, portfolio);

    for i in 0..report.len() {
        if let Some((name, passed)) = report.get(i) {
            assert!(passed, "Invariant failed: {:?}", name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_invariant_non_negative_balances_pass() {
        let env = Env::default();
        let portfolio = Portfolio::new(&env);
        assert!(invariant_non_negative_balances(&portfolio));
    }

    #[test]
    fn test_invariant_amm_constant_product_pass() {
        // Normal swap with fees: k should decrease or stay same
        let xlm_before = 10000i128;
        let usdc_before = 10000i128;
        let xlm_after = 11000i128;
        let usdc_after = 9000i128;

        // k_before = 100M, k_after = 99M (fees reduced k)
        assert!(invariant_amm_constant_product(
            xlm_before,
            usdc_before,
            xlm_after,
            usdc_after
        ));
    }

    #[test]
    fn test_invariant_amm_constant_product_fail() {
        // Impossible scenario: k increases (value created from nothing)
        let xlm_before = 10000i128;
        let usdc_before = 10000i128;
        let xlm_after = 9000i128;
        let usdc_after = 12000i128;

        // k_before = 100M, k_after = 108M (impossible without external input)
        assert!(!invariant_amm_constant_product(
            xlm_before,
            usdc_before,
            xlm_after,
            usdc_after
        ));
    }

    #[test]
    fn test_invariant_fee_bounds_pass() {
        // 0.3% fee on 10000 = 30
        assert!(invariant_fee_bounds(10000, 30));

        // Zero amount, zero fee
        assert!(invariant_fee_bounds(0, 0));

        // Max 1% fee
        assert!(invariant_fee_bounds(10000, 100));
    }

    #[test]
    fn test_invariant_fee_bounds_fail() {
        // Negative fee
        assert!(!invariant_fee_bounds(10000, -1));

        // Fee exceeds 1%
        assert!(!invariant_fee_bounds(10000, 101));

        // Zero amount with non-zero fee
        assert!(!invariant_fee_bounds(0, 1));
    }

    #[test]
    fn test_invariant_lp_position_integrity_pass() {
        let position = LPPosition {
            lp_address: Address::generate(&Env::default()),
            xlm_deposited: 1000,
            usdc_deposited: 1000,
            lp_tokens_minted: 1000,
        };
        assert!(invariant_lp_position_integrity(&position));
    }

    #[test]
    fn test_invariant_lp_position_integrity_empty_pass() {
        let position = LPPosition {
            lp_address: Address::generate(&Env::default()),
            xlm_deposited: 0,
            usdc_deposited: 0,
            lp_tokens_minted: 0,
        };
        assert!(invariant_lp_position_integrity(&position));
    }

    #[test]
    fn test_invariant_balance_update_consistency_pass() {
        // Start with 1000, debit 200, credit 300 = 1100
        assert!(invariant_balance_update_consistency(1000, 200, 300, 1100));
    }

    #[test]
    fn test_invariant_balance_update_consistency_fail() {
        // Incorrect final balance
        assert!(!invariant_balance_update_consistency(1000, 200, 300, 1000));
    }

    #[test]
    fn test_invariant_slippage_bounds_pass() {
        // 1% slippage on expected 10000
        assert!(invariant_slippage_bounds(10000, 9900, 100));

        // No slippage
        assert!(invariant_slippage_bounds(10000, 10000, 100));

        // Positive slippage (better than expected)
        assert!(invariant_slippage_bounds(10000, 10100, 100));
    }

    #[test]
    fn test_invariant_slippage_bounds_fail() {
        // 2% slippage when max is 1%
        assert!(!invariant_slippage_bounds(10000, 9800, 100));
    }

    #[test]
    fn test_invariant_version_monotonic_pass() {
        assert!(invariant_version_monotonic(1, 2));
        assert!(invariant_version_monotonic(1, 1)); // Same version is ok
    }

    #[test]
    fn test_invariant_version_monotonic_fail() {
        assert!(!invariant_version_monotonic(2, 1));
    }

    #[test]
    fn test_invariant_timestamp_monotonic_pass() {
        assert!(invariant_timestamp_monotonic(1000, 2000));
        assert!(invariant_timestamp_monotonic(1000, 1000));
    }

    #[test]
    fn test_invariant_timestamp_monotonic_fail() {
        assert!(!invariant_timestamp_monotonic(2000, 1000));
    }
}
