// governance/integration_tests.rs
//
// Comprehensive integration tests for the full governance module:
//   - Treasury management with multi-sig controls
//   - Protocol upgrade mechanism with version management
//   - Emergency controls and pause functionality
//   - Governance reward distribution

#[cfg(test)]
mod treasury_integration_tests {
    use super::super::treasury::*;

    fn create_treasury_with_funds() -> Treasury {
        let signers = vec![
            "alice".to_string(),
            "bob".to_string(),
            "carol".to_string(),
            "dave".to_string(),
            "eve".to_string(),
        ];
        let mut treasury = Treasury::new(signers, 3, 2);
        treasury
            .deposit("XLM", 10_000_000, "foundation", 100)
            .unwrap();
        treasury
            .deposit("USDC", 5_000_000, "foundation", 100)
            .unwrap();
        treasury.deposit("BTC", 100_000_000, "donor", 100).unwrap();
        treasury
    }

    #[test]
    fn test_full_withdrawal_lifecycle() {
        let mut treasury = create_treasury_with_funds();

        // Create withdrawal proposal
        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::Grant,
                "XLM",
                1_000_000,
                "community_wallet",
                "Q1 2024 community grants budget",
                200,
                86400 * 7,
            )
            .unwrap();

        // Collect approvals
        assert_eq!(
            treasury.approve_proposal(&proposal_id, "bob", 300).unwrap(),
            2
        );
        assert_eq!(
            treasury
                .approve_proposal(&proposal_id, "carol", 300)
                .unwrap(),
            3
        );

        // Verify approved status
        let proposal = treasury.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.status, TreasuryOperationStatus::Approved);

        // Execute
        let initial_balance = treasury.balance_of("XLM");
        treasury.execute_proposal(&proposal_id, 400).unwrap();
        assert_eq!(treasury.balance_of("XLM"), initial_balance - 1_000_000);

        // Verify audit trail
        assert!(treasury.verify_audit_trail());
    }

    #[test]
    fn test_emergency_withdrawal_fast_track() {
        let mut treasury = create_treasury_with_funds();

        // Emergency withdrawal only needs 2 approvals
        let proposal_id = treasury
            .create_proposal(
                "alice",
                ProposalType::EmergencyWithdrawal,
                "BTC",
                50_000_000,
                "security_team",
                "Urgent: patch vulnerability in bridge",
                200,
                86400, // 1 day TTL
            )
            .unwrap();

        // Only need 2 approvals for emergency
        treasury.approve_proposal(&proposal_id, "bob", 300).unwrap();
        let proposal = treasury.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.status, TreasuryOperationStatus::Approved);
        assert_eq!(proposal.required_approvals, 2);

        // Execute
        treasury.execute_proposal(&proposal_id, 400).unwrap();
        assert_eq!(treasury.balance_of("BTC"), 50_000_000);
    }

    #[test]
    fn test_budget_with_spending_limits() {
        let mut treasury = create_treasury_with_funds();

        // Set monthly spending limit: 2M XLM per 30 days
        treasury.set_spending_limit("XLM", 2_000_000, 2_592_000, 100);

        // First withdrawal: 1M XLM (within limit)
        let id1 = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                1_000_000,
                "grant_1",
                "First grant",
                200,
                86400 * 7,
            )
            .unwrap();
        treasury.approve_proposal(&id1, "bob", 200).unwrap();
        treasury.approve_proposal(&id1, "carol", 200).unwrap();
        treasury.execute_proposal(&id1, 300).unwrap();
        assert_eq!(treasury.balance_of("XLM"), 9_000_000);

        // Second withdrawal: 1M XLM (within remaining limit)
        let id2 = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                1_000_000,
                "grant_2",
                "Second grant",
                300,
                86400 * 7,
            )
            .unwrap();
        treasury.approve_proposal(&id2, "bob", 300).unwrap();
        treasury.approve_proposal(&id2, "carol", 300).unwrap();
        treasury.execute_proposal(&id2, 400).unwrap();
        assert_eq!(treasury.balance_of("XLM"), 8_000_000);

        // Third withdrawal: 500K XLM (over limit, should fail)
        let id3 = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                500_000,
                "grant_3",
                "Third grant",
                400,
                86400 * 7,
            )
            .unwrap();
        treasury.approve_proposal(&id3, "bob", 400).unwrap();
        treasury.approve_proposal(&id3, "carol", 400).unwrap();
        let result = treasury.execute_proposal(&id3, 500);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("spending limit"));
    }

    #[test]
    fn test_multisig_signer_rotation() {
        let mut treasury = create_treasury_with_funds();
        assert_eq!(treasury.signer_count(), 5);

        // Add new signer
        treasury.add_signer("frank".to_string()).unwrap();
        assert_eq!(treasury.signer_count(), 6);
        assert!(treasury.is_signer("frank"));

        // Remove old signer
        treasury.remove_signer("eve").unwrap();
        assert_eq!(treasury.signer_count(), 5);
        assert!(!treasury.is_signer("eve"));
        assert!(treasury.is_signer("frank"));

        // New signer can participate
        treasury.deposit("XLM", 100, "frank", 600).unwrap();
        assert_eq!(treasury.balance_of("XLM"), 10_000_100);
    }

    #[test]
    fn test_audit_trail_completeness() {
        let mut treasury = create_treasury_with_funds();

        // Perform various operations
        treasury.deposit("ETH", 500, "donor2", 200).unwrap();

        let id = treasury
            .create_proposal(
                "alice",
                ProposalType::Withdrawal,
                "XLM",
                100_000,
                "recip",
                "Test",
                300,
                86400 * 7,
            )
            .unwrap();

        treasury.approve_proposal(&id, "bob", 400).unwrap();
        treasury.approve_proposal(&id, "carol", 400).unwrap();
        treasury.execute_proposal(&id, 500).unwrap();

        treasury.update_threshold(4, 600).unwrap();

        // Verify complete audit trail
        assert!(treasury.verify_audit_trail());
        // 3 initial deposits + 1 ETH deposit + create + 2 approves + execute + threshold update = 9
        assert_eq!(treasury.audit_log_length(), 9);
    }
}

