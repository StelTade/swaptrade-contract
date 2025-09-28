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