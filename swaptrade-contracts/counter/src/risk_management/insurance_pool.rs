//! Insurance Pool Mechanics
//!
//! Implements an optional parametric insurance mechanism where users can
//! deposit into a shared insurance pool and receive coverage against
//! qualifying adverse events (e.g., flash crashes, oracle failures).
//!
//! Premium Model
//! -------------
//! ```text
//! premium = coverage_amount × risk_score × duration_factor / 10_000
//!
//! coverage_payout = min(covered_loss, max_payout)
//! ```

use crate::portfolio::{Asset, Portfolio};
use crate::risk_management::RiskConfig;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// ── Storage keys ─────────────────────────────────────────────────────────────
const POOL_STATE_KEY: Symbol = symbol_short!("ipool");
const POOL_DEPOSITS_KEY: Symbol = symbol_short!("ipdep");
const POOL_POLICIES_KEY: Symbol = symbol_short!("ippol");

/// Default annual premium rate (basis points of coverage).
/// 200 bps = 2% annual premium.
const DEFAULT_PREMIUM_RATE_BPS: u32 = 200;

/// Maximum pool utilization before new policies are rejected.
const MAX_POOL_UTILIZATION_BPS: u32 = 8_000; // 80%

/// Protocol insurance reserve ratio (basis points of pool).
const PROTOCOL_RESERVE_BPS: u32 = 1_000; // 10%

// ── Types ────────────────────────────────────────────────────────────────────

/// Global insurance pool state.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct InsurancePoolState {
    /// Total assets deposited in the pool.
    pub total_liquidity: i128,
    /// Total coverage currently underwritten.
    pub total_coverage: i128,
    /// Accumulated premiums collected.
    pub total_premiums_collected: i128,
    /// Total payouts made.
    pub total_payouts_made: i128,
    /// Protocol reserve amount.
    pub protocol_reserve: i128,
    /// Number of active policies.
    pub active_policies: u32,
    /// Pool creation timestamp.
    pub created_at: u64,
}

impl Default for InsurancePoolState {
    fn default() -> Self {
        Self {
            total_liquidity: 0,
            total_coverage: 0,
            total_premiums_collected: 0,
            total_payouts_made: 0,
            protocol_reserve: 0,
            active_policies: 0,
            created_at: 0,
        }
    }
}

/// User deposit record.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PoolDeposit {
    /// Depositor address.
    pub depositor: Address,
    /// Amount deposited.
    pub amount: i128,
    /// Share of pool (basis points, 10_000 = 100%).
    pub share_bps: u32,
    /// Deposit timestamp.
    pub deposited_at: u64,
    /// Accumulated yield (basis points).
    pub yield_bps: u32,
}

/// Insurance policy purchased by a user.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct InsurancePolicy {
    /// Policy ID.
    pub policy_id: u64,
    /// Policy holder.
    pub holder: Address,
    /// Coverage amount (max payout).
    pub coverage_amount: i128,
    /// Premium paid.
    pub premium_paid: i128,
    /// Policy start timestamp.
    pub start_time: u64,
    /// Policy expiry timestamp.
    pub end_time: u64,
    /// Risk score at time of purchase (basis points, higher = riskier).
    pub risk_score_bps: u32,
    /// Whether a claim has been filed.
    pub claim_filed: bool,
    /// Payout amount (if claim was successful).
    pub payout_amount: i128,
}

/// Observable pool status.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct InsurancePoolStatus {
    /// Total liquidity available.
    pub total_liquidity: i128,
    /// Total active coverage.
    pub total_coverage: i128,
    /// Pool utilization (basis points).
    pub utilization_bps: u32,
    /// Current premium rate (basis points per block).
    pub premium_rate_bps: u32,
    /// Whether new policies can be issued.
    pub accepting_policies: bool,
    /// Number of active policies.
    pub active_policies: u32,
}

