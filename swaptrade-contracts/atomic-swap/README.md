# Soroban Atomic Swap Contract

A trusted-escrow atomic swap contract for Stellar/Soroban that enables peer-to-peer asset exchanges with trustline verification, expiry enforcement, and full lifecycle management.

## Overview

Two parties agree on an asset swap. The contract holds both sides in escrow and executes the atomic transfer when both parties have funded and one accepts. If the swap expires before acceptance, either party can trigger a refund.

```
Creator ──create──► Created ──fund×2──► Funded ──accept──► Accepted
                                  │                          │
                                  ├──cancel──► Cancelled     └── assets moved
                                  └──refund (after expiry)──► Refunded
```

## Contract Methods

### `create_swap`

Create a new swap offer.

```rust
fn create_swap(
    env: Env,
    creator: Address,        // Initiating party (requires auth)
    counterparty: Address,   // Other party
    asset_a: Address,        // Stellar asset contract for side A
    amount_a: i128,          // Amount of asset A (must be > 0)
    asset_b: Address,        // Stellar asset contract for side B
    amount_b: i128,          // Amount of asset B (must be > 0)
    expiry: u64,             // Ledger timestamp after which refund is allowed
    nonce: u64,              // Client-supplied nonce for idempotency
) -> Result<u64, SwapError>
```

**Validation:**
- `amount_a > 0 && amount_b > 0`
- `asset_a != asset_b`
- `creator != counterparty`
- `expiry > now + min_expiry` (default 300s)
- Creator must hold a trustline for `asset_a`

**Idempotency:** Same `(creator, nonce)` pair returns the existing swap ID without creating a duplicate.

---

### `fund_swap`

Deposit assets into escrow.

```rust
fn fund_swap(
    env: Env,
    swap_id: u64,
    funder: Address,         // Creator or counterparty (requires auth)
) -> Result<(), SwapError>
```

**Validation:**
- `funder` must be the creator or counterparty
- Swap must be in `Created` state
- Funder must not have already funded their side
- Funder must hold a trustline for the relevant asset
- Token transfer must succeed (balance sufficient)

**Note:** State transitions to `Funded` only when **both** parties have deposited.

---

### `accept_swap`

Accept a fully-funded swap — executes the atomic transfer.

```rust
fn accept_swap(
    env: Env,
    swap_id: u64,
    acceptor: Address,       // Must be the counterparty (requires auth)
) -> Result<(), SwapError>
```

