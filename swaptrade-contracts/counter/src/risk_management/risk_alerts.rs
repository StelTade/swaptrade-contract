//! Risk Alerts – Pre-Breach Warning System
//!
//! Monitors risk thresholds and emits alerts before limits are breached,
//! giving users time to adjust positions. Alerts are tiered:
//!
//! - **Info** (≥ 50% utilization): informational only
//! - **Warning** (≥ 75% utilization): on-chain event emitted
//! - **Critical** (≥ 90% utilization): on-chain event + trading restriction hint
//!
//! All alerts are persisted so that front-ends can query active warnings.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

// ── Storage keys ─────────────────────────────────────────────────────────────
const ALERTS_KEY: Symbol = symbol_short!("ralrt");
const ALERTS_CONFIG_KEY: Symbol = symbol_short!("ralcfg");

// ── Constants ────────────────────────────────────────────────────────────────

const INFO_THRESHOLD_BPS: u32 = 5_000; // 50%
const WARNING_THRESHOLD_BPS: u32 = 7_500; // 75%
const CRITICAL_THRESHOLD_BPS: u32 = 9_000; // 90%

// ── Types ────────────────────────────────────────────────────────────────────

/// Severity level for a risk alert.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// A single risk alert record.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RiskAlert {
    /// Unique alert ID.
    pub alert_id: u64,
    /// User this alert pertains to.
    pub user: Address,
    /// Category of the alert (e.g., "position", "concentration", "leverage").
    pub category: Symbol,
    /// Current utilization (basis points).
    pub utilization_bps: u32,
    /// Threshold that was crossed (basis points).
    pub threshold_bps: u32,
    /// Severity.
    pub severity: AlertSeverity,
    /// Timestamp when the alert was created.
    pub created_at: u64,
    /// Whether the alert has been acknowledged.
    pub acknowledged: bool,
    /// Human-readable description tag.
    pub description: Symbol,
}

/// Observable risk alert status for a user.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AlertStatus {
    /// Total active (unacknowledged) alerts.
    pub active_alerts: u32,
    /// Number of info alerts.
    pub info_count: u32,
    /// Number of warning alerts.
    pub warning_count: u32,
    /// Number of critical alerts.
    pub critical_count: u32,
    /// Most severe active alert.
    pub max_severity: AlertSeverity,
}

/// Configuration for alert thresholds.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AlertConfig {
    /// Info threshold (basis points).
    pub info_threshold_bps: u32,
    /// Warning threshold (basis points).
    pub warning_threshold_bps: u32,
    /// Critical threshold (basis points).
    pub critical_threshold_bps: u32,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            info_threshold_bps: INFO_THRESHOLD_BPS,
            warning_threshold_bps: WARNING_THRESHOLD_BPS,
            critical_threshold_bps: CRITICAL_THRESHOLD_BPS,
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Evaluate a risk metric and emit an alert if thresholds are crossed.
///
/// `category` is a Symbol identifying the risk type (e.g., "position", "conc",
/// "leverage", "dloss"). `current_bps` is the current utilization in basis
/// points. `threshold_bps` is the hard limit in basis points.
///
/// Returns the `AlertSeverity` of the emitted alert, or `None` if below all
/// thresholds.
pub fn evaluate_and_alert(
    env: &Env,
    user: &Address,
    category: Symbol,
    current_bps: u32,
    threshold_bps: u32,
) -> Option<AlertSeverity> {
    let config = get_alert_config(env);

    // Normalize current_bps relative to the hard threshold
    let utilization = if threshold_bps > 0 {
        (current_bps as u64 * 10_000 / threshold_bps as u64) as u32
    } else {
        current_bps
    };

    if utilization >= config.critical_threshold_bps {
        let severity = AlertSeverity::Critical;
        emit_alert(
            env,
            user,
            category,
            utilization,
            threshold_bps,
            severity.clone(),
            symbol_short!("crit"),
        );
        Some(severity)
    } else if utilization >= config.warning_threshold_bps {
        let severity = AlertSeverity::Warning;
        emit_alert(
            env,
            user,
            category,
            utilization,
            threshold_bps,
            severity.clone(),
            symbol_short!("warn"),
        );
        Some(severity)
    } else if utilization >= config.info_threshold_bps {
        let severity = AlertSeverity::Info;
        emit_alert(
            env,
            user,
            category,
            utilization,
            threshold_bps,
            severity.clone(),
            symbol_short!("info"),
        );
        Some(severity)
    } else {
        None
    }
}

