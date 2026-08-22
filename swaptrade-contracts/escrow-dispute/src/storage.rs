use soroban_sdk::{symbol_short, Address, Env, IntoVal, Map, Symbol, Vec};

use crate::errors::EscrowError;
use crate::types::{Dispute, DisputeEvidence, Escrow, DisputeVote};

// ── Storage keys ──────────────────────────────────────────
const ESCROWS_KEY: Symbol = symbol_short!("escrows");
const DISPUTES_KEY: Symbol = symbol_short!("disputes");
const EVIDENCE_KEY: Symbol = symbol_short!("evidence");
const VOTES_KEY: Symbol = symbol_short!("votes");
const NONCE_KEY: Symbol = symbol_short!("nonce");
const NEXT_ID_KEY: Symbol = symbol_short!("nxid");
const SIGNERS_KEY: Symbol = symbol_short!("signers");
const THRESHOLD_KEY: Symbol = symbol_short!("thresh");

/// Default minimum timelock duration: 1 hour.
const DEFAULT_MIN_TIMELOCK: u64 = 3600;

/// Default dispute resolution window: 7 days.
const DEFAULT_DISPUTE_WINDOW: u64 = 604800;

// ── Escrow CRUD ───────────────────────────────────────────

fn escrow_map(env: &Env) -> Map<u64, Escrow> {
    env.storage()
        .persistent()
        .get(&ESCROWS_KEY)
        .unwrap_or_else(|| Map::new(env))
}

fn save_escrow_map(env: &Env, map: &Map<u64, Escrow>) {
    env.storage().persistent().set(&ESCROWS_KEY, map);
}

fn nonce_index(env: &Env) -> Map<(Address, u64), u64> {
    env.storage()
        .persistent()
        .get(&NONCE_KEY)
        .unwrap_or_else(|| Map::new(env))
}

fn save_nonce_index(env: &Env, map: &Map<(Address, u64), u64>) {
    env.storage().persistent().set(&NONCE_KEY, map);
}

/// Allocate the next escrow ID.
pub fn next_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&NEXT_ID_KEY)
        .unwrap_or(1u64);
    env.storage().persistent().set(&NEXT_ID_KEY, &(id + 1));
    id
}

/// Find an existing escrow by (seller, nonce) for idempotency.
pub fn find_by_nonce(env: &Env, seller: &Address, nonce: u64) -> Option<u64> {
    let idx = nonce_index(env);
    idx.get((seller.clone(), nonce))
}

/// Persist a new escrow and update the nonce index.
pub fn save_escrow(env: &Env, escrow: &Escrow) {
    let mut map = escrow_map(env);
    map.set(escrow.id, escrow.clone());
    save_escrow_map(env, &map);

    let mut idx = nonce_index(env);
    idx.set((escrow.seller.clone(), escrow.nonce), escrow.id);
    save_nonce_index(env, &idx);
}

/// Load an escrow by ID.
pub fn load_escrow(env: &Env, id: u64) -> Result<Escrow, EscrowError> {
    let map = escrow_map(env);
    map.get(id).ok_or(EscrowError::EscrowNotFound)
}

/// Update an existing escrow in storage.
pub fn update_escrow(env: &Env, escrow: &Escrow) {
    let mut map = escrow_map(env);
    map.set(escrow.id, escrow.clone());
    save_escrow_map(env, &map);
}

// ── Dispute CRUD ──────────────────────────────────────────

fn dispute_map(env: &Env) -> Map<u64, Dispute> {
    env.storage()
        .persistent()
        .get(&DISPUTES_KEY)
        .unwrap_or_else(|| Map::new(env))
}

fn save_dispute_map(env: &Env, map: &Map<u64, Dispute>) {
    env.storage().persistent().set(&DISPUTES_KEY, map);
}

/// Persist a dispute record.
pub fn save_dispute(env: &Env, dispute: &Dispute) {
    let mut map = dispute_map(env);
    map.set(dispute.escrow_id, dispute.clone());
    save_dispute_map(env, &map);
}

/// Load a dispute by escrow ID.
pub fn load_dispute(env: &Env, escrow_id: u64) -> Result<Dispute, EscrowError> {
    let map = dispute_map(env);
    map.get(escrow_id).ok_or(EscrowError::EscrowNotFound)
}

/// Update a dispute record.
pub fn update_dispute(env: &Env, dispute: &Dispute) {
    let mut map = dispute_map(env);
    map.set(dispute.escrow_id, dispute.clone());
    save_dispute_map(env, &map);
}

// ── Evidence CRUD ─────────────────────────────────────────

fn evidence_map(env: &Env) -> Map<u64, Vec<DisputeEvidence>> {
    env.storage()
        .persistent()
        .get(&EVIDENCE_KEY)
        .unwrap_or_else(|| Map::new(env))
}

fn save_evidence_map(env: &Env, map: &Map<u64, Vec<DisputeEvidence>>) {
    env.storage().persistent().set(&EVIDENCE_KEY, map);
}

/// Append a piece of evidence to a dispute's evidence list.
pub fn append_evidence(env: &Env, escrow_id: u64, evidence: &DisputeEvidence) {
    let mut map = evidence_map(env);
    let mut list = map.get(escrow_id).unwrap_or_else(|| Vec::new(env));
    list.push_back(evidence.clone());
    map.set(escrow_id, list);
    save_evidence_map(env, &map);
}

/// Retrieve all evidence for a dispute.
pub fn get_evidence(env: &Env, escrow_id: u64) -> Vec<DisputeEvidence> {
    let map = evidence_map(env);
    map.get(escrow_id).unwrap_or_else(|| Vec::new(env))
}

