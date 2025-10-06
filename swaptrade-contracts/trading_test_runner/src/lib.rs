#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    fn swap(
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

    #[test]
    fn happy_path_swap_updates_balances() {
        let mut balances = HashMap::new();
        balances.insert("XLM".to_string(), 1000u64);
        balances.insert("USDC".to_string(), 0u64);

        let mut rates = HashMap::new();
        rates.insert(("XLM".to_string(), "USDC".to_string()), 1.0f64);

        let res = swap(&mut balances, "XLM", "USDC", 500, &rates);
        assert!(res.is_ok());
        assert_eq!(balances.get("XLM"), Some(&500u64));
        assert_eq!(balances.get("USDC"), Some(&500u64));
    }

    #[test]
    fn insufficient_balance_returns_error() {
        let mut balances = HashMap::new();
        balances.insert("XLM".to_string(), 100u64);
        balances.insert("USDC".to_string(), 0u64);

        let mut rates = HashMap::new();
        rates.insert(("XLM".to_string(), "USDC".to_string()), 1.0f64);

        let err = swap(&mut balances, "XLM", "USDC", 200, &rates).unwrap_err();
        assert_eq!(err, "Insufficient balance");
    }

    #[test]
    fn unsupported_token_returns_error() {
        let mut balances = HashMap::new();
        balances.insert("XLM".to_string(), 100u64);
        balances.insert("USDC".to_string(), 0u64);

        let rates = HashMap::new();

        let err = swap(&mut balances, "XLM", "BTC", 10, &rates).unwrap_err();
        assert_eq!(err, "Unsupported token");
    }

    #[test]
    fn rounding_truncation_behavior() {
        let mut balances = HashMap::new();
        balances.insert("XLM".to_string(), 3u64);
        balances.insert("USDC".to_string(), 0u64);

        let mut rates = HashMap::new();
        rates.insert(("XLM".to_string(), "USDC".to_string()), 0.333333_f64);

        let ok = swap(&mut balances, "XLM", "USDC", 1, &rates);
        assert!(ok.is_ok());
        assert_eq!(balances.get("XLM"), Some(&2u64));
        assert_eq!(balances.get("USDC"), Some(&0u64));

        swap(&mut balances, "XLM", "USDC", 2, &rates).unwrap();
        assert_eq!(balances.get("XLM"), Some(&0u64));
        assert_eq!(balances.get("USDC"), Some(&0u64));
    }

    #[test]
    fn simulated_concurrent_order_isolation() {
        let mut alice = HashMap::new();
        alice.insert("XLM".to_string(), 500u64);
        alice.insert("USDC".to_string(), 0u64);

        let mut bob = HashMap::new();
        bob.insert("XLM".to_string(), 300u64);
        bob.insert("USDC".to_string(), 0u64);

        let mut rates = HashMap::new();
        rates.insert(("XLM".to_string(), "USDC".to_string()), 1.0f64);

        swap(&mut alice, "XLM", "USDC", 200, &rates).unwrap();
        swap(&mut bob, "XLM", "USDC", 300, &rates).unwrap();

        assert_eq!(alice.get("XLM"), Some(&300u64));
        assert_eq!(alice.get("USDC"), Some(&200u64));

        assert_eq!(bob.get("XLM"), Some(&0u64));
        assert_eq!(bob.get("USDC"), Some(&300u64));
    }

    #[test]
    fn amm_round_trip_identity() {
        let mut balances = HashMap::new();
        balances.insert("XLM".to_string(), 1000u64);
        balances.insert("USDC".to_string(), 0u64);

        let mut rates = HashMap::new();
        // 1:1 for the simple AMM
        rates.insert(("XLM".to_string(), "USDC".to_string()), 1.0f64);
        rates.insert(("USDC".to_string(), "XLM".to_string()), 1.0f64);

        // XLM -> USDC
        swap(&mut balances, "XLM", "USDC", 250, &rates).unwrap();
        // USDC -> XLM
        swap(&mut balances, "USDC", "XLM", 250, &rates).unwrap();

        assert_eq!(balances.get("XLM"), Some(&1000u64));
        assert_eq!(balances.get("USDC"), Some(&0u64));
    }
}
