#![cfg_attr(all(not(test), target_family = "wasm"), no_std)]

extern crate alloc;

pub mod errors;
pub mod events;
pub mod liquidity_pool;
pub mod matching;
pub mod orderbook;
pub mod storage;
pub mod token;
pub mod types;

pub use errors::TradeError;
pub use types::*;

use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

use events::{emit_order_cancelled, emit_order_placed, emit_pool_added};
use liquidity_pool::PoolManager;
use matching::MatchingEngine;
use orderbook::OrderBookManager;
use storage::{get_next_order_id, get_next_pool_id, StorageKey};
use token::transfer_token;

#[contract]
pub struct TradeEngineContract;

#[contractimpl]
impl TradeEngineContract {
    /// Initialize the Trade Engine contract with admin
    pub fn initialize(env: Env, admin: Address) -> Result<(), TradeError> {
        admin.require_auth();
        if env.storage().persistent().has(&StorageKey::Admin) {
            return Err(TradeError::AlreadyExists);
        }
        env.storage().persistent().set(&StorageKey::Admin, &admin);
        Ok(())
    }

    /// Place a limit or market order on the order book and escrow underlying tokens
    pub fn place_order(
        env: Env,
        owner: Address,
        base_asset: Address,
        quote_asset: Address,
        side: OrderSide,
        order_type: OrderType,
        price: u128,
        amount: i128,
        expires_at: u64,
    ) -> Result<u64, TradeError> {
        owner.require_auth();

        if amount <= 0 {
            return Err(TradeError::InvalidAmount);
        }
        if price == 0 && order_type == OrderType::Limit {
            return Err(TradeError::InvalidPrice);
        }
        if base_asset == quote_asset {
            return Err(TradeError::SameAsset);
        }

        let contract_addr = env.current_contract_address();

        // Escrow funds into contract
        match side {
            OrderSide::Sell => {
                // Selling base_asset -> escrow base_asset
                transfer_token(&env, &base_asset, &owner, &contract_addr, amount)?;
            }
            OrderSide::Buy => {
                // Buying base_asset with quote_asset -> escrow quote_asset = (amount * price) / PRICE_PRECISION
                let quote_amount = (amount as u128)
                    .saturating_mul(price)
                    / PRICE_PRECISION;
                transfer_token(&env, &quote_asset, &owner, &contract_addr, quote_amount as i128)?;
            }
        }

        let order_id = get_next_order_id(&env);
        let now = env.ledger().timestamp();

        let order = Order {
            order_id,
            owner: owner.clone(),
            base_asset: base_asset.clone(),
            quote_asset: quote_asset.clone(),
            side: side.clone(),
            order_type,
            price,
            amount,
            filled_amount: 0,
            status: OrderStatus::Pending,
            created_at: now,
            expires_at,
        };

        OrderBookManager::save_order(&env, &order);
        OrderBookManager::add_user_order(&env, &owner, order_id);
        OrderBookManager::add_order(&env, &order);

        emit_order_placed(&env, &order);

        Ok(order_id)
    }

    /// Cancel a pending or partially filled order, clean up order book state, and refund escrowed tokens
    pub fn cancel_order(env: Env, owner: Address, order_id: u64) -> Result<(), TradeError> {
        owner.require_auth();

        let mut order = OrderBookManager::get_order(&env, order_id).ok_or(TradeError::OrderNotFound)?;

        if order.owner != owner {
            return Err(TradeError::Unauthorized);
        }

        if order.status != OrderStatus::Pending && order.status != OrderStatus::PartiallyFilled {
            return Err(TradeError::InvalidState);
        }

        let unfilled_base = order.amount.saturating_sub(order.filled_amount);

        order.status = OrderStatus::Cancelled;
        OrderBookManager::save_order(&env, &order);
        OrderBookManager::remove_order_id(&env, &order.base_asset, &order.quote_asset, order.side.clone(), order_id);

        let contract_addr = env.current_contract_address();

        // Refund unfilled escrowed tokens
        if unfilled_base > 0 {
            match order.side {
                OrderSide::Sell => {
                    transfer_token(&env, &order.base_asset, &contract_addr, &owner, unfilled_base)?;
                }
                OrderSide::Buy => {
                    let unfilled_quote = (unfilled_base as u128)
                        .saturating_mul(order.price)
                        / PRICE_PRECISION;
                    transfer_token(&env, &order.quote_asset, &contract_addr, &owner, unfilled_quote as i128)?;
                }
            }
        }

        emit_order_cancelled(&env, order_id, &owner);

        Ok(())
    }

    /// Execute a multi-pair trade across 1+ asset pairs simultaneously with all-or-nothing atomic execution
    pub fn execute_multi_pair_trade(
        env: Env,
        trader: Address,
        legs: Vec<TradeLeg>,
    ) -> Result<TradeExecutionResult, TradeError> {
        trader.require_auth();
        MatchingEngine::execute_multi_pair_trade(&env, &trader, &legs)
    }

    /// Add fallback liquidity pool and deposit reserve tokens into pool escrow
    pub fn add_liquidity_pool(
        env: Env,
        admin: Address,
        asset_a: Address,
        asset_b: Address,
        reserve_a: i128,
        reserve_b: i128,
        fee_bps: u32,
    ) -> Result<u64, TradeError> {
        admin.require_auth();

        if reserve_a <= 0 || reserve_b <= 0 {
            return Err(TradeError::InvalidAmount);
        }
        if asset_a == asset_b {
            return Err(TradeError::SameAsset);
        }

        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&StorageKey::Admin)
            .ok_or(TradeError::Unauthorized)?;

        if admin != stored_admin {
            return Err(TradeError::Unauthorized);
        }

        let contract_addr = env.current_contract_address();

        // Transfer liquidity reserves from admin into contract pool escrow
        transfer_token(&env, &asset_a, &admin, &contract_addr, reserve_a)?;
        transfer_token(&env, &asset_b, &admin, &contract_addr, reserve_b)?;

        let pool_id = get_next_pool_id(&env);
        let pool = LiquidityPool {
            pool_id,
            asset_a: asset_a.clone(),
            asset_b: asset_b.clone(),
            reserve_a,
            reserve_b,
            fee_bps,
        };

        PoolManager::save_pool(&env, &pool);
        emit_pool_added(&env, pool_id, &asset_a, &asset_b);

        Ok(pool_id)
    }

    /// Query live aggregated Order Book summary for an asset pair
    pub fn get_orderbook(
        env: Env,
        base_asset: Address,
        quote_asset: Address,
        max_levels: u32,
    ) -> OrderBookSummary {
        OrderBookManager::get_summary(&env, base_asset, quote_asset, max_levels)
    }

    /// Query individual order details by order ID
    pub fn get_order(env: Env, order_id: u64) -> Option<Order> {
        OrderBookManager::get_order(&env, order_id)
    }

    /// Query active orders belonging to a user
    pub fn get_user_orders(env: Env, user: Address) -> Vec<Order> {
        let order_ids = OrderBookManager::get_user_orders(&env, &user);
        let mut user_orders = Vec::new(&env);
        for i in 0..order_ids.len() {
            let id = order_ids.get(i).unwrap();
            if let Some(order) = OrderBookManager::get_order(&env, id) {
                user_orders.push_back(order);
            }
        }
        user_orders
    }

    /// Query liquidity pool details by pool ID
    pub fn get_pool(env: Env, pool_id: u64) -> Option<LiquidityPool> {
        PoolManager::get_pool(&env, pool_id)
    }
}
