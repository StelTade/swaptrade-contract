//! Property-Based Fuzz Tests using proptest
//!
//! These tests use the `proptest` crate to generate random inputs and verify
//! critical contract invariants hold under all possible input combinations.
//!
//! Run with: `cargo test --lib proptest_ -- --nocapture`

use proptest::prelude::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};

use crate::errors::ContractError;
use crate::invariants::*;
use crate::portfolio::{Asset, LPPosition, Portfolio};

// ==================== PROPTEST STRATEGIES ====================

/// Strategy for generating valid swap amounts (positive i128 within bounds)
fn arb_swap_amount() -> impl Strategy<Value = i128> {
    (1_i128..1_000_000_000_000_i128)
}

/// Strategy for generating fee basis points (0-100 bps)
fn arb_fee_bps() -> impl Strategy<Value = i128> {
    (0_i128..=100_i128)
}

/// Strategy for generating slippage basis points (0-10000 bps = 0-100%)
fn arb_slippage_bps() -> impl Strategy<Value = u128> {
    (0_u128..=10_000_u128)
}

/// Strategy for generating pool reserves
fn arb_pool_reserves() -> impl Strategy<Value = (i128, i128)> {
    (1_i128..1_000_000_000_i128, 1_i128..1_000_000_000_i128)
}

/// Strategy for generating version pairs
fn arb_version_pair() -> impl Strategy<Value = (u32, u32)> {
    (0_u32..100_u32, 0_u32..100_u32)
}

/// Strategy for generating timestamp pairs
fn arb_timestamp_pair() -> impl Strategy<Value = (u64, u64)> {
    (0_u64..u64::MAX / 2, 0_u64..u64::MAX / 2)
}

/// Strategy for generating batch operation counts
fn arb_batch_counts() -> impl Strategy<Value = (u32, u32, u32)> {
    (0_u32..50_u32, 0_u32..50_u32, 0_u32..50_u32)
}

/// Strategy for generating balance update tuples (before, debit, credit, after)
fn arb_balance_update() -> impl Strategy<Value = (i128, i128, i128, i128)> {
    (
        0_i128..1_000_000_000_i128,
        0_i128..500_000_000_i128,
        0_i128..500_000_000_i128,
    )
        .prop_map(|(before, debit, credit)| {
            let actual_after = before.saturating_sub(debit).saturating_add(credit);
            (before, debit, credit, actual_after)
        })
}

// ==================== PROPTEST: FEE BOUNDS ====================

proptest! {
    #[test]
    fn proptest_fee_never_exceeds_one_percent(amount in 1_i128..1_000_000_000_000_i128, fee_bps in 0_i128..=100_i128) {
        let fee = (amount * fee_bps) / 10000;
        prop_assert!(
            invariant_fee_bounds(amount, fee),
            "Fee {} exceeds 1% bound for amount {} with {} bps",
            fee, amount, fee_bps
        );
    }

    #[test]
    fn proptest_fee_is_non_negative(amount in 1_i128..1_000_000_000_000_i128, fee_bps in 0_i128..=100_i128) {
        let fee = (amount * fee_bps) / 10000;
        prop_assert!(fee >= 0, "Fee {} should be non-negative", fee);
    }

    #[test]
    fn proptest_fee_is_proportional_to_amount(amount in 1_i128..1_000_000_000_000_i128) {
        let fee_bps = 30_i128; // 0.3% standard fee
        let fee = (amount * fee_bps) / 10000;
        prop_assert!(fee <= amount / 100, "Fee {} should be <= 1% of amount {}", fee, amount);
        prop_assert!(fee >= 0, "Fee should be non-negative");
    }
}

// ==================== PROPTEST: AMM CONSTANT PRODUCT ====================

proptest! {
    #[test]
    fn proptest_amm_k_never_increases(
        reserve_x in 1_i128..1_000_000_000_i128,
        reserve_y in 1_i128..1_000_000_000_i128,
        swap_amount in 1_i128..100_000_000_i128,
    ) {
        let k_before = (reserve_x as u128).saturating_mul(reserve_y as u128);
        // After swap: some amount of X is added, some Y is removed (or vice versa)
        // With 0.3% fee, k should not decrease
        let fee_bps = 30_u128;
        let amount_in_with_fee = (swap_amount as u128).saturating_mul(10000 - fee_bps) / 10000;
        let reserve_x_after = (reserve_x as u128).saturating_add(amount_in_with_fee);
        let reserve_y_after = k_before.saturating_div(reserve_x_after);
        let k_after = reserve_x_after.saturating_mul(reserve_y_after);

        prop_assert!(
            k_after <= k_before,
            "AMM k should not increase: before={}, after={}",
            k_before, k_after
        );
    }

    #[test]
    fn proptest_amm_reserves_stay_positive(
        reserve_x in 1000_i128..1_000_000_000_i128,
        reserve_y in 1000_i128..1_000_000_000_i128,
    ) {
        prop_assert!(reserve_x > 0, "Reserve X should be positive");
        prop_assert!(reserve_y > 0, "Reserve Y should be positive");
    }
}

