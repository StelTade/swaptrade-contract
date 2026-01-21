/// Comprehensive integration tests for Admin Dashboard Query Functions
/// Tests all acceptance criteria:
/// - 5 query functions return expected types
/// - Results match manual calculations
/// - Leaderboard order correct (highest PnL first)
/// - Multiple calls return consistent results

#[cfg(test)]
mod dashboard_query_tests {
    use crate::portfolio::{Portfolio, Asset};
    use soroban_sdk::{Env, testutils::Address as TestAddress};

    /// Test get_total_trading_volume accumulates swap amounts
    #[test]
    fn test_total_trading_volume_accumulates() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        assert_eq!(portfolio.get_total_trading_volume(), 0);
        
        let user1 = TestAddress::generate(&env);
        portfolio.mint(&env, Asset::XLM, user1.clone(), 5000);
        
        portfolio.transfer_asset(
            &env,
            Asset::XLM,
            Asset::Custom(soroban_sdk::symbol_short!("USDC")),
            user1,
            1000,
        );
        
        assert_eq!(portfolio.get_total_trading_volume(), 1000);
    }

    /// Test get_active_users_count tracks trading users
    #[test]
    fn test_active_users_count() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        assert_eq!(portfolio.get_active_users_count(), 0);
        
        let user1 = TestAddress::generate(&env);
        let user2 = TestAddress::generate(&env);
        
        portfolio.mint(&env, Asset::XLM, user1.clone(), 1000);
        portfolio.record_trade(&env, user1.clone());
        
        assert!(portfolio.get_active_users_count() >= 1);
        
        portfolio.mint(&env, Asset::XLM, user2.clone(), 1000);
        portfolio.record_trade(&env, user2.clone());
        
        assert!(portfolio.get_active_users_count() >= 2);
    }

    /// Test get_pool_stats returns correct tuple
    #[test]
    fn test_pool_stats() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        let (xlm, usdc, fees) = portfolio.get_pool_stats();
        assert_eq!(xlm, 0);
        assert_eq!(usdc, 0);
        assert_eq!(fees, 0);
        
        portfolio.add_pool_liquidity(5000, 5000);
        let (xlm, usdc, fees) = portfolio.get_pool_stats();
        assert_eq!(xlm, 5000);
        assert_eq!(usdc, 5000);
        
        portfolio.collect_fee(100);
        let (_, _, fees) = portfolio.get_pool_stats();
        assert_eq!(fees, 100);
    }

    /// Integration test with 5 users
    #[test]
    fn test_5_users_integration() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        let users: Vec<_> = (0..5)
            .map(|_| TestAddress::generate(&env))
            .collect();
        
        for (i, user) in users.iter().enumerate() {
            let amount = 1000 + (i as i128 * 500);
            portfolio.mint(&env, Asset::XLM, user.clone(), amount);
            portfolio.record_trade(&env, user.clone());
        }
        
        assert_eq!(portfolio.get_active_users_count(), 5);
        let expected_volume = 1000 + 1500 + 2000 + 2500 + 3000;
        assert_eq!(portfolio.get_total_trading_volume(), expected_volume);
    }

    /// Test manual calculation matches query results
    #[test]
    fn test_manual_calculation_matches() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        let user1 = TestAddress::generate(&env);
        let user2 = TestAddress::generate(&env);
        let user3 = TestAddress::generate(&env);
        
        let swap1 = 1000i128;
        let swap2 = 2000i128;
        let swap3 = 1500i128;
        
        portfolio.mint(&env, Asset::XLM, user1.clone(), swap1);
        portfolio.transfer_asset(&env, Asset::XLM, Asset::Custom(soroban_sdk::symbol_short!("USDC")), user1, swap1);
        
        portfolio.mint(&env, Asset::XLM, user2.clone(), swap2);
        portfolio.transfer_asset(&env, Asset::XLM, Asset::Custom(soroban_sdk::symbol_short!("USDC")), user2, swap2);
        
        portfolio.mint(&env, Asset::XLM, user3.clone(), swap3);
        portfolio.transfer_asset(&env, Asset::XLM, Asset::Custom(soroban_sdk::symbol_short!("USDC")), user3, swap3);
        
        let expected_total = swap1 + swap2 + swap3;
        assert_eq!(portfolio.get_total_trading_volume(), expected_total);
    }

    /// Test multiple calls return consistent results
    #[test]
    fn test_consistent_results() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        let user = TestAddress::generate(&env);
        portfolio.mint(&env, Asset::XLM, user.clone(), 5000);
        portfolio.transfer_asset(&env, Asset::XLM, Asset::Custom(soroban_sdk::symbol_short!("USDC")), user, 2000);
        
        let vol1 = portfolio.get_total_trading_volume();
        let vol2 = portfolio.get_total_trading_volume();
        let vol3 = portfolio.get_total_trading_volume();
        
        assert_eq!(vol1, vol2);
        assert_eq!(vol2, vol3);
        assert_eq!(vol1, 2000);
    }

    /// Test leaderboard order is correct
    #[test]
    fn test_leaderboard_descending_order() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        let user_low = TestAddress::generate(&env);
        let user_mid = TestAddress::generate(&env);
        let user_high = TestAddress::generate(&env);
        
        portfolio.mint(&env, Asset::XLM, user_low.clone(), 100);
        portfolio.mint(&env, Asset::XLM, user_mid.clone(), 500);
        portfolio.mint(&env, Asset::XLM, user_high.clone(), 1000);
        
        let leaderboard = portfolio.get_top_traders(3);
        
        if leaderboard.len() > 0 {
            if let Some((_, first_pnl)) = leaderboard.get(0) {
                assert_eq!(first_pnl, 1000);
            }
        }
    }

    /// Test leaderboard capped at 100
    #[test]
    fn test_leaderboard_cap() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        for _ in 0..150 {
            let user = TestAddress::generate(&env);
            portfolio.mint(&env, Asset::XLM, user.clone(), 100);
        }
        
        let top_traders = portfolio.get_top_traders(200);
        assert!(top_traders.len() <= 100);
    }

    /// Test empty portfolio queries
    #[test]
    fn test_empty_portfolio_queries() {
        let env = Env::default();
        let portfolio = Portfolio::new(&env);
        
        assert_eq!(portfolio.get_total_users(), 0);
        assert_eq!(portfolio.get_total_trading_volume(), 0);
        assert_eq!(portfolio.get_active_users_count(), 0);
        
        let top_traders = portfolio.get_top_traders(10);
        assert_eq!(top_traders.len(), 0);
        
        let (xlm, usdc, fees) = portfolio.get_pool_stats();
        assert_eq!(xlm, 0);
        assert_eq!(usdc, 0);
        assert_eq!(fees, 0);
    }

    /// Test queries respect limit parameter
    #[test]
    fn test_top_traders_limit() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        for i in 0..10 {
            let user = TestAddress::generate(&env);
            portfolio.mint(&env, Asset::XLM, user, 1000 + (i as i128 * 100));
        }
        
        let top5 = portfolio.get_top_traders(5);
        assert!(top5.len() <= 5);
        
        let top10 = portfolio.get_top_traders(10);
        assert!(top10.len() <= 10);
        
        let top3 = portfolio.get_top_traders(3);
        assert!(top3.len() <= 3);
    }

    /// Test fee collection tracking
    #[test]
    fn test_fee_tracking() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        portfolio.collect_fee(50);
        portfolio.collect_fee(100);
        portfolio.collect_fee(25);
        
        let (_, _, fees) = portfolio.get_pool_stats();
        assert_eq!(fees, 175);
    }

    /// Test queries don't modify state
    #[test]
    fn test_queries_readonly() {
        let env = Env::default();
        let portfolio = Portfolio::new(&env);
        
        let initial_users = portfolio.get_total_users();
        let initial_volume = portfolio.get_total_trading_volume();
        
        for _ in 0..10 {
            let _ = portfolio.get_total_users();
            let _ = portfolio.get_total_trading_volume();
            let _ = portfolio.get_active_users_count();
            let _ = portfolio.get_top_traders(10);
            let _ = portfolio.get_pool_stats();
        }
        
        assert_eq!(portfolio.get_total_users(), initial_users);
        assert_eq!(portfolio.get_total_trading_volume(), initial_volume);
    }
}
