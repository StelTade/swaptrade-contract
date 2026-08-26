//! Leverage Controls
//!
//! Enforces configurable maximum leverage per user tier and triggers automatic
//! deleveraging when positions exceed safe leverage ratios.
//!
//! Leverage Model
//! --------------
//! ```text
//! leverage = total_exposure / margin
//!
//! margin = Σ balance(asset_i) * maintenance_margin_ratio(asset_i)
//!
//! deleverage_trigger ⟺ leverage > max_leverage(user_tier)
//! ```

use crate::portfolio::{Asset, Portfolio};
use crate::risk_management::RiskConfig;
use crate::tiers::UserTier;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// ── Storage keys ─────────────────────────────────────────────────────────────
const LEVERAGE_STATE_KEY: Symbol = symbol_short!("levst");

/// Default maximum leverage (in multiples × 1000 for integer math).
/// 10× = 10_000 basis-multiples.
const DEFAULT_MAX_LEVERAGE_X1000: u32 = 10_000;

/// Maintenance margin ratio per asset (basis points of asset value).
/// Default: 50 % (i.e., 5 000 bps).
const DEFAULT_MAINTENANCE_MARGIN_BPS: u32 = 5_000;

/// Deleveraging safety buffer – trigger at 90 % of max leverage.
const DELEVERAGE_TRIGGER_BPS: u32 = 9_000;

// ── Types ────────────────────────────────────────────────────────────────────

/// Per-user leverage tracking state.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LeverageState {
    /// Current leverage × 1000 (e.g., 2500 = 2.5×).
    pub current_leverage_x1000: u32,
    /// Maximum leverage allowed for this user's tier × 1000.
    pub max_leverage_x1000: u32,
    /// Total notional exposure.
    pub total_exposure: i128,
    /// Total margin (collateral).
    pub total_margin: i128,
    /// Whether deleveraging is in progress.
    pub deleveraging: bool,
    /// Timestamp of last leverage check.
    pub last_check: u64,
}

impl Default for LeverageState {
    fn default() -> Self {
        Self {
            current_leverage_x1000: 0,
            max_leverage_x1000: DEFAULT_MAX_LEVERAGE_X1000,
            total_exposure: 0,
            total_margin: 0,
            deleveraging: false,
            last_check: 0,
        }
    }
}

/// Observable leverage status for queries.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LeverageStatus {
    /// Current leverage (×1000 scale, e.g., 2500 = 2.5×).
    pub current_leverage_x1000: u32,
    /// Max leverage for tier (×1000).
    pub max_leverage_x1000: u32,
    /// Whether the user is within safe leverage.
    pub within_limits: bool,
    /// Whether deleveraging should be triggered.
    pub should_deleverage: bool,
    /// Total exposure value.
    pub total_exposure: i128,
    /// Total margin.
    pub total_margin: i128,
    /// Margin utilization (basis points of max).
    pub margin_utilization_bps: u32,
}

/// Result of a leverage check that may require action.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LeverageCheckResult {
    /// Whether leverage is within limits.
    pub within_limits: bool,
    /// Amount to deleverage (0 if none needed).
    pub deleverage_amount: i128,
    /// Asset to deleverage from.
    pub deleverage_asset: Asset,
    /// Current leverage.
    pub current_leverage_x1000: u32,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Get the maximum leverage (×1000) for a given user tier.
pub fn get_tier_max_leverage(tier: &UserTier) -> u32 {
    match tier {
        UserTier::Novice => 2_000,  // 2×
        UserTier::Trader => 5_000,  // 5×
        UserTier::Expert => 10_000, // 10×
        UserTier::Whale => 20_000,  // 20×
    }
}

/// Calculate the maintenance margin for a given balance.
/// margin = balance × maintenance_margin_bps / 10_000
pub fn calculate_maintenance_margin(balance: i128, maintenance_margin_bps: u32) -> i128 {
    if balance <= 0 {
        return 0;
    }
    ((balance as u128) * (maintenance_margin_bps as u128) / 10_000) as i128
}

