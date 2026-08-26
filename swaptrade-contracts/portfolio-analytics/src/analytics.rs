use soroban_sdk::{Address, Env, Vec};

use crate::errors::PortfolioError;
use crate::events;
use crate::portfolio::PortfolioManager;
use crate::storage;
use crate::types::{
    BPS_PRECISION, PerformanceMetrics, PortfolioSnapshot, PositionSummary,
    ReturnPeriod, TransactionType,
};

pub struct AnalyticsEngine;

impl AnalyticsEngine {
    // ── Portfolio Snapshots ──────────────────────────────────────────────────

    /// Capture a point-in-time portfolio snapshot
    pub fn capture_snapshot(
        env: &Env,
        user: &Address,
    ) -> Result<PortfolioSnapshot, PortfolioError> {
        let positions = PortfolioManager::get_all_positions(env, user)?;
        let snapshot_id = storage::get_next_snapshot_id(env)?;
        let now = env.ledger().timestamp();

        let mut total_value = 0i128;
        let mut total_cost_basis = 0i128;
        let mut total_realized_pnl = 0i128;
        let mut total_unrealized_pnl = 0i128;
        let mut total_invested = 0i128;
        let mut total_divested = 0i128;

        let mut position_summaries = Vec::new(env);

        for i in 0..positions.len() {
            let pos = positions.get(i).unwrap();
            let market_value = PortfolioManager::get_market_value(env, &pos).unwrap_or(0);
            let unrealized_pnl =
                PortfolioManager::calculate_unrealized_pnl(env, &pos).unwrap_or(0);

            total_value = total_value.saturating_add(market_value);
            total_cost_basis = total_cost_basis.saturating_add(pos.total_cost_basis);
            total_realized_pnl = total_realized_pnl.saturating_add(pos.realized_pnl);
            total_unrealized_pnl = total_unrealized_pnl.saturating_add(unrealized_pnl);
            total_invested = total_invested.saturating_add(pos.total_invested);
            total_divested = total_divested.saturating_add(pos.total_divested);

            position_summaries.push_back(PositionSummary {
                asset: pos.asset.clone(),
                quote_asset: pos.quote_asset.clone(),
                quantity: pos.quantity,
                avg_cost_basis: pos.avg_cost_basis,
                total_cost_basis: pos.total_cost_basis,
                realized_pnl: pos.realized_pnl,
                unrealized_pnl,
                market_value,
                allocation_pct: 0, // Computed below
            });
        }

        // Compute allocation percentages
        if total_value > 0 {
            let mut updated_summaries = Vec::new(env);
            for i in 0..position_summaries.len() {
                let mut summary = position_summaries.get(i).unwrap();
                let alloc = (summary.market_value as i128)
                    .saturating_mul(BPS_PRECISION)
                    .checked_div(total_value)
                    .unwrap_or(0) as u32;
                summary.allocation_pct = alloc;
                updated_summaries.push_back(summary);
            }
            position_summaries = updated_summaries;
        }

        let snapshot = PortfolioSnapshot {
            snapshot_id,
            user: user.clone(),
            timestamp: now,
            positions: position_summaries,
            total_value,
            total_cost_basis,
            total_realized_pnl,
            total_unrealized_pnl,
            total_invested,
            total_divested,
        };

        storage::save_snapshot(env, snapshot_id, user, &snapshot);
        events::emit_snapshot_captured(env, user, snapshot_id, total_value, snapshot.positions.len());

        Ok(snapshot)
    }

    /// Get a previously captured snapshot by ID
    pub fn get_snapshot(
        env: &Env,
        snapshot_id: u64,
    ) -> Result<PortfolioSnapshot, PortfolioError> {
        storage::get_snapshot(env, snapshot_id).ok_or(PortfolioError::EmptyPortfolio)
    }

    /// Get all snapshot IDs for a user
    pub fn get_user_snapshot_ids(env: &Env, user: &Address) -> Vec<u64> {
        storage::get_user_snapshot_ids(env, user)
    }

