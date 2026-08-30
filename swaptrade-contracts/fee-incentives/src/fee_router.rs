use soroban_sdk::{Address, Env};

use crate::errors::FeeError;
use crate::events::{emit_treasury_withdrawn, emit_relayer_withdrawn};
use crate::storage::StorageKey;
use crate::token::transfer_token;
use crate::types::{FeeConfig, FeeRouting, FeeTotals};

/// Default fee config used when no pair-specific config is set.
const DEFAULT_TREASURY_BPS: u32 = 20; // 0.2%
const DEFAULT_LP_BPS: u32 = 50;       // 0.5%
const DEFAULT_RELAYER_BPS: u32 = 0;   // 0% by default

pub struct FeeRouter;

impl FeeRouter {
    // ─── Configuration ──────────────────────────────────────────────

    /// Get fee config for a pair, falling back to the default config.
    pub fn get_fee_config(env: &Env, base_asset: &Address, quote_asset: &Address) -> FeeConfig {
        let key = StorageKey::FeeConfig(base_asset.clone(), quote_asset.clone());
        if let Some(config) = env.storage().persistent().get::<_, FeeConfig>(&key) {
            return config;
        }

        // Try reverse order
        let rev_key = StorageKey::FeeConfig(quote_asset.clone(), base_asset.clone());
        if let Some(config) = env.storage().persistent().get::<_, FeeConfig>(&rev_key) {
            return config;
        }

        // Fall back to default
        env.storage()
            .persistent()
            .get(&StorageKey::DefaultFeeConfig)
            .unwrap_or(FeeConfig {
                treasury_fee_bps: DEFAULT_TREASURY_BPS,
                lp_fee_bps: DEFAULT_LP_BPS,
                relayer_fee_bps: DEFAULT_RELAYER_BPS,
            })
    }

    /// Save fee config for a specific pair.
    pub fn save_fee_config(
        env: &Env,
        base_asset: &Address,
        quote_asset: &Address,
        config: &FeeConfig,
    ) {
        let key = StorageKey::FeeConfig(base_asset.clone(), quote_asset.clone());
        env.storage().persistent().set(&key, config);
    }

    // ─── Fee Calculation & Routing ──────────────────────────────────

    /// Calculate fee amounts and route them to the appropriate sub-ledgers.
    /// Returns a FeeRouting summarizing where each portion went.
    pub fn calculate_and_route(
        env: &Env,
        config: &FeeConfig,
        _operation: &crate::errors::FeeOperation,
        trade_amount: i128,
        _payer: &Address,
        relayer: &Option<Address>,
        base_asset: &Address,
        quote_asset: &Address,
    ) -> Result<FeeRouting, FeeError> {
        let total_bps = config.total_bps();
        if total_bps == 0 {
            return Ok(FeeRouting {
                treasury_amount: 0,
                lp_amount: 0,
                relayer_amount: 0,
                total_fee: 0,
            });
        }

        let trade_amount_u128 = trade_amount as u128;

        // Calculate each component: (trade_amount * bps) / 10_000
        let treasury_amount =
            (trade_amount_u128 * config.treasury_fee_bps as u128 / 10_000) as i128;
        let lp_amount = (trade_amount_u128 * config.lp_fee_bps as u128 / 10_000) as i128;
        let relayer_amount = if relayer.is_some() {
            (trade_amount_u128 * config.relayer_fee_bps as u128 / 10_000) as i128
        } else {
            0
        };

        let total_fee = treasury_amount
            .saturating_add(lp_amount)
            .saturating_add(relayer_amount);

        // Ensure we don't exceed the trade amount
        if total_fee > trade_amount {
            return Err(FeeError::InvalidAmount);
        }

        // Route: increment sub-ledger balances (accounting only; no token transfer
        // here — the calling contract handles actual token movement)
        Self::add_treasury_balance(env, quote_asset, treasury_amount);
        Self::add_lp_pool_balance(env, quote_asset, lp_amount);

        if let Some(ref r) = relayer {
            if relayer_amount > 0 {
                Self::add_relayer_balance(env, r, quote_asset, relayer_amount);
            }
        }

        // Update pair lifetime totals
        Self::increment_pair_totals(
            env,
            base_asset,
            quote_asset,
            treasury_amount,
            lp_amount,
            relayer_amount,
        );

        Ok(FeeRouting {
            treasury_amount,
            lp_amount,
            relayer_amount,
            total_fee,
        })
    }

    // ─── Treasury ───────────────────────────────────────────────────

