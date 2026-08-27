//! Orderbook Gas Optimization Benchmarks
//!
//! Measures the gas cost of orderbook-heavy operations (add_order, remove_order,
//! get_summary) across different order book sizes. These benchmarks establish a
//! baseline for the refactoring in `orderbook.rs`.

use soroban_sdk::{testutils::Address as _, token, Address, Env, Vec};
use trade_engine::{
    OrderSide, OrderType, TradeEngineContract, TradeEngineContractClient, TradeLeg, PRICE_PRECISION,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct GasSnapshot {
    instructions: i64,
    mem_bytes: i64,
    disk_read_entries: u32,
    memory_read_entries: u32,
    write_entries: u32,
    events_bytes: u32,
}

impl GasSnapshot {
    fn capture(env: &Env) -> Self {
        let res = env.cost_estimate().resources();
        Self {
            instructions: res.instructions,
            mem_bytes: res.mem_bytes,
            disk_read_entries: res.disk_read_entries,
            memory_read_entries: res.memory_read_entries,
            write_entries: res.write_entries,
            events_bytes: res.contract_events_size_bytes,
        }
    }

    fn diff(&self, before: &Self) -> String {
        let di = self.instructions.saturating_sub(before.instructions);
        let dm = self.mem_bytes.saturating_sub(before.mem_bytes);
        let dr = self
            .disk_read_entries
            .saturating_sub(before.disk_read_entries);
        let mr = self
            .memory_read_entries
            .saturating_sub(before.memory_read_entries);
        let dw = self.write_entries.saturating_sub(before.write_entries);
        let de = self.events_bytes.saturating_sub(before.events_bytes);
        format!(
            "instructions={di} mem_bytes={dm} disk_reads={dr} mem_reads={mr} writes={dw} events={de}",
        )
    }
}

fn setup() -> (Env, TradeEngineContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let contract_id = env.register(TradeEngineContract, ());
    let client = TradeEngineContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

fn create_token(env: &Env, admin: &Address) -> Address {
    let t = env.register_stellar_asset_contract_v2(admin.clone());
    t.address()
}

fn fund(env: &Env, token: &Address, to: &Address, amount: i128) {
    let c = token::StellarAssetClient::new(env, token);
    c.mint(to, &amount);
}

// ---------------------------------------------------------------------------
// Order book: add_order cost scaling
// ---------------------------------------------------------------------------

/// Measures the cost of placing the Nth sell order on an empty order book.
/// The sorted-insertion in `add_order` rebuilds the Vec each time, so cost
/// should scale roughly O(n).
#[test]
fn bench_add_order_scaling() {
    let (env, client, admin) = setup();
    let base = create_token(&env, &admin);
    let quote = create_token(&env, &admin);
    let price_base = 10 * PRICE_PRECISION;

    for n in [1, 2, 5, 10, 20, 50] {
        let mut placed = 0u32;
        for i in 0..n {
            let maker = Address::generate(&env);
            fund(&env, &base, &maker, 100_000);
            let p = price_base + (i as u128);
            client.place_order(
                &maker,
                &base,
                &quote,
                &OrderSide::Sell,
                &OrderType::Limit,
                &p,
                &100,
                &0,
            );
            placed += 1;
        }

        // Now measure cost of placing the NEXT order
        let new_maker = Address::generate(&env);
        fund(&env, &base, &new_maker, 100_000);
        let new_price = price_base - 1;

        let before = GasSnapshot::capture(&env);
        let _ = client.place_order(
            &new_maker,
            &base,
            &quote,
            &OrderSide::Sell,
            &OrderType::Limit,
            &new_price,
            &100,
            &0,
        );
        let after = GasSnapshot::capture(&env);

        println!("add_order (after {n} existing): {}", after.diff(&before));
    }
}

/// Measures the cost of removing orders from the book.
/// `remove_order_id` rebuilds the Vec filtering out the target.
#[test]
fn bench_remove_order_scaling() {
    let (env, client, admin) = setup();
    let base = create_token(&env, &admin);
    let quote = create_token(&env, &admin);
    let price_base = 10 * PRICE_PRECISION;

    for n in [5, 10, 20, 50] {
        let mut order_ids = std::vec::Vec::new();
        for i in 0..n {
            let maker = Address::generate(&env);
            fund(&env, &base, &maker, 100_000);
            let p = price_base + (i as u128);
            let oid = client.place_order(
                &maker,
                &base,
                &quote,
                &OrderSide::Sell,
                &OrderType::Limit,
                &p,
                &100,
                &0,
            );
            order_ids.push((maker, oid));
        }

        // Cancel the last order (worst case: must scan entire list)
        let (owner, oid) = order_ids.last().unwrap().clone();

        let before = GasSnapshot::capture(&env);
        client.cancel_order(&owner, &oid);
        let after = GasSnapshot::capture(&env);

        println!(
            "cancel_order (book has {n} orders): {}",
            after.diff(&before)
        );
    }
}

/// Measures the cost of get_orderbook with varying numbers of price levels.
/// The `get_summary` function iterates all order IDs and loads each order.
#[test]
fn bench_get_orderbook_scaling() {
    let (env, client, admin) = setup();
    let base = create_token(&env, &admin);
    let quote = create_token(&env, &admin);
    let price_base = 10 * PRICE_PRECISION;

    for n in [5, 10, 20, 50] {
        for i in 0..n {
            // Sell order
            let sell_maker = Address::generate(&env);
            fund(&env, &base, &sell_maker, 100_000);
            client.place_order(
                &sell_maker,
                &base,
                &quote,
                &OrderSide::Sell,
                &OrderType::Limit,
                &(price_base + i as u128),
                &100,
                &0,
            );

            // Buy order
            let buy_maker = Address::generate(&env);
            let buy_price = price_base - (i as u128 + 1);
            let quote_needed = (100u128 * buy_price / PRICE_PRECISION) as i128;
            fund(&env, &quote, &buy_maker, quote_needed);
            client.place_order(
                &buy_maker,
                &base,
                &quote,
                &OrderSide::Buy,
                &OrderType::Limit,
                &buy_price,
                &100,
                &0,
            );
        }

        let before = GasSnapshot::capture(&env);
        let _summary = client.get_orderbook(&base, &quote, &100);
        let after = GasSnapshot::capture(&env);

        println!(
            "get_orderbook ({} bids + {} asks, requesting 100): {}",
            n,
            n,
            after.diff(&before)
        );
    }
}

/// Measures get_user_orders scaling with the number of orders a user has placed.
#[test]
fn bench_get_user_orders_scaling() {
    let (env, client, admin) = setup();
    let base = create_token(&env, &admin);
    let quote = create_token(&env, &admin);
    let price_base = 10 * PRICE_PRECISION;
    let user = Address::generate(&env);
    fund(&env, &base, &user, 10_000_000);
    fund(&env, &quote, &user, 10_000_000);

    for n in [5, 10, 20, 50] {
        for i in 0..n {
            let p = price_base + (i as u128);
            client.place_order(
                &user,
                &base,
                &quote,
                &OrderSide::Sell,
                &OrderType::Limit,
                &p,
                &100,
                &0,
            );
        }

        let before = GasSnapshot::capture(&env);
        let orders = client.get_user_orders(&user);
        let after = GasSnapshot::capture(&env);

        println!(
            "get_user_orders ({n} orders): {} (returned {})",
            after.diff(&before),
            orders.len()
        );
    }
}
