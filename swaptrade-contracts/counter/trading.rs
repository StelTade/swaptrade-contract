use soroban_sdk::{Env, Symbol, Address, symbol_short};

use crate::portfolio::{Portfolio, Asset};
use crate::oracle::{self, Oracle, PRECISION};

fn symbol_to_asset(env: &Env, sym: &Symbol) -> Option<Asset> {
    if *sym == symbol_short!("XLM") {
        return Some(Asset::XLM);
    }
    if *sym == Symbol::new(env, "USDCSIM") {
        return Some(Asset::Custom(sym.clone()));
    }
    None
}

/// Performs a swap with price feeds and slippage protection.
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
    assert!(from != to, "Tokens must be different");

    let from_asset = symbol_to_asset(env, &from).expect("Invalid from token");
    let to_asset = symbol_to_asset(env, &to).expect("Invalid to token");

    // Get price and liquidity from Oracle
    let price_data = Oracle::get_price(env, (from.clone(), to.clone()))
        .expect("Price not available");
    
    // Calculate theoretical output
    let amount_u128 = amount as u128;
    let price = price_data.price;
    // price is 18 decimal fixed point
    let theoretical_output = amount_u128.checked_mul(price).unwrap() / PRECISION;
    
    // Calculate price impact
    let liquidity = price_data.liquidity;
    if liquidity == 0 {
        panic!("Liquidity is zero");
    }
    
    // impact_bps = (theoretical_output * 10000) / liquidity
    let impact_bps = (theoretical_output * 10000) / liquidity;
    
    // Check max slippage
    let max_slippage = oracle::get_max_slippage(env);
    if max_slippage > 0 && impact_bps > max_slippage as u128 {
        panic!("Slippage exceeded");
    }
    
    // Apply slippage
    let actual_output = if impact_bps >= 10000 {
        0
    } else {
        theoretical_output * (10000 - impact_bps) / 10000
    };
    
    let actual_output_i128 = actual_output as i128;
    
    // Execute swap using the new swap_asset method which supports different in/out amounts
    portfolio.swap_asset(env, from_asset, to_asset, user, amount, actual_output_i128);
    
    actual_output_i128
}
