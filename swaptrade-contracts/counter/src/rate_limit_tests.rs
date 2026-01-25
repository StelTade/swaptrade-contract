#[cfg(test)]
mod rate_limit_tests {
    use crate::{CounterContract, RateLimiter, UserTier};
    use soroban_sdk::{testutils::{self, Address as _, Ledger}, Address, Env, Symbol, symbol_short};

    fn create_test_env() -> (Env, Address) {
        let env = Env::default();
        let user = Address::generate(&env);
        (env, user)
    }

    #[test]
    fn test_novice_swap_limit() {
        let (env, user) = create_test_env();
        let novice = UserTier::Novice;

        // First 5 swaps should succeed
        for i in 0..5 {
            env.ledger().set_timestamp(3600 * i as u64 + 1);
            let result = RateLimiter::check_swap_limit(&env, &user, &novice);
            assert!(result.is_ok(), "Swap {} should be allowed", i + 1);
            RateLimiter::record_swap(&env, &user, env.ledger().timestamp());
        }

        // 6th swap should fail
        env.ledger().set_timestamp(3600 * 5 + 1);
        let result = RateLimiter::check_swap_limit(&env, &user, &novice);
        assert!(result.is_err(), "6th swap should be rate limited");

        let status = result.unwrap_err();
        assert_eq!(status.used, 5);
        assert_eq!(status.limit, 5);
        assert!(status.cooldown_ms > 0, "Should have cooldown");
    }

    #[test]
    fn test_trader_swap_limit() {
        let (env, user) = create_test_env();
        let trader = UserTier::Trader;

        // Trader should allow 20 swaps per hour
        for i in 0..20 {
            env.ledger().set_timestamp(3600 + i);
            let result = RateLimiter::check_swap_limit(&env, &user, &trader);
            assert!(result.is_ok(), "Swap {} should be allowed for Trader", i + 1);
            RateLimiter::record_swap(&env, &user, env.ledger().timestamp());
        }

        // 21st should fail
        env.ledger().set_timestamp(3600 + 20);
        let result = RateLimiter::check_swap_limit(&env, &user, &trader);
        assert!(result.is_err(), "21st swap should be rate limited");
    }

    #[test]
    fn test_expert_swap_limit() {
        let (env, user) = create_test_env();
        let expert = UserTier::Expert;

        // Expert should allow 100 swaps per hour
        for i in 0..100 {
            env.ledger().set_timestamp(3600 + i);
            let result = RateLimiter::check_swap_limit(&env, &user, &expert);
            assert!(result.is_ok(), "Swap {} should be allowed for Expert", i + 1);
            RateLimiter::record_swap(&env, &user, env.ledger().timestamp());
        }

        // 101st should fail
        env.ledger().set_timestamp(3600 + 100);
        let result = RateLimiter::check_swap_limit(&env, &user, &expert);
        assert!(result.is_err(), "101st swap should be rate limited");
    }

    #[test]
    fn test_whale_unlimited_swaps() {
        let (env, user) = create_test_env();
        let whale = UserTier::Whale;

        // Whale tier should have unlimited swaps (u32::MAX)
        for i in 0..200 {
            env.ledger().set_timestamp(3600 + i);
            let result = RateLimiter::check_swap_limit(&env, &user, &whale);
            assert!(result.is_ok(), "Whale should always be allowed, swap {}", i + 1);
        }
    }

    #[test]
    fn test_hourly_window_boundary() {
        let (env, user) = create_test_env();
        let novice = UserTier::Novice;

        // Consume 5 swaps in hour 0
        for i in 0..5 {
            env.ledger().set_timestamp(100 + i);
            let result = RateLimiter::check_swap_limit(&env, &user, &novice);
            assert!(result.is_ok());
            RateLimiter::record_swap(&env, &user, env.ledger().timestamp());
        }

        // Should be rate limited at end of hour 0
        env.ledger().set_timestamp(3500);
        let result = RateLimiter::check_swap_limit(&env, &user, &novice);
        assert!(result.is_err(), "Should be rate limited in same hour");

        // Move to next hour - should reset
        env.ledger().set_timestamp(3600);
        let result = RateLimiter::check_swap_limit(&env, &user, &novice);
        assert!(result.is_ok(), "Should allow swap in new hour");
    }

    #[test]
    fn test_novice_lp_limit() {
        let (env, user) = create_test_env();
        let novice = UserTier::Novice;

        // First 10 LP ops should succeed (daily limit)
        for i in 0..10 {
            env.ledger().set_timestamp(86400 + i as u64);
            let result = RateLimiter::check_lp_limit(&env, &user, &novice);
            assert!(result.is_ok(), "LP op {} should be allowed", i + 1);
            RateLimiter::record_lp_op(&env, &user, env.ledger().timestamp());
        }

        // 11th should fail
        env.ledger().set_timestamp(86400 + 10);
        let result = RateLimiter::check_lp_limit(&env, &user, &novice);
        assert!(result.is_err(), "11th LP op should be rate limited");
    }

    #[test]
    fn test_trader_lp_limit() {
        let (env, user) = create_test_env();
        let trader = UserTier::Trader;

        // Trader allows 30 LP ops per day
        for i in 0..30 {
            env.ledger().set_timestamp(86400 + i as u64);
            let result = RateLimiter::check_lp_limit(&env, &user, &trader);
            assert!(result.is_ok(), "LP op {} should be allowed for Trader", i + 1);
            RateLimiter::record_lp_op(&env, &user, env.ledger().timestamp());
        }

        // 31st should fail
        env.ledger().set_timestamp(86400 + 30);
        let result = RateLimiter::check_lp_limit(&env, &user, &trader);
        assert!(result.is_err(), "31st LP op should be rate limited");
    }

