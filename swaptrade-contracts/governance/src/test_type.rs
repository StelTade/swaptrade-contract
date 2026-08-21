#![cfg_attr(all(not(test), target_family = "wasm"), no_std)]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestProposal {
    pub id: u64,
    pub action: TestAction,
    pub signer: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TestAction {
    Pause,
    Unpause,
    Upgrade(Address),
}

#[contract]
pub struct TestContract;

#[contractimpl]
impl TestContract {
    pub fn get(env: Env) -> TestProposal {
        TestProposal {
            id: 1,
            action: TestAction::Pause,
            signer: Address::generate(&env),
        }
    }
}
