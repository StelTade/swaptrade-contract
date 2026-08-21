use soroban_sdk::{Address, Env, Vec, Symbol, symbol_short};

use crate::errors::GovernanceError;
use crate::storage::{
    load_proposal_count, load_proposals, load_signers, load_threshold, store_proposal_count,
    store_proposals, store_signers, store_threshold, Proposal, ProposalAction,
};
use crate::events::{
    proposal_canceled, proposal_created, proposal_executed, proposal_signed, threshold_updated,
};

pub fn propose(
    env: &Env,
    proposer: Address,
    action: ProposalAction,
) -> Result<u64, GovernanceError> {
    proposer.require_auth();

    let signers = load_signers(env);
    if signers.is_empty() {
        return Err(GovernanceError::EmptySigners);
    }

    if !signers.contains(&proposer) {
        return Err(GovernanceError::NotSigner);
    }

    let nonce = crate::storage::load_nonce(env) + 1;
    crate::storage::store_nonce(env, nonce);

    let proposal_id = load_proposal_count(env);
    let threshold = load_threshold(env);
    let timelock_delay = crate::storage::load_timelock_delay(env);

    let proposal = Proposal {
        id: proposal_id,
        proposer: proposer.clone(),
        action: action.clone(),
        signatures: Vec::new(env),
        threshold,
        created_at: env.ledger().timestamp(),
        execute_after: env.ledger().timestamp().saturating_add(timelock_delay),
        executed: false,
        canceled: false,
        nonce,
    };

    let mut proposals = load_proposals(env);
    proposals.set(proposal_id, proposal);
    store_proposals(env, &proposals);
    store_proposal_count(env, proposal_id + 1);

    let action_tag = action_symbol(&action);
    proposal_created(env, proposal_id, proposer, action_tag);

    Ok(proposal_id)
}

pub fn approve(
    env: &Env,
    signer: Address,
    proposal_id: u64,
) -> Result<(), GovernanceError> {
    signer.require_auth();

    let mut proposals = load_proposals(env);
    let mut proposal = proposals
        .get(proposal_id)
        .ok_or(GovernanceError::ProposalNotFound)?;

    if proposal.executed {
        return Err(GovernanceError::ProposalAlreadyExecuted);
    }

    if proposal.canceled {
        return Err(GovernanceError::ProposalNotFound);
    }

    let signers = load_signers(env);
    if !signers.contains(&signer) {
        return Err(GovernanceError::NotSigner);
    }

    if proposal.signatures.iter().any(|s| s == signer) {
        return Err(GovernanceError::AlreadySigned);
    }

    proposal.signatures.push_back(signer.clone());
    proposals.set(proposal_id, proposal.clone());
    store_proposals(env, &proposals);

    proposal_signed(env, proposal_id, signer, proposal.signatures.len() as u32);

    Ok(())
}

pub fn revoke(
    env: &Env,
    signer: Address,
    proposal_id: u64,
) -> Result<(), GovernanceError> {
    signer.require_auth();

    let mut proposals = load_proposals(env);
    let mut proposal = proposals
        .get(proposal_id)
        .ok_or(GovernanceError::ProposalNotFound)?;

    if proposal.executed {
        return Err(GovernanceError::ProposalAlreadyExecuted);
    }

    if let Some(pos) = proposal
        .signatures
        .iter()
        .position(|s| s == signer)
    {
        proposal.signatures.remove(pos as u32);
        proposals.set(proposal_id, proposal.clone());
        store_proposals(env, &proposals);
    }

    Ok(())
}

pub fn execute(
    env: &Env,
    executor: Address,
    proposal_id: u64,
) -> Result<(), GovernanceError> {
    executor.require_auth();

    let mut proposals = load_proposals(env);
    let mut proposal = proposals
        .get(proposal_id)
        .ok_or(GovernanceError::ProposalNotFound)?;

    if proposal.executed {
        return Err(GovernanceError::ProposalAlreadyExecuted);
    }

    if proposal.canceled {
        return Err(GovernanceError::ProposalNotFound);
    }

    let now = env.ledger().timestamp();
    if now < proposal.execute_after {
        return Err(GovernanceError::TimelockNotElapsed);
    }

    if proposal.signatures.len() < proposal.threshold as usize {
        return Err(GovernanceError::InsufficientSignatures);
    }

    match &proposal.action {
        ProposalAction::Pause => {
            crate::storage::store_paused(env, true);
            crate::events::paused(env, executor.clone());
        }
        ProposalAction::Unpause => {
            crate::storage::store_paused(env, false);
            crate::events::unpaused(env, executor.clone());
        }
        ProposalAction::Upgrade(new_impl) => {
            let old_impl = crate::storage::load_implementation(env);
            crate::storage::store_implementation(env, new_impl);
            crate::storage::store_upgrade_scheduled(env, None);
            crate::events::upgraded(env, proposal_id, old_impl, new_impl.clone());
        }
        ProposalAction::AddSigner(signer) => {
            let mut signers = load_signers(env);
            if !signers.contains(signer) {
                signers.push_back(signer.clone());
                store_signers(env, &signers);
            }
            crate::events::signer_added(env, signer.clone());
        }
        ProposalAction::RemoveSigner(signer) => {
            let mut signers = load_signers(env);
            if let Some(pos) = signers.iter().position(|s| s == signer) {
                signers.remove(pos as u32);
                store_signers(env, &signers);
            }
            crate::events::signer_removed(env, signer.clone());
        }
        ProposalAction::SetThreshold(new_threshold) => {
            let old = load_threshold(env);
            store_threshold(env, *new_threshold);
            threshold_updated(env, old, *new_threshold);
        }
        ProposalAction::SetTimelockDelay(new_delay) => {
            crate::storage::store_timelock_delay(env, *new_delay);
        }
    };

    proposal.executed = true;
    proposals.set(proposal_id, proposal.clone());
    store_proposals(env, &proposals);

    proposal_executed(env, proposal_id, executor);

    Ok(())
}

pub fn cancel(
    env: &Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), GovernanceError> {
    caller.require_auth();

    let mut proposals = load_proposals(env);
    let mut proposal = proposals
        .get(proposal_id)
        .ok_or(GovernanceError::ProposalNotFound)?;

    if proposal.executed {
        return Err(GovernanceError::ProposalAlreadyExecuted);
    }

    if proposal.proposer != caller {
        return Err(GovernanceError::NotProposer);
    }

    proposal.canceled = true;
    proposals.set(proposal_id, proposal.clone());
    store_proposals(env, &proposals);

    proposal_canceled(env, proposal_id, caller);

    Ok(())
}

pub fn get_proposal(env: &Env, proposal_id: u64) -> Result<Proposal, GovernanceError> {
    let proposals = load_proposals(env);
    proposals
        .get(proposal_id)
        .ok_or(GovernanceError::ProposalNotFound)
}

pub fn is_signer(env: &Env, address: &Address) -> bool {
    let signers = load_signers(env);
    signers.contains(address)
}

fn action_symbol(action: &ProposalAction) -> Symbol {
    match action {
        ProposalAction::Pause => symbol_short!("pause"),
        ProposalAction::Unpause => symbol_short!("unpause"),
        ProposalAction::Upgrade(_) => symbol_short!("upgrade"),
        ProposalAction::AddSigner(_) => symbol_short!("add_sgnr"),
        ProposalAction::RemoveSigner(_) => symbol_short!("rm_sgnr"),
        ProposalAction::SetThreshold(_) => symbol_short!("set_thr"),
        ProposalAction::SetTimelockDelay(_) => symbol_short!("set_tl"),
    }
}
