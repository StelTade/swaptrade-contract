use soroban_sdk::{contracttype, Address, Env, Map, symbol_short};
use crate::tiers::UserTier;

/// Rate limit configuration per tier
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimitConfig {
    /// Maximum swaps per hour
    pub swaps_per_hour: u32,
    /// Maximum LP operations per day
    pub lp_ops_per_day: u32,
}

impl RateLimitConfig {
    pub fn for_tier(tier: &UserTier) -> Self {
        match tier {
            UserTier::Novice => RateLimitConfig {
                swaps_per_hour: 5,
                lp_ops_per_day: 10,
            },
            UserTier::Trader => RateLimitConfig {
                swaps_per_hour: 20,
                lp_ops_per_day: 30,
            },
            UserTier::Expert => RateLimitConfig {
                swaps_per_hour: 100,
                lp_ops_per_day: u32::MAX,
            },
            UserTier::Whale => RateLimitConfig {
                swaps_per_hour: u32::MAX,
                lp_ops_per_day: u32::MAX,
            },
        }
    }
}

/// Rate limit status response
#[contracttype]
#[derive(Clone, Debug)]
pub struct RateLimitStatus {
    /// Current operations used in time window
    pub used: u32,
    /// Limit for this time window
    pub limit: u32,
    /// Milliseconds until limit resets
    pub cooldown_ms: u64,
}

/// Time window info
#[contracttype]
#[derive(Clone, Debug)]
pub struct TimeWindow {
    /// Timestamp of window start (Unix seconds)
    pub window_start: u64,
    /// Window duration in seconds
    pub window_duration: u64,
}

impl TimeWindow {
    /// Create hourly window (3600 seconds)
    pub fn hourly(current_timestamp: u64) -> Self {
        let window_duration = 3600u64;
        let window_start = (current_timestamp / window_duration) * window_duration;
        TimeWindow {
            window_start,
            window_duration,
        }
    }

    /// Create daily window (86400 seconds)
    pub fn daily(current_timestamp: u64) -> Self {
        let window_duration = 86400u64;
        let window_start = (current_timestamp / window_duration) * window_duration;
        TimeWindow {
            window_start,
            window_duration,
        }
    }

    /// Get milliseconds until next window
    pub fn cooldown_ms(&self, current_timestamp: u64) -> u64 {
        let next_window = self.window_start + self.window_duration;
        if current_timestamp >= next_window {
            0
        } else {
            (next_window - current_timestamp) * 1000
        }
    }
}

/// Rate limiter for swap and LP operations
pub struct RateLimiter;

impl RateLimiter {
    /// Check and record a swap operation for the user
    /// Returns Ok(()) if operation is allowed, Err with cooldown if rate limited
    pub fn check_swap_limit(
        env: &Env,
        user: &Address,
        tier: &UserTier,
    ) -> Result<(), RateLimitStatus> {
        let config = RateLimitConfig::for_tier(tier);
        
        // Unlimited for Whale tier with max u32 limit
        if config.swaps_per_hour == u32::MAX {
            return Ok(());
        }

        let timestamp = env.ledger().timestamp();
        let window = TimeWindow::hourly(timestamp);
        let count_key = (user.clone(), symbol_short!("swap"), window.window_start);

        // Get current count
        let current_count: u32 = env
            .storage()
            .persistent()
            .get(&count_key)
            .unwrap_or(0);

        if current_count >= config.swaps_per_hour {
            return Err(RateLimitStatus {
                used: current_count,
                limit: config.swaps_per_hour,
                cooldown_ms: window.cooldown_ms(timestamp),
            });
        }

        Ok(())
    }

    /// Record a swap operation in storage
    pub fn record_swap(env: &Env, user: &Address, timestamp: u64) {
        let window = TimeWindow::hourly(timestamp);
        let count_key = (user.clone(), symbol_short!("swap"), window.window_start);

        let current_count: u32 = env
            .storage()
            .persistent()
            .get(&count_key)
            .unwrap_or(0);

        env.storage()
            .persistent()
            .set(&count_key, &(current_count + 1));
    }