#[cfg(test)]
mod upgrade_integration_tests {
    use super::super::upgrade::*;

    fn setup_upgrade_manager() -> UpgradeManager {
        let signers = vec![
            "alice".to_string(),
            "bob".to_string(),
            "carol".to_string(),
            "dave".to_string(),
            "eve".to_string(),
        ];
        UpgradeManager::new(Version::new(1, 0, 0), signers, UpgradeConfig::default())
    }

    #[test]
    fn test_full_upgrade_lifecycle() {
        let mut mgr = setup_upgrade_manager();
        assert_eq!(mgr.current_version(), &Version::new(1, 0, 0));

        // Propose minor upgrade
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(1, 1, 0),
                UpgradeType::Minor,
                "Add limit order support",
                None,
                vec![
                    "Deploy new contract".to_string(),
                    "Migrate users".to_string(),
                ],
                100,
            )
            .unwrap();

        // Collect approvals (need 3 for minor)
        mgr.approve_upgrade(id, "bob", 200).unwrap();
        mgr.approve_upgrade(id, "carol", 200).unwrap();

        // Should now be in Timelock state
        let proposal = mgr.get_proposal(id).unwrap();
        assert_eq!(proposal.status, UpgradeStatus::Timelock);

        // Cannot execute before timelock
        assert!(mgr.execute_upgrade(id, 300).is_err());

        // Execute after timelock
        let timelock_end = 200 + 172800;
        mgr.execute_upgrade(id, timelock_end).unwrap();

