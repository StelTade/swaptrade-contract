use soroban_sdk::{Address, Env, Vec};

use crate::errors::PortfolioError;
use crate::events;
use crate::storage;
use crate::types::{
    AssetPrice, CostBasisMethod, CostLot, Position, TransactionRecord, TransactionType,
    PRICE_PRECISION,
};

pub struct PortfolioManager;

impl PortfolioManager {
    // ── Record a Buy Transaction ─────────────────────────────────────────────

    /// Record a buy/inflow that opens or adds to a position.
    /// Updates cost basis, cost lots, and position state.
    pub fn record_buy(
        env: &Env,
        user: &Address,
        asset: &Address,
        quote_asset: &Address,
        quantity: i128,
        price: i128,
    ) -> Result<TransactionRecord, PortfolioError> {
        if quantity <= 0 {
            return Err(PortfolioError::InvalidQuantity);
        }
        if price <= 0 {
            return Err(PortfolioError::InvalidPrice);
        }

        let tx_id = storage::get_next_tx_id(env)?;
        let now = env.ledger().timestamp();
        let total_value = (quantity as i128)
            .saturating_mul(price)
            .checked_div(PRICE_PRECISION)
            .unwrap_or(0);

        let existing = storage::get_position(env, user, asset);

        let position = if let Some(mut pos) = existing {
            // Position already exists — add to it
            pos.cost_lots
                .push_back(CostLot {
                    timestamp: now,
                    quantity,
                    cost_per_unit: price,
                    total_cost: total_value,
                });
            pos.quantity = pos.quantity.saturating_add(quantity);
            pos.total_cost_basis = pos.total_cost_basis.saturating_add(total_value);
            pos.total_invested = pos.total_invested.saturating_add(total_value);
            pos.buy_count = pos.buy_count.saturating_add(1);
            pos.last_updated = now;

            // Recalculate average cost basis
            pos.avg_cost_basis = if pos.quantity > 0 {
                (pos.total_cost_basis as i128)
                    .checked_mul(PRICE_PRECISION)
                    .unwrap_or(i128::MAX)
                    .checked_div(pos.quantity as i128)
                    .unwrap_or(0)
            } else {
                0
            };

            events::emit_position_updated(env, user, asset, pos.quantity, pos.avg_cost_basis, 0);

            pos
        } else {
            // New position
            let mut cost_lots = Vec::new(env);
            cost_lots.push_back(CostLot {
                timestamp: now,
                quantity,
                cost_per_unit: price,
                total_cost: total_value,
            });

            let pos = Position {
                user: user.clone(),
                asset: asset.clone(),
                quote_asset: quote_asset.clone(),
                quantity,
                avg_cost_basis: price,
                total_cost_basis: total_value,
                realized_pnl: 0,
                total_invested: total_value,
                total_divested: 0,
                buy_count: 1,
                sell_count: 0,
                cost_method: CostBasisMethod::WeightedAverage,
                cost_lots,
                last_updated: now,
                open_time: now,
            };

            events::emit_position_opened(
                env,
                user,
                asset,
                quote_asset,
                quantity,
                price,
                &CostBasisMethod::WeightedAverage,
            );

            pos
        };

        storage::save_position(env, &position);

        // Create and save transaction record
        let record = TransactionRecord {
            tx_id,
            user: user.clone(),
            asset: asset.clone(),
            quote_asset: quote_asset.clone(),
            tx_type: TransactionType::Buy,
            quantity,
            price,
            total_value,
            realized_pnl: 0,
            remaining_quantity: position.quantity,
            timestamp: now,
        };

        storage::save_transaction(env, &record);
        events::emit_transaction(env, &record);

        Ok(record)
    }

    // ── Record a Sell Transaction ────────────────────────────────────────────

