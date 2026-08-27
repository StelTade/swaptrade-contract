#![cfg(test)]

extern crate std;
use std::println;

use escrow_dispute::{DisputeStatus, EscrowDisputeContract, EscrowState};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{contract, contractimpl, symbol_short, Address, BytesN, Env, IntoVal, Symbol, Vec};

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

    /// Mint tokens to an address (test-only helper).
    pub fn mint(env: Env, to: Address, amount: i128) {
        let mut bal: soroban_sdk::Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&BAL_KEY)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        let cur = bal.get(to.clone()).unwrap_or(0);
        bal.set(to, cur + amount);
        env.storage().persistent().set(&BAL_KEY, &bal);
    }

    pub fn approve(_env: Env, _from: Address, _spender: Address, _amount: i128, _exp: u32) -> i128 {
        0
    }
}

/// Deploy a stub token.
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

/// Mint tokens to an address.
fn mint_tokens(env: &Env, token: &Address, to: &Address, amount: i128) {
    env.invoke_contract::<()>(
        token,
        &Symbol::new(env, "mint"),
        soroban_sdk::vec![env, to.to_val(), amount.into_val(env)],
    );
}

// ══════════════════════════════════════════════════════════════
//  Test context and helpers
// ══════════════════════════════════════════════════════════════

struct TestContext {
    env: Env,
    seller: Address,
    buyer: Address,
    signer1: Address,
    signer2: Address,
    signer3: Address,
    asset: Address,
    client: escrow_dispute::EscrowDisputeContractClient<'static>,
}

fn setup() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let signer3 = Address::generate(&env);
    let asset = create_token(&env, &seller, 100_000);

    // Give buyer tokens so they can fund escrows
    mint_tokens(&env, &asset, &buyer, 100_000);

    #[allow(deprecated)]
    let contract_id = env.register_contract(None, EscrowDisputeContract);
    let client = escrow_dispute::EscrowDisputeContractClient::new(&env, &contract_id);

    // Initialize with 3 additional signers, threshold = 2, 7-day dispute window
    // Note: seller (admin) is auto-added as signer, so total = 4 signers
    let signers = Vec::from_array(&env, [signer1.clone(), signer2.clone(), signer3.clone()]);
    client.initialize(&seller, &signers, &2, &604800u64);

    TestContext {
        env,
        seller,
        buyer,
        signer1,
        signer2,
        signer3,
        asset,
        client,
    }
}

/// Create an escrow: seller → buyer, asset, amount 100, timelock 86400 (1 day).
fn create_escrow_helper(ctx: &TestContext, nonce: u64) -> u64 {
    ctx.client.create_escrow(
        &ctx.seller,
        &ctx.buyer,
        &ctx.asset,
        &100i128,
        &86400u64,
        &nonce,
    )
}

/// Fund the escrow (buyer deposits).
fn fund_escrow_helper(ctx: &TestContext, escrow_id: u64) {
    ctx.client.fund_escrow(&escrow_id, &ctx.buyer);
}

/// Raise a dispute with a given window.
fn raise_dispute_helper(ctx: &TestContext, escrow_id: u64, disputer: &Address, window: u64) {
    ctx.client.raise_dispute(&escrow_id, disputer, &window);
}

/// Submit evidence for a dispute.
fn submit_evidence_helper(ctx: &TestContext, escrow_id: u64, submitter: &Address, tag: &str) {
    let hash = BytesN::from_array(&ctx.env, &[42u8; 32]);
    let desc = Symbol::new(&ctx.env, tag);
    ctx.client.submit_evidence(&escrow_id, submitter, &hash, &desc);
}

/// Cast a vote on a dispute.
fn vote_helper(ctx: &TestContext, escrow_id: u64, signer: &Address, in_favour: bool) {
    ctx.client.vote(&escrow_id, signer, &in_favour);
}

fn get_escrow_state(ctx: &TestContext, escrow_id: u64) -> EscrowState {
    ctx.client.get_escrow(&escrow_id).state
}

fn get_dispute_status(ctx: &TestContext, escrow_id: u64) -> DisputeStatus {
    ctx.client.get_dispute(&escrow_id).status
}

fn balance_of(ctx: &TestContext, addr: &Address) -> i128 {
    ctx.env.invoke_contract::<i128>(
        &ctx.asset,
        &symbol_short!("balance"),
        soroban_sdk::vec![&ctx.env, addr.to_val()],
    )
}

