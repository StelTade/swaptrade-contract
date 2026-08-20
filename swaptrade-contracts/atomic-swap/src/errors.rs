use soroban_sdk::contracterror;

/// Error types for the atomic swap contract.
///
/// `#[contracterror]` is an **attribute macro** (not a derive macro).
/// It automatically implements `Into<soroban_sdk::Error>` and
/// `TryFrom<soroban_sdk::Error>`, which are required for
/// `env.try_invoke_contract` to work with custom error types.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SwapError {
    /// The swap was not found in storage.
    SwapNotFound = 1,
    /// Caller is not an authorized party for this swap.
    Unauthorized = 2,
    /// The caller lacks the required trustline for the asset.
    MissingTrustline = 3,
    /// The swap is not in the expected state for this operation.
    InvalidState = 4,
    /// Amount must be strictly positive.
    InvalidAmount = 5,
    /// Expiry must be a future timestamp beyond the minimum window.
    InvalidExpiry = 6,
    /// Cannot accept an expired swap.
    Expired = 7,
    /// Asset addresses must differ (no self-swaps).
    SameAsset = 8,
    /// Transfer amount did not match the expected value.
    TransferMismatch = 9,
    /// Trustline check failed for the recipient of a transfer.
    TrustlineCheckFailed = 10,
}
