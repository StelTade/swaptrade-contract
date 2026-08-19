#![cfg_attr(all(not(test), target_family = "wasm"), no_std)]

mod errors;
mod events;
mod storage;
mod types;

pub use errors::SwapError;
pub use types::{Swap, SwapState};

use soroban_sdk::{contract, contractimpl, Address, Env};

/// Soroban atomic swap contract.
///
/// Provides a trusted-escrow pattern where:
/// 1. Creator defines swap terms (assets, amounts, expiry).
/// 2. Both parties fund their sides via trustline-verified transfers.
/// 3. Counterparty accepts → assets move atomically.
/// 4. If expired before acceptance, creator can refund both sides.
#[contract]
pub struct AtomicSwapContract;

#[contractimpl]
impl AtomicSwapContract {
    /// Create a new atomic swap offer.
    ///
    /// # Parameters
    /// * `creator` – the initiating party (requires auth)
    /// * `counterparty` – the other party who will fund & accept
    /// * `asset_a` – Stellar asset contract address for side A
    /// * `amount_a` – amount of asset A (must be > 0)
    /// * `asset_b` – Stellar asset contract address for side B
    /// * `amount_b` – amount of asset B (must be > 0)
    /// * `expiry` – ledger timestamp after which unaccepted swaps can be refunded
    /// * `nonce` – client-supplied nonce for idempotency
    ///
    /// # Returns
    /// The unique swap id.  If (creator, nonce) was already used, returns the
    /// existing swap id without creating a duplicate.
    pub fn create_swap(
        env: Env,
        creator: Address,
        counterparty: Address,
        asset_a: Address,
        amount_a: i128,
        asset_b: Address,
        amount_b: i128,
        expiry: u64,
        nonce: u64,
    ) -> Result<u64, SwapError> {
        creator.require_auth();

        // ── Param validation ──────────────────────────────────
        if amount_a <= 0 || amount_b <= 0 {
            return Err(SwapError::InvalidAmount);
        }
        if asset_a == asset_b {
            return Err(SwapError::SameAsset);
        }
        if creator == counterparty {
            return Err(SwapError::Unauthorized);
        }

        let now = env.ledger().timestamp();
        let min_exp = storage::min_expiry(&env);
        if expiry <= now.saturating_add(min_exp) {
            return Err(SwapError::InvalidExpiry);
        }

        // ── Trustline pre-check: creator must trust asset_a ───
        if !storage::has_trustline(&env, &creator, &asset_a) {
            return Err(SwapError::MissingTrustline);
        }

        // ── Idempotency ─────────────────────────────────────
        if let Some(existing_id) = storage::find_by_nonce(&env, &creator, nonce) {
            return Ok(existing_id);
        }

        // ── Persist ──────────────────────────────────────────
        let id = storage::next_id(&env);
        let swap = Swap {
            id,
            nonce,
            creator: creator.clone(),
            counterparty: counterparty.clone(),
            asset_a: asset_a.clone(),
            amount_a,
            asset_b: asset_b.clone(),
            amount_b,
            expiry,
            state: SwapState::Created,
            creator_funded: false,
            counterparty_funded: false,
            created_at: now,
        };
        storage::save_swap(&env, &swap);
        events::swap_created(&env, &swap);

        Ok(id)
    }

    /// Placeholder for fund_swap — added in next commit.
    #[allow(dead_code)]
    fn _placeholder() {}
}
