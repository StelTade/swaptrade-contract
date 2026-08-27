use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::types::{Escrow, Dispute, DisputeEvidence};

// ── Escrow event topics ───────────────────────────────────
const TOPIC_CREATED: Symbol = symbol_short!("created");
const TOPIC_FUNDED: Symbol = symbol_short!("funded");
const TOPIC_RELEASED: Symbol = symbol_short!("released");
const TOPIC_REFUNDED: Symbol = symbol_short!("refunded");

// ── Dispute event topics ──────────────────────────────────
const TOPIC_DISPUTED: Symbol = symbol_short!("disputed");
const TOPIC_EVIDENCE: Symbol = symbol_short!("evidence");
const TOPIC_RESOLVED: Symbol = symbol_short!("resolved");
const TOPIC_AUTOREFUND: Symbol = symbol_short!("autoref");

/// Emitted when a new escrow is created.
pub fn escrow_created(env: &Env, escrow: &Escrow) {
    env.events().publish(
        (TOPIC_CREATED, escrow.id),
        (
            escrow.seller.clone(),
            escrow.buyer.clone(),
            escrow.asset.clone(),
            escrow.amount,
            env.ledger().timestamp(),
        ),
    );
}

/// Emitted when the buyer funds the escrow.
pub fn escrow_funded(env: &Env, escrow: &Escrow) {
    env.events().publish(
        (TOPIC_FUNDED, escrow.id),
        (
            escrow.buyer.clone(),
            escrow.asset.clone(),
            escrow.amount,
            env.ledger().timestamp(),
        ),
    );
}

/// Emitted when a dispute is raised, freezing funds.
pub fn dispute_raised(env: &Env, dispute: &Dispute) {
    env.events().publish(
        (TOPIC_DISPUTED, dispute.escrow_id),
        (
            dispute.raised_by.clone(),
            dispute.deadline,
            dispute.raised_at,
        ),
    );
}

/// Emitted when evidence is submitted for a dispute.
pub fn evidence_submitted(env: &Env, dispute: &Dispute, evidence: &DisputeEvidence) {
    env.events().publish(
        (TOPIC_EVIDENCE, dispute.escrow_id),
        (
            evidence.submitted_by.clone(),
            evidence.hash.clone(),
            evidence.description.clone(),
            evidence.submitted_at,
        ),
    );
}

/// Emitted when a dispute is resolved (release or refund).
pub fn dispute_resolved(env: &Env, dispute: &Dispute, resolved_by: &Address) {
    let outcome = match dispute.status {
        crate::types::DisputeStatus::ResolvedRelease => symbol_short!("release"),
        crate::types::DisputeStatus::ResolvedRefund => symbol_short!("refund"),
        _ => symbol_short!("unknown"),
    };
    env.events().publish(
        (TOPIC_RESOLVED, dispute.escrow_id),
        (resolved_by.clone(), outcome, env.ledger().timestamp()),
    );
}

/// Emitted when escrow is released to the seller.
pub fn escrow_released(env: &Env, escrow: &Escrow) {
    env.events().publish(
        (TOPIC_RELEASED, escrow.id),
        (
            escrow.seller.clone(),
            escrow.asset.clone(),
            escrow.amount,
            env.ledger().timestamp(),
        ),
    );
}

/// Emitted when escrow is refunded to the buyer.
pub fn escrow_refunded(env: &Env, escrow: &Escrow) {
    env.events().publish(
        (TOPIC_REFUNDED, escrow.id),
        (
            escrow.buyer.clone(),
            escrow.asset.clone(),
            escrow.amount,
            env.ledger().timestamp(),
        ),
    );
}

/// Emitted when a dispute is auto-refunded after timelock expiry.
pub fn dispute_auto_refunded(env: &Env, dispute: &Dispute) {
    env.events().publish(
        (TOPIC_AUTOREFUND, dispute.escrow_id),
        (
            dispute.deadline,
            env.ledger().timestamp(),
        ),
    );
}
