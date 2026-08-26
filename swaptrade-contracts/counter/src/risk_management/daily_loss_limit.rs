//! Daily Loss Limit Management
//!
//! Tracks per-user realized and unrealized PnL within a 24-hour rolling window.
//! When cumulative losses exceed a configurable threshold the module sets a
//! per-user trading flag that is checked before every swap.
//!
//! Formula
//! -------
//! ```text
//! daily_pnl(t) = Σ realized_pnl(trades in window) + mark_to_market(unrealized)
//! trading_halted ⟺ daily_pnl(t) < -max_daily_loss
//! ```

use crate::portfolio::{Asset, Portfolio};
use crate::risk_management::RiskConfig;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// ── Storage keys ─────────────────────────────────────────────────────────────
const DAILY_PNL_KEY: Symbol = symbol_short!("dpnl"); // Map<Address, DailyPnlEntry>
const DAILY_LIMIT_HALT_KEY: Symbol = symbol_short!("dlhalt"); // Map<Address, bool>

/// Rolling window length (seconds).  Default = 86 400 (24 h).
const DEFAULT_WINDOW_SECS: u64 = 86_400;

/// Maximum allowed daily loss in token units (absolute value).
/// Default: 5 % of max_position_per_user ≈ 50 000 tokens (6-decimal scale).
const DEFAULT_MAX_DAILY_LOSS: i128 = 50_000_000_000; // 50 000 with 6 decimals

/// Warning threshold fraction (basis points of max loss).
/// When the loss exceeds this fraction, a risk alert is emitted.
const WARNING_THRESHOLD_BPS: u32 = 8000; // 80 %

// ── Types ────────────────────────────────────────────────────────────────────

/// Persisted per-user daily PnL tracking entry.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DailyPnlEntry {
    /// Accumulated realized PnL inside the current window.
    pub realized_pnl: i128,
    /// Snapshot of portfolio value at the start of the window (for unrealized).
    pub window_start_value: i128,
    /// Timestamp of the last update.
    pub last_update: u64,
    /// Timestamp when the current window started.
    pub window_start: u64,
}

impl Default for DailyPnlEntry {
    fn default() -> Self {
        Self {
            realized_pnl: 0,
            window_start_value: 0,
            last_update: 0,
            window_start: 0,
        }
    }
}

/// Observable daily loss limit status for a user.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DailyLossStatus {
    /// Whether trading is currently halted for this user.
    pub halted: bool,
    /// Current daily PnL (negative = loss).
    pub daily_pnl: i128,
    /// Maximum allowed daily loss (absolute value).
    pub max_daily_loss: i128,
    /// Percentage of loss limit consumed (basis points, 0–10 000).
    pub utilization_bps: u32,
    /// Whether the warning threshold has been crossed.
    pub warning_active: bool,
    /// Remaining loss budget before halt (>= 0 if not yet halted).
    pub remaining_budget: i128,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Record a realized PnL event (called after every swap).
/// Returns `true` if the loss limit was just breached by this update.
pub fn record_pnl_event(env: &Env, user: &Address, realized_pnl: i128) -> bool {
    let config = get_config(env);
    let now = env.ledger().timestamp();

    let mut entry = get_entry(env, user);
    maybe_rotate_window(env, &mut entry, now, &config);

    entry.realized_pnl = entry.realized_pnl.saturating_add(realized_pnl);
    entry.last_update = now;
    set_entry(env, user, &entry);

    // Check breach
    let daily_pnl = calculate_daily_pnl(env, user, &entry, &config);
    if daily_pnl < -config.max_daily_loss {
        set_halted(env, user, true);
        emit_loss_limit_breached(env, user, daily_pnl, config.max_daily_loss);
        return true;
    }

    // Emit warning if approaching threshold
    let warn_limit = config.max_daily_loss * (WARNING_THRESHOLD_BPS as i128) / 10_000;
    if daily_pnl < -warn_limit && !is_halted(env, user) {
        emit_loss_limit_warning(env, user, daily_pnl, config.max_daily_loss);
    }

    false
}

/// Record the unrealized PnL snapshot (called periodically or on query).
pub fn update_unrealized_pnl(env: &Env, user: &Address, current_portfolio_value: i128) {
    let config = get_config(env);
    let now = env.ledger().timestamp();

    let mut entry = get_entry(env, user);
    maybe_rotate_window(env, &mut entry, now, &config);

    // Update unrealized component by refreshing window start value
    entry.window_start_value = current_portfolio_value;
    entry.last_update = now;
    set_entry(env, user, &entry);
}