        assert_eq!(mgr.current_version(), &Version::new(1, 1, 0));
        assert_eq!(mgr.history().len(), 1);
    }

    #[test]
    fn test_emergency_hotfix_workflow() {
        let mut mgr = setup_upgrade_manager();

        // Propose hotfix (1 hour timelock)
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(1, 0, 1),
                UpgradeType::Hotfix,
                "Fix critical rounding error",
                Some([0xAB; 32]),
                vec![],
                100,
            )
            .unwrap();

        // Approve (need 3 for hotfix = minor threshold)
        mgr.approve_upgrade(id, "bob", 100).unwrap();
        mgr.approve_upgrade(id, "carol", 100).unwrap();

        // Execute after 1 hour
        mgr.execute_upgrade(id, 100 + 3600).unwrap();
        assert_eq!(mgr.current_version(), &Version::new(1, 0, 1));
    }

    #[test]
    fn test_major_upgrade_with_rollback() {
        let mut mgr = setup_upgrade_manager();

        // Major upgrade: 1.0.0 -> 2.0.0
        let id = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "New AMM engine",
                None,
                vec![
                    "Migrate all pools".to_string(),
                    "Update oracles".to_string(),
                ],
                100,
            )
            .unwrap();

        // Need 5 approvals for major
        mgr.approve_upgrade(id, "bob", 200).unwrap();
        mgr.approve_upgrade(id, "carol", 200).unwrap();
        mgr.approve_upgrade(id, "dave", 200).unwrap();
        mgr.approve_upgrade(id, "eve", 200).unwrap();
        mgr.execute_upgrade(id, 200 + 172800).unwrap();

        assert_eq!(mgr.current_version(), &Version::new(2, 0, 0));

        // Critical bug found - rollback
        assert_eq!(mgr.rollback_version(), Some(&Version::new(1, 0, 0)));
        mgr.rollback("Critical: AMM exploit found", 200 + 172900)
            .unwrap();
        assert_eq!(mgr.current_version(), &Version::new(1, 0, 0));
        assert_eq!(mgr.history().len(), 2);
    }

    #[test]
    fn test_upgrade_chain() {
        let mut mgr = setup_upgrade_manager();

        // 1.0.0 -> 1.0.1 (patch)
        let id1 = mgr
            .propose_upgrade(
                "alice",
                Version::new(1, 0, 1),
                UpgradeType::Patch,
                "Fix",
                None,
                vec![],
                100,
            )
            .unwrap();
        mgr.approve_upgrade(id1, "bob", 200).unwrap();
        mgr.execute_upgrade(id1, 200 + 172800).unwrap();
        assert_eq!(mgr.current_version(), &Version::new(1, 0, 1));

        // 1.0.1 -> 1.1.0 (minor)
        let id2 = mgr
            .propose_upgrade(
                "alice",
                Version::new(1, 1, 0),
                UpgradeType::Minor,
                "Feature",
                None,
                vec![],
                400 + 172800,
            )
            .unwrap();
        mgr.approve_upgrade(id2, "bob", 500 + 172800).unwrap();
        mgr.approve_upgrade(id2, "carol", 500 + 172800).unwrap();
        mgr.execute_upgrade(id2, 500 + 172800 + 172800).unwrap();
        assert_eq!(mgr.current_version(), &Version::new(1, 1, 0));

        // 1.1.0 -> 2.0.0 (major)
        let id3 = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Major",
                None,
                vec![],
                500 + 172800 + 172800,
            )
            .unwrap();
        for signer in ["bob", "carol", "dave", "eve"] {
            mgr.approve_upgrade(id3, signer, 600 + 172800 + 172800)
                .unwrap();
        }
        mgr.execute_upgrade(id3, 600 + 2 * 172800 + 172800).unwrap();
        assert_eq!(mgr.current_version(), &Version::new(2, 0, 0));
        assert_eq!(mgr.history().len(), 3);
    }

    #[test]
    fn test_upgrade_rejected_then_new_proposal() {
        let mut mgr = setup_upgrade_manager();

        let id1 = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "Old plan",
                None,
                vec![],
                100,
            )
            .unwrap();
        mgr.cancel_upgrade(id1, "bob", 200).unwrap();

        // New proposal with better plan
        let id2 = mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "New plan v2",
                None,
                vec![],
                300,
            )
            .unwrap();

        assert_ne!(id1, id2);
        let proposal = mgr.get_proposal(id2).unwrap();
        assert_eq!(proposal.status, UpgradeStatus::Proposed);
    }
}

#[cfg(test)]
mod emergency_integration_tests {
    use super::super::emergency::*;

    fn setup_emergency_controller() -> EmergencyController {
        let config = EmergencyConfig {
            signers: vec![
                "alice".to_string(),
                "bob".to_string(),
                "carol".to_string(),
                "dave".to_string(),
                "eve".to_string(),
            ],
            standard_threshold: 2,
            critical_threshold: 3,
            default_expiry_secs: 3600,
            max_expiry_secs: 86400,
            escalation_cooldown_secs: 300,
        };
        EmergencyController::new(config)
    }

    #[test]
    fn test_graduated_emergency_response() {
        let mut ctrl = setup_emergency_controller();

        // Step 1: Low-level warning (applies immediately for Low)
        ctrl.set_emergency_level(
            "alice",
            EmergencyLevel::Low,
            "Suspicious pattern detected",
            100,
            None,
        )
        .unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Low);
        assert!(!ctrl.is_operation_allowed(&OperationType::Swap));
        assert!(ctrl.is_operation_allowed(&OperationType::LpWithdraw));