**Validation:**
- Only the counterparty can accept
- Both sides must be funded
- Swap must not have expired
- Counterparty must trust `asset_a` (they'll receive it)
- Creator must trust `asset_b` (they'll receive it)

**Effect:** Asset A transfers contract → counterparty, Asset B transfers contract → creator.

---

### `cancel_swap`

Cancel an unfunded swap.

```rust
fn cancel_swap(
    env: Env,
    swap_id: u64,
) -> Result<(), SwapError>
```

**Validation:**
- Only the creator can cancel
- Swap must be in `Created` state
- Counterparty must not have funded (they have economic interest)
- Creator must not have funded (use `refund_swap` instead)

---

### `refund_swap`

Refund a funded-but-unaccepted swap after expiry.

```rust
fn refund_swap(
    env: Env,
    swap_id: u64,
) -> Result<(), SwapError>
```

**Validation:**
- Only the creator can trigger
- Swap must not be accepted, cancelled, or already refunded
- At least one party must have funded
- Must be past the expiry timestamp

**Effect:** Each funded party receives their original deposit back.

---

### Read-Only Queries

```rust
fn get_swap(env: Env, swap_id: u64) -> Result<Swap, SwapError>
fn check_trustline(env: Env, address: Address, asset: Address) -> bool
fn get_min_expiry(env: Env) -> u64
fn set_min_expiry(env: Env, caller: Address, seconds: u64)  // admin only
```

## Error Codes

| Code | Error | Description |
|------|-------|-------------|
| 1 | `SwapNotFound` | Swap ID not in storage |
| 2 | `Unauthorized` | Caller is not an authorized party |
| 3 | `MissingTrustline` | Required trustline not found |
| 4 | `InvalidState` | Wrong lifecycle state for this operation |
| 5 | `InvalidAmount` | Amount must be strictly positive |
| 6 | `InvalidExpiry` | Expiry must be > now + min_expiry |
| 7 | `Expired` | Cannot accept an expired swap |
| 8 | `SameAsset` | Asset A and B must differ |
| 9 | `TransferMismatch` | Token transfer returned unexpected result |
| 10 | `TrustlineCheckFailed` | Trustline verification failed |

## Events

All events are published via Soroban's event system for off-chain indexing.

| Topic | Payload | Emitted When |
|-------|---------|--------------|
| `("created", swap_id)` | `(actor, timestamp)` | Swap created |
| `("funded", swap_id)` | `(actor, timestamp)` | Party funds their side |
| `("accepted", swap_id)` | `(actor, timestamp)` | Swap executed atomically |
| `("cancelled", swap_id)` | `(actor, timestamp)` | Creator cancels |
| `("refunded", swap_id)` | `(actor, timestamp)` | Swap refunded after expiry |

**Off-chain indexing:** Filter events by topic `(symbol, swap_id)` to reconstruct swap history.

## Swap Metadata

```rust
struct Swap {
    id: u64,                 // Unique identifier
    nonce: u64,              // Idempotency nonce
    creator: Address,        // Initiating party
    counterparty: Address,   // Other party
    asset_a: Address,        // Creator's asset
    amount_a: i128,          // Creator's amount
    asset_b: Address,        // Counterparty's asset
    amount_b: i128,          // Counterparty's amount
    expiry: u64,             // Expiry timestamp (seconds)
    state: SwapState,        // Created | Funded | Accepted | Cancelled | Refunded
    creator_funded: bool,    // Creator has deposited
    counterparty_funded: bool, // Counterparty has deposited
    created_at: u64,         // Creation timestamp
}
```

## Gas Estimates

Approximate compute unit costs on localnet (actual costs vary by network load):

| Operation | CU Budget | Approx. Cost (XLM) |
|-----------|-----------|---------------------|
| `create_swap` | ~200K | ~0.02 XLM |
| `fund_swap` | ~300K | ~0.03 XLM |
| `accept_swap` | ~500K | ~0.05 XLM |
| `cancel_swap` | ~150K | ~0.015 XLM |
| `refund_swap` | ~400K | ~0.04 XLM |
| `get_swap` | ~50K | ~0.005 XLM |

**Note:** Cross-contract calls (trustline checks, token transfers) dominate gas costs. The `accept_swap` operation performs 4 cross-contract calls (2 trustline checks + 2 transfers).

## Security Notes

### Trustline Verification

- **Pre-creation:** Creator must hold a trustline for `asset_a` before the swap is created.
- **Pre-fund:** The funder must hold a trustline for the asset they're depositing.
- **Pre-accept:** Both recipients are checked for trustlines before the atomic transfer executes. This prevents assets from being locked in the contract if a party lacks a trustline.

### Authorization Model

- `create_swap`: Requires auth from `creator`.
- `fund_swap`: Requires auth from the funder (creator or counterparty).
- `accept_swap`: Requires auth from the `counterparty` only.
- `cancel_swap`: Requires auth from `creator` only.
- `refund_swap`: Requires auth from `creator` only.

### Expiry Enforcement

- The minimum expiry window (default 300 seconds) prevents front-running and ensures both parties have time to fund.
- After expiry, funded swaps can be refunded — no party can accept an expired swap.
- The `set_min_expiry` admin function allows adjusting the minimum window.

### Atomicity

- `accept_swap` performs both transfers in a single transaction. If either transfer fails, the entire transaction rolls back — no partial execution.
- The contract itself holds the assets in escrow during the swap lifecycle.

### Idempotency

- `create_swap` uses a `(creator, nonce)` index to prevent duplicate swaps from the same creator with the same nonce.
- Subsequent calls with the same pair return the existing swap ID.

### Known Limitations

1. **No partial fills:** The entire `amount_a` / `amount_b` must be swapped.
2. **No multi-party swaps:** Only two-party swaps are supported.
3. **Creator-only refund:** Only the creator can trigger a refund after expiry. A future version could allow either party to refund.
4. **Storage persistence:** Swap data remains on-chain after completion for audit purposes. A future version could add optional cleanup.

## Project Structure

```
swaptrade-contracts/atomic-swap/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs          # Contract implementation (create, fund, accept, cancel, refund)
│   ├── types.rs        # Data structures (Swap, SwapState)
│   ├── errors.rs       # Error enum (SwapError)
│   ├── events.rs       # Event publishing helpers
│   └── storage.rs      # Persistent storage, trustline checks, token transfers
├── tests/
│   └── atomic_swap_tests.rs   # 20 unit tests covering happy path + edge cases
└── examples/
    └── atomic_swap_client.ts  # TypeScript client for full swap cycle
```

## Running Tests

```bash
# From workspace root
cargo test -p atomic-swap

# With verbose output
cargo test -p atomic-swap -- --nocapture
```

## Deploying to Localnet

```bash
# Build WASM
stellar contract build --path swaptrade-contracts/atomic-swap

# Deploy
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/atomic_swap.wasm \
  --network standalone \
  --source admin
```

## Client Example

See `examples/atomic_swap_client.ts` for a complete TypeScript demonstration of:

1. Creating a swap
2. Both parties funding
3. Atomic acceptance
4. Cancel flow
5. Refund flow (requires time advancement on localnet)

```bash
cd examples
SWAP_CONTRACT_ID=<deployed_id> \
ASSET_A_ID=<asset_a_address> \
ASSET_B_ID=<asset_b_address> \
npx tsx atomic_swap_client.ts
```