/// Calculate current leverage for a user.
/// leverage_x1000 = (total_exposure × 10_000) / total_margin
pub fn calculate_leverage_x1000(total_exposure: i128, total_margin: i128) -> u32 {
    if total_margin <= 0 {
        return 0;
    }
    let exposure = if total_exposure < 0 {
        -total_exposure
    } else {
        total_exposure
    };
    let margin = if total_margin < 0 {
        -total_margin
    } else {
        total_margin
    };

    // lever × 1000 = (exposure × 10_000 × 1000) / margin = exposure × 10_000_000 / margin
    let lev = ((exposure as u128) * 10_000_000) / (margin as u128);
    if lev > u32::MAX as u128 {
        u32::MAX
    } else {
        lev as u32
    }
}

/// Check leverage limits and return a result indicating whether deleveraging
/// is required.
pub fn check_leverage(env: &Env, portfolio: &Portfolio, user: &Address) -> LeverageCheckResult {
    let tier = portfolio.get_user_tier(env, user.clone());
    let max_leverage_x1000 = get_tier_max_leverage(&tier);
    let maint_bps = get_maintenance_margin_bps(env);

    let (exposure, margin) = calculate_exposure_and_margin(env, portfolio, user, maint_bps);
    let current_leverage_x1000 = calculate_leverage_x1000(exposure, margin);

    let within_limits = current_leverage_x1000 <= max_leverage_x1000;
    let should_deleverage =
        current_leverage_x1000 > (max_leverage_x1000 * DELEVERAGE_TRIGGER_BPS / 10_000);

    let deleverage_amount = if should_deleverage && exposure > 0 {
        // Calculate how much to deleverage to get back to trigger level
        let target_exposure = (margin as u128) * (max_leverage_x1000 as u128) / 10_000;
        let excess = (exposure as u128).saturating_sub(target_exposure);
        excess as i128
    } else {
        0
    };

    // Determine which asset to deleverage (largest position)
    let deleverage_asset = get_largest_position_asset(env, portfolio, user);

    // Update state
    let state = LeverageState {
        current_leverage_x1000,
        max_leverage_x1000,
        total_exposure: exposure,
        total_margin: margin,
        deleveraging: should_deleverage,
        last_check: env.ledger().timestamp(),
    };
    save_state(env, user, &state);

    LeverageCheckResult {
        within_limits,
        deleverage_amount,
        deleverage_asset,
        current_leverage_x1000,
    }
}

/// Get observable leverage status for a user.
pub fn get_status(env: &Env, user: &Address) -> LeverageStatus {
    let state = get_state(env, user);
    let within_limits = state.current_leverage_x1000 <= state.max_leverage_x1000;
    let should_deleverage =
        state.current_leverage_x1000 > (state.max_leverage_x1000 * DELEVERAGE_TRIGGER_BPS / 10_000);

    let margin_utilization_bps = if state.max_leverage_x1000 > 0 {
        ((state.current_leverage_x1000 as u64 * 10_000) / (state.max_leverage_x1000 as u64)) as u32
    } else {
        0
    };

    LeverageStatus {
        current_leverage_x1000: state.current_leverage_x1000,
        max_leverage_x1000: state.max_leverage_x1000,
        within_limits,
        should_deleverage,
        total_exposure: state.total_exposure,
        total_margin: state.total_margin,
        margin_utilization_bps,
    }
}

/// Mark that deleveraging has been executed (called after forced sell).
pub fn mark_deleveraged(env: &Env, user: &Address) {
    let mut state = get_state(env, user);
    state.deleveraging = false;
    save_state(env, user, &state);
}

/// Get the maintenance margin basis points from config / storage.
pub fn get_maintenance_margin_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&symbol_short!("mmargin"))
        .unwrap_or(DEFAULT_MAINTENANCE_MARGIN_BPS)
}

/// Set the maintenance margin basis points (admin only).
pub fn set_maintenance_margin_bps(env: &Env, bps: u32) {
    env.storage()
        .instance()
        .set(&symbol_short!("mmargin"), &bps);
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn calculate_exposure_and_margin(
    env: &Env,
    portfolio: &Portfolio,
    user: &Address,
    maint_bps: u32,
) -> (i128, i128) {
    let xlm_balance = portfolio.balance_of(env, Asset::XLM, user.clone());
    let usdc_balance =
        portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());

    let exposure = xlm_balance + usdc_balance;
    let margin = calculate_maintenance_margin(xlm_balance, maint_bps)
        + calculate_maintenance_margin(usdc_balance, maint_bps);

    (exposure, margin)
}

