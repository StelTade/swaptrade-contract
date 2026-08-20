use soroban_sdk::{contracttype, Address};

/// Lifecycle states for an atomic swap.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum SwapState {
    /// Swap created, awaiting funding.
    Created,
    /// Both parties have deposited their assets into escrow.
    Funded,
    /// Swap executed — assets transferred atomically.
    Accepted,
    /// Creator cancelled before acceptance.
    Cancelled,
    /// Refunded after expiry (funded but unaccepted).
    Refunded,
}

/// Minimal on-chain metadata for a single swap.
#[derive(Clone, Debug)]
#[contracttype]
pub struct Swap {
    /// Unique swap identifier.
    pub id: u64,
    /// Client-supplied nonce for idempotency.
    pub nonce: u64,
    /// The party who initiates and funds side A.
    pub creator: Address,
    /// The party who funds side B and can accept.
    pub counterparty: Address,

    // ── Side A (creator) ──────────────────────────────────
    /// Stellar asset contract address for side A.
    pub asset_a: Address,
    /// Amount of asset A the creator deposits.
    pub amount_a: i128,

    // ── Side B (counterparty) ─────────────────────────────
    /// Stellar asset contract address for side B.
    pub asset_b: Address,
    /// Amount of asset B the counterparty must deposit.
    pub amount_b: i128,

    // ── Lifecycle ─────────────────────────────────────────
    /// Ledger timestamp (seconds) after which the swap expires.
    pub expiry: u64,
    /// Current lifecycle state.
    pub state: SwapState,

    // ── Funding flags ─────────────────────────────────────
    /// Whether the creator has deposited side A.
    pub creator_funded: bool,
    /// Whether the counterparty has deposited side B.
    pub counterparty_funded: bool,

    /// Ledger timestamp when the swap was created.
    pub created_at: u64,
}

impl Swap {
    /// Determine whether `addr` is the creator or counterparty.
    pub fn is_party(&self, addr: &Address) -> bool {
        *addr == self.creator || *addr == self.counterparty
    }
}