    pub fn treasury_balance(env: &Env, asset: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&StorageKey::TreasuryBalance(asset.clone()))
            .unwrap_or(0)
    }

    fn add_treasury_balance(env: &Env, asset: &Address, amount: i128) {
        let current = Self::treasury_balance(env, asset);
        env.storage().persistent().set(
            &StorageKey::TreasuryBalance(asset.clone()),
            &current.saturating_add(amount),
        );
    }

    /// Admin withdraws accumulated treasury fees for an asset.
    pub fn withdraw_treasury(env: &Env, asset: &Address) -> Result<i128, FeeError> {
        let balance = Self::treasury_balance(env, asset);
        if balance <= 0 {
            return Err(FeeError::InsufficientBalance);
        }

        // Zero out before transfer (replay-safe pattern)
        env.storage().persistent().set(
            &StorageKey::TreasuryBalance(asset.clone()),
            &0i128,
        );

        let admin: Address = env
            .storage()
            .persistent()
            .get(&StorageKey::Admin)
            .ok_or(FeeError::NotInitialized)?;

        let contract_addr = env.current_contract_address();
        transfer_token(env, asset, &contract_addr, &admin, balance)?;

        emit_treasury_withdrawn(env, asset, balance);
        Ok(balance)
    }

    // ─── Relayer ────────────────────────────────────────────────────

    pub fn relayer_balance(env: &Env, relayer: &Address, asset: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&StorageKey::RelayerBalance(relayer.clone(), asset.clone()))
            .unwrap_or(0)
    }

    fn add_relayer_balance(env: &Env, relayer: &Address, asset: &Address, amount: i128) {
        let current = Self::relayer_balance(env, relayer, asset);
        env.storage().persistent().set(
            &StorageKey::RelayerBalance(relayer.clone(), asset.clone()),
            &current.saturating_add(amount),
        );
    }

    /// Relayer withdraws their accumulated fees.
    pub fn withdraw_relayer(
        env: &Env,
        relayer: &Address,
        asset: &Address,
    ) -> Result<i128, FeeError> {
        let balance = Self::relayer_balance(env, relayer, asset);
        if balance <= 0 {
            return Err(FeeError::InsufficientBalance);
        }

        // Zero out before transfer
        env.storage().persistent().set(
            &StorageKey::RelayerBalance(relayer.clone(), asset.clone()),
            &0i128,
        );

        let contract_addr = env.current_contract_address();
        transfer_token(env, asset, &contract_addr, relayer, balance)?;

        emit_relayer_withdrawn(env, relayer, asset, balance);
        Ok(balance)
    }

    // ─── LP Pool ────────────────────────────────────────────────────

    pub fn lp_pool_balance(env: &Env, asset: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&StorageKey::LpPoolBalance(asset.clone()))
            .unwrap_or(0)
    }

    pub fn add_lp_pool_balance(env: &Env, asset: &Address, amount: i128) {
        let current = Self::lp_pool_balance(env, asset);
        env.storage().persistent().set(
            &StorageKey::LpPoolBalance(asset.clone()),
            &current.saturating_add(amount),
        );
    }

    // ─── Pair Totals ────────────────────────────────────────────────

    fn increment_pair_totals(
        env: &Env,
        base_asset: &Address,
        quote_asset: &Address,
        treasury: i128,
        lp: i128,
        relayer: i128,
    ) {
        // Store totals keyed by the (base_asset, quote_asset) pair.
        let key = StorageKey::PairTotals(base_asset.clone(), quote_asset.clone());
        let mut totals: FeeTotals = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(FeeTotals {
                total_treasury: 0,
                total_lp: 0,
                total_relayer: 0,
                total_fees: 0,
            });

        totals.total_treasury = totals.total_treasury.saturating_add(treasury);
        totals.total_lp = totals.total_lp.saturating_add(lp);
        totals.total_relayer = totals.total_relayer.saturating_add(relayer);
        totals.total_fees = totals.total_fees.saturating_add(treasury.saturating_add(lp).saturating_add(relayer));

        env.storage().persistent().set(&key, &totals);
    }

    pub fn pair_totals(env: &Env, base_asset: &Address, quote_asset: &Address) -> FeeTotals {
        // Check base->quote first, then quote->base
        let key = StorageKey::PairTotals(base_asset.clone(), quote_asset.clone());
        if let Some(totals) = env.storage().persistent().get::<_, FeeTotals>(&key) {
            return totals;
        }
        let rev_key = StorageKey::PairTotals(quote_asset.clone(), base_asset.clone());
        if let Some(totals) = env.storage().persistent().get::<_, FeeTotals>(&rev_key) {
            return totals;
        }
        FeeTotals {
            total_treasury: 0,
            total_lp: 0,
            total_relayer: 0,
            total_fees: 0,
        }
    }
}
