#![no_std]
use soroban_sdk::{contract, contractimpl, Env, String};


#[contract]
pub struct PingContract;

#[contractimpl]
impl PingContract {
    pub fn ping(env: Env) -> String {
        String::from_str(&env, "pong")
    }
}

mod test;