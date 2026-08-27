use soroban_sdk::{testutils::Address as _, token, Address, Env, Vec};
use std::time::Instant;
use trade_engine::{
    OrderSide, OrderType, TradeEngineContract, TradeEngineContractClient, TradeLeg, PRICE_PRECISION,
};

#[test]
fn test_10_pair_trade_performance_benchmark() {
    let env = Env::default();
    env.mock_all_auths();
    env.budget().reset_unlimited();

    let contract_id = env.register(TradeEngineContract, ());
    let client = TradeEngineContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let trader = Address::generate(&env);
    client.initialize(&admin);

    // Create 11 tokens = 10 asset pairs
    let mut tokens = std::vec::Vec::new();
    for _ in 0..11 {
        let t = env.register_stellar_asset_contract_v2(admin.clone());
        tokens.push(t.address());
    }

    let mut legs = Vec::new(&env);

    // Populate order books for 10 asset pairs with token minting and escrow
    for i in 0..10 {
        let maker = Address::generate(&env);
        let base = tokens[i].clone();
        let quote = tokens[i + 1].clone();
        let price = (10 + i as u128) * PRICE_PRECISION;
        let amount = 1_000i128;

        // Mint base asset to maker for sell order escrow
        let base_admin = token::StellarAssetClient::new(&env, &base);
        base_admin.mint(&maker, &amount);

        // Mint quote asset to trader for buy leg settlement
        let quote_admin = token::StellarAssetClient::new(&env, &quote);
        let quote_required = (100u128 * price / PRICE_PRECISION) as i128;
        quote_admin.mint(&trader, &quote_required);

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

    // Benchmark 10-pair atomic trade execution
    let start_time = Instant::now();
    let result = client.execute_multi_pair_trade(&trader, &legs);
    let duration = start_time.elapsed();

    assert!(result.success);
    assert_eq!(result.legs_executed, 10);
    assert_eq!(result.fills.len(), 10);

    // Performance target: execution time must be < 1000ms
    println!("Execution time for 10-pair trade: {:?}", duration);
    assert!(
        duration.as_millis() < 1000,
        "Execution time exceeds performance benchmark threshold: {:?}",
        duration
    );
}
