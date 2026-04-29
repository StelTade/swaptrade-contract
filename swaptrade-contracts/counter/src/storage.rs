use soroban_sdk::{contracttype, symbol_short, Symbol, Address};

pub const ADMIN_KEY: Symbol = symbol_short!("admin");
pub const PAUSED_KEY: Symbol = symbol_short!("paused");
pub const POOL_REGISTRY_KEY: Symbol = symbol_short!("pools");

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // Existing keys
    Admin,
    Paused,
    PoolRegistry,
    
    // Referral system keys
    Referrer(Address),
    ReferralInfo(Address),
    ReferralStats(Address),
    TradingVolume(Address),
    CommissionBalance(Address),
}
