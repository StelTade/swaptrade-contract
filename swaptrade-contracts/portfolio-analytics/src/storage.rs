use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::errors::PortfolioError;
use crate::types::{AssetPrice, Position, TransactionRecord};

/// Storage keys for all persistent portfolio data
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    /// Admin address for the contract
    Admin,
    /// Sequential transaction counter: next_tx_id
    NextTxId,
    /// Sequential snapshot counter: next_snapshot_id
    NextSnapshotId,
    /// User position for a specific asset: Position(user, asset)
    Position(Address, Address),
    /// List of assets a user holds: UserAssets(user)
    UserAssets(Address),
    /// Transaction record by ID: Transaction(tx_id)
    Transaction(u64),
    /// Transaction IDs for a user: UserTransactions(user)
    UserTransactions(Address),
    /// Portfolio snapshot by ID: Snapshot(snapshot_id)
    Snapshot(u64),
    /// Snapshot IDs for a user: UserSnapshots(user)
    UserSnapshots(Address),
    /// Current market price for an asset: AssetPrice(asset)
    AssetPriceRecord(Address),
    /// Historical prices for return calculation: PriceHistory(asset)
    PriceHistory(Address),
    /// Return periods for a user: ReturnHistory(user)
    ReturnHistory(Address),
}

// ── ID Generators ───────────────────────────────────────────────────────────

/// Get and increment the next transaction ID
pub fn get_next_tx_id(env: &Env) -> Result<u64, PortfolioError> {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&StorageKey::NextTxId)
        .unwrap_or(1);
    if id == u64::MAX {
        return Err(PortfolioError::TxIdOverflow);
    }
    env.storage()
        .persistent()
        .set(&StorageKey::NextTxId, &(id + 1));
    Ok(id)
}

/// Get and increment the next snapshot ID
pub fn get_next_snapshot_id(env: &Env) -> Result<u64, PortfolioError> {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&StorageKey::NextSnapshotId)
        .unwrap_or(1);
    if id == u64::MAX {
        return Err(PortfolioError::SnapshotIdOverflow);
    }
    env.storage()
        .persistent()
        .set(&StorageKey::NextSnapshotId, &(id + 1));
    Ok(id)
}

// ── Position Storage ────────────────────────────────────────────────────────

/// Save a user's position for a specific asset
pub fn save_position(env: &Env, position: &Position) {
    let key = StorageKey::Position(position.user.clone(), position.asset.clone());
    env.storage().persistent().set(&key, position);

    // Maintain user assets index for enumeration
    let assets_key = StorageKey::UserAssets(position.user.clone());
    let mut assets: Vec<Address> = env
        .storage()
        .persistent()
        .get(&assets_key)
        .unwrap_or_else(|| Vec::new(env));

    // Only add if not already tracked
    let mut found = false;
    for i in 0..assets.len() {
        if assets.get(i).unwrap() == position.asset {
            found = true;
            break;
        }
    }
    if !found {
        assets.push_back(position.asset.clone());
        env.storage().persistent().set(&assets_key, &assets);
    }
}

/// Load a user's position for a specific asset
pub fn get_position(env: &Env, user: &Address, asset: &Address) -> Option<Position> {
    let key = StorageKey::Position(user.clone(), asset.clone());
    env.storage().persistent().get(&key)
}

/// Remove a user's position for a specific asset from storage
pub fn remove_position(env: &Env, user: &Address, asset: &Address) {
    let key = StorageKey::Position(user.clone(), asset.clone());
    env.storage().persistent().remove(&key);
}