    /// Record a sell/outflow that reduces or closes a position.
    /// Calculates realized P&L based on the position's cost basis method.
    pub fn record_sell(
        env: &Env,
        user: &Address,
        asset: &Address,
        quantity: i128,
        price: i128,
    ) -> Result<TransactionRecord, PortfolioError> {
        if quantity <= 0 {
            return Err(PortfolioError::InvalidQuantity);
        }
        if price <= 0 {
            return Err(PortfolioError::InvalidPrice);
        }

        let mut position =
            storage::get_position(env, user, asset).ok_or(PortfolioError::PositionNotFound)?;

        if position.quantity < quantity {
            return Err(PortfolioError::InsufficientQuantity);
        }

        let tx_id = storage::get_next_tx_id(env)?;
        let now = env.ledger().timestamp();
        let total_value = (quantity as i128)
            .saturating_mul(price)
            .checked_div(PRICE_PRECISION)
            .unwrap_or(0);

        // Calculate realized P&L based on cost basis method
        let realized_pnl = Self::calculate_realized_pnl(env, &mut position, quantity, price)?;

        // Update position state
        position.quantity = position.quantity.saturating_sub(quantity);
        position.total_divested = position.total_divested.saturating_add(total_value);
        position.sell_count = position.sell_count.saturating_add(1);
        position.last_updated = now;

        if position.quantity == 0 {
            // Position fully closed
            events::emit_position_closed(
                env,
                user,
                asset,
                position.realized_pnl,
                position.total_invested,
                position.total_divested,
            );
            // Remove from tracked assets
            storage::remove_user_asset(env, user, asset);
            // Remove position from storage (no longer active)
            storage::remove_position(env, user, asset);
        } else {
            // Partial close — update cost basis
            if position.cost_method == CostBasisMethod::WeightedAverage {
                position.total_cost_basis = (position.quantity as i128)
                    .saturating_mul(position.avg_cost_basis)
                    .checked_div(PRICE_PRECISION)
                    .unwrap_or(0);
            }
            events::emit_position_updated(
                env,
                user,
                asset,
                position.quantity,
                position.avg_cost_basis,
                realized_pnl,
            );
            storage::save_position(env, &position);
        }

        // Create and save transaction record
        let record = TransactionRecord {
            tx_id,
            user: user.clone(),
            asset: asset.clone(),
            quote_asset: position.quote_asset.clone(),
            tx_type: TransactionType::Sell,
            quantity,
            price,
            total_value,
            realized_pnl,
            remaining_quantity: position.quantity,
            timestamp: now,
        };

        storage::save_transaction(env, &record);
        events::emit_transaction(env, &record);

        Ok(record)
    }

    // ── Cost Basis Calculation (Realized P&L) ───────────────────────────────

    /// Calculate realized P&L for a sell and dispose of cost lots accordingly.
    fn calculate_realized_pnl(
        env: &Env,
        position: &mut Position,
        sell_quantity: i128,
        sell_price: i128,
    ) -> Result<i128, PortfolioError> {
        match position.cost_method {
            CostBasisMethod::Fifo => {
                Self::dispose_lots_fifo(env, position, sell_quantity, sell_price)
            }
            CostBasisMethod::Lifo => {
                Self::dispose_lots_lifo(env, position, sell_quantity, sell_price)
            }
            CostBasisMethod::WeightedAverage => {
                Self::dispose_lots_weighted_avg(position, sell_quantity, sell_price)
            }
        }
    }

    /// FIFO: Dispose of the oldest cost lots first
    fn dispose_lots_fifo(
        env: &Env,
        position: &mut Position,
        sell_quantity: i128,
        sell_price: i128,
    ) -> Result<i128, PortfolioError> {
        let mut remaining_to_sell = sell_quantity;
        let mut total_cost_of_sold = 0i128;
        let mut lots_to_remove: Vec<u32> = Vec::new(env);
        let mut partial_lot_idx: Option<u32> = None;
        let mut partial_lot_new_qty = 0i128;

        for i in 0..position.cost_lots.len() {
            if remaining_to_sell <= 0 {
                break;
            }
            let lot = position.cost_lots.get(i).unwrap();
            if lot.quantity <= remaining_to_sell {
                // Fully consume this lot
                total_cost_of_sold = total_cost_of_sold.saturating_add(lot.total_cost);
                remaining_to_sell = remaining_to_sell.saturating_sub(lot.quantity);
                lots_to_remove.push_back(i);
            } else {
                // Partially consume this lot
                let consumed_qty = remaining_to_sell;
                let consumed_cost = (consumed_qty as i128)
                    .saturating_mul(lot.cost_per_unit);
                total_cost_of_sold = total_cost_of_sold.saturating_add(consumed_cost);
                partial_lot_new_qty = lot.quantity - consumed_qty;
                partial_lot_idx = Some(i);
                remaining_to_sell = 0;
            }
        }

        // Update partial lot if needed
        if let Some(idx) = partial_lot_idx {
            let mut lot = position.cost_lots.get(idx as u32).unwrap();
            lot.quantity = partial_lot_new_qty;
            lot.total_cost = lot
                .quantity
                .saturating_mul(lot.cost_per_unit);
            position.cost_lots.set(idx as u32, lot);
        }

        // Remove fully consumed lots (iterate in reverse to avoid index shifting)
        let mut idx = lots_to_remove.len();
        while idx > 0 {
            idx -= 1;
            let lot_idx = lots_to_remove.get(idx).unwrap();
            position.cost_lots.remove(lot_idx as u32);
        }

        // Realized P&L = proceeds - cost of goods sold
        let proceeds = (sell_quantity as i128)
            .saturating_mul(sell_price)
            .checked_div(PRICE_PRECISION)
            .unwrap_or(0);
        let pnl = proceeds.saturating_sub(total_cost_of_sold);
        position.realized_pnl = position.realized_pnl.saturating_add(pnl);

        Ok(pnl)
    }

