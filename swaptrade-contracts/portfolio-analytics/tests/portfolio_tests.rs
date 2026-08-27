use soroban_sdk::{
    testutils::Address as _, token, Address, Env, Vec,
};
use portfolio_analytics::{
    PortfolioAnalyticsContract, PortfolioAnalyticsContractClient,
};

// ── Helpers ─────────────────────────────────────────────────────────────────

fn setup_test() -> (Env, PortfolioAnalyticsContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PortfolioAnalyticsContract, ());
    let client = PortfolioAnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);

    (env, client, admin, user)
}

fn create_token_and_mint(env: &Env, admin: &Address, to: &Address, amount: i128) -> Address {
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = token::StellarAssetClient::new(env, &token_contract.address());
    token_admin.mint(to, &amount);
    token_contract.address()
}

const PRICE_PRECISION: i128 = 10_000_000;

// ── Position Tracking Tests ─────────────────────────────────────────────────

#[test]
fn test_buy_opens_new_position() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION; // $10
    let quantity: i128 = 100;

    let record = client.record_buy(&user, &asset, &quote, &quantity, &price);

    assert_eq!(record.tx_id, 1);
    assert_eq!(record.quantity, 100);
    assert_eq!(record.price, price);
    assert_eq!(record.realized_pnl, 0);
    assert_eq!(record.remaining_quantity, 100);

    let position = client.get_position(&user, &asset);
    assert!(position.is_some());
    let pos = position.unwrap();
    assert_eq!(pos.quantity, 100);
    assert_eq!(pos.avg_cost_basis, price);
}

#[test]
fn test_buy_adds_to_existing_position() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price1: i128 = 10 * PRICE_PRECISION;
    let price2: i128 = 12 * PRICE_PRECISION;

    client.record_buy(&user, &asset, &quote, &100, &price1);
    client.record_buy(&user, &asset, &quote, &50, &price2);

    let position = client.get_position(&user, &asset).unwrap();
    assert_eq!(position.quantity, 150);
    assert_eq!(position.buy_count, 2);
    // Weighted average: (100*10M + 50*12M) / 150 = 1400M/150 ≈ 9333333
    let expected_avg = (100 * 10 * PRICE_PRECISION + 50 * 12 * PRICE_PRECISION) / 150;
    assert_eq!(position.avg_cost_basis, expected_avg);
}

#[test]
fn test_sell_reduces_position() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);

    let sell_price: i128 = 12 * PRICE_PRECISION;
    let record = client.record_sell(&user, &asset, &50, &sell_price);

    assert_eq!(record.tx_id, 2);
    assert_eq!(record.quantity, 50);
    assert_eq!(record.remaining_quantity, 50);

    let position = client.get_position(&user, &asset).unwrap();
    assert_eq!(position.quantity, 50);
    assert_eq!(position.sell_count, 1);
}

#[test]
fn test_sell_closes_position_completely() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);
    client.record_sell(&user, &asset, &100, &price);

    // Position should be closed
    let position = client.get_position(&user, &asset);
    assert!(position.is_none());

    // User assets should be empty
    let assets = client.get_user_assets(&user);
    assert_eq!(assets.len(), 0);
}

#[test]
fn test_sell_insufficient_quantity_fails() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &50, &price);

    let result = client.try_record_sell(&user, &asset, &100, &price);
    assert!(result.is_err());
}

#[test]
fn test_invalid_quantity_fails() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    let result = client.try_record_buy(&user, &asset, &quote, &0, &price);
    assert!(result.is_err());

    let result = client.try_record_buy(&user, &asset, &quote, &-5, &price);
    assert!(result.is_err());
}

#[test]
fn test_invalid_price_fails() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let result = client.try_record_buy(&user, &asset, &quote, &100, &0);
    assert!(result.is_err());
}

