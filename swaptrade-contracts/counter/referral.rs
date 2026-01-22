use soroban_sdk::{contracttype, Address, Env, Symbol, Map, Vec};

#[derive(Clone)]
#[contracttype]
pub struct ReferralSystem {
    // Maps user addresses to their referral codes
    referral_codes: Map<Address, Symbol>,
    
    // Maps referral codes to the user who created them
    code_to_user: Map<Symbol, Address>,
    
    // Maps referrers to the list of referees they invited
    referrals: Map<Address, Vec<Address>>,
    
    // Maps referees to their referrer
    referee_to_referrer: Map<Address, Address>,
    
    // Tracks accumulated rewards for each user from referrals
    referral_rewards: Map<Address, i128>,
    
    // Tracks number of trades made by each referee to apply discount limits
    referee_trade_counts: Map<Address, u32>,
    
    // Tracks whether a user has used a referral code (to prevent multiple registrations)
    registered_with_referral: Map<Address, bool>,
}

impl ReferralSystem {
    pub fn new(env: &Env) -> Self {
        Self {
            referral_codes: Map::new(env),
            code_to_user: Map::new(env),
            referrals: Map::new(env),
            referee_to_referrer: Map::new(env),
            referral_rewards: Map::new(env),
            referee_trade_counts: Map::new(env),
            registered_with_referral: Map::new(env),
        }
    }

    /// Generate a unique referral code for a user
    pub fn generate_referral_code(&mut self, env: &Env, user: Address) -> Symbol {
        // Check if user already has a referral code
        if self.referral_codes.contains_key(user.clone()) {
            return self.referral_codes.get(user).unwrap();
        }

        // Generate a unique 8-character alphanumeric referral code
        let code = self.generate_unique_code(env);
        
        // Store the mapping
        self.referral_codes.set(user.clone(), code);
        self.code_to_user.set(code, user);

        code
    }

    /// Register a new user with a referral code
    pub fn register_with_referral(&mut self, env: &Env, referral_code: Symbol, new_user: Address) -> Result<(), &'static str> {
        // Prevent duplicate registration
        if self.registered_with_referral.contains_key(new_user.clone()) {
            return Err("User already registered");
        }

        // Validate referral code exists
        if !self.code_to_user.contains_key(referral_code) {
            return Err("Invalid referral code");
        }

        // Get the referrer from the code
        let referrer = self.code_to_user.get(referral_code).unwrap();

        // Prevent self-referral
        if referrer == new_user {
            return Err("Cannot refer yourself");
        }

        // Check if user was already referred by someone else (circular reference prevention)
        if self.referee_to_referrer.contains_key(new_user.clone()) {
            return Err("User already has a referrer");
        }

        // Link referee to referrer
        self.referee_to_referrer.set(new_user.clone(), referrer.clone());

        // Add referee to referrer's referral list
        let mut ref_list = self.referrals.get(referrer.clone()).unwrap_or_else(|| Vec::new(env));
        ref_list.push_back(new_user.clone());
        self.referrals.set(referrer, ref_list);

        // Mark user as registered with referral
        self.registered_with_referral.set(new_user, true);

        Ok(())
    }

    /// Get referral code for a user
    pub fn get_referral_code(&self, env: &Env, user: Address) -> Symbol {
        self.referral_codes.get(user).unwrap_or(Symbol::new(env, ""))
    }

    /// Get list of referrals for a user
    pub fn get_referrals(&self, env: &Env, user: Address) -> Vec<Address> {
        self.referrals.get(user).unwrap_or_else(|| Vec::new(env))
    }

    /// Get referral rewards for a user
    pub fn get_referral_rewards(&self, env: &Env, user: Address) -> i128 {
        self.referral_rewards.get(user).unwrap_or(0)
    }

    /// Process a trade for referral rewards
    /// Returns the discount percentage for the referee (0-10)
    pub fn process_trade_for_referral(&mut self, env: &Env, referee: Address, trade_fee: i128) -> i128 {
        // Check if the referee has a referrer
        if !self.referee_to_referrer.contains_key(referee.clone()) {
            return 0; // No discount
        }

        let referrer = self.referee_to_referrer.get(referee.clone()).unwrap();

        // Calculate the discount for the referee (10% off for first 50 trades)
        let current_trade_count = self.referee_trade_counts.get(referee.clone()).unwrap_or(0);
        let discount_percentage = if current_trade_count < 50 {
            10  // 10% discount
        } else {
            0   // No discount after 50 trades
        };

        // Increment trade count for the referee
        self.referee_trade_counts.set(referee.clone(), current_trade_count + 1);

        // Calculate referral reward for referrer (5% of trade fee)
        let referral_reward = (trade_fee * 5) / 100; // 5% of fee

        // Add referral reward to referrer's balance
        let current_rewards = self.referral_rewards.get(referrer.clone()).unwrap_or(0);
        self.referral_rewards.set(referrer, current_rewards + referral_reward);

        discount_percentage
    }

    /// Claim referral rewards for a user
    pub fn claim_referral_rewards(&mut self, env: &Env, user: Address) -> i128 {
        let rewards = self.get_referral_rewards(env, user.clone());
        
        if rewards > 0 {
            self.referral_rewards.set(user, 0); // Reset rewards to 0
        }

        rewards
    }

    /// Generate a unique referral code
    fn generate_unique_code(&mut self, env: &Env) -> Symbol {
        // In a real implementation, we'd want to ensure uniqueness more rigorously
        // For now, we'll use a simple approach based on environment data
        let mut attempts = 0;
        loop {
            // Generate a pseudo-random 8-character code
            let code_str = self.create_random_code(env, attempts);
            let code = Symbol::new(env, &code_str);

            // Check if code already exists
            if !self.code_to_user.contains_key(code) {
                return code;
            }

            attempts += 1;
            
            // Safety check to prevent infinite loop
            if attempts > 1000 {
                panic!("Could not generate unique referral code after 1000 attempts");
            }
        }
    }

    /// Create a random-looking referral code
    fn create_random_code(&self, env: &Env, attempt: u32) -> String {
        // Use ledger sequence number and attempt number to create pseudo-randomness
        let ledger_seq = env.ledger().sequence();
        let seed = ledger_seq as u64 + attempt as u64;
        
        // Convert to base36-like string (alphanumeric)
        let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut result = String::new();
        let mut temp_seed = seed;
        
        // Generate 8 characters
        for _ in 0..8 {
            let idx = (temp_seed % 36) as usize;
            if let Some(c) = chars.chars().nth(idx) {
                result.push(c);
            }
            temp_seed /= 36;
        }
        
        // Pad with 'A' if needed
        while result.len() < 8 {
            result.push('A');
        }
        
        // Ensure exactly 8 characters
        result[..8.min(result.len())].to_string()
    }
}