/// Acknowledge (dismiss) an alert.
pub fn acknowledge_alert(env: &Env, user: &Address, alert_id: u64) -> bool {
    let mut alerts = get_alerts(env);
    if let Some(mut alert) = alerts.get(alert_id) {
        if alert.user == *user {
            alert.acknowledged = true;
            alerts.set(alert_id, alert);
            save_alerts(env, &alerts);
            return true;
        }
    }
    false
}

/// Get all active (unacknowledged) alerts for a user.
pub fn get_user_alerts(env: &Env, user: &Address) -> Vec<RiskAlert> {
    let alerts = get_alerts(env);
    let mut result: Vec<RiskAlert> = Vec::new(env);
    let total = env
        .storage()
        .instance()
        .get::<_, u64>(&Symbol::short("racnt"))
        .unwrap_or(0);

    for id in 1..=total {
        if let Some(alert) = alerts.get(id) {
            if alert.user == *user && !alert.acknowledged {
                result.push_back(alert);
            }
        }
    }
    result
}

/// Get alert status summary for a user.
pub fn get_alert_status(env: &Env, user: &Address) -> AlertStatus {
    let alerts = get_alerts(env);
    let total = env
        .storage()
        .instance()
        .get::<_, u64>(&Symbol::short("racnt"))
        .unwrap_or(0);

    let mut active = 0u32;
    let mut info_count = 0u32;
    let mut warning_count = 0u32;
    let mut critical_count = 0u32;
    let mut max_severity = AlertSeverity::Info;

    for id in 1..=total {
        if let Some(alert) = alerts.get(id) {
            if alert.user == *user && !alert.acknowledged {
                active = active.saturating_add(1);
                match alert.severity {
                    AlertSeverity::Info => {
                        info_count = info_count.saturating_add(1);
                    }
                    AlertSeverity::Warning => {
                        warning_count = warning_count.saturating_add(1);
                        if matches!(max_severity, AlertSeverity::Info) {
                            max_severity = AlertSeverity::Warning;
                        }
                    }
                    AlertSeverity::Critical => {
                        critical_count = critical_count.saturating_add(1);
                        max_severity = AlertSeverity::Critical;
                    }
                }
            }
        }
    }

    AlertStatus {
        active_alerts: active,
        info_count,
        warning_count,
        critical_count,
        max_severity,
    }
}

/// Dismiss all alerts for a user.
pub fn dismiss_all(env: &Env, user: &Address) -> u32 {
    let mut alerts = get_alerts(env);
    let total = env
        .storage()
        .instance()
        .get::<_, u64>(&Symbol::short("racnt"))
        .unwrap_or(0);
    let mut dismissed = 0u32;

    for id in 1..=total {
        if let Some(mut alert) = alerts.get(id) {
            if alert.user == *user && !alert.acknowledged {
                alert.acknowledged = true;
                alerts.set(id, alert);
                dismissed = dismissed.saturating_add(1);
            }
        }
    }

    if dismissed > 0 {
        save_alerts(env, &alerts);
    }

    dismissed
}

/// Update alert thresholds (admin only).
pub fn set_alert_config(env: &Env, config: &AlertConfig) {
    env.storage().instance().set(&ALERTS_CONFIG_KEY, config);
}

