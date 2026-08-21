// Dynamic Gamification & Rewards System
//
// Provides achievement badges, leaderboards, streak tracking, dynamic scoring,
// challenge campaigns, reward distribution, and tier-based progression for traders.

use soroban_sdk::{
    contracttype, symbol_short, Address, Env, Symbol, Vec,
};

// ────────────────────────────────────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────────────────────────────────────

/// Maximum leaderboard size per metric
const MAX_LEADERBOARD_SIZE: u32 = 100;

/// Maximum concurrent active challenges
const MAX_ACTIVE_CHALLENGES: u32 = 50;

/// Base reward amount for first-tier achievements
const BASE_REWARD_AMOUNT: i128 = 100;

/// Maximum streak bonus multiplier (10x)
const MAX_STREAK_MULTIPLIER: i128 = 10;

/// Challenge minimum duration (1 hour in seconds)
const MIN_CHALLENGE_DURATION: u64 = 3600;

/// Challenge maximum duration (30 days in seconds)
const MAX_CHALLENGE_DURATION: u64 = 2_592_000;

// ────────────────────────────────────────────────────────────────────────────
// Data Structures
// ────────────────────────────────────────────────────────────────────────────

/// Extended achievement badge types for the gamification system
#[derive(Clone, PartialEq, Eq, Debug)]
#[contracttype]
pub enum AchievementBadge {
    // Trading Milestones
    FirstTrade,
    TenTrades,
    FiftyTrades,
    HundredTrades,
    ThousandTrades,

    // Performance Milestones
    ProfitableTrader,       // 10+ winning trades
    HighRoller,             // Single trade > 10,000 XLM
    ConsistentPerformer,    // 7+ consecutive profitable days
    PerfectWeek,            // 7+ consecutive winning trades

    // Portfolio Milestones
    WealthBuilder,          // Portfolio 10x starting value
    DiversifiedPortfolio,   // Hold 5+ different assets
    LPProvider,             // Provide liquidity at least once
    DeepLiquidity,          // Provide > 1,000 XLM liquidity

    // Social & Learning
    CommunityContributor,   // Refer 3+ users
    ChallengeChampion,      // Win 3+ challenge campaigns
    StreakMaster,           // Maintain 30+ day trading streak

    // Special / Time-limited
    EarlyAdopter,           // First 100 users
    SeasonChampion,         // Top 3 in any season leaderboard
    MasterTrader,           // Achieve all trading badges
}

/// Progression tier based on cumulative score
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[contracttype]
pub enum ProgressionTier {
    Bronze,
    Silver,
    Gold,
    Platinum,
    Diamond,
}

impl ProgressionTier {
    /// Score threshold to reach this tier
    pub fn score_threshold(&self) -> i128 {
        match self {
            ProgressionTier::Bronze => 0,
            ProgressionTier::Silver => 1_000,
            ProgressionTier::Gold => 5_000,
            ProgressionTier::Platinum => 20_000,
            ProgressionTier::Diamond => 100_000,
        }
    }

    /// Reward multiplier for this tier (basis points, 1.0x = 10000)
    pub fn reward_multiplier_bps(&self) -> i128 {
        match self {
            ProgressionTier::Bronze => 10_000,    // 1.0x
            ProgressionTier::Silver => 12_000,    // 1.2x
            ProgressionTier::Gold => 15_000,      // 1.5x
            ProgressionTier::Platinum => 20_000,  // 2.0x
            ProgressionTier::Diamond => 30_000,   // 3.0x
        }
    }

    /// Get the next tier, if any
    pub fn next_tier(&self) -> Option<ProgressionTier> {
        match self {
            ProgressionTier::Bronze => Some(ProgressionTier::Silver),
            ProgressionTier::Silver => Some(ProgressionTier::Gold),
            ProgressionTier::Gold => Some(ProgressionTier::Platinum),
            ProgressionTier::Platinum => Some(ProgressionTier::Diamond),
            ProgressionTier::Diamond => None,
        }
    }
}

/// User gamification profile
#[derive(Clone, Debug)]
#[contracttype]
pub struct GamificationProfile {
    /// User's current progression tier
    pub tier: ProgressionTier,
    /// Cumulative gamification score
    pub score: i128,
    /// Total points earned (never decreases)
    pub lifetime_score: i128,
    /// Current trading streak (consecutive profitable trades)
    pub current_trade_streak: u32,
    /// Best trading streak ever
    pub best_trade_streak: u32,
    /// Current daily activity streak (consecutive days with activity)
    pub current_daily_streak: u32,
    /// Best daily streak ever
    pub best_daily_streak: u32,
    /// Number of challenges won
    pub challenges_won: u32,
    /// Number of challenges participated in
    pub challenges_participated: u32,
    /// Timestamp of last activity
    pub last_activity_timestamp: u64,
    /// Total rewards earned (in base units)
    pub total_rewards_earned: i128,
    /// Rewards pending claim
    pub pending_rewards: i128,
}

impl GamificationProfile {
    pub fn new(env: &Env) -> Self {
        Self {
            tier: ProgressionTier::Bronze,
            score: 0,
            lifetime_score: 0,
            current_trade_streak: 0,
            best_trade_streak: 0,
            current_daily_streak: 0,
            best_daily_streak: 0,
            challenges_won: 0,
            challenges_participated: 0,
            last_activity_timestamp: 0,
            total_rewards_earned: 0,
            pending_rewards: 0,
        }
    }
}

