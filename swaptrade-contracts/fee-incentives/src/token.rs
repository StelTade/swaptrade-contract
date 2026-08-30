use soroban_sdk::{token, Address, Env};

/// Transfer tokens between two addresses using the Soroban token interface.
pub fn transfer_token(
    env: &Env,
    asset: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<(), super::errors::FeeError> {
    if amount <= 0 {
        return Ok(());
    }
    let client = token::Client::new(env, asset);
    client.transfer(from, to, &amount);
    Ok(())
}