    /// LIFO: Dispose of the newest cost lots first
    fn dispose_lots_lifo(
        env: &Env,
        position: &mut Position,
        sell_quantity: i128,
        sell_price: i128,
    ) -> Result<i128, PortfolioError> {
        let mut remaining_to_sell = sell_quantity;
        let mut total_cost_of_sold = 0i128;
        let mut lots_to_remove: Vec<u32> = Vec::new(env);
        let mut partial_lot_idx: Option<u32> = None;
        let mut partial_lot_new_qty = 0i128;

        // Iterate from newest to oldest
        let lot_count = position.cost_lots.len();
        for i in (0..lot_count).rev() {
            if remaining_to_sell <= 0 {
                break;
            }
            let lot = position.cost_lots.get(i).unwrap();
            if lot.quantity <= remaining_to_sell {
                total_cost_of_sold = total_cost_of_sold.saturating_add(lot.total_cost);
                remaining_to_sell = remaining_to_sell.saturating_sub(lot.quantity);
                lots_to_remove.push_back(i);
            } else {
                let consumed_qty = remaining_to_sell;
                let consumed_cost = (consumed_qty as i128)
                    .saturating_mul(lot.cost_per_unit);
                total_cost_of_sold = total_cost_of_sold.saturating_add(consumed_cost);
                partial_lot_new_qty = lot.quantity - consumed_qty;
                partial_lot_idx = Some(i);
                remaining_to_sell = 0;
            }
        }

        // Update partial lot
        if let Some(idx) = partial_lot_idx {
            let mut lot = position.cost_lots.get(idx as u32).unwrap();
            lot.quantity = partial_lot_new_qty;
            lot.total_cost = lot
                .quantity
                .saturating_mul(lot.cost_per_unit);
            position.cost_lots.set(idx as u32, lot);
        }

        // Remove fully consumed lots (in reverse to preserve indices)
        let mut idx = lots_to_remove.len();
        while idx > 0 {
            idx -= 1;
            let lot_idx = lots_to_remove.get(idx).unwrap();
            position.cost_lots.remove(lot_idx as u32);
        }

        let proceeds = (sell_quantity as i128)
            .saturating_mul(sell_price)
            .checked_div(PRICE_PRECISION)
            .unwrap_or(0);
        let pnl = proceeds.saturating_sub(total_cost_of_sold);
        position.realized_pnl = position.realized_pnl.saturating_add(pnl);

        Ok(pnl)
    }

    /// Weighted Average: Single blended cost basis, P&L against average
    fn dispose_lots_weighted_avg(
        position: &mut Position,
        sell_quantity: i128,
        sell_price: i128,
    ) -> Result<i128, PortfolioError> {
        let avg_cost = position.avg_cost_basis;
        let cost_of_sold = (sell_quantity as i128)
            .saturating_mul(avg_cost)
            .checked_div(PRICE_PRECISION)
            .unwrap_or(0);
        let proceeds = (sell_quantity as i128)
            .saturating_mul(sell_price)
            .checked_div(PRICE_PRECISION)
            .unwrap_or(0);

        let pnl = proceeds.saturating_sub(cost_of_sold);
        position.realized_pnl = position.realized_pnl.saturating_add(pnl);

        // Remove proportional cost from total
        position.total_cost_basis = position.total_cost_basis.saturating_sub(cost_of_sold);

        // Rebuild cost lots for remaining quantity (single lot at avg cost)
        let remaining = position.quantity - sell_quantity;
        if remaining > 0 {
            let mut new_lots = Vec::new(position.cost_lots.env());
            new_lots.push_back(CostLot {
                timestamp: position.last_updated,
                quantity: remaining,
                cost_per_unit: avg_cost,
                total_cost: (remaining as i128)
                    .saturating_mul(avg_cost)
                    .checked_div(PRICE_PRECISION)
                    .unwrap_or(0),
            });
            position.cost_lots = new_lots;
        } else {
            let env = position.cost_lots.env();
            position.cost_lots = Vec::new(&env);
        }

        Ok(pnl)
    }

    // ── Unrealized P&L ───────────────────────────────────────────────────────

