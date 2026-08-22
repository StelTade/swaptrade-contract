use soroban_sdk::{
    testutils::Address as _,
    token, Address, Env,
};
use trade_engine::{
    OrderSide, OrderType, TradeEngineContract, TradeEngineContractClient, TradeLeg,
    PRICE_PRECISION,
};

fn setup_test() -> (Env, TradeEngineContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TradeEngineContract, ());
    let client = TradeEngineContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let trader = Address::generate(&env);

    client.initialize(&admin);

    (env, client, admin, trader)
}

fn create_token_and_mint(env: &Env, admin: &Address, to: &Address, amount: i128) -> Address {
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = token::StellarAssetClient::new(env, &token_contract.address());
    token_admin.mint(to, &amount);
    token_contract.address()
}

#[test]
fn test_order_placement_and_cancellation_with_token_escrow() {
    let (env, client, admin, trader) = setup_test();

    let base = create_token_and_mint(&env, &admin, &trader, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &trader, 1_000_000);

    let price = 10 * PRICE_PRECISION; // $10
    let amount = 100;

    let quote_token_client = token::Client::new(&env, &quote);

    let trader_quote_before = quote_token_client.balance(&trader);

    // Place buy order -> quote tokens escrowed
    let order_id = client.place_order(
        &trader,
        &base,
        &quote,
        &OrderSide::Buy,
        &OrderType::Limit,
        &price,
        &amount,
        &0,
    );

    assert_eq!(order_id, 1);
    let expected_escrow_quote = (amount as u128 * price / PRICE_PRECISION) as i128;
    assert_eq!(
        quote_token_client.balance(&trader),
        trader_quote_before - expected_escrow_quote
    );

    // Cancel order -> quote tokens refunded
    client.cancel_order(&trader, &order_id);
    assert_eq!(quote_token_client.balance(&trader), trader_quote_before);
}

#[test]
fn test_multi_asset_atomic_trade_3_pairs() {
    let (env, client, admin, trader) = setup_test();
    let maker1 = Address::generate(&env);
    let maker2 = Address::generate(&env);
    let maker3 = Address::generate(&env);

    let xlm = create_token_and_mint(&env, &admin, &maker1, 1_000_000);
    let usdc = create_token_and_mint(&env, &admin, &trader, 10_000_000);
    let btc = create_token_and_mint(&env, &admin, &maker2, 100);
    let eth = create_token_and_mint(&env, &admin, &trader, 10);

    // Give maker3 USDC to place buy order on ETH
    let usdc_admin = token::StellarAssetClient::new(&env, &usdc);
    usdc_admin.mint(&maker3, &100_000);

    let p1 = 5_000_000u128; // $0.50
    client.place_order(&maker1, &xlm, &usdc, &OrderSide::Sell, &OrderType::Limit, &p1, &1_000, &0);

    let p2 = 60_000 * PRICE_PRECISION; // $60,000
    client.place_order(&maker2, &btc, &usdc, &OrderSide::Sell, &OrderType::Limit, &p2, &10, &0);

    let p3 = 3_000 * PRICE_PRECISION; // $3,000
    client.place_order(&maker3, &eth, &usdc, &OrderSide::Buy, &OrderType::Limit, &p3, &5, &0);

    let legs = soroban_sdk::vec![
        &env,
        TradeLeg {
            base_asset: xlm.clone(),
            quote_asset: usdc.clone(),
            side: OrderSide::Buy,
            amount: 500,
            limit_price: p1,
            min_output_amount: 500,
        },
        TradeLeg {
            base_asset: btc.clone(),
            quote_asset: usdc.clone(),
            side: OrderSide::Buy,
            amount: 2,
            limit_price: p2,
            min_output_amount: 2,
        },
        TradeLeg {
            base_asset: eth.clone(),
            quote_asset: usdc.clone(),
            side: OrderSide::Sell,
            amount: 1,
            limit_price: p3,
            min_output_amount: 3_000,
        },
    ];

    let result = client.execute_multi_pair_trade(&trader, &legs);
    assert!(result.success);
    assert_eq!(result.legs_executed, 3);
    assert_eq!(result.fills.len(), 3);
}

#[test]
fn test_fallback_liquidity_pool_integration() {
    let (env, client, admin, trader) = setup_test();

    let base = create_token_and_mint(&env, &admin, &admin, 1_000_000);
    let quote = create_token_and_mint(&env, &admin, &admin, 1_000_000);

    // Mint trader base asset to sell
    let base_mint = token::StellarAssetClient::new(&env, &base);
    base_mint.mint(&trader, &10_000);

    let pool_id = client.add_liquidity_pool(&admin, &base, &quote, &1_000_000, &1_000_000, &30);
    assert_eq!(pool_id, 1);

    let legs = soroban_sdk::vec![
        &env,
        TradeLeg {
            base_asset: base.clone(),
            quote_asset: quote.clone(),
            side: OrderSide::Sell,
            amount: 1_000,
            limit_price: 0,
            min_output_amount: 900,
        },
    ];

    let result = client.execute_multi_pair_trade(&trader, &legs);
    assert!(result.success);
    assert_eq!(result.fills.get(0).unwrap().filled_via_pool, true);
}