/// Get current alert configuration.
pub fn get_alert_config_stored(env: &Env) -> AlertConfig {
    get_alert_config(env)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn get_alert_config(env: &Env) -> AlertConfig {
    env.storage()
        .instance()
        .get(&ALERTS_CONFIG_KEY)
        .unwrap_or_default()
}

fn get_alerts(env: &Env) -> soroban_sdk::Map<u64, RiskAlert> {
    env.storage()
        .instance()
        .get(&ALERTS_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

fn save_alerts(env: &Env, alerts: &soroban_sdk::Map<u64, RiskAlert>) {
    env.storage().instance().set(&ALERTS_KEY, alerts);
}

fn next_alert_id(env: &Env) -> u64 {
    let key = Symbol::short("racnt");
    let current: u64 = env.storage().instance().get(&key).unwrap_or(0);
    let next = current.saturating_add(1);
    env.storage().instance().set(&key, &next);
    next
}

fn emit_alert(
    env: &Env,
    user: &Address,
    category: Symbol,
    utilization_bps: u32,
    threshold_bps: u32,
    severity: AlertSeverity,
    description: Symbol,
) {
    let alert_id = next_alert_id(env);
    let now = env.ledger().timestamp();

    let alert = RiskAlert {
        alert_id,
        user: user.clone(),
        category: category.clone(),
        utilization_bps,
        threshold_bps,
        severity: severity.clone(),
        created_at: now,
        acknowledged: false,
        description: description.clone(),
    };

    let mut alerts = get_alerts(env);
    alerts.set(alert_id, alert);
    save_alerts(env, &alerts);

    // Emit on-chain event for off-chain indexing
    let severity_tag = match severity {
        AlertSeverity::Info => symbol_short!("info"),
        AlertSeverity::Warning => symbol_short!("warn"),
        AlertSeverity::Critical => symbol_short!("crit"),
    };

    env.events().publish(
        (Symbol::new(env, "RiskAlertEmitted"), user, category),
        (alert_id, severity_tag, utilization_bps, threshold_bps, now),
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
    fn test_no_alert_below_threshold() {
        let (env, user) = setup();
        let result = evaluate_and_alert(
            &env,
            &user,
            symbol_short!("pos"),
            3000, // 30% utilization
            10_000,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_info_alert() {
        let (env, user) = setup();
        let result = evaluate_and_alert(
            &env,
            &user,
            symbol_short!("pos"),
            5500, // 55% → above 50% info threshold
            10_000,
        );
        assert_eq!(result, Some(AlertSeverity::Info));
    }

    #[test]
    fn test_warning_alert() {
        let (env, user) = setup();
        let result = evaluate_and_alert(
            &env,
            &user,
            symbol_short!("pos"),
            8000, // 80% → above 75% warning threshold
            10_000,
        );
        assert_eq!(result, Some(AlertSeverity::Warning));
    }

    #[test]
    fn test_critical_alert() {
        let (env, user) = setup();
        let result = evaluate_and_alert(
            &env,
            &user,
            symbol_short!("pos"),
            9500, // 95% → above 90% critical threshold
            10_000,
        );
        assert_eq!(result, Some(AlertSeverity::Critical));
    }

    #[test]
    fn test_acknowledge_alert() {
        let (env, user) = setup();
        evaluate_and_alert(&env, &user, symbol_short!("pos"), 8000, 10_000);

        let status = get_alert_status(&env, &user);
        assert_eq!(status.active_alerts, 1);

        // Acknowledge the alert (id = 1 since it's the first)
        assert!(acknowledge_alert(&env, &user, 1));

        let status = get_alert_status(&env, &user);
        assert_eq!(status.active_alerts, 0);
    }

    #[test]
    fn test_get_user_alerts() {
        let (env, user) = setup();
        evaluate_and_alert(&env, &user, symbol_short!("pos"), 8000, 10_000);
        evaluate_and_alert(&env, &user, symbol_short!("lev"), 9500, 10_000);

        let alerts = get_user_alerts(&env, &user);
        assert_eq!(alerts.len(), 2);
    }

    #[test]
    fn test_dismiss_all() {
        let (env, user) = setup();
        evaluate_and_alert(&env, &user, symbol_short!("pos"), 8000, 10_000);
        evaluate_and_alert(&env, &user, symbol_short!("lev"), 9500, 10_000);

        let dismissed = dismiss_all(&env, &user);
        assert_eq!(dismissed, 2);

        let status = get_alert_status(&env, &user);
        assert_eq!(status.active_alerts, 0);
    }

    #[test]
    fn test_alert_status_counts() {
        let (env, user) = setup();
        evaluate_and_alert(&env, &user, symbol_short!("a"), 5500, 10_000); // Info
        evaluate_and_alert(&env, &user, symbol_short!("b"), 8000, 10_000); // Warning
        evaluate_and_alert(&env, &user, symbol_short!("c"), 9500, 10_000); // Critical

        let status = get_alert_status(&env, &user);
        assert_eq!(status.active_alerts, 3);
        assert_eq!(status.info_count, 1);
        assert_eq!(status.warning_count, 1);
        assert_eq!(status.critical_count, 1);
        assert_eq!(status.max_severity, AlertSeverity::Critical);
    }

    #[test]
    fn test_custom_alert_config() {
        let (env, _user) = setup();
        let config = AlertConfig {
            info_threshold_bps: 3_000,
            warning_threshold_bps: 6_000,
            critical_threshold_bps: 8_000,
        };
        set_alert_config(&env, &config);
        assert_eq!(get_alert_config_stored(&env).warning_threshold_bps, 6_000);
    }

    #[test]
    fn test_multiple_users_independent() {
        let env = Env::default();
        let user_a = Address::generate(&env);
        let user_b = Address::generate(&env);

        evaluate_and_alert(&env, &user_a, symbol_short!("pos"), 8000, 10_000);

        let status_a = get_alert_status(&env, &user_a);
        let status_b = get_alert_status(&env, &user_b);
        assert_eq!(status_a.active_alerts, 1);
        assert_eq!(status_b.active_alerts, 0);
    }
}
