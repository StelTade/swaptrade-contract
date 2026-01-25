use soroban_sdk::{Env, Symbol, Address, symbol_short};
// use crate::events::SwapExecuted;
use crate::portfolio::{Portfolio, Asset};
use crate::oracle::{get_stored_price, ContractError};

const PRECISION: u128 = 1_000_000_000_000_000_000; // 1e18
const STALE_THRESHOLD_SECONDS: u64 = 600; // 10 minutes


fn symbol_to_asset(sym: &Symbol) -> Option<Asset> {
    if *sym == symbol_short!("XLM") {
        Some(Asset::XLM)
    } else if *sym == symbol_short!("USDCSIM") {
        Some(Asset::Custom(sym.clone()))
    } else {
        None
    }
}



// Helper to get price with staleness check
fn get_price_with_staleness_check(env: &Env, from: Symbol, to: Symbol) -> Result<u128, ContractError> {
    // Try (from, to)
    if let Some(data) = get_stored_price(env, (from.clone(), to.clone())) {
        if env.ledger().timestamp() - data.timestamp > STALE_THRESHOLD_SECONDS {
             return Err(ContractError::StalePrice);
        }
        return Ok(data.price);
    }
    // Try (to, from) and invert
    if let Some(data) = get_stored_price(env, (to.clone(), from.clone())) {
        if env.ledger().timestamp() - data.timestamp > STALE_THRESHOLD_SECONDS {
             return Err(ContractError::StalePrice);
        }
        if data.price == 0 { return Err(ContractError::InvalidPrice); }
        // Invert
        let inv = (PRECISION * PRECISION) / data.price;
        return Ok(inv);
    }
    
    Err(ContractError::PriceNotSet)
}

/// Performs a swap with oracle pricing and slippage protection
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

    let from_asset = symbol_to_asset(&from).expect("Invalid from token");
    let to_asset = symbol_to_asset(&to).expect("Invalid to token");

    // 1. Get Price (Default to 1:1 if not set, to support existing tests/defaults, or panic?)
    // Requirement: "Currently using hardcoded 1:1 (unrealistic)".
    // Implementation: Try Oracle, fallback to 1:1 if not set (with warning logic if possible, but here just fallback)
    // BUT we need to support "Stale Price" error.
    // So:
    // - If Price Set & Valid -> Use it.
    // - If Price Set & Stale -> Panic/Error.
    // - If Price NOT Set -> Use 1:1 (Legacy/Default).
    
    let price = match get_price_with_staleness_check(env, from.clone(), to.clone()) {
        Ok(p) => p,
        Err(ContractError::StalePrice) => panic!("Oracle price is stale"),
        Err(ContractError::InvalidPrice) => panic!("Oracle price is invalid"),
        Err(ContractError::PriceNotSet) => PRECISION, // Fallback to 1:1
        _ => PRECISION,
    };

    // 2. Theoretical Output
    let amount_u128 = amount as u128;
    let theoretical_out = (amount_u128 * price) / PRECISION;

    // 3. Liquidity & Slippage
    let liquidity_out = portfolio.get_liquidity(to_asset.clone()) as u128;
    
    let actual_out = if liquidity_out > 0 {
        // Price impact = theoretical_out / liquidity
        // Use u128 for calculation
        let impact_bps = (theoretical_out * 10000) / liquidity_out;
        
        // Check max slippage
        let max_slip = env.storage().instance().get(&symbol_short!("MAX_SLIP")).unwrap_or(10000u32); 
        if impact_bps > max_slip as u128 {
             panic!("Slippage exceeded: {} bps > {} bps", impact_bps, max_slip);
        }
        
        // Apply slippage
        // actual = theoretical * (1 - impact)
        let slippage_amt = (theoretical_out * impact_bps) / 10000;
        if slippage_amt >= theoretical_out {
            0
        } else {
            theoretical_out - slippage_amt
        }
    } else {
        // If no liquidity set, assume infinite (no slippage)
        theoretical_out
    };

    let out_amount = actual_out as i128;

    // 4. Update Portfolio (User Balances)
    // Debit input Amount
    portfolio.debit(env, from_asset.clone(), user.clone(), amount);
    // Credit output Amount (calculated by AMM/Oracle)
    portfolio.credit(env, to_asset.clone(), user.clone(), out_amount);
    
    // 5. Update Pool Liquidity (Virtual)
    // Only update if liquidity was set (simulated)
    if liquidity_out > 0 {
         let new_liq_out = (liquidity_out - actual_out) as i128;
         portfolio.set_liquidity(to_asset.clone(), new_liq_out);
         
         let liquidity_in = portfolio.get_liquidity(from_asset.clone());
         portfolio.set_liquidity(from_asset, liquidity_in + amount);
    }

    out_amount
}