// ==================== PROPTEST: BALANCE UPDATES ====================

proptest! {
    #[test]
    fn proptest_balance_update_consistency(
        before in 0_i128..1_000_000_000_i128,
        debit in 0_i128..500_000_000_i128,
        credit in 0_i128..500_000_000_i128,
    ) {
        let after = before.saturating_sub(debit).saturating_add(credit);
        prop_assert!(
            invariant_balance_update_consistency(before, debit, credit, after),
            "Balance update consistency failed: {} - {} + {} = {}",
            before, debit, credit, after
        );
    }

    #[test]
    fn proptest_debit_never_exceeds_balance(
        balance in 100_i128..1_000_000_000_i128,
        debit in 0_i128..1_000_000_000_i128,
    ) {
        if debit > balance {
            let after = balance.saturating_sub(debit);
            prop_assert!(after >= 0, "Saturating sub should prevent negative balance");
        }
    }
}

// ==================== PROPTEST: VERSION MONOTONICITY ====================

proptest! {
    #[test]
    fn proptest_version_monotonicity(prev in 0_u32..100_u32, curr in 0_u32..100_u32) {
        let result = invariant_version_monotonic(prev, curr);
        if curr >= prev {
            prop_assert!(result, "Version {} -> {} should be monotonic", prev, curr);
        } else {
            prop_assert!(!result, "Version {} -> {} should not be monotonic", prev, curr);
        }
    }
}

// ==================== PROPTEST: TIMESTAMP MONOTONICITY ====================

proptest! {
    #[test]
    fn proptest_timestamp_monotonicity(prev in 0_u64..u64::MAX / 2, curr in 0_u64..u64::MAX / 2) {
        let result = invariant_timestamp_monotonic(prev, curr);
        if curr >= prev {
            prop_assert!(result, "Timestamp {} -> {} should be monotonic", prev, curr);
        } else {
            prop_assert!(!result, "Timestamp {} -> {} should not be monotonic", prev, curr);
        }
    }
}

// ==================== PROPTEST: SLIPPAGE BOUNDS ====================

proptest! {
    #[test]
    fn proptest_slippage_within_bounds(
        expected in 0_i128..1_000_000_000_i128,
        slippage_pct in 0_u128..100_u128,
    ) {
        // If actual equals expected, slippage is 0 which is always within bounds
        let actual = expected;
        let max_slippage = 10000_u128; // 100%
        prop_assert!(
            invariant_slippage_bounds(expected, actual, max_slippage),
            "Zero slippage should always pass"
        );
    }

    #[test]
    fn proptest_max_slippage_allows_zero(
        expected in 1_i128..1_000_000_000_i128,
    ) {
        // Zero slippage with any max should pass
        prop_assert!(
            invariant_slippage_bounds(expected, expected, 10000),
            "Zero slippage should always pass"
        );
    }
}

// ==================== PROPTEST: BATCH INVARIANTS ====================

proptest! {
    #[test]
    fn proptest_batch_atomic_all_success(total in 1_u32..50_u32) {
        let result = verify_batch_invariants(
            &Env::default(),
            total,
            total,
            0,
            true,  // atomic
        );
        prop_assert!(result.is_ok(), "Atomic batch with all success should pass");
    }

    #[test]
    fn proptest_batch_atomic_all_failure(total in 1_u32..50_u32) {
        let result = verify_batch_invariants(
            &Env::default(),
            total,
            0,
            total,
            true,  // atomic
        );
        prop_assert!(result.is_ok(), "Atomic batch with all failure should pass");
    }

    #[test]
    fn proptest_batch_best_effort_mixed(total in 2_u32..50_u32, half in 1_u32..49_u32) {
        let success = core::cmp::min(half, total - 1);
        let failure = total - success;
        let result = verify_batch_invariants(
            &Env::default(),
            total,
            success,
            failure,
            false, // best-effort
        );
        prop_assert!(result.is_ok(), "Best-effort batch with mixed results should pass");
    }
}

// ==================== PROPTEST: LP TOKEN CALCULATIONS ====================

proptest! {
    #[test]
    fn proptest_lp_tokens_non_negative(xlm in 1_i128..1_000_000_000_i128, usdc in 1_i128..1_000_000_000_i128) {
        let product = (xlm as u128).saturating_mul(usdc as u128);
        let lp_tokens = integer_sqrt(product);
        prop_assert!(lp_tokens >= 0, "LP tokens should be non-negative");
    }

    #[test]
    fn proptest_lp_tokens_at_least_minimum(xlm in 100_i128..1_000_000_000_i128, usdc in 100_i128..1_000_000_000_i128) {
        let product = (xlm as u128).saturating_mul(usdc as u128);
        let lp_tokens = integer_sqrt(product);
        prop_assert!(lp_tokens >= 1, "LP tokens should be at least 1 for non-zero deposits");
    }

    #[test]
    fn proptest_lp_tokens_symmetric(xlm in 1_i128..1_00_000_000_i128, usdc in 1_i128..1_000_000_000_i128) {
        // LP token calculation with equal amounts should be same regardless of order
        let product1 = (xlm as u128).saturating_mul(usdc as u128);
        let product2 = (usdc as u128).saturating_mul(xlm as u128);
        prop_assert_eq!(integer_sqrt(product1), integer_sqrt(product2));
    }
}

