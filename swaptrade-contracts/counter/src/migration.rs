use soroban_sdk::{Env, Symbol};
use crate::portfolio::Portfolio;

pub fn migrate_from_v1_to_v2(env: &Env) -> Result<(), u32> {
    // 1. Check current version
    let current_version = get_stored_version(env);
    
    // If already V2, return success (idempotency)
    if current_version >= 2 {
        return Ok(());
    }

    // 2. Perform data migration
    // We load the portfolio. In a real upgrade, if the struct layout changed incompatibly,
    // we would deserialize into a PortfolioV1 struct, map it to Portfolio (V2), and save.
    // Here we simulate the schema evolution by populating the new `migration_time` field.
    let mut portfolio: Portfolio = env
        .storage()
        .instance()
        .get(&())
        .unwrap_or_else(|| Portfolio::new(env));

    // Update the data structure: Set migration timestamp if it wasn't set (simulating V2 feature)
    if portfolio.migration_time.is_none() {
        portfolio.migration_time = Some(env.ledger().timestamp());
        
        // Save the updated portfolio
        env.storage().instance().set(&(), &portfolio);
    }

    // 3. Update version to 2
    set_stored_version(env, 2);

    Ok(())
}

/// Helper to get version from storage
pub fn get_stored_version(env: &Env) -> u32 {
    env.storage().instance().get(&Symbol::short("v_code")).unwrap_or(0)
}

/// Helper to set version in storage
fn set_stored_version(env: &Env, version: u32) {
    env.storage().instance().set(&Symbol::short("v_code"), &version);
}
