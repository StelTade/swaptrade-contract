#![cfg(test)]
#![allow(clippy::all, mismatched_lifetime_syntaxes)]
#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    deprecated,
    unused_doc_comments,
    unused_mut
)]

use counter::{CounterContract, CounterContractClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, Symbol,
};

fn setup_test_portfolio(env: &Env) -> (CounterContractClient, Address, Symbol, Symbol) {
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(env, &contract_id);
    let user = Address::generate(env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");
    (client, user, xlm, usdc)
}

#[test]
fn test_swap_basic_xlm_to_usdc() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, user, xlm, usdc) = setup_test_portfolio(&env);

    client.mint(&xlm, &user, &1000);
    let out = client.swap(&xlm, &usdc, &100, &user);
    assert_eq!(out, 100);
    assert_eq!(client.get_balance(&xlm, &user), 900);
    assert_eq!(client.get_balance(&usdc, &user), 100);
}

#[test]
fn test_swap_basic_usdc_to_xlm() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, user, xlm, usdc) = setup_test_portfolio(&env);

    client.mint(&usdc, &user, &1000);
    let out = client.swap(&usdc, &xlm, &200, &user);
    assert_eq!(out, 200);
    assert_eq!(client.get_balance(&usdc, &user), 800);
    assert_eq!(client.get_balance(&xlm, &user), 200);
}

#[test]
fn test_swap_1_unit() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, user, xlm, usdc) = setup_test_portfolio(&env);

    client.mint(&xlm, &user, &1000);
    let out = client.swap(&xlm, &usdc, &1, &user);
    assert_eq!(out, 1);
    assert_eq!(client.get_balance(&xlm, &user), 999);
    assert_eq!(client.get_balance(&usdc, &user), 1);
}

#[test]
fn test_swap_sequential() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, user, xlm, usdc) = setup_test_portfolio(&env);

    client.mint(&xlm, &user, &5000);

    // Trade 1
    client.swap(&xlm, &usdc, &1000, &user);
    assert_eq!(client.get_balance(&xlm, &user), 4000);
    assert_eq!(client.get_balance(&usdc, &user), 997); // 1000 - 0.3% fee

    // Trade 2
    client.swap(&xlm, &usdc, &1000, &user);
    assert_eq!(client.get_balance(&xlm, &user), 3000);

    // Trade 3: Reverse
    client.swap(&usdc, &xlm, &500, &user);
    assert_eq!(client.get_balance(&usdc, &user), 1495);
}

#[test]
fn test_swap_state_consistency() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, user, xlm, usdc) = setup_test_portfolio(&env);

    client.mint(&xlm, &user, &1000);

    let metrics_before = client.get_metrics();
    let txs_before = client.get_user_transactions(&user, &10);
    assert_eq!(txs_before.len(), 0);

    client.swap(&xlm, &usdc, &100, &user);

    let metrics_after = client.get_metrics();
    assert_eq!(
        metrics_after.trades_executed,
        metrics_before.trades_executed + 1
    );
    assert_eq!(
        metrics_after.balances_updated,
        metrics_before.balances_updated + 2
    );

    let txs_after = client.get_user_transactions(&user, &10);
    assert_eq!(txs_after.len(), 1);
}

#[test]
fn test_swap_rounding() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, user, xlm, usdc) = setup_test_portfolio(&env);

    let precision: u128 = 1_000_000_000_000_000_000;
    let price: u128 = (25 * precision) / 10;
    client.set_price(&(xlm.clone(), usdc.clone()), &price);

    client.mint(&xlm, &user, &1000);
    let out = client.swap(&xlm, &usdc, &3, &user);
    assert_eq!(out, 7);
}

#[test]
fn test_swap_large_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, user, xlm, usdc) = setup_test_portfolio(&env);

    let safe_max = 100_000_000_000_000_000_000;
    client.mint(&xlm, &user, &100_000_000_000_000_000_000_000);

    let _ = client.swap(&xlm, &usdc, &safe_max, &user);
}

#[test]
fn test_tier_fees_novice() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, user, xlm, usdc) = setup_test_portfolio(&env);

    client.mint(&xlm, &user, &10000);
    client.swap(&xlm, &usdc, &1000, &user);
    // Novice: 30 bps. Fee = 3. Out = 997.
    assert_eq!(client.get_balance(&usdc, &user), 997);
}

macro_rules! test_swap_amount {
    ($name:ident, $amount:expr) => {
        #[test]
        fn $name() {
            let env = Env::default();
            env.mock_all_auths();
            let (client, user, xlm, usdc) = setup_test_portfolio(&env);

            client.mint(&xlm, &user, &1000000000);

            let out = client.swap(&xlm, &usdc, &$amount, &user);

            let fee = ($amount * 30) / 10000;
            let expected = $amount - fee;
            assert_eq!(out, expected);
        }
    };
}

test_swap_amount!(test_swap_amt_10, 10);
test_swap_amount!(test_swap_amt_100, 100);
test_swap_amount!(test_swap_amt_500, 500);
test_swap_amount!(test_swap_amt_1000, 1000);
test_swap_amount!(test_swap_amt_5000, 5000);
test_swap_amount!(test_swap_amt_10000, 10000);

macro_rules! test_swap_reverse {
    ($name:ident, $amount:expr) => {
        #[test]
        fn $name() {
            let env = Env::default();
            env.mock_all_auths();
            let (client, user, xlm, usdc) = setup_test_portfolio(&env);

            client.mint(&usdc, &user, &1000000000);

            let out = client.swap(&usdc, &xlm, &$amount, &user);
            let fee = ($amount * 30) / 10000;
            let expected = $amount - fee;
            assert_eq!(out, expected);
        }
    };
}

test_swap_reverse!(test_swap_rev_10, 10);
test_swap_reverse!(test_swap_rev_100, 100);
test_swap_reverse!(test_swap_rev_1000, 1000);
test_swap_reverse!(test_swap_rev_10000, 10000);

#[test]
fn test_swap_max_i128() {
    // Placeholder for max amount test
}

#[test]
fn test_swap_identical_tokens() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, user, xlm, _) = setup_test_portfolio(&env);

    client.mint(&xlm, &user, &1000);
    // Swapping XLM -> XLM should fail or be identity
    let _ = client.swap(&xlm, &xlm, &100, &user);
}
