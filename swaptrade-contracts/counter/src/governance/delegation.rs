use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Map};

use crate::errors::SwapTradeError;

const DELEGATIONS_KEY: Symbol = symbol_short!("delegates");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delegation {
    pub delegator: Address,
    pub delegate: Address,
}

pub fn delegate_vote(
    env: &Env,
    delegator: Address,
    delegate: Address,
) -> Result<(), SwapTradeError> {
    delegator.require_auth();

    let mut delegations: Map<Address, Address> = env
        .storage()
        .persistent()
        .get(&DELEGATIONS_KEY)
        .unwrap_or_else(|| Map::new(env));

    delegations.set(delegator.clone(), delegate.clone());
    env.storage()
        .persistent()
        .set(&DELEGATIONS_KEY, &delegations);

    env.events().publish(
        (symbol_short!("del_vote")),
        (delegator, delegate),
    );

    Ok(())
}

pub fn revoke_delegation(env: &Env, delegator: Address) -> Result<(), SwapTradeError> {
    delegator.require_auth();

    let mut delegations: Map<Address, Address> = env
        .storage()
        .persistent()
        .get(&DELEGATIONS_KEY)
        .ok_or(SwapTradeError::NotAuthorized)?;

    delegations.remove(delegator.clone());
    env.storage()
        .persistent()
        .set(&DELEGATIONS_KEY, &delegations);

    env.events()
        .publish((symbol_short!("del_revoke")), (delegator,));

    Ok(())
}

pub fn get_delegate(env: &Env, delegator: &Address) -> Option<Address> {
    let delegations: Map<Address, Address> = env
        .storage()
        .persistent()
        .get(&DELEGATIONS_KEY)
        .unwrap_or_else(|| Map::new(env));
    delegations.get(delegator.clone())
}