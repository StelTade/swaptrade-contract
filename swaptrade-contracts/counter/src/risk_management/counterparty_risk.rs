//! Counterparty Risk Monitoring
//!
//! Tracks exposure to specific trade counterparties to prevent excessive
//! concentration in any single counterparty. Maintains a rolling window
//! of trade volumes per counterparty and enforces configurable limits.
//!
//! Exposure Model
//! --------------
//! ```text
//! counterparty_exposure(cp) = Σ |trade_value| in window
//! limit_breached ⟺ counterparty_exposure(cp) > max_counterparty_exposure
//! ```

use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

// ── Storage keys ─────────────────────────────────────────────────────────────
const CP_EXPOSURE_KEY: Symbol = symbol_short!("cpexp"); // Map<Address, Map<Address, CpEntry>>
const CP_CONFIG_KEY: Symbol = symbol_short!("cpconf");

/// Default max exposure to a single counterparty.
const DEFAULT_MAX_CP_EXPOSURE: i128 = 500_000_000_000; // 500K with 6 decimals

/// Default time window (seconds) for rolling exposure.
const DEFAULT_CP_WINDOW_SECS: u64 = 86_400; // 24 hours

/// Warning threshold (basis points of max exposure).
const CP_WARNING_BPS: u32 = 7_500; // 75%

// ── Types ────────────────────────────────────────────────────────────────────

/// Configuration for counterparty risk limits.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CounterpartyConfig {
    /// Maximum exposure to a single counterparty.
    pub max_counterparty_exposure: i128,
    /// Rolling window in seconds.
    pub window_secs: u64,
    /// Maximum number of counterparties tracked per user.
    pub max_counterparties: u32,
}

impl Default for CounterpartyConfig {
    fn default() -> Self {
        Self {
            max_counterparty_exposure: DEFAULT_MAX_CP_EXPOSURE,
            window_secs: DEFAULT_CP_WINDOW_SECS,
            max_counterparties: 50,
        }
    }
}

/// Per-counterparty tracking entry.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CpEntry {
    /// Rolling exposure total (absolute trade values).
    pub exposure: i128,
    /// Number of trades with this counterparty.
    pub trade_count: u64,
    /// Timestamp of last trade.
    pub last_trade_at: u64,
    /// Timestamp of window start.
    pub window_start: u64,
    /// Whether a warning was emitted.
    pub warning_emitted: bool,
}

impl Default for CpEntry {
    fn default() -> Self {
        Self {
            exposure: 0,
            trade_count: 0,
            last_trade_at: 0,
            window_start: 0,
            warning_emitted: false,
        }
    }
}

/// Observable counterparty exposure status.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CounterpartyExposureStatus {
    /// Counterparty address.
    pub counterparty: Address,
    /// Current exposure amount.
    pub exposure: i128,
    /// Maximum allowed exposure.
    pub max_exposure: i128,
    /// Exposure as percentage (basis points).
    pub utilization_bps: u32,
    /// Number of trades in window.
    pub trade_count: u64,
    /// Whether limit is breached.
    pub limit_breached: bool,
    /// Whether warning is active.
    pub warning_active: bool,
}

/// Summary of all counterparty exposures for a user.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CounterpartySummary {
    /// Total number of counterparties.
    pub total_counterparties: u32,
    /// Number of counterparties with warnings.
    pub warned_counterparties: u32,
    /// Number of counterparties breaching limits.
    pub breached_counterparties: u32,
    /// Total exposure across all counterparties.
    pub total_exposure: i128,
    /// Largest single counterparty exposure.
    pub max_single_exposure: i128,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Record a trade with a counterparty. Returns `true` if the exposure limit
