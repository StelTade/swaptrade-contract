#![cfg(test)]

use soroban_sdk::{Env, Symbol, Address, testutils::Address as _};
use crate::{CounterContract, CounterContractClient};

#[test]
fn test_migration_v1_to_v2() {
    let env = Env::default();
    env.mock_all_auths();
    
    // Register contract
    let contract_id = env.register_contract(None, CounterContract);
    let client = CounterContractClient::new(&env, &contract_id);

    // 1. Initialize (sets version to 1)
    client.initialize();

    // Verify version is 1
    assert_eq!(client.get_contract_version(), 1);

    // 2. Create a user and some state (simulating V1 usage)
    let user = Address::generate(&env);
    // Mint creates a Portfolio. Since Portfolio::new sets migration_time to None, 
    // this effectively simulates a V1 portfolio (where the field didn't exist/was null).
    client.mint(&Symbol::short("XLM"), &user, &1000);

    // Verify data exists
    assert_eq!(client.get_balance(&Symbol::short("XLM"), &user), 1000);

    // 3. Perform Migration
    // This should detect version < 2, detect migration_time is None, set it, and bump version.
    client.migrate();

    // 4. Verify version is 2
    assert_eq!(client.get_contract_version(), 2);

    // 5. Verify data still exists (old data accessible)
    assert_eq!(client.get_balance(&Symbol::short("XLM"), &user), 1000);

    // 6. Idempotency check
    // Calling migrate again should do nothing and stay at version 2
    client.migrate();
    assert_eq!(client.get_contract_version(), 2);
    
    // Optional: We could add a getter to verify migration_time is Some, 
    // but the version bump implies the logic executed.
}