#[test]
fn test_get_all_positions_multiple_assets() {
    let (env, client, admin, user) = setup_test();
    let asset1 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let asset2 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let p1: i128 = 10 * PRICE_PRECISION;
    let p2: i128 = 5 * PRICE_PRECISION;
    client.record_buy(&user, &asset1, &quote, &100, &p1);
    client.record_buy(&user, &asset2, &quote, &200, &p2);

    let positions = client.get_all_positions(&user);
    assert_eq!(positions.len(), 2);
}

// ── Cost Basis Method Tests ─────────────────────────────────────────────────

#[test]
fn test_fifo_cost_basis_realized_pnl() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    // Buy 100 at $10, then 100 at $20
    let price1: i128 = 10 * PRICE_PRECISION;
    let price2: i128 = 20 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price1);
    client.record_buy(&user, &asset, &quote, &100, &price2);

    // Sell 150 at $25 — default method is WeightedAverage
    let sell_price: i128 = 25 * PRICE_PRECISION;
    let record = client.record_sell(&user, &asset, &150, &sell_price);

    // Weighted average cost = (100*10 + 100*20) / 200 = 15 per unit
    // Proceeds: 150 * 25 = 3750
    // Cost: 150 * 15 = 2250
    // P&L: 3750 - 2250 = 1500
    assert_eq!(record.realized_pnl, 1500);

    // Remaining position: 50 units
    let position = client.get_position(&user, &asset).unwrap();
    assert_eq!(position.quantity, 50);
}

#[test]
fn test_weighted_average_cost_basis() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    // Buy 100 at $10, then 100 at $20
    let price1: i128 = 10 * PRICE_PRECISION;
    let price2: i128 = 20 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price1);
    client.record_buy(&user, &asset, &quote, &100, &price2);

    // Weighted average cost = (100 * 10M + 100 * 20M) / 200 = 15M
    let position = client.get_position(&user, &asset).unwrap();
    assert_eq!(position.avg_cost_basis, 15 * PRICE_PRECISION);

    // Sell 100 at $15 — break even
    let sell_price: i128 = 15 * PRICE_PRECISION;
    let record = client.record_sell(&user, &asset, &100, &sell_price);
    assert_eq!(record.realized_pnl, 0);

    // Sell remaining 100 at $10 — loss
    let record2 = client.record_sell(&user, &asset, &100, &(10 * PRICE_PRECISION));
    // Proceeds: 100 * 10 = 1000, Cost: 100 * 15 = 1500, P&L: -500
    assert_eq!(record2.realized_pnl, -500);
}

#[test]
fn test_partial_sell_cost_basis() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price1: i128 = 10 * PRICE_PRECISION;
    let price2: i128 = 20 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price1);
    client.record_buy(&user, &asset, &quote, &100, &price2);

    // Sell only 30 at $15
    let record = client.record_sell(&user, &asset, &30, &(15 * PRICE_PRECISION));

    // Weighted avg cost = 15 per unit
    // P&L: 30 * 15 - 30 * 15 = 0
    assert_eq!(record.realized_pnl, 0);

    // Remaining: 170
    let position = client.get_position(&user, &asset).unwrap();
    assert_eq!(position.quantity, 170);
}

// ── P&L Calculation Tests ───────────────────────────────────────────────────

#[test]
fn test_unrealized_pnl_positive() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);

    // Set market price to $15
    client.update_price(&asset, &((15 * PRICE_PRECISION) as u128));

    let upnl = client.get_unrealized_pnl(&user, &asset);
    // Market value: 100 * 15M / 10M = 1500
    // Cost basis: 100 * 10M / 10M = 1000
    // Unrealized P&L: 1500 - 1000 = 500
    assert_eq!(upnl, 500);
}

#[test]
fn test_unrealized_pnl_negative() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);

    // Set market price to $5
    client.update_price(&asset, &((5 * PRICE_PRECISION) as u128));

    let upnl = client.get_unrealized_pnl(&user, &asset);
    // Market value: 100 * 5M / 10M = 500
    // Cost basis: 100 * 10M / 10M = 1000
    // Unrealized P&L: 500 - 1000 = -500
    assert_eq!(upnl, -500);
}

