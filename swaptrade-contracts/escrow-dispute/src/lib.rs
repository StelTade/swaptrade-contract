#![cfg_attr(all(not(test), target_family = "wasm"), no_std)]
#![allow(clippy::all)]
#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    deprecated,
    unused_doc_comments,
    unused_mut
)]

extern crate alloc;

mod errors;
mod events;
mod storage;
mod types;

pub use errors::EscrowError;
pub use types::{Dispute, DisputeEvidence, DisputeStatus, DisputeVote, Escrow, EscrowState};

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol, Vec};

/// Soroban escrow contract with time-locked dispute resolution.
///
/// Provides a trusted-escrow pattern with:
/// 1. Seller creates escrow defining terms (asset, amount, timelock).
/// 2. Buyer funds the escrow.
/// 3. Either party can raise a dispute, freezing funds.
/// 4. Evidence is submitted off-chain (IPFS/Arweave) with on-chain hash references.
/// 5. Multisig signers resolve the dispute (release to seller or refund to buyer).
/// 6. If no resolution within the dispute window, anyone can trigger auto-refund.
#[contract]
pub struct EscrowDisputeContract;

#[contractimpl]
impl EscrowDisputeContract {
    // ════════════════════════════════════════════════════════
    //  INITIALIZATION
    // ════════════════════════════════════════════════════════

    /// Initialize the contract with multisig signers and threshold.
    ///
    /// # Parameters
    /// * `admin` – the deploying admin (requires auth, also becomes a signer)
    /// * `signers` – list of multisig signer addresses (admin is always included)
    /// * `threshold` – minimum votes required to resolve a dispute
    /// * `timelock_duration` – default dispute resolution window in seconds
    pub fn initialize(
        env: Env,
        admin: Address,
        signers: Vec<Address>,
        threshold: u32,
        timelock_duration: u64,
    ) {
        admin.require_auth();

        if threshold == 0 {
            panic!("threshold must be > 0");
        }

        let mut all_signers = Vec::new(&env);
        all_signers.push_back(admin.clone());

        // Add additional signers (skip admin if already included)
        for s in signers.iter() {
            if s != admin {
                all_signers.push_back(s);
            }
        }

        if threshold > all_signers.len() as u32 {
            panic!("threshold exceeds signer count");
        }

        storage::store_signers(&env, &all_signers);
        storage::store_threshold(&env, threshold);
        storage::set_dispute_window(&env, timelock_duration);
    }

    // ════════════════════════════════════════════════════════
    //  CREATE ESCROW
    // ════════════════════════════════════════════════════════

    /// Create a new escrow agreement.
    ///
    /// # Parameters
    /// * `seller` – the party creating the escrow (requires auth)
    /// * `buyer` – the party who will fund the escrow
    /// * `asset` – Stellar asset contract address held in escrow
    /// * `amount` – amount of the asset (must be > 0)
    /// * `timelock` – seconds after creation before the escrow expires
    /// * `nonce` – client-supplied nonce for idempotency
    ///
    /// # Returns
    /// The unique escrow ID.
    pub fn create_escrow(
        env: Env,
        seller: Address,
        buyer: Address,
        asset: Address,
        amount: i128,
        timelock: u64,
        nonce: u64,
    ) -> Result<u64, EscrowError> {
        seller.require_auth();

        // ── Validation ─────────────────────────────────────
        if amount <= 0 {
            return Err(EscrowError::InvalidAmount);
        }
        if seller == buyer {
            return Err(EscrowError::Unauthorized);
        }

        let now = env.ledger().timestamp();
        let min_tl = storage::min_timelock(&env);
        if timelock < min_tl {
            return Err(EscrowError::InvalidTimelock);
        }

        // Seller must hold a trustline for the asset
        if !storage::has_trustline(&env, &seller, &asset) {
            return Err(EscrowError::MissingTrustline);
        }

        // ── Idempotency ────────────────────────────────────
        if let Some(existing_id) = storage::find_by_nonce(&env, &seller, nonce) {
            return Ok(existing_id);
        }

        // ── Persist ────────────────────────────────────────
        let id = storage::next_id(&env);
        let escrow = Escrow {
            id,
            nonce,
            seller: seller.clone(),
            buyer: buyer.clone(),
            asset: asset.clone(),
            amount,
            state: EscrowState::Created,
            created_at: now,
        };
        storage::save_escrow(&env, &escrow);
        events::escrow_created(&env, &escrow);

        Ok(id)
    }

