use soroban_sdk::contracttype;

/// Fee configuration for a specific asset pair.
/// All values in basis points (bps); 100 bps = 1%.
/// The sum must be <= MAX_TOTAL_FEE_BPS (1000 = 10%).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeConfig {
    /// Fee portion routed to the protocol treasury.
    pub treasury_fee_bps: u32,
    /// Fee portion routed to the LP reward pool.
    pub lp_fee_bps: u32,
    /// Fee portion routed to the relayer (optional; 0 if none).
    pub relayer_fee_bps: u32,
}

impl FeeConfig {
    /// Validate that the fee components are sane.
    pub fn validate(&self) -> Result<(), super::errors::FeeError> {
        let total = self
            .treasury_fee_bps
            .saturating_add(self.lp_fee_bps)
            .saturating_add(self.relayer_fee_bps);

        if total > MAX_TOTAL_FEE_BPS {
            return Err(super::errors::FeeError::FeeTooHigh);
        }
        Ok(())
    }

    /// Total fee in basis points for this config.
    pub fn total_bps(&self) -> u32 {
        self.treasury_fee_bps
            .saturating_add(self.lp_fee_bps)
            .saturating_add(self.relayer_fee_bps)
    }
}

/// Maximum total fee: 1000 bps = 10%.
pub const MAX_TOTAL_FEE_BPS: u32 = 1000;

/// Result of fee routing: how much went where.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeRouting {
    pub treasury_amount: i128,
    pub lp_amount: i128,
    pub relayer_amount: i128,
    pub total_fee: i128,
}

/// Lifetime fee totals for a pair.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeTotals {
    pub total_treasury: i128,
    pub total_lp: i128,
    pub total_relayer: i128,
    pub total_fees: i128,
}

/// Per-user, per-asset reward ledger entry with replay protection.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardLedger {
    /// Accumulated but unclaimed rewards.
    pub balance: i128,
    /// Total lifetime claimed amount.
    pub total_claimed: i128,
    /// Nonce incremented on each claim to prevent replay.
    pub claim_nonce: u64,
}

impl Default for RewardLedger {
    fn default() -> Self {
        Self {
            balance: 0,
            total_claimed: 0,
            claim_nonce: 0,
        }
    }
}
