use soroban_sdk::{contracttype, Address, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiSig {
    pub signers: Vec<Address>,
    pub threshold: u32,
}

pub fn create_multi_sig(
    env: &Env,
    signers: Vec<Address>,
    threshold: u32,
) -> Result<MultiSig, ()> {
    if threshold > signers.len() {
        return Err(());
    }

    Ok(MultiSig { signers, threshold })
}