    /// Calculate unrealized P&L for a position using current market price
    pub fn calculate_unrealized_pnl(
        env: &Env,
        position: &Position,
    ) -> Result<i128, PortfolioError> {
        let price_data = storage::get_asset_price(env, &position.asset)
            .ok_or(PortfolioError::NoPriceData)?;

        let market_value = (position.quantity as i128)
            .saturating_mul(price_data.price as i128)
            .checked_div(PRICE_PRECISION)
            .unwrap_or(0);

        Ok(market_value.saturating_sub(position.total_cost_basis))
    }

    /// Get current market value of a position
    pub fn get_market_value(
        env: &Env,
        position: &Position,
    ) -> Result<i128, PortfolioError> {
        let price_data = storage::get_asset_price(env, &position.asset)
            .ok_or(PortfolioError::NoPriceData)?;

        Ok((position.quantity as i128)
            .saturating_mul(price_data.price as i128)
            .checked_div(PRICE_PRECISION)
            .unwrap_or(0))
    }

    // ── Price Oracle ─────────────────────────────────────────────────────────

    /// Update market price for an asset (called by oracle or admin)
    pub fn update_price(
        env: &Env,
        asset: &Address,
        price: u128,
    ) -> Result<(), PortfolioError> {
        if price == 0 {
            return Err(PortfolioError::InvalidPrice);
        }
        let now = env.ledger().timestamp();
        let price_data = AssetPrice {
            asset: asset.clone(),
            price,
            timestamp: now,
        };
        storage::save_asset_price(env, &price_data);
        events::emit_price_update(env, asset, price, now);
        Ok(())
    }

    /// Get current price for an asset
    pub fn get_price(env: &Env, asset: &Address) -> Result<AssetPrice, PortfolioError> {
        storage::get_asset_price(env, &asset).ok_or(PortfolioError::NoPriceData)
    }

    // ── Portfolio Aggregation ────────────────────────────────────────────────

    /// Get all positions for a user with current unrealized P&L
    pub fn get_all_positions(
        env: &Env,
        user: &Address,
    ) -> Result<Vec<Position>, PortfolioError> {
        let assets = storage::get_user_assets(env, user);
        let mut positions = Vec::new(env);
        for i in 0..assets.len() {
            let asset = assets.get(i).unwrap();
            if let Some(pos) = storage::get_position(env, user, &asset) {
                positions.push_back(pos);
            }
        }
        Ok(positions)
    }

    /// Calculate total portfolio value (sum of all position market values)
    pub fn get_total_portfolio_value(
        env: &Env,
        user: &Address,
    ) -> Result<i128, PortfolioError> {
        let positions = Self::get_all_positions(env, user)?;
        let mut total = 0i128;

        for i in 0..positions.len() {
            let pos = positions.get(i).unwrap();
            if let Ok(val) = Self::get_market_value(env, &pos) {
                total = total.saturating_add(val);
            }
        }

        Ok(total)
    }

    /// Calculate total unrealized P&L across all positions
    pub fn get_total_unrealized_pnl(
        env: &Env,
        user: &Address,
    ) -> Result<i128, PortfolioError> {
        let positions = Self::get_all_positions(env, user)?;
        let mut total = 0i128;

        for i in 0..positions.len() {
            let pos = positions.get(i).unwrap();
            if let Ok(pnl) = Self::calculate_unrealized_pnl(env, &pos) {
                total = total.saturating_add(pnl);
            }
        }

        Ok(total)
    }

    /// Calculate total realized P&L across all positions
    pub fn get_total_realized_pnl(
        env: &Env,
        user: &Address,
    ) -> Result<i128, PortfolioError> {
        let positions = Self::get_all_positions(env, user)?;
        let mut total = 0i128;

        for i in 0..positions.len() {
            let pos = positions.get(i).unwrap();
            total = total.saturating_add(pos.realized_pnl);
        }

        Ok(total)
    }

    /// Get total invested across all positions
    pub fn get_total_invested(
        env: &Env,
        user: &Address,
    ) -> Result<i128, PortfolioError> {
        let positions = Self::get_all_positions(env, user)?;
        let mut total = 0i128;
        for i in 0..positions.len() {
            let pos = positions.get(i).unwrap();
            total = total.saturating_add(pos.total_invested);
        }
        Ok(total)
    }

    /// Get total returned from all sells across positions
    pub fn get_total_divested(
        env: &Env,
        user: &Address,
    ) -> Result<i128, PortfolioError> {
        let positions = Self::get_all_positions(env, user)?;
        let mut total = 0i128;
        for i in 0..positions.len() {
            let pos = positions.get(i).unwrap();
            total = total.saturating_add(pos.total_divested);
        }
        Ok(total)
    }
}
