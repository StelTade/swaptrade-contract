use soroban_sdk::{Address, Env};

use crate::errors::TradeError;
use crate::storage::StorageKey;
use crate::types::LiquidityPool;

pub struct PoolManager;

impl PoolManager {
    /// Save pool
    pub fn save_pool(env: &Env, pool: &LiquidityPool) {
        let key = StorageKey::LiquidityPool(pool.pool_id);
        env.storage().persistent().set(&key, pool);

        let pair_key = StorageKey::PairPool(pool.asset_a.clone(), pool.asset_b.clone());
        env.storage().persistent().set(&pair_key, &pool.pool_id);
    }

    /// Load pool by ID
    pub fn get_pool(env: &Env, pool_id: u64) -> Option<LiquidityPool> {
        let key = StorageKey::LiquidityPool(pool_id);
        env.storage().persistent().get(&key)
    }

    /// Load pool by asset pair
    pub fn get_pool_by_pair(env: &Env, asset_a: &Address, asset_b: &Address) -> Option<LiquidityPool> {
        let pair_key = StorageKey::PairPool(asset_a.clone(), asset_b.clone());
        if let Some(pool_id) = env.storage().persistent().get::<_, u64>(&pair_key) {
            return Self::get_pool(env, pool_id);
        }
        // Try reverse order
        let rev_pair_key = StorageKey::PairPool(asset_b.clone(), asset_a.clone());
        if let Some(pool_id) = env.storage().persistent().get::<_, u64>(&rev_pair_key) {
            return Self::get_pool(env, pool_id);
        }
        None
    }

    /// Calculate constant-product output for fallback trade execution
    /// input_amount * reserve_out * (10000 - fee_bps) / (reserve_in * 10000 + input_amount * (10000 - fee_bps))
    pub fn get_amount_out(
        amount_in: i128,
        reserve_in: i128,
        reserve_out: i128,
        fee_bps: u32,
    ) -> Result<i128, TradeError> {
        if amount_in <= 0 || reserve_in <= 0 || reserve_out <= 0 {
            return Err(TradeError::InsufficientLiquidity);
        }

        let fee_multiplier = 10_000i128.saturating_sub(fee_bps as i128);
        let amount_in_with_fee = amount_in.saturating_mul(fee_multiplier);
        let numerator = amount_in_with_fee.saturating_mul(reserve_out);
        let denominator = reserve_in.saturating_mul(10_000i128).saturating_add(amount_in_with_fee);

        if denominator == 0 {
            return Err(TradeError::InsufficientLiquidity);
        }

        let amount_out = numerator / denominator;
        if amount_out <= 0 || amount_out > reserve_out {
            return Err(TradeError::InsufficientLiquidity);
        }

        Ok(amount_out)
    }
}
