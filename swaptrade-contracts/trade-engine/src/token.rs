use soroban_sdk::{token, Address, Env};

use crate::errors::TradeError;

pub fn transfer_token(
    env: &Env,
    asset: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<(), TradeError> {
    if amount <= 0 {
        return Ok(());
    }
    let client = token::Client::new(env, asset);
    client.transfer(from, to, &amount);
    Ok(())
}
