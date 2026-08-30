use soroban_sdk::{contracttype, Address};

/// Storage keys for the fee & incentives contract.
/// Uses persistent storage for cross-invocation durability.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    /// Admin account address.
    Admin,
    /// Default fee config for unconfigured pairs.
    DefaultFeeConfig,
    /// Fee config for a specific (base_asset, quote_asset) pair.
    FeeConfig(Address, Address),
    /// Treasury accumulated balance per asset.
    TreasuryBalance(Address),
    /// Relayer accumulated balance per (relayer, asset).
    RelayerBalance(Address, Address),
    /// LP reward pool accumulated balance per asset.
    LpPoolBalance(Address),
    /// Per-user reward ledger per (user, asset).
    RewardLedger(Address, Address),
    /// Lifetime fee totals for a pair: (base_asset, quote_asset).
    PairTotals(Address, Address),
}