    // ════════════════════════════════════════════════════════
    //  FUND ESCROW
    // ════════════════════════════════════════════════════════

    /// Fund the escrow — buyer deposits assets into escrow.
    ///
    /// # Parameters
    /// * `escrow_id` – the escrow to fund
    /// * `funder` – must be the buyer (requires auth)
    pub fn fund_escrow(env: Env, escrow_id: u64, funder: Address) -> Result<(), EscrowError> {
        funder.require_auth();

        let mut escrow = storage::load_escrow(&env, escrow_id)?;

        if funder != escrow.buyer {
            return Err(EscrowError::Unauthorized);
        }
        if escrow.state != EscrowState::Created {
            return Err(EscrowError::InvalidState);
        }

        // ── Trustline check ────────────────────────────────
        if !storage::has_trustline(&env, &funder, &escrow.asset) {
            return Err(EscrowError::MissingTrustline);
        }

        // ── Transfer buyer → contract (escrow) ──────────────
        let contract_addr = env.current_contract_address();
        storage::transfer_token_no_auth(
            &env,
            &escrow.asset,
            &funder,
            &contract_addr,
            escrow.amount,
        )?;

        escrow.state = EscrowState::Escrowed;
        storage::update_escrow(&env, &escrow);
        events::escrow_funded(&env, &escrow);

        Ok(())
    }

    // ════════════════════════════════════════════════════════
    //  RAISE DISPUTE
    // ════════════════════════════════════════════════════════

    /// Raise a dispute on a funded escrow, freezing the funds.
    ///
    /// Either the seller or buyer can raise a dispute. The funds are
    /// frozen until the dispute is resolved or the timelock expires.
    ///
    /// # Parameters
    /// * `escrow_id` – the escrow to dispute
    /// * `disputer` – the party raising the dispute (requires auth)
    /// * `dispute_window` – seconds until auto-refund is available
    pub fn raise_dispute(
        env: Env,
        escrow_id: u64,
        disputer: Address,
        dispute_window: u64,
    ) -> Result<(), EscrowError> {
        disputer.require_auth();

        let mut escrow = storage::load_escrow(&env, escrow_id)?;

        if !escrow.is_party(&disputer) {
            return Err(EscrowError::Unauthorized);
        }
        if escrow.state != EscrowState::Escrowed {
            return Err(EscrowError::InvalidState);
        }

        let now = env.ledger().timestamp();
        let deadline = now.saturating_add(dispute_window);

        let dispute = Dispute {
            escrow_id,
            raised_by: disputer.clone(),
            status: DisputeStatus::Open,
            raised_at: now,
            deadline,
            evidence_count: 0,
            vote_count: 0,
        };

        escrow.state = EscrowState::Disputed;
        storage::update_escrow(&env, &escrow);
        storage::save_dispute(&env, &dispute);

        events::dispute_raised(&env, &dispute);
        Ok(())
    }

    // ════════════════════════════════════════════════════════
    //  SUBMIT EVIDENCE
    // ════════════════════════════════════════════════════════

    /// Submit evidence for a dispute.
    ///
    /// Evidence is stored as a hash reference — the actual evidence
    /// lives off-chain on IPFS or Arweave. Both parties can submit
    /// evidence multiple times.
    ///
    /// # Parameters
    /// * `escrow_id` – the escrow under dispute
    /// * `submitter` – the party submitting evidence (requires auth)
    /// * `evidence_hash` – SHA-256 hash of the off-chain evidence document
    /// * `description` – short label (e.g., "delivery_receipt", "contract_pdf")
    pub fn submit_evidence(
        env: Env,
        escrow_id: u64,
        submitter: Address,
        evidence_hash: BytesN<32>,
        description: Symbol,
    ) -> Result<(), EscrowError> {
        submitter.require_auth();

        let mut dispute = storage::load_dispute(&env, escrow_id)?;
        let escrow = storage::load_escrow(&env, escrow_id)?;

        if !escrow.is_party(&submitter) {
            return Err(EscrowError::Unauthorized);
        }
        if dispute.status != DisputeStatus::Open {
            return Err(EscrowError::DisputeAlreadyResolved);
        }

        let now = env.ledger().timestamp();

        let evidence = DisputeEvidence {
            hash: evidence_hash,
            submitted_by: submitter,
            submitted_at: now,
            description,
        };

        dispute.evidence_count += 1;
        storage::update_dispute(&env, &dispute);
        storage::append_evidence(&env, escrow_id, &evidence);

        events::evidence_submitted(&env, &dispute, &evidence);
        Ok(())
    }

