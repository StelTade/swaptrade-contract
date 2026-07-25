
use soroban_sdk::{contracttype, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vote {
    pub voter: soroban_sdk::Address,
    pub vote_weight: i64,
}

pub fn calculate_voting_power(balance: u64) -> u64 {
    (balance as f64).sqrt() as u64
}

pub fn tally_votes(votes: &Vec<Vote>) -> (u64, u64) {
    let mut total_for = 0;
    let mut total_against = 0;

    for vote in votes.iter() {
        if vote.vote_weight > 0 {
            total_for += vote.vote_weight as u64;
        } else {
            total_against += vote.vote_weight.abs() as u64;
        }
    }

    (total_for, total_against)
}