fn make_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Happy path: full escrow + dispute + release
// ══════════════════════════════════════════════════════════════

#[test]
fn full_escrow_dispute_release_lifecycle() {
    let ctx = setup();

    // 1. Create escrow
    let id = create_escrow_helper(&ctx, 1);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Created);

    // 2. Fund escrow
    fund_escrow_helper(&ctx, id);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Escrowed);

    // Verify buyer balance decreased
    assert_eq!(balance_of(&ctx, &ctx.buyer), 100_000 - 100);

    // 3. Raise dispute
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Disputed);
    assert_eq!(get_dispute_status(&ctx, id), DisputeStatus::Open);

    // 4. Submit evidence
    submit_evidence_helper(&ctx, id, &ctx.seller, "delivery_proof");
    submit_evidence_helper(&ctx, id, &ctx.buyer, "non_receipt");

    // 5. Two signers vote for release
    vote_helper(&ctx, id, &ctx.signer1, true);
    vote_helper(&ctx, id, &ctx.signer2, true);

    // 6. Resolve dispute → release
    ctx.client.resolve_dispute(&id, &ctx.signer1);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Released);
    assert_eq!(get_dispute_status(&ctx, id), DisputeStatus::ResolvedRelease);

    // 7. Verify seller received funds
    assert_eq!(balance_of(&ctx, &ctx.seller), 100_000 + 100);
    println!("✓ full_escrow_dispute_release_lifecycle passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Happy path: full escrow + dispute + refund
// ══════════════════════════════════════════════════════════════

#[test]
fn full_escrow_dispute_refund_lifecycle() {
    let ctx = setup();

    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.buyer, 604800);
    submit_evidence_helper(&ctx, id, &ctx.buyer, "fraud_evidence");

    // Two signers vote for refund
    vote_helper(&ctx, id, &ctx.signer1, false);
    vote_helper(&ctx, id, &ctx.signer2, false);

    // Resolve → refund
    ctx.client.resolve_dispute(&id, &ctx.signer3);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Refunded);
    assert_eq!(get_dispute_status(&ctx, id), DisputeStatus::ResolvedRefund);

    // Buyer gets funds back
    assert_eq!(balance_of(&ctx, &ctx.buyer), 100_000);
    println!("✓ full_escrow_dispute_refund_lifecycle passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Auto-refund after timelock expiry
// ══════════════════════════════════════════════════════════════

#[test]
fn auto_refund_after_deadline() {
    let ctx = setup();

    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);

    // Raise dispute with 1-hour window
    raise_dispute_helper(&ctx, id, &ctx.seller, 3600);

    // Advance past deadline
    let now = ctx.env.ledger().timestamp();
    ctx.env.ledger().set_timestamp(now + 3601);

    // Auto-refund
    ctx.client.auto_refund(&id);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Refunded);
    assert_eq!(get_dispute_status(&ctx, id), DisputeStatus::AutoRefunded);

    // Buyer gets funds back
    assert_eq!(balance_of(&ctx, &ctx.buyer), 100_000);
    println!("✓ auto_refund_after_deadline passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — No funds lost regardless of dispute outcome
// ══════════════════════════════════════════════════════════════

#[test]
fn no_funds_lost_release_outcome() {
    let ctx = setup();
    let buyer_initial = balance_of(&ctx, &ctx.buyer);
    let seller_initial = balance_of(&ctx, &ctx.seller);

    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);

    // Escrow holds 100
    assert_eq!(balance_of(&ctx, &ctx.buyer), buyer_initial - 100);

    // Dispute → release
    raise_dispute_helper(&ctx, id, &ctx.buyer, 604800);
    vote_helper(&ctx, id, &ctx.signer1, true);
    vote_helper(&ctx, id, &ctx.signer2, true);
    ctx.client.resolve_dispute(&id, &ctx.signer3);

    // Seller gets the 100
    assert_eq!(balance_of(&ctx, &ctx.seller), seller_initial + 100);
    assert_eq!(balance_of(&ctx, &ctx.buyer), buyer_initial - 100);
    println!("✓ no_funds_lost_release_outcome passed");
}

#[test]
fn no_funds_lost_refund_outcome() {
    let ctx = setup();
    let buyer_initial = balance_of(&ctx, &ctx.buyer);
    let seller_initial = balance_of(&ctx, &ctx.seller);

    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);

    // Dispute → refund
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);
    vote_helper(&ctx, id, &ctx.signer1, false);
    vote_helper(&ctx, id, &ctx.signer2, false);
    ctx.client.resolve_dispute(&id, &ctx.signer3);

    // Buyer gets the 100 back
    assert_eq!(balance_of(&ctx, &ctx.buyer), buyer_initial);
    assert_eq!(balance_of(&ctx, &ctx.seller), seller_initial);
    println!("✓ no_funds_lost_refund_outcome passed");
}

