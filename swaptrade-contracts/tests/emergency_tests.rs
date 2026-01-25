#[test]
fn test_pause_blocks_swap() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    emergency::set_admin(&env, admin.clone());
    emergency::pause(&env, admin.clone());

    let mut portfolio = Portfolio::new(&env);
    let result = std::panic::catch_unwind(|| {
        perform_swap(&env, &mut portfolio, symbol_short!("XLM"), symbol_short!("USDCSIM"), 100, user.clone());
    });
    assert!(result.is_err());
}

#[test]
fn test_pause_blocks_swap() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    emergency::set_admin(&env, admin.clone());
    emergency::pause(&env, admin.clone());

    let mut portfolio = Portfolio::new(&env);
    let result = std::panic::catch_unwind(|| {
        perform_swap(&env, &mut portfolio, symbol_short!("XLM"), symbol_short!("USDCSIM"), 100, user.clone());
    });
    assert!(result.is_err());
}

#[test]
fn test_unpause_restores_swap() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    emergency::set_admin(&env, admin.clone());
    emergency::pause(&env, admin.clone());
    emergency::unpause(&env, admin.clone());

    let mut portfolio = Portfolio::new(&env);
    let result = perform_swap(&env, &mut portfolio, symbol_short!("XLM"), symbol_short!("USDCSIM"), 100, user.clone());
    assert!(result >= 0);
}

#[test]
fn test_frozen_user_blocked() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    emergency::set_admin(&env, admin.clone());
    emergency::freeze_user(&env, admin.clone(), user.clone());

    let mut portfolio = Portfolio::new(&env);
    let result = std::panic::catch_unwind(|| {
        perform_swap(&env, &mut portfolio, symbol_short!("XLM"), symbol_short!("USDCSIM"), 100, user.clone());
    });
    assert!(result.is_err());
}

#[test]
fn test_snapshot_returns_state() {
    let env = Env::default();
    let admin = Address::generate(&env);

    emergency::set_admin(&env, admin.clone());

    let mut portfolio = Portfolio::new(&env);
    let snap = emergency::snapshot(&env, &portfolio);

    assert_eq!(snap.paused, false);
}


#[test]
fn test_unpause_restores_swap() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    emergency::set_admin(&env, admin.clone());
    emergency::pause(&env, admin.clone());
    emergency::unpause(&env, admin.clone());

    let mut portfolio = Portfolio::new(&env);
    let result = perform_swap(&env, &mut portfolio, symbol_short!("XLM"), symbol_short!("USDCSIM"), 100, user.clone());
    assert!(result >= 0);
}

#[test]
fn test_frozen_user_blocked() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    emergency::set_admin(&env, admin.clone());
    emergency::freeze_user(&env, admin.clone(), user.clone());

    let mut portfolio = Portfolio::new(&env);
    let result = std::panic::catch_unwind(|| {
        perform_swap(&env, &mut portfolio, symbol_short!("XLM"), symbol_short!("USDCSIM"), 100, user.clone());
    });
    assert!(result.is_err());
}

#[test]
fn test_snapshot_returns_state() {
    let env = Env::default();
    let admin = Address::generate(&env);

    emergency::set_admin(&env, admin.clone());

    let mut portfolio = Portfolio::new(&env);
    let snap = emergency::snapshot(&env, &portfolio);

    assert_eq!(snap.paused, false);
}
