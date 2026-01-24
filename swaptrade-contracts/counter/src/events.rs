use soroban_sdk::{Address, Env, Symbol};

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
        env.events().publish(
            (Symbol::new(env, "SwapExecuted"), user, from_token, to_token),
            (from_amount, to_amount, timestamp),
        );
    }

    pub fn liquidity_added(
        env: &Env,
        xlm_amount: i128,
        usdc_amount: i128,
        lp_tokens_minted: i128,
        user: Address,
        timestamp: i64,
    ) {
        env.events().publish(
            (Symbol::new(env, "LiquidityAdded"), user),
            (xlm_amount, usdc_amount, lp_tokens_minted, timestamp),
        );
    }

    pub fn liquidity_removed(
        env: &Env,
        xlm_amount: i128,
        usdc_amount: i128,
        lp_tokens_burned: i128,
        user: Address,
        timestamp: i64,
    ) {
        env.events().publish(
            (Symbol::new(env, "LiquidityRemoved"), user),
            (xlm_amount, usdc_amount, lp_tokens_burned, timestamp),
        );
    }

    pub fn badge_awarded(
        env: &Env,
        user: Address,
        badge: crate::portfolio::Badge,
        timestamp: i64,
    ) {
        env.events().publish(
            (Symbol::new(env, "BadgeAwarded"), user),
            (badge, timestamp),
        );
    }

    pub fn user_tier_changed(
        env: &Env,
        user: Address,
        old_tier: crate::tiers::UserTier,
        new_tier: crate::tiers::UserTier,
        timestamp: i64,
    ) {
        env.events().publish(
            (Symbol::new(env, "UserTierChanged"), user),
            (old_tier, new_tier, timestamp),
        );
    }

    pub fn admin_paused(env: &Env, admin: Address, timestamp: i64) {
        env.events().publish(
            (Symbol::new(env, "AdminPaused"), admin),
            (timestamp,),
        );
    }

    pub fn admin_resumed(env: &Env, admin: Address, timestamp: i64) {
        env.events().publish(
            (Symbol::new(env, "AdminResumed"), admin),
            (timestamp,),
        );
    }
}