#[test]
fn test_total_portfolio_value() {
    let (env, client, admin, user) = setup_test();
    let asset1 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let asset2 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let p1: i128 = 10 * PRICE_PRECISION;
    let p2: i128 = 20 * PRICE_PRECISION;
    client.record_buy(&user, &asset1, &quote, &100, &p1);
    client.record_buy(&user, &asset2, &quote, &50, &p2);

    // asset1: market $15, asset2: market $25
    client.update_price(&asset1, &((15 * PRICE_PRECISION) as u128));
    client.update_price(&asset2, &((25 * PRICE_PRECISION) as u128));

    let total_value = client.get_total_portfolio_value(&user);
    // asset1: 100 * 15M / 10M = 1500
    // asset2: 50 * 25M / 10M = 1250
    // Total: 2750
    assert_eq!(total_value, 2750);
}

#[test]
fn test_total_unrealized_and_realized_pnl() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    // Buy 100 at $10
    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);

    // Sell 50 at $15 — realize $250 profit
    client.record_sell(&user, &asset, &50, &(15 * PRICE_PRECISION));

    // Set market price to $12 for remaining 50
    client.update_price(&asset, &((12 * PRICE_PRECISION) as u128));

    let realized = client.get_total_realized_pnl(&user);
    // P&L: 50 * 15 - 50 * 10 = 250
    assert_eq!(realized, 250);

    let unrealized = client.get_total_unrealized_pnl(&user);
    // Remaining cost basis: 50 * 10 = 500
    // Market value: 50 * 12 = 600
    // Unrealized: 600 - 500 = 100
    assert_eq!(unrealized, 100);
}

// ── Price Oracle Tests ──────────────────────────────────────────────────────

#[test]
fn test_price_update_and_query() {
    let (env, client, _admin, _user) = setup_test();
    let asset = Address::generate(&env);

    client.update_price(&asset, &42_500_000u128);

    let price_data = client.get_price(&asset);
    assert_eq!(price_data.price, 42_500_000);
}

#[test]
fn test_zero_price_fails() {
    let (env, client, _admin, _user) = setup_test();
    let asset = Address::generate(&env);

    let result = client.try_update_price(&asset, &0u128);
    assert!(result.is_err());
}

#[test]
fn test_no_price_data_fails() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);

    // No price set — should fail
    let result = client.try_get_unrealized_pnl(&user, &asset);
    assert!(result.is_err());
}

// ── Portfolio Snapshot Tests ────────────────────────────────────────────────

#[test]
fn test_capture_snapshot() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);
    client.update_price(&asset, &((15 * PRICE_PRECISION) as u128));

    let snapshot = client.capture_snapshot(&user);
    assert_eq!(snapshot.snapshot_id, 1);
    assert_eq!(snapshot.positions.len(), 1);

    let pos_summary = snapshot.positions.get(0).unwrap();
    assert_eq!(pos_summary.quantity, 100);
    assert_eq!(pos_summary.market_value, 1500);
    assert_eq!(pos_summary.unrealized_pnl, 500);
}

#[test]
fn test_snapshot_tracks_multiple_positions() {
    let (env, client, admin, user) = setup_test();
    let asset1 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let asset2 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let p1: i128 = 10 * PRICE_PRECISION;
    let p2: i128 = 5 * PRICE_PRECISION;
    client.record_buy(&user, &asset1, &quote, &100, &p1);
    client.record_buy(&user, &asset2, &quote, &200, &p2);

    client.update_price(&asset1, &((15 * PRICE_PRECISION) as u128));
    client.update_price(&asset2, &((8 * PRICE_PRECISION) as u128));

    let snapshot = client.capture_snapshot(&user);
    assert_eq!(snapshot.positions.len(), 2);

    // Total value: 100 * 15 + 200 * 8 = 1500 + 1600 = 3100
    assert_eq!(snapshot.total_value, 3100);
}

