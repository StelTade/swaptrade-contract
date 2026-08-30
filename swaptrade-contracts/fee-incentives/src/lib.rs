#![cfg_attr(all(not(test), target_family = "wasm"), no_std)]

extern crate alloc;

pub mod errors;
pub mod events;
pub mod fee_router;
pub mod reward_manager;
pub mod storage;
pub mod token;
pub mod types;

pub use errors::{FeeError, FeeOperation, FeeDestination};
pub use types::*;

use soroban_sdk::{contract, contractimpl, Address, Env};

use events::{emit_fee_config_updated, emit_fee_collected};
use fee_router::FeeRouter;
use reward_manager::RewardManager;
use storage::StorageKey;



#[contract]
pub struct FeeIncentivesContract;

#[contractimpl]
impl FeeIncentivesContract {
    /// Initialize the fee & incentives contract with an admin account.
    pub fn initialize(env: Env, admin: Address) -> Result<(), FeeError> {
        admin.require_auth();

        if env.storage().persistent().has(&StorageKey::Admin) {
            return Err(FeeError::AlreadyInitialized);
        }

        env.storage().persistent().set(&StorageKey::Admin, &admin);
        Ok(())
    }

    // ─── Fee Configuration ───────────────────────────────────────────

    /// Set fee configuration for a specific asset pair.
    /// `pair` is a (base_asset, quote_asset) tuple.
    /// Fee bps must sum to <= MAX_TOTAL_FEE_BPS (1000 = 10%).
    pub fn set_fee_config(
        env: Env,
        admin: Address,
        base_asset: Address,
        quote_asset: Address,
        config: FeeConfig,
    ) -> Result<(), FeeError> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;

        config.validate()?;
        FeeRouter::save_fee_config(&env, &base_asset, &quote_asset, &config);

        emit_fee_config_updated(&env, &base_asset, &quote_asset, &config);
        Ok(())
    }

    /// Set default fee configuration for all unconfigured pairs.
    pub fn set_default_fee_config(
        env: Env,
        admin: Address,
        config: FeeConfig,
    ) -> Result<(), FeeError> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;

        config.validate()?;
        env.storage()
            .persistent()
            .set(&StorageKey::DefaultFeeConfig, &config);

        Ok(())
    }

    /// Get fee configuration for a pair (falls back to default).
    pub fn get_fee_config(
        env: Env,
        base_asset: Address,
        quote_asset: Address,
    ) -> FeeConfig {
        FeeRouter::get_fee_config(&env, &base_asset, &quote_asset)
    }

    // ─── Fee Collection & Routing ─────────────────────────────────────

    /// Collect and route fees from a swap/order execution.
    /// Called by the trade engine or pool contract after a successful trade.
    /// `operation` describes the type (Swap, PoolSwap, OrderFill).
    /// `trade_amount` is the notional value in quote asset.
    /// `payer` pays the fee; `relayer` (optional) receives a portion.
    /// `base_asset` and `quote_asset` identify the pair.
    pub fn collect_fee(
        env: Env,
        caller: Address,
        base_asset: Address,
        quote_asset: Address,
        operation: FeeOperation,
        trade_amount: i128,
        payer: Address,
        relayer: Option<Address>,
    ) -> Result<FeeRouting, FeeError> {
        // Only authorized contracts/addresses may call collect_fee.
        // In production this would check against registered caller set.
        caller.require_auth();

        if trade_amount <= 0 {
            return Err(FeeError::InvalidAmount);
        }

        let config = FeeRouter::get_fee_config(&env, &base_asset, &quote_asset);
        let routing = FeeRouter::calculate_and_route(
            &env,
            &config,
            &operation,
            trade_amount,
            &payer,
            &relayer,
            &base_asset,
            &quote_asset,
        )?;

        // Emit per-component events
        if routing.treasury_amount > 0 {
            emit_fee_collected(&env, &quote_asset, &payer, FeeDestination::Treasury, routing.treasury_amount);
        }
        if routing.lp_amount > 0 {
            emit_fee_collected(&env, &quote_asset, &payer, FeeDestination::LpPool, routing.lp_amount);
        }
        if routing.relayer_amount > 0 {
            if let Some(ref r) = relayer {
                emit_fee_collected(&env, &quote_asset, r, FeeDestination::Relayer, routing.relayer_amount);
            }
        }

        Ok(routing)
    }

    // ─── LP / Staker Reward Claiming ─────────────────────────────────

    /// Claim accrued LP/staking rewards for the calling user.
    /// Returns the amount transferred. Uses replay protection via
    /// a nonce tracked per user.
    pub fn claim_rewards(
        env: Env,
        user: Address,
        asset: Address,
    ) -> Result<i128, FeeError> {
        user.require_auth();

        let amount = RewardManager::claim(&env, &user, &asset)?;
        Ok(amount)
    }

    /// View accrued (unclaimed) LP reward balance for a user.
    pub fn get_pending_rewards(env: Env, user: Address, asset: Address) -> i128 {
        RewardManager::pending_balance(&env, &user, &asset)
    }

    // ─── Treasury / Relayer Withdrawals ──────────────────────────────

    /// Treasury (admin) withdraws accumulated protocol fees.
    pub fn claim_treasury(
        env: Env,
        admin: Address,
        asset: Address,
    ) -> Result<i128, FeeError> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;

        FeeRouter::withdraw_treasury(&env, &asset)
    }

    /// Relayer withdraws their accumulated fee portion.
    pub fn claim_relayer_fee(
        env: Env,
        relayer: Address,
        asset: Address,
    ) -> Result<i128, FeeError> {
        relayer.require_auth();

        FeeRouter::withdraw_relayer(&env, &relayer, &asset)
    }

    // ─── View Helpers ────────────────────────────────────────────────

    /// View the treasury balance for an asset (not yet claimed).
    pub fn get_treasury_balance(env: Env, asset: Address) -> i128 {
        FeeRouter::treasury_balance(&env, &asset)
    }

    /// View a relayer's accumulated fee balance for an asset.
    pub fn get_relayer_balance(env: Env, relayer: Address, asset: Address) -> i128 {
        FeeRouter::relayer_balance(&env, &relayer, &asset)
    }

    /// View the total fees collected for a pair over the lifetime of the contract.
    pub fn get_pair_total_fees(
        env: Env,
        base_asset: Address,
        quote_asset: Address,
    ) -> FeeTotals {
        FeeRouter::pair_totals(&env, &base_asset, &quote_asset)
    }

    // ─── Admin Helpers ───────────────────────────────────────────────

    /// Update the admin address (requires current admin auth).
    pub fn set_admin(env: Env, current_admin: Address, new_admin: Address) -> Result<(), FeeError> {
        current_admin.require_auth();
        Self::require_admin(&env, &current_admin)?;

        env.storage()
            .persistent()
            .set(&StorageKey::Admin, &new_admin);
        Ok(())
    }

    /// Get the current admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .persistent()
            .get(&StorageKey::Admin)
            .expect("contract not initialized")
    }

    // ─── Internal ────────────────────────────────────────────────────

    fn require_admin(env: &Env, caller: &Address) -> Result<(), FeeError> {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&StorageKey::Admin)
            .ok_or(FeeError::NotInitialized)?;

        if *caller != admin {
            return Err(FeeError::Unauthorized);
        }
        Ok(())
    }
}
