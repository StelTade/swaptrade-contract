//! Performance benchmarks for SwapTrade contract operations
//! Uses Criterion for statistical benchmarking

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use soroban_sdk::{symbol_short, Address, Env, Symbol};
use swaptrade_contracts::{CounterContract, CounterContractClient};

const SWAP_AMOUNT: i128 = 1000;
const LIQUIDITY_AMOUNT: i128 = 1000000;
const QUERY_LIMIT: u32 = 100;

// Set up the contract environment for benchmarking
fn setup_benchmark_env() -> (Env, CounterContractClient) {
    let env = Env::default();
    let contract_id = env.register_contract(None, CounterContract);
    let client = CounterContractClient::new(&env, &contract_id);
    
    // Set up initial state
    let user = Address::generate(&env);
    
    // Set up initial prices
    client.set_price(
        (symbol_short!("XLM"), symbol_short!("USDCSIM")),
        1_000_000_000_000_000_000 // 1:1 ratio
    );
    
    // Set up liquidity pools
    client.set_pool_liquidity(symbol_short!("XLM"), LIQUIDITY_AMOUNT);
    client.set_pool_liquidity(symbol_short!("USDCSIM"), LIQUIDITY_AMOUNT);
    
    // Mint initial tokens for user
    client.mint(symbol_short!("XLM"), &user, LIQUIDITY_AMOUNT / 10);
    client.mint(symbol_short!("USDCSIM"), &user, LIQUIDITY_AMOUNT / 10);
    
    (env, client)
}

// Benchmark the swap function
fn bench_swap(c: &mut Criterion) {
    let (_env, client) = setup_benchmark_env();
    let user = Address::generate(&client.env());
    client.mint(symbol_short!("XLM"), &user, 1000000);
    
    c.bench_with_input(
        BenchmarkId::new("swap", "basic"),
        &(&client, &user),
        |b, (client, user)| {
            b.iter(|| {
                black_box(client.swap(
                    symbol_short!("XLM"),
                    symbol_short!("USDCSIM"),
                    SWAP_AMOUNT,
                    user
                ))
            })
        },
    );
}

// Benchmark the get_portfolio function
fn bench_get_portfolio(c: &mut Criterion) {
    let (_env, client) = setup_benchmark_env();
    let user = Address::generate(&client.env());
    client.mint(symbol_short!("XLM"), &user, 1000000);
    
    // Execute a swap first to populate portfolio data
    client.swap(
        symbol_short!("XLM"),
        symbol_short!("USDCSIM"),
        SWAP_AMOUNT,
        &user
    );
    
    c.bench_with_input(
        BenchmarkId::new("get_portfolio", "with_data"),
        &(&client, &user),
        |b, (client, user)| {
            b.iter(|| {
                black_box(client.get_portfolio(user))
            })
        },
    );
}

// Benchmark the get_top_traders function
fn bench_get_top_traders(c: &mut Criterion) {
    let (_env, client) = setup_benchmark_env();
    
    // Create multiple users and execute trades to populate trader data
    for i in 0..10 {
        let user = Address::generate(&client.env());
        client.mint(symbol_short!("XLM"), &user, 1000000);
        
        // Execute multiple swaps to populate trader data
        for _ in 0..10 {
            client.swap(
                symbol_short!("XLM"),
                symbol_short!("USDCSIM"),
                SWAP_AMOUNT,
                &user
            );
        }
    }
    
    c.bench_with_input(
        BenchmarkId::new("get_top_traders", "100_users"),
        &(&client, QUERY_LIMIT),
        |b, (client, limit)| {
            b.iter(|| {
                black_box(client.get_top_traders(*limit))
            })
        },
    );
}

// Benchmark batch operations
fn bench_batch_operations(c: &mut Criterion) {
    let (_env, client) = setup_benchmark_env();
    let user = Address::generate(&client.env());
    client.mint(symbol_short!("XLM"), &user, 10000000); // More for multiple swaps
    
    // Prepare 5 swap operations
    let operations = {
        let env = &client.env();
        let mut ops = soroban_sdk::vec![env];
        for _ in 0..5 {
            ops.push_back(
                swaptrade_contracts::BatchOperation::Swap(
                    symbol_short!("XLM"),
                    symbol_short!("USDCSIM"),
                    SWAP_AMOUNT,
                    user.clone()
                )
            );
        }
        ops
    };
    
    c.bench_with_input(
        BenchmarkId::new("batch_execute", "5_ops"),
        &(&client, operations),
        |b, (client, ops)| {
            b.iter(|| {
                black_box(client.execute_batch(ops.clone()))
            })
        },
    );
}

// Benchmark sequential operations for comparison
fn bench_sequential_swaps(c: &mut Criterion) {
    let (_env, client) = setup_benchmark_env();
    let user = Address::generate(&client.env());
    client.mint(symbol_short!("XLM"), &user, 10000000); // More for multiple swaps
    
    c.bench_with_input(
        BenchmarkId::new("sequential_swaps", "5_ops"),
        &(&client, &user),
        |b, (client, user)| {
            b.iter(|| {
                for _ in 0..5 {
                    black_box(client.swap(
                        symbol_short!("XLM"),
                        symbol_short!("USDCSIM"),
                        SWAP_AMOUNT,
                        user
                    ));
                }
            })
        },
    );
}

