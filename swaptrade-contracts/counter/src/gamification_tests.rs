//! Comprehensive tests for Dynamic Gamification & Rewards System
//!
//! Tests cover:
//! - Achievement badge issuance and uniqueness
//! - Leaderboard accuracy and pagination
//! - Dynamic scoring calculations
//! - Streak tracking and rewards
//! - Challenge campaigns lifecycle
//! - Progression tier advancement
//! - Reward distribution and claiming
//! - Edge cases and error conditions

#[cfg(test)]
mod gamification_tests {
    use crate::gamification::{
        AchievementBadge, ChallengeCampaign, ChallengeObjective, ChallengeProgress,
        ChallengeStatus, GamificationEngine, GamificationError, GamificationGlobalStats,
        GamificationProfile, LeaderboardEntry, LeaderboardMetric, ProgressionTier,
        RewardDistribution, ScoreBreakdown,
    };
    use soroban_sdk::{
        testutils::{Address as _, Events as _},
        Address, Env, Symbol, Vec,
    };

    // ════════════════════════════════════════════════════════════════════════
    // Helper Functions
    // ════════════════════════════════════════════════════════════════════════

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let user = Address::generate(&env);
        (env, user)
    }

    // ════════════════════════════════════════════════════════════════════════
    // Profile Management Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_get_or_create_profile_creates_new() {
        let (env, user) = setup();

        let profile = GamificationEngine::get_or_create_profile(&env, &user);

        assert_eq!(profile.tier, ProgressionTier::Bronze);
        assert_eq!(profile.score, 0);
        assert_eq!(profile.lifetime_score, 0);
        assert_eq!(profile.current_trade_streak, 0);
        assert_eq!(profile.best_trade_streak, 0);
        assert_eq!(profile.current_daily_streak, 0);
        assert_eq!(profile.best_daily_streak, 0);
        assert_eq!(profile.challenges_won, 0);
        assert_eq!(profile.challenges_participated, 0);
        assert_eq!(profile.total_rewards_earned, 0);
        assert_eq!(profile.pending_rewards, 0);
    }

    #[test]
    fn test_get_or_create_profile_returns_existing() {
        let (env, user) = setup();

        let mut profile = GamificationEngine::get_or_create_profile(&env, &user);
        profile.score = 100;
        GamificationEngine::save_profile(&env, &user, &profile);

        let loaded = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(loaded.score, 100);
    }

    #[test]
    fn test_save_and_load_profile_persists() {
        let (env, user) = setup();

        let mut profile = GamificationEngine::get_or_create_profile(&env, &user);
        profile.score = 500;
        profile.tier = ProgressionTier::Silver;
        profile.current_trade_streak = 5;
        profile.best_trade_streak = 10;
        GamificationEngine::save_profile(&env, &user, &profile);

        let loaded = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(loaded.score, 500);
        assert_eq!(loaded.tier, ProgressionTier::Silver);
        assert_eq!(loaded.current_trade_streak, 5);
        assert_eq!(loaded.best_trade_streak, 10);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Achievement Badge Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_award_badge_first_trade() {
        let (env, user) = setup();

        let awarded = GamificationEngine::award_badge(&env, &user, AchievementBadge::FirstTrade);

        assert!(awarded);
        assert!(GamificationEngine::has_badge(&env, &user, &AchievementBadge::FirstTrade));
    }

    #[test]
    fn test_award_badge_no_duplicates() {
        let (env, user) = setup();

        GamificationEngine::award_badge(&env, &user, AchievementBadge::FirstTrade);
        let second_award = GamificationEngine::award_badge(&env, &user, AchievementBadge::FirstTrade);

        assert!(!second_award);
    }

    #[test]
    fn test_get_user_badges_initially_empty() {
        let (env, user) = setup();

        let badges = GamificationEngine::get_user_badges(&env, &user);
        assert_eq!(badges.len(), 0);
    }

    #[test]
    fn test_get_user_badges_after_awards() {
        let (env, user) = setup();

        GamificationEngine::award_badge(&env, &user, AchievementBadge::FirstTrade);
        GamificationEngine::award_badge(&env, &user, AchievementBadge::TenTrades);
        GamificationEngine::award_badge(&env, &user, AchievementBadge::LPProvider);

        let badges = GamificationEngine::get_user_badges(&env, &user);
        assert_eq!(badges.len(), 3);
    }

    #[test]
    fn test_has_badge_returns_false_when_not_earned() {
        let (env, user) = setup();

        assert!(!GamificationEngine::has_badge(&env, &user, &AchievementBadge::FirstTrade));
    }

    #[test]
    fn test_badge_awards_points() {
        let (env, user) = setup();

        GamificationEngine::award_badge(&env, &user, AchievementBadge::FirstTrade);

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        // FirstTrade badge awards 50 points
        assert_eq!(profile.score, 50);
        assert_eq!(profile.lifetime_score, 50);
    }

    #[test]
    fn test_multiple_badges_accumulate_points() {
        let (env, user) = setup();

        GamificationEngine::award_badge(&env, &user, AchievementBadge::FirstTrade); // 50
        GamificationEngine::award_badge(&env, &user, AchievementBadge::TenTrades); // 100
        GamificationEngine::award_badge(&env, &user, AchievementBadge::LPProvider); // 100

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(profile.score, 250);
        assert_eq!(profile.lifetime_score, 250);
    }

    #[test]
    fn test_badge_isolation_between_users() {
        let (env, user1) = setup();
        let user2 = Address::generate(&env);

        GamificationEngine::award_badge(&env, &user1, AchievementBadge::FirstTrade);

        assert!(GamificationEngine::has_badge(&env, &user1, &AchievementBadge::FirstTrade));
        assert!(!GamificationEngine::has_badge(&env, &user2, &AchievementBadge::FirstTrade));
    }

    // ════════════════════════════════════════════════════════════════════════
    // Dynamic Scoring Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_add_score_increases_profile() {
        let (env, user) = setup();

        GamificationEngine::add_score(&env, &user, 100, symbol_short!("test"));

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(profile.score, 100);
        assert_eq!(profile.lifetime_score, 100);
    }

    #[test]
    fn test_add_score_negative_ignored() {
        let (env, user) = setup();

        GamificationEngine::add_score(&env, &user, -100, symbol_short!("test"));

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(profile.score, 0);
    }

    #[test]
    fn test_add_score_zero_ignored() {
        let (env, user) = setup();

        GamificationEngine::add_score(&env, &user, 0, symbol_short!("test"));

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(profile.score, 0);
    }

    #[test]
    fn test_calculate_trading_score_basic() {
        let score = GamificationEngine::calculate_trading_score(10, 7, 500, 10000);
        assert!(score > 0);
    }

    #[test]
    fn test_calculate_trading_score_no_trades() {
        let score = GamificationEngine::calculate_trading_score(0, 0, 0, 0);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_calculate_trading_score_higher_with_more_trades() {
        let score_low = GamificationEngine::calculate_trading_score(5, 3, 100, 1000);
        let score_high = GamificationEngine::calculate_trading_score(50, 30, 5000, 50000);
        assert!(score_high > score_low);
    }

    #[test]
    fn test_calculate_trading_score_higher_with_better_win_rate() {
        let score_low = GamificationEngine::calculate_trading_score(20, 10, 100, 10000);
        let score_high = GamificationEngine::calculate_trading_score(20, 18, 100, 10000);
        assert!(score_high > score_low);
    }

    #[test]
    fn test_calculate_composite_score_weighted() {
        let score = GamificationEngine::calculate_composite_score(100, 100, 100, 100, 100, 100);
        // All equal: (100*40 + 100*25 + 100*15 + 100*10 + 100*5 + 100*5) / 100 = 100
        assert_eq!(score, 100);
    }

    #[test]
    fn test_calculate_composite_score_trading_weighted() {
        let score = GamificationEngine::calculate_composite_score(200, 0, 0, 0, 0, 0);
        // (200*40) / 100 = 80
        assert_eq!(score, 80);
    }

    #[test]
    fn test_get_score_breakdown_default() {
        let (env, user) = setup();

        let breakdown = GamificationEngine::get_score_breakdown(&env, &user);
        assert_eq!(breakdown.trading_score, 0);
        assert_eq!(breakdown.performance_score, 0);
        assert_eq!(breakdown.learning_score, 0);
        assert_eq!(breakdown.community_score, 0);
        assert_eq!(breakdown.streak_bonus, 0);
        assert_eq!(breakdown.challenge_bonus, 0);
    }

    #[test]
    fn test_save_score_breakdown() {
        let (env, user) = setup();

        let breakdown = ScoreBreakdown {
            trading_score: 100,
            performance_score: 50,
            learning_score: 25,
            community_score: 10,
            streak_bonus: 5,
            challenge_bonus: 15,
        };
        GamificationEngine::save_score_breakdown(&env, &user, &breakdown);

        let loaded = GamificationEngine::get_score_breakdown(&env, &user);
        assert_eq!(loaded.trading_score, 100);
        assert_eq!(loaded.performance_score, 50);
        assert_eq!(loaded.learning_score, 25);
        assert_eq!(loaded.community_score, 10);
        assert_eq!(loaded.streak_bonus, 5);
        assert_eq!(loaded.challenge_bonus, 15);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Progression Tier Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_calculate_tier_bronze() {
        assert_eq!(GamificationEngine::calculate_tier(0), ProgressionTier::Bronze);
        assert_eq!(GamificationEngine::calculate_tier(999), ProgressionTier::Bronze);
    }

    #[test]
    fn test_calculate_tier_silver() {
        assert_eq!(GamificationEngine::calculate_tier(1000), ProgressionTier::Silver);
        assert_eq!(GamificationEngine::calculate_tier(4999), ProgressionTier::Silver);
    }

    #[test]
    fn test_calculate_tier_gold() {
        assert_eq!(GamificationEngine::calculate_tier(5000), ProgressionTier::Gold);
        assert_eq!(GamificationEngine::calculate_tier(19999), ProgressionTier::Gold);
    }

    #[test]
    fn test_calculate_tier_platinum() {
        assert_eq!(
            GamificationEngine::calculate_tier(20000),
            ProgressionTier::Platinum
        );
        assert_eq!(
            GamificationEngine::calculate_tier(99999),
            ProgressionTier::Platinum
        );
    }

    #[test]
    fn test_calculate_tier_diamond() {
        assert_eq!(
            GamificationEngine::calculate_tier(100000),
            ProgressionTier::Diamond
        );
        assert_eq!(
            GamificationEngine::calculate_tier(1_000_000),
            ProgressionTier::Diamond
        );
    }

    #[test]
    fn test_tier_progression_on_score_add() {
        let (env, user) = setup();

        // Start at Bronze
        let tier = GamificationEngine::get_user_tier(&env, &user);
        assert_eq!(tier, ProgressionTier::Bronze);

        // Add enough to reach Silver
        GamificationEngine::add_score(&env, &user, 1000, symbol_short!("test"));
        let tier = GamificationEngine::get_user_tier(&env, &user);
        assert_eq!(tier, ProgressionTier::Silver);

        // Add more to reach Gold
        GamificationEngine::add_score(&env, &user, 4000, symbol_short!("test"));
        let tier = GamificationEngine::get_user_tier(&env, &user);
        assert_eq!(tier, ProgressionTier::Gold);
    }

    #[test]
    fn test_tier_progress_tuple() {
        let (env, user) = setup();

        GamificationEngine::add_score(&env, &user, 2500, symbol_short!("test"));

        let (current, next) = GamificationEngine::get_tier_progress(&env, &user);
        assert_eq!(current, 2500);
        assert_eq!(next, 5000); // Gold threshold
    }

    #[test]
    fn test_tier_progress_at_max() {
        let (env, user) = setup();

        GamificationEngine::add_score(&env, &user, 150_000, symbol_short!("test"));

        let (current, next) = GamificationEngine::get_tier_progress(&env, &user);
        // At Diamond (max), next == current
        assert_eq!(current, 150_000);
        assert_eq!(next, 150_000);
    }

    #[test]
    fn test_tier_next_tier() {
        assert_eq!(
            ProgressionTier::Bronze.next_tier(),
            Some(ProgressionTier::Silver)
        );
        assert_eq!(
            ProgressionTier::Silver.next_tier(),
            Some(ProgressionTier::Gold)
        );
        assert_eq!(
            ProgressionTier::Gold.next_tier(),
            Some(ProgressionTier::Platinum)
        );
        assert_eq!(
            ProgressionTier::Platinum.next_tier(),
            Some(ProgressionTier::Diamond)
        );
        assert_eq!(ProgressionTier::Diamond.next_tier(), None);
    }

    #[test]
    fn test_tier_reward_multiplier_increases() {
        let bronze_mult = ProgressionTier::Bronze.reward_multiplier_bps();
        let silver_mult = ProgressionTier::Silver.reward_multiplier_bps();
        let gold_mult = ProgressionTier::Gold.reward_multiplier_bps();
        let plat_mult = ProgressionTier::Platinum.reward_multiplier_bps();
        let diamond_mult = ProgressionTier::Diamond.reward_multiplier_bps();

        assert!(silver_mult > bronze_mult);
        assert!(gold_mult > silver_mult);
        assert!(plat_mult > gold_mult);
        assert!(diamond_mult > plat_mult);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Streak Tracking Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_record_profitable_trade_increments_streak() {
        let (env, user) = setup();

        GamificationEngine::record_profitable_trade(&env, &user);
        let (current, best) = GamificationEngine::get_trading_streak(&env, &user);
        assert_eq!(current, 1);
        assert_eq!(best, 1);
    }

    #[test]
    fn test_record_profitable_trade_accumulates() {
        let (env, user) = setup();

        for _ in 0..5 {
            GamificationEngine::record_profitable_trade(&env, &user);
        }
        let (current, best) = GamificationEngine::get_trading_streak(&env, &user);
        assert_eq!(current, 5);
        assert_eq!(best, 5);
    }

    #[test]
    fn test_record_losing_trade_resets_streak() {
        let (env, user) = setup();

        for _ in 0..5 {
            GamificationEngine::record_profitable_trade(&env, &user);
        }
        GamificationEngine::record_losing_trade(&env, &user);

        let (current, best) = GamificationEngine::get_trading_streak(&env, &user);
        assert_eq!(current, 0);
        assert_eq!(best, 5); // Best preserved
    }

    #[test]
    fn test_streak_milestone_awards_points() {
        let (env, user) = setup();

        // 5 streak awards 25 points
        for _ in 0..5 {
            GamificationEngine::record_profitable_trade(&env, &user);
        }

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert!(profile.score >= 25);
    }

    #[test]
    fn test_streak_multiplier_increases() {
        let (env, user) = setup();

        // Base multiplier is 10000 (1.0x)
        let mult_0 = GamificationEngine::streak_multiplier_bps(&env, &user);
        assert_eq!(mult_0, 10_000);

        // After 5 trades, multiplier increases
        for _ in 0..5 {
            GamificationEngine::record_profitable_trade(&env, &user);
        }
        let mult_5 = GamificationEngine::streak_multiplier_bps(&env, &user);
        assert!(mult_5 > mult_0);
    }

    #[test]
    fn test_streak_multiplier_capped() {
        let (env, user) = setup();

        // 50 streak should cap at 20000 (2.0x)
        for _ in 0..50 {
            GamificationEngine::record_profitable_trade(&env, &user);
        }
        let mult = GamificationEngine::streak_multiplier_bps(&env, &user);
        assert!(mult <= 20_000);
    }

    #[test]
    fn test_best_streak_preserved_after_break() {
        let (env, user) = setup();

        for _ in 0..10 {
            GamificationEngine::record_profitable_trade(&env, &user);
        }
        GamificationEngine::record_losing_trade(&env, &user);

        let (_, best) = GamificationEngine::get_trading_streak(&env, &user);
        assert_eq!(best, 10);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Daily Streak Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_record_activity_first_time() {
        let (env, user) = setup();

        GamificationEngine::record_activity(&env, &user);

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(profile.current_daily_streak, 1);
        assert!(profile.last_activity_timestamp > 0);
    }

    #[test]
    fn test_daily_streak_getters() {
        let (env, user) = setup();

        let (current, best) = GamificationEngine::get_daily_streak(&env, &user);
        assert_eq!(current, 0);
        assert_eq!(best, 0);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Leaderboard Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_update_leaderboard_entry() {
        let (env, user) = setup();

        GamificationEngine::update_leaderboard_entry(
            &env,
            &user,
            &LeaderboardMetric::ByScore,
            100,
        );

        let leaderboard =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 0, 10);
        assert_eq!(leaderboard.len(), 1);
        assert_eq!(leaderboard.get(0).unwrap().score, 100);
        assert_eq!(leaderboard.get(0).unwrap().rank, 1);
    }

    #[test]
    fn test_leaderboard_sorting_descending() {
        let (env, _) = setup();
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let user3 = Address::generate(&env);

        GamificationEngine::update_leaderboard_entry(
            &env,
            &user1,
            &LeaderboardMetric::ByScore,
            50,
        );
        GamificationEngine::update_leaderboard_entry(
            &env,
            &user2,
            &LeaderboardMetric::ByScore,
            200,
        );
        GamificationEngine::update_leaderboard_entry(
            &env,
            &user3,
            &LeaderboardMetric::ByScore,
            100,
        );

        let leaderboard =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 0, 10);

        assert_eq!(leaderboard.len(), 3);
        assert_eq!(leaderboard.get(0).unwrap().score, 200); // user2 first
        assert_eq!(leaderboard.get(0).unwrap().rank, 1);
        assert_eq!(leaderboard.get(1).unwrap().score, 100); // user3 second
        assert_eq!(leaderboard.get(1).unwrap().rank, 2);
        assert_eq!(leaderboard.get(2).unwrap().score, 50); // user1 third
        assert_eq!(leaderboard.get(2).unwrap().rank, 3);
    }

    #[test]
    fn test_leaderboard_update_existing_entry() {
        let (env, user) = setup();

        GamificationEngine::update_leaderboard_entry(
            &env,
            &user,
            &LeaderboardMetric::ByScore,
            100,
        );
        GamificationEngine::update_leaderboard_entry(
            &env,
            &user,
            &LeaderboardMetric::ByScore,
            250,
        );

        let leaderboard =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 0, 10);
        assert_eq!(leaderboard.len(), 1);
        assert_eq!(leaderboard.get(0).unwrap().score, 250);
    }

    #[test]
    fn test_leaderboard_pagination() {
        let (env, _) = setup();

        // Add 15 entries
        for i in 0..15 {
            let user = Address::generate(&env);
            GamificationEngine::update_leaderboard_entry(
                &env,
                &user,
                &LeaderboardMetric::ByScore,
                (i + 1) * 10,
            );
        }

        // Page 0, size 5
        let page0 =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 0, 5);
        assert_eq!(page0.len(), 5);

        // Page 1, size 5
        let page1 =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 1, 5);
        assert_eq!(page1.len(), 5);

        // Page 2, size 5
        let page2 =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 2, 5);
        assert_eq!(page2.len(), 5);

        // Page 3 (beyond data)
        let page3 =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 3, 5);
        assert_eq!(page3.len(), 0);
    }

    #[test]
    fn test_leaderboard_size() {
        let (env, _) = setup();

        assert_eq!(
            GamificationEngine::get_leaderboard_size(&env, &LeaderboardMetric::ByScore),
            0
        );

        let user = Address::generate(&env);
        GamificationEngine::update_leaderboard_entry(
            &env,
            &user,
            &LeaderboardMetric::ByScore,
            100,
        );

        assert_eq!(
            GamificationEngine::get_leaderboard_size(&env, &LeaderboardMetric::ByScore),
            1
        );
    }

    #[test]
    fn test_user_rank_on_leaderboard() {
        let (env, user) = setup();
        let other = Address::generate(&env);

        GamificationEngine::update_leaderboard_entry(
            &env,
            &other,
            &LeaderboardMetric::ByScore,
            200,
        );
        GamificationEngine::update_leaderboard_entry(
            &env,
            &user,
            &LeaderboardMetric::ByScore,
            100,
        );

        let rank = GamificationEngine::get_user_rank(&env, &user, &LeaderboardMetric::ByScore);
        assert_eq!(rank, Some(2));
    }

    #[test]
    fn test_user_not_on_leaderboard() {
        let (env, user) = setup();

        let rank = GamificationEngine::get_user_rank(&env, &user, &LeaderboardMetric::ByScore);
        assert_eq!(rank, None);
    }

    #[test]
    fn test_leaderboard_capped_at_100() {
        let (env, _) = setup();

        // Add 110 entries
        for i in 0..110 {
            let user = Address::generate(&env);
            GamificationEngine::update_leaderboard_entry(
                &env,
                &user,
                &LeaderboardMetric::ByScore,
                (i + 1) as i128,
            );
        }

        let size =
            GamificationEngine::get_leaderboard_size(&env, &LeaderboardMetric::ByScore);
        assert_eq!(size, 100);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Challenge Campaign Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_create_challenge() {
        let (env, _) = setup();

        let result = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            86400, // 1 day
            10_000,
            100,
            ProgressionTier::Bronze,
        );

        assert!(result.is_ok());
        let challenge_id = result.unwrap();
        assert!(challenge_id > 0);
    }

    #[test]
    fn test_create_challenge_invalid_duration_too_short() {
        let (env, _) = setup();

        let result = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            100, // Too short (< 3600)
            10_000,
            100,
            ProgressionTier::Bronze,
        );

        assert_eq!(result.unwrap_err(), GamificationError::InvalidChallengeDuration);
    }

    #[test]
    fn test_create_challenge_invalid_duration_too_long() {
        let (env, _) = setup();

        let result = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            300_000_000, // Too long (> 2_592_000)
            10_000,
            100,
            ProgressionTier::Bronze,
        );

        assert_eq!(result.unwrap_err(), GamificationError::InvalidChallengeDuration);
    }

    #[test]
    fn test_join_challenge() {
        let (env, user) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        let result = GamificationEngine::join_challenge(&env, &user, challenge_id);
        assert!(result.is_ok());

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(profile.challenges_participated, 1);
    }

    #[test]
    fn test_join_challenge_not_found() {
        let (env, user) = setup();

        let result = GamificationEngine::join_challenge(&env, &user, 999);
        assert_eq!(result.unwrap_err(), GamificationError::ChallengeNotFound);
    }

    #[test]
    fn test_join_challenge_already_joined() {
        let (env, user) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        GamificationEngine::join_challenge(&env, &user, challenge_id).unwrap();
        let result = GamificationEngine::join_challenge(&env, &user, challenge_id);
        assert_eq!(result.unwrap_err(), GamificationError::AlreadyJoined);
    }

    #[test]
    fn test_update_challenge_progress() {
        let (env, user) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        GamificationEngine::join_challenge(&env, &user, challenge_id).unwrap();

        // Update progress
        let result =
            GamificationEngine::update_challenge_progress(&env, &user, challenge_id, 5);
        assert!(result.is_ok());

        let progress = GamificationEngine::get_challenge_progress(&env, &user, challenge_id)
            .unwrap();
        assert_eq!(progress.current_progress, 5);
        assert!(!progress.objective_met);
    }

    #[test]
    fn test_update_challenge_progress_completes() {
        let (env, user) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(5),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        GamificationEngine::join_challenge(&env, &user, challenge_id).unwrap();

        // Complete the objective
        let result =
            GamificationEngine::update_challenge_progress(&env, &user, challenge_id, 5);
        assert!(result.is_ok());

        let progress = GamificationEngine::get_challenge_progress(&env, &user, challenge_id)
            .unwrap();
        assert_eq!(progress.current_progress, 5);
        assert!(progress.objective_met);
        assert!(progress.completed_at.is_some());
    }

    #[test]
    fn test_update_challenge_progress_not_participant() {
        let (env, user) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        let result =
            GamificationEngine::update_challenge_progress(&env, &user, challenge_id, 5);
        assert_eq!(result.unwrap_err(), GamificationError::NotParticipant);
    }

    #[test]
    fn test_claim_challenge_reward() {
        let (env, user) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(3),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        GamificationEngine::join_challenge(&env, &user, challenge_id).unwrap();
        GamificationEngine::update_challenge_progress(&env, &user, challenge_id, 3).unwrap();

        let result = GamificationEngine::claim_challenge_reward(&env, &user, challenge_id);
        assert!(result.is_ok());

        let reward = result.unwrap();
        assert!(reward > 0);

        // Verify reward is pending
        let pending = GamificationEngine::get_pending_rewards(&env, &user);
        assert!(pending > 0);
    }

    #[test]
    fn test_claim_challenge_reward_not_completed() {
        let (env, user) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        GamificationEngine::join_challenge(&env, &user, challenge_id).unwrap();

        let result = GamificationEngine::claim_challenge_reward(&env, &user, challenge_id);
        assert_eq!(result.unwrap_err(), GamificationError::ObjectiveNotMet);
    }

    #[test]
    fn test_claim_challenge_reward_already_claimed() {
        let (env, user) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(3),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        GamificationEngine::join_challenge(&env, &user, challenge_id).unwrap();
        GamificationEngine::update_challenge_progress(&env, &user, challenge_id, 3).unwrap();
        GamificationEngine::claim_challenge_reward(&env, &user, challenge_id).unwrap();

        let result = GamificationEngine::claim_challenge_reward(&env, &user, challenge_id);
        assert_eq!(
            result.unwrap_err(),
            GamificationError::RewardAlreadyClaimed
        );
    }

    #[test]
    fn test_get_challenge_details() {
        let (env, _) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        let challenge = GamificationEngine::get_challenge(&env, challenge_id).unwrap();
        assert_eq!(challenge.challenge_id, challenge_id);
        assert_eq!(challenge.reward_pool, 10_000);
        assert_eq!(challenge.status, ChallengeStatus::Active);
        assert_eq!(challenge.max_participants, 100);
        assert_eq!(challenge.participant_count, 0);
    }

    #[test]
    fn test_get_active_challenges() {
        let (env, _) = setup();

        GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(10),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg2"),
            symbol_short!("desc2"),
            ChallengeObjective::TradeVolume(5000),
            86400,
            20_000,
            50,
            ProgressionTier::Silver,
        )
        .unwrap();

        let challenges = GamificationEngine::get_active_challenges(&env);
        assert_eq!(challenges.len(), 2);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Reward Distribution Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_claim_rewards() {
        let (env, user) = setup();

        // Manually set pending rewards
        let mut profile = GamificationEngine::get_or_create_profile(&env, &user);
        profile.pending_rewards = 500;
        GamificationEngine::save_profile(&env, &user, &profile);

        let claimed = GamificationEngine::claim_rewards(&env, &user).unwrap();
        assert_eq!(claimed, 500);

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(profile.pending_rewards, 0);
        assert_eq!(profile.total_rewards_earned, 500);
    }

    #[test]
    fn test_claim_rewards_none_pending() {
        let (env, user) = setup();

        let result = GamificationEngine::claim_rewards(&env, &user);
        assert_eq!(result.unwrap_err(), GamificationError::NoRewardsPending);
    }

    #[test]
    fn test_get_pending_rewards() {
        let (env, user) = setup();

        let pending = GamificationEngine::get_pending_rewards(&env, &user);
        assert_eq!(pending, 0);
    }

    #[test]
    fn test_get_lifetime_rewards() {
        let (env, user) = setup();

        let lifetime = GamificationEngine::get_lifetime_rewards(&env, &user);
        assert_eq!(lifetime, 0);
    }

    #[test]
    fn test_reward_history() {
        let (env, user) = setup();

        GamificationEngine::record_reward_distribution(
            &env,
            &user,
            100,
            symbol_short!("test"),
            None,
        );

        let history = GamificationEngine::get_reward_history(&env);
        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0).unwrap().amount, 100);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Global Stats Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_global_stats_default() {
        let (env, _) = setup();

        let stats = GamificationEngine::get_global_stats(&env);
        assert_eq!(stats.total_badges_awarded, 0);
        assert_eq!(stats.total_challenges_created, 0);
        assert_eq!(stats.total_challenges_completed, 0);
        assert_eq!(stats.total_rewards_distributed, 0);
        assert_eq!(stats.total_active_users, 0);
    }

    #[test]
    fn test_record_global_badge_award() {
        let (env, _) = setup();

        GamificationEngine::record_global_badge_award(&env);
        GamificationEngine::record_global_badge_award(&env);

        let stats = GamificationEngine::get_global_stats(&env);
        assert_eq!(stats.total_badges_awarded, 2);
    }

    #[test]
    fn test_record_global_challenge_completion() {
        let (env, _) = setup();

        GamificationEngine::record_global_challenge_completion(&env);

        let stats = GamificationEngine::get_global_stats(&env);
        assert_eq!(stats.total_challenges_completed, 1);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Achievement Check Integration Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_check_and_award_achievements_first_trade() {
        let (env, user) = setup();

        let awarded = GamificationEngine::check_and_award_achievements(
            &env,
            &user,
            1,  // trade_count
            1,  // winning_trades
            100, // pnl
            1,  // trade_streak
            1,  // daily_streak
            1,  // unique_pairs
            false, // lp_provided
            0,  // liquidity_amount
            0,  // referral_count
            0,  // challenges_won
        );

        assert!(awarded.len() > 0);
        assert!(GamificationEngine::has_badge(&env, &user, &AchievementBadge::FirstTrade));
    }

    #[test]
    fn test_check_and_award_achievements_ten_trades() {
        let (env, user) = setup();

        let awarded = GamificationEngine::check_and_award_achievements(
            &env,
            &user,
            10,  // trade_count
            8,  // winning_trades
            500, // pnl
            8,  // trade_streak
            5,  // daily_streak
            3,  // unique_pairs
            false, // lp_provided
            0,  // liquidity_amount
            0,  // referral_count
            0,  // challenges_won
        );

        assert!(GamificationEngine::has_badge(&env, &user, &AchievementBadge::FirstTrade));
        assert!(GamificationEngine::has_badge(&env, &user, &AchievementBadge::TenTrades));
    }

    #[test]
    fn test_check_and_award_achievements_lp_provider() {
        let (env, user) = setup();

        GamificationEngine::check_and_award_achievements(
            &env,
            &user,
            0,
            0,
            0,
            0,
            0,
            0,
            true,  // lp_provided
            100,
            0,
            0,
        );

        assert!(GamificationEngine::has_badge(&env, &user, &AchievementBadge::LPProvider));
    }

    #[test]
    fn test_check_and_award_achievements_diversified() {
        let (env, user) = setup();

        GamificationEngine::check_and_award_achievements(
            &env,
            &user,
            5,
            3,
            100,
            3,
            2,
            5, // unique_pairs >= 5
            false,
            0,
            0,
            0,
        );

        assert!(GamificationEngine::has_badge(
            &env,
            &user,
            &AchievementBadge::DiversifiedPortfolio
        ));
    }

    #[test]
    fn test_check_and_award_achievements_community() {
        let (env, user) = setup();

        GamificationEngine::check_and_award_achievements(
            &env,
            &user,
            0,
            0,
            0,
            0,
            0,
            0,
            false,
            0,
            3, // referral_count >= 3
            0,
        );

        assert!(GamificationEngine::has_badge(
            &env,
            &user,
            &AchievementBadge::CommunityContributor
        ));
    }

    #[test]
    fn test_check_and_award_achievements_master_trader() {
        let (env, user) = setup();

        GamificationEngine::check_and_award_achievements(
            &env,
            &user,
            100,   // trade_count >= 100
            15,    // winning_trades >= 10
            50000, // pnl >= 10M (scaled)
            10,    // trade_streak >= 7
            5,
            3,
            false,
            0,
            0,
            0,
        );

        assert!(GamificationEngine::has_badge(&env, &user, &AchievementBadge::FirstTrade));
        assert!(GamificationEngine::has_badge(&env, &user, &AchievementBadge::TenTrades));
        assert!(GamificationEngine::has_badge(&env, &user, &AchievementBadge::HundredTrades));
        assert!(GamificationEngine::has_badge(
            &env,
            &user,
            &AchievementBadge::ProfitableTrader
        ));
        assert!(GamificationEngine::has_badge(
            &env,
            &user,
            &AchievementBadge::ConsistentPerformer
        ));
        assert!(GamificationEngine::has_badge(
            &env,
            &user,
            &AchievementBadge::MasterTrader
        ));
    }

    // ════════════════════════════════════════════════════════════════════════
    // Edge Cases and Error Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_leaderboard_empty_page() {
        let (env, _) = setup();

        let page =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 0, 10);
        assert_eq!(page.len(), 0);
    }

    #[test]
    fn test_multiple_leaderboard_metrics_independent() {
        let (env, user) = setup();
        let other = Address::generate(&env);

        GamificationEngine::update_leaderboard_entry(
            &env,
            &user,
            &LeaderboardMetric::ByScore,
            100,
        );
        GamificationEngine::update_leaderboard_entry(
            &env,
            &other,
            &LeaderboardMetric::ByTradeCount,
            200,
        );

        let score_lb =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 0, 10);
        let trade_lb =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByTradeCount, 0, 10);

        assert_eq!(score_lb.len(), 1);
        assert_eq!(trade_lb.len(), 1);
        assert_eq!(score_lb.get(0).unwrap().user, user);
        assert_eq!(trade_lb.get(0).unwrap().user, other);
    }

    #[test]
    fn test_challenge_objective_types() {
        let (env, user) = setup();

        // Test with different objective types
        let objectives = vec![
            ChallengeObjective::CompleteTrades(5),
            ChallengeObjective::TradeVolume(1000),
            ChallengeObjective::WinStreak(3),
            ChallengeObjective::Diversify(4),
            ChallengeObjective::LiquidityProvision(500),
        ];

        for objective in objectives {
            let challenge_id = GamificationEngine::create_challenge(
                &env,
                symbol_short!("chlg"),
                symbol_short!("desc"),
                objective,
                86400,
                5_000,
                100,
                ProgressionTier::Bronze,
            )
            .unwrap();

            GamificationEngine::join_challenge(&env, &user, challenge_id).unwrap();
        }
    }

    #[test]
    fn test_challenge_progress_overflow_protection() {
        let (env, user) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(5),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        GamificationEngine::join_challenge(&env, &user, challenge_id).unwrap();

        // Add massive progress
        GamificationEngine::update_challenge_progress(&env, &user, challenge_id, 1_000_000)
            .unwrap();

        let progress = GamificationEngine::get_challenge_progress(&env, &user, challenge_id)
            .unwrap();
        assert!(progress.objective_met);
    }

    #[test]
    fn test_user_profile_independent_of_badges() {
        let (env, user) = setup();

        // Profile should exist independently of badges
        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert_eq!(profile.tier, ProgressionTier::Bronze);

        // Award badges should update profile score
        GamificationEngine::award_badge(&env, &user, AchievementBadge::FirstTrade);
        GamificationEngine::award_badge(&env, &user, AchievementBadge::TenTrades);

        let profile = GamificationEngine::get_or_create_profile(&env, &user);
        assert!(profile.score > 0);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Multi-User Simulation Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_multi_user_leaderboard_competition() {
        let (env, _) = setup();

        let users: Vec<Address> = (0..5)
            .map(|_| Address::generate(&env))
            .collect();

        // Give each user different scores
        for (i, user) in users.iter().enumerate() {
            GamificationEngine::add_score(&env, user, ((i + 1) * 100) as i128, symbol_short!("test"));
        }

        let leaderboard =
            GamificationEngine::get_leaderboard(&env, &LeaderboardMetric::ByScore, 0, 10);

        assert_eq!(leaderboard.len(), 5);
        // Verify descending order
        for i in 1..leaderboard.len() {
            assert!(leaderboard.get(i - 1).unwrap().score >= leaderboard.get(i).unwrap().score);
        }
    }

    #[test]
    fn test_multi_user_challenge_participation() {
        let (env, _) = setup();

        let challenge_id = GamificationEngine::create_challenge(
            &env,
            symbol_short!("chlg1"),
            symbol_short!("desc1"),
            ChallengeObjective::CompleteTrades(3),
            86400,
            10_000,
            100,
            ProgressionTier::Bronze,
        )
        .unwrap();

        // Multiple users join and progress
        let users: Vec<Address> = (0..3)
            .map(|_| Address::generate(&env))
            .collect();

        for user in &users {
            GamificationEngine::join_challenge(&env, user, challenge_id).unwrap();
        }

        // User 0 completes
        GamificationEngine::update_challenge_progress(&env, &users.get(0).unwrap(), challenge_id, 3)
            .unwrap();

        // User 1 partially progresses
        GamificationEngine::update_challenge_progress(&env, &users.get(1).unwrap(), challenge_id, 2)
            .unwrap();

        let progress0 =
            GamificationEngine::get_challenge_progress(&env, &users.get(0).unwrap(), challenge_id)
                .unwrap();
        let progress1 =
            GamificationEngine::get_challenge_progress(&env, &users.get(1).unwrap(), challenge_id)
                .unwrap();

        assert!(progress0.objective_met);
        assert!(!progress1.objective_met);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Scoring Calculation Property Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_scoring_never_negative() {
        // All scoring functions should return non-negative values
        let score = GamificationEngine::calculate_trading_score(0, 0, -1000, -5000);
        assert!(score >= 0);
    }

    #[test]
    fn test_scoring_monotonic_with_trades() {
        // More trades should equal or greater score
        let score1 = GamificationEngine::calculate_trading_score(1, 0, 0, 0);
        let score10 = GamificationEngine::calculate_trading_score(10, 0, 0, 0);
        let score100 = GamificationEngine::calculate_trading_score(100, 0, 0, 0);

        assert!(score10 >= score1);
        assert!(score100 >= score10);
    }

    #[test]
    fn test_composite_score_bounds() {
        // Composite score with max inputs should be reasonable
        let score = GamificationEngine::calculate_composite_score(
            1000, 1000, 1000, 1000, 1000, 1000,
        );
        assert!(score > 0);
        assert!(score <= 6000); // Max possible
    }
}