fn get_largest_position_asset(env: &Env, portfolio: &Portfolio, user: &Address) -> Asset {
    let xlm = portfolio.balance_of(env, Asset::XLM, user.clone());
    let usdc = portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());
    if xlm >= usdc {
        Asset::XLM
    } else {
        Asset::Custom(Symbol::short("USDCSIM"))
    }
}

fn get_state(env: &Env, user: &Address) -> LeverageState {
    let map: soroban_sdk::Map<Address, LeverageState> = env
        .storage()
        .persistent()
        .get(&LEVERAGE_STATE_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env));
    map.get(user.clone()).unwrap_or_default()
}

fn save_state(env: &Env, user: &Address, state: &LeverageState) {
    let mut map: soroban_sdk::Map<Address, LeverageState> = env
        .storage()
        .persistent()
        .get(&LEVERAGE_STATE_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env));
    map.set(user.clone(), state.clone());
    env.storage().persistent().set(&LEVERAGE_STATE_KEY, &map);
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
    fn test_tier_max_leverage() {
        assert_eq!(get_tier_max_leverage(&UserTier::Novice), 2_000); // 2×
        assert_eq!(get_tier_max_leverage(&UserTier::Trader), 5_000); // 5×
        assert_eq!(get_tier_max_leverage(&UserTier::Expert), 10_000); // 10×
        assert_eq!(get_tier_max_leverage(&UserTier::Whale), 20_000); // 20×
    }

    #[test]
    fn test_maintenance_margin_calculation() {
        // 1000 balance × 50% = 500 margin
        let margin = calculate_maintenance_margin(1000, 5000);
        assert_eq!(margin, 500);
    }

    #[test]
    fn test_maintenance_margin_zero_balance() {
        assert_eq!(calculate_maintenance_margin(0, 5000), 0);
    }

    #[test]
    fn test_leverage_calculation() {
        // exposure=1000, margin=500 → leverage = 2×
        let lev = calculate_leverage_x1000(1000, 500);
        assert_eq!(lev, 2000); // 2.0×
    }

    #[test]
    fn test_leverage_calculation_high() {
        // exposure=5000, margin=500 → leverage = 10×
        let lev = calculate_leverage_x1000(5000, 500);
        assert_eq!(lev, 10_000); // 10.0×
    }

    #[test]
    fn test_leverage_calculation_zero_margin() {
        assert_eq!(calculate_leverage_x1000(1000, 0), 0);
    }

    #[test]
    fn test_status_initial() {
        let (env, user) = setup();
        let status = get_status(&env, &user);
        assert_eq!(status.current_leverage_x1000, 0);
        assert!(status.within_limits);
        assert!(!status.should_deleverage);
    }

    #[test]
    fn test_check_leverage_with_portfolio() {
        let (env, user) = setup();
        let mut portfolio = Portfolio::new(&env);
        portfolio.credit(&env, Asset::XLM, user.clone(), 1000);
        portfolio.credit(
            &env,
            Asset::Custom(Symbol::short("USDCSIM")),
            user.clone(),
            1000,
        );

        let result = check_leverage(&env, &portfolio, &user);
        assert!(result.within_limits);
        assert_eq!(result.deleverage_amount, 0);
    }

    #[test]
    fn test_mark_deleveraged() {
        let (env, user) = setup();
        let mut state = get_state(&env, &user);
        state.deleveraging = true;
        save_state(&env, &user, &state);

        mark_deleveraged(&env, &user);
        let updated = get_state(&env, &user);
        assert!(!updated.deleveraging);
    }

    #[test]
    fn test_custom_maintenance_margin() {
        let (env, _user) = setup();
        set_maintenance_margin_bps(&env, 2500); // 25%
        assert_eq!(get_maintenance_margin_bps(&env), 2500);
    }
}
