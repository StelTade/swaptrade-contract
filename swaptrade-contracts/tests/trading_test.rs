pub fn swap(
    balances: &mut HashMap<String, u64>,
    from: &str,
    to: &str,
    amount: u64,
    rates: &HashMap<(String, String), f64>,
) -> Result<(), String> {
    if !balances.contains_key(from) || !balances.contains_key(to) {
        return Err("Unsupported token".into());
    }

    let from_balance = balances.get_mut(from).unwrap();
    if *from_balance < amount {
        return Err("Insufficient balance".into());
    }

    if let Some(rate) = rates.get(&(from.to_string(), to.to_string())) {
        *from_balance -= amount;
        let converted = (amount as f64 * rate) as u64;
        *balances.get_mut(to).unwrap() += converted;
        Ok(())
    } else {
        Err("Swap rate not found".into())
    }
}