        // Step 2: Escalate after cooldown
        ctrl.set_emergency_level(
            "bob",
            EmergencyLevel::Medium,
            "Confirmed exploit attempt",
            601,
            None,
        )
        .unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Medium);

        // Step 3: Full emergency (Critical needs multisig approval)
        let id = ctrl
            .set_emergency_level(
                "carol",
                EmergencyLevel::Critical,
                "Active attack",
                1200,
                None,
            )
            .unwrap();
        // Still Medium - Critical needs 3 approvals
        assert_eq!(ctrl.current_level(), EmergencyLevel::Medium);

        // Approve to reach threshold
        ctrl.approve_emergency(id, "dave", 1200).unwrap();
        ctrl.approve_emergency(id, "eve", 1200).unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::Critical);

        // Emergency resume
        ctrl.emergency_resume("alice", "Attack mitigated", 1800)
            .unwrap();
        assert_eq!(ctrl.current_level(), EmergencyLevel::None);
        assert!(ctrl.is_operation_allowed(&OperationType::Swap));
    }

    #[test]
    fn test_emergency_with_auto_expiry() {
        let mut ctrl = setup_emergency_controller();

        // Use Medium level (applies immediately with single signer)
        ctrl.set_emergency_level(
            "alice",
            EmergencyLevel::Medium,
            "Network anomaly",
            100,
            Some(600), // 10 minute auto-expiry
        )
        .unwrap();

        assert_eq!(ctrl.current_level(), EmergencyLevel::Medium);

        // Still active
        ctrl.tick(500);
        assert_eq!(ctrl.current_level(), EmergencyLevel::Medium);

        // Auto-expired
        ctrl.tick(701);
        assert_eq!(ctrl.current_level(), EmergencyLevel::None);
    }

    #[test]
    fn test_whitelist_during_emergency() {
        let mut ctrl = setup_emergency_controller();

        // Use Medium level (applies immediately)
        ctrl.set_emergency_level("alice", EmergencyLevel::Medium, "Attack", 100, None)
            .unwrap();

        // Trusted contracts can still operate
        ctrl.add_to_whitelist("trusted_dex", 200).unwrap();
        assert!(ctrl.is_whitelisted("trusted_dex"));

        // Regular operations blocked at Medium level
        assert!(!ctrl.is_operation_allowed(&OperationType::Swap));
        // LP withdrawals still allowed at Medium
        assert!(ctrl.is_operation_allowed(&OperationType::LpWithdraw));
    }

    #[test]
    fn test_emergency_audit_trail() {
        let mut ctrl = setup_emergency_controller();

        let id = ctrl
            .set_emergency_level("alice", EmergencyLevel::Low, "Warning", 100, None)
            .unwrap();
        ctrl.lift_emergency(id, "bob", 200).unwrap();

        assert!(ctrl.verify_audit_trail());
        assert_eq!(ctrl.audit_log_length(), 2);
    }
}

#[cfg(test)]
mod rewards_integration_tests {
    use super::super::rewards::*;

    fn setup_reward_manager() -> RewardManager {
        let config = RewardConfig {
            epoch_reward_pool: 2_000_000,
            epoch_duration_secs: 604800,
            claim_cooldown_secs: 60,
            max_reward_per_participant: u128::MAX,
            proposal_creation_bonus: 10,
            vote_cast_reward: 5,
            voting_power_multiplier_bps: 100,
            min_qualification_score: 10,
            diminishing_returns_bps: 100,
        };
        RewardManager::new(config, 0)
    }

