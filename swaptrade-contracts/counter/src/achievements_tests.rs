/// Comprehensive tests for Badge & Achievement System
/// Tests all 6 badge types, unlock conditions, progress tracking, and progression

#[cfg(test)]
mod badge_achievement_tests {
    use crate::portfolio::{Portfolio, Asset, Badge};
    use soroban_sdk::{Env, testutils::Address as TestAddress, Symbol};

    // ===== INDIVIDUAL BADGE UNLOCK TESTS =====

    /// Test FirstTrade badge unlocks at 1+ trade
    #[test]
    fn test_first_trade_badge_at_one_trade() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        // No badges initially
        assert!(!portfolio.has_badge(&env, user.clone(), Badge::FirstTrade));
        
        // Record first trade
        portfolio.record_trade(&env, user.clone());
        
        // FirstTrade badge should be awarded
        assert!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade));
    }

    /// Test Trader badge unlocks at exactly 10 trades
    #[test]
    fn test_trader_badge_at_ten_trades() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        // Mint starting balance for tracking
        portfolio.mint(&env, Asset::XLM, user.clone(), 1000);
        portfolio.record_initial_balance(user.clone(), 1000);
        
        // Record 9 trades - no Trader badge yet
        for _ in 0..9 {
            portfolio.record_trade(&env, user.clone());
        }
        portfolio.check_and_award_badges(&env, user.clone());
        assert!(!portfolio.has_badge(&env, user.clone(), Badge::Trader));
        
        // Record 10th trade
        portfolio.record_trade(&env, user.clone());
        portfolio.check_and_award_badges(&env, user.clone());
        
        // Trader badge should now be awarded
        assert!(portfolio.has_badge(&env, user.clone(), Badge::Trader));
    }

    /// Test WealthBuilder badge unlocks at 10x starting balance
    #[test]
    fn test_wealth_builder_badge_at_10x_balance() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        let starting_balance = 100i128;
        portfolio.record_initial_balance(user.clone(), starting_balance);
        
        // Create initial balance via mint
        portfolio.mint(&env, Asset::XLM, user.clone(), starting_balance);
        portfolio.check_and_award_badges(&env, user.clone());
        assert!(!portfolio.has_badge(&env, user.clone(), Badge::WealthBuilder));
        
        // Add more tokens to reach 10x
        portfolio.mint(&env, Asset::XLM, user.clone(), starting_balance * 9);
        portfolio.check_and_award_badges(&env, user.clone());
        
        // WealthBuilder badge should be awarded
        assert!(portfolio.has_badge(&env, user.clone(), Badge::WealthBuilder));
    }

    /// Test LiquidityProvider badge unlocks at 1+ LP deposit
    #[test]
    fn test_liquidity_provider_badge_at_one_deposit() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        // No LP badge initially
        assert!(!portfolio.has_badge(&env, user.clone(), Badge::LiquidityProvider));
        
        // Record LP deposit
        portfolio.record_lp_deposit(user.clone());
        portfolio.check_and_award_badges(&env, user.clone());
        
        // LiquidityProvider badge should be awarded
        assert!(portfolio.has_badge(&env, user.clone(), Badge::LiquidityProvider));
    }

    /// Test Diversifier badge unlocks at 5+ different token pairs
    #[test]
    fn test_diversifier_badge_at_five_pairs() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        // Mint initial tokens
        portfolio.mint(&env, Asset::XLM, user.clone(), 5000);
        
        // Record trades with different token pairs
        let token1 = soroban_sdk::symbol_short!("USD");
        let token2 = soroban_sdk::symbol_short!("EUR");
        let token3 = soroban_sdk::symbol_short!("GBP");
        let token4 = soroban_sdk::symbol_short!("JPY");
        let token5 = soroban_sdk::symbol_short!("CHF");
        
        // Track 4 different pairs - no Diversifier badge yet
        for i in 0..4 {
            let to_token = match i {
                0 => token1.clone(),
                1 => token2.clone(),
                2 => token3.clone(),
                _ => token4.clone(),
            };
            portfolio.track_trade_for_badges(&env, user.clone(), soroban_sdk::symbol_short!("XLM"), to_token, 100 + (i as u64));
        }
        portfolio.check_and_award_badges(&env, user.clone());
        assert!(!portfolio.has_badge(&env, user.clone(), Badge::Diversifier));
        
        // Track 5th different pair
        portfolio.track_trade_for_badges(&env, user.clone(), soroban_sdk::symbol_short!("XLM"), token5.clone(), 104);
        portfolio.check_and_award_badges(&env, user.clone());
        
        // Diversifier badge should be awarded
        assert!(portfolio.has_badge(&env, user.clone(), Badge::Diversifier));
    }

    /// Test Consistency badge unlocks at 7+ different ledger heights
    #[test]
    fn test_consistency_badge_at_seven_heights() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        // Mint initial tokens
        portfolio.mint(&env, Asset::XLM, user.clone(), 5000);
        
        let xlm = soroban_sdk::symbol_short!("XLM");
        let usdc = soroban_sdk::symbol_short!("USD");
        
        // Trade at 6 different ledger heights - no Consistency badge yet
        for i in 0..6 {
            portfolio.track_trade_for_badges(&env, user.clone(), xlm.clone(), usdc.clone(), 1000 + (i as u64));
        }
        portfolio.check_and_award_badges(&env, user.clone());
        assert!(!portfolio.has_badge(&env, user.clone(), Badge::Consistency));
        
        // Trade at 7th different ledger height
        portfolio.track_trade_for_badges(&env, user.clone(), xlm.clone(), usdc.clone(), 1006);
        portfolio.check_and_award_badges(&env, user.clone());
        
        // Consistency badge should be awarded
        assert!(portfolio.has_badge(&env, user.clone(), Badge::Consistency));
    }

    // ===== PROGRESS TRACKING TESTS =====

    /// Test progress tracking shows intermediate numbers
    #[test]
    fn test_badge_progress_tracking() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        portfolio.mint(&env, Asset::XLM, user.clone(), 1000);
        portfolio.record_initial_balance(user.clone(), 1000);
        
        // Record 3 trades
        for _ in 0..3 {
            portfolio.record_trade(&env, user.clone());
        }
        
        // Get progress
        let progress = portfolio.get_badge_progress(&env, user.clone());
        
        // Check Trader badge progress (should show 3/10)
        let mut found_trader_progress = false;
        for (badge, current, target) in progress.iter() {
            if badge == Badge::Trader {
                assert_eq!(current, 3);
                assert_eq!(target, 10);
                found_trader_progress = true;
            }
        }
        assert!(found_trader_progress);
    }

    /// Test progress tracking for all badges
    #[test]
    fn test_all_badge_progress_returned() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        let progress = portfolio.get_badge_progress(&env, user.clone());
        
        // Should return progress for all 6 badges
        assert_eq!(progress.len(), 6);
        
        // Verify all badge types are present
        let mut has_first_trade = false;
        let mut has_trader = false;
        let mut has_wealth_builder = false;
        let mut has_liquidity_provider = false;
        let mut has_diversifier = false;
        let mut has_consistency = false;
        
        for (badge, _, _) in progress.iter() {
            match badge {
                Badge::FirstTrade => has_first_trade = true,
                Badge::Trader => has_trader = true,
                Badge::WealthBuilder => has_wealth_builder = true,
                Badge::LiquidityProvider => has_liquidity_provider = true,
                Badge::Diversifier => has_diversifier = true,
                Badge::Consistency => has_consistency = true,
            }
        }
        
        assert!(has_first_trade);
        assert!(has_trader);
        assert!(has_wealth_builder);
        assert!(has_liquidity_provider);
        assert!(has_diversifier);
        assert!(has_consistency);
    }

    // ===== BADGE INDEPENDENCE TESTS =====

    /// Test that earning one badge doesn't prevent earning others
    #[test]
    fn test_badge_conditions_independent() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        portfolio.mint(&env, Asset::XLM, user.clone(), 1000);
        portfolio.record_initial_balance(user.clone(), 1000);
        
        // Earn FirstTrade badge
        portfolio.record_trade(&env, user.clone());
        assert!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade));
        
        // Can still earn other badges
        portfolio.record_lp_deposit(user.clone());
        portfolio.check_and_award_badges(&env, user.clone());
        assert!(portfolio.has_badge(&env, user.clone(), Badge::LiquidityProvider));
        
        // Can still earn Trader badge
        for _ in 0..9 {
            portfolio.record_trade(&env, user.clone());
        }
        portfolio.check_and_award_badges(&env, user.clone());
        assert!(portfolio.has_badge(&env, user.clone(), Badge::Trader));
        
        // All three badges earned
        let badges = portfolio.get_user_badges(&env, user.clone());
        assert_eq!(badges.len(), 3);
    }

    // ===== MULTI-BADGE PROGRESSION TESTS =====

    /// Integration test: 10 trades → multiple badges earned in sequence
    #[test]
    fn test_10_trades_progression_earning_multiple_badges() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        portfolio.mint(&env, Asset::XLM, user.clone(), 1000);
        portfolio.record_initial_balance(user.clone(), 1000);
        
        // Record trades and check badges at each milestone
        for trade_num in 1..=10 {
            portfolio.record_trade(&env, user.clone());
            portfolio.check_and_award_badges(&env, user.clone());
            
            let badges = portfolio.get_user_badges(&env, user.clone());
            
            match trade_num {
                1 => {
                    // At 1 trade: should have FirstTrade
                    assert!(badges.len() >= 1);
                    assert!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade));
                }
                10 => {
                    // At 10 trades: should have FirstTrade + Trader
                    assert!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade));
                    assert!(portfolio.has_badge(&env, user.clone(), Badge::Trader));
                }
                _ => {}
            }
        }
        
        // Verify final state: at least 2 badges (FirstTrade + Trader)
        let final_badges = portfolio.get_user_badges(&env, user.clone());
        assert!(final_badges.len() >= 2);
    }

    /// Test no duplicate badges awarded
    #[test]
    fn test_no_duplicate_badges() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        portfolio.mint(&env, Asset::XLM, user.clone(), 1000);
        portfolio.record_initial_balance(user.clone(), 1000);
        
        // Record 15 trades (exceeds 10-trade threshold multiple times)
        for _ in 0..15 {
            portfolio.record_trade(&env, user.clone());
            portfolio.check_and_award_badges(&env, user.clone());
        }
        
        // Get all badges
        let badges = portfolio.get_user_badges(&env, user.clone());
        
        // Count Trader badges (should only appear once)
        let mut trader_count = 0;
        for badge in badges.iter() {
            if badge == Badge::Trader {
                trader_count += 1;
            }
        }
        assert_eq!(trader_count, 1); // Should appear exactly once
    }

    /// Test complex progression with multiple badge types
    #[test]
    fn test_complex_progression_multiple_badge_types() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        let starting = 100i128;
        portfolio.mint(&env, Asset::XLM, user.clone(), starting);
        portfolio.record_initial_balance(user.clone(), starting);
        
        // Progress phase 1: First trade + LP deposit
        portfolio.record_trade(&env, user.clone());
        portfolio.record_lp_deposit(user.clone());
        portfolio.check_and_award_badges(&env, user.clone());
        
        assert!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade));
        assert!(portfolio.has_badge(&env, user.clone(), Badge::LiquidityProvider));
        
        // Progress phase 2: 10 trades total
        for _ in 0..9 {
            portfolio.record_trade(&env, user.clone());
        }
        portfolio.check_and_award_badges(&env, user.clone());
        
        assert!(portfolio.has_badge(&env, user.clone(), Badge::Trader));
        
        // Progress phase 3: Multiple token pairs
        let xlm = soroban_sdk::symbol_short!("XLM");
        let usd = soroban_sdk::symbol_short!("USD");
        let eur = soroban_sdk::symbol_short!("EUR");
        let gbp = soroban_sdk::symbol_short!("GBP");
        let jpy = soroban_sdk::symbol_short!("JPY");
        let chf = soroban_sdk::symbol_short!("CHF");
        
        portfolio.track_trade_for_badges(&env, user.clone(), xlm.clone(), usd.clone(), 1000);
        portfolio.track_trade_for_badges(&env, user.clone(), xlm.clone(), eur.clone(), 1001);
        portfolio.track_trade_for_badges(&env, user.clone(), xlm.clone(), gbp.clone(), 1002);
        portfolio.track_trade_for_badges(&env, user.clone(), xlm.clone(), jpy.clone(), 1003);
        portfolio.track_trade_for_badges(&env, user.clone(), xlm.clone(), chf.clone(), 1004);
        portfolio.check_and_award_badges(&env, user.clone());
        
        assert!(portfolio.has_badge(&env, user.clone(), Badge::Diversifier));
        
        // Progress phase 4: Different ledger heights
        for i in 5..12 {
            portfolio.track_trade_for_badges(&env, user.clone(), xlm.clone(), usd.clone(), 2000 + (i as u64));
        }
        portfolio.check_and_award_badges(&env, user.clone());
        
        assert!(portfolio.has_badge(&env, user.clone(), Badge::Consistency));
        
        // Progress phase 5: Wealth building
        portfolio.mint(&env, Asset::XLM, user.clone(), starting * 9);
        portfolio.check_and_award_badges(&env, user.clone());
        
        assert!(portfolio.has_badge(&env, user.clone(), Badge::WealthBuilder));
        
        // Verify all 6 badges earned
        let all_badges = portfolio.get_user_badges(&env, user.clone());
        assert_eq!(all_badges.len(), 6);
    }

    // ===== PERSISTENCE TESTS =====

    /// Test badge awards persist after multiple check calls
    #[test]
    fn test_badge_persistence_across_checks() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        let user = TestAddress::generate(&env);
        
        portfolio.mint(&env, Asset::XLM, user.clone(), 1000);
        portfolio.record_initial_balance(user.clone(), 1000);
        
        // Award badge
        portfolio.record_trade(&env, user.clone());
        portfolio.check_and_award_badges(&env, user.clone());
        assert!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade));
        
        // Check multiple times - badge should persist
        for _ in 0..5 {
            portfolio.check_and_award_badges(&env, user.clone());
            assert!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade));
        }
    }

    /// Test independent user badge tracking
    #[test]
    fn test_badge_isolation_between_users() {
        let env = Env::default();
        let mut portfolio = Portfolio::new(&env);
        
        let user1 = TestAddress::generate(&env);
        let user2 = TestAddress::generate(&env);
        
        // User1 gets FirstTrade badge
        portfolio.record_trade(&env, user1.clone());
        
        // User2 should not have it
        assert!(portfolio.has_badge(&env, user1.clone(), Badge::FirstTrade));
        assert!(!portfolio.has_badge(&env, user2.clone(), Badge::FirstTrade));
        
        // User2 gets LP badge
        portfolio.record_lp_deposit(user2.clone());
        portfolio.check_and_award_badges(&env, user2.clone());
        
        // User1 should not have it
        assert!(!portfolio.has_badge(&env, user1.clone(), Badge::LiquidityProvider));
        assert!(portfolio.has_badge(&env, user2.clone(), Badge::LiquidityProvider));
    }
}