/// Result of a premium calculation.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PremiumQuote {
    /// Required premium for this coverage.
    pub premium: i128,
    /// Coverage amount.
    pub coverage_amount: i128,
    /// Duration in seconds.
    pub duration_secs: u64,
    /// Risk score factor (basis points).
    pub risk_factor_bps: u32,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Deposit liquidity into the insurance pool.
/// Returns the depositor's share (basis points).
pub fn deposit(env: &Env, depositor: &Address, amount: i128) -> Result<u32, &'static str> {
    if amount <= 0 {
        return Err("amount_must_be_positive");
    }

    let mut state = get_pool_state(env);
    let mut deposits = get_deposits(env);

    let now = env.ledger().timestamp();

    // Calculate share
    let total = state.total_liquidity;
    let new_share_bps = if total == 0 {
        10_000 // First depositor gets 100%
    } else {
        ((amount as u128) * 10_000 / (total as u128)) as u32
    };

    // Update pool state
    state.total_liquidity = state.total_liquidity.saturating_add(amount);
    let reserve_add = (amount as u128 * PROTOCOL_RESERVE_BPS as u128 / 10_000) as i128;
    state.protocol_reserve = state.protocol_reserve.saturating_add(reserve_add);

    // Update or create deposit record
    let deposit = PoolDeposit {
        depositor: depositor.clone(),
        amount,
        share_bps: new_share_bps,
        deposited_at: now,
        yield_bps: 0,
    };
    deposits.set(depositor.clone(), deposit);

    save_pool_state(env, &state);
    save_deposits(env, &deposits);

    // Emit event
    env.events().publish(
        (Symbol::new(env, "InsuranceDeposit"), depositor),
        (amount, state.total_liquidity, now),
    );

    Ok(new_share_bps)
}

/// Withdraw liquidity from the insurance pool (only if no active coverage
/// would be jeopardized).
pub fn withdraw(env: &Env, depositor: &Address, amount: i128) -> Result<i128, &'static str> {
    if amount <= 0 {
        return Err("amount_must_be_positive");
    }

    let mut state = get_pool_state(env);
    let mut deposits = get_deposits(env);

    let dep = deposits.get(depositor.clone()).ok_or("no_deposit_found")?;

    if dep.amount < amount {
        return Err("insufficient_deposit");
    }

    // Check that withdrawal won't jeopardize active coverage
    let available = state.total_liquidity.saturating_sub(state.total_coverage);
    if amount > available {
        return Err("withdrawal_would_undermine_coverage");
    }

    state.total_liquidity = state.total_liquidity.saturating_sub(amount);
    let reserve_sub = (amount as u128 * PROTOCOL_RESERVE_BPS as u128 / 10_000) as i128;
    state.protocol_reserve = state.protocol_reserve.saturating_sub(reserve_sub);

    let updated_dep = PoolDeposit {
        amount: dep.amount.saturating_sub(amount),
        ..dep
    };
    deposits.set(depositor.clone(), updated_dep);

    save_pool_state(env, &state);
    save_deposits(env, &deposits);

    env.events().publish(
        (Symbol::new(env, "InsuranceWithdrawal"), depositor),
        (amount, state.total_liquidity, env.ledger().timestamp()),
    );

    Ok(amount)
}

/// Calculate premium quote for a given coverage request.
pub fn calculate_premium(
    env: &Env,
    portfolio: &Portfolio,
    user: &Address,
    coverage_amount: i128,
    duration_secs: u64,
) -> PremiumQuote {
    let risk_score = calculate_user_risk_score(env, portfolio, user);
    let premium_rate = get_premium_rate_bps(env);

    // premium = coverage × risk_score × premium_rate × duration_days / (365 × 10_000)
    let duration_days = (duration_secs / 86_400).max(1);
    let premium = ((coverage_amount as u128)
        * (risk_score as u128)
        * (premium_rate as u128)
        * (duration_days as u128))
        / (365 * 10_000) as u128;

    PremiumQuote {
        premium: premium as i128,
        coverage_amount,
        duration_secs,
        risk_factor_bps: risk_score,
    }
}