/// was just breached.
pub fn record_trade(env: &Env, user: &Address, counterparty: &Address, trade_value: i128) -> bool {
    let config = get_config(env);
    let now = env.ledger().timestamp();

    let mut user_map = get_user_cp_map(env, user);
    let mut entry = user_map.get(counterparty.clone()).unwrap_or_default();

    // Rotate window if expired
    if entry.window_start == 0 || now.saturating_sub(entry.window_start) >= config.window_secs {
        entry.window_start = now;
        entry.exposure = 0;
        entry.trade_count = 0;
        entry.warning_emitted = false;
    }

    let abs_value = if trade_value < 0 {
        -trade_value
    } else {
        trade_value
    };
    entry.exposure = entry.exposure.saturating_add(abs_value);
    entry.trade_count = entry.trade_count.saturating_add(1);
    entry.last_trade_at = now;

    let breached = entry.exposure > config.max_counterparty_exposure;

    // Emit warning if approaching threshold
    let warn_limit = config.max_counterparty_exposure * (CP_WARNING_BPS as i128) / 10_000;
    if entry.exposure >= warn_limit && !entry.warning_emitted {
        entry.warning_emitted = true;
        emit_counterparty_warning(
            env,
            user,
            counterparty,
            entry.exposure,
            config.max_counterparty_exposure,
        );
    }

    if breached {
        emit_counterparty_limit_breached(
            env,
            user,
            counterparty,
            entry.exposure,
            config.max_counterparty_exposure,
        );
    }

    user_map.set(counterparty.clone(), entry);
    save_user_cp_map(env, user, &user_map);

    breached
}

/// Check if a trade with a counterparty would exceed limits.
pub fn check_exposure(
    env: &Env,
    user: &Address,
    counterparty: &Address,
    proposed_value: i128,
) -> bool {
    let config = get_config(env);
    let entry = get_cp_entry(env, user, counterparty);

    let abs_proposed = if proposed_value < 0 {
        -proposed_value
    } else {
        proposed_value
    };
    let new_exposure = entry.exposure.saturating_add(abs_proposed);

    new_exposure > config.max_counterparty_exposure
}

/// Get observable exposure status for a specific counterparty.
pub fn get_exposure_status(
    env: &Env,
    user: &Address,
    counterparty: &Address,
) -> CounterpartyExposureStatus {
    let config = get_config(env);
    let entry = get_cp_entry(env, user, counterparty);

    let utilization_bps = if config.max_counterparty_exposure > 0 {
        ((entry.exposure as u128) * 10_000 / (config.max_counterparty_exposure as u128)) as u32
    } else {
        0
    };

    let warn_limit = config.max_counterparty_exposure * (CP_WARNING_BPS as i128) / 10_000;

    CounterpartyExposureStatus {
        counterparty: counterparty.clone(),
        exposure: entry.exposure,
        max_exposure: config.max_counterparty_exposure,
        utilization_bps,
        trade_count: entry.trade_count,
        limit_breached: entry.exposure > config.max_counterparty_exposure,
        warning_active: entry.exposure >= warn_limit,
    }
}

/// Get summary of all counterparty exposures for a user.
pub fn get_user_summary(env: &Env, user: &Address) -> CounterpartySummary {
    let config = get_config(env);
    let user_map = get_user_cp_map(env, user);

    let mut total_counterparties = 0u32;
    let mut warned = 0u32;
    let mut breached = 0u32;
    let mut total_exposure: i128 = 0;
    let mut max_single: i128 = 0;

    let warn_limit = config.max_counterparty_exposure * (CP_WARNING_BPS as i128) / 10_000;

    // Iterate through the map
    let cp_list: Vec<Address> = user_map.keys();
    for i in 0..cp_list.len() {
        if let Some(cp) = cp_list.get(i) {
            if let Some(entry) = user_map.get(cp.clone()) {
                // Only count entries within the active window
                let now = env.ledger().timestamp();
                if entry.window_start > 0
                    && now.saturating_sub(entry.window_start) < config.window_secs
                {
                    total_counterparties = total_counterparties.saturating_add(1);
                    total_exposure = total_exposure.saturating_add(entry.exposure);
                    if entry.exposure > max_single {
                        max_single = entry.exposure;
                    }
                    if entry.exposure > config.max_counterparty_exposure {
                        breached = breached.saturating_add(1);
                    } else if entry.exposure >= warn_limit {
                        warned = warned.saturating_add(1);
                    }
                }
            }
        }
    }

    CounterpartySummary {
        total_counterparties,
        warned_counterparties: warned,
        breached_counterparties: breached,
        total_exposure,
        max_single_exposure: max_single,
    }
}

/// Update counterparty risk configuration (admin only).
pub fn set_config(env: &Env, config: &CounterpartyConfig) {
    env.storage().instance().set(&CP_CONFIG_KEY, config);
}

