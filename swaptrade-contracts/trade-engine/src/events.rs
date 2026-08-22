use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{FillResult, Order};

pub fn emit_order_placed(env: &Env, order: &Order) {
    env.events().publish(
        (symbol_short!("ord_place"), order.order_id),
        (
            order.owner.clone(),
            order.base_asset.clone(),
            order.quote_asset.clone(),
            order.side.clone(),
            order.price,
            order.amount,
        ),
    );
}

pub fn emit_order_cancelled(env: &Env, order_id: u64, owner: &Address) {
    env.events().publish(
        (symbol_short!("ord_canc"), order_id),
        owner.clone(),
    );
}

pub fn emit_trade_executed(
    env: &Env,
    trader: &Address,
    legs_count: u32,
    fills_count: u32,
) {
    env.events().publish(
        (symbol_short!("trd_exec"), trader.clone()),
        (legs_count, fills_count),
    );
}

pub fn emit_fill(env: &Env, fill: &FillResult) {
    env.events().publish(
        (symbol_short!("ord_fill"), fill.order_id),
        (
            fill.maker.clone(),
            fill.taker.clone(),
            fill.price,
            fill.filled_base,
            fill.filled_quote,
            fill.filled_via_pool,
        ),
    );
}

pub fn emit_pool_added(env: &Env, pool_id: u64, asset_a: &Address, asset_b: &Address) {
    env.events().publish(
        (symbol_short!("pool_add"), pool_id),
        (asset_a.clone(), asset_b.clone()),
    );
}