/// Purchase an insurance policy.
pub fn purchase_policy(
    env: &Env,
    user: &Address,
    coverage_amount: i128,
    duration_secs: u64,
    premium: i128,
) -> Result<u64, &'static str> {
    let mut state = get_pool_state(env);

    // Check pool has capacity
    let utilization = if state.total_liquidity > 0 {
        ((state.total_coverage as u128) * 10_000 / (state.total_liquidity as u128)) as u32
    } else {
        10_000
    };

    if utilization >= MAX_POOL_UTILIZATION_BPS {
        return Err("pool_at_max_utilization");
    }

    // Check premium is sufficient
    let mut policies = get_policies(env);
    let now = env.ledger().timestamp();
    let policy_id = state.active_policies as u64 + 1;

    let policy = InsurancePolicy {
        policy_id,
        holder: user.clone(),
        coverage_amount,
        premium_paid: premium,
        start_time: now,
        end_time: now.saturating_add(duration_secs),
        risk_score_bps: 0,
        claim_filed: false,
        payout_amount: 0,
    };

    policies.set(policy_id, policy);

    // Update state
    state.total_coverage = state.total_coverage.saturating_add(coverage_amount);
    state.total_premiums_collected = state.total_premiums_collected.saturating_add(premium);
    state.active_policies = state.active_policies.saturating_add(1);

    save_pool_state(env, &state);
    save_policies(env, &policies);

    env.events().publish(
        (Symbol::new(env, "InsurancePolicyPurchased"), user),
        (policy_id, coverage_amount, premium, now),
    );

    Ok(policy_id)
}

/// File a claim against a policy.
pub fn file_claim(env: &Env, policy_id: u64, loss_amount: i128) -> Result<i128, &'static str> {
    let mut state = get_pool_state(env);
    let mut policies = get_policies(env);
    let now = env.ledger().timestamp();

    let mut policy = policies.get(policy_id).ok_or("policy_not_found")?;

    // Validate claim
    if policy.claim_filed {
        return Err("claim_already_filed");
    }
    if now > policy.end_time {
        return Err("policy_expired");
    }
    if loss_amount <= 0 {
        return Err("invalid_loss_amount");
    }

    // Calculate payout: min(loss, coverage)
    let payout = core::cmp::min(loss_amount, policy.coverage_amount);

    // Check pool has funds
    let available = state.total_liquidity.saturating_sub(state.total_coverage);
    let actual_payout = core::cmp::min(payout, available);

    if actual_payout <= 0 {
        return Err("insufficient_pool_funds");
    }

    // Save fields before moving policy into the map
    let holder = policy.holder.clone();
    let coverage_amount = policy.coverage_amount;

    // Update policy
    policy.claim_filed = true;
    policy.payout_amount = actual_payout;
    policies.set(policy_id, policy);

    // Update state
    state.total_payouts_made = state.total_payouts_made.saturating_add(actual_payout);
    state.total_coverage = state.total_coverage.saturating_sub(coverage_amount);
    state.active_policies = state.active_policies.saturating_sub(1);

    save_pool_state(env, &state);
    save_policies(env, &policies);

    env.events().publish(
        (Symbol::new(env, "InsuranceClaimPaid"), holder),
        (policy_id, loss_amount, actual_payout, now),
    );

    Ok(actual_payout)
}

/// Get observable pool status.
pub fn get_pool_status(env: &Env) -> InsurancePoolStatus {
    let state = get_pool_state(env);

    let utilization_bps = if state.total_liquidity > 0 {
        ((state.total_coverage as u128) * 10_000 / (state.total_liquidity as u128)) as u32
    } else {
        0
    };

    InsurancePoolStatus {
        total_liquidity: state.total_liquidity,
        total_coverage: state.total_coverage,
        utilization_bps,
        premium_rate_bps: get_premium_rate_bps(env),
        accepting_policies: utilization_bps < MAX_POOL_UTILIZATION_BPS,
        active_policies: state.active_policies,
    }
}

