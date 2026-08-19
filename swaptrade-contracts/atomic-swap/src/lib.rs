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

        if !storage::has_trustline(&env, &creator, &asset_a) {
            return Err(SwapError::MissingTrustline);
        }

        if let Some(existing_id) = storage::find_by_nonce(&env, &creator, nonce) {
            return Ok(existing_id);
        }

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

    // ════════════════════════════════════════════════════════
    //  FUND SWAP
    // ════════════════════════════════════════════════════════

    /// Deposit assets into escrow from either party.
    ///
    /// Verifies the funder holds a trustline for the relevant asset and
    /// that the asset contract `transfer` succeeds.  Both parties must
    /// fund before the swap can be accepted.
    pub fn fund_swap(env: Env, swap_id: u64, funder: Address) -> Result<(), SwapError> {
        funder.require_auth();

        let mut swap = storage::load_swap(&env, swap_id)?;

        if !swap.is_party(&funder) {
            return Err(SwapError::Unauthorized);
        }
        if swap.state != SwapState::Created {
            return Err(SwapError::InvalidState);
        }
        // Prevent double-funding: check if this party already funded.
        let already_funded = if funder == swap.creator {
            swap.creator_funded
        } else {
            swap.counterparty_funded
        };
        if already_funded {
            return Err(SwapError::InvalidState);
        }

        // ── Determine which side this funder is covering ──────
        let (asset, amount, flag) = if funder == swap.creator {
            (
                swap.asset_a.clone(),
                swap.amount_a,
                &mut swap.creator_funded,
            )
        } else {
            (
                swap.asset_b.clone(),
                swap.amount_b,
                &mut swap.counterparty_funded,
            )
        };

        // ── Trustline check ───────────────────────────────────
        if !storage::has_trustline(&env, &funder, &asset) {
            return Err(SwapError::MissingTrustline);
        }

        // ── Transfer funder → contract (escrow) ───────────────
        // Auth already checked above; use no_auth variant to avoid double-consume.
        let contract_addr = env.current_contract_address();
        storage::transfer_token_no_auth(&env, &asset, &funder, &contract_addr, amount)?;

        *flag = true;
        // Only mark as Funded when both parties have deposited.
        if swap.creator_funded && swap.counterparty_funded {
            swap.state = SwapState::Funded;
        }
        storage::update_swap(&env, &swap);
        events::swap_funded(&env, &swap, funder);

        Ok(())
    }

    // ════════════════════════════════════════════════════════
    //  CANCEL SWAP
    // ════════════════════════════════════════════════════════

    /// Cancel a swap that has not yet been fully funded.
    ///
    /// Only the creator may cancel.  Once the counterparty has funded,
    /// the creator cannot cancel (counterparty has economic interest).
    pub fn cancel_swap(env: Env, swap_id: u64) -> Result<(), SwapError> {
        let mut swap = storage::load_swap(&env, swap_id)?;

        swap.creator.require_auth();

        if swap.state != SwapState::Created {
            return Err(SwapError::InvalidState);
        }
        // Counterparty has funded → creator cannot cancel.
        if swap.counterparty_funded {
            return Err(SwapError::InvalidState);
        }
        // Creator already funded → must use refund_swap instead.
        if swap.creator_funded {
            return Err(SwapError::InvalidState);
        }

        swap.state = SwapState::Cancelled;
        storage::update_swap(&env, &swap);
        events::swap_cancelled(&env, &swap);

        Ok(())
    }

    /// Placeholder for accept_swap + refund_swap — added in next commit.
    #[allow(dead_code)]
    fn _placeholder() {}
}