    #[test]
    fn test_active_governance_participant_earns_rewards() {
        // Compare rewards for two participants using calculate_reward
        // (not claim_rewards, which is pool-limited)
        let config = RewardConfig {
            epoch_reward_pool: 10_000,
            epoch_duration_secs: 604800,
            claim_cooldown_secs: 60,
            max_reward_per_participant: u128::MAX,
            proposal_creation_bonus: 10,
            vote_cast_reward: 5,
            voting_power_multiplier_bps: 100,
            min_qualification_score: 10,
            diminishing_returns_bps: 100,
        };
        let mut mgr = RewardManager::new(config, 0);

        // Alice is very active: 10 votes, 3 proposals
        for i in 0..10 {
            mgr.record_vote_cast("alice", 200, 100 + i * 10).unwrap();
        }
        for i in 0..3 {
            mgr.record_proposal_created("alice", 100 + i * 10).unwrap();
        }

        // Bob is moderately active: 5 votes, 1 proposal
        for i in 0..5 {
            mgr.record_vote_cast("bob", 150, 200 + i * 10).unwrap();
        }
        mgr.record_proposal_created("bob", 200).unwrap();

        // Charlie is barely active: 1 vote
        mgr.record_vote_cast("charlie", 50, 300).unwrap();

        mgr.finalize_epoch(1, 700000).unwrap();

        // Use calculate_reward to compare without pool constraints
        let alice_reward = mgr.calculate_reward("alice", 1);
        let bob_reward = mgr.calculate_reward("bob", 1);
        let charlie_reward = mgr.calculate_reward("charlie", 1);

        // Alice (score=90) should get more than Bob (score=75)
        assert!(alice_reward > bob_reward);
        // Bob should get more than Charlie (score=10)
        assert!(bob_reward > charlie_reward);
        assert!(alice_reward > 0);
    }

    #[test]
    fn test_rewards_across_epochs() {
        let mut mgr = setup_reward_manager();

        // Epoch 1
        mgr.record_vote_cast("alice", 500, 100).unwrap();
        mgr.record_proposal_created("alice", 100).unwrap();
        mgr.finalize_epoch(1, 700000).unwrap();
        let reward1 = mgr.claim_rewards("alice", 1, 700001).unwrap();
        assert!(reward1 > 0);

        // Epoch 2
        mgr.record_vote_cast("alice", 500, 700100).unwrap();
        mgr.record_proposal_created("alice", 700100).unwrap();
        mgr.finalize_epoch(2, 1400000).unwrap();
        let reward2 = mgr.claim_rewards("alice", 2, 1400001).unwrap();
        assert!(reward2 > 0);

        let stats = mgr.all_time_stats("alice").unwrap();
        assert_eq!(stats.epochs_participated, 2);
        assert!(stats.total_rewards_claimed > 0);
    }

    #[test]
    fn test_quorum_participation_bonus() {
        let mut mgr = setup_reward_manager();

        // User with full participation (votes + proposals)
        mgr.record_vote_cast("full_user", 1000, 100).unwrap();
        mgr.record_proposal_created("full_user", 100).unwrap();
        mgr.record_delegation_received("full_user", 100).unwrap();

        // User with partial participation (votes only)
        mgr.record_vote_cast("partial_user", 1000, 200).unwrap();

        let full_record = mgr.participation_record("full_user", 1).unwrap();
        let partial_record = mgr.participation_record("partial_user", 1).unwrap();

        assert!(full_record.score() > partial_record.score());
    }

    #[test]
    fn test_unqualified_participant() {
        let mut mgr = setup_reward_manager();

        // Minimal participation
        mgr.record_vote_cast("minimal", 10, 100).unwrap();
        mgr.finalize_epoch(1, 700000).unwrap();

        let reward = mgr.calculate_reward("minimal", 1);
        // With only 1 vote and 10 power, score is 10 (min qualification)
        // Reward should be positive but much less than the pool
        assert!(reward > 0);
        assert!(reward < 2_000_000); // Well under pool
    }
}

#[cfg(test)]
mod end_to_end_governance_tests {
    use super::super::emergency::*;
    use super::super::rewards::*;
    use super::super::treasury::*;
    use super::super::upgrade::*;

