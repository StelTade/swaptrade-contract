#![cfg(test)]

extern crate std;
use std::println;

use atomic_swap::{AtomicSwapContract, SwapState};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, IntoVal, Symbol};

// ══════════════════════════════════════════════════════════════
//  Stub Token (simulates Stellar Asset Contract)
// ══════════════════════════════════════════════════════════════

const BAL_KEY: Symbol = symbol_short!("bal");

#[contract]
struct StubToken;

#[contractimpl]
impl StubToken {
    pub fn initialize(env: Env, admin: Address, supply: i128) {
        let mut bal: soroban_sdk::Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&BAL_KEY)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        bal.set(admin, supply);
        env.storage().persistent().set(&BAL_KEY, &bal);
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        let bal: soroban_sdk::Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&BAL_KEY)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        bal.get(id).unwrap_or(0)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> i128 {
        from.require_auth();
        let mut bal: soroban_sdk::Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&BAL_KEY)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        let from_bal = bal.get(from.clone()).unwrap_or(0);
        assert!(from_bal >= amount, "insufficient balance");
        bal.set(from.clone(), from_bal - amount);
        bal.set(to.clone(), bal.get(to.clone()).unwrap_or(0) + amount);
        env.storage().persistent().set(&BAL_KEY, &bal);
        amount
    }

    pub fn approve(_env: Env, _from: Address, _spender: Address, _amount: i128, _exp: u32) -> i128 {
        0
    }
}

/// Deploy a stub token: register the contract, then call `initialize` via invoke_contract.
fn create_token(env: &Env, admin: &Address, supply: i128) -> Address {
    #[allow(deprecated)]
    let addr = env.register_contract(None, StubToken);
    env.invoke_contract::<()>(
        &addr,
        &Symbol::new(env, "initialize"),
        soroban_sdk::vec![env, admin.to_val(), supply.into_val(env)],
    );
    addr
}

// ══════════════════════════════════════════════════════════════
//  Test helpers
// ══════════════════════════════════════════════════════════════

struct TestContext {
    env: Env,
    creator: Address,
    counterparty: Address,
    asset_a: Address,
    asset_b: Address,
    client: atomic_swap::AtomicSwapContractClient<'static>,
}

fn setup() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();

    let creator = Address::generate(&env);
    let counterparty = Address::generate(&env);
    let asset_a = create_token(&env, &creator, 100_000);
    let asset_b = create_token(&env, &counterparty, 100_000);

    #[allow(deprecated)]
    let contract_id = env.register_contract(None, AtomicSwapContract);
    let client = atomic_swap::AtomicSwapContractClient::new(&env, &contract_id);

    TestContext {
        env,
        creator,
        counterparty,
        asset_a,
        asset_b,
        client,
    }
}

fn create_swap_helper(ctx: &TestContext, nonce: u64, expiry_offset: u64) -> u64 {
    let now = ctx.env.ledger().timestamp();
    ctx.client.create_swap(
        &ctx.creator,
        &ctx.counterparty,
        &ctx.asset_a,
        &100i128,
        &ctx.asset_b,
        &200i128,
        &(now + expiry_offset),
        &nonce,
    )
}

fn fund_creator(ctx: &TestContext, swap_id: u64) {
    ctx.client.fund_swap(&swap_id, &ctx.creator);
}

fn fund_counterparty(ctx: &TestContext, swap_id: u64) {
    ctx.client.fund_swap(&swap_id, &ctx.counterparty);
}

fn accept_swap_helper(ctx: &TestContext, swap_id: u64) {
    ctx.client.accept_swap(&swap_id, &ctx.counterparty);
}

fn get_swap_state(ctx: &TestContext, swap_id: u64) -> SwapState {
    ctx.client.get_swap(&swap_id).state
}

fn balance_of(ctx: &TestContext, asset: &Address, addr: &Address) -> i128 {
    ctx.env.invoke_contract::<i128>(
        asset,
        &symbol_short!("balance"),
        soroban_sdk::vec![&ctx.env, addr.to_val()],
    )
}

// ══════════════════════════════════════════════════════════════
//  TESTS
// ══════════════════════════════════════════════════════════════

// ── 1. Happy path: full lifecycle ────────────────────────────

#[test]
fn full_swap_lifecycle() {
    let ctx = setup();

    let swap_id = create_swap_helper(&ctx, 1, 600);
    assert_eq!(get_swap_state(&ctx, swap_id), SwapState::Created);

    fund_creator(&ctx, swap_id);
    // State stays Created until both parties fund
    assert_eq!(get_swap_state(&ctx, swap_id), SwapState::Created);

    fund_counterparty(&ctx, swap_id);
    // Now both have funded → state transitions to Funded
    assert_eq!(get_swap_state(&ctx, swap_id), SwapState::Funded);

    accept_swap_helper(&ctx, swap_id);
    assert_eq!(get_swap_state(&ctx, swap_id), SwapState::Accepted);

    // Verify tokens moved
    assert_eq!(balance_of(&ctx, &ctx.asset_a, &ctx.counterparty), 100);
    assert_eq!(balance_of(&ctx, &ctx.asset_b, &ctx.creator), 200);
    println!("✓ full_swap_lifecycle passed");
}

