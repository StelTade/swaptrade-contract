pub mod delegation;
pub mod emergency;
pub mod multi_asset_staking;
pub mod multi_sig;
pub mod quadratic_voting;
pub mod rewards;
pub mod treasury;
pub mod upgrade;
pub mod voting;
pub mod delegation;
pub mod upgrade_manager;

// The full governance system (GovernanceContract, MultiSigCoordinator, Timelock, etc.)
// uses std types (HashMap, HashSet) and external crates (sha2, serde, hex) which are
// only available in test/dev mode. Gate behind cfg(test) for Soroban wasm builds.
#[cfg(test)]
pub mod governance;
#[cfg(test)]
pub mod admin;
#[cfg(test)]
pub mod multi_asset_staking;

// Re-export key types for convenience (test-only since governance.rs uses std)
#[cfg(test)]
pub use governance::{
    MultiSigCoordinator, MultiSigProposal, MULTISIG_THRESHOLD, MULTISIG_TOTAL,
    GovernanceContract, GovernancePhase, Timelock, TimelockEntry,
    DecentralizationSchedule, GovernanceLog, GovernanceEvent,
    SchnorrProof, make_schnorr_proof, verify_schnorr_proof_test_compat,
    now_secs, TIMELOCK_DELAY_SECS, SECS_PER_MONTH,
    DecentralizationStatus,
};
#[cfg(test)]
pub use admin::AdminController;