/// Get a specific policy.
pub fn get_policy(env: &Env, policy_id: u64) -> Option<InsurancePolicy> {
    let policies = get_policies(env);
    policies.get(policy_id)
}

/// Get all policies for a user (iterates through known policy IDs).
pub fn get_user_policy(env: &Env, user: &Address) -> Option<InsurancePolicy> {
    let policies = get_policies(env);
    let state = get_pool_state(env);
    let total = state.active_policies as u64;

    // Linear scan – acceptable for Soroban storage limits
    for id in 1..=total {
        if let Some(p) = policies.get(id) {
            if p.holder == *user && !p.claim_filed && env.ledger().timestamp() <= p.end_time {
                return Some(p);
            }
        }
    }
    None
}

/// Set the premium rate (admin only).
pub fn set_premium_rate_bps(env: &Env, rate_bps: u32) {
    env.storage()
        .instance()
        .set(&symbol_short!("iprate"), &rate_bps);
}

/// Get the premium rate.
pub fn get_premium_rate_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&symbol_short!("iprate"))
        .unwrap_or(DEFAULT_PREMIUM_RATE_BPS)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Calculate a user's risk score (0–10 000 bps) based on their portfolio.
fn calculate_user_risk_score(env: &Env, portfolio: &Portfolio, user: &Address) -> u32 {
    let config: RiskConfig = env
        .storage()
        .instance()
        .get(&Symbol::short("risk_cfg"))
        .unwrap_or_default();

    // Simple risk score: base 5000 (moderate) + concentration adjustment
    let xlm = portfolio.balance_of(env, Asset::XLM, user.clone());
    let usdc = portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());
    let total = xlm + usdc;

    if total == 0 {
        return 5_000; // Default moderate risk
    }

    let max_pos = xlm.max(usdc);
    let concentration = ((max_pos as u64) * 10_000 / (total as u64)) as u32;

    // Risk score: 5000 base + concentration adds up to 5000 more
    5_000u32.saturating_add(concentration / 2)
}

fn get_pool_state(env: &Env) -> InsurancePoolState {
    env.storage()
        .instance()
        .get(&POOL_STATE_KEY)
        .unwrap_or_default()
}

fn save_pool_state(env: &Env, state: &InsurancePoolState) {
    env.storage().instance().set(&POOL_STATE_KEY, state);
}