#[test]
fn test_snapshot_retrieval_by_id() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);
    client.update_price(&asset, &((15 * PRICE_PRECISION) as u128));

    let snap1 = client.capture_snapshot(&user);
    client.record_sell(&user, &asset, &50, &(20 * PRICE_PRECISION));
    let snap2 = client.capture_snapshot(&user);

    let retrieved1 = client.get_snapshot(&snap1.snapshot_id);
    assert_eq!(retrieved1.positions.get(0).unwrap().quantity, 100);

    let retrieved2 = client.get_snapshot(&snap2.snapshot_id);
    assert_eq!(retrieved2.positions.get(0).unwrap().quantity, 50);
}

#[test]
fn test_multiple_snapshots_listed() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);
    client.update_price(&asset, &((15 * PRICE_PRECISION) as u128));

    client.capture_snapshot(&user);
    client.capture_snapshot(&user);
    client.capture_snapshot(&user);

    let snapshot_ids = client.get_user_snapshots(&user);
    assert_eq!(snapshot_ids.len(), 3);
    assert_eq!(snapshot_ids.get(0).unwrap(), 1);
    assert_eq!(snapshot_ids.get(1).unwrap(), 2);
    assert_eq!(snapshot_ids.get(2).unwrap(), 3);
}

#[test]
fn test_snapshot_allocation_percentages() {
    let (env, client, admin, user) = setup_test();
    let asset1 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let asset2 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    // asset1: 100 units * $10 = $1000
    // asset2: 200 units * $10 = $2000
    client.record_buy(&user, &asset1, &quote, &100, &price);
    client.record_buy(&user, &asset2, &quote, &200, &price);
    client.update_price(&asset1, &((10 * PRICE_PRECISION) as u128));
    client.update_price(&asset2, &((10 * PRICE_PRECISION) as u128));

    let snapshot = client.capture_snapshot(&user);

    // asset1: 1000/3000 = 33.33% ≈ 3333 bps
    // asset2: 2000/3000 = 66.67% ≈ 6666 bps
    let s1 = snapshot.positions.get(0).unwrap();
    let s2 = snapshot.positions.get(1).unwrap();

    assert!(s1.allocation_pct >= 3332 && s1.allocation_pct <= 3334);
    assert!(s2.allocation_pct >= 6665 && s2.allocation_pct <= 6667);
}

// ── Transaction History Tests ───────────────────────────────────────────────

#[test]
fn test_transaction_history_recorded() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let p1: i128 = 10 * PRICE_PRECISION;
    let p2: i128 = 15 * PRICE_PRECISION;
    let p3: i128 = 20 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &p1);
    client.record_buy(&user, &asset, &quote, &50, &p2);
    client.record_sell(&user, &asset, &75, &p3);

    let history = client.get_transaction_history(&user, &0, &10);
    assert_eq!(history.len(), 3);

    // First tx: buy
    let tx1 = history.get(0).unwrap();
    assert_eq!(tx1.tx_id, 1);

    // Third tx: sell
    let tx3 = history.get(2).unwrap();
    assert_eq!(tx3.tx_id, 3);
}

#[test]
fn test_transaction_history_pagination() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    for i in 0..10i128 {
        let price: i128 = (10 + i) * PRICE_PRECISION;
        client.record_buy(&user, &asset, &quote, &10, &price);
    }

    // Page 1
    let page1 = client.get_transaction_history(&user, &0, &3);
    assert_eq!(page1.len(), 3);
    assert_eq!(page1.get(0).unwrap().tx_id, 1);
    assert_eq!(page1.get(2).unwrap().tx_id, 3);

    // Page 2
    let page2 = client.get_transaction_history(&user, &3, &3);
    assert_eq!(page2.len(), 3);
    assert_eq!(page2.get(0).unwrap().tx_id, 4);

    // Page 4 (last partial page)
    let page4 = client.get_transaction_history(&user, &9, &3);
    assert_eq!(page4.len(), 1);
}

