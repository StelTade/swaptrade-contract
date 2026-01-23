use soroban_sdk::{contracttype, Address, Env, String, symbol_short};

// --- Data Structures ---

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Badge {
    FirstTrade,
}

#[contracttype]
pub enum DataKey {
    UserBadge(Address, Badge),
}

// --- Implementation ---

/// Award the "First Trade" badge to a user upon their first trade.
pub fn award_first_trade(env: &Env, user: Address) {
    // Define the unique key for this user's specific badge
    let key = DataKey::UserBadge(user.clone(), Badge::FirstTrade);

    // 1. Prevent duplicate awards: Check if the badge already exists
    if env.storage().persistent().has(&key) {
        // Badge already exists, we exit early to avoid overwriting or redundant logic
        return;
    }

    // 2. Store awarded badge in contract state
    // We use persistent storage so the achievement is permanent
    env.storage().persistent().set(&key, &true);

    // 3. Emit an event (Best practice for gamification)
    // This allows the frontend to trigger a "Congratulations!" popup
    env.events().publish(
        (symbol_short!("reward"), user),
        symbol_short!("1st_trade")
    );
}

/// Helper function to verify if a user has a specific badge
pub fn has_badge(env: &Env, user: Address, badge: Badge) -> bool {
    let key = DataKey::UserBadge(user, badge);
    env.storage().persistent().has(&key)
}