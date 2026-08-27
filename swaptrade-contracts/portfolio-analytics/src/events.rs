use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{CostBasisMethod, TransactionRecord};

/// Emitted when a position is opened (first buy into an asset)
pub fn emit_position_opened(
    env: &Env,
    user: &Address,
    asset: &Address,
    quote_asset: &Address,
    quantity: i128,
    price: i128,
    cost_method: &CostBasisMethod,
) {
    env.events().publish(
        (symbol_short!("pos_open"), user.clone(), asset.clone()),
        (
            quote_asset.clone(),
            quantity,
            price,
            cost_method.clone(),
        ),
    );
}

/// Emitted when a position is fully closed (quantity reaches zero)
pub fn emit_position_closed(
    env: &Env,
    user: &Address,
    asset: &Address,
    realized_pnl: i128,
    total_invested: i128,
    total_divested: i128,
) {
    env.events().publish(
        (symbol_short!("pos_close"), user.clone(), asset.clone()),
        (realized_pnl, total_invested, total_divested),
    );
}

/// Emitted when a position's quantity changes (partial fill, add, reduce)
pub fn emit_position_updated(
    env: &Env,
    user: &Address,
    asset: &Address,
    new_quantity: i128,
    avg_cost_basis: i128,
    realized_pnl: i128,
) {
    env.events().publish(
        (symbol_short!("pos_upd"), user.clone(), asset.clone()),
        (new_quantity, avg_cost_basis, realized_pnl),
    );
}

/// Emitted for every transaction (buy or sell) as part of the audit trail
pub fn emit_transaction(env: &Env, record: &TransactionRecord) {
    env.events().publish(
        (symbol_short!("txn_rec"), record.tx_id),
        (
            record.user.clone(),
            record.asset.clone(),
            record.quote_asset.clone(),
            record.tx_type.clone(),
            record.quantity,
            record.price,
            record.total_value,
            record.realized_pnl,
            record.remaining_quantity,
        ),
    );
}

/// Emitted when a portfolio snapshot is captured
pub fn emit_snapshot_captured(
    env: &Env,
    user: &Address,
    snapshot_id: u64,
    total_value: i128,
    positions_count: u32,
) {
    env.events().publish(
        (symbol_short!("snap_cap"), user.clone(), snapshot_id),
        (total_value, positions_count),
    );
}

/// Emitted when performance metrics are computed
pub fn emit_metrics_computed(
    env: &Env,
    user: &Address,
    roi_bps: i128,
    sharpe_ratio: i128,
    total_trades: u64,
) {
    env.events().publish(
        (symbol_short!("met_comp"), user.clone()),
        (roi_bps, sharpe_ratio, total_trades),
    );
}

/// Emitted when a price oracle update is recorded for mark-to-market
pub fn emit_price_update(env: &Env, asset: &Address, price: u128, timestamp: u64) {
    env.events().publish(
        (symbol_short!("price_upd"), asset.clone()),
        (price, timestamp),
    );
}