// Benchmark add_liquidity function
fn bench_add_liquidity(c: &mut Criterion) {
    let (_env, client) = setup_benchmark_env();
    let user = Address::generate(&client.env());
    
    // Mint tokens for liquidity provision
    client.mint(symbol_short!("XLM"), &user, 10000000);
    client.mint(symbol_short!("USDCSIM"), &user, 10000000);
    
    c.bench_with_input(
        BenchmarkId::new("add_liquidity", "xlm_usdcsim"),
        &(&client, &user),
        |b, (client, user)| {
            b.iter(|| {
                // Note: This would need to use the actual add_liquidity function
                // Since it's not exposed directly in the contract, we'll simulate
                // by calling set_pool_liquidity which is the underlying operation
                black_box(client.set_pool_liquidity(symbol_short!("XLM"), 5000000));
            })
        },
    );
}

// Custom benchmark for measuring performance against targets
fn bench_performance_targets(c: &mut Criterion) {
    let mut group = c.benchmark_group("performance_targets");
    
    // Swap target: <10ms
    group.bench_function("swap_target", |b| {
        let (_env, client) = setup_benchmark_env();
        let user = Address::generate(&client.env());
        client.mint(symbol_short!("XLM"), &user, 1000000);
        
        b.iter(|| {
            black_box(client.swap(
                symbol_short!("XLM"),
                symbol_short!("USDCSIM"),
                SWAP_AMOUNT,
                &user
            ))
        })
    });
    
    // Query target: <5ms
    group.bench_function("query_target", |b| {
        let (_env, client) = setup_benchmark_env();
        let user = Address::generate(&client.env());
        client.mint(symbol_short!("XLM"), &user, 1000000);
        
        // Execute a swap first to populate portfolio data
        client.swap(
            symbol_short!("XLM"),
            symbol_short!("USDCSIM"),
            SWAP_AMOUNT,
            &user
        );
        
        b.iter(|| {
            black_box(client.get_portfolio(&user))
        })
    });
    
    group.finish();
}

// Benchmark orderbook operations
fn bench_place_order(c: &mut Criterion) {
    let (_env, client) = setup_benchmark_env();
    
    c.bench_with_input(
        BenchmarkId::new("place_limit_order", "single"),
        &client,
        |b, client| {
            b.iter(|| {
                let maker = Address::generate(&client.env());
                client.mint(symbol_short!("XLM"), &maker, 1000000);
                
                black_box(client.place_limit_order(
                    symbol_short!("XLM"),
                    symbol_short!("USDCSIM"),
                    crate::swaptrade_contracts::orders::OrderSide::Sell,
                    1000,
                    1_000_000_000_000_000_000, // 1:1 price
                    None,
                    &maker
                ))
            })
        },
    );
}

fn bench_take_order(c: &mut Criterion) {
    let (_env, client) = setup_benchmark_env();
    let maker = Address::generate(&client.env());
    client.mint(symbol_short!("XLM"), &maker, 1000000);
    
    // Pre-place an order to take
    client.place_limit_order(
        symbol_short!("XLM"),
        symbol_short!("USDCSIM"),
        crate::swaptrade_contracts::orders::OrderSide::Sell,
        1000,
        1_000_000_000_000_000_000,
        None,
        &maker
    );
    
    c.bench_with_input(
        BenchmarkId::new("take_order", "full_fill"),
        &client,
        |b, client| {
            b.iter(|| {
                let taker = Address::generate(&client.env());
                client.mint(symbol_short!("USDCSIM"), &taker, 1000000);
                
                black_box(client.take_order(
                    symbol_short!("XLM"),
                    symbol_short!("USDCSIM"),
                    crate::swaptrade_contracts::orders::OrderSide::Buy,
                    1000,
                    None,
                    &taker
                ))
            })
        },
    );
}

fn bench_get_orderbook_snapshot(c: &mut Criterion) {
    let (_env, client) = setup_benchmark_env();
    
    // Pre-populate orderbook with multiple orders
    for _ in 0..20 {
        let maker = Address::generate(&client.env());
        client.mint(symbol_short!("XLM"), &maker, 1000000);
        client.mint(symbol_short!("USDCSIM"), &maker, 1000000);
        
        // Place both buy and sell orders
        client.place_limit_order(
            symbol_short!("XLM"),
            symbol_short!("USDCSIM"),
            crate::swaptrade_contracts::orders::OrderSide::Sell,
            500,
            1_000_000_000_000_000_000,
            None,
            &maker
        ).unwrap();
        
        client.place_limit_order(
            symbol_short!("XLM"),
            symbol_short!("USDCSIM"),
            crate::swaptrade_contracts::orders::OrderSide::Buy,
            500,
            990_000_000_000_000_000,
            None,
            &maker
        ).unwrap();
    }
    
    c.bench_with_input(
        BenchmarkId::new("get_orderbook_snapshot", "10_levels"),
        &client,
        |b, client| {
            b.iter(|| {
                black_box(client.get_orderbook_snapshot(
                    symbol_short!("XLM"),
                    symbol_short!("USDCSIM"),
                    10
                ))
            })
        },
    );
}

criterion_group!(
    name = contract_benchmarks;
    config = Criterion::default().sample_size(10);
    targets = 
        bench_swap,
        bench_get_portfolio, 
        bench_get_top_traders,
        bench_batch_operations,
        bench_sequential_swaps,
        bench_add_liquidity,
        bench_performance_targets,
        bench_place_order,
        bench_take_order,
        bench_get_orderbook_snapshot
);
criterion_main!(contract_benchmarks);