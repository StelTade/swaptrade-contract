// emergency.rs
//
// Re-export emergency stub functions for crate-level access.
// trading.rs and swap.rs use `crate::emergency::*` to check emergency state.

pub use crate::emergency_stub::*;
