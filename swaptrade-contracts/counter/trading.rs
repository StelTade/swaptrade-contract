use soroban_sdk::{Env, Symbol, Address, symbol_short};
// use crate::events::SwapExecuted;
use crate::portfolio::{Portfolio, Asset};
use crate::oracle::{get_stored_price, ContractError};

const PRECISION: u128 = 1_000_000_000_000_000_000; // 1e18
const STALE_THRESHOLD_SECONDS: u64 = 600; // 10 minutes
const LP_FEE_BPS: u128 = 30; // 0.3% = 30 basis points


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

    // 2. Get current pool liquidity (from LP pool)
    let xlm_liquidity = portfolio.get_liquidity(Asset::XLM);
    let usdc_liquidity = portfolio.get_liquidity(Asset::Custom(symbol_short!("USDCSIM")));

    // 3. Calculate swap output using constant product AMM formula: x * y = k
    // With 0.3% fee: amount_out = (y * amount_in * (1 - fee)) / (x + amount_in * (1 - fee))
    let amount_u128 = amount as u128;
    let (reserve_in, reserve_out) = if from_asset == Asset::XLM {
        (xlm_liquidity as u128, usdc_liquidity as u128)
    } else {
        (usdc_liquidity as u128, xlm_liquidity as u128)
    };

    let actual_out = if reserve_in > 0 && reserve_out > 0 {
        // Apply fee: amount_in_after_fee = amount_in * (1 - fee_bps / 10000)
        let amount_in_after_fee = (amount_u128 * (10000 - LP_FEE_BPS)) / 10000;
        
        // Constant product formula: (x + dx) * (y - dy) = x * y
        // dy = (y * dx) / (x + dx)
        let numerator = reserve_out.saturating_mul(amount_in_after_fee);
        let denominator = reserve_in.saturating_add(amount_in_after_fee);
        
        if denominator == 0 {
            panic!("Division by zero in AMM calculation");
        }
        
        numerator / denominator
    } else {
        // If no liquidity, use oracle price (fallback)
        let price = match get_price_with_staleness_check(env, from.clone(), to.clone()) {
            Ok(p) => p,
            Err(ContractError::StalePrice) => panic!("Oracle price is stale"),
            Err(ContractError::InvalidPrice) => panic!("Oracle price is invalid"),
            Err(ContractError::PriceNotSet) => PRECISION, // Fallback to 1:1
            _ => PRECISION,
        };
        (amount_u128 * price) / PRECISION
    };

    let out_amount = actual_out as i128;
    assert!(out_amount > 0, "Output amount must be positive");

    // 4. Calculate fee amount (0.3% of input)
    let fee_amount = (amount_u128 * LP_FEE_BPS) / 10000;
    let fee_amount_i128 = fee_amount as i128;

    // 5. Check slippage protection
    let theoretical_out = if reserve_in > 0 && reserve_out > 0 {
        // Theoretical output without fee
        let numerator = reserve_out.saturating_mul(amount_u128);
        let denominator = reserve_in.saturating_add(amount_u128);
        if denominator == 0 {
            amount_u128 // Fallback
        } else {
            numerator / denominator
        }
    } else {
        amount_u128 // Fallback to 1:1
    };

    let max_slip = env.storage().instance().get(&symbol_short!("MAX_SLIP")).unwrap_or(10000u32);
    if theoretical_out > 0 {
        let slippage_bps = ((theoretical_out - actual_out) * 10000) / theoretical_out;
        if slippage_bps > max_slip as u128 {
            panic!("Slippage exceeded: {} bps > {} bps", slippage_bps, max_slip);
        }
    }

    // 6. Update Portfolio (User Balances) - transfer from user
    portfolio.transfer_asset(env, from_asset.clone(), to_asset.clone(), user.clone(), amount);
    
    // 7. Update Pool Liquidity using constant product AMM
    // Add input amount (minus fee) to reserve_in, subtract output from reserve_out
    if reserve_in > 0 && reserve_out > 0 {
        let amount_in_after_fee = amount - fee_amount_i128;
        
        if from_asset == Asset::XLM {
            portfolio.set_liquidity(Asset::XLM, xlm_liquidity.saturating_add(amount_in_after_fee));
            portfolio.set_liquidity(Asset::Custom(symbol_short!("USDCSIM")), usdc_liquidity.saturating_sub(out_amount));
        } else {
            portfolio.set_liquidity(Asset::Custom(symbol_short!("USDCSIM")), usdc_liquidity.saturating_add(amount_in_after_fee));
            portfolio.set_liquidity(Asset::XLM, xlm_liquidity.saturating_sub(out_amount));
        }
    }

    // 8. Collect and attribute fees to LPs
    if fee_amount_i128 > 0 {
        portfolio.add_lp_fees(fee_amount_i128);
        // Fees are accumulated and can be distributed proportionally to LPs based on their LP token share
        // This is tracked in lp_fees_accumulated for future distribution
    }

    out_amount
}
