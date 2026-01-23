use soroban_sdk::{Address, Env, Event, Symbol};

pub struct Events;

impl Events {
    pub fn swap_executed(
        env: &Env,
        from_token: Symbol,
        to_token: Symbol,
        from_amount: i128,
        to_amount: i128,
        user: Address,
        timestamp: i64,
    ) {
        Event::new(env)
            .topic("SwapExecuted")
            .topic(user.clone())
            .topic(from_token.clone())
            .topic(to_token.clone())
            .data((from_amount, to_amount, timestamp))
            .publish();
    }

    pub fn liquidity_added(
        env: &Env,
        xlm_amount: i128,
        usdc_amount: i128,
        lp_tokens_minted: i128,
        user: Address,
        timestamp: i64,
    ) {
        Event::new(env)
            .topic("LiquidityAdded")
            .topic(user.clone())
            .data((xlm_amount, usdc_amount, lp_tokens_minted, timestamp))
            .publish();
    }

    pub fn liquidity_removed(
        env: &Env,
        xlm_amount: i128,
        usdc_amount: i128,
        lp_tokens_burned: i128,
        user: Address,
        timestamp: i64,
    ) {
        Event::new(env)
            .topic("LiquidityRemoved")
            .topic(user.clone())
            .data((xlm_amount, usdc_amount, lp_tokens_burned, timestamp))
            .publish();
    }

    pub fn badge_awarded(
        env: &Env,
        user: Address,
        badge: crate::portfolio::Badge,
        timestamp: i64,
    ) {
        Event::new(env)
            .topic("BadgeAwarded")
            .topic(user.clone())
            .data((badge, timestamp))
            .publish();
    }

    pub fn user_tier_changed(
        env: &Env,
        user: Address,
        old_tier: crate::tiers::UserTier,
        new_tier: crate::tiers::UserTier,
        timestamp: i64,
    ) {
        Event::new(env)
            .topic("UserTierChanged")
            .topic(user.clone())
            .data((old_tier, new_tier, timestamp))
            .publish();
    }

    pub fn admin_paused(env: &Env, admin: Address, timestamp: i64) {
        Event::new(env)
            .topic("AdminPaused")
            .topic(admin.clone())
            .data((timestamp,))
            .publish();
    }

    pub fn admin_resumed(env: &Env, admin: Address, timestamp: i64) {
        Event::new(env)
            .topic("AdminResumed")
            .topic(admin.clone())
            .data((timestamp,))
            .publish();
    }
}
