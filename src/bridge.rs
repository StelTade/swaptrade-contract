
#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol, Vec, Map, Val, symbol_short};

mod rate_limiter;
mod audit_log;
mod wrapped_asset;
mod light_client;
mod circuit_breaker;

// --- Events ---
#[contract]
pub struct Bridge;

#[contractimpl]
impl Bridge {
    /// Deposit event.
    pub fn deposit(env: Env, asset: Address, amount: i128, user: Address) {
        let topics = (symbol_short!("deposit"), user.clone());
        env.events().publish(topics, (asset, amount));
    }

    /// Withdrawal event.
    pub fn withdraw(env: Env, asset: Address, amount: i128, user: Address) {
        let topics = (symbol_short!("withdraw"), user.clone());
        env.events().publish(topics, (asset, amount));
    }

    /// Failure event.
    pub fn failure(env: Env, reason: Symbol, user: Address) {
        let topics = (symbol_short!("failure"), user.clone());
        env.events().publish(topics, reason);
    }
}

// --- Contract ---
#[contract]
pub struct BridgeContract;

#[contractimpl]
impl BridgeContract {
    /// Initialize the bridge contract with the validator set and other parameters.
    pub fn initialize(
        env: Env,
        validators: Map<Address, Val>,
        daily_limit: i128,
        global_limit: i128,
        wrapped_assets: Map<Symbol, Address>,
        light_client: Address,
        circuit_breaker: Address,
    ) {
        // TODO: Add access control to ensure this can only be called once.
        env.storage().instance().set(&Symbol::new(&env, "validators"), &validators);
        env.storage().instance().set(&Symbol::new(&env, "daily_limit"), &daily_limit);
        env.storage().instance().set(&Symbol::new(&env, "global_limit"), &global_limit);
        env.storage().instance().set(&Symbol::new(&env, "attestations"), &Map::<BytesN<32>, Map<Address, Val>>::new(&env));
        env.storage().instance().set(&Symbol::new(&env, "wrapped_assets"), &wrapped_assets);
        env.storage().instance().set(&Symbol::new(&env, "light_client"), &light_client);
        env.storage().instance().set(&Symbol::new(&env, "circuit_breaker"), &circuit_breaker);
    }

    /// Submit an attestation for a deposit event.
    pub fn submit_attestation(
        env: Env,
        tx_hash: BytesN<32>,
        validator: Address,
        asset_symbol: Symbol,
        amount: i128,
        user: Address,
        block_hash: BytesN<32>,
        merkle_proof: BytesN<32>,
    ) {
        Self::check_circuit_breaker(&env);
        validator.require_auth();

        let light_client: Address = env.storage().instance().get(&Symbol::new(&env, "light_client")).unwrap();
        let client = light_client::LightClientClient::new(&env, &light_client);
        if !client.verify_transaction(&tx_hash, &block_hash, &merkle_proof) {
            panic!("Invalid transaction");
        }

        let validators: Map<Address, Val> = env.storage().instance().get(&Symbol::new(&env, "validators")).unwrap();
        if !validators.contains_key(validator.clone()) {
            panic!("Not a valid validator");
        }

        let mut attestations: Map<BytesN<32>, Map<Address, Val>> = env.storage().instance().get(&Symbol::new(&env, "attestations")).unwrap();
        let mut tx_attestations = attestations.get(tx_hash.clone()).unwrap_or(Map::<Address, Val>::new(&env));

        if tx_attestations.contains_key(validator.clone()) {
            panic!("Attestation already submitted");
        }

        tx_attestations.set(validator.clone(), Val::from_void());
        attestations.set(tx_hash.clone(), tx_attestations);
        env.storage().instance().set(&Symbol::new(&env, "attestations"), &attestations);

        // Check if the attestation threshold has been met.
        let tx_attestations: Map<Address, Val> = attestations.get(tx_hash.clone()).unwrap();
        if tx_attestations.len() >= 9 {
            Self::mint_wrapped_asset(env.clone(), asset_symbol, amount, user.clone());
            audit_log::AuditLog::log_bridge_event(&env, symbol_short!("mint"), symbol_short!("threshold_met"));
        }
    }

    /// Withdraw assets from the bridge.
    pub fn withdraw(
        env: Env,
        asset_symbol: Symbol,
        amount: i128,
        user: Address,
    ) {
        Self::check_circuit_breaker(&env);
        user.require_auth();

        if !rate_limiter::SensitiveRateLimiter::check_and_update_rate_limit(&env, &user, amount) {
            panic!("Rate limit exceeded");
        }

        let global_limit: i128 = env.storage().instance().get(&Symbol::new(&env, "global_limit")).unwrap();
        if amount > global_limit {
            Self::trip_circuit_breaker(&env);
            panic!("Global limit exceeded");
        }

        Self::burn_wrapped_asset(env.clone(), asset_symbol.clone(), amount, user.clone());

        // TODO: Implement the withdrawal logic.
        // This will involve locking the assets and submitting an attestation request.

        let wrapped_assets: Map<Symbol, Address> = env.storage().instance().get(&Symbol::new(&env, "wrapped_assets")).unwrap();
        let asset_address = wrapped_assets.get(asset_symbol).unwrap();
        Bridge::withdraw(env.clone(), asset_address, amount, user);
        audit_log::AuditLog::log_bridge_event(&env, symbol_short!("withdraw"), symbol_short!("initiated"));
    }

    /// Mint wrapped assets.
    fn mint_wrapped_asset(env: Env, asset_symbol: Symbol, amount: i128, user: Address) {
        let wrapped_assets: Map<Symbol, Address> = env.storage().instance().get(&Symbol::new(&env, "wrapped_assets")).unwrap();
        let asset_address = wrapped_assets.get(asset_symbol).unwrap();
        let client = wrapped_asset::WrappedAssetClient::new(&env, &asset_address);
        client.mint(&user, &amount);
    }

    /// Burn wrapped assets.
    fn burn_wrapped_asset(env: Env, asset_symbol: Symbol, amount: i128, user: Address) {
        let wrapped_assets: Map<Symbol, Address> = env.storage().instance().get(&Symbol::new(&env, "wrapped_assets")).unwrap();
        let asset_address = wrapped_assets.get(asset_symbol).unwrap();
        let client = wrapped_asset::WrappedAssetClient::new(&env, &asset_address);
        client.burn(&user, &amount);
    }

    /// Check if the circuit breaker is open.
    fn check_circuit_breaker(env: &Env) {
        let circuit_breaker: Address = env.storage().instance().get(&Symbol::new(&env, "circuit_breaker")).unwrap();
        let client = circuit_breaker::CircuitBreakerClient::new(&env, &circuit_breaker);
        if client.is_open() {
            panic!("Circuit breaker is open");
        }
    }

    /// Trip the circuit breaker.
    fn trip_circuit_breaker(env: &Env) {
        let circuit_breaker: Address = env.storage().instance().get(&Symbol::new(&env, "circuit_breaker")).unwrap();
        let client = circuit_breaker::CircuitBreakerClient::new(&env, &circuit_breaker);
        client.trip();
    }

    /// Reset the circuit breaker.
    pub fn reset_circuit_breaker(env: Env, admin: Address) {
        admin.require_auth();
        let circuit_breaker: Address = env.storage().instance().get(&Symbol::new(&env, "circuit_breaker")).unwrap();
        let client = circuit_breaker::CircuitBreakerClient::new(&env, &circuit_breaker);
        client.reset();
    }
}