// ── 2. Cancel before any funding ─────────────────────────────

#[test]
fn cancel_unfunded_swap() {
    let ctx = setup();

    let swap_id = create_swap_helper(&ctx, 1, 600);
    ctx.client.cancel_swap(&swap_id);
    assert_eq!(get_swap_state(&ctx, swap_id), SwapState::Cancelled);
    println!("✓ cancel_unfunded_swap passed");
}

// ── 3. Refund after expiry ───────────────────────────────────

#[test]
fn refund_after_expiry() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);

    fund_creator(&ctx, swap_id);
    fund_counterparty(&ctx, swap_id);

    let now = ctx.env.ledger().timestamp();
    ctx.env.ledger().set_timestamp(now + 601);

    ctx.client.refund_swap(&swap_id);
    assert_eq!(get_swap_state(&ctx, swap_id), SwapState::Refunded);

    // Verify tokens returned
    assert_eq!(balance_of(&ctx, &ctx.asset_a, &ctx.creator), 100_000);
    assert_eq!(balance_of(&ctx, &ctx.asset_b, &ctx.counterparty), 100_000);
    println!("✓ refund_after_expiry passed");
}

// ── 4. Accept expired swap fails ─────────────────────────────

#[test]
fn accept_expired_swap_fails() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);

    fund_creator(&ctx, swap_id);
    fund_counterparty(&ctx, swap_id);

    // Advance past expiry
    let now = ctx.env.ledger().timestamp();
    ctx.env.ledger().set_timestamp(now + 601);

    let result = ctx.client.try_accept_swap(&swap_id, &ctx.counterparty);
    assert!(result.is_err());
    println!("✓ accept_expired_swap_fails passed");
}

// ── 5. Creator cannot cancel if counterparty funded ──────────

#[test]
fn cancel_after_cp_funded_fails() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);

    fund_counterparty(&ctx, swap_id);

    let result = ctx.client.try_cancel_swap(&swap_id);
    assert!(result.is_err());
    println!("✓ cancel_after_cp_funded_fails passed");
}

// ── 6. Creator cannot cancel after self-funding ──────────────

#[test]
fn cancel_after_self_funded_fails() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);

    fund_creator(&ctx, swap_id);

    let result = ctx.client.try_cancel_swap(&swap_id);
    assert!(result.is_err());
    println!("✓ cancel_after_self_funded_fails passed");
}

// ── 7. Zero amount rejected ──────────────────────────────────

#[test]
fn zero_amount_rejected() {
    let ctx = setup();
    let now = ctx.env.ledger().timestamp();

    let result = ctx.client.try_create_swap(
        &ctx.creator,
        &ctx.counterparty,
        &ctx.asset_a,
        &0i128,
        &ctx.asset_b,
        &200i128,
        &(now + 600),
        &1u64,
    );
    assert!(result.is_err());
    println!("✓ zero_amount_rejected passed");
}

// ── 8. Negative amount rejected ──────────────────────────────

#[test]
fn negative_amount_rejected() {
    let ctx = setup();
    let now = ctx.env.ledger().timestamp();

    let result = ctx.client.try_create_swap(
        &ctx.creator,
        &ctx.counterparty,
        &ctx.asset_a,
        &-5i128,
        &ctx.asset_b,
        &200i128,
        &(now + 600),
        &1u64,
    );
    assert!(result.is_err());
    println!("✓ negative_amount_rejected passed");
}

// ── 9. Same asset pair rejected ──────────────────────────────

#[test]
fn same_asset_rejected() {
    let ctx = setup();
    let now = ctx.env.ledger().timestamp();

    let result = ctx.client.try_create_swap(
        &ctx.creator,
        &ctx.counterparty,
        &ctx.asset_a,
        &100i128,
        &ctx.asset_a,
        &200i128,
        &(now + 600),
        &1u64,
    );
    assert!(result.is_err());
    println!("✓ same_asset_rejected passed");
}

// ── 10. Self-swap rejected ───────────────────────────────────

#[test]
fn self_swap_rejected() {
    let ctx = setup();
    let now = ctx.env.ledger().timestamp();

    let result = ctx.client.try_create_swap(
        &ctx.creator,
        &ctx.creator,
        &ctx.asset_a,
        &100i128,
        &ctx.asset_b,
        &200i128,
        &(now + 600),
        &1u64,
    );
    assert!(result.is_err());
    println!("✓ self_swap_rejected passed");
}

