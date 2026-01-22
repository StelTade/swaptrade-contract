use soroban_sdk::{Env, Symbol, Address};

use crate::portfolio::{Portfolio, Asset};
use crate::referral::ReferralSystem;

fn symbol_to_asset(sym: &Symbol) -> Option<Asset> {
    let s = sym.to_string();
    match s.as_str() {
        "XLM" => Some(Asset::XLM),
        "USDC-SIM" => Some(Asset::Custom(sym.clone())),
        _ => None,
    }
}

/// Calculates swap fees based on amount (currently 0.3%)
fn calculate_swap_fee(amount: i128) -> i128 {
    (amount * 3) / 1000 // 0.3% fee
}

/// Performs a simplified 1:1 swap between XLM and USDC-SIM for a user.
/// Returns the amount received after applying referral discounts.
pub fn perform_swap(
    env: &Env,
    portfolio: &mut Portfolio,
    from: Symbol,
    to: Symbol,
    amount: i128,
    user: Address,
) -> i128 {
    assert!(amount > 0, "Amount must be positive");

    // Validate tokens and disallow identical pairs
    assert!(from != to, "Tokens must be different");

    let from_asset = symbol_to_asset(&from).expect("Invalid from token");
    let to_asset = symbol_to_asset(&to).expect("Invalid to token");

    // Calculate swap fee
    let original_fee = calculate_swap_fee(amount);
    
    // Load referral system to check for discounts
    let mut referral_system: ReferralSystem = env
        .storage()
        .instance()
        .get(&Symbol::new(env, "referral_system"))
        .unwrap_or_else(|| ReferralSystem::new(env));
    
    // Process trade for referral rewards and get discount
    let discount_percentage = referral_system.process_trade_for_referral(env, user.clone(), original_fee);
    
    // Apply discount to fee if applicable
    let discounted_fee = if discount_percentage > 0 {
        original_fee - ((original_fee * discount_percentage as i128) / 100)
    } else {
        original_fee
    };
    
    // Update referral system in storage
    env.storage().instance().set(&Symbol::new(env, "referral_system"), &referral_system);
    
    // Simplified AMM: 1:1 rate between XLM and USDC-SIM
    // Amount after fees is what the user receives
    let out_amount = amount - (original_fee - discounted_fee); // Only difference is the discount
    
    // Debit from 'from_asset' and credit to 'to_asset'
    portfolio.transfer_asset(env, from_asset, to_asset, user, amount);
    
    // Collect the actual fee charged
    let actual_fee = original_fee - discounted_fee;
    if actual_fee > 0 {
        portfolio.collect_fee(actual_fee);
    }

    out_amount
}
