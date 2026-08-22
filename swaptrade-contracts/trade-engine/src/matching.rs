use soroban_sdk::{Address, Env, Vec};

use crate::errors::TradeError;
use crate::events::{emit_fill, emit_trade_executed};
use crate::liquidity_pool::PoolManager;
use crate::orderbook::OrderBookManager;
use crate::token::transfer_token;
use crate::types::{
    FillResult, OrderSide, OrderStatus, TradeExecutionResult, TradeLeg,
    PRICE_PRECISION,
};

pub struct MatchingEngine;

impl MatchingEngine {
    /// Execute a multi-pair trade across multiple trade legs atomically.
    /// Either ALL legs satisfy prices/slippage limits and execute successfully, or NONE do.
    pub fn execute_multi_pair_trade(
        env: &Env,
        trader: &Address,
        legs: &Vec<TradeLeg>,
    ) -> Result<TradeExecutionResult, TradeError> {
        if legs.is_empty() {
            return Err(TradeError::InvalidLegs);
        }

        let mut all_fills = Vec::new(env);
        let mut legs_executed = 0u32;

        // Iterate through each leg and attempt to match against order book and fallback pools
        for i in 0..legs.len() {
            let leg = legs.get(i).unwrap();
            let fills = Self::match_leg(env, trader, &leg)?;

            for j in 0..fills.len() {
                let fill = fills.get(j).unwrap();
                all_fills.push_back(fill);
            }
            legs_executed += 1;
        }

        emit_trade_executed(env, trader, legs_executed, all_fills.len());

        Ok(TradeExecutionResult {
            success: true,
            legs_executed,
            fills: all_fills,
        })
    }