    // ════════════════════════════════════════════════════════
    //  VOTE (multisig signers)
    // ════════════════════════════════════════════════════════

    /// Cast a vote on a dispute resolution.
    ///
    /// Only registered multisig signers can vote. Each signer can
    /// vote once per dispute. Once the threshold is reached, the
    /// dispute can be resolved via `resolve_dispute`.
    ///
    /// # Parameters
    /// * `escrow_id` – the escrow under dispute
    /// * `signer` – multisig signer casting the vote (requires auth)
    /// * `in_favour_of_release` – true to release funds to seller, false to refund buyer
    pub fn vote(
        env: Env,
        escrow_id: u64,
        signer: Address,
        in_favour_of_release: bool,
    ) -> Result<(), EscrowError> {
        signer.require_auth();

        let dispute = storage::load_dispute(&env, escrow_id)?;

        if dispute.status != DisputeStatus::Open {
            return Err(EscrowError::DisputeAlreadyResolved);
        }
        if !storage::is_signer(&env, &signer) {
            return Err(EscrowError::Unauthorized);
        }

        let now = env.ledger().timestamp();
        let vote = DisputeVote {
            signer,
            in_favour_of_release,
            voted_at: now,
        };

        storage::record_vote(&env, escrow_id, &vote)?;
        Ok(())
    }

    // ════════════════════════════════════════════════════════
    //  RESOLVE DISPUTE (multisig / admin)
    // ════════════════════════════════════════════════════════

    /// Resolve a dispute once the multisig threshold has been met.
    ///
    /// The outcome is determined by the majority vote. If release
    /// votes >= threshold, funds are released to the seller. If refund
    /// votes >= threshold, funds are returned to the buyer. If neither
    /// side has reached the threshold yet, the call fails.
    ///
    /// # Parameters
    /// * `escrow_id` – the escrow under dispute
    /// * `resolver` – any registered signer (requires auth)
    pub fn resolve_dispute(
        env: Env,
        escrow_id: u64,
        resolver: Address,
    ) -> Result<(), EscrowError> {
        resolver.require_auth();

        let mut dispute = storage::load_dispute(&env, escrow_id)?;
        let mut escrow = storage::load_escrow(&env, escrow_id)?;

        if dispute.status != DisputeStatus::Open {
            return Err(EscrowError::DisputeAlreadyResolved);
        }
        if !storage::is_signer(&env, &resolver) {
            return Err(EscrowError::Unauthorized);
        }

        let threshold = storage::load_threshold(&env);
        let release_votes = storage::count_release_votes(&env, escrow_id);
        let refund_votes = storage::count_refund_votes(&env, escrow_id);

        let contract_addr = env.current_contract_address();

        if release_votes >= threshold {
            // ── Release to seller ───────────────────────────
            dispute.status = DisputeStatus::ResolvedRelease;
            escrow.state = EscrowState::Released;

            storage::transfer_token_no_auth(
                &env,
                &escrow.asset,
                &contract_addr,
                &escrow.seller,
                escrow.amount,
            )?;

            storage::update_escrow(&env, &escrow);
            storage::update_dispute(&env, &dispute);
            events::dispute_resolved(&env, &dispute, &resolver);
            events::escrow_released(&env, &escrow);
        } else if refund_votes >= threshold {
            // ── Refund to buyer ─────────────────────────────
            dispute.status = DisputeStatus::ResolvedRefund;
            escrow.state = EscrowState::Refunded;

            storage::transfer_token_no_auth(
                &env,
                &escrow.asset,
                &contract_addr,
                &escrow.buyer,
                escrow.amount,
            )?;

            storage::update_escrow(&env, &escrow);
            storage::update_dispute(&env, &dispute);
            events::dispute_resolved(&env, &dispute, &resolver);
            events::escrow_refunded(&env, &escrow);
        } else {
            return Err(EscrowError::InsufficientSignatures);
        }

        Ok(())
    }

    // ════════════════════════════════════════════════════════
    //  AUTO-REFUND (timelock expiry)
    // ════════════════════════════════════════════════════════