    /// Simulates a complete governance lifecycle:
    /// 1. Community proposes a protocol upgrade
    /// 2. Treasury allocates funds for the upgrade
    /// 3. Emergency controls protect during migration
    /// 4. Active voters earn rewards
    #[test]
    fn test_complete_governance_lifecycle() {
        let signers = vec![
            "alice".to_string(),
            "bob".to_string(),
            "carol".to_string(),
            "dave".to_string(),
            "eve".to_string(),
        ];

        // Initialize all systems
        let mut treasury = Treasury::new(signers.clone(), 3, 2);
        let mut upgrade_mgr = UpgradeManager::new(
            Version::new(1, 0, 0),
            signers.clone(),
            UpgradeConfig::default(),
        );
        let mut emergency = EmergencyController::new(EmergencyConfig {
            signers: signers.clone(),
            standard_threshold: 2,
            critical_threshold: 3,
            ..EmergencyConfig::default()
        });
        let mut rewards = RewardManager::new(RewardConfig::default(), 100);

        // Setup: Fund treasury
        treasury
            .deposit("XLM", 10_000_000, "foundation", 100)
            .unwrap();

        // Phase 1: Governance discussion and voting
        rewards.record_proposal_created("alice", 100).unwrap();
        rewards.record_vote_cast("alice", 1000, 200).unwrap();
        rewards.record_vote_cast("bob", 500, 200).unwrap();
        rewards.record_vote_cast("carol", 300, 200).unwrap();
        rewards.record_delegation_received("alice", 200).unwrap();

        // Phase 2: Treasury allocation for upgrade
        let treasury_id = treasury
            .create_proposal(
                "alice",
                ProposalType::BudgetAllocation,
                "XLM",
                2_000_000,
                "upgrade_fund",
                "Budget for v2.0.0 upgrade development",
                300,
                86400 * 7,
            )
            .unwrap();
        treasury.approve_proposal(&treasury_id, "bob", 400).unwrap();
        treasury
            .approve_proposal(&treasury_id, "carol", 400)
            .unwrap();
        treasury.execute_proposal(&treasury_id, 500).unwrap();
        assert_eq!(treasury.balance_of("XLM"), 8_000_000);

        // Phase 3: Emergency protection during upgrade window
        let em_id = emergency
            .set_emergency_level(
                "alice",
                EmergencyLevel::Medium,
                "Pre-upgrade safety",
                600,
                Some(3600),
            )
            .unwrap();
        assert!(!emergency.is_operation_allowed(&OperationType::Swap));

        // Phase 4: Execute protocol upgrade
        let upgrade_id = upgrade_mgr
            .propose_upgrade(
                "alice",
                Version::new(2, 0, 0),
                UpgradeType::Major,
                "New AMM engine with governance",
                None,
                vec![
                    "Deploy v2 contract".to_string(),
                    "Migrate state".to_string(),
                ],
                600,
            )
            .unwrap();
        upgrade_mgr.approve_upgrade(upgrade_id, "bob", 700).unwrap();
        upgrade_mgr
            .approve_upgrade(upgrade_id, "carol", 700)
            .unwrap();
        upgrade_mgr
            .approve_upgrade(upgrade_id, "dave", 700)
            .unwrap();
        upgrade_mgr.approve_upgrade(upgrade_id, "eve", 700).unwrap();
        upgrade_mgr
            .execute_upgrade(upgrade_id, 700 + 172800)
            .unwrap();

        // Phase 5: Lift emergency after successful upgrade
        emergency
            .lift_emergency(em_id, "alice", 700 + 172801)
            .unwrap();
        assert!(emergency.is_operation_allowed(&OperationType::Swap));

        // Phase 6: Distribute governance rewards
        rewards.finalize_epoch(1, 700 + 172802).unwrap();

        // Verify final state
        assert_eq!(upgrade_mgr.current_version(), &Version::new(2, 0, 0));
        assert_eq!(treasury.balance_of("XLM"), 8_000_000);
        assert_eq!(emergency.current_level(), EmergencyLevel::None);
        assert!(treasury.verify_audit_trail());
        assert!(emergency.verify_audit_trail());
        assert_eq!(upgrade_mgr.history().len(), 1);

        // Rewards can be claimed
        let alice_reward = rewards.claim_rewards("alice", 1, 700 + 172803);
        assert!(alice_reward.is_ok());
        assert!(alice_reward.unwrap() > 0);
    }

    /// Test that all modules maintain independent audit trails.
    #[test]
    fn test_independent_audit_trails() {
        let signers: Vec<String> = (0..5).map(|i| format!("signer_{}", i)).collect();

        let mut treasury = Treasury::new(signers.clone(), 3, 2);
        let mut emergency = EmergencyController::new(EmergencyConfig {
            signers: signers.clone(),
            ..EmergencyConfig::default()
        });

        treasury.deposit("XLM", 1_000_000, "funder", 100).unwrap();
        emergency
            .set_emergency_level("signer_0", EmergencyLevel::Low, "test", 200, None)
            .unwrap();

        assert!(treasury.verify_audit_trail());
        assert!(emergency.verify_audit_trail());
        assert_eq!(treasury.audit_log_length(), 1);
        assert_eq!(emergency.audit_log_length(), 1);
    }
}