    /// Match a single trade leg against order book bids/asks and fallback liquidity pool if needed.
    fn match_leg(
        env: &Env,
        trader: &Address,
        leg: &TradeLeg,
    ) -> Result<Vec<FillResult>, TradeError> {
        if leg.amount <= 0 {
            return Err(TradeError::InvalidAmount);
        }
        if leg.base_asset == leg.quote_asset {
            return Err(TradeError::SameAsset);
        }

        let mut remaining_base = leg.amount;
        let mut total_quote_accumulated: i128 = 0;
        let mut total_base_accumulated: i128 = 0;
        let mut fills = Vec::new(env);
        let now = env.ledger().timestamp();
        let contract_addr = env.current_contract_address();

        let mut book = OrderBookManager::load_book(env, &leg.base_asset, &leg.quote_asset);

        match leg.side {
            OrderSide::Buy => {
                // Trader wants to buy base asset by matching against asks (sell orders)
                let ask_ids = book.ask_order_ids.clone();
                let mut updated_ask_ids = Vec::new(env);

                for i in 0..ask_ids.len() {
                    if remaining_base <= 0 {
                        for k in i..ask_ids.len() {
                            updated_ask_ids.push_back(ask_ids.get(k).unwrap());
                        }
                        break;
                    }

                    let order_id = ask_ids.get(i).unwrap();
                    if let Some(mut order) = OrderBookManager::get_order(env, order_id) {
                        if (order.status != OrderStatus::Pending && order.status != OrderStatus::PartiallyFilled)
                            || (order.expires_at > 0 && order.expires_at <= now)
                        {
                            continue;
                        }

                        if leg.limit_price > 0 && order.price > leg.limit_price {
                            updated_ask_ids.push_back(order_id);
                            continue;
                        }

                        let order_remaining_base = order.amount.saturating_sub(order.filled_amount);
                        let fill_base = remaining_base.min(order_remaining_base);
                        let fill_quote = (fill_base as u128)
                            .saturating_mul(order.price)
                            / PRICE_PRECISION;
                        let fill_quote_i128 = fill_quote as i128;

                        order.filled_amount = order.filled_amount.saturating_add(fill_base);
                        if order.filled_amount >= order.amount {
                            order.status = OrderStatus::Filled;
                        } else {
                            order.status = OrderStatus::PartiallyFilled;
                            updated_ask_ids.push_back(order_id);
                        }

                        OrderBookManager::save_order(env, &order);

                        // Token Settlement:
                        // Trader sends quote_asset to Maker
                        transfer_token(env, &leg.quote_asset, trader, &order.owner, fill_quote_i128)?;
                        // Contract releases base_asset (from maker escrow) to Trader
                        transfer_token(env, &leg.base_asset, &contract_addr, trader, fill_base)?;

                        remaining_base -= fill_base;
                        total_base_accumulated += fill_base;
                        total_quote_accumulated += fill_quote_i128;

                        let fill = FillResult {
                            order_id,
                            maker: order.owner.clone(),
                            taker: trader.clone(),
                            base_asset: leg.base_asset.clone(),
                            quote_asset: leg.quote_asset.clone(),
                            price: order.price,
                            filled_base: fill_base,
                            filled_quote: fill_quote_i128,
                            filled_via_pool: false,
                            pool_id: 0,
                        };

                        emit_fill(env, &fill);
                        fills.push_back(fill);
                    }
                }

                book.ask_order_ids = updated_ask_ids;
                OrderBookManager::save_book(env, &book);

                // Fallback Liquidity Pool matching
                if remaining_base > 0 {
                    if let Some(mut pool) = PoolManager::get_pool_by_pair(env, &leg.base_asset, &leg.quote_asset) {
                        let (is_a_to_b, reserve_in, reserve_out) = if pool.asset_a == leg.quote_asset {
                            (true, pool.reserve_a, pool.reserve_b)
                        } else {
                            (false, pool.reserve_b, pool.reserve_a)
                        };

                        if reserve_in > 0 && reserve_out >= remaining_base {
                            let fee_multiplier = 10_000i128.saturating_sub(pool.fee_bps as i128);
                            let num = reserve_in.saturating_mul(remaining_base).saturating_mul(10_000i128);
                            let den = (reserve_out.saturating_sub(remaining_base)).saturating_mul(fee_multiplier);

                            if den > 0 {
                                let quote_in = num / den + 1;
                                let fill_base = remaining_base;
                                let fill_quote = quote_in;

                                // Token Settlement for Liquidity Pool Buy:
                                // Trader deposits quote_asset to contract escrow
                                transfer_token(env, &leg.quote_asset, trader, &contract_addr, fill_quote)?;
                                // Contract transfers base_asset to trader
                                transfer_token(env, &leg.base_asset, &contract_addr, trader, fill_base)?;

                                if is_a_to_b {
                                    pool.reserve_a += fill_quote;
                                    pool.reserve_b -= fill_base;
                                } else {
                                    pool.reserve_b += fill_quote;
                                    pool.reserve_a -= fill_base;
                                }
                                PoolManager::save_pool(env, &pool);

                                let effective_price = (fill_quote as u128)
                                    .saturating_mul(PRICE_PRECISION)
                                    / (fill_base as u128);

                                if leg.limit_price == 0 || effective_price <= leg.limit_price {
                                    remaining_base = 0;
                                    total_base_accumulated += fill_base;
                                    total_quote_accumulated += fill_quote;

                                    let fill = FillResult {
                                        order_id: 0,
                                        maker: contract_addr.clone(),
                                        taker: trader.clone(),
                                        base_asset: leg.base_asset.clone(),
                                        quote_asset: leg.quote_asset.clone(),
                                        price: effective_price,
                                        filled_base: fill_base,
                                        filled_quote: fill_quote,
                                        filled_via_pool: true,
                                        pool_id: pool.pool_id,
                                    };

                                    emit_fill(env, &fill);
                                    fills.push_back(fill);
                                }
                            }
                        }
                    }
                }

                if remaining_base > 0 {
                    return Err(TradeError::InsufficientLiquidity);
                }

                if leg.min_output_amount > 0 && total_base_accumulated < leg.min_output_amount {
                    return Err(TradeError::SlippageExceeded);
                }
            }

            OrderSide::Sell => {
                // Trader wants to sell base asset by matching against bids (buy orders)
                let bid_ids = book.bid_order_ids.clone();
                let mut updated_bid_ids = Vec::new(env);

                for i in 0..bid_ids.len() {
                    if remaining_base <= 0 {
                        for k in i..bid_ids.len() {
                            updated_bid_ids.push_back(bid_ids.get(k).unwrap());
                        }
                        break;
                    }

                    let order_id = bid_ids.get(i).unwrap();
                    if let Some(mut order) = OrderBookManager::get_order(env, order_id) {
                        if (order.status != OrderStatus::Pending && order.status != OrderStatus::PartiallyFilled)
                            || (order.expires_at > 0 && order.expires_at <= now)
                        {
                            continue;
                        }

                        if leg.limit_price > 0 && order.price < leg.limit_price {
                            updated_bid_ids.push_back(order_id);
                            continue;
                        }

                        let order_remaining_base = order.amount.saturating_sub(order.filled_amount);
                        let fill_base = remaining_base.min(order_remaining_base);
                        let fill_quote = (fill_base as u128)
                            .saturating_mul(order.price)
                            / PRICE_PRECISION;
                        let fill_quote_i128 = fill_quote as i128;

                        order.filled_amount = order.filled_amount.saturating_add(fill_base);
                        if order.filled_amount >= order.amount {
                            order.status = OrderStatus::Filled;
                        } else {
                            order.status = OrderStatus::PartiallyFilled;
                            updated_bid_ids.push_back(order_id);
                        }

                        OrderBookManager::save_order(env, &order);

                        // Token Settlement:
                        // Trader sends base_asset to Maker
                        transfer_token(env, &leg.base_asset, trader, &order.owner, fill_base)?;
                        // Contract releases quote_asset (from maker bid escrow) to Trader
                        transfer_token(env, &leg.quote_asset, &contract_addr, trader, fill_quote_i128)?;

                        remaining_base -= fill_base;
                        total_base_accumulated += fill_base;
                        total_quote_accumulated += fill_quote_i128;

                        let fill = FillResult {
                            order_id,
                            maker: order.owner.clone(),
                            taker: trader.clone(),
                            base_asset: leg.base_asset.clone(),
                            quote_asset: leg.quote_asset.clone(),
                            price: order.price,
                            filled_base: fill_base,
                            filled_quote: fill_quote_i128,
                            filled_via_pool: false,
                            pool_id: 0,
                        };

                        emit_fill(env, &fill);
                        fills.push_back(fill);
                    }
                }

                book.bid_order_ids = updated_bid_ids;
                OrderBookManager::save_book(env, &book);

                // Fallback Liquidity Pool matching
                if remaining_base > 0 {
                    if let Some(mut pool) = PoolManager::get_pool_by_pair(env, &leg.base_asset, &leg.quote_asset) {
                        let (is_a_to_b, reserve_in, reserve_out) = if pool.asset_a == leg.base_asset {
                            (true, pool.reserve_a, pool.reserve_b)
                        } else {
                            (false, pool.reserve_b, pool.reserve_a)
                        };

                        if reserve_in > 0 && reserve_out > 0 {
                            let fill_quote = PoolManager::get_amount_out(remaining_base, reserve_in, reserve_out, pool.fee_bps)?;
                            let fill_base = remaining_base;

                            // Token Settlement for Liquidity Pool Sell:
                            // Trader deposits base_asset into contract escrow
                            transfer_token(env, &leg.base_asset, trader, &contract_addr, fill_base)?;
                            // Contract transfers quote_asset to trader
                            transfer_token(env, &leg.quote_asset, &contract_addr, trader, fill_quote)?;

                            if is_a_to_b {
                                pool.reserve_a += fill_base;
                                pool.reserve_b -= fill_quote;
                            } else {
                                pool.reserve_b += fill_base;
                                pool.reserve_a -= fill_quote;
                            }
                            PoolManager::save_pool(env, &pool);

                            let effective_price = (fill_quote as u128)
                                .saturating_mul(PRICE_PRECISION)
                                / (fill_base as u128);

                            if leg.limit_price == 0 || effective_price >= leg.limit_price {
                                remaining_base = 0;
                                total_base_accumulated += fill_base;
                                total_quote_accumulated += fill_quote;

                                let fill = FillResult {
                                    order_id: 0,
                                    maker: contract_addr.clone(),
                                    taker: trader.clone(),
                                    base_asset: leg.base_asset.clone(),
                                    quote_asset: leg.quote_asset.clone(),
                                    price: effective_price,
                                    filled_base: fill_base,
                                    filled_quote: fill_quote,
                                    filled_via_pool: true,
                                    pool_id: pool.pool_id,
                                };

                                emit_fill(env, &fill);
                                fills.push_back(fill);
                            }
                        }
                    }
                }

                if remaining_base > 0 {
                    return Err(TradeError::InsufficientLiquidity);
                }

                if leg.min_output_amount > 0 && total_quote_accumulated < leg.min_output_amount {
                    return Err(TradeError::SlippageExceeded);
                }
            }
        }

        let _ = total_quote_accumulated;
        let _ = total_base_accumulated;

        Ok(fills)
    }
}
