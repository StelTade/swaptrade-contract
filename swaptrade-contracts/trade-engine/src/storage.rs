use soroban_sdk::{contracttype, Address, Env};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    NextOrderId,
    NextPoolId,
    Order(u64),
    // Pair (Asset A, Asset B) order book key
    OrderBook(Address, Address),
    UserOrders(Address),
    LiquidityPool(u64),
    PairPool(Address, Address),
    SlippageConfig,
    Admin,
}

pub fn get_next_order_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&StorageKey::NextOrderId)
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&StorageKey::NextOrderId, &(id + 1));
    id
}

pub fn get_next_pool_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&StorageKey::NextPoolId)
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&StorageKey::NextPoolId, &(id + 1));
    id
}
