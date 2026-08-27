// Multi-Asset Staking
//
// Tracks staked balances per (staker, asset) pair and computes
// simple time-weighted rewards per asset.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StakeEntry {
    pub asset: String,
    pub amount: u128,
    pub staked_at: u64,
}

#[derive(Debug)]
pub struct MultiAssetStaking {
    stakes: HashMap<(String, String), StakeEntry>,
    reward_rates: HashMap<String, u64>,
}

impl MultiAssetStaking {
    pub fn new() -> Self {
        Self {
            stakes: HashMap::new(),
            reward_rates: HashMap::new(),
        }
    }

    pub fn set_reward_rate(&mut self, asset: &str, rate_bps_per_sec: u64) {
        self.reward_rates
            .insert(asset.to_string(), rate_bps_per_sec);
    }

    pub fn stake(
        &mut self,
        staker: &str,
        asset: &str,
        amount: u128,
        now: u64,
    ) -> Result<(), String> {
        if amount == 0 {
            return Err("stake amount must be greater than zero".to_string());
        }
        let key = (staker.to_string(), asset.to_string());
        let entry = self.stakes.entry(key).or_insert(StakeEntry {
            asset: asset.to_string(),
            amount: 0,
            staked_at: now,
        });
        entry.amount = entry.amount.checked_add(amount).ok_or("overflow")?;
        Ok(())
    }

    pub fn unstake(&mut self, staker: &str, asset: &str, amount: u128) -> Result<(), String> {
        let key = (staker.to_string(), asset.to_string());
        let entry = self.stakes.get_mut(&key).ok_or("no stake found")?;
        if amount > entry.amount {
            return Err("insufficient staked balance".to_string());
        }
        entry.amount -= amount;
        if entry.amount == 0 {
            self.stakes.remove(&key);
        }
        Ok(())
    }

    pub fn pending_rewards(&self, staker: &str, asset: &str, now: u64) -> u128 {
        let key = (staker.to_string(), asset.to_string());
        let entry = match self.stakes.get(&key) {
            Some(e) => e,
            None => return 0,
        };
        let rate = *self.reward_rates.get(asset).unwrap_or(&0) as u128;
        let elapsed = now.saturating_sub(entry.staked_at) as u128;
        entry.amount * rate * elapsed / 10_000
    }

    pub fn staked_balance(&self, staker: &str, asset: &str) -> u128 {
        self.stakes
            .get(&(staker.to_string(), asset.to_string()))
            .map(|e| e.amount)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stake_and_balance() {
        let mut s = MultiAssetStaking::new();
        s.stake("alice", "USDC", 1_000, 0).unwrap();
        assert_eq!(s.staked_balance("alice", "USDC"), 1_000);
    }

    #[test]
    fn test_unstake() {
        let mut s = MultiAssetStaking::new();
        s.stake("alice", "XLM", 500, 0).unwrap();
        s.unstake("alice", "XLM", 200).unwrap();
        assert_eq!(s.staked_balance("alice", "XLM"), 300);
    }

    #[test]
    fn test_insufficient_unstake() {
        let mut s = MultiAssetStaking::new();
        s.stake("bob", "ETH", 100, 0).unwrap();
        assert!(s.unstake("bob", "ETH", 200).is_err());
    }

    #[test]
    fn test_pending_rewards() {
        let mut s = MultiAssetStaking::new();
        s.set_reward_rate("USDC", 1);
        s.stake("alice", "USDC", 10_000, 0).unwrap();
        assert_eq!(s.pending_rewards("alice", "USDC", 100), 100);
    }
}
