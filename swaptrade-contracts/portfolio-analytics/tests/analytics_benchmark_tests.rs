use std::time::Instant;
use soroban_sdk::{
    testutils::Address as _, token, Address, Env,
};
use portfolio_analytics::{
    PortfolioAnalyticsContract, PortfolioAnalyticsContractClient,
};

const PRICE_PRECISION: i128 = 10_000_000;

fn create_token_and_mint(env: &Env, admin: &Address, to: &Address, amount: i128) -> Address {
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = token::StellarAssetClient::new(env, &token_contract.address());
    token_admin.mint(to, &amount);
    token_contract.address()
}

/// Benchmark: snapshot of 20 positions (within Soroban's ledger entry limit)
/// In production, larger portfolios use paginated queries across multiple calls.
#[test]
fn test_20_positions_snapshot_benchmark() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PortfolioAnalyticsContract, ());
    let client = PortfolioAnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);

    let quote = create_token_and_mint(&env, &admin, &user, 100_000_000);

    // Create 20 positions with prices and market data
    for i in 0..20i128 {
        let asset = create_token_and_mint(&env, &admin, &user, 10_000);
        let price: i128 = (10 + i % 10) * PRICE_PRECISION;
        client.record_buy(&user, &asset, &quote, &100, &price);
        let market_price: u128 = ((12 + i % 10) * PRICE_PRECISION) as u128;
        client.update_price(&asset, &market_price);
    }

    let start = Instant::now();
    let snapshot = client.capture_snapshot(&user);
    let duration = start.elapsed();

    assert_eq!(snapshot.positions.len(), 20);
    assert!(
        duration.as_millis() < 2000,
        "Snapshot of 20 positions exceeded 2s: {:?}",
        duration
    );
    println!("20-position snapshot time: {:?}", duration);
}

/// Benchmark: metrics computation for 20 buy/sell pairs
#[test]
fn test_20_pair_metrics_benchmark() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PortfolioAnalyticsContract, ());
    let client = PortfolioAnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);

    let quote = create_token_and_mint(&env, &admin, &user, 100_000_000);

    // Create 20 buy/sell pairs (closed trades)
    for i in 0..20i128 {
        let asset = create_token_and_mint(&env, &admin, &user, 10_000);
        let buy_price: i128 = (10 + i) * PRICE_PRECISION;
        let sell_price: i128 = (12 + i) * PRICE_PRECISION;

        client.record_buy(&user, &asset, &quote, &100, &buy_price);
        client.record_sell(&user, &asset, &50, &sell_price);

        let market_price: u128 = ((15 + i) * PRICE_PRECISION) as u128;
        client.update_price(&asset, &market_price);
    }

    let start = Instant::now();
    let metrics = client.compute_metrics(&user);
    let duration = start.elapsed();

    assert!(metrics.total_trades > 0);
    assert!(
        duration.as_millis() < 2000,
        "Metrics computation exceeded 2s: {:?}",
        duration
    );
    println!("Metrics computation time: {:?}", duration);
}

/// Benchmark: paginated retrieval of 200 transaction records
#[test]
fn test_transaction_history_pagination_benchmark() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PortfolioAnalyticsContract, ());
    let client = PortfolioAnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);

    let asset = create_token_and_mint(&env, &admin, &user, 10_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 10_000_000);

    // Create 200 transactions
    for i in 0..200i128 {
        let price: i128 = (10 + i % 50) * PRICE_PRECISION;
        if i % 2 == 0 {
            client.record_buy(&user, &asset, &quote, &10, &price);
        } else {
            client.record_sell(&user, &asset, &10, &price);
        }
    }

    // Paginated retrieval should be fast
    let start = Instant::now();
    let page = client.get_transaction_history(&user, &0, &50);
    let duration = start.elapsed();

    assert_eq!(page.len(), 50);
    assert!(
        duration.as_millis() < 500,
        "Paginated retrieval exceeded 500ms: {:?}",
        duration
    );
    println!("Paginated retrieval time: {:?}", duration);
}