#[test]
fn no_funds_lost_auto_refund_outcome() {
    let ctx = setup();
    let buyer_initial = balance_of(&ctx, &ctx.buyer);
    let seller_initial = balance_of(&ctx, &ctx.seller);

    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);

    // Dispute → auto-refund
    raise_dispute_helper(&ctx, id, &ctx.seller, 3600);
    let now = ctx.env.ledger().timestamp();
    ctx.env.ledger().set_timestamp(now + 3601);
    ctx.client.auto_refund(&id);

    // Buyer gets the 100 back
    assert_eq!(balance_of(&ctx, &ctx.buyer), buyer_initial);
    assert_eq!(balance_of(&ctx, &ctx.seller), seller_initial);
    println!("✓ no_funds_lost_auto_refund_outcome passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Cancel before funding
// ══════════════════════════════════════════════════════════════

#[test]
fn cancel_unfunded_escrow() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Created);

    ctx.client.cancel_escrow(&id);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Refunded);
    println!("✓ cancel_unfunded_escrow passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Validation errors
// ══════════════════════════════════════════════════════════════

#[test]
fn zero_amount_rejected() {
    let ctx = setup();
    let result = ctx.client.try_create_escrow(
        &ctx.seller,
        &ctx.buyer,
        &ctx.asset,
        &0i128,
        &86400u64,
        &1u64,
    );
    assert!(result.is_err());
    println!("✓ zero_amount_rejected passed");
}

#[test]
fn negative_amount_rejected() {
    let ctx = setup();
    let result = ctx.client.try_create_escrow(
        &ctx.seller,
        &ctx.buyer,
        &ctx.asset,
        &-5i128,
        &86400u64,
        &1u64,
    );
    assert!(result.is_err());
    println!("✓ negative_amount_rejected passed");
}

#[test]
fn self_escrow_rejected() {
    let ctx = setup();
    let result = ctx.client.try_create_escrow(
        &ctx.seller,
        &ctx.seller,
        &ctx.asset,
        &100i128,
        &86400u64,
        &1u64,
    );
    assert!(result.is_err());
    println!("✓ self_escrow_rejected passed");
}

#[test]
fn timelock_too_short_rejected() {
    let ctx = setup();
    // Default min_timelock is 3600, so 100 is too short
    let result = ctx.client.try_create_escrow(
        &ctx.seller,
        &ctx.buyer,
        &ctx.asset,
        &100i128,
        &100u64,
        &1u64,
    );
    assert!(result.is_err());
    println!("✓ timelock_too_short_rejected passed");
}

#[test]
fn unauthorized_funder_rejected() {
    let ctx = setup();
    let random = Address::generate(&ctx.env);
    let id = create_escrow_helper(&ctx, 1);

    let result = ctx.client.try_fund_escrow(&id, &random);
    assert!(result.is_err());
    println!("✓ unauthorized_funder_rejected passed");
}

#[test]
fn dispute_on_unfunded_escrow_rejected() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);

    let result = ctx.client.try_raise_dispute(&id, &ctx.seller, &604800u64);
    assert!(result.is_err());
    println!("✓ dispute_on_unfunded_escrow_rejected passed");
}

#[test]
fn dispute_by_non_party_rejected() {
    let ctx = setup();
    let random = Address::generate(&ctx.env);
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);

    let result = ctx.client.try_raise_dispute(&id, &random, &604800u64);
    assert!(result.is_err());
    println!("✓ dispute_by_non_party_rejected passed");
}