/// Leaderboard entry
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct LeaderboardEntry {
    pub user: Address,
    pub score: i128,
    pub rank: u32,
}

/// Leaderboard metric type
#[derive(Clone, PartialEq, Eq, Debug)]
#[contracttype]
pub enum LeaderboardMetric {
    ByScore,            // Total gamification score
    ByROI,              // Return on investment
    ByTradeCount,       // Total number of trades
    ByTradeStreak,      // Current trading streak
    ByVolume,           // Total trading volume
    ByWinRate,          // Win rate percentage
}

/// Challenge campaign status
#[derive(Clone, PartialEq, Eq, Debug)]
#[contracttype]
pub enum ChallengeStatus {
    Active,
    Completed,
    Cancelled,
}

/// Challenge objective type
#[derive(Clone, PartialEq, Eq, Debug)]
#[contracttype]
pub enum ChallengeObjective {
    CompleteTrades(u32),         // Complete N trades
    AchieveROI(i128),           // Achieve target ROI (bps)
    TradeVolume(i128),          // Trade at least N amount
    WinStreak(u32),             // Achieve N consecutive wins
    Diversify(u32),             // Trade N different pairs
    LiquidityProvision(i128),   // Provide at least N liquidity
}

/// Challenge campaign definition
#[derive(Clone, Debug)]
#[contracttype]
pub struct ChallengeCampaign {
    /// Unique challenge ID
    pub challenge_id: u64,
    /// Challenge name
    pub name: Symbol,
    /// Description
    pub description: Symbol,
    /// Objective that must be met
    pub objective: ChallengeObjective,
    /// Start timestamp
    pub start_time: u64,
    /// End timestamp
    pub end_time: u64,
    /// Reward pool for this challenge
    pub reward_pool: i128,
    /// Status
    pub status: ChallengeStatus,
    /// Maximum participants (0 = unlimited)
    pub max_participants: u32,
    /// Current participant count
    pub participant_count: u32,
    /// Tier requirement to participate
    pub min_tier: ProgressionTier,
}

/// User's progress in a challenge
#[derive(Clone, Debug)]
#[contracttype]
pub struct ChallengeProgress {
    /// Current progress value toward objective
    pub current_progress: i128,
    /// Whether the objective has been met
    pub objective_met: bool,
    /// Whether reward has been claimed
    pub reward_claimed: bool,
    /// Timestamp when objective was first met
    pub completed_at: Option<u64>,
}

/// Reward distribution record for audit trail
#[derive(Clone, Debug)]
#[contracttype]
pub struct RewardDistribution {
    /// Recipient
    pub user: Address,
    /// Amount distributed
    pub amount: i128,
    /// Reason for distribution
    pub reason: Symbol,
    /// Timestamp
    pub timestamp: u64,
    /// Associated challenge ID (if applicable)
    pub challenge_id: Option<u64>,
}

/// Score breakdown for transparency
#[derive(Clone, Debug)]
#[contracttype]
pub struct ScoreBreakdown {
    pub trading_score: i128,
    pub performance_score: i128,
    pub learning_score: i128,
    pub community_score: i128,
    pub streak_bonus: i128,
    pub challenge_bonus: i128,
}

// ────────────────────────────────────────────────────────────────────────────
// Storage Keys
// ────────────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Eq)]
#[contracttype]
pub enum GamificationStorageKey {
    /// User's gamification profile
    UserProfile(Address),
    /// User's earned achievement badges
    UserBadges(Address),
    /// Leaderboard for a specific metric
    Leaderboard(LeaderboardMetric),
    /// Active challenge campaigns
    ActiveChallenge(u64),
    /// Challenge list
    ChallengeList,
    /// Challenge progress for a user
    ChallengeProgress(u64, Address),
    /// Reward distribution history
    RewardHistory,
    /// Global gamification stats
    GlobalStats,
    /// Next challenge ID counter
    NextChallengeId,
    /// Score breakdowns for users
    ScoreBreakdown(Address),
}

// ────────────────────────────────────────────────────────────────────────────
// Gamification Engine
// ────────────────────────────────────────────────────────────────────────────

pub struct GamificationEngine;

impl GamificationEngine {
    // ════════════════════════════════════════════════════════════════════════
    // Profile Management
    // ════════════════════════════════════════════════════════════════════════

    /// Get or create a user's gamification profile
    pub fn get_or_create_profile(env: &Env, user: &Address) -> GamificationProfile {
        env.storage()
            .persistent()
            .get(&GamificationStorageKey::UserProfile(user.clone()))
            .unwrap_or_else(|| GamificationProfile::new(env))
    }

    /// Save a user's gamification profile
    pub fn save_profile(env: &Env, user: &Address, profile: &GamificationProfile) {
        env.storage()
            .persistent()
            .set(&GamificationStorageKey::UserProfile(user.clone()), profile);
    }