    // ── Performance Metrics ──────────────────────────────────────────────────

    /// Compute comprehensive performance metrics for a user
    pub fn compute_metrics(
        env: &Env,
        user: &Address,
    ) -> Result<PerformanceMetrics, PortfolioError> {
        let positions = PortfolioManager::get_all_positions(env, user)?;

        let tx_ids = storage::get_user_transaction_ids(env, user);
        if positions.is_empty() && tx_ids.is_empty() {
            return Err(PortfolioError::EmptyPortfolio);
        }

        // Aggregate across all positions
        let mut total_value = 0i128;
        let mut total_cost_basis = 0i128;
        let mut realized_pnl = 0i128;
        let mut unrealized_pnl = 0i128;
        let mut winning_trades = 0u64;
        let mut losing_trades = 0u64;

        for i in 0..positions.len() {
            let pos = positions.get(i).unwrap();
            let mv = PortfolioManager::get_market_value(env, &pos).unwrap_or(0);
            let upnl = PortfolioManager::calculate_unrealized_pnl(env, &pos).unwrap_or(0);

            total_value = total_value.saturating_add(mv);
            total_cost_basis = total_cost_basis.saturating_add(pos.total_cost_basis);
            realized_pnl = realized_pnl.saturating_add(pos.realized_pnl);
            unrealized_pnl = unrealized_pnl.saturating_add(upnl);
        }

        // Count winning/losing trades from sell transaction records
        for i in 0..tx_ids.len() {
            let tx_id = tx_ids.get(i).unwrap();
            if let Some(record) = storage::get_transaction(env, tx_id) {
                if record.tx_type == TransactionType::Sell {
                    if record.realized_pnl > 0 {
                        winning_trades = winning_trades.saturating_add(1);
                    } else if record.realized_pnl < 0 {
                        losing_trades = losing_trades.saturating_add(1);
                    }
                }
            }
        }

        let closed_trades = winning_trades + losing_trades;

        // ROI in basis points: (total_value - total_cost_basis) / total_cost_basis * 10000
        let roi_bps = if total_cost_basis > 0 {
            let pnl = total_value.saturating_sub(total_cost_basis);
            pnl.saturating_mul(BPS_PRECISION)
                .checked_div(total_cost_basis)
                .unwrap_or(0)
        } else {
            0
        };

        // Win/Loss ratio in basis points
        let win_loss_ratio = if losing_trades > 0 {
            (winning_trades as i128)
                .saturating_mul(BPS_PRECISION)
                .checked_div(losing_trades as i128)
                .unwrap_or(0)
        } else if winning_trades > 0 {
            BPS_PRECISION * 2 // Very high ratio when no losses
        } else {
            0
        };

        // Compute Sharpe ratio and volatility from return history
        let return_history = storage::get_return_history(env, user);
        let sharpe_ratio = Self::compute_sharpe_ratio(&return_history);
        let volatility_bps = Self::compute_volatility(&return_history);
        let max_drawdown = Self::compute_max_drawdown(&return_history);

        let metrics = PerformanceMetrics {
            user: user.clone(),
            roi_bps,
            sharpe_ratio,
            win_loss_ratio,
            total_trades: closed_trades,
            winning_trades,
            losing_trades,
            max_drawdown_bps: max_drawdown,
            volatility_bps,
            total_value,
            total_cost_basis,
            realized_pnl,
            unrealized_pnl,
        };

        events::emit_metrics_computed(env, user, roi_bps, sharpe_ratio, closed_trades);

        Ok(metrics)
    }

    // ── Sharpe Ratio Calculation ─────────────────────────────────────────────

