// PnL engine tests for issue #214 (weighted-average cost basis, realized on
// sells, unrealized vs current oracle price, zeroed summaries, cache behavior).

#[cfg(test)]
mod pnl_tests {
    use crate::errors::ContractError;
    use crate::{CounterContract, CounterContractClient};
    use soroban_sdk::{symbol_short, Address, Env};

    const PRECISION: u128 = 1_000_000_000_000_000_000; // 1e18, matches trading.rs

    fn setup() -> (Env, CounterContractClient<'static>, Address) {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = CounterContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);

        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");
        client.mint(&xlm, &user, &1_000_000);
        client.mint(&usdc, &user, &0);

        // USDCSIM priced at 0.5 XLM
        client.set_price(&(usdc.clone(), xlm.clone()), &(PRECISION / 2));
        (env, client, user)
    }

    #[test]
    fn weighted_average_cost_basis_across_multiple_buys() {
        let (_env, client, user) = setup();
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Buy #1: pay 1000 XLM, receive 2000 USDCSIM @ 0.5 XLM each
        let realized = client.record_pnl_trade(&user, &xlm, &usdc, &1_000, &2_000);
        assert_eq!(realized, 0, "acquisitions book no realized PnL");

        // Price moves to 0.75 XLM; buy #2: pay 1500 XLM for 2000 USDCSIM
        client.set_price(&(usdc.clone(), xlm.clone()), &((PRECISION * 3) / 4));
        let realized = client.record_pnl_trade(&user, &xlm, &usdc, &1_500, &2_000);
        assert_eq!(realized, 0);

        let summary = client.get_portfolio_pnl(&user);
        assert_eq!(summary.realized, 0);
        // basis: 1000 + 1500 = 2500 XLM for 4000 USDCSIM (weighted avg 0.625)
        // value at 0.75: 4000 * 0.75 = 3000; unrealized = +500
        assert_eq!(summary.total_value, 3_000);
        assert_eq!(summary.unrealized, 500);
    }

    #[test]
    fn realized_pnl_booked_on_sell() {
        let (_env, client, user) = setup();
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Build a basis of 2500 XLM over 4000 USDCSIM (avg 0.625)
        client.record_pnl_trade(&user, &xlm, &usdc, &1_000, &2_000);
        client.set_price(&(usdc.clone(), xlm.clone()), &((PRECISION * 3) / 4));
        client.record_pnl_trade(&user, &xlm, &usdc, &1_500, &2_000);

        // Sell 2000 USDCSIM at 0.8 XLM -> proceeds 1600, released basis 1250
        client.set_price(&(usdc.clone(), xlm.clone()), &((PRECISION * 4) / 5));
        let realized = client.record_pnl_trade(&user, &usdc, &xlm, &2_000, &1_600);
        assert_eq!(realized, 350);

        // Remaining position: 2000 USDCSIM with 1250 XLM basis.
        // Price drops to 0.7 -> value 1400, unrealized +150
        client.set_price(&(usdc.clone(), xlm.clone()), &((PRECISION * 7) / 10));
        let summary = client.get_portfolio_pnl(&user);
        assert_eq!(summary.realized, 350);
        assert_eq!(summary.total_value, 1_400);
        assert_eq!(summary.unrealized, 150);
    }

    #[test]
    fn unrealized_reflects_price_moves() {
        let (_env, client, user) = setup();
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        client.record_pnl_trade(&user, &xlm, &usdc, &1_000, &2_000);

        // Price doubles to 1.0 XLM: value 2000 vs basis 1000 -> unrealized +1000
        client.set_price(&(usdc.clone(), xlm.clone()), &PRECISION);
        let summary = client.get_portfolio_pnl(&user);
        assert_eq!(summary.unrealized, 1_000);
        assert_eq!(summary.total_value, 2_000);

        // Price crashes to 0.25 XLM: value 500 vs basis 1000 -> unrealized -500
        client.set_price(&(usdc.clone(), xlm.clone()), &(PRECISION / 4));
        let summary = client.get_portfolio_pnl(&user);
        assert_eq!(summary.unrealized, -500);
        assert_eq!(summary.total_value, 500);
    }

    #[test]
    fn zeroed_summary_for_user_without_trades() {
        let (env, client, _user) = setup();
        let bystander = Address::generate(&env);
        let summary = client.get_portfolio_pnl(&bystander);
        assert_eq!(summary.realized, 0);
        assert_eq!(summary.unrealized, 0);
        assert_eq!(summary.total_value, 0);
    }

    #[test]
    fn sell_more_than_held_fails() {
        let (_env, client, user) = setup();
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        client.record_pnl_trade(&user, &xlm, &usdc, &1_000, &2_000);

        // Try to dispose 5000 USDCSIM while holding only 2000
        let result = client.try_record_pnl_trade(&user, &usdc, &xlm, &5_000, &4_000);
        assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
    }

    #[test]
    fn unknown_price_is_rejected() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = CounterContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);

        let xlm = symbol_short!("XLM");
        let gold = symbol_short!("GOLD");
        client.mint(&xlm, &user, &1_000);
        // No price published for (GOLD, XLM)

        let result = client.try_record_pnl_trade(&user, &xlm, &gold, &100, &100);
        assert_eq!(result, Err(Ok(ContractError::InvalidPrice)));
    }

    #[test]
    fn cache_invalidates_on_new_trade() {
        let (_env, client, user) = setup();
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Warm the PnL query cache (miss then hit)
        let _ = client.get_portfolio_pnl(&user);
        let _ = client.get_portfolio_pnl(&user);
        let (_, misses_before, _) = client.get_cache_stats();

        // A new trade must invalidate cached PnL queries
        client.record_pnl_trade(&user, &xlm, &usdc, &1_000, &2_000);
        let _ = client.get_portfolio_pnl(&user);
        let (_, misses_after, _) = client.get_cache_stats();

        assert!(
            misses_after > misses_before,
            "new trade should force a PnL cache miss"
        );
    }
}