    /// Update a user's activity timestamp and streak
    pub fn record_activity(env: &Env, user: &Address) {
        let mut profile = Self::get_or_create_profile(env, user);
        let now = env.ledger().timestamp();

        if profile.last_activity_timestamp > 0 {
            let elapsed = now.saturating_sub(profile.last_activity_timestamp);
            // 86400 seconds = 1 day
            if elapsed <= 86400 * 2 {
                // Within 2 days = continue streak
                if elapsed > 0 && elapsed <= 86400 {
                    profile.current_daily_streak = profile.current_daily_streak.saturating_add(1);
                }
            } else {
                // Streak broken
                profile.current_daily_streak = 1;
            }
        } else {
            profile.current_daily_streak = 1;
        }

        if profile.current_daily_streak > profile.best_daily_streak {
            profile.best_daily_streak = profile.current_daily_streak;
        }

        profile.last_activity_timestamp = now;
        Self::save_profile(env, user, &profile);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Achievement Badge System
    // ════════════════════════════════════════════════════════════════════════

    /// Get all badges for a user
    pub fn get_user_badges(env: &Env, user: &Address) -> Vec<AchievementBadge> {
        env.storage()
            .persistent()
            .get(&GamificationStorageKey::UserBadges(user.clone()))
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Award a badge to a user if not already earned. Returns true if newly awarded.
    pub fn award_badge(env: &Env, user: &Address, badge: AchievementBadge) -> bool {
        let mut badges = Self::get_user_badges(env, user);

        // Check for duplicate
        for i in 0..badges.len() {
            if let Some(existing) = badges.get(i) {
                if existing == badge {
                    return false;
                }
            }
        }

        badges.push_back(badge.clone());
        env.storage()
            .persistent()
            .set(&GamificationStorageKey::UserBadges(user.clone()), &badges);

        // Award score points for badge
        let points = Self::badge_points(&badge);
        Self::add_score(env, user, points, symbol_short!("badge"));

        // Emit event
        env.events().publish(
            (Symbol::new(env, "BadgeAwarded"), user.clone()),
            (badge, points, env.ledger().timestamp()),
        );

        true
    }

    /// Check if user has a specific badge
    pub fn has_badge(env: &Env, user: &Address, badge: &AchievementBadge) -> bool {
        let badges = Self::get_user_badges(env, user);
        for i in 0..badges.len() {
            if let Some(existing) = badges.get(i) {
                if &existing == badge {
                    return true;
                }
            }
        }
        false
    }

    /// Get score points for a badge
    fn badge_points(badge: &AchievementBadge) -> i128 {
        match badge {
            // Trading milestones
            AchievementBadge::FirstTrade => 50,
            AchievementBadge::TenTrades => 100,
            AchievementBadge::FiftyTrades => 300,
            AchievementBadge::HundredTrades => 750,
            AchievementBadge::ThousandTrades => 5000,

            // Performance milestones
            AchievementBadge::ProfitableTrader => 200,
            AchievementBadge::HighRoller => 300,
            AchievementBadge::ConsistentPerformer => 500,
            AchievementBadge::PerfectWeek => 400,

            // Portfolio milestones
            AchievementBadge::WealthBuilder => 1000,
            AchievementBadge::DiversifiedPortfolio => 150,
            AchievementBadge::LPProvider => 100,
            AchievementBadge::DeepLiquidity => 300,

            // Social & Learning
            AchievementBadge::CommunityContributor => 200,
            AchievementBadge::ChallengeChampion => 500,
            AchievementBadge::StreakMaster => 400,

            // Special
            AchievementBadge::EarlyAdopter => 500,
            AchievementBadge::SeasonChampion => 2000,
            AchievementBadge::MasterTrader => 5000,
        }
    }

    /// Check all badge conditions and award applicable badges
    pub fn check_and_award_achievements(
        env: &Env,
        user: &Address,
        trade_count: u32,
        winning_trades: u32,
        pnl: i128,
        trade_streak: u32,
        daily_streak: u32,
        unique_pairs: u32,
        lp_provided: bool,
        liquidity_amount: i128,
        referral_count: u32,
        challenges_won: u32,
    ) -> Vec<AchievementBadge> {
        let mut newly_awarded = Vec::new(env);

        // Check each badge condition and award if met
        let badge_checks = [
            (AchievementBadge::FirstTrade, trade_count >= 1),
            (AchievementBadge::TenTrades, trade_count >= 10),
            (AchievementBadge::FiftyTrades, trade_count >= 50),
            (AchievementBadge::HundredTrades, trade_count >= 100),
            (AchievementBadge::ThousandTrades, trade_count >= 1000),
            (AchievementBadge::ProfitableTrader, winning_trades >= 10),
            (AchievementBadge::ConsistentPerformer, trade_streak >= 7),
            (AchievementBadge::PerfectWeek, trade_streak >= 7),
            (AchievementBadge::WealthBuilder, pnl >= 10_000_000),
            (AchievementBadge::DiversifiedPortfolio, unique_pairs >= 5),
            (AchievementBadge::LPProvider, lp_provided),
            (AchievementBadge::DeepLiquidity, liquidity_amount >= 1_000),
            (AchievementBadge::CommunityContributor, referral_count >= 3),
            (AchievementBadge::ChallengeChampion, challenges_won >= 3),
            (AchievementBadge::StreakMaster, daily_streak >= 30),
        ];

        for (badge, condition) in badge_checks.iter() {
            if *condition {
                if Self::award_badge(env, user, badge.clone()) {
                    newly_awarded.push_back(badge.clone());
                }
            }
        }

        // Check MasterTrader: must have all trading badges
        if Self::has_badge(env, user, &AchievementBadge::FirstTrade)
            && Self::has_badge(env, user, &AchievementBadge::TenTrades)
            && Self::has_badge(env, user, &AchievementBadge::HundredTrades)
            && Self::has_badge(env, user, &AchievementBadge::ProfitableTrader)
            && Self::has_badge(env, user, &AchievementBadge::ConsistentPerformer)
        {
            if Self::award_badge(env, user, AchievementBadge::MasterTrader) {
                newly_awarded.push_back(AchievementBadge::MasterTrader);
            }
        }

        newly_awarded
    }

    // ════════════════════════════════════════════════════════════════════════
    // Dynamic Scoring System
    // ════════════════════════════════════════════════════════════════════════

    /// Add score points to a user's profile and update tier
    pub fn add_score(env: &Env, user: &Address, points: i128, _reason: Symbol) {
        if points <= 0 {
            return;
        }

        let mut profile = Self::get_or_create_profile(env, user);
        let old_tier = profile.tier.clone();

        profile.score = profile.score.saturating_add(points);
        profile.lifetime_score = profile.lifetime_score.saturating_add(points);

        // Update tier based on lifetime score
        profile.tier = Self::calculate_tier(profile.lifetime_score);

        Self::save_profile(env, user, &profile);

        // Update leaderboard
        Self::update_leaderboard_entry(env, user, &LeaderboardMetric::ByScore, profile.score);

        // Emit tier change event if tier changed
        if old_tier != profile.tier {
            env.events().publish(
                (Symbol::new(env, "TierUpgraded"), user.clone()),
                (old_tier, profile.tier.clone(), env.ledger().timestamp()),
            );
        }
    }

    /// Calculate score from trading performance metrics
    pub fn calculate_trading_score(
        trade_count: u32,
        winning_trades: u32,
        total_pnl: i128,
        volume: i128,
    ) -> i128 {
        let mut score: i128 = 0;

        // Base points per trade (diminishing returns past 100)
        let effective_trades = if trade_count > 100 {
            100 + ((trade_count - 100) as i128 / 10)
        } else {
            trade_count as i128
        };
        score = score.saturating_add(effective_trades * 10);

        // Win rate bonus (up to 500 points)
        if trade_count > 0 {
            let win_rate_bps = (winning_trades as i128 * 10_000) / (trade_count as i128);
            let win_bonus = (win_rate_bps * 500) / 10_000;
            score = score.saturating_add(win_bonus);
        }

        // PnL bonus (scaled, up to 1000 points)
        let pnl_bonus = if total_pnl > 0 {
            core::cmp::min(total_pnl / 1000, 1000)
        } else {
            0
        };
        score = score.saturating_add(pnl_bonus);

        // Volume bonus (up to 500 points)
        let vol_bonus = core::cmp::min(volume / 10_000, 500);
        score = score.saturating_add(vol_bonus);

        score
    }

    /// Calculate composite score from all factors
    pub fn calculate_composite_score(
        trading_score: i128,
        performance_score: i128,
        learning_score: i128,
        community_score: i128,
        streak_bonus: i128,
        challenge_bonus: i128,
    ) -> i128 {
        // Weighted composite: 40% trading, 25% performance, 15% learning,
        // 10% community, 5% streak, 5% challenge
        let weighted = trading_score.saturating_mul(40)
            .saturating_add(performance_score.saturating_mul(25))
            .saturating_add(learning_score.saturating_mul(15))
            .saturating_add(community_score.saturating_mul(10))
            .saturating_add(streak_bonus.saturating_mul(5))
            .saturating_add(challenge_bonus.saturating_mul(5));

        weighted / 100
    }

    /// Get score breakdown for a user
    pub fn get_score_breakdown(env: &Env, user: &Address) -> ScoreBreakdown {
        env.storage()
            .persistent()
            .get(&GamificationStorageKey::ScoreBreakdown(user.clone()))
            .unwrap_or(ScoreBreakdown {
                trading_score: 0,
                performance_score: 0,
                learning_score: 0,
                community_score: 0,
                streak_bonus: 0,
                challenge_bonus: 0,
            })
    }

    /// Save score breakdown
    pub fn save_score_breakdown(env: &Env, user: &Address, breakdown: &ScoreBreakdown) {
        env.storage()
            .persistent()
            .set(&GamificationStorageKey::ScoreBreakdown(user.clone()), breakdown);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Progression Tier System
    // ════════════════════════════════════════════════════════════════════════

    /// Calculate progression tier from lifetime score
    pub fn calculate_tier(lifetime_score: i128) -> ProgressionTier {
        if lifetime_score >= ProgressionTier::Diamond.score_threshold() {
            ProgressionTier::Diamond
        } else if lifetime_score >= ProgressionTier::Platinum.score_threshold() {
            ProgressionTier::Platinum
        } else if lifetime_score >= ProgressionTier::Gold.score_threshold() {
            ProgressionTier::Gold
        } else if lifetime_score >= ProgressionTier::Silver.score_threshold() {
            ProgressionTier::Silver
        } else {
            ProgressionTier::Bronze
        }
    }

    /// Get user's current progression tier
    pub fn get_user_tier(env: &Env, user: &Address) -> ProgressionTier {
        let profile = Self::get_or_create_profile(env, user);
        profile.tier
    }

    /// Get progress toward next tier (current score, threshold)
    pub fn get_tier_progress(env: &Env, user: &Address) -> (i128, i128) {
        let profile = Self::get_or_create_profile(env, user);
        let current = profile.lifetime_score;

        match profile.tier.next_tier() {
            Some(next) => (current, next.score_threshold()),
            None => (current, current), // Already at max tier
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Streak Tracking
    // ════════════════════════════════════════════════════════════════════════

    /// Record a profitable trade and update streak
    pub fn record_profitable_trade(env: &Env, user: &Address) {
        let mut profile = Self::get_or_create_profile(env, user);
        profile.current_trade_streak = profile.current_trade_streak.saturating_add(1);

        if profile.current_trade_streak > profile.best_trade_streak {
            profile.best_trade_streak = profile.current_trade_streak;
        }

        // Streak milestones
        let streak = profile.current_trade_streak;
        Self::save_profile(env, user, &profile);

        if streak >= 5 {
            Self::add_score(env, user, 25, symbol_short!("streak5"));
        }
        if streak >= 10 {
            Self::add_score(env, user, 50, symbol_short!("strk10"));
        }
        if streak >= 25 {
            Self::add_score(env, user, 150, symbol_short!("strk25"));
        }
    }

    /// Record a losing trade and break the streak
    pub fn record_losing_trade(env: &Env, user: &Address) {
        let mut profile = Self::get_or_create_profile(env, user);
        profile.current_trade_streak = 0;
        Self::save_profile(env, user, &profile);
    }

    /// Get streak multiplier for rewards (basis points, 1.0x = 10000)
    pub fn streak_multiplier_bps(env: &Env, user: &Address) -> i128 {
        let profile = Self::get_or_create_profile(env, user);
        let streak = profile.current_trade_streak;

        // Multiplier: 1.0x base + 0.1x per 5 streaks, capped at 2.0x
        let bonus = core::cmp::min((streak as i128 / 5) * 1_000, 10_000);
        10_000 + bonus
    }

    /// Get best streak for a user
    pub fn get_best_streak(env: &Env, user: &Address) -> u32 {
        let profile = Self::get_or_create_profile(env, user);
        profile.best_trade_streak
    }

    // ════════════════════════════════════════════════════════════════════════
    // Leaderboard System
    // ════════════════════════════════════════════════════════════════════════

    /// Update a user's entry on a leaderboard
    pub fn update_leaderboard_entry(
        env: &Env,
        user: &Address,
        metric: &LeaderboardMetric,
        score: i128,
    ) {
        let mut leaderboard: Vec<LeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::Leaderboard(metric.clone()))
            .unwrap_or_else(|| Vec::new(env));

        // Remove existing entry for this user
        let mut found_index: Option<u32> = None;
        for i in 0..leaderboard.len() {
            if let Some(entry) = leaderboard.get(i) {
                if entry.user == *user {
                    found_index = Some(i);
                    break;
                }
            }
        }

        if let Some(idx) = found_index {
            leaderboard.remove(idx);
        }

        // Insert new entry
        leaderboard.push_back(LeaderboardEntry {
            user: user.clone(),
            score,
            rank: 0, // Will be assigned during sort
        });

        // Sort by score descending (insertion sort for small list)
        for i in (1..leaderboard.len()).rev() {
            if let (Some(curr), Some(prev)) = (leaderboard.get(i), leaderboard.get(i - 1)) {
                if curr.score > prev.score {
                    let temp = leaderboard.get(i).unwrap();
                    leaderboard.set(i, leaderboard.get(i - 1).unwrap());
                    leaderboard.set(i - 1, temp);
                } else {
                    break;
                }
            }
        }

        // Cap at max size
        while leaderboard.len() > MAX_LEADERBOARD_SIZE {
            leaderboard.pop_back();
        }

        // Assign ranks
        for i in 0..leaderboard.len() {
            if let Some(mut entry) = leaderboard.get(i) {
                entry.rank = i + 1;
                leaderboard.set(i, entry);
            }
        }

        env.storage().persistent().set(
            &GamificationStorageKey::Leaderboard(metric.clone()),
            &leaderboard,
        );
    }

    /// Get paginated leaderboard for a metric
    pub fn get_leaderboard(
        env: &Env,
        metric: &LeaderboardMetric,
        page: u32,
        page_size: u32,
    ) -> Vec<LeaderboardEntry> {
        let leaderboard: Vec<LeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::Leaderboard(metric.clone()))
            .unwrap_or_else(|| Vec::new(env));

        let start = (page * page_size) as usize;
        let mut result = Vec::new(env);

        for i in start..leaderboard.len() as usize {
            if let Some(entry) = leaderboard.get(i as u32) {
                result.push_back(entry);
            }
            if result.len() as u32 >= page_size {
                break;
            }
        }

        result
    }

    /// Get a user's rank on a specific leaderboard
    pub fn get_user_rank(env: &Env, user: &Address, metric: &LeaderboardMetric) -> Option<u32> {
        let leaderboard: Vec<LeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::Leaderboard(metric.clone()))
            .unwrap_or_else(|| Vec::new(env));

        for i in 0..leaderboard.len() {
            if let Some(entry) = leaderboard.get(i) {
                if entry.user == *user {
                    return Some(entry.rank);
                }
            }
        }
        None
    }

    /// Get leaderboard size
    pub fn get_leaderboard_size(env: &Env, metric: &LeaderboardMetric) -> u32 {
        let leaderboard: Vec<LeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::Leaderboard(metric.clone()))
            .unwrap_or_else(|| Vec::new(env));
        leaderboard.len()
    }

    // ════════════════════════════════════════════════════════════════════════
    // Challenge Campaign System
    // ════════════════════════════════════════════════════════════════════════

    /// Create a new challenge campaign (admin only)
    pub fn create_challenge(
        env: &Env,
        name: Symbol,
        description: Symbol,
        objective: ChallengeObjective,
        duration_secs: u64,
        reward_pool: i128,
        max_participants: u32,
        min_tier: ProgressionTier,
    ) -> Result<u64, GamificationError> {
        if duration_secs < MIN_CHALLENGE_DURATION || duration_secs > MAX_CHALLENGE_DURATION {
            return Err(GamificationError::InvalidChallengeDuration);
        }

        let next_id: u64 = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::NextChallengeId)
            .unwrap_or(1);

        let now = env.ledger().timestamp();
        let name_clone = name.clone();
        let challenge = ChallengeCampaign {
            challenge_id: next_id,
            name: name.clone(),
            description: description.clone(),
            objective,
            start_time: now,
            end_time: now + duration_secs,
            reward_pool,
            status: ChallengeStatus::Active,
            max_participants,
            participant_count: 0,
            min_tier,
        };

        env.storage().persistent().set(
            &GamificationStorageKey::ActiveChallenge(next_id),
            &challenge,
        );
        env.storage().persistent().set(
            &GamificationStorageKey::NextChallengeId,
            &(next_id + 1),
        );

        // Add to challenge list
        let mut list: Vec<u64> = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::ChallengeList)
            .unwrap_or_else(|| Vec::new(env));
        list.push_back(next_id);
        env.storage()
            .persistent()
            .set(&GamificationStorageKey::ChallengeList, &list);

