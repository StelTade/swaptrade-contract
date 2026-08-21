use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GovernanceError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    NotGovernance = 3,
    NotSigner = 4,
    NotProposer = 5,
    InvalidThreshold = 10,
    ThresholdExceeded = 11,
    InsufficientSignatures = 12,
    DuplicateSignature = 13,
    ProposalNotFound = 14,
    ProposalAlreadyExecuted = 15,
    ProposalExpired = 16,
    AlreadySigned = 17,
    CannotReclaim = 18,
    TimelockNotElapsed = 20,
    TimelockTooShort = 21,
    InvalidImplementation = 30,
    UpgradeNotScheduled = 31,
    UpgradeAlreadyExecuted = 32,
    ImplementationUnchanged = 33,
    ContractPaused = 40,
    ContractNotPaused = 41,
    InvalidAddress = 50,
    InvalidNonce = 51,
    ZeroThreshold = 52,
    EmptySigners = 53,
}