// ── 11. Expiry too soon rejected ─────────────────────────────

#[test]
fn expiry_too_soon_rejected() {
    let ctx = setup();
    let now = ctx.env.ledger().timestamp();

    let result = ctx.client.try_create_swap(
        &ctx.creator,
        &ctx.counterparty,
        &ctx.asset_a,
        &100i128,
        &ctx.asset_b,
        &200i128,
        &(now + 100),
        &1u64,
    );
    assert!(result.is_err());
    println!("✓ expiry_too_soon_rejected passed");
}

// ── 12. Unauthorized funder rejected ─────────────────────────

#[test]
fn unauthorized_funder_rejected() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);
    let random = Address::generate(&ctx.env);

    let result = ctx.client.try_fund_swap(&swap_id, &random);
    assert!(result.is_err());
    println!("✓ unauthorized_funder_rejected passed");
}

// ── 13. Wrong acceptor rejected ──────────────────────────────

#[test]
fn wrong_acceptor_rejected() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);

    fund_creator(&ctx, swap_id);
    fund_counterparty(&ctx, swap_id);

    let random = Address::generate(&ctx.env);
    let result = ctx.client.try_accept_swap(&swap_id, &random);
    assert!(result.is_err());
    println!("✓ wrong_acceptor_rejected passed");
}

// ── 14. Double fund by same party rejected ───────────────────

#[test]
fn double_fund_rejected() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);

    fund_creator(&ctx, swap_id);

    let result = ctx.client.try_fund_swap(&swap_id, &ctx.creator);
    assert!(result.is_err());
    println!("✓ double_fund_rejected passed");
}

// ── 15. Refund before expiry rejected ────────────────────────

#[test]
fn refund_before_expiry_rejected() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);

    fund_creator(&ctx, swap_id);
    fund_counterparty(&ctx, swap_id);

    let result = ctx.client.try_refund_swap(&swap_id);
    assert!(result.is_err());
    println!("✓ refund_before_expiry_rejected passed");
}

// ── 16. Idempotent create_swap (same nonce) ──────────────────

#[test]
fn idempotent_create_swap() {
    let ctx = setup();
    let id1 = create_swap_helper(&ctx, 42, 600);
    let id2 = create_swap_helper(&ctx, 42, 600);
    assert_eq!(id1, id2);

    let id3 = create_swap_helper(&ctx, 99, 600);
    assert_ne!(id1, id3);
    println!("✓ idempotent_create_swap passed");
}

// ── 17. Accept before both funded rejected ───────────────────

#[test]
fn accept_partial_fund_rejected() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);

    fund_creator(&ctx, swap_id);

    let result = ctx.client.try_accept_swap(&swap_id, &ctx.counterparty);
    assert!(result.is_err());
    println!("✓ accept_partial_fund_rejected passed");
}

// ── 18. Multiple independent swaps ───────────────────────────

#[test]
fn multiple_swaps_independent() {
    let ctx = setup();

    let id1 = create_swap_helper(&ctx, 1, 600);
    let id2 = create_swap_helper(&ctx, 2, 600);

    assert_ne!(id1, id2);
    assert_eq!(get_swap_state(&ctx, id1), SwapState::Created);
    assert_eq!(get_swap_state(&ctx, id2), SwapState::Created);

    ctx.client.cancel_swap(&id1);
    assert_eq!(get_swap_state(&ctx, id1), SwapState::Cancelled);
    assert_eq!(get_swap_state(&ctx, id2), SwapState::Created);
    println!("✓ multiple_swaps_independent passed");
}

// ── 19. Non-existent swap returns error ──────────────────────

#[test]
fn non_existent_swap_returns_error() {
    let ctx = setup();

    let result = ctx.client.try_get_swap(&9999u64);
    assert!(result.is_err());
    println!("✓ non_existent_swap_returns_error passed");
}

// ── 20. Partial refund (only one party funded) ───────────────

#[test]
fn partial_refund_one_party() {
    let ctx = setup();
    let swap_id = create_swap_helper(&ctx, 1, 600);

    fund_creator(&ctx, swap_id);

    let now = ctx.env.ledger().timestamp();
    ctx.env.ledger().set_timestamp(now + 601);

    ctx.client.refund_swap(&swap_id);
    assert_eq!(get_swap_state(&ctx, swap_id), SwapState::Refunded);

    assert_eq!(balance_of(&ctx, &ctx.asset_a, &ctx.creator), 100_000);
    println!("✓ partial_refund_one_party passed");
}