        // Emit event
        env.events().publish(
            (Symbol::new(env, "ChallengeCreated"),),
            (next_id, name_clone, reward_pool, env.ledger().timestamp()),
        );

        Ok(next_id)
    }

    /// Join a challenge campaign
    pub fn join_challenge(
        env: &Env,
        user: &Address,
        challenge_id: u64,
    ) -> Result<(), GamificationError> {
        let mut challenge: ChallengeCampaign = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::ActiveChallenge(challenge_id))
            .ok_or(GamificationError::ChallengeNotFound)?;

        if challenge.status != ChallengeStatus::Active {
            return Err(GamificationError::ChallengeNotActive);
        }

        let now = env.ledger().timestamp();
        if now < challenge.start_time || now > challenge.end_time {
            return Err(GamificationError::ChallengeNotActive);
        }

        // Check tier requirement
        let user_tier = Self::get_user_tier(env, user);
        if user_tier < challenge.min_tier {
            return Err(GamificationError::InsufficientTier);
        }

        // Check participation limit
        if challenge.max_participants > 0
            && challenge.participant_count >= challenge.max_participants
        {
            return Err(GamificationError::ChallengeFull);
        }

        // Check if already joined
        let progress: Option<ChallengeProgress> = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::ChallengeProgress(challenge_id, user.clone()));
        if progress.is_some() {
            return Err(GamificationError::AlreadyJoined);
        }

        // Initialize progress
        let progress = ChallengeProgress {
            current_progress: 0,
            objective_met: false,
            reward_claimed: false,
            completed_at: None,
        };

        env.storage().persistent().set(
            &GamificationStorageKey::ChallengeProgress(challenge_id, user.clone()),
            &progress,
        );

        challenge.participant_count = challenge.participant_count.saturating_add(1);
        env.storage().persistent().set(
            &GamificationStorageKey::ActiveChallenge(challenge_id),
            &challenge,
        );

        // Update profile
        let mut profile = Self::get_or_create_profile(env, user);
        profile.challenges_participated = profile.challenges_participated.saturating_add(1);
        Self::save_profile(env, user, &profile);

        // Emit event
        env.events().publish(
            (Symbol::new(env, "ChallengeJoined"), user.clone()),
            (challenge_id, env.ledger().timestamp()),
        );

        Ok(())
    }

    /// Update progress in a challenge
    pub fn update_challenge_progress(
        env: &Env,
        user: &Address,
        challenge_id: u64,
        progress_increment: i128,
    ) -> Result<(), GamificationError> {
        let mut challenge: ChallengeCampaign = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::ActiveChallenge(challenge_id))
            .ok_or(GamificationError::ChallengeNotFound)?;

        if challenge.status != ChallengeStatus::Active {
            return Err(GamificationError::ChallengeNotActive);
        }

        let now = env.ledger().timestamp();
        if now > challenge.end_time {
            return Err(GamificationError::ChallengeExpired);
        }

        let mut progress: ChallengeProgress = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::ChallengeProgress(
                challenge_id,
                user.clone(),
            ))
            .ok_or(GamificationError::NotParticipant)?;

        if progress.objective_met {
            return Ok(()); // Already completed
        }

        progress.current_progress = progress.current_progress.saturating_add(progress_increment);

        // Check if objective is met
        let target = Self::challenge_target(&challenge.objective);
        if progress.current_progress >= target {
            progress.objective_met = true;
            progress.completed_at = Some(now);

            // Award completion score
            let bonus = Self::challenge_completion_score(&challenge.objective);
            Self::add_score(env, user, bonus, symbol_short!("chlg"));

            // Emit completion event
            env.events().publish(
                (Symbol::new(env, "ChallengeCompleted"), user.clone()),
                (challenge_id, progress.current_progress, env.ledger().timestamp()),
            );
        }

        env.storage().persistent().set(
            &GamificationStorageKey::ChallengeProgress(challenge_id, user.clone()),
            &progress,
        );

        Ok(())
    }

    /// Claim challenge reward
    pub fn claim_challenge_reward(
        env: &Env,
        user: &Address,
        challenge_id: u64,
    ) -> Result<i128, GamificationError> {
        let challenge: ChallengeCampaign = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::ActiveChallenge(challenge_id))
            .ok_or(GamificationError::ChallengeNotFound)?;

        let mut progress: ChallengeProgress = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::ChallengeProgress(
                challenge_id,
                user.clone(),
            ))
            .ok_or(GamificationError::NotParticipant)?;

        if !progress.objective_met {
            return Err(GamificationError::ObjectiveNotMet);
        }

        if progress.reward_claimed {
            return Err(GamificationError::RewardAlreadyClaimed);
        }

        // Calculate reward: base reward * tier multiplier * streak multiplier
        let mut profile = Self::get_or_create_profile(env, user);
        let tier_mult = profile.tier.reward_multiplier_bps();
        let streak_mult = Self::streak_multiplier_bps(env, user);

        let base_reward = challenge.reward_pool / 10; // 10% of pool per claimer (simplified)
        let reward = (base_reward * tier_mult * streak_mult) / (10_000 * 10_000);

        profile.pending_rewards = profile.pending_rewards.saturating_add(reward);
        Self::save_profile(env, user, &profile);

        progress.reward_claimed = true;
        env.storage().persistent().set(
            &GamificationStorageKey::ChallengeProgress(challenge_id, user.clone()),
            &progress,
        );

        // Record distribution
        Self::record_reward_distribution(
            env,
            user,
            reward,
            symbol_short!("chlg_rwd"),
            Some(challenge_id),
        );

        // Update profile stats
        let mut profile = Self::get_or_create_profile(env, user);
        profile.challenges_won = profile.challenges_won.saturating_add(1);
        Self::save_profile(env, user, &profile);

        // Check for ChallengeChampion badge
        if profile.challenges_won >= 3 {
            Self::award_badge(env, user, AchievementBadge::ChallengeChampion);
        }

        // Emit event
        env.events().publish(
            (Symbol::new(env, "RewardClaimed"), user.clone()),
            (challenge_id, reward, env.ledger().timestamp()),
        );

        Ok(reward)
    }

    /// Get challenge details
    pub fn get_challenge(
        env: &Env,
        challenge_id: u64,
    ) -> Option<ChallengeCampaign> {
        env.storage()
            .persistent()
            .get(&GamificationStorageKey::ActiveChallenge(challenge_id))
    }

    /// Get user's challenge progress
    pub fn get_challenge_progress(
        env: &Env,
        user: &Address,
        challenge_id: u64,
    ) -> Option<ChallengeProgress> {
        env.storage().persistent().get(
            &GamificationStorageKey::ChallengeProgress(challenge_id, user.clone()),
        )
    }

    /// Get all active challenge IDs
    pub fn get_active_challenges(env: &Env) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&GamificationStorageKey::ChallengeList)
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Complete expired challenges
    pub fn finalize_expired_challenges(env: &Env) {
        let now = env.ledger().timestamp();
        let challenge_ids = Self::get_active_challenges(env);

        for i in 0..challenge_ids.len() {
            if let Some(id) = challenge_ids.get(i) {
                if let Some(mut challenge) = Self::get_challenge(env, id) {
                    if challenge.status == ChallengeStatus::Active && now > challenge.end_time {
                        challenge.status = ChallengeStatus::Completed;
                        env.storage().persistent().set(
                            &GamificationStorageKey::ActiveChallenge(id),
                            &challenge,
                        );

                        env.events().publish(
                            (Symbol::new(env, "ChallengeFinalized"),),
                            (id, now),
                        );
                    }
                }
            }
        }
    }

    fn challenge_target(objective: &ChallengeObjective) -> i128 {
        match objective {
            ChallengeObjective::CompleteTrades(n) => *n as i128,
            ChallengeObjective::AchieveROI(r) => *r,
            ChallengeObjective::TradeVolume(v) => *v,
            ChallengeObjective::WinStreak(n) => *n as i128,
            ChallengeObjective::Diversify(n) => *n as i128,
            ChallengeObjective::LiquidityProvision(v) => *v,
        }
    }

    fn challenge_completion_score(objective: &ChallengeObjective) -> i128 {
        match objective {
            ChallengeObjective::CompleteTrades(_) => 100,
            ChallengeObjective::AchieveROI(_) => 200,
            ChallengeObjective::TradeVolume(_) => 150,
            ChallengeObjective::WinStreak(_) => 300,
            ChallengeObjective::Diversify(_) => 100,
            ChallengeObjective::LiquidityProvision(_) => 150,
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Reward Distribution
    // ════════════════════════════════════════════════════════════════════════

    /// Record a reward distribution for audit trail
    pub fn record_reward_distribution(
        env: &Env,
        user: &Address,
        amount: i128,
        reason: Symbol,
        challenge_id: Option<u64>,
    ) {
        let distribution = RewardDistribution {
            user: user.clone(),
            amount,
            reason,
            timestamp: env.ledger().timestamp(),
            challenge_id,
        };

        let mut history: Vec<RewardDistribution> = env
            .storage()
            .persistent()
            .get(&GamificationStorageKey::RewardHistory)
            .unwrap_or_else(|| Vec::new(env));
        history.push_back(distribution);
        env.storage()
            .persistent()
            .set(&GamificationStorageKey::RewardHistory, &history);
    }

    /// Claim accumulated rewards
    pub fn claim_rewards(env: &Env, user: &Address) -> Result<i128, GamificationError> {
        let mut profile = Self::get_or_create_profile(env, user);

        if profile.pending_rewards <= 0 {
            return Err(GamificationError::NoRewardsPending);
        }

        let amount = profile.pending_rewards;
        profile.pending_rewards = 0;
        profile.total_rewards_earned = profile.total_rewards_earned.saturating_add(amount);
        Self::save_profile(env, user, &profile);

        // Record distribution
        Self::record_reward_distribution(env, user, amount, symbol_short!("claim"), None);

        // Emit event
        env.events().publish(
            (Symbol::new(env, "RewardsClaimed"), user.clone()),
            (amount, env.ledger().timestamp()),
        );

        Ok(amount)
    }

    /// Get pending rewards for a user
    pub fn get_pending_rewards(env: &Env, user: &Address) -> i128 {
        let profile = Self::get_or_create_profile(env, user);
        profile.pending_rewards
    }

    /// Get total lifetime rewards
    pub fn get_lifetime_rewards(env: &Env, user: &Address) -> i128 {
        let profile = Self::get_or_create_profile(env, user);
        profile.total_rewards_earned
    }

    /// Get reward distribution history
    pub fn get_reward_history(env: &Env) -> Vec<RewardDistribution> {
        env.storage()
            .persistent()
            .get(&GamificationStorageKey::RewardHistory)
            .unwrap_or_else(|| Vec::new(env))
    }

    // ════════════════════════════════════════════════════════════════════════
    // Global Stats
    // ════════════════════════════════════════════════════════════════════════

    /// Get global gamification statistics
    pub fn get_global_stats(env: &Env) -> GamificationGlobalStats {
        env.storage()
            .persistent()
            .get(&GamificationStorageKey::GlobalStats)
            .unwrap_or(GamificationGlobalStats::new())
    }

    /// Update global stats
    pub fn record_global_badge_award(env: &Env) {
        let mut stats = Self::get_global_stats(env);
        stats.total_badges_awarded = stats.total_badges_awarded.saturating_add(1);
        env.storage()
            .persistent()
            .set(&GamificationStorageKey::GlobalStats, &stats);
    }

    /// Record a global challenge completion
    pub fn record_global_challenge_completion(env: &Env) {
        let mut stats = Self::get_global_stats(env);
        stats.total_challenges_completed = stats.total_challenges_completed.saturating_add(1);
        env.storage()
            .persistent()
            .set(&GamificationStorageKey::GlobalStats, &stats);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Error Types
// ────────────────────────────────────────────────────────────────────────────

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GamificationError {
    ChallengeNotFound = 1,
    ChallengeNotActive = 2,
    ChallengeExpired = 3,
    ChallengeFull = 4,
    InvalidChallengeDuration = 5,
    AlreadyJoined = 6,
    NotParticipant = 7,
    ObjectiveNotMet = 8,
    RewardAlreadyClaimed = 9,
    NoRewardsPending = 10,
    InsufficientTier = 11,
    NotAdmin = 12,
}

// ────────────────────────────────────────────────────────────────────────────
// Global Stats
// ────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
#[contracttype]
pub struct GamificationGlobalStats {
    pub total_badges_awarded: u64,
    pub total_challenges_created: u64,
    pub total_challenges_completed: u64,
    pub total_rewards_distributed: i128,
    pub total_active_users: u64,
}

impl GamificationGlobalStats {
    pub fn new() -> Self {
        Self {
            total_badges_awarded: 0,
            total_challenges_created: 0,
            total_challenges_completed: 0,
            total_rewards_distributed: 0,
            total_active_users: 0,
        }
    }
}