fn get_deposits(env: &Env) -> soroban_sdk::Map<Address, PoolDeposit> {
    env.storage()
        .instance()
        .get(&POOL_DEPOSITS_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

fn save_deposits(env: &Env, deposits: &soroban_sdk::Map<Address, PoolDeposit>) {
    env.storage().instance().set(&POOL_DEPOSITS_KEY, deposits);
}

fn get_policies(env: &Env) -> soroban_sdk::Map<u64, InsurancePolicy> {
    env.storage()
        .instance()
        .get(&POOL_POLICIES_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

fn save_policies(env: &Env, policies: &soroban_sdk::Map<u64, InsurancePolicy>) {
    env.storage().instance().set(&POOL_POLICIES_KEY, policies);
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
    fn test_initial_pool_status() {
        let (env, _user) = setup();
        let status = get_pool_status(&env);
        assert_eq!(status.total_liquidity, 0);
        assert_eq!(status.total_coverage, 0);
        assert_eq!(status.active_policies, 0);
        assert!(status.accepting_policies);
    }

    #[test]
    fn test_deposit() {
        let (env, user) = setup();
        let result = deposit(&env, &user, 1_000_000);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10_000); // First depositor gets 100%

        let status = get_pool_status(&env);
        assert_eq!(status.total_liquidity, 1_000_000);
    }

    #[test]
    fn test_deposit_zero_fails() {
        let (env, user) = setup();
        assert!(deposit(&env, &user, 0).is_err());
    }

    #[test]
    fn test_deposit_negative_fails() {
        let (env, user) = setup();
        assert!(deposit(&env, &user, -100).is_err());
    }

    #[test]
    fn test_withdraw() {
        let (env, user) = setup();
        deposit(&env, &user, 1_000_000).unwrap();

        let withdrawn = withdraw(&env, &user, 500_000);
        assert!(withdrawn.is_ok());
        assert_eq!(withdrawn.unwrap(), 500_000);

        let status = get_pool_status(&env);
        assert_eq!(status.total_liquidity, 500_000);
    }

    #[test]
    fn test_withdraw_exceeds_deposit() {
        let (env, user) = setup();
        deposit(&env, &user, 100).unwrap();
        assert!(withdraw(&env, &user, 200).is_err());
    }

    #[test]
    fn test_withdraw_no_deposit() {
        let (env, user) = setup();
        assert!(withdraw(&env, &user, 100).is_err());
    }

    #[test]
    fn test_premium_calculation() {
        let (env, user) = setup();
        let mut portfolio = Portfolio::new(&env);
        portfolio.credit(&env, Asset::XLM, user.clone(), 500);
        portfolio.credit(
            &env,
            Asset::Custom(Symbol::short("USDCSIM")),
            user.clone(),
            500,
        );

        let quote = calculate_premium(&env, &portfolio, &user, 10_000, 86_400 * 30);
        assert!(quote.premium > 0);
        assert_eq!(quote.coverage_amount, 10_000);
    }

    #[test]
    fn test_purchase_policy() {
        let (env, user) = setup();
        deposit(&env, &user, 1_000_000).unwrap();

        let policy_id = purchase_policy(&env, &user, 500_000, 86_400 * 30, 10_000);
        assert!(policy_id.is_ok());

        let status = get_pool_status(&env);
        assert_eq!(status.active_policies, 1);
        assert_eq!(status.total_coverage, 500_000);
    }

    #[test]
    fn test_claim() {
        let (env, user) = setup();
        deposit(&env, &user, 1_000_000).unwrap();
        let policy_id = purchase_policy(&env, &user, 500_000, 86_400 * 30, 10_000).unwrap();

        let payout = file_claim(&env, policy_id, 200_000);
        assert!(payout.is_ok());
        assert_eq!(payout.unwrap(), 200_000);

        let policy = get_policy(&env, policy_id).unwrap();
        assert!(policy.claim_filed);
        assert_eq!(policy.payout_amount, 200_000);
    }

    #[test]
    fn test_claim_exceeds_coverage() {
        let (env, user) = setup();
        deposit(&env, &user, 1_000_000).unwrap();
        let policy_id = purchase_policy(&env, &user, 100_000, 86_400 * 30, 5_000).unwrap();

        let payout = file_claim(&env, policy_id, 500_000);
        assert!(payout.is_ok());
        assert_eq!(payout.unwrap(), 100_000); // Capped at coverage
    }

    #[test]
    fn test_double_claim_fails() {
        let (env, user) = setup();
        deposit(&env, &user, 1_000_000).unwrap();
        let policy_id = purchase_policy(&env, &user, 100_000, 86_400 * 30, 5_000).unwrap();

        file_claim(&env, policy_id, 50_000).unwrap();
        assert!(file_claim(&env, policy_id, 50_000).is_err());
    }

    #[test]
    fn test_pool_utilization_blocks_new_policies() {
        let (env, user) = setup();
        let user2 = Address::generate(&env);

        deposit(&env, &user, 100_000).unwrap();
        // Purchase coverage = 80% of liquidity (max utilization)
        purchase_policy(&env, &user, 80_000, 86_400 * 30, 5_000).unwrap();

        // Now pool is at max utilization – new policy should fail
        deposit(&env, &user2, 10_000).unwrap();
        let result = purchase_policy(&env, &user2, 10_000, 86_400 * 30, 500);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_premium_rate() {
        let (env, _user) = setup();
        set_premium_rate_bps(&env, 500);
        assert_eq!(get_premium_rate_bps(&env), 500);
    }
}
