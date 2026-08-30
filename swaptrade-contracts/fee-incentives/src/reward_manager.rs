use soroban_sdk::{Address, Env};

use crate::errors::FeeError;
use crate::events::emit_rewards_claimed;
use crate::storage::StorageKey;
use crate::token::transfer_token;
use crate::types::RewardLedger;

pub struct RewardManager;

impl RewardManager {
    /// Get the reward ledger for a user/asset pair.
    fn get_ledger(env: &Env, user: &Address, asset: &Address) -> RewardLedger {
        env.storage()
            .persistent()
            .get(&StorageKey::RewardLedger(user.clone(), asset.clone()))
            .unwrap_or(RewardLedger::default())
    }

    /// Save a reward ledger entry.
    fn save_ledger(env: &Env, user: &Address, asset: &Address, ledger: &RewardLedger) {
        env.storage().persistent().set(
            &StorageKey::RewardLedger(user.clone(), asset.clone()),
            ledger,
        );
    }

    /// Add LP/staking rewards for a user.
    pub fn add_rewards(env: &Env, user: &Address, asset: &Address, amount: i128) {
        if amount <= 0 {
            return;
        }
        let mut ledger = Self::get_ledger(env, user, asset);
        ledger.balance = ledger.balance.saturating_add(amount);
        Self::save_ledger(env, user, asset, &ledger);
    }

    /// View the unclaimed (pending) reward balance for a user.
    pub fn pending_balance(env: &Env, user: &Address, asset: &Address) -> i128 {
        let ledger = Self::get_ledger(env, user, asset);
        ledger.balance
    }

    /// Claim accrued rewards with replay protection.
    /// Each claim increments `claim_nonce`; balance zeroed before transfer.
    pub fn claim(env: &Env, user: &Address, asset: &Address) -> Result<i128, FeeError> {
        let mut ledger = Self::get_ledger(env, user, asset);

        if ledger.balance <= 0 {
            return Err(FeeError::NoRewardsToClaim);
        }

        let amount = ledger.balance;
        let nonce = ledger.claim_nonce.saturating_add(1);

        // Zero balance before transfer (replay-safe)
        ledger.balance = 0;
        ledger.total_claimed = ledger.total_claimed.saturating_add(amount);
        ledger.claim_nonce = nonce;
        Self::save_ledger(env, user, asset, &ledger);

        let contract_addr = env.current_contract_address();
        transfer_token(env, asset, &contract_addr, user, amount)?;

        emit_rewards_claimed(env, user, asset, amount, nonce);

        Ok(amount)
    }
}
