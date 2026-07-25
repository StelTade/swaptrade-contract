
#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol, Map, Val};

#[contract]
pub struct SensitiveRateLimiter;

#[contractimpl]
impl SensitiveRateLimiter {
    /// Checks if the rate limit has been exceeded.
    pub fn check_and_update_rate_limit(
        env: &Env,
        user: &soroban_sdk::Address,
        amount: i128,
    ) -> bool {
        let daily_limit: i128 = env.storage().instance().get(&Symbol::new(&env, "daily_limit")).unwrap();
        let mut user_withdrawal_amounts: Map<soroban_sdk::Address, i128> = env.storage().instance().get(&Symbol::new(&env, "user_withdrawal_amounts")).unwrap_or(Map::<soroban_sdk::Address, i128>::new(&env));

        let user_withdrawal_amount = user_withdrawal_amounts.get(user.clone()).unwrap_or(0);

        if user_withdrawal_amount + amount > daily_limit {
            return false;
        }

        user_withdrawal_amounts.set(user.clone(), user_withdrawal_amount + amount);
        env.storage().instance().set(&Symbol::new(&env, "user_withdrawal_amounts"), &user_withdrawal_amounts);

        true
    }
}