
#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol};

#[contract]
pub struct AuditLog;

#[contractimpl]
impl AuditLog {
    /// Logs a bridge event.
    pub fn log_bridge_event(env: &Env, event_type: Symbol, details: Symbol) {
        soroban_sdk::log!(env, "Bridge Event: {:?} - {:?}", event_type, details);
    }
}