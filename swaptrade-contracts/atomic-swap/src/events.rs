use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::types::Swap;

// ── Event topics (short symbols to save space) ──────────────
const TOPIC_CREATED: Symbol = symbol_short!("created");
const TOPIC_FUNDED: Symbol = symbol_short!("funded");
const TOPIC_ACCEPTED: Symbol = symbol_short!("accepted");
const TOPIC_CANCELLED: Symbol = symbol_short!("cancelled");
const TOPIC_REFUNDED: Symbol = symbol_short!("refunded");

/// Payload emitted for every lifecycle event.
#[derive(Clone, Debug)]
pub struct SwapEvent {
    pub swap_id: u64,
    pub actor: Address,
    pub timestamp: u64,
}

fn publish(env: &Env, topic: Symbol, event: SwapEvent) {
    env.events()
        .publish((topic, event.swap_id), (event.actor, event.timestamp));
}

/// Emitted when a new swap is created.
pub fn swap_created(env: &Env, swap: &Swap) {
    publish(
        env,
        TOPIC_CREATED,
        SwapEvent {
            swap_id: swap.id,
            actor: swap.creator.clone(),
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Emitted when a party funds their side.
pub fn swap_funded(env: &Env, swap: &Swap, funder: Address) {
    publish(
        env,
        TOPIC_FUNDED,
        SwapEvent {
            swap_id: swap.id,
            actor: funder,
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Emitted when the swap is atomically executed.
pub fn swap_accepted(env: &Env, swap: &Swap) {
    publish(
        env,
        TOPIC_ACCEPTED,
        SwapEvent {
            swap_id: swap.id,
            actor: swap.counterparty.clone(),
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Emitted when the creator cancels an unfunded swap.
pub fn swap_cancelled(env: &Env, swap: &Swap) {
    publish(
        env,
        TOPIC_CANCELLED,
        SwapEvent {
            swap_id: swap.id,
            actor: swap.creator.clone(),
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Emitted when a funded-but-unaccepted swap is refunded after expiry.
pub fn swap_refunded(env: &Env, swap: &Swap) {
    publish(
        env,
        TOPIC_REFUNDED,
        SwapEvent {
            swap_id: swap.id,
            actor: swap.creator.clone(),
            timestamp: env.ledger().timestamp(),
        },
    );
}
