
#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, String, Bytes, symbol_short};
use soroban_sdk::token::{self, Interface as _};

#[contract]
pub struct WrappedAsset;

#[contractimpl]
impl WrappedAsset {
    /// Initialize the wrapped asset contract.
    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
    ) {
        // TODO: Add access control to ensure this can only be called once.
        env.storage().instance().set(&symbol_short!("admin"), &admin);
        let token = token::Client::new(&env, &env.current_contract_address());
        token.initialize(&admin, &decimals, &name, &symbol);
    }

    /// Mint new wrapped assets.
    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&symbol_short!("admin")).unwrap();
        admin.require_auth();

        let token = token::Client::new(&env, &env.current_contract_address());
        token.mint(&to, &amount);
    }

    /// Burn wrapped assets.
    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();

        let token = token::Client::new(&env, &env.current_contract_address());
        token.burn(&from, &amount);
    }
}