/// Get counterparty risk configuration.
pub fn get_config_stored(env: &Env) -> CounterpartyConfig {
    get_config(env)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn get_config(env: &Env) -> CounterpartyConfig {
    env.storage()
        .instance()
        .get(&CP_CONFIG_KEY)
        .unwrap_or_default()
}

fn user_cp_map_key(user: &Address) -> Symbol {
    // Derive a unique symbol per user from address bytes
    // Use a simple hash-like approach with the address
    Symbol::short("cpx")
}

fn get_user_cp_map(env: &Env, user: &Address) -> Map<Address, CpEntry> {
    let key = Symbol::short("cpx");
    let outer: Map<Address, Map<Address, CpEntry>> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Map::new(env));
    outer.get(user.clone()).unwrap_or_else(|| Map::new(env))
}

fn save_user_cp_map(env: &Env, user: &Address, inner: &Map<Address, CpEntry>) {
    let key = Symbol::short("cpx");
    let mut outer: Map<Address, Map<Address, CpEntry>> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Map::new(env));
    outer.set(user.clone(), inner.clone());
    env.storage().persistent().set(&key, &outer);
}

fn get_cp_entry(env: &Env, user: &Address, counterparty: &Address) -> CpEntry {
    let map = get_user_cp_map(env, user);
    map.get(counterparty.clone()).unwrap_or_default()
}

// ── Event emitters ───────────────────────────────────────────────────────────

fn emit_counterparty_warning(
    env: &Env,
    user: &Address,
    counterparty: &Address,
    exposure: i128,
    max_exposure: i128,
) {
    env.events().publish(
        (Symbol::new(env, "CounterpartyWarning"), user, counterparty),
        (exposure, max_exposure, env.ledger().timestamp()),
    );
}

fn emit_counterparty_limit_breached(
    env: &Env,
    user: &Address,
    counterparty: &Address,
    exposure: i128,
    max_exposure: i128,
) {
    env.events().publish(
        (
            Symbol::new(env, "CounterpartyLimitBreached"),
            user,
            counterparty,
        ),
        (exposure, max_exposure, env.ledger().timestamp()),
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        let user = Address::generate(&env);
        let cp = Address::generate(&env);
        (env, user, cp)
    }

    #[test]
    fn test_record_trade_no_breach() {
        let (env, user, cp) = setup();
        let breached = record_trade(&env, &user, &cp, 100_000);
        assert!(!breached);
    }

    #[test]
    fn test_record_trade_breach() {
        let (env, user, cp) = setup();
        // Default max is 500B; exceed it
        let breached = record_trade(&env, &user, &cp, 600_000_000_000);
        assert!(breached);
    }

    #[test]
    fn test_cumulative_exposure() {
        let (env, user, cp) = setup();
        record_trade(&env, &user, &cp, 300_000_000_000);
        assert!(!record_trade(&env, &user, &cp, 300_000_000_000)); // total 600B > 500B → breach

        let status = get_exposure_status(&env, &user, &cp);
        assert!(status.limit_breached);
        assert_eq!(status.trade_count, 2);
    }

    #[test]
    fn test_check_exposure_within_limit() {
        let (env, user, cp) = setup();
        assert!(!check_exposure(&env, &user, &cp, 100_000));
    }

    #[test]
    fn test_check_exposure_exceeds_limit() {
        let (env, user, cp) = setup();
        assert!(check_exposure(&env, &user, &cp, 600_000_000_000));
    }

    #[test]
    fn test_exposure_status_initial() {
        let (env, user, cp) = setup();
        let status = get_exposure_status(&env, &user, &cp);
        assert_eq!(status.exposure, 0);
        assert!(!status.limit_breached);
        assert!(!status.warning_active);
    }

    #[test]
    fn test_user_summary() {
        let (env, user, cp) = setup();
        record_trade(&env, &user, &cp, 100_000);

        let summary = get_user_summary(&env, &user);
        assert_eq!(summary.total_counterparties, 1);
        assert_eq!(summary.total_exposure, 100_000);
    }

    #[test]
    fn test_custom_config() {
        let (env, _user, _cp) = setup();
        let config = CounterpartyConfig {
            max_counterparty_exposure: 1_000,
            window_secs: 3600,
            max_counterparties: 10,
        };
        set_config(&env, &config);
        assert_eq!(get_config_stored(&env).max_counterparty_exposure, 1_000);
    }

    #[test]
    fn test_multiple_counterparties() {
        let (env, user, _cp) = setup();
        let cp2 = Address::generate(&env);

        record_trade(&env, &user, &_cp, 100_000);
        record_trade(&env, &user, &cp2, 200_000);

        let summary = get_user_summary(&env, &user);
        assert_eq!(summary.total_counterparties, 2);
        assert_eq!(summary.total_exposure, 300_000);
    }
}