#[test]
fn cancel_funded_escrow_rejected() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);

    let result = ctx.client.try_cancel_escrow(&id);
    assert!(result.is_err());
    println!("✓ cancel_funded_escrow_rejected passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Auto-refund before deadline rejected
// ══════════════════════════════════════════════════════════════

#[test]
fn auto_refund_before_deadline_rejected() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);

    let result = ctx.client.try_auto_refund(&id);
    assert!(result.is_err());
    println!("✓ auto_refund_before_deadline_rejected passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Resolve before threshold met rejected
// ══════════════════════════════════════════════════════════════

#[test]
fn resolve_insufficient_signatures_rejected() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);

    // Only 1 vote (threshold is 2)
    vote_helper(&ctx, id, &ctx.signer1, true);

    let result = ctx.client.try_resolve_dispute(&id, &ctx.signer1);
    assert!(result.is_err());
    println!("✓ resolve_insufficient_signatures_rejected passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Duplicate vote rejected
// ══════════════════════════════════════════════════════════════

#[test]
fn duplicate_vote_rejected() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);

    vote_helper(&ctx, id, &ctx.signer1, true);
    let result = ctx.client.try_vote(&id, &ctx.signer1, &true);
    assert!(result.is_err());
    println!("✓ duplicate_vote_rejected passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Non-signer vote rejected
// ══════════════════════════════════════════════════════════════

#[test]
fn non_signer_vote_rejected() {
    let ctx = setup();
    let random = Address::generate(&ctx.env);
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);

    let result = ctx.client.try_vote(&id, &random, &true);
    assert!(result.is_err());
    println!("✓ non_signer_vote_rejected passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Evidence submission
// ══════════════════════════════════════════════════════════════

#[test]
fn evidence_submission_tracking() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);

    submit_evidence_helper(&ctx, id, &ctx.seller, "proof_a");
    submit_evidence_helper(&ctx, id, &ctx.buyer, "proof_b");

    let evidence = ctx.client.get_evidence(&id);
    assert_eq!(evidence.len(), 2);
    println!("✓ evidence_submission_tracking passed");
}