    /// Check and record an LP operation for the user
    pub fn check_lp_limit(
        env: &Env,
        user: &Address,
        tier: &UserTier,
    ) -> Result<(), RateLimitStatus> {
        let config = RateLimitConfig::for_tier(tier);
        
        // Unlimited for Expert+ tiers with max u32 limit
        if config.lp_ops_per_day == u32::MAX {
            return Ok(());
        }

        let timestamp = env.ledger().timestamp();
        let window = TimeWindow::daily(timestamp);
        let count_key = (user.clone(), symbol_short!("lp_op"), window.window_start);

        let current_count: u32 = env
            .storage()
            .persistent()
            .get(&count_key)
            .unwrap_or(0);

        if current_count >= config.lp_ops_per_day {
            return Err(RateLimitStatus {
                used: current_count,
                limit: config.lp_ops_per_day,
                cooldown_ms: window.cooldown_ms(timestamp),
            });
        }

        Ok(())
    }

    /// Record an LP operation in storage
    pub fn record_lp_op(env: &Env, user: &Address, timestamp: u64) {
        let window = TimeWindow::daily(timestamp);
        let count_key = (user.clone(), symbol_short!("lp_op"), window.window_start);

        let current_count: u32 = env
            .storage()
            .persistent()
            .get(&count_key)
            .unwrap_or(0);

        env.storage()
            .persistent()
            .set(&count_key, &(current_count + 1));
    }

    /// Get rate limit status for swaps
    pub fn get_swap_status(
        env: &Env,
        user: &Address,
        tier: &UserTier,
    ) -> RateLimitStatus {
        let config = RateLimitConfig::for_tier(tier);
        let timestamp = env.ledger().timestamp();
        let window = TimeWindow::hourly(timestamp);
        let count_key = (user.clone(), symbol_short!("swap"), window.window_start);

        let used: u32 = env
            .storage()
            .persistent()
            .get(&count_key)
            .unwrap_or(0);

        RateLimitStatus {
            used,
            limit: config.swaps_per_hour,
            cooldown_ms: window.cooldown_ms(timestamp),
        }
    }

    /// Get rate limit status for LP operations
    pub fn get_lp_status(
        env: &Env,
        user: &Address,
        tier: &UserTier,
    ) -> RateLimitStatus {
        let config = RateLimitConfig::for_tier(tier);
        let timestamp = env.ledger().timestamp();
        let window = TimeWindow::daily(timestamp);
        let count_key = (user.clone(), symbol_short!("lp_op"), window.window_start);

        let used: u32 = env
            .storage()
            .persistent()
            .get(&count_key)
            .unwrap_or(0);

        RateLimitStatus {
            used,
            limit: config.lp_ops_per_day,
            cooldown_ms: window.cooldown_ms(timestamp),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config_tiers() {
        let novice = RateLimitConfig::for_tier(&UserTier::Novice);
        assert_eq!(novice.swaps_per_hour, 5);
        assert_eq!(novice.lp_ops_per_day, 10);

        let trader = RateLimitConfig::for_tier(&UserTier::Trader);
        assert_eq!(trader.swaps_per_hour, 20);
        assert_eq!(trader.lp_ops_per_day, 30);

        let expert = RateLimitConfig::for_tier(&UserTier::Expert);
        assert_eq!(expert.swaps_per_hour, 100);
        assert_eq!(expert.lp_ops_per_day, u32::MAX);

        let whale = RateLimitConfig::for_tier(&UserTier::Whale);
        assert_eq!(whale.swaps_per_hour, u32::MAX);
        assert_eq!(whale.lp_ops_per_day, u32::MAX);
    }

    #[test]
    fn test_hourly_window() {
        let ts = 7200u64; // 2 hours
        let window = TimeWindow::hourly(ts);
        assert_eq!(window.window_start, 3600u64); // Start of hour 2
        assert_eq!(window.window_duration, 3600u64);
    }

    #[test]
    fn test_daily_window() {
        let ts = 172800u64; // 2 days
        let window = TimeWindow::daily(ts);
        assert_eq!(window.window_start, 86400u64); // Start of day 2
        assert_eq!(window.window_duration, 86400u64);
    }

    #[test]
    fn test_cooldown_calculation() {
        let window = TimeWindow::hourly(3600u64);
        let cooldown = window.cooldown_ms(3600u64);
        assert_eq!(cooldown, 3600000u64); // 1 hour in ms

        let cooldown_half = window.cooldown_ms(5400u64);
        assert_eq!(cooldown_half, 1800000u64); // 30 min in ms

        let cooldown_expired = window.cooldown_ms(7200u64);
        assert_eq!(cooldown_expired, 0u64);
    }
}
