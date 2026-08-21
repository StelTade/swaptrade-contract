#![cfg(test)]

use super::*;
use crate::errors::ContractError;
use soroban_sdk::{symbol_short, Env, Address};
use soroban_sdk::testutils::Address as _;
use crate::orders::{OrderManager, OrderType, OrderStatus};

const PRECISION: u128 = 1_000_000_000_000_000_000;

#[test]
fn test_place_limit_order() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place limit order
    let order_id = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        1000,
        PRECISION, // 1:1 price
        None,      // No expiry
    )
    .unwrap();

    assert_eq!(order_id, 1);

    // Verify order was created
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.order_id, 1);
    assert_eq!(order.owner, user);
    assert_eq!(order.order_type, OrderType::Limit);
    assert_eq!(order.token_in, xlm);
    assert_eq!(order.token_out, usdc);
    assert_eq!(order.amount_in, 1000);
    assert_eq!(order.status, OrderStatus::Pending);
    assert_eq!(order.limit_price, Some(PRECISION));
}

#[test]
fn test_place_stop_loss() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place stop-loss order
    let trigger_price = (PRECISION as u128).saturating_mul(9_500) / 10_000; // 5% below
    let order_id = OrderManager::place_stop_loss(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        500,
        trigger_price,
        None,
    )
    .unwrap();

    assert_eq!(order_id, 1);

    // Verify order
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.order_type, OrderType::StopLoss);
    assert_eq!(order.trigger_price, Some(trigger_price));
}

#[test]
fn test_cancel_order() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place order
    let order_id =
        OrderManager::place_limit_order(&env, user.clone(), xlm, usdc, 1000, PRECISION, None)
            .unwrap();

    // Cancel order
    OrderManager::cancel_order(&env, order_id, user.clone()).unwrap();

    // Verify status
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.status, OrderStatus::Cancelled);
}

#[test]
fn test_cancel_order_wrong_owner() {
    let env = Env::default();
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place order with user1
    let order_id =
        OrderManager::place_limit_order(&env, user1.clone(), xlm, usdc, 1000, PRECISION, None)
            .unwrap();

    // Try to cancel with user2 (should fail)
    let result = OrderManager::cancel_order(&env, order_id, user2);
    assert!(result.is_err());
}

#[test]
fn test_get_user_orders() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place multiple orders
    OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        1000,
        PRECISION,
        None,
    )
    .unwrap();
    OrderManager::place_stop_loss(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        500,
        PRECISION,
        None,
    )
    .unwrap();

    // Get user orders
    let orders = OrderManager::get_user_orders(&env, user);
    assert_eq!(orders.len(), 2);
}

#[test]
fn test_order_with_expiry() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place order with expiry (1 hour from now)
    let expiry = env.ledger().timestamp() + 3600;
    let order_id = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm,
        usdc,
        1000,
        PRECISION,
        Some(expiry),
    )
    .unwrap();

    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.expires_at, Some(expiry));
}

#[test]
fn test_invalid_order_amount() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Try to place order with zero amount
    let result = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        0,
        PRECISION,
        None,
    );
    assert!(result.is_err());

    // Try to place order with negative amount
    let result =
        OrderManager::place_stop_loss(&env, user.clone(), xlm, usdc, -100, PRECISION, None);
    assert!(result.is_err());
}

#[test]
fn test_invalid_order_price() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Try to place limit order with zero price
    let result = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        1000,
        0,
        None,
    );
    assert!(result.is_err());

    // Try to place stop-loss with zero trigger
    let result = OrderManager::place_stop_loss(&env, user, xlm, usdc, 500, 0, None);
    assert!(result.is_err());
}

#[test]
fn test_order_id_increment() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place multiple orders
    let id1 = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        100,
        PRECISION,
        None,
    )
    .unwrap();
    let id2 = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        200,
        PRECISION,
        None,
    )
    .unwrap();
    let id3 = OrderManager::place_stop_loss(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        300,
        PRECISION,
        None,
    )
    .unwrap();

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn test_match_pending_orders() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place limit order at PRECISION
    let order_id = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        1000,
        PRECISION,
        None,
    )
    .unwrap();

    // Match orders with current price at or below limit
    let current_price = (PRECISION as u128).saturating_mul(9_900) / 10_000; // 1% below limit
    let executed =
        OrderManager::match_pending_orders(&env, xlm.clone(), usdc.clone(), current_price).unwrap();

    // Order should be executed
    assert!(executed.len() > 0);

    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.status, OrderStatus::Filled);
}

#[test]
fn test_new_place_limit_order() {
    let env = Env::default();
    let maker = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place limit sell order (sell XLM for USDC at 1.0 USDC/XLM)
    let order_id = OrderManager::place_limit_order(
        &env,
        maker.clone(),
        xlm.clone(),
        usdc.clone(),
        OrderSide::Sell,
        1000, // 1000 XLM
        PRECISION, // 1 USDC per XLM
        None,
    ).unwrap();

    assert_eq!(order_id, 1);

    // Verify order was created correctly
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.order_id, 1);
    assert_eq!(order.owner, maker);
    assert_eq!(order.side, OrderSide::Sell);
    assert_eq!(order.base_token, xlm);
    assert_eq!(order.quote_token, usdc);
    assert_eq!(order.amount, 1000);
    assert_eq!(order.amount_remaining, 1000);
    assert_eq!(order.price, PRECISION);
    assert_eq!(order.status, OrderStatus::Pending);
}