    #[test]
    fn test_expert_unlimited_lp() {
        let (env, user) = create_test_env();
        let expert = UserTier::Expert;

        // Expert tier should have unlimited LP ops
        for i in 0..100 {
            env.ledger().set_timestamp(86400 + i as u64);
            let result = RateLimiter::check_lp_limit(&env, &user, &expert);
            assert!(result.is_ok(), "Expert should always be allowed, LP op {}", i + 1);
        }
    }

    #[test]
    fn test_daily_window_boundary() {
        let (env, user) = create_test_env();
        let novice = UserTier::Novice;

        // Consume 10 LP ops in day 0
        for i in 0..10 {
            env.ledger().set_timestamp(100 + i as u64);
            let result = RateLimiter::check_lp_limit(&env, &user, &novice);
            assert!(result.is_ok());
            RateLimiter::record_lp_op(&env, &user, env.ledger().timestamp());
        }

        // Should be rate limited at end of day 0
        env.ledger().set_timestamp(85000);
        let result = RateLimiter::check_lp_limit(&env, &user, &novice);
        assert!(result.is_err(), "Should be rate limited in same day");

        // Move to next day - should reset
        env.ledger().set_timestamp(86400);
        let result = RateLimiter::check_lp_limit(&env, &user, &novice);
        assert!(result.is_ok(), "Should allow LP op in new day");
    }

    #[test]
    fn test_cooldown_calculation() {
        let (env, user) = create_test_env();
        let novice = UserTier::Novice;

        // Fill up swap limit
        for i in 0..5 {
            env.ledger().set_timestamp(100 + i as u64);
            RateLimiter::record_swap(&env, &user, env.ledger().timestamp());
        }

        // Check cooldown at various times
        env.ledger().set_timestamp(1000);
        let result = RateLimiter::check_swap_limit(&env, &user, &novice);
        assert!(result.is_err());
        let status = result.unwrap_err();
        let cooldown_at_1000 = status.cooldown_ms;

        // Cooldown should decrease as time moves forward
        env.ledger().set_timestamp(2000);
        let result = RateLimiter::check_swap_limit(&env, &user, &novice);
        let status = result.unwrap_err();
        let cooldown_at_2000 = status.cooldown_ms;

        assert!(cooldown_at_2000 < cooldown_at_1000, "Cooldown should decrease over time");
    }

    #[test]
    fn test_rate_limit_status_queries() {
        let (env, user) = create_test_env();
        let novice = UserTier::Novice;

        // Record 3 swaps
        for i in 0..3 {
            env.ledger().set_timestamp(100 + i as u64);
            RateLimiter::record_swap(&env, &user, env.ledger().timestamp());
        }

        env.ledger().set_timestamp(500);
        let status = RateLimiter::get_swap_status(&env, &user, &novice);

        assert_eq!(status.used, 3);
        assert_eq!(status.limit, 5);
        assert!(status.cooldown_ms > 0);
    }

    #[test]
    fn test_different_users_independent_limits() {
        let env = Env::default();
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let novice = UserTier::Novice;

        // User1 consumes 5 swaps
        for i in 0..5 {
            env.ledger().set_timestamp(100 + i as u64);
            RateLimiter::record_swap(&env, &user1, env.ledger().timestamp());
        }

        // User1 should be limited
        env.ledger().set_timestamp(200);
        assert!(RateLimiter::check_swap_limit(&env, &user1, &novice).is_err());

        // User2 should still be able to swap (independent counter)
        env.ledger().set_timestamp(200);
        assert!(RateLimiter::check_swap_limit(&env, &user2, &novice).is_ok());
    }

    #[test]
    fn test_swap_and_lp_ops_independent() {
        let (env, user) = create_test_env();
        let novice = UserTier::Novice;

        // Consume swap limit
        for i in 0..5 {
            env.ledger().set_timestamp(3600 + i as u64);
            RateLimiter::record_swap(&env, &user, env.ledger().timestamp());
        }

        // LP ops should still be allowed (different time window)
        env.ledger().set_timestamp(86400);
        assert!(RateLimiter::check_lp_limit(&env, &user, &novice).is_ok(), "LP ops should be independent");

        // Consume LP limit
        for i in 0..10 {
            env.ledger().set_timestamp(86400 + i as u64);
            RateLimiter::record_lp_op(&env, &user, env.ledger().timestamp());
        }

        // Swaps in hour 1 should still be limited
        env.ledger().set_timestamp(3600 + 100);
        assert!(
            RateLimiter::check_swap_limit(&env, &user, &novice).is_err(),
            "Swap limit from hour 0 should still apply"
        );

        // But swaps in hour 2 should work (new window)
        env.ledger().set_timestamp(7200);
        assert!(
            RateLimiter::check_swap_limit(&env, &user, &novice).is_ok(),
            "Swaps in new hour should be allowed"
        );
    }

    #[test]
    fn test_status_at_limit_boundary() {
        let (env, user) = create_test_env();
        let trader = UserTier::Trader;

        // Record exactly 20 swaps (at limit)
        for i in 0..20 {
            env.ledger().set_timestamp(3600 + i as u64);
            RateLimiter::record_swap(&env, &user, env.ledger().timestamp());
        }

        let status = RateLimiter::get_swap_status(&env, &user, &trader);
        assert_eq!(status.used, 20);
        assert_eq!(status.limit, 20);

        // Next swap should fail
        let check = RateLimiter::check_swap_limit(&env, &user, &trader);
        assert!(check.is_err());
    }
}
