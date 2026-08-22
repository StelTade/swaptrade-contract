//! Gas Cost Benchmark Suite for SwapTrade Soroban Contracts
//!
//! Measures Soroban resource consumption (CPU instructions, memory, storage I/O)
//! for core trade-engine operations. Each benchmark captures:
//!   - CPU instructions consumed
//!   - Memory bytes consumed
//!   - Persistent storage read entries
//!   - Persistent storage write entries
//!   - Contract events size (bytes)
//!   - Wall-clock elapsed time
//!
//! Use `cargo test -p gas-benchmarks --test gas_benchmarks -- --nocapture` to run.

use soroban_sdk::{testutils::Address as _, token, Address, Env, Vec};
use trade_engine::{
    OrderSide, OrderType, TradeEngineContract, TradeEngineContractClient, TradeLeg, PRICE_PRECISION,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct GasSnapshot {
    label: String,
    instructions: i64,
    mem_bytes: i64,
    disk_read_entries: u32,
    memory_read_entries: u32,
    write_entries: u32,
    events_bytes: u32,
}

impl GasSnapshot {
    fn capture(env: &Env, label: &str) -> Self {
        let res = env.cost_estimate().resources();
        Self {
            label: label.to_string(),
            instructions: res.instructions,
            mem_bytes: res.mem_bytes,
            disk_read_entries: res.disk_read_entries,
            memory_read_entries: res.memory_read_entries,
            write_entries: res.write_entries,
            events_bytes: res.contract_events_size_bytes,
        }
    }

    fn diff(&self, after: &Self) -> String {
        let di = after.instructions.saturating_sub(self.instructions);
        let dm = after.mem_bytes.saturating_sub(self.mem_bytes);
        let dr = after
            .disk_read_entries
            .saturating_sub(self.disk_read_entries);
        let mr = after
            .memory_read_entries
            .saturating_sub(self.memory_read_entries);
        let dw = after.write_entries.saturating_sub(self.write_entries);
        let de = after.events_bytes.saturating_sub(self.events_bytes);
        format!(
            "{label}: instructions={di} mem_bytes={dm} disk_reads={dr} mem_reads={mr} writes={dw} events={de}",
            label = self.label,
        )
    }
}

fn print_budget_delta(env: &Env, before: &GasSnapshot, after: &GasSnapshot) {
    println!("  {}", before.diff(after));
    println!("  fee estimate (stroops): {:?}", env.cost_estimate().fee());
}

/// Set up a fresh trade engine with two tokens and a funded user.
fn setup() -> (
    Env,
    TradeEngineContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let contract_id = env.register(TradeEngineContract, ());
    let client = TradeEngineContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);

    let base = env.register_stellar_asset_contract_v2(admin.clone());
    let quote = env.register_stellar_asset_contract_v2(admin.clone());

    let base_addr = base.address();
    let quote_addr = quote.address();

    // Fund the user with both tokens
    let base_admin = token::StellarAssetClient::new(&env, &base_addr);
    let quote_admin = token::StellarAssetClient::new(&env, &quote_addr);
    base_admin.mint(&user, &1_000_000_000);
    quote_admin.mint(&user, &1_000_000_000);

    (env, client, admin, user, base_addr, quote_addr)
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Benchmark: place a single sell limit order
#[test]
fn bench_place_order_single() {
    let (env, client, _admin, user, base, quote) = setup();
    let price = 10 * PRICE_PRECISION;
    let amount = 1_000;

    let before = GasSnapshot::capture(&env, "place_order(1 order)");
    let _order_id = client.place_order(
        &user,
        &base,
        &quote,
        &OrderSide::Sell,
        &OrderType::Limit,
        &price,
        &amount,
        &0,
    );
    let after = GasSnapshot::capture(&env, "place_order(1 order)");

    println!("\n=== place_order (sell, 1 order on empty book) ===");
    print_budget_delta(&env, &before, &after);
}

/// Benchmark: place order when the book already has N orders (N = 5, 10, 20)
/// Measures the O(n) sorted-insertion cost in `add_order`.
#[test]
fn bench_place_order_with_existing_orders() {
    for n in [5, 10, 20] {
        let (env, client, _admin, user, base, quote) = setup();
        let price_base = 10 * PRICE_PRECISION;

        // Pre-populate with `n` sell orders at ascending prices
        for i in 0..n {
            let maker = Address::generate(&env);
            let base_admin = token::StellarAssetClient::new(&env, &base);
            base_admin.mint(&maker, &100_000);
            let p = price_base + (i as u128 + 1);
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
        }

        let new_price = price_base - 1; // Will be inserted at the front
        let before = GasSnapshot::capture(&env, &format!("place_order({n} existing)"));
        let _ = client.place_order(
            &user,
            &base,
            &quote,
            &OrderSide::Sell,
            &OrderType::Limit,
            &new_price,
            &100,
            &0,
        );
        let after = GasSnapshot::capture(&env, &format!("place_order({n} existing)"));

        println!("\n=== place_order (after {n} existing sell orders) ===");
        print_budget_delta(&env, &before, &after);
    }
}

/// Benchmark: place a buy order (escrows quote tokens via amount*price computation)
#[test]
fn bench_place_buy_order() {
    let (env, client, _admin, user, base, quote) = setup();
    let price = 10 * PRICE_PRECISION;
    let amount = 1_000;

    let before = GasSnapshot::capture(&env, "place_buy_order");
    let _ = client.place_order(
        &user,
        &base,
        &quote,
        &OrderSide::Buy,
        &OrderType::Limit,
        &price,
        &amount,
        &0,
    );
    let after = GasSnapshot::capture(&env, "place_buy_order");

    println!("\n=== place_order (buy, escrows quote tokens) ===");
    print_budget_delta(&env, &before, &after);
}

/// Benchmark: cancel a pending order and refund escrowed tokens
#[test]
fn bench_cancel_order() {
    let (env, client, _admin, user, base, quote) = setup();
    let price = 10 * PRICE_PRECISION;
    let amount = 1_000;

    let order_id = client.place_order(
        &user,
        &base,
        &quote,
        &OrderSide::Sell,
        &OrderType::Limit,
        &price,
        &amount,
        &0,
    );

    let before = GasSnapshot::capture(&env, "cancel_order");
    client.cancel_order(&user, &order_id);
    let after = GasSnapshot::capture(&env, "cancel_order");

    println!("\n=== cancel_order (pending sell order) ===");
    print_budget_delta(&env, &before, &after);
}

/// Benchmark: cancel a buy order (involves quote-token refund computation)
#[test]
fn bench_cancel_buy_order() {
    let (env, client, _admin, user, base, quote) = setup();
    let price = 10 * PRICE_PRECISION;
    let amount = 1_000;

    let order_id = client.place_order(
        &user,
        &base,
        &quote,
        &OrderSide::Buy,
        &OrderType::Limit,
        &price,
        &amount,
        &0,
    );

    let before = GasSnapshot::capture(&env, "cancel_buy_order");
    client.cancel_order(&user, &order_id);
    let after = GasSnapshot::capture(&env, "cancel_buy_order");

    println!("\n=== cancel_order (pending buy order) ===");
    print_budget_delta(&env, &before, &after);
}

/// Benchmark: fill an order via execute_multi_pair_trade (1 leg, orderbook match)
#[test]
fn bench_execute_trade_1_leg_orderbook() {
    let (env, client, _admin, trader, base, quote) = setup();
    let maker = Address::generate(&env);

    // Setup: maker places a sell order
    let base_admin = token::StellarAssetClient::new(&env, &base);
    base_admin.mint(&maker, &100_000);
    let price = 10 * PRICE_PRECISION;
    let amount = 1_000;
    client.place_order(
        &maker,
        &base,
        &quote,
        &OrderSide::Sell,
        &OrderType::Limit,
        &price,
        &amount,
        &0,
    );

    // Fund trader with quote tokens
    let quote_admin = token::StellarAssetClient::new(&env, &quote);
    let quote_needed = (amount as u128 * price / PRICE_PRECISION) as i128;
    quote_admin.mint(&trader, &quote_needed);

    let legs = soroban_sdk::vec![
        &env,
        TradeLeg {
            base_asset: base,
            quote_asset: quote,
            side: OrderSide::Buy,
            amount: amount / 2,
            limit_price: price,
            min_output_amount: amount / 2,
        },
    ];

    let before = GasSnapshot::capture(&env, "execute_trade(1 leg, OB fill)");
    let result = client.execute_multi_pair_trade(&trader, &legs);
    let after = GasSnapshot::capture(&env, "execute_trade(1 leg, OB fill)");

    assert!(result.success);
    println!("\n=== execute_multi_pair_trade (1 leg, orderbook fill) ===");
    print_budget_delta(&env, &before, &after);
}

/// Benchmark: fill an order via execute_multi_pair_trade with 3 legs
#[test]
fn bench_execute_trade_3_legs() {
    let (env, client, _admin, trader, _base, _quote) = setup();

    let mut tokens = std::vec::Vec::new();
    for _ in 0..4 {
        let t = env.register_stellar_asset_contract_v2(_admin.clone());
        tokens.push(t.address());
    }

    let mut legs = Vec::new(&env);
    let price = 10 * PRICE_PRECISION;
    let amount = 500;

    for i in 0..3 {
        let base = tokens[i].clone();
        let quote = tokens[i + 1].clone();

        let maker = Address::generate(&env);
        let base_admin = token::StellarAssetClient::new(&env, &base);
        base_admin.mint(&maker, &100_000);

        client.place_order(
            &maker,
            &base,
            &quote,
            &OrderSide::Sell,
            &OrderType::Limit,
            &price,
            &amount,
            &0,
        );

        let quote_admin = token::StellarAssetClient::new(&env, &quote);
        let quote_needed = (amount as u128 * price / PRICE_PRECISION) as i128;
        quote_admin.mint(&trader, &quote_needed);

        legs.push_back(TradeLeg {
            base_asset: base,
            quote_asset: quote,
            side: OrderSide::Buy,
            amount: 100,
            limit_price: price,
            min_output_amount: 100,
        });
    }

    let before = GasSnapshot::capture(&env, "execute_trade(3 legs)");
    let result = client.execute_multi_pair_trade(&trader, &legs);
    let after = GasSnapshot::capture(&env, "execute_trade(3 legs)");

    assert!(result.success);
    println!("\n=== execute_multi_pair_trade (3 legs, all orderbook fills) ===");
    print_budget_delta(&env, &before, &after);
}

/// Benchmark: fill an order via liquidity pool fallback
#[test]
fn bench_execute_trade_pool_fallback() {
    let (env, client, admin, trader, base, quote) = setup();

    // Setup: admin creates a liquidity pool
    let base_admin = token::StellarAssetClient::new(&env, &base);
    let quote_admin = token::StellarAssetClient::new(&env, &quote);
    base_admin.mint(&admin, &1_000_000);
    quote_admin.mint(&admin, &1_000_000);

    client.add_liquidity_pool(&admin, &base, &quote, &1_000_000, &1_000_000, &30);

    // Fund trader with base to sell
    base_admin.mint(&trader, &10_000);

    let legs = soroban_sdk::vec![
        &env,
        TradeLeg {
            base_asset: base,
            quote_asset: quote,
            side: OrderSide::Sell,
            amount: 1_000,
            limit_price: 0,
            min_output_amount: 900,
        },
    ];

    let before = GasSnapshot::capture(&env, "execute_trade(pool fallback)");
    let result = client.execute_multi_pair_trade(&trader, &legs);
    let after = GasSnapshot::capture(&env, "execute_trade(pool fallback)");

    assert!(result.success);
    assert!(result.fills.get(0).unwrap().filled_via_pool);
    println!("\n=== execute_multi_pair_trade (1 leg, pool fallback fill) ===");
    print_budget_delta(&env, &before, &after);
}

/// Benchmark: 10-pair atomic trade (stress test)
#[test]
fn bench_execute_trade_10_pairs() {
    let (env, client, admin, trader, _base, _quote) = setup();

    let mut tokens = std::vec::Vec::new();
    for _ in 0..11 {
        let t = env.register_stellar_asset_contract_v2(admin.clone());
        tokens.push(t.address());
    }

    let mut legs = Vec::new(&env);

    for i in 0..10 {
        let base = tokens[i].clone();
        let quote = tokens[i + 1].clone();
        let maker = Address::generate(&env);
        let price = (10 + i as u128) * PRICE_PRECISION;
        let amount = 1_000i128;

        let base_admin = token::StellarAssetClient::new(&env, &base);
        base_admin.mint(&maker, &amount);

        let quote_admin = token::StellarAssetClient::new(&env, &quote);
        let quote_needed = (100u128 * price / PRICE_PRECISION) as i128;
        quote_admin.mint(&trader, &quote_needed);

        client.place_order(
            &maker,
            &base,
            &quote,
            &OrderSide::Sell,
            &OrderType::Limit,
            &price,
            &amount,
            &0,
        );

        legs.push_back(TradeLeg {
            base_asset: base,
            quote_asset: quote,
            side: OrderSide::Buy,
            amount: 100,
            limit_price: price,
            min_output_amount: 100,
        });
    }

    let before = GasSnapshot::capture(&env, "execute_trade(10 pairs)");
    let result = client.execute_multi_pair_trade(&trader, &legs);
    let after = GasSnapshot::capture(&env, "execute_trade(10 pairs)");

    assert!(result.success);
    assert_eq!(result.legs_executed, 10);
    println!("\n=== execute_multi_pair_trade (10 pairs) ===");
    print_budget_delta(&env, &before, &after);
}

/// Benchmark: get_orderbook with N levels
#[test]
fn bench_get_orderbook() {
    for n in [5, 10, 20] {
        let (env, client, _admin, _user, base, quote) = setup();
        let price_base = 10 * PRICE_PRECISION;

        // Populate order book with n sell orders at different prices
        for i in 0..n {
            let maker = Address::generate(&env);
            let base_admin = token::StellarAssetClient::new(&env, &base);
            base_admin.mint(&maker, &100_000);
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
            // Also add buy orders
            let maker2 = Address::generate(&env);
            let quote_admin = token::StellarAssetClient::new(&env, &quote);
            let buy_price = price_base - (i as u128 + 1);
            let quote_needed = (100u128 * buy_price / PRICE_PRECISION) as i128;
            quote_admin.mint(&maker2, &quote_needed);
            client.place_order(
                &maker2,
                &base,
                &quote,
                &OrderSide::Buy,
                &OrderType::Limit,
                &buy_price,
                &100,
                &0,
            );
        }

        let before = GasSnapshot::capture(&env, &format!("get_orderbook({n} levels)"));
        let _summary = client.get_orderbook(&base, &quote, &(n as u32));
        let after = GasSnapshot::capture(&env, &format!("get_orderbook({n} levels)"));

        println!("\n=== get_orderbook ({n} levels, {n} bids + {n} asks) ===");
        print_budget_delta(&env, &before, &after);
    }
}

/// Benchmark: get_user_orders for a user with N active orders
#[test]
fn bench_get_user_orders() {
    for n in [5, 10, 20] {
        let (env, client, _admin, user, base, quote) = setup();
        let price_base = 10 * PRICE_PRECISION;

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

        let before = GasSnapshot::capture(&env, &format!("get_user_orders({n})"));
        let _orders = client.get_user_orders(&user);
        let after = GasSnapshot::capture(&env, &format!("get_user_orders({n})"));

        println!("\n=== get_user_orders ({n} orders) ===");
        print_budget_delta(&env, &before, &after);
    }
}
