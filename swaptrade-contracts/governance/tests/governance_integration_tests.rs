use soroban_sdk::{Address, Env, Map, Symbol, Vec};

use governance::{
    GovernanceContract, GovernanceError, ProposalAction, approve, cancel, execute, get_proposal,
    is_paused, is_signer, pause, propose, schedule_upgrade, unpause,
};

fn setup() -> (Env, Address, Vec<Address>) {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let contract_id = env.register_contract(None, GovernanceContract);
    let admin = Address::generate(&env);

    let mut signers: Vec<Address> = Vec::new(&env);
    for _ in 0..5 {
        signers.push_back(Address::generate(&env));
    }

    let impl_addr = Address::generate(&env);

    env.as_contract(&contract_id, || {
        GovernanceContract::initialize(
            env.clone(),
            admin.clone(),
            signers.clone(),
            3,
            100,
            impl_addr.clone(),
        )
        .unwrap();
    });

    (env, contract_id, signers)
}

// ── Multisig propose/approve/execute ─────────────────────────────────────────

#[test]
fn test_propose_and_approve() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let proposal_id = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();

        let proposal = get_proposal(&env, proposal_id).unwrap();
        assert_eq!(proposal.proposer, proposer);
        assert_eq!(proposal.signatures.len(), 0);
        assert!(!proposal.executed);
        assert!(!proposal.canceled);
        assert_eq!(proposal.threshold, 3);
    });
}

#[test]
fn test_approve_and_execute_pause() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let proposal_id = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();

        for i in 0..3 {
            let signer = signers.get(i).unwrap();
            approve(&env, signer, proposal_id).unwrap();
        }

        let proposal = get_proposal(&env, proposal_id).unwrap();
        assert_eq!(proposal.signatures.len(), 3);

        env.ledger().with_mut(|l| {
            l.timestamp = proposal.execute_after + 1;
        });

        execute(&env, signers.get(1).unwrap(), proposal_id).unwrap();

        assert!(is_paused(&env));
    });
}

#[test]
fn test_execute_unpause() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let pause_id = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();

        for i in 0..3 {
            approve(&env, signers.get(i).unwrap(), pause_id).unwrap();
        }

        let proposal = get_proposal(&env, pause_id).unwrap();
        env.ledger().with_mut(|l| {
            l.timestamp = proposal.execute_after + 1;
        });
        execute(&env, signers.get(1).unwrap(), pause_id).unwrap();

        assert!(is_paused(&env));

        let unpause_id = propose(&env, proposer.clone(), ProposalAction::Unpause).unwrap();
        for i in 0..3 {
            approve(&env, signers.get(i).unwrap(), unpause_id).unwrap();
        }

        let unpause_proposal = get_proposal(&env, unpause_id).unwrap();
        env.ledger().with_mut(|l| {
            l.timestamp = unpause_proposal.execute_after + 1;
        });
        execute(&env, signers.get(1).unwrap(), unpause_id).unwrap();

        assert!(!is_paused(&env));
    });
}

// ── Replay prevention ────────────────────────────────────────────────────────

#[test]
fn test_replay_prevention() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let id1 = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();

        for i in 0..3 {
            approve(&env, signers.get(i).unwrap(), id1).unwrap();
        }

        let proposal = get_proposal(&env, id1).unwrap();
        env.ledger().with_mut(|l| {
            l.timestamp = proposal.execute_after + 1;
        });
        execute(&env, signers.get(1).unwrap(), id1).unwrap();

        let result = execute(&env, signers.get(2).unwrap(), id1);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            GovernanceError::ProposalAlreadyExecuted
        );
    });
}

#[test]
fn test_unique_nonce_per_proposal() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let id1 = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();
        let id2 = propose(&env, proposer.clone(), ProposalAction::Unpause).unwrap();

        assert_ne!(id1, id2);

        let p1 = get_proposal(&env, id1).unwrap();
        let p2 = get_proposal(&env, id2).unwrap();
        assert_ne!(p1.nonce, p2.nonce);
    });
}

// ── Timelock enforcement ─────────────────────────────────────────────────────

#[test]
fn test_timelock_blocks_execution() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let proposal_id = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();

        for i in 0..3 {
            approve(&env, signers.get(i).unwrap(), proposal_id).unwrap();
        }

        let proposal = get_proposal(&env, proposal_id).unwrap();
        env.ledger().with_mut(|l| {
            l.timestamp = proposal.execute_after - 1;
        });

        let result = execute(&env, signers.get(1).unwrap(), proposal_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), GovernanceError::TimelockNotElapsed);
    });
}

// ── Governance upgrade with state preservation ───────────────────────────────