    /// Compute Sharpe ratio from return history.
    /// Sharpe = (mean_return - risk_free_rate) / std_deviation
    /// Risk-free rate assumed 0 for simplicity (Stablecoin-denominated returns).
    /// Returns scaled by 10_000 for 4 decimal places.
    fn compute_sharpe_ratio(returns: &Vec<ReturnPeriod>) -> i128 {
        let n = returns.len();
        if n < 2 {
            return 0;
        }

        // Calculate mean return
        let mut sum = 0i128;
        for i in 0..n {
            let r = returns.get(i).unwrap().return_bps;
            sum = sum.saturating_add(r);
        }
        let mean = sum / (n as i128);
        if mean == 0 {
            return 0;
        }

        // Calculate standard deviation
        let mut sum_sq_diff = 0i128;
        for i in 0..n {
            let r = returns.get(i).unwrap().return_bps;
            let diff = r - mean;
            sum_sq_diff = sum_sq_diff.saturating_add(diff.saturating_mul(diff));
        }
        let variance = sum_sq_diff / ((n - 1) as i128);
        let std_dev = Self::isqrt(variance);

        if std_dev == 0 {
            return 0;
        }

        // Sharpe = mean / std_dev, scaled by 10000
        mean.saturating_mul(10_000) / std_dev
    }

    // ── Volatility Calculation ───────────────────────────────────────────────

    /// Compute annualized portfolio volatility from return history.
    /// Volatility = std_dev(returns) * sqrt(periods_per_year)
    /// Returns in basis points.
    fn compute_volatility(returns: &Vec<ReturnPeriod>) -> i128 {
        let n = returns.len();
        if n < 2 {
            return 0;
        }

        let mut sum = 0i128;
        for i in 0..n {
            sum = sum.saturating_add(returns.get(i).unwrap().return_bps);
        }
        let mean = sum / (n as i128);

        let mut sum_sq_diff = 0i128;
        for i in 0..n {
            let r = returns.get(i).unwrap().return_bps;
            let diff = r - mean;
            sum_sq_diff = sum_sq_diff.saturating_add(diff.saturating_mul(diff));
        }
        let variance = sum_sq_diff / ((n - 1) as i128);
        let std_dev = Self::isqrt(variance);

        // Annualize: assume periods are daily, multiply by sqrt(365)
        let annualization_factor = Self::isqrt(365); // ~19
        std_dev.saturating_mul(annualization_factor)
    }

    // ── Max Drawdown ─────────────────────────────────────────────────────────

    /// Compute maximum drawdown from return history.
    /// Max drawdown = largest peak-to-trough decline, in basis points.
    fn compute_max_drawdown(returns: &Vec<ReturnPeriod>) -> i128 {
        let n = returns.len();
        if n == 0 {
            return 0;
        }

        // Reconstruct equity curve starting at 10_000_000 (100%)
        let mut peak = 10_000_000i128;
        let mut equity = 10_000_000i128;
        let mut max_dd = 0i128;

        for i in 0..n {
            let r = returns.get(i).unwrap().return_bps;
            // equity *= (1 + r/10000)
            equity = equity.saturating_add(equity.saturating_mul(r) / BPS_PRECISION);

            if equity > peak {
                peak = equity;
            }

            let drawdown = if peak > 0 {
                (peak - equity).saturating_mul(BPS_PRECISION) / peak
            } else {
                0
            };

            if drawdown > max_dd {
                max_dd = drawdown;
            }
        }

        max_dd
    }

    // ── Return Period Recording ──────────────────────────────────────────────

    /// Record a return period for analytics.
    /// Should be called periodically (e.g., after each trade or at time intervals).
    pub fn record_return_period(
        env: &Env,
        user: &Address,
        start_timestamp: u64,
        start_value: i128,
        end_timestamp: u64,
        end_value: i128,
    ) -> Result<(), PortfolioError> {
        let return_bps = if start_value > 0 {
            (end_value - start_value)
                .saturating_mul(BPS_PRECISION)
                .checked_div(start_value)
                .unwrap_or(0)
        } else {
            0
        };

        let period = ReturnPeriod {
            start_timestamp,
            end_timestamp,
            return_bps,
            start_value,
            end_value,
        };

        storage::save_return_period(env, user, &period);
        Ok(())
    }

