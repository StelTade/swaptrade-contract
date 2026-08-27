# SEP-10 × Soroban: On-Chain Integration Patterns

SEP-10 authenticates a *Stellar account* to a *backend*. Soroban contracts,
however, only see signed authorization entries and the arguments passed to
them. This document describes three battle-tested ways to bridge the two,
from least to most trustless, so DApps built on SwapTrade can adopt SEP-10
and qualify for Stellar Community Fund / GrantFox review.

---

## Pattern A — Backend gate only (off-chain)

The contract is unaware of SEP-10; every invocation passes through an
authenticated backend endpoint (see `src/server.ts`, `/invoke`).

```
user ──SEP-10──► backend ──submitTransaction──► Soroban RPC
```

**When it fits:** read-heavy or low-value operations (quotes, escrow
lookups, simulated trades), admin dashboards, onboarding funnels.

**Trust assumption:** the backend is honest and correctly enforces policy.

**Contract changes:** none.

---

## Pattern B — Relayer with user-signed envelopes

The backend never invents transactions. It returns an unsigned envelope
bound to the session subject (`createInvocationForSigning`); the user signs
it with their wallet key and posts it back. Before paying fees, the
backend runs `verifyUserAuthorization`:

1. envelope source account == SEP-10 session subject;
2. that account's signature verifies over the exact operation hash.

Only then does it submit to Soroban RPC (fee-bumped by the relayer if
desired). The contract still needs no awareness of SEP-10, but the
*authorization entry* the Soroban host produces is bound to the user's
actual key signature — so even a fully compromised backend cannot obtain
a valid execution of something the user did not sign.

```
user ──GET build-invocation──► backend
user ◄─unsigned envelope────── backend
user signs with wallet key
user ──POST /invoke───────────► backend ──verify + fee bump──► Soroban RPC
```

**When it fits:** state-changing operations where users keep custody of
keys but lack XLM for fees.

---

## Pattern C — Contract-side verification of SEP-10-bound payloads

For contracts that must be trustless w.r.t. the backend (e.g. value-bearing
escrow release), mirror the check on-chain: the backend issues a *signed
payload* binding `{contract, function, caller, expiry, nonce}` and the
contract verifies the signature itself before executing.

Soroban exposes `ed25519_verify` through `Env::crypto()`. The verifying
party is whatever account/keypair your backend uses for this purpose —
store its public key in the contract at initialization:

```rust
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Payload the backend signs for each gated action.
#[derive(Clone)]
struct GatePayload {
    /// Function the caller may execute.
    action: Symbol,
    /// Account the payload was issued for.
    caller: Address,
    /// Single-use nonce.
    nonce: u128,
    /// Ledger number after which the payload is worthless.
    expires_at_ledger: u32,
}

pub fn gated_action(env: Env, payload_bytes: BytesN<96>, sig: BytesN<64>, args: Vec<val::Val>) {
    let decoded = decode_payload(&env, &payload_bytes); // your canonical encoding
    let stored_key: BytesN<32> = env
        .storage()
        .instance()
        .get(&(symbol_short!("gatekey"),))
        .unwrap();

    // 1. Signature must come from the trusted auth service key,
    //    over exactly these payload bytes.
    env.crypto().ed25519_verify(&stored_key, &payload_bytes.to_bytes(), &sig);

    // 2. Replay protection: nonces are single-use.
    assert!(!nonce_used(&env, decoded.nonce), "nonce replay");
    mark_nonce_used(&env, decoded.nonce);

    // 3. Freshness: expired payloads are worthless.
    assert!(env.ledger().sequence() <= decoded.expires_at_ledger, "expired");

    // 4. The payload names both the caller and the action being taken;
    //    the invoker cannot redirect it elsewhere.
    assert_eq!(decoded.action, symbol_short!("lock_asset"));
    decoded.caller.require_auth();

    execute_lock_asset(&env, &decoded.caller, args);
}
```

(The helper functions `decode_payload`, `nonce_used`, `mark_nonce_used`,
and `execute_lock_asset` are application-specific; the security-relevant
lines are the ones shown in full.)

| Off-chain concept | On-chain mirror |
| --- | --- |
| Challenge nonce | `nonce_used` storage set |
| Timebounds | `expires_at_ledger` vs `env.ledger().sequence()` |
| Session subject | `caller` inside the signed payload + `require_auth` |
| Endpoint allowlist | `action` match against permitted symbols |

> Note: `Address::require_auth(&env)` on `decoded.caller` additionally
> forces the user to sign the Soroban authorization entry themselves when
> submitting directly — combine Patterns B and C for defense in depth:
> the relayer gate off-chain, the payload gate on-chain.

**When it fits:** escrow release, payouts, transfers of real value — any
place where "the backend said so" is not acceptable finality.

---

## Choosing a pattern

| Requirement | A | B | C |
| --- | :-: | :-: | :-: |
| No contract changes | ✔ | ✔ | ✖ |
| User keeps key custody end-to-end | ✖ | ✔ | ✔ |
| Survives malicious backend | ✖ | partial | ✔ |
| Works for fee-less users | ✔ | ✔ | ✔ (with relayer) |

## GrantFox / Stellar Community Fund eligibility notes

Reviewers look for correct SEP-10 adoption plus a credible story about how
authentication reaches the contract layer:

1. Run the standard flow against your deployed anchor domain (TOML
   `WEB_AUTH_FOR_CONTRACTS` entry pointing at the auth endpoint).
2. Demonstrate replay rejection — the test suite in this directory doubles
   as evidence (`npm test`).
3. State explicitly which pattern(s) your app uses per endpoint class, and
   why (the table above is a reasonable starting point).
4. For Pattern C, include the payload format and nonce handling in your
   docs; reviewers will look for single-use enforcement and expiry.