/// Get all asset addresses a user holds positions in
pub fn get_user_assets(env: &Env, user: &Address) -> Vec<Address> {
    let key = StorageKey::UserAssets(user.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

/// Remove a user's asset from the tracked list when position is fully closed
pub fn remove_user_asset(env: &Env, user: &Address, asset: &Address) {
    let assets_key = StorageKey::UserAssets(user.clone());
    let assets: Vec<Address> = env
        .storage()
        .persistent()
        .get(&assets_key)
        .unwrap_or_else(|| Vec::new(env));

    let mut new_assets = Vec::new(env);
    for i in 0..assets.len() {
        let a = assets.get(i).unwrap();
        if a != *asset {
            new_assets.push_back(a);
        }
    }
    env.storage().persistent().set(&assets_key, &new_assets);
}

// ── Transaction Record Storage ──────────────────────────────────────────────

/// Save a transaction record
pub fn save_transaction(env: &Env, record: &TransactionRecord) {
    let key = StorageKey::Transaction(record.tx_id);
    env.storage().persistent().set(&key, record);

    // Maintain user transaction index
    let user_tx_key = StorageKey::UserTransactions(record.user.clone());
    let mut tx_ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&user_tx_key)
        .unwrap_or_else(|| Vec::new(env));
    tx_ids.push_back(record.tx_id);
    env.storage().persistent().set(&user_tx_key, &tx_ids);
}

/// Load a transaction record by ID
pub fn get_transaction(env: &Env, tx_id: u64) -> Option<TransactionRecord> {
    let key = StorageKey::Transaction(tx_id);
    env.storage().persistent().get(&key)
}

/// Get all transaction IDs for a user
pub fn get_user_transaction_ids(env: &Env, user: &Address) -> Vec<u64> {
    let key = StorageKey::UserTransactions(user.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

/// Get transaction records for a user, up to max_count
pub fn get_user_transactions(
    env: &Env,
    user: &Address,
    offset: u32,
    max_count: u32,
) -> Vec<TransactionRecord> {
    let tx_ids = get_user_transaction_ids(env, user);
    let mut records = Vec::new(env);
    let start = offset as usize;
    let end = (offset + max_count) as usize;
    for i in 0..tx_ids.len() {
        let idx = i as usize;
        if idx >= start && idx < end {
            if let Some(record) = get_transaction(env, tx_ids.get(i).unwrap()) {
                records.push_back(record);
            }
        }
        if idx >= end {
            break;
        }
    }
    records
}

// ── Snapshot Storage ────────────────────────────────────────────────────────

/// Save a portfolio snapshot
pub fn save_snapshot(
    env: &Env,
    snapshot_id: u64,
    user: &Address,
    snapshot: &crate::types::PortfolioSnapshot,
) {
    let key = StorageKey::Snapshot(snapshot_id);
    env.storage().persistent().set(&key, snapshot);

    let user_snap_key = StorageKey::UserSnapshots(user.clone());
    let mut snap_ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&user_snap_key)
        .unwrap_or_else(|| Vec::new(env));
    snap_ids.push_back(snapshot_id);
    env.storage().persistent().set(&user_snap_key, &snap_ids);
}

/// Load a snapshot by ID
pub fn get_snapshot(
    env: &Env,
    snapshot_id: u64,
) -> Option<crate::types::PortfolioSnapshot> {
    let key = StorageKey::Snapshot(snapshot_id);
    env.storage().persistent().get(&key)
}

/// Get all snapshot IDs for a user
pub fn get_user_snapshot_ids(env: &Env, user: &Address) -> Vec<u64> {
    let key = StorageKey::UserSnapshots(user.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

// ── Price Oracle Storage ────────────────────────────────────────────────────

/// Save current market price for an asset
pub fn save_asset_price(env: &Env, price: &AssetPrice) {
    let key = StorageKey::AssetPriceRecord(price.asset.clone());
    env.storage().persistent().set(&key, price);
}

/// Get current market price for an asset
pub fn get_asset_price(env: &Env, asset: &Address) -> Option<AssetPrice> {
    let key = StorageKey::AssetPriceRecord(asset.clone());
    env.storage().persistent().get(&key)
}

// ── Return History Storage ──────────────────────────────────────────────────

/// Save return period data for a user
pub fn save_return_period(
    env: &Env,
    user: &Address,
    period: &crate::types::ReturnPeriod,
) {
    let key = StorageKey::ReturnHistory(user.clone());
    let mut periods: Vec<crate::types::ReturnPeriod> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    periods.push_back(period.clone());
    env.storage().persistent().set(&key, &periods);
}

/// Get return history for a user
pub fn get_return_history(env: &Env, user: &Address) -> Vec<crate::types::ReturnPeriod> {
    let key = StorageKey::ReturnHistory(user.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}
