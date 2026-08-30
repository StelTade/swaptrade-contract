use soroban_sdk::{symbol_short, Address, Env};

use crate::errors::FeeDestination;
use crate::types::FeeConfig;

pub fn emit_fee_config_updated(
    env: &Env,
    base_asset: &Address,
    quote_asset: &Address,
    config: &FeeConfig,
) {
    env.events().publish(
        (symbol_short!("fee_cfg"), base_asset, quote_asset),
        (
            config.treasury_fee_bps,
            config.lp_fee_bps,
            config.relayer_fee_bps,
            config.total_bps(),
            env.ledger().timestamp(),
        ),
    );
}

pub fn emit_fee_collected(
    env: &Env,
    asset: &Address,
    payer: &Address,
    destination: FeeDestination,
    amount: i128,
) {
    let dest_tag = match &destination {
        FeeDestination::Treasury => symbol_short!("treasury"),
        FeeDestination::LpPool => symbol_short!("lp_pool"),
        FeeDestination::Relayer => symbol_short!("relayer"),
    };

    env.events().publish(
        (symbol_short!("fee_col"), dest_tag),
        (asset, payer, amount, env.ledger().timestamp()),
    );
}

pub fn emit_rewards_claimed(
    env: &Env,
    user: &Address,
    asset: &Address,
    amount: i128,
    nonce: u64,
) {
    env.events().publish(
        (symbol_short!("rwrd_clm"), user, asset),
        (amount, nonce, env.ledger().timestamp()),
    );
}

pub fn emit_treasury_withdrawn(env: &Env, asset: &Address, amount: i128) {
    env.events()
        .publish((symbol_short!("treas_wd"), asset), (amount, env.ledger().timestamp()));
}

pub fn emit_relayer_withdrawn(env: &Env, relayer: &Address, asset: &Address, amount: i128) {
    env.events().publish(
        (symbol_short!("rel_wd"), relayer, asset),
        (amount, env.ledger().timestamp()),
    );
}
