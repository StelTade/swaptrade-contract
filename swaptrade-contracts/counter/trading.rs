use soroban_sdk::{Env, Symbol, Address};

use crate::portfolio::{Portfolio, Asset};

fn symbol_to_asset(sym: &Symbol) -> Option<Asset> {
    let s = sym.to_string();
    match s.as_str() {
        "XLM" => Some(Asset::XLM),
        "USDC-SIM" => Some(Asset::Custom(sym.clone())),
        _ => None,
    }
}

/// Performs a simplified 1:1 swap between XLM and USDC-SIM for a user.
/// Returns the amount received.
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

    // Simplified AMM: 1:1 rate between XLM and USDC-SIM
    let out_amount = amount;

    // Debit from 'from_asset' and credit to 'to_asset'
    portfolio.transfer_asset(env, from_asset, to_asset, user, amount);

    out_amount
}
