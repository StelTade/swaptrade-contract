use soroban_sdk::{contracterror, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GovernanceError {
    // ── Access Control ───────────────────────────────────────────────────────
    NotInitialized = 1,
    AlreadyInitialized = 2,
    NotGovernance = 3,
    NotSigner = 4,
    NotProposer = 5,

    // ── Multisig ─────────────────────────────────────────────────────────────
    InvalidThreshold = 10,
    ThresholdExceeded = 11,
    InsufficientSignatures = 12,
    DuplicateSignature = 13,
    ProposalNotFound = 14,
    ProposalAlreadyExecuted = 15,
    ProposalExpired = 16,
    AlreadySigned = 17,
    CannotReclaim = 18,

    // ── Timelock ─────────────────────────────────────────────────────────────
    TimelockNotElapsed = 20,
    TimelockTooShort = 21,

    // ── Upgradeability ───────────────────────────────────────────────────────
    InvalidImplementation = 30,
    UpgradeNotScheduled = 31,
    UpgradeAlreadyExecuted = 32,
    ImplementationUnchanged = 33,

    // ── Pause ────────────────────────────────────────────────────────────────
    ContractPaused = 40,
    ContractNotPaused = 41,

    // ── Validation ───────────────────────────────────────────────────────────
    InvalidAddress = 50,
    InvalidNonce = 51,
    ZeroThreshold = 52,
    EmptySigners = 53,
}

/// Alias kept for modules that still import `ContractError` by name.
pub type ContractError = GovernanceError;
