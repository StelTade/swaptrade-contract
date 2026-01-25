#![cfg(test)]

use super::*;
use soroban_sdk::{Env, String};

#[test]
fn test_ping() {
    let env = Env::default();
    let contract_id = env.register(PingContract, ());
    let client = PingContractClient::new(&env, &contract_id);

    let pong = client.ping();
    assert_eq!(pong, String::from_str(&env, "pong"));
}