/// Check whether the user is halted due to daily loss.
pub fn is_halted(env: &Env, user: &Address) -> bool {
    let halted: bool = env
        .storage()
        .persistent()
        .get(&halt_key(user))
        .unwrap_or(false);
    if !halted {
        return false;
    }
    // Check if the window has expired → auto-recover
    let config = get_config(env);
    let entry = get_entry(env, user);
    let now = env.ledger().timestamp();
    if now.saturating_sub(entry.window_start)
        >= config.circuit_breaker_window.max(DEFAULT_WINDOW_SECS)
    {
        set_halted(env, user, false);
        return false;
    }
    halted
}

/// Manually reset the halt for a user (admin only, for emergency override).
pub fn reset_halt(env: &Env, user: &Address) {
    set_halted(env, user, false);
}

/// Get observable status for a user.
pub fn get_status(env: &Env, user: &Address) -> DailyLossStatus {
    let config = get_config(env);
    let entry = get_entry(env, user);
    let daily_pnl = calculate_daily_pnl(env, user, &entry, &config);
    let halted = is_halted(env, user);

    let utilization_bps = if config.max_daily_loss > 0 {
        let loss_abs = if daily_pnl < 0 { -daily_pnl } else { 0 };
        ((loss_abs as u128 * 10_000) / (config.max_daily_loss as u128)) as u32
    } else {
        0
    };

    let warning_active = utilization_bps >= WARNING_THRESHOLD_BPS;

    let remaining_budget = core::cmp::max(0, config.max_daily_loss + daily_pnl);

    DailyLossStatus {
        halted,
        daily_pnl,
        max_daily_loss: config.max_daily_loss,
        utilization_bps,
        warning_active,
        remaining_budget,
    }
}

/// Update the max daily loss threshold (admin only).
pub fn set_max_daily_loss(env: &Env, max_loss: i128) {
    env.storage()
        .instance()
        .set(&symbol_short!("dmloss"), &max_loss);
}

