#![cfg(test)]

use super::*;
use soroban_sdk::Env;

#[test]
fn test_increment() {
    let env = Env::default();
    let contract_id = env.register(SwapTradeContract, ());
    let client = SwapTradeContractClient::new(&env, &contract_id);

    assert_eq!(client.get_count(), 0);
    
    assert_eq!(client.increment(), 1);
    assert_eq!(client.get_count(), 1);
    
    assert_eq!(client.increment(), 2);
    assert_eq!(client.get_count(), 2);
}

#[test]
fn test_init() {
    let env = Env::default();
    let contract_id = env.register(SwapTradeContract, ());
    let client = SwapTradeContractClient::new(&env, &contract_id);

    assert_eq!(client.init(), 0);
    assert_eq!(client.get_count(), 0);
}
