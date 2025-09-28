use soroban_sdk::{contracttype, Address, Env, Symbol, Map};

#[derive(Clone)]
#[contracttype]
pub enum Asset {
    XLM,
    Custom(Symbol), // e.g., "USDC-SIM"
}

#[derive(Clone)]
#[contracttype]
pub struct Portfolio {
    balances: Map<(Address, Asset), i128>,
}

impl Portfolio {
    pub fn new() -> Self {
        Self {
            balances: Map::new(),
        }
    }

    /// Mint tokens (XLM or a custom token) to a user’s balance.
    pub fn mint(&mut self, env: &Env, token: Asset, to: Address, amount: i128) {
        assert!(amount > 0, "Amount must be positive");

        let key = (to.clone(), token.clone());
        let current = self.balances.get(env, &key).unwrap_or(0);
        let new_balance = current + amount;

        self.balances.set(env, &key, &new_balance);
    }

    /// Get balance of a token for a given user.
    /// Returns 0 if no balance exists for the requested token/address.
    pub fn balance_of(&self, env: &Env, token: Asset, user: Address) -> i128 {
        let key = (user, token);
        self.balances.get(env, &key).unwrap_or(0)
    }
}


#[test]
#[should_panic(expected = "Amount must be positive")] 
fn test_mint_negative_should_panic() {
    let env = Env::default(); 
    let user = Address::generate(&env); 
    let mut portfolio = Portfolio::new(); 

    // This should panic 
    portfolio.mint(&env, Asset::XLM, user.clone(), -100);
}

#[test]
fn test_balance_of_returns_zero_for_new_user() {
    let env = Env::default();
    let user = Address::generate(&env);
    let portfolio = Portfolio::new();
    
    // Should return 0 for a user with no balance
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), 0);
}

#[test]
fn test_balance_of_returns_correct_balance_after_mint() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new();
    let amount = 1000;
    
    // Mint some tokens
    portfolio.mint(&env, Asset::XLM, user.clone(), amount);
    
    // Should return the minted amount
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), amount);
}

#[test]
fn test_balance_of_returns_updated_balance_after_multiple_mints() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new();
    
    // First mint
    portfolio.mint(&env, Asset::XLM, user.clone(), 500);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user.clone()), 500);
    
    // Second mint
    portfolio.mint(&env, Asset::XLM, user.clone(), 300);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user.clone()), 800);
    
    // Third mint
    portfolio.mint(&env, Asset::XLM, user.clone(), 200);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), 1000);
}

#[test]
fn test_balance_of_works_with_custom_assets() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new();
    let custom_asset = Asset::Custom(soroban_sdk::symbol_short!("USDC"));
    
    // Mint to custom asset
    portfolio.mint(&env, custom_asset.clone(), user.clone(), 2000);
    
    // Should return correct balance for custom asset
    assert_eq!(portfolio.balance_of(&env, custom_asset, user), 2000);
}

#[test]
fn test_balance_of_isolates_different_users() {
    let env = Env::default();
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let mut portfolio = Portfolio::new();
    
    // Mint to user1
    portfolio.mint(&env, Asset::XLM, user1.clone(), 1000);
    
    // user1 should have balance, user2 should have 0
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user1), 1000);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user2), 0);
}