#[test]
fn evidence_on_resolved_dispute_rejected() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);
    vote_helper(&ctx, id, &ctx.signer1, true);
    vote_helper(&ctx, id, &ctx.signer2, true);
    ctx.client.resolve_dispute(&id, &ctx.signer1);

    let result = ctx.client.try_submit_evidence(
        &id,
        &ctx.seller,
        &make_hash(&ctx.env, 1),
        &Symbol::new(&ctx.env, "late"),
    );
    assert!(result.is_err());
    println!("✓ evidence_on_resolved_dispute_rejected passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Idempotent create_escrow
// ══════════════════════════════════════════════════════════════

#[test]
fn idempotent_create_escrow() {
    let ctx = setup();
    let id1 = create_escrow_helper(&ctx, 42);
    let id2 = create_escrow_helper(&ctx, 42);
    assert_eq!(id1, id2);

    let id3 = create_escrow_helper(&ctx, 99);
    assert_ne!(id1, id3);
    println!("✓ idempotent_create_escrow passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Vote counts
// ══════════════════════════════════════════════════════════════

#[test]
fn vote_counts_tracking() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);

    vote_helper(&ctx, id, &ctx.signer1, true);
    vote_helper(&ctx, id, &ctx.signer2, false);
    vote_helper(&ctx, id, &ctx.signer3, true);

    assert_eq!(ctx.client.get_release_vote_count(&id), 2);
    assert_eq!(ctx.client.get_refund_vote_count(&id), 1);
    println!("✓ vote_counts_tracking passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Signer queries
// ══════════════════════════════════════════════════════════════

#[test]
fn signer_queries() {
    let ctx = setup();

    // seller (admin) is auto-added as signer, plus 3 explicit signers = 4 total
    assert!(ctx.client.is_signer(&ctx.seller));
    assert!(ctx.client.is_signer(&ctx.signer1));
    assert!(ctx.client.is_signer(&ctx.signer2));
    assert!(ctx.client.is_signer(&ctx.signer3));

    let random = Address::generate(&ctx.env);
    assert!(!ctx.client.is_signer(&random));

    let signers = ctx.client.get_signers();
    assert_eq!(signers.len(), 4);
    assert_eq!(ctx.client.get_threshold(), 2);
    println!("✓ signer_queries passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Multiple independent escrows
// ══════════════════════════════════════════════════════════════

#[test]
fn multiple_escrows_independent() {
    let ctx = setup();

    let id1 = create_escrow_helper(&ctx, 1);
    let id2 = create_escrow_helper(&ctx, 2);

    assert_ne!(id1, id2);
    assert_eq!(get_escrow_state(&ctx, id1), EscrowState::Created);
    assert_eq!(get_escrow_state(&ctx, id2), EscrowState::Created);

    fund_escrow_helper(&ctx, id1);
    assert_eq!(get_escrow_state(&ctx, id1), EscrowState::Escrowed);
    assert_eq!(get_escrow_state(&ctx, id2), EscrowState::Created);
    println!("✓ multiple_escrows_independent passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Non-existent escrow returns error
// ══════════════════════════════════════════════════════════════

#[test]
fn non_existent_escrow_returns_error() {
    let ctx = setup();
    let result = ctx.client.try_get_escrow(&9999u64);
    assert!(result.is_err());
    println!("✓ non_existent_escrow_returns_error passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Mixed vote: release wins over refund
// ══════════════════════════════════════════════════════════════

#[test]
fn mixed_votes_release_wins() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);

    // 2 release, 1 refund
    vote_helper(&ctx, id, &ctx.signer1, true);
    vote_helper(&ctx, id, &ctx.signer2, true);
    vote_helper(&ctx, id, &ctx.signer3, false);

    ctx.client.resolve_dispute(&id, &ctx.signer1);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Released);
    println!("✓ mixed_votes_release_wins passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Mixed vote: refund wins over release
// ══════════════════════════════════════════════════════════════

#[test]
fn mixed_votes_refund_wins() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.buyer, 604800);

    // 1 release, 2 refund
    vote_helper(&ctx, id, &ctx.signer1, true);
    vote_helper(&ctx, id, &ctx.signer2, false);
    vote_helper(&ctx, id, &ctx.signer3, false);

    ctx.client.resolve_dispute(&id, &ctx.signer1);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Refunded);
    println!("✓ mixed_votes_refund_wins passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Double resolve rejected
// ══════════════════════════════════════════════════════════════

#[test]
fn double_resolve_rejected() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);

    vote_helper(&ctx, id, &ctx.signer1, true);
    vote_helper(&ctx, id, &ctx.signer2, true);
    ctx.client.resolve_dispute(&id, &ctx.signer1);

    let result = ctx.client.try_resolve_dispute(&id, &ctx.signer2);
    assert!(result.is_err());
    println!("✓ double_resolve_rejected passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Buyer can also raise dispute
// ══════════════════════════════════════════════════════════════

#[test]
fn buyer_can_raise_dispute() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);

    // Buyer raises dispute (not seller)
    raise_dispute_helper(&ctx, id, &ctx.buyer, 604800);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Disputed);

    let dispute = ctx.client.get_dispute(&id);
    assert_eq!(dispute.raised_by, ctx.buyer);
    println!("✓ buyer_can_raise_dispute passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Unanimous release (all 4 signers, threshold = 2)
// ══════════════════════════════════════════════════════════════

#[test]
fn unanimous_release() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);

    vote_helper(&ctx, id, &ctx.signer1, true);
    vote_helper(&ctx, id, &ctx.signer2, true);
    vote_helper(&ctx, id, &ctx.signer3, true);

    ctx.client.resolve_dispute(&id, &ctx.signer1);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Released);
    assert_eq!(balance_of(&ctx, &ctx.seller), 100_000 + 100);
    println!("✓ unanimous_release passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Seller cannot fund own escrow
// ══════════════════════════════════════════════════════════════

#[test]
fn seller_cannot_fund_own_escrow() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);

    // Only buyer can fund
    let result = ctx.client.try_fund_escrow(&id, &ctx.seller);
    assert!(result.is_err());
    println!("✓ seller_cannot_fund_own_escrow passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Double fund rejected
// ══════════════════════════════════════════════════════════════

#[test]
fn double_fund_rejected() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);

    let result = ctx.client.try_fund_escrow(&id, &ctx.buyer);
    assert!(result.is_err());
    println!("✓ double_fund_rejected passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Config queries
// ══════════════════════════════════════════════════════════════

#[test]
fn config_queries() {
    let ctx = setup();
    assert_eq!(ctx.client.get_dispute_window(), 604800);
    assert_eq!(ctx.client.get_min_timelock(), 3600);
    println!("✓ config_queries passed");
}

// ══════════════════════════════════════════════════════════════
//  TESTS — Escrow without dispute remains in Escrowed state
// ══════════════════════════════════════════════════════════════

#[test]
fn escrow_with_no_dispute_remains_escaped() {
    let ctx = setup();
    let id = create_escrow_helper(&ctx, 1);
    fund_escrow_helper(&ctx, id);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Escrowed);

    // Can still raise dispute later
    raise_dispute_helper(&ctx, id, &ctx.seller, 604800);
    assert_eq!(get_escrow_state(&ctx, id), EscrowState::Disputed);
    println!("✓ escrow_with_no_dispute_remains_escaped passed");
}