    // ── Position Composition ─────────────────────────────────────────────────

    /// Get portfolio composition with allocation percentages
    pub fn get_portfolio_composition(
        env: &Env,
        user: &Address,
    ) -> Result<Vec<PositionSummary>, PortfolioError> {
        let positions = PortfolioManager::get_all_positions(env, user)?;
        let total_value = PortfolioManager::get_total_portfolio_value(env, user)?;

        let mut summaries = Vec::new(env);

        for i in 0..positions.len() {
            let pos = positions.get(i).unwrap();
            let market_value = PortfolioManager::get_market_value(env, &pos).unwrap_or(0);
            let unrealized_pnl =
                PortfolioManager::calculate_unrealized_pnl(env, &pos).unwrap_or(0);

            let allocation_pct = if total_value > 0 {
                (market_value as i128)
                    .saturating_mul(BPS_PRECISION)
                    .checked_div(total_value)
                    .unwrap_or(0) as u32
            } else {
                0
            };

            summaries.push_back(PositionSummary {
                asset: pos.asset.clone(),
                quote_asset: pos.quote_asset.clone(),
                quantity: pos.quantity,
                avg_cost_basis: pos.avg_cost_basis,
                total_cost_basis: pos.total_cost_basis,
                realized_pnl: pos.realized_pnl,
                unrealized_pnl,
                market_value,
                allocation_pct,
            });
        }

        Ok(summaries)
    }

    // ── Helper: Integer Square Root ──────────────────────────────────────────

    /// Integer square root using Newton's method for Sharpe/volatility calculations
    fn isqrt(n: i128) -> i128 {
        if n <= 0 {
            return 0;
        }
        let n = n as u128;
        if n < 2 {
            return n as i128;
        }

        let mut x = n;
        let mut y = (x + 1) / 2;
        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }
        x as i128
    }

    // ── Value at Risk (VaR) ──────────────────────────────────────────────────

    /// Compute parametric Value at Risk at a given confidence level.
    /// confidence_bps: e.g., 9500 for 95%, 9900 for 99%
    /// Returns the maximum expected loss in basis points over the period.
    pub fn compute_var(
        returns: &Vec<ReturnPeriod>,
        confidence_bps: i128,
    ) -> i128 {
        let n = returns.len();
        if n < 2 {
            return 0;
        }

        // Mean return
        let mut sum = 0i128;
        for i in 0..n {
            sum = sum.saturating_add(returns.get(i).unwrap().return_bps);
        }
        let mean = sum / (n as i128);

        // Standard deviation
        let mut sum_sq = 0i128;
        for i in 0..n {
            let diff = returns.get(i).unwrap().return_bps - mean;
            sum_sq = sum_sq.saturating_add(diff.saturating_mul(diff));
        }
        let variance = sum_sq / ((n - 1) as i128);
        let std_dev = Self::isqrt(variance);

        // Z-score approximation for confidence level (scaled by 10000)
        // 95% -> z ~ 1.645 -> 16450, 99% -> z ~ 2.326 -> 23260
        let z_score = Self::z_score_for_confidence(confidence_bps);

        // VaR = |mean - z * std_dev|
        let var = (z_score.saturating_mul(std_dev) / 10_000) - mean;
        if var > 0 {
            var
        } else {
            0
        }
    }

    /// Approximate z-score for common confidence levels (scaled by 10000)
    fn z_score_for_confidence(confidence_bps: i128) -> i128 {
        if confidence_bps >= 9900 {
            23263 // z ≈ 2.3263 for 99%
        } else if confidence_bps >= 9500 {
            16449 // z ≈ 1.6449 for 95%
        } else if confidence_bps >= 9000 {
            12816 // z ≈ 1.2816 for 90%
        } else {
            10000 // z ≈ 1.0 for anything below 90%
        }
    }
}
