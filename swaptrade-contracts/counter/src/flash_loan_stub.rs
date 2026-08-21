// flash_loan_stub.rs
//
// Stub flash loan module.

use soroban_sdk::{Address, Env, Symbol, Vec};

pub struct FlashLoanManager;

impl FlashLoanManager {
    pub fn flash_loan(
        _env: &Env,
        _pool_id: u64,
        _receiver: Address,
        _asset: Symbol,
        _amount: i128,
        _data: Vec<u8>,
    ) -> Result<i128, crate::errors::SwapTradeError> {
        Err(crate::errors::SwapTradeError::NotAuthorized)
    }
}