#[test]
fn test_take_order_full_fill() {
    let env = Env::default();
    let maker = Address::generate(&env);
    let taker = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Maker places a limit sell order: sell 1000 XLM at 1.0 USDC/XLM
    let order_id = OrderManager::place_limit_order(
        &env,
        maker.clone(),
        xlm.clone(),
        usdc.clone(),
        OrderSide::Sell,
        1000,
        PRECISION,
        None,
    ).unwrap();

    // Taker places a buy order for 1000 XLM, accepts any price
    let fills = OrderManager::take_order(
        &env,
        taker.clone(),
        xlm.clone(),
        usdc.clone(),
        OrderSide::Buy,
        1000,
        None,
    ).unwrap();

    // Verify single full fill
    assert_eq!(fills.len(), 1);
    let fill = fills.get(0).unwrap();
    assert_eq!(fill.order_id, order_id);
    assert_eq!(fill.filled_amount_base, 1000);
    assert_eq!(fill.filled_amount_quote, 1000); // 1000 XLM * 1 USDC/XLM = 1000 USDC
    assert_eq!(fill.is_complete_fill, true);
    assert_eq!(fill.maker, maker);
    assert_eq!(fill.taker, taker);

    // Verify order is now filled
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.status, OrderStatus::Filled);
    assert_eq!(order.amount_remaining, 0);
    assert_eq!(order.amount_filled, 1000);
}

#[test]
fn test_take_order_partial_fill() {
    let env = Env::default();
    let maker = Address::generate(&env);
    let taker = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Maker places a limit sell order: sell 1000 XLM at 1.0 USDC/XLM
    let order_id = OrderManager::place_limit_order(
        &env,
        maker.clone(),
        xlm.clone(),
        usdc.clone(),
        OrderSide::Sell,
        1000,
        PRECISION,
        None,
    ).unwrap();

    // Taker only buys 400 XLM (partial fill)
    let fills = OrderManager::take_order(
        &env,
        taker.clone(),
        xlm.clone(),
        usdc.clone(),
        OrderSide::Buy,
        400,
        None,
    ).unwrap();

    // Verify partial fill
    assert_eq!(fills.len(), 1);
    let fill = fills.get(0).unwrap();
    assert_eq!(fill.filled_amount_base, 400);
    assert_eq!(fill.is_complete_fill, false);

    // Verify order is partially filled
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.status, OrderStatus::PartiallyFilled);
    assert_eq!(order.amount_remaining, 600);
    assert_eq!(order.amount_filled, 400);
}

#[test]
fn test_time_price_priority() {
    let env = Env::default();
    let maker1 = Address::generate(&env);
    let maker2 = Address::generate(&env);
    let maker3 = Address::generate(&env);
    let taker = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place multiple sell orders with different prices
    // Maker1: sell at 1.1 USDC/XLM
    OrderManager::place_limit_order(
        &env, maker1.clone(), xlm.clone(), usdc.clone(), OrderSide::Sell, 500, (PRECISION*11/10), None
    ).unwrap();
    
    // Maker2: sell at 1.0 USDC/XLM (better price for taker)
    OrderManager::place_limit_order(
        &env, maker2.clone(), xlm.clone(), usdc.clone(), OrderSide::Sell, 500, PRECISION, None
    ).unwrap();
    
    // Maker3: sell at 0.9 USDC/XLM (best price)
    let order_id3 = OrderManager::place_limit_order(
        &env, maker3.clone(), xlm.clone(), usdc.clone(), OrderSide::Sell, 500, (PRECISION*9/10), None
    ).unwrap();

    // Taker buys 400 XLM - should fill the lowest price first (maker3's order)
    let fills = OrderManager::take_order(
        &env, taker.clone(), xlm.clone(), usdc.clone(), OrderSide::Buy, 400, None
    ).unwrap();

    assert_eq!(fills.len(), 1);
    assert_eq!(fills.get(0).unwrap().order_id, order_id3);
    assert_eq!(fills.get(0).unwrap().price, (PRECISION*9/10));
}

#[test]
fn test_orderbook_snapshot() {
    let env = Env::default();
    let maker1 = Address::generate(&env);
    let maker2 = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place bid orders (buy XLM)
    OrderManager::place_limit_order(
        &env, maker1.clone(), xlm.clone(), usdc.clone(), OrderSide::Buy, 1000, PRECISION, None
    ).unwrap();
    
    // Place another bid at same price
    OrderManager::place_limit_order(
        &env, maker2.clone(), xlm.clone(), usdc.clone(), OrderSide::Buy, 2000, PRECISION, None
    ).unwrap();

    // Get orderbook snapshot
    let snapshot = OrderManager::get_orderbook_snapshot(
        &env, xlm.clone(), usdc.clone(), 10
    ).unwrap();

    // Verify snapshot contains aggregated bids
    assert_eq!(snapshot.bids.len(), 1);
    let bid_level = snapshot.bids.get(0).unwrap();
    assert_eq!(bid_level.price, PRECISION);
    assert_eq!(bid_level.total_amount, 3000); // 1000 + 2000
    assert_eq!(bid_level.order_count, 2);
}

#[test]
fn test_cancel_order_frees_balance() {
    let env = Env::default();
    let maker = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place order
    let order_id = OrderManager::place_limit_order(
        &env, maker.clone(), xlm.clone(), usdc.clone(), OrderSide::Sell, 1000, PRECISION, None
    ).unwrap();

    // Cancel order
    OrderManager::cancel_order(&env, order_id, maker.clone()).unwrap();

    // Verify order is cancelled
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.status, OrderStatus::Cancelled);
    
    // Verify it's not in the orderbook anymore (won't be filled in future takes)
    let taker = Address::generate(&env);
    let fills = OrderManager::take_order(
        &env, taker.clone(), xlm.clone(), usdc.clone(), OrderSide::Buy, 1000, None
    ).unwrap();
    
    assert_eq!(fills.len(), 0); // No orders left to fill
}