use soroban_sdk::{Address, Env, Vec};

use crate::errors::TradeError;
use crate::events::{emit_fill, emit_trade_executed};
use crate::liquidity_pool::PoolManager;
use crate::orderbook::OrderBookManager;
use crate::token::transfer_token;
use crate::types::{
    FillResult, OrderSide, OrderStatus, TradeExecutionResult, TradeLeg, PRICE_PRECISION,
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
                // Iterate ask price levels (prices sorted ascending for best fill)
                let ask_keys = book.asks.keys();

                for ki in 0..ask_keys.len() {
                    if remaining_base <= 0 {
                        break;
                    }

                    let price = ask_keys.get(ki).unwrap();
                    let mut order_ids = book.asks.get(price).unwrap();

                    let mut i = 0;
                    while i < order_ids.len() {
                        if remaining_base <= 0 {
                            break;
                        }

                        let order_id = order_ids.get(i).unwrap();
                        if let Some(mut order) = OrderBookManager::get_order(env, order_id) {
                            if (order.status != OrderStatus::Pending
                                && order.status != OrderStatus::PartiallyFilled)
                                || (order.expires_at > 0 && order.expires_at <= now)
                            {
                                i += 1;
                                continue;
                            }

                            if leg.limit_price > 0 && order.price > leg.limit_price {
                                i += 1;
                                continue;
                            }

                            let order_remaining_base =
                                order.amount.saturating_sub(order.filled_amount);
                            let fill_base = remaining_base.min(order_remaining_base);
                            let fill_quote =
                                (fill_base as u128).saturating_mul(order.price) / PRICE_PRECISION;
                            let fill_quote_i128 = fill_quote as i128;

                            order.filled_amount = order.filled_amount.saturating_add(fill_base);
                            let fully_filled = order.filled_amount >= order.amount;
                            if fully_filled {
                                order.status = OrderStatus::Filled;
                            } else {
                                order.status = OrderStatus::PartiallyFilled;
                            }

                            OrderBookManager::save_order(env, &order);

                            // Token Settlement:
                            // Trader sends quote_asset to Maker
                            transfer_token(
                                env,
                                &leg.quote_asset,
                                trader,
                                &order.owner,
                                fill_quote_i128,
                            )?;
                            // Contract releases base_asset (from maker escrow) to Trader
                            transfer_token(
                                env,
                                &leg.base_asset,
                                &contract_addr,
                                trader,
                                fill_base,
                            )?;

                            // Update price-level aggregate
                            OrderBookManager::update_level_after_fill(
                                env, &mut book, &order, fill_base,
                            );

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

                            if fully_filled {
                                // Remove filled order from the price-level Vec
                                let mut new_ids = soroban_sdk::Vec::new(env);
                                for j in 0..order_ids.len() {
                                    let id = order_ids.get(j).unwrap();
                                    if id != order_id {
                                        new_ids.push_back(id);
                                    }
                                }
                                order_ids = new_ids;
                                // Don't increment i; check the next order at same position
                            } else {
                                i += 1;
                            }
                        } else {
                            i += 1;
                        }
                    }

                    // Save the updated order IDs back
                    book.asks.set(price, order_ids);
                }

                OrderBookManager::save_book(env, &book);

                // Fallback Liquidity Pool matching
                if remaining_base > 0 {
                    if let Some(mut pool) =
                        PoolManager::get_pool_by_pair(env, &leg.base_asset, &leg.quote_asset)
                    {
                        let (is_a_to_b, reserve_in, reserve_out) =
                            if pool.asset_a == leg.quote_asset {
                                (true, pool.reserve_a, pool.reserve_b)
                            } else {
                                (false, pool.reserve_b, pool.reserve_a)
                            };

                        if reserve_in > 0 && reserve_out >= remaining_base {
                            let fee_multiplier = 10_000i128.saturating_sub(pool.fee_bps as i128);
                            let num = reserve_in
                                .saturating_mul(remaining_base)
                                .saturating_mul(10_000i128);
                            let den = (reserve_out.saturating_sub(remaining_base))
                                .saturating_mul(fee_multiplier);

                            if den > 0 {
                                let quote_in = num / den + 1;
                                let fill_base = remaining_base;
                                let fill_quote = quote_in;

                                transfer_token(
                                    env,
                                    &leg.quote_asset,
                                    trader,
                                    &contract_addr,
                                    fill_quote,
                                )?;
                                transfer_token(
                                    env,
                                    &leg.base_asset,
                                    &contract_addr,
                                    trader,
                                    fill_base,
                                )?;

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
                // Iterate bid price levels (prices sorted descending for best fill)
                let bid_keys = book.bids.keys();

                for ki in 0..bid_keys.len() {
                    if remaining_base <= 0 {
                        break;
                    }

                    let price = bid_keys.get(ki).unwrap();
                    let mut order_ids = book.bids.get(price).unwrap();

                    let mut i = 0;
                    while i < order_ids.len() {
                        if remaining_base <= 0 {
                            break;
                        }

                        let order_id = order_ids.get(i).unwrap();
                        if let Some(mut order) = OrderBookManager::get_order(env, order_id) {
                            if (order.status != OrderStatus::Pending
                                && order.status != OrderStatus::PartiallyFilled)
                                || (order.expires_at > 0 && order.expires_at <= now)
                            {
                                i += 1;
                                continue;
                            }

                            if leg.limit_price > 0 && order.price < leg.limit_price {
                                i += 1;
                                continue;
                            }

                            let order_remaining_base =
                                order.amount.saturating_sub(order.filled_amount);
                            let fill_base = remaining_base.min(order_remaining_base);
                            let fill_quote =
                                (fill_base as u128).saturating_mul(order.price) / PRICE_PRECISION;
                            let fill_quote_i128 = fill_quote as i128;

                            order.filled_amount = order.filled_amount.saturating_add(fill_base);
                            let fully_filled = order.filled_amount >= order.amount;
                            if fully_filled {
                                order.status = OrderStatus::Filled;
                            } else {
                                order.status = OrderStatus::PartiallyFilled;
                            }

                            OrderBookManager::save_order(env, &order);

                            // Token Settlement:
                            // Trader sends base_asset to Maker
                            transfer_token(env, &leg.base_asset, trader, &order.owner, fill_base)?;
                            // Contract releases quote_asset (from maker bid escrow) to Trader
                            transfer_token(
                                env,
                                &leg.quote_asset,
                                &contract_addr,
                                trader,
                                fill_quote_i128,
                            )?;

                            // Update price-level aggregate
                            OrderBookManager::update_level_after_fill(
                                env, &mut book, &order, fill_base,
                            );

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

                            if fully_filled {
                                let mut new_ids = soroban_sdk::Vec::new(env);
                                for j in 0..order_ids.len() {
                                    let id = order_ids.get(j).unwrap();
                                    if id != order_id {
                                        new_ids.push_back(id);
                                    }
                                }
                                order_ids = new_ids;
                            } else {
                                i += 1;
                            }
                        } else {
                            i += 1;
                        }
                    }

                    book.bids.set(price, order_ids);
                }

                OrderBookManager::save_book(env, &book);

                // Fallback Liquidity Pool matching
                if remaining_base > 0 {
                    if let Some(mut pool) =
                        PoolManager::get_pool_by_pair(env, &leg.base_asset, &leg.quote_asset)
                    {
                        let (is_a_to_b, reserve_in, reserve_out) = if pool.asset_a == leg.base_asset
                        {
                            (true, pool.reserve_a, pool.reserve_b)
                        } else {
                            (false, pool.reserve_b, pool.reserve_a)
                        };

                        if reserve_in > 0 && reserve_out > 0 {
                            let fill_quote = PoolManager::get_amount_out(
                                remaining_base,
                                reserve_in,
                                reserve_out,
                                pool.fee_bps,
                            )?;
                            let fill_base = remaining_base;

                            transfer_token(
                                env,
                                &leg.base_asset,
                                trader,
                                &contract_addr,
                                fill_base,
                            )?;
                            transfer_token(
                                env,
                                &leg.quote_asset,
                                &contract_addr,
                                trader,
                                fill_quote,
                            )?;

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
