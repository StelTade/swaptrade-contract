# Testing Guide for SwapTrade Contracts

This document describes how to run tests, generate coverage reports, and deploy contracts locally.

## Prerequisites

- Rust toolchain (stable)
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- Soroban CLI: `cargo install --locked --git https://github.com/stellar/soroban-tools soroban-cli`
- Docker (for localnet): `stellar/quickstart:soroban-dev`
- `cargo-tarpaulin` (for coverage): `cargo install cargo-tarpaulin`

## Running Tests

### Unit Tests

Run all library unit tests:

```bash
cargo test --workspace --lib --verbose
```

### Formal Verification Tests

Run the formal verification integration suite:

```bash
cargo test --manifest-path swaptrade-contracts/counter/Cargo.toml \
    --test formal_verification_tests formal_verification \
    -- --nocapture --test-threads=1
```

### Exhaustive Property Tests (10,000+ sequences)

```bash
cargo test --manifest-path swaptrade-contracts/counter/Cargo.toml \
    --test formal_verification_tests exhaustive_ \
    -- --nocapture --test-threads=1
```

### Fuzz Tests (proptest)

Run property-based fuzz tests using proptest:

```bash
cargo test --manifest-path swaptrade-contracts/counter/Cargo.toml \
    --lib proptest_ \
    -- --nocapture
```

### Inline Fuzz Tests

```bash
cargo test --manifest-path swaptrade-contracts/counter/Cargo.toml \
    --lib fuzz_ \
    -- --nocapture
```

### KYC Tests

```bash
cargo test --manifest-path swaptrade-contracts/counter/Cargo.toml \
    --lib kyc_tests \
    -- --nocapture
```

### Error Catalog Validation

```bash
cargo test --manifest-path swaptrade-contracts/counter/Cargo.toml \
    error_code_tests \
    -- --nocapture
```

### Complete Formal Verification Script

```bash
./scripts/verify_formal.sh           # Full suite
./scripts/verify_formal.sh --quick   # Skip exhaustive tests
./scripts/verify_formal.sh --coverage # Include coverage report
```

## Coverage Reports

### Generate HTML Coverage Report

```bash
cargo tarpaulin --workspace --lib \
    --out html \
    --output-dir coverage \
    --timeout 600 \
    --exclude-files "*/tests/*" "*/benches/*"
```

Open `coverage/tarpaulin-report.html` in a browser.

### Generate XML Coverage Report (for CI)

```bash
cargo tarpaulin --workspace --lib \
    --out xml \
    --output-dir coverage \
    --timeout 600
```

## Localnet Deployment

### Start Localnet

```bash
docker run --rm -it -p 8000:8000 stellar/quickstart:soroban-dev
```

### Deploy Contracts

```bash
./scripts/deploy_localnet.sh          # Deploy all contracts
./scripts/deploy_localnet.sh --clean  # Clean build first
```

### Interact with Deployed Contracts

```bash
# List deployed contracts
soroban contract ls --network standalone

# Invoke a function
soroban contract invoke \
    --id <CONTRACT_ID> \
    --network standalone \
    --source deployer \
    -- <function_name> <args>

# Read contract state
soroban contract read \
    --id <CONTRACT_ID> \
    --network standalone
```

## CI Pipeline

The CI pipeline runs automatically on PRs and pushes to `main`:

| Job | Description |
|-----|-------------|
| **Quality** | `cargo fmt --check`, `cargo clippy`, `cargo check`, KYC guard verification |
| **Build** | `cargo build` + `cargo build --release` |
| **Test** | Unit tests, KYC tests, error catalog, formal verification |
| **Fuzz** | proptest property-based tests, exhaustive invariant checks |
| **Coverage** | `cargo-tarpaulin` HTML + XML coverage reports |
| **Gas Report** | `cargo bench` benchmark results |
| **Localnet** | Full localnet deployment + integration tests |

All test artifacts (coverage reports, gas reports) are uploaded as GitHub Actions artifacts.

## Adding New Tests

### Unit Test Pattern

```rust
#[cfg(test)]
mod my_tests {
    use super::*;

    #[test]
    fn test_basic_operation() {
        let env = Env::default();
        // ... test code
    }
}
```

### proptest Pattern

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_with_random_inputs(amount in 1_i128..1_000_000_000_i128) {
        let env = Env::default();
        // ... test code with random `amount`
        prop_assert!(condition);
    }
}
```

### Integration Test Pattern

```rust
use soroban_sdk::{Env, testutils::Address};

#[test]
fn test_scenario() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    // ... deploy and interact with contract
}
```

## Invariant Properties Verified

| Invariant | Description |
|-----------|-------------|
| Non-negative balances | User balances never go below zero |
| Pool liquidity non-negative | Pool reserves never negative |
| LP token conservation | LP tokens properly tracked |
| Metrics non-negative | Trade counters only increase |
| Fee accumulation non-negative | Collected fees never negative |
| User counts consistent | Active users <= total users |
| AMM constant product | k never increases after swaps |
| Fee bounds | Fees always within [0%, 1%] |
| Version monotonicity | Contract version never decreases |
| Timestamp monotonicity | Timestamps always increase |