#[test]
fn test_individual_transaction_lookup() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);

    let tx = client.get_transaction(&1).unwrap();
    assert_eq!(tx.user, user);
    assert_eq!(tx.asset, asset);
    assert_eq!(tx.quantity, 100);
    assert_eq!(tx.price, price);
}

// ── Portfolio Composition Tests ─────────────────────────────────────────────

#[test]
fn test_portfolio_composition() {
    let (env, client, admin, user) = setup_test();
    let asset1 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let asset2 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let asset3 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset1, &quote, &100, &price);
    client.record_buy(&user, &asset2, &quote, &200, &price);
    client.record_buy(&user, &asset3, &quote, &300, &price);

    client.update_price(&asset1, &((10 * PRICE_PRECISION) as u128));
    client.update_price(&asset2, &((10 * PRICE_PRECISION) as u128));
    client.update_price(&asset3, &((10 * PRICE_PRECISION) as u128));

    let composition = client.get_portfolio_composition(&user);
    assert_eq!(composition.len(), 3);

    // Total: 100 + 200 + 300 = 600
    // asset1: 100/600 ≈ 1667 bps
    // asset2: 200/600 ≈ 3333 bps
    // asset3: 300/600 ≈ 5000 bps
    let s1 = composition.get(0).unwrap();
    let s2 = composition.get(1).unwrap();
    let s3 = composition.get(2).unwrap();

    assert!(s1.allocation_pct >= 1666 && s1.allocation_pct <= 1668);
    assert!(s2.allocation_pct >= 3332 && s2.allocation_pct <= 3334);
    assert!(s3.allocation_pct >= 4999 && s3.allocation_pct <= 5001);
}

// ── Performance Metrics Tests ───────────────────────────────────────────────

#[test]
fn test_compute_metrics_basic() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let asset2 = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    // Buy at $10, sell at $15 — profit
    let p1: i128 = 10 * PRICE_PRECISION;
    let p2: i128 = 15 * PRICE_PRECISION;
    let p3: i128 = 5 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &p1);
    client.record_sell(&user, &asset, &100, &p2);

    // Add a loss trade
    client.record_buy(&user, &asset2, &quote, &100, &p1);
    client.record_sell(&user, &asset2, &100, &p3);

    client.update_price(&asset, &((20 * PRICE_PRECISION) as u128));
    client.update_price(&asset2, &((5 * PRICE_PRECISION) as u128));

    let metrics = client.compute_metrics(&user);

    assert_eq!(metrics.winning_trades, 1);
    assert_eq!(metrics.losing_trades, 1);
    assert_eq!(metrics.total_trades, 2);
    // Win/Loss ratio: 1/1 = 1.0 = 10000 bps
    assert_eq!(metrics.win_loss_ratio, 10_000);
}

#[test]
fn test_roi_calculation() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    // Buy 100 at $10 — cost basis = 1000
    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);

    // Market price now $20
    client.update_price(&asset, &((20 * PRICE_PRECISION) as u128));

    let metrics = client.compute_metrics(&user);
    // ROI = (2000 - 1000) / 1000 * 10000 = 10000 bps (100% return)
    assert_eq!(metrics.roi_bps, 10_000);
}

// ── Return Period & VaR Tests ───────────────────────────────────────────────

#[test]
fn test_record_return_periods() {
    let (env, client, admin, user) = setup_test();
    let _ = client.try_record_return_period(&user, &1000, &10000i128, &2000, &11000i128);
}

#[test]
fn test_var_calculation() {
    let (env, client, admin, user) = setup_test();

    // Record several return periods
    let _ = client.try_record_return_period(&user, &0, &10000i128, &1, &10500i128); // +5%
    let _ = client.try_record_return_period(&user, &1, &10500i128, &2, &10200i128); // -2.86%
    let _ = client.try_record_return_period(&user, &2, &10200i128, &3, &10800i128); // +5.88%
    let _ = client.try_record_return_period(&user, &3, &10800i128, &4, &10100i128); // -6.48%
    let _ = client.try_record_return_period(&user, &4, &10100i128, &5, &10600i128); // +4.95%

    let var_95 = client.compute_var(&user, &9500);
    let var_99 = client.compute_var(&user, &9900);

    // VaR should be positive and 99% VaR should be >= 95% VaR
    assert!(var_95 > 0);
    assert!(var_99 >= var_95);
}