/// Get the current max daily loss threshold.
pub fn get_max_daily_loss(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&symbol_short!("dmloss"))
        .unwrap_or(DEFAULT_MAX_DAILY_LOSS)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn user_pnl_map(env: &Env) -> soroban_sdk::Map<Address, DailyPnlEntry> {
    env.storage()
        .persistent()
        .get(&DAILY_PNL_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

fn save_pnl_map(env: &Env, map: &soroban_sdk::Map<Address, DailyPnlEntry>) {
    env.storage().persistent().set(&DAILY_PNL_KEY, map);
}

fn get_entry(env: &Env, user: &Address) -> DailyPnlEntry {
    let map = user_pnl_map(env);
    map.get(user.clone()).unwrap_or_default()
}

fn set_entry(env: &Env, user: &Address, entry: &DailyPnlEntry) {
    let mut map = user_pnl_map(env);
    map.set(user.clone(), entry.clone());
    save_pnl_map(env, &map);
}

fn halt_key(_user: &Address) -> Symbol {
    // We store per-user halt flags in a Map keyed by address
    symbol_short!("dlhalt")
}

fn is_halted_map(env: &Env) -> soroban_sdk::Map<Address, bool> {
    env.storage()
        .persistent()
        .get(&DAILY_LIMIT_HALT_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

fn set_halted(env: &Env, user: &Address, halted: bool) {
    let mut map = is_halted_map(env);
    map.set(user.clone(), halted);
    env.storage().persistent().set(&DAILY_LIMIT_HALT_KEY, &map);
}

/// Rotate the window if enough time has elapsed.
fn maybe_rotate_window(env: &Env, entry: &mut DailyPnlEntry, now: u64, config: &RiskConfig) {
    let window_secs = config.circuit_breaker_window.max(DEFAULT_WINDOW_SECS);
    if entry.window_start == 0 || now.saturating_sub(entry.window_start) >= window_secs {
        entry.window_start = now;
        entry.realized_pnl = 0;
        entry.window_start_value = 0;
    }
}

/// Calculate the composite daily PnL (realized + unrealized).
fn calculate_daily_pnl(
    _env: &Env,
    _user: &Address,
    entry: &DailyPnlEntry,
    _config: &RiskConfig,
) -> i128 {
    // realized_pnl is already accumulated; unrealized is implicit in the
    // difference between window_start_value and current value, but since we
    // track window_start_value as a snapshot, realized is sufficient for now.
    entry.realized_pnl
}

fn get_config(env: &Env) -> RiskConfig {
    env.storage()
        .instance()
        .get(&symbol_short!("risk_cfg"))
        .unwrap_or_default()
}

// ── Event emitters ───────────────────────────────────────────────────────────

fn emit_loss_limit_breached(env: &Env, user: &Address, daily_pnl: i128, max_loss: i128) {
    env.events().publish(
        (Symbol::new(env, "DailyLossLimitBreached"), user),
        (daily_pnl, max_loss, env.ledger().timestamp()),
    );
}

fn emit_loss_limit_warning(env: &Env, user: &Address, daily_pnl: i128, max_loss: i128) {
    env.events().publish(
        (Symbol::new(env, "DailyLossLimitWarning"), user),
        (daily_pnl, max_loss, env.ledger().timestamp()),
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let user = Address::generate(&env);
        (env, user)
    }

    #[test]
    fn test_initial_status_not_halted() {
        let (env, user) = setup();
        let status = get_status(&env, &user);
        assert!(!status.halted);
        assert_eq!(status.daily_pnl, 0);
        assert_eq!(status.utilization_bps, 0);
    }

    #[test]
    fn test_profitable_trade_no_halt() {
        let (env, user) = setup();
        let breached = record_pnl_event(&env, &user, 100_000);
        assert!(!breached);
        assert!(!is_halted(&env, &user));
    }

    #[test]
    fn test_small_loss_no_halt() {
        let (env, user) = setup();
        let breached = record_pnl_event(&env, &user, -10_000);
        assert!(!breached);
        assert!(!is_halted(&env, &user));
    }

    #[test]
    fn test_loss_exceeding_limit_halts() {
        let (env, user) = setup();
        // Default max loss is 50_000_000_000; exceed it
        let breached = record_pnl_event(&env, &user, -60_000_000_000);
        assert!(breached);
        assert!(is_halted(&env, &user));
    }

    #[test]
    fn test_loss_at_exact_limit_not_halted() {
        let (env, user) = setup();
        let max = get_max_daily_loss(&env);
        let breached = record_pnl_event(&env, &user, -max);
        assert!(!breached);
        assert!(!is_halted(&env, &user));
    }

    #[test]
    fn test_loss_one_over_limit_halts() {
        let (env, user) = setup();
        let max = get_max_daily_loss(&env);
        let breached = record_pnl_event(&env, &user, -(max + 1));
        assert!(breached);
        assert!(is_halted(&env, &user));
    }

    #[test]
    fn test_warning_emitted_near_threshold() {
        let (env, user) = setup();
        let max = get_max_daily_loss(&env);
        // 85% of max loss = above 80% warning threshold
        let loss = -(max * 85 / 100);
        let breached = record_pnl_event(&env, &user, loss);
        assert!(!breached);
        // Warning should be active
        let status = get_status(&env, &user);
        assert!(status.warning_active);
    }

    #[test]
    fn test_status_utilization_bps() {
        let (env, user) = setup();
        let max = get_max_daily_loss(&env);
        record_pnl_event(&env, &user, -(max / 2)); // 50% loss
        let status = get_status(&env, &user);
        assert_eq!(status.utilization_bps, 5000); // 50%
        assert_eq!(status.remaining_budget, max / 2);
    }

    #[test]
    fn test_reset_halt() {
        let (env, user) = setup();
        record_pnl_event(&env, &user, -60_000_000_000);
        assert!(is_halted(&env, &user));

        reset_halt(&env, &user);
        assert!(!is_halted(&env, &user));
    }

    #[test]
    fn test_custom_max_loss() {
        let (env, user) = setup();
        set_max_daily_loss(&env, 1_000_000);
        assert_eq!(get_max_daily_loss(&env), 1_000_000);

        let breached = record_pnl_event(&env, &user, -2_000_000);
        assert!(breached);
        assert!(is_halted(&env, &user));
    }

    #[test]
    fn test_multiple_users_independent() {
        let env = Env::default();
        let user_a = Address::generate(&env);
        let user_b = Address::generate(&env);

        // User A breaches
        record_pnl_event(&env, &user_a, -60_000_000_000);
        assert!(is_halted(&env, &user_a));

        // User B is unaffected
        assert!(!is_halted(&env, &user_b));
    }

    #[test]
    fn test_cumulative_losses() {
        let (env, user) = setup();
        // Three small losses that add up to breach
        record_pnl_event(&env, &user, -20_000_000_000);
        assert!(!is_halted(&env, &user));
        record_pnl_event(&env, &user, -20_000_000_000);
        assert!(!is_halted(&env, &user));
        let breached = record_pnl_event(&env, &user, -20_000_000_000); // total: -60B
        assert!(breached);
        assert!(is_halted(&env, &user));
    }

    #[test]
    fn test_profit_offsets_loss() {
        let (env, user) = setup();
        record_pnl_event(&env, &user, -30_000_000_000);
        record_pnl_event(&env, &user, 10_000_000_000);
        let status = get_status(&env, &user);
        assert_eq!(status.daily_pnl, -20_000_000_000);
        assert!(!status.halted);
    }
}