#[test]
fn test_governance_upgrade_preserves_state() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let new_impl = Address::generate(&env);

        let proposal_id =
            propose(&env, proposer.clone(), ProposalAction::Upgrade(new_impl.clone())).unwrap();

        for i in 0..3 {
            approve(&env, signers.get(i).unwrap(), proposal_id).unwrap();
        }

        let proposal = get_proposal(&env, proposal_id).unwrap();
        env.ledger().with_mut(|l| {
            l.timestamp = proposal.execute_after + 1;
        });
        execute(&env, signers.get(1).unwrap(), proposal_id).unwrap();

        assert_eq!(get_proposal(&env, proposal_id).unwrap().executed, true);
    });
}

#[test]
fn test_upgrade_requires_governance() {
    let (env, contract_id, _signers) = setup();
    let outsider = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let result = schedule_upgrade(&env, outsider.clone());
        assert!(result.is_err());
    });
}

// ── Governance-controlled pause/unpause integration ──────────────────────────

#[test]
fn test_pause_blocks_operations() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let proposal_id = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();

        for i in 0..3 {
            approve(&env, signers.get(i).unwrap(), proposal_id).unwrap();
        }

        let proposal = get_proposal(&env, proposal_id).unwrap();
        env.ledger().with_mut(|l| {
            l.timestamp = proposal.execute_after + 1;
        });
        execute(&env, signers.get(1).unwrap(), proposal_id).unwrap();

        assert!(is_paused(&env));
    });
}

// ── Signer and threshold management ──────────────────────────────────────────

#[test]
fn test_add_and_remove_signer() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let new_signer = Address::generate(&env);

        let add_id =
            propose(&env, proposer.clone(), ProposalAction::AddSigner(new_signer.clone())).unwrap();
        for i in 0..3 {
            approve(&env, signers.get(i).unwrap(), add_id).unwrap();
        }
        let proposal = get_proposal(&env, add_id).unwrap();
        env.ledger().with_mut(|l| {
            l.timestamp = proposal.execute_after + 1;
        });
        execute(&env, signers.get(1).unwrap(), add_id).unwrap();

        assert!(is_signer(&env, &new_signer));

        let rm_id =
            propose(&env, proposer.clone(), ProposalAction::RemoveSigner(new_signer)).unwrap();
        for i in 0..3 {
            approve(&env, signers.get(i).unwrap(), rm_id).unwrap();
        }
        let rm_proposal = get_proposal(&env, rm_id).unwrap();
        env.ledger().with_mut(|l| {
            l.timestamp = rm_proposal.execute_after + 1;
        });
        execute(&env, signers.get(1).unwrap(), rm_id).unwrap();
    });
}

#[test]
fn test_update_threshold() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let proposal_id =
            propose(&env, proposer.clone(), ProposalAction::SetThreshold(5)).unwrap();

        for i in 0..3 {
            approve(&env, signers.get(i).unwrap(), proposal_id).unwrap();
        }

        let proposal = get_proposal(&env, proposal_id).unwrap();
        env.ledger().with_mut(|l| {
            l.timestamp = proposal.execute_after + 1;
        });
        execute(&env, signers.get(1).unwrap(), proposal_id).unwrap();
    });
}

// ── Cancellation ─────────────────────────────────────────────────────────────

#[test]
fn test_cancel_proposal() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let proposal_id = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();

        cancel(&env, proposer.clone(), proposal_id).unwrap();

        let proposal = get_proposal(&env, proposal_id).unwrap();
        assert!(proposal.canceled);
        assert!(!proposal.executed);
    });
}

// ── Signature tracking ───────────────────────────────────────────────────────

#[test]
fn test_signature_tracking() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let proposal_id = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();

        approve(&env, signers.get(0).unwrap(), proposal_id).unwrap();
        let proposal = get_proposal(&env, proposal_id).unwrap();
        assert_eq!(proposal.signatures.len(), 1);

        approve(&env, signers.get(1).unwrap(), proposal_id).unwrap();
        let proposal = get_proposal(&env, proposal_id).unwrap();
        assert_eq!(proposal.signatures.len(), 2);
    });
}

// ── Non-signer rejection ─────────────────────────────────────────────────────

#[test]
fn test_non_signer_cannot_propose() {
    let (env, contract_id, _signers) = setup();
    let outsider = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let result =
            propose(&env, outsider.clone(), ProposalAction::Pause);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), GovernanceError::NotSigner);
    });
}

// ── Duplicate signature rejection ────────────────────────────────────────────

#[test]
fn test_duplicate_signature_rejected() {
    let (env, contract_id, signers) = setup();

    env.as_contract(&contract_id, || {
        let proposer = signers.get(0).unwrap();
        let proposal_id = propose(&env, proposer.clone(), ProposalAction::Pause).unwrap();

        approve(&env, signers.get(0).unwrap(), proposal_id).unwrap();
        let result = approve(&env, signers.get(0).unwrap(), proposal_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), GovernanceError::AlreadySigned);
    });
}