// ── Vote tracking ─────────────────────────────────────────

fn vote_map(env: &Env) -> Map<u64, Vec<DisputeVote>> {
    env.storage()
        .persistent()
        .get(&VOTES_KEY)
        .unwrap_or_else(|| Map::new(env))
}

fn save_vote_map(env: &Env, map: &Map<u64, Vec<DisputeVote>>) {
    env.storage().persistent().set(&VOTES_KEY, map);
}

/// Record a vote on a dispute. Returns Err if the signer already voted.
pub fn record_vote(
    env: &Env,
    escrow_id: u64,
    vote: &DisputeVote,
) -> Result<(), EscrowError> {
    let mut map = vote_map(env);
    let mut votes = map.get(escrow_id).unwrap_or_else(|| Vec::new(env));

    // Check for duplicate vote
    for v in votes.iter() {
        if v.signer == vote.signer {
            return Err(EscrowError::DuplicateVote);
        }
    }

    votes.push_back(vote.clone());
    map.set(escrow_id, votes);
    save_vote_map(env, &map);
    Ok(())
}

/// Retrieve all votes for a dispute.
pub fn get_votes(env: &Env, escrow_id: u64) -> Vec<DisputeVote> {
    let map = vote_map(env);
    map.get(escrow_id).unwrap_or_else(|| Vec::new(env))
}

/// Count votes in favour of release for a dispute.
pub fn count_release_votes(env: &Env, escrow_id: u64) -> u32 {
    let votes = get_votes(env, escrow_id);
    let mut count = 0u32;
    for v in votes.iter() {
        if v.in_favour_of_release {
            count += 1;
        }
    }
    count
}

/// Count votes in favour of refund for a dispute.
pub fn count_refund_votes(env: &Env, escrow_id: u64) -> u32 {
    let votes = get_votes(env, escrow_id);
    let mut count = 0u32;
    for v in votes.iter() {
        if !v.in_favour_of_release {
            count += 1;
        }
    }
    count
}

// ── Multisig signers & threshold ──────────────────────────

/// Store the list of multisig signers.
pub fn store_signers(env: &Env, signers: &Vec<Address>) {
    env.storage().persistent().set(&SIGNERS_KEY, signers);
}

/// Load the list of multisig signers.
pub fn load_signers(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&SIGNERS_KEY)
        .unwrap_or_else(|| Vec::new(env))
}

/// Store the multisig threshold.
pub fn store_threshold(env: &Env, threshold: u32) {
    env.storage().persistent().set(&THRESHOLD_KEY, &threshold);
}

/// Load the multisig threshold.
pub fn load_threshold(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&THRESHOLD_KEY)
        .unwrap_or(0)
}

/// Check if an address is a registered multisig signer.
pub fn is_signer(env: &Env, address: &Address) -> bool {
    let signers = load_signers(env);
    for s in signers.iter() {
        if s == *address {
            return true;
        }
    }
    false
}

// ── Config ────────────────────────────────────────────────

const MIN_TIMELOCK_KEY: Symbol = symbol_short!("mintlck");
const DISPUTE_WINDOW_KEY: Symbol = symbol_short!("dsptwin");

/// Get minimum timelock duration (seconds). Default: 3600 (1 hour).
pub fn min_timelock(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&MIN_TIMELOCK_KEY)
        .unwrap_or(DEFAULT_MIN_TIMELOCK)
}

/// Set minimum timelock duration.
pub fn set_min_timelock(env: &Env, seconds: u64) {
    env.storage()
        .persistent()
        .set(&MIN_TIMELOCK_KEY, &seconds);
}

/// Get dispute resolution window (seconds). Default: 604800 (7 days).
pub fn dispute_window(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DISPUTE_WINDOW_KEY)
        .unwrap_or(DEFAULT_DISPUTE_WINDOW)
}

/// Set dispute resolution window.
pub fn set_dispute_window(env: &Env, seconds: u64) {
    env.storage()
        .persistent()
        .set(&DISPUTE_WINDOW_KEY, &seconds);
}

// ── Trustline & token helpers ─────────────────────────────

/// Returns `true` if `address` holds a trustline for `asset`.
pub fn has_trustline(env: &Env, address: &Address, asset: &Address) -> bool {
    let args: Vec<soroban_sdk::Val> = Vec::from_array(env, [address.to_val()]);
    let result =
        env.try_invoke_contract::<i128, EscrowError>(asset, &Symbol::new(env, "balance"), args);
    matches!(result, Ok(Ok(_)))
}

/// Internal transfer helper. Does NOT call `require_auth`.
fn invoke_transfer(
    env: &Env,
    asset: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<i128, EscrowError> {
    let args: Vec<soroban_sdk::Val> =
        soroban_sdk::vec![env, from.to_val(), to.to_val(), amount.into_val(env),];
    let result = env.try_invoke_contract::<i128, EscrowError>(
        asset,
        &Symbol::new(env, "transfer"),
        args,
    );
    match result {
        Ok(Ok(transferred)) => Ok(transferred),
        Ok(Err(_)) => Err(EscrowError::TransferFailed),
        Err(_) => Err(EscrowError::TransferFailed),
    }
}

/// Transfer tokens with auth on `from`.
pub fn transfer_token(
    env: &Env,
    asset: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<i128, EscrowError> {
    from.require_auth();
    invoke_transfer(env, asset, from, to, amount)
}

/// Transfer tokens without requiring auth (for internal use when auth is
/// satisfied at the entry point).
pub fn transfer_token_no_auth(
    env: &Env,
    asset: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<i128, EscrowError> {
    invoke_transfer(env, asset, from, to, amount)
}
