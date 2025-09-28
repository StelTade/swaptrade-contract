use soroban_sdk::{contracttype, Address, Env, Symbol, Map, Vec};

#[derive(Clone)]
#[contracttype]
pub enum Asset {
    XLM,
    Custom(Symbol),
}

#[derive(Clone)]
#[contracttype]
pub struct Portfolio {
    balances: Map<(Address, Asset), i128>,
    trades: Map<Address, u32>,       // number of trades per user
    pnl: Map<Address, i128>,         // cumulative balance change placeholder
}

impl Portfolio {
    pub fn new() -> Self {
        Self {
            balances: Map::new(),
            trades: Map::new(),
            pnl: Map::new(),
        }
    }

    /// Mint tokens (XLM or a custom token) to a user’s balance.
    pub fn mint(&mut self, env: &Env, token: Asset, to: Address, amount: i128) {
        assert!(amount > 0, "Amount must be positive");

        let key = (to.clone(), token.clone());
        let current = self.balances.get(env, &key).unwrap_or(0);
        let new_balance = current + amount;

        self.balances.set(env, &key, &new_balance);

        // Update PnL placeholder
        let current_pnl = self.pnl.get(env, &to).unwrap_or(0);
        self.pnl.set(env, &to, &(current_pnl + amount));
    }

    /// Record a swap execution (increase trade count).
    pub fn record_trade(&mut self, env: &Env, user: Address) {
        let count = self.trades.get(env, &user).unwrap_or(0);
        self.trades.set(env, &user, &(count + 1));
    }

    /// Get balance of a token for a given user.
    pub fn balance_of(&self, env: &Env, token: Asset, user: Address) -> i128 {
        let key = (user, token);
        self.balances.get(env, &key).unwrap_or(0)
    }

    /// Get portfolio stats for a given user.
    pub fn get_portfolio(&self, env: &Env, user: Address) -> (u32, i128, Vec<(Asset, i128)>) {
        let trades = self.trades.get(env, &user).unwrap_or(0);
        let pnl = self.pnl.get(env, &user).unwrap_or(0);

        // Collect balances for all assets the user has
        let mut balances_vec: Vec<(Asset, i128)> = Vec::new(env);
        for ((addr, asset), bal) in self.balances.iter(env) {
            if addr == user {
                balances_vec.push_back((asset, bal));
            }
        }

        (trades, pnl, balances_vec)
    }
}