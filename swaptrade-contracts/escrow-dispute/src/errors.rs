use soroban_sdk::contracterror;

/// Error types for the escrow-dispute contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EscrowError {
    /// Escrow not found in storage.
    EscrowNotFound = 1,
    /// Caller is not an authorized party for this escrow.
    Unauthorized = 2,
    /// Caller lacks the required trustline for the asset.
    MissingTrustline = 3,
    /// Escrow is not in the expected state for this operation.
    InvalidState = 4,
    /// Deposit amount must be strictly positive.
    InvalidAmount = 5,
    /// Timelock duration must be above the configured minimum.
    InvalidTimelock = 6,
    /// Dispute deadline has passed — use auto-refund instead.
    DisputeExpired = 7,
    /// Dispute has already been resolved.
    DisputeAlreadyResolved = 8,
    /// No evidence has been submitted yet.
    NoEvidenceSubmitted = 9,
    /// Token transfer returned an unexpected result.
    TransferFailed = 10,
    /// Signer has already voted on this dispute.
    DuplicateVote = 11,
    /// Not enough multisig signers have voted yet.
    InsufficientSignatures = 12,
    /// Dispute has not reached its deadline yet (for auto-refund).
    DeadlineNotReached = 13,
}
