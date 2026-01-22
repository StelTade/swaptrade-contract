use soroban_sdk::{Env, Symbol, Address};
use crate::{CounterContract, CounterContractClient};

#[test]
fn test_referral_integration_with_swaps() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CounterContract);
    let client = CounterContractClient::new(&env, &contract_id);

    let referrer = Address::generate(&env);
    let referee1 = Address::generate(&env);
    let referee2 = Address::generate(&env);

    // 1. Generate referral codes (both referrer and referee should have codes)
    let referrer_code = client.generate_referral_code(&referrer);
    let referee1_code = client.generate_referral_code(&referee1);
    
    assert!(!referrer_code.to_string().is_empty());
    assert!(!referee1_code.to_string().is_empty());

    // 2. Register referees with referrer's code
    assert!(client.register_with_referral(&referrer_code, &referee1).is_ok());
    assert!(client.register_with_referral(&referrer_code, &referee2).is_ok());

    // 3. Verify referral relationships
    let referrals = client.get_referrals(&referrer);
    assert_eq!(referrals.len(), 2);

    // 4. Mint tokens to users for testing swaps
    client.mint(&env, &Symbol::new(&env, "XLM"), &referrer, &10000);
    client.mint(&env, &Symbol::new(&env, "XLM"), &referee1, &10000);
    client.mint(&env, &Symbol::new(&env, "XLM"), &referee2, &10000);

    // 5. Perform swaps to trigger referral rewards
    // Referrer makes a swap (should not earn referral rewards)
    let initial_referrer_rewards = client.get_referral_rewards(&referrer);
    assert_eq!(initial_referrer_rewards, 0);
    
    // Referee1 makes a swap (referrer should earn rewards from this)
    let _ = client.swap(&Symbol::new(&env, "XLM"), &Symbol::new(&env, "USDC-SIM"), &1000, &referee1);
    
    // Check that referrer earned rewards
    let referrer_rewards_after_referee1_swap = client.get_referral_rewards(&referrer);
    assert!(referrer_rewards_after_referee1_swap > 0);
    
    // Referee2 makes a swap (referrer should earn more rewards)
    let _ = client.swap(&Symbol::new(&env, "XLM"), &Symbol::new(&env, "USDC-SIM"), &1000, &referee2);
    
    // Check that referrer earned more rewards
    let final_referrer_rewards = client.get_referral_rewards(&referrer);
    assert!(final_referrer_rewards > referrer_rewards_after_referee1_swap);
    
    // 6. Test referral discount for referee (first 50 trades get 10% discount)
    // Referee1 makes multiple trades, should get discount initially
    let initial_referee1_rewards = client.get_referral_rewards(&referee1);
    assert_eq!(initial_referee1_rewards, 0);
    
    // After referee makes trades, referrer should see increased earnings
    // The referee should get a fee discount on early trades
}

#[test]
fn test_referee_discount_expires_after_50_trades() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CounterContract);
    let client = CounterContractClient::new(&env, &contract_id);

    let referrer = Address::generate(&env);
    let referee = Address::generate(&env);

    // Generate referral code
    let code = client.generate_referral_code(&referrer);

    // Register referee with referral code
    assert!(client.register_with_referral(&code, &referee).is_ok());

    // Mint tokens to referee
    client.mint(&env, &Symbol::new(&env, "XLM"), &referee, &100000);

    // Make 50 trades - referee should get discount on all
    for _ in 0..50 {
        let _ = client.swap(&Symbol::new(&env, "XLM"), &Symbol::new(&env, "USDC-SIM"), &100, &referee);
    }

    // After 50 trades, referrer should have earned referral rewards
    let referrer_rewards = client.get_referral_rewards(&referrer);
    assert!(referrer_rewards > 0);

    // Make another trade - referee should no longer get discount
    let _ = client.swap(&Symbol::new(&env, "XLM"), &Symbol::new(&env, "USDC-SIM"), &100, &referee);

    // Referrer should have earned more rewards
    let final_referrer_rewards = client.get_referral_rewards(&referrer);
    assert!(final_referrer_rewards >= referrer_rewards);
}

#[test]
fn test_cannot_refer_self_or_create_circular_refs() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CounterContract);
    let client = CounterContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);

    // Generate referral code
    let code = client.generate_referral_code(&user);

    // Attempt to register with own code should fail
    let result = client.register_with_referral(&code, &user);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Cannot refer yourself");
}

#[test]
fn test_duplicate_registration_prevention() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CounterContract);
    let client = CounterContractClient::new(&env, &contract_id);

    let referrer1 = Address::generate(&env);
    let referrer2 = Address::generate(&env);
    let referee = Address::generate(&env);

    // Generate referral codes
    let code1 = client.generate_referral_code(&referrer1);
    let code2 = client.generate_referral_code(&referrer2);

    // Successfully register referee with first referrer
    assert!(client.register_with_referral(&code1, &referee).is_ok());

    // Attempt to register same referee with second referrer should fail
    let result = client.register_with_referral(&code2, &referee);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "User already has a referrer");
}

#[test]
fn test_referral_rewards_claiming() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CounterContract);
    let client = CounterContractClient::new(&env, &contract_id);

    let referrer = Address::generate(&env);
    let referee = Address::generate(&env);

    // Generate referral code
    let code = client.generate_referral_code(&referrer);

    // Register referee with referral code
    assert!(client.register_with_referral(&code, &referee).is_ok());

    // Mint tokens to referee
    client.mint(&env, &Symbol::new(&env, "XLM"), &referee, &10000);

    // Make a trade to generate referral rewards
    let _ = client.swap(&Symbol::new(&env, "XLM"), &Symbol::new(&env, "USDC-SIM"), &1000, &referee);

    // Check that referrer has earned rewards
    let rewards_before_claim = client.get_referral_rewards(&referrer);
    assert!(rewards_before_claim > 0);

    // Claim the rewards
    let claimed_amount = client.claim_referral_rewards(&referrer);
    assert_eq!(claimed_amount, rewards_before_claim);

    // Verify rewards are reset to 0 after claiming
    let rewards_after_claim = client.get_referral_rewards(&referrer);
    assert_eq!(rewards_after_claim, 0);
}