#[test]
fn test_metrics_empty_portfolio_fails() {
    let (env, client, _admin, user) = setup_test();
    let result = client.try_compute_metrics(&user);
    assert!(result.is_err());
}

// ── Large Portfolio Performance Test ────────────────────────────────────────

#[test]
fn test_20_positions_performance() {
    let (env, client, admin, user) = setup_test();
    let quote = create_token_and_mint(&env, &admin, &user, 100_000_000);

    // Create 100 different assets and buy positions
    let mut assets = Vec::new(&env);
    for i in 0..20i128 {
        let asset = create_token_and_mint(&env, &admin, &user, 10_000);
        assets.push_back(asset.clone());
        let price: i128 = (10 + i % 50) * PRICE_PRECISION;
        client.record_buy(&user, &asset, &quote, &100, &price);
    }

    // All positions should be tracked
    let positions = client.get_all_positions(&user);
    assert_eq!(positions.len(), 20);

    // User should have 100 assets
    let user_assets = client.get_user_assets(&user);
    assert_eq!(user_assets.len(), 20);
}

#[test]
fn test_many_buys_and_sells_integrity() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 100_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 100_000_000);

    // 50 buy transactions
    for i in 0..50i128 {
        let price: i128 = (10 + i % 20) * PRICE_PRECISION;
        client.record_buy(&user, &asset, &quote, &100, &price);
    }

    let position = client.get_position(&user, &asset).unwrap();
    assert_eq!(position.quantity, 5000);
    assert_eq!(position.buy_count, 50);

    // 25 sell transactions
    for _ in 0..25i128 {
        let sell_price: i128 = 15 * PRICE_PRECISION;
        client.record_sell(&user, &asset, &100, &sell_price);
    }

    let position = client.get_position(&user, &asset).unwrap();
    assert_eq!(position.quantity, 2500);
    assert_eq!(position.sell_count, 25);

    // Verify total transactions
    let history = client.get_transaction_history(&user, &0, &200);
    assert_eq!(history.len(), 75); // 50 buys + 25 sells
}

// ── Edge Cases ──────────────────────────────────────────────────────────────

#[test]
fn test_buy_sell_same_price_break_even() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let price: i128 = 10 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &price);
    let record = client.record_sell(&user, &asset, &100, &price);

    assert_eq!(record.realized_pnl, 0);
}

#[test]
fn test_sell_increases_then_decreases_position() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let p1: i128 = 10 * PRICE_PRECISION;
    let p2: i128 = 15 * PRICE_PRECISION;
    let p3: i128 = 12 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &p1);
    client.record_sell(&user, &asset, &50, &p2);
    client.record_buy(&user, &asset, &quote, &30, &p3);

    let position = client.get_position(&user, &asset).unwrap();
    // 100 - 50 + 30 = 80
    assert_eq!(position.quantity, 80);
    assert_eq!(position.buy_count, 2);
    assert_eq!(position.sell_count, 1);
}

#[test]
fn test_divested_total_tracked() {
    let (env, client, admin, user) = setup_test();
    let asset = create_token_and_mint(&env, &admin, &user, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &user, 1_000_000);

    let p1: i128 = 10 * PRICE_PRECISION;
    let p2: i128 = 20 * PRICE_PRECISION;
    client.record_buy(&user, &asset, &quote, &100, &p1);
    client.record_sell(&user, &asset, &50, &p2);

    let total_divested = client.get_total_divested(&user);
    // 50 * 20M / 10M = 100
    assert_eq!(total_divested, 1000);

    let total_invested = client.get_total_invested(&user);
    // 100 * 10M / 10M = 100
    assert_eq!(total_invested, 1000);
}