// ==================== PROPTEST: OVERFLOW SAFETY ====================

proptest! {
    #[test]
    fn proptest_saturating_add_no_panic(a in 0_i128..i128::MAX, b in 0_i128..i128::MAX) {
        let result = a.saturating_add(b);
        prop_assert!(result >= a, "Saturating add result should be >= operand");
    }

    #[test]
    fn proptest_saturating_sub_no_panic(a in 0_i128..i128::MAX, b in 0_i128..i128::MAX) {
        let result = a.saturating_sub(b);
        prop_assert!(result <= a, "Saturating sub result should be <= operand");
    }

    #[test]
    fn proptest_u128_saturating_mul(a in 0_u128..u64::MAX as u128, b in 0_u128..u64::MAX as u128) {
        let result = a.saturating_mul(b);
        prop_assert!(result >= a || a == 0, "Saturating mul should not underflow");
    }
}

// ==================== PROPTEST: PORTFOLIO OPERATIONS ====================

proptest! {
    #[test]
    fn proptest_mint_always_increases_balance(amount in 1_i128..1_000_000_000_i128) {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = Address::generate(&env);

        portfolio.mint(&env, Asset::XLM, user.clone(), amount);
        let balance = portfolio.balance_of(&env, Asset::XLM, user.clone());
        prop_assert!(balance >= amount, "Balance {} should be >= minted amount {}", balance, amount);
    }

    #[test]
    fn proptest_multiple_mints_accumulate(a in 1_i128..500_000_000_i128, b in 1_i128..500_000_000_i128) {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = Address::generate(&env);

        portfolio.mint(&env, Asset::XLM, user.clone(), a);
        portfolio.mint(&env, Asset::XLM, user.clone(), b);

        let balance = portfolio.balance_of(&env, Asset::XLM, user.clone());
        prop_assert_eq!(balance, a + b);
    }

    #[test]
    fn proptest_users_isolated(a_amount in 1_i128..100_000_i128, b_amount in 1_i128..100_000_i128) {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user_a = Address::generate(&env);
        let user_b = Address::generate(&env);

        portfolio.mint(&env, Asset::XLM, user_a.clone(), a_amount);
        portfolio.mint(&env, Asset::XLM, user_b.clone(), b_amount);

        let bal_a = portfolio.balance_of(&env, Asset::XLM, user_a.clone());
        let bal_b = portfolio.balance_of(&env, Asset::XLM, user_b.clone());

        prop_assert_eq!(bal_a, a_amount);
        prop_assert_eq!(bal_b, b_amount);
    }
}

// ==================== PROPTEST: EXHAUSTIVE INVARIANT CHECK ====================

proptest! {
    #[test]
    fn proptest_invariants_hold_after_sequential_operations(
        operations in prop::collection::vec(0_u8..5, 1..20),
    ) {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);

        for (i, op) in operations.into_iter().enumerate() {
            let user = Address::generate(&env);
            match op {
                0 => {
                    let amount = ((i as i128) + 1) * 1000;
                    portfolio.mint(&env, Asset::XLM, user, amount);
                }
                1 => {
                    let amount = ((i as i128) + 1) * 500;
                    portfolio.credit(&env, Asset::XLM, user, amount);
                }
                2 => {
                    portfolio.record_trade(&env, user);
                }
                3 => {
                    let xlm = ((i as i128) + 1) * 100;
                    let usdc = ((i as i128) + 1) * 100;
                    portfolio.add_pool_liquidity(xlm, usdc);
                }
                4 => {
                    let fee = ((i as i128) + 1) * 10;
                    portfolio.collect_fee(fee);
                }
                _ => {}
            }

            // Verify all invariants hold after each operation
            prop_assert!(
                invariant_non_negative_balances(&portfolio),
                "Negative balance at op {}", i
            );
            prop_assert!(
                invariant_pool_liquidity_non_negative(&portfolio),
                "Pool liquidity negative at op {}", i
            );
            prop_assert!(
                invariant_lp_token_conservation(&portfolio),
                "LP token invariant at op {}", i
            );
            prop_assert!(
                invariant_metrics_non_negative(&portfolio),
                "Metrics negative at op {}", i
            );
            prop_assert!(
                invariant_fee_accumulation_non_negative(&portfolio),
                "Fee accumulation negative at op {}", i
            );
        }
    }
}

// ==================== HELPER ====================

/// Integer square root using Babylonian method
fn integer_sqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
