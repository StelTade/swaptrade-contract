use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env};

#[test]
fn test_set_admin_requires_admin() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let random_user = Address::generate(&env);

    // Initialize admin
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);

    // Non-admin user should fail to set admin
    let result = client.try_set_admin(&random_user, &new_admin);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), SwapTradeError::NotAdmin);
}

#[test]
fn test_set_admin_emits_event() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    // Initialize admin
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);

    // Admin can set new admin
    client.set_admin(&admin, &new_admin);

    // Verify admin changed
    let stored_admin: Address = env.storage().persistent().get(&ADMIN_KEY).unwrap();
    assert_eq!(stored_admin, new_admin);

    // Verify event was emitted
    let events = env.events().all();
    assert!(events.len() > 0);
    let last_event = events.get(events.len() - 1).unwrap();
    assert_eq!(last_event.topic, (symbol_short!("AdminChanged"),));
}

#[test]
fn test_pause_trading_requires_admin() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let random_user = Address::generate(&env);

    // Initialize admin
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);

    // Non-admin user should fail to pause trading
    let result = client.try_pause_trading(&random_user);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), SwapTradeError::NotAdmin);
}

#[test]
fn test_pause_trading_emits_event_and_sets_flag() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Initialize admin
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);

    // Admin can pause trading
    let result = client.pause_trading(&admin);
    assert!(result.unwrap());

    // Verify paused flag is set
    let paused: bool = env.storage().persistent().get(&PAUSED_KEY).unwrap_or(false);
    assert!(paused);

    // Verify event was emitted
    let events = env.events().all();
    assert!(events.len() > 0);
    let last_event = events.get(events.len() - 1).unwrap();
    assert_eq!(last_event.topic, (symbol_short!("AdminPaused"), admin));
}

#[test]
fn test_resume_trading_requires_admin() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let random_user = Address::generate(&env);

    // Initialize admin and pause trading
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);
    env.storage()
        .persistent()
        .set(&PAUSED_KEY, &true);

    // Non-admin user should fail to resume trading
    let result = client.try_resume_trading(&random_user);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), SwapTradeError::NotAdmin);
}

#[test]
fn test_resume_trading_emits_event_and_clears_flag() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Initialize admin and pause trading
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);
    env.storage()
        .persistent()
        .set(&PAUSED_KEY, &true);

    // Admin can resume trading
    let result = client.resume_trading(&admin);
    assert!(result.unwrap());

    // Verify paused flag is cleared
    let paused: bool = env.storage().persistent().get(&PAUSED_KEY).unwrap_or(false);
    assert!(!paused);

    // Verify event was emitted
    let events = env.events().all();
    assert!(events.len() > 0);
    let last_event = events.get(events.len() - 1).unwrap();
    assert_eq!(last_event.topic, (symbol_short!("AdminResumed"), admin));
}

#[test]
fn test_swap_rejects_when_paused() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Initialize admin and pause trading
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);
    env.storage()
        .persistent()
        .set(&PAUSED_KEY, &true);

    // Mint tokens to user
    client.mint(&xlm, &user, &1000);

    // Swap should fail with TradingPaused error
    let result = client.try_swap(&xlm, &usdc, &500, &user);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::TradingPaused);
}

#[test]
fn test_safe_swap_returns_zero_when_paused() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Initialize admin and pause trading
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);
    env.storage()
        .persistent()
        .set(&PAUSED_KEY, &true);

    // Mint tokens to user
    client.mint(&xlm, &user, &1000);

    // safe_swap should return 0 when paused
    let deadline = env.ledger().timestamp() + 1000;
    let result = client.safe_swap(&xlm, &usdc, &500, &user, &deadline);
    assert_eq!(result, 0);
}

#[test]
fn test_add_liquidity_rejects_when_paused() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Initialize admin and pause trading
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);
    env.storage()
        .persistent()
        .set(&PAUSED_KEY, &true);

    // Mint tokens to user
    client.mint(&xlm, &user, &1000);
    client.mint(&usdc, &user, &1000);

    // add_liquidity should fail with TradingPaused error
    let result = client.try_add_liquidity(&500, &500, &user);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::TradingPaused);
}

#[test]
fn test_remove_liquidity_rejects_when_paused() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize admin and pause trading
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);
    env.storage()
        .persistent()
        .set(&PAUSED_KEY, &true);

    // remove_liquidity should fail with TradingPaused error
    let result = client.try_remove_liquidity(&100, &user);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::TradingPaused);
}

#[test]
fn test_pool_swap_rejects_when_paused() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let trader = Address::generate(&env);
    let xlm = symbol_short!("XLM");

    // Initialize admin and pause trading
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);
    env.storage()
        .persistent()
        .set(&PAUSED_KEY, &true);

    // pool_swap should fail with TradingPaused error
    let result = client.try_pool_swap(&1, &xlm, &100, &0, &trader);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::TradingPaused);
}

#[test]
fn test_execute_batch_atomic_rejects_when_paused() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Initialize admin and pause trading
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);
    env.storage()
        .persistent()
        .set(&PAUSED_KEY, &true);

    // Create batch operation
    let mut operations = Vec::new(&env);
    operations.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 100, user.clone()));

    // execute_batch_atomic should fail when paused
    let result = client.execute_batch_atomic(operations);
    assert_eq!(result.operations_failed, 1);
}

#[test]
fn test_execute_batch_best_effort_rejects_when_paused() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Initialize admin and pause trading
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);
    env.storage()
        .persistent()
        .set(&PAUSED_KEY, &true);

    // Create batch operation
    let mut operations = Vec::new(&env);
    operations.push_back(BatchOperation::Swap(xlm.clone(), usdc.clone(), 100, user.clone()));

    // execute_batch_best_effort should fail when paused
    let result = client.execute_batch_best_effort(operations);
    assert_eq!(result.operations_failed, 1);
}

#[test]
fn test_swap_succeeds_after_resume() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDCSIM");

    // Initialize admin
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);

    // Mint tokens to user
    client.mint(&xlm, &user, &1000);

    // Pause trading
    client.pause_trading(&admin);

    // Swap should fail
    let result = client.try_swap(&xlm, &usdc, &500, &user);
    assert!(result.is_err());

    // Resume trading
    client.resume_trading(&admin);

    // Swap should succeed
    let result = client.swap(&xlm, &usdc, &500, &user);
    assert!(result.is_ok());
}

#[test]
fn test_set_treasury_requires_admin() {
    let env = Env::default();
    let contract_id = env.register(CounterContract, ());
    let client = CounterContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_treasury = Address::generate(&env);
    let random_user = Address::generate(&env);

    // Initialize admin
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, &admin);

    // Non-admin user should fail to set treasury
    let result = client.try_set_treasury(&random_user, &new_treasury);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), SwapTradeError::NotAdmin);
}
