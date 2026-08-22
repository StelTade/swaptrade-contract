use soroban_sdk::{contracttype, Address};

/// Lifecycle states for an escrow.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum EscrowState {
    /// Escrow created by seller, awaiting buyer funding.
    Created,
    /// Both parties have interest but funds not yet deposited.
    Funded,
    /// Buyer has deposited the asset into escrow.
    Escrowed,
    /// Dispute raised — funds frozen until resolution.
    Disputed,
    /// Dispute resolved: funds released to seller.
    Released,
    /// Dispute resolved (or auto-refunded): funds returned to buyer.
    Refunded,
}

/// Status of an active or past dispute.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum DisputeStatus {
    /// Dispute raised, awaiting evidence and resolution.
    Open,
    /// Multisig/admin has resolved in favour of release (seller wins).
    ResolvedRelease,
    /// Multisig/admin has resolved in favour of refund (buyer wins).
    ResolvedRefund,
    /// Timelock expired without resolution — automatic refund.
    AutoRefunded,
}

/// On-chain metadata for a single escrow.
#[derive(Clone, Debug)]
#[contracttype]
pub struct Escrow {
    /// Unique escrow identifier.
    pub id: u64,
    /// Client-supplied nonce for idempotency.
    pub nonce: u64,
    /// The seller who creates and can receive released funds.
    pub seller: Address,
    /// The buyer who funds and can receive refunded funds.
    pub buyer: Address,
    /// Stellar asset contract held in escrow.
    pub asset: Address,
    /// Amount of the asset held in escrow.
    pub amount: i128,
    /// Current lifecycle state.
    pub state: EscrowState,
    /// Ledger timestamp (seconds) when the escrow was created.
    pub created_at: u64,
}

impl Escrow {
    /// Determine whether `addr` is the seller or buyer.
    pub fn is_party(&self, addr: &Address) -> bool {
        *addr == self.seller || *addr == self.buyer
    }
}

/// Metadata for a dispute on an escrow.
#[derive(Clone, Debug)]
#[contracttype]
pub struct Dispute {
    /// The escrow this dispute is about.
    pub escrow_id: u64,
    /// Address that raised the dispute.
    pub raised_by: Address,
    /// Current dispute status.
    pub status: DisputeStatus,
    /// Ledger timestamp when the dispute was raised.
    pub raised_at: u64,
    /// Deadline: if not resolved by this time, auto-refund is allowed.
    pub deadline: u64,
    /// Number of evidence submissions so far.
    pub evidence_count: u64,
    /// Number of votes cast (for release or refund).
    pub vote_count: u32,
}

/// A piece of evidence submitted for a dispute.
#[derive(Clone, Debug)]
#[contracttype]
pub struct DisputeEvidence {
    /// Hash of evidence stored off-chain (IPFS/Arweave CID).
    pub hash: soroban_sdk::BytesN<32>,
    /// Address of the party who submitted this evidence.
    pub submitted_by: Address,
    /// Ledger timestamp when submitted.
    pub submitted_at: u64,
    /// Optional description tag for the evidence.
    pub description: soroban_sdk::Symbol,
}

/// Vote cast by a multisig signer on a dispute.
#[derive(Clone, Debug)]
#[contracttype]
pub struct DisputeVote {
    /// Address of the signer who voted.
    pub signer: Address,
    /// Whether the signer voted for release (true) or refund (false).
    pub in_favour_of_release: bool,
    /// Ledger timestamp when the vote was cast.
    pub voted_at: u64,
}