    /// Trigger automatic refund when a dispute has not been resolved
    /// within its timelock window.
    ///
    /// Anyone can call this — no auth required beyond the transaction
    /// itself. This ensures funds are never permanently locked.
    ///
    /// # Parameters
    /// * `escrow_id` – the escrow with an unresolved dispute
    pub fn auto_refund(env: Env, escrow_id: u64) -> Result<(), EscrowError> {
        let mut dispute = storage::load_dispute(&env, escrow_id)?;
        let mut escrow = storage::load_escrow(&env, escrow_id)?;

        if dispute.status != DisputeStatus::Open {
            return Err(EscrowError::DisputeAlreadyResolved);
        }

        let now = env.ledger().timestamp();
        if now < dispute.deadline {
            return Err(EscrowError::DeadlineNotReached);
        }

        // ── Auto-refund to buyer ───────────────────────────
        dispute.status = DisputeStatus::AutoRefunded;
        escrow.state = EscrowState::Refunded;

        let contract_addr = env.current_contract_address();
        storage::transfer_token_no_auth(
            &env,
            &escrow.asset,
            &contract_addr,
            &escrow.buyer,
            escrow.amount,
        )?;

        storage::update_escrow(&env, &escrow);
        storage::update_dispute(&env, &dispute);

        events::dispute_auto_refunded(&env, &dispute);
        events::escrow_refunded(&env, &escrow);

        Ok(())
    }

    // ════════════════════════════════════════════════════════
    //  CANCEL ESCROW (before funding)
    // ════════════════════════════════════════════════════════

    /// Cancel an escrow that has not yet been funded.
    ///
    /// Only the seller can cancel. Once the buyer has funded,
    /// the escrow cannot be cancelled (use dispute instead).
    pub fn cancel_escrow(env: Env, escrow_id: u64) -> Result<(), EscrowError> {
        let mut escrow = storage::load_escrow(&env, escrow_id)?;

        escrow.seller.require_auth();

        if escrow.state != EscrowState::Created {
            return Err(EscrowError::InvalidState);
        }

        escrow.state = EscrowState::Refunded;
        storage::update_escrow(&env, &escrow);
        Ok(())
    }

    // ════════════════════════════════════════════════════════
    //  READ-ONLY QUERIES
    // ════════════════════════════════════════════════════════

    /// Fetch full escrow metadata.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<Escrow, EscrowError> {
        storage::load_escrow(&env, escrow_id)
    }

    /// Fetch dispute metadata for an escrow.
    pub fn get_dispute(env: Env, escrow_id: u64) -> Result<Dispute, EscrowError> {
        storage::load_dispute(&env, escrow_id)
    }

    /// Retrieve all evidence for a dispute.
    pub fn get_evidence(env: Env, escrow_id: u64) -> Vec<DisputeEvidence> {
        storage::get_evidence(&env, escrow_id)
    }

    /// Retrieve all votes for a dispute.
    pub fn get_votes(env: Env, escrow_id: u64) -> Vec<DisputeVote> {
        storage::get_votes(&env, escrow_id)
    }

    /// Get the current release vote count for a dispute.
    pub fn get_release_vote_count(env: Env, escrow_id: u64) -> u32 {
        storage::count_release_votes(&env, escrow_id)
    }

    /// Get the current refund vote count for a dispute.
    pub fn get_refund_vote_count(env: Env, escrow_id: u64) -> u32 {
        storage::count_refund_votes(&env, escrow_id)
    }

    /// Check whether `address` is a registered multisig signer.
    pub fn is_signer(env: Env, address: Address) -> bool {
        storage::is_signer(&env, &address)
    }

    /// Get the list of multisig signers.
    pub fn get_signers(env: Env) -> Vec<Address> {
        storage::load_signers(&env)
    }

    /// Get the multisig threshold.
    pub fn get_threshold(env: Env) -> u32 {
        storage::load_threshold(&env)
    }

    /// Get the default dispute window duration.
    pub fn get_dispute_window(env: Env) -> u64 {
        storage::dispute_window(&env)
    }

    /// Get the minimum timelock duration.
    pub fn get_min_timelock(env: Env) -> u64 {
        storage::min_timelock(&env)
    }

    /// Admin: set the default dispute window.
    pub fn set_dispute_window(env: Env, caller: Address, seconds: u64) {
        caller.require_auth();
        storage::set_dispute_window(&env, seconds);
    }

    /// Admin: set the minimum timelock.
    pub fn set_min_timelock(env: Env, caller: Address, seconds: u64) {
        caller.require_auth();
        storage::set_min_timelock(&env, seconds);
    }
}
