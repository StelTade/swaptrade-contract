#![cfg_attr(all(not(test), target_family = "wasm"), no_std)]

extern crate alloc;

pub mod analytics;
pub mod errors;
pub mod events;
pub mod portfolio;
pub mod storage;
pub mod types;

pub use errors::PortfolioError;
pub use types::*;

use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

use analytics::AnalyticsEngine;
use portfolio::PortfolioManager;

#[contract]
pub struct PortfolioAnalyticsContract;

#[contractimpl]
impl PortfolioAnalyticsContract {
    // ── Initialization ──────────────────────────────────────────────────────

    /// Initialize the Portfolio Analytics contract
    pub fn initialize(env: Env, admin: Address) -> Result<(), PortfolioError> {
        admin.require_auth();
        if env.storage().persistent().has(&storage::StorageKey::Admin) {
            return Err(PortfolioError::AlreadyExists);
        }
        env.storage()
            .persistent()
            .set(&storage::StorageKey::Admin, &admin);
        Ok(())
    }

    // ── Position Tracking ───────────────────────────────────────────────────

    /// Record a buy/inflow that opens or adds to a position
    pub fn record_buy(
        env: Env,
        user: Address,
        asset: Address,
        quote_asset: Address,
        quantity: i128,
        price: i128,
    ) -> Result<TransactionRecord, PortfolioError> {
        user.require_auth();
        PortfolioManager::record_buy(&env, &user, &asset, &quote_asset, quantity, price)
    }

    /// Record a sell/outflow that reduces or closes a position
    pub fn record_sell(
        env: Env,
        user: Address,
        asset: Address,
        quantity: i128,
        price: i128,
    ) -> Result<TransactionRecord, PortfolioError> {
        user.require_auth();
        PortfolioManager::record_sell(&env, &user, &asset, quantity, price)
    }

    // ── Position Queries ────────────────────────────────────────────────────

    /// Get a specific position for a user and asset
    pub fn get_position(
        env: Env,
        user: Address,
        asset: Address,
    ) -> Option<Position> {
        storage::get_position(&env, &user, &asset)
    }

    /// Get all positions for a user
    pub fn get_all_positions(env: Env, user: Address) -> Vec<Position> {
        PortfolioManager::get_all_positions(&env, &user).unwrap_or_else(|_| Vec::new(&env))
    }

    /// Get all asset addresses a user holds
    pub fn get_user_assets(env: Env, user: Address) -> Vec<Address> {
        storage::get_user_assets(&env, &user)
    }

    // ── P&L Calculations ────────────────────────────────────────────────────

    /// Calculate unrealized P&L for a specific position
    pub fn get_unrealized_pnl(
        env: Env,
        user: Address,
        asset: Address,
    ) -> Result<i128, PortfolioError> {
        let position =
            storage::get_position(&env, &user, &asset).ok_or(PortfolioError::PositionNotFound)?;
        PortfolioManager::calculate_unrealized_pnl(&env, &position)
    }

    /// Get total unrealized P&L across all positions
    pub fn get_total_unrealized_pnl(
        env: Env,
        user: Address,
    ) -> Result<i128, PortfolioError> {
        PortfolioManager::get_total_unrealized_pnl(&env, &user)
    }

    /// Get total realized P&L across all positions
    pub fn get_total_realized_pnl(
        env: Env,
        user: Address,
    ) -> Result<i128, PortfolioError> {
        PortfolioManager::get_total_realized_pnl(&env, &user)
    }

    /// Get total portfolio market value
    pub fn get_total_portfolio_value(
        env: Env,
        user: Address,
    ) -> Result<i128, PortfolioError> {
        PortfolioManager::get_total_portfolio_value(&env, &user)
    }

    /// Get total invested across all positions
    pub fn get_total_invested(
        env: Env,
        user: Address,
    ) -> Result<i128, PortfolioError> {
        PortfolioManager::get_total_invested(&env, &user)
    }

    /// Get total returned from all sells
    pub fn get_total_divested(
        env: Env,
        user: Address,
    ) -> Result<i128, PortfolioError> {
        PortfolioManager::get_total_divested(&env, &user)
    }

    // ── Price Oracle ─────────────────────────────────────────────────────────

    /// Update market price for an asset (oracle/admin)
    pub fn update_price(
        env: Env,
        asset: Address,
        price: u128,
    ) -> Result<(), PortfolioError> {
        PortfolioManager::update_price(&env, &asset, price)
    }

    /// Get current market price for an asset
    pub fn get_price(env: Env, asset: Address) -> Result<AssetPrice, PortfolioError> {
        PortfolioManager::get_price(&env, &asset)
    }

    // ── Portfolio Snapshots ──────────────────────────────────────────────────

    /// Capture a point-in-time portfolio snapshot
    pub fn capture_snapshot(
        env: Env,
        user: Address,
    ) -> Result<PortfolioSnapshot, PortfolioError> {
        user.require_auth();
        AnalyticsEngine::capture_snapshot(&env, &user)
    }

    /// Get a previously captured snapshot by ID
    pub fn get_snapshot(
        env: Env,
        snapshot_id: u64,
    ) -> Result<PortfolioSnapshot, PortfolioError> {
        AnalyticsEngine::get_snapshot(&env, snapshot_id)
    }

    /// Get all snapshot IDs for a user
    pub fn get_user_snapshots(env: Env, user: Address) -> Vec<u64> {
        AnalyticsEngine::get_user_snapshot_ids(&env, &user)
    }

    // ── Performance Analytics ────────────────────────────────────────────────

    /// Compute comprehensive performance metrics
    pub fn compute_metrics(
        env: Env,
        user: Address,
    ) -> Result<PerformanceMetrics, PortfolioError> {
        AnalyticsEngine::compute_metrics(&env, &user)
    }

    /// Get portfolio composition with allocation percentages
    pub fn get_portfolio_composition(
        env: Env,
        user: Address,
    ) -> Result<Vec<PositionSummary>, PortfolioError> {
        AnalyticsEngine::get_portfolio_composition(&env, &user)
    }

    /// Record a return period for analytics calculations
    pub fn record_return_period(
        env: Env,
        user: Address,
        start_timestamp: u64,
        start_value: i128,
        end_timestamp: u64,
        end_value: i128,
    ) -> Result<(), PortfolioError> {
        user.require_auth();
        AnalyticsEngine::record_return_period(
            &env,
            &user,
            start_timestamp,
            start_value,
            end_timestamp,
            end_value,
        )
    }

    /// Compute parametric Value at Risk (VaR) at given confidence level
    pub fn compute_var(
        env: Env,
        user: Address,
        confidence_bps: i128,
    ) -> i128 {
        let return_history = storage::get_return_history(&env, &user);
        AnalyticsEngine::compute_var(&return_history, confidence_bps)
    }

    // ── Transaction History ──────────────────────────────────────────────────

    /// Get transaction records for a user (paginated)
    pub fn get_transaction_history(
        env: Env,
        user: Address,
        offset: u32,
        max_count: u32,
    ) -> Vec<TransactionRecord> {
        storage::get_user_transactions(&env, &user, offset, max_count)
    }

    /// Get a specific transaction record by ID
    pub fn get_transaction(
        env: Env,
        tx_id: u64,
    ) -> Option<TransactionRecord> {
        storage::get_transaction(&env, tx_id)
    }
}
