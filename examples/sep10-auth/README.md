# SEP-10 / Soroban-Compatible Authentication Patterns

Reference implementation of [SEP-10](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md)
(Stellar Web Authentication) for SwapTrade: a working challenge-response
backend, replay protection, and the patterns for gating Soroban contract
invocations behind verified sessions.

```
┌────────┐  GET /auth?account=G…   ┌─────────┐
│ client │ ───────────────────────►│ backend │  builds challenge tx
│        │◄─────────────────────── │         │  (source=client, seq=0,
│        │  signed challenge XDR   │         │   manageData "<domain> auth")
│        │                         │         │
│ signs  │  POST /auth {transaction}          server co-signs first;
│ with   │ ───────────────────────►│         │  backend verifies both
│ wallet │  session token          │         │  signatures + replay guard
└────────┘◄─────────────────────── └─────────┘
     │
     │ POST /invoke  (Bearer token + user-signed envelope)
     ▼
Soroban RPC → atomic-swap contract
```

## What's in here

| File | Purpose |
| --- | --- |
| `src/sep10.ts` | Challenge construction, client co-signing helper, full server-side verification |
| `src/token.ts` | Dependency-free HMAC session tokens bound to the consumed challenge nonce |
| `src/nonce-store.ts` | Single-use nonce registry (the replay guard) |
| `src/middleware.ts` | Bearer-token gate for any endpoint that touches contract state |
| `src/contract-gate.ts` | Allowlist policy + relayer safety checks for Soroban invocations |
| `src/server.ts` | Runnable `node:http` demo wiring everything together |
| `test/sep10.test.ts` | 21 tests: accepted/rejected flows and replay protection |
| `docs/SOROBAN-INTEGRATION.md` | On-chain side: verifying SEP-10-bound payloads inside a Soroban contract |

## Quick start

```bash
npm install
npm test        # builds then runs the test suite (node:test, no extra deps)
npm run demo    # starts the demo server on :8787
```

## Step by step

### 1. Client requests a challenge

```bash
curl "http://localhost:8787/auth?account=GCLIENT..."
# {
#   "transaction": "<base64 XDR challenge>",
#   "nonce": "...",            // opaque; tracked server-side
#   "clientAccountId": "GCLIENT..."
# }
```

The challenge is a transaction whose **source account is the client**
(that is how it names its subject) with **sequence number 0**, containing
one `manageData` operation named `<domain> auth` carrying **64 random
bytes**, valid between *now* and *now + 300 s*. The backend signs it first
with its domain account key.

### 2. Client co-signs

Wherever the user's secret lives (Freighter, wallet, custodial signer):

```ts
import { TransactionBuilder } from "@stellar/stellar-sdk";

const tx = TransactionBuilder.fromXDR(challenge.transaction, Networks.TESTNET);
tx.sign(clientKeypair); // e.g. Freighter sign transaction
const signed = tx.toXDR();
```

`signChallenge()` in `src/sep10.ts` does the same for non-wallet clients.

### 3. Backend verifies and issues a session

```bash
curl -X POST http://localhost:8787/auth \
  -H 'content-type: application/json' \
  -d '{"transaction":"<co-signed XDR>"}'
# { "token": "<payload>.<hmac>", "expires_at": 1700001200 }
```

Verification (`verifyChallenge`) enforces, in order:

1. envelope decodes for the configured network passphrase;
2. sequence number is exactly `0`;
3. exactly one `manageData` operation, named `<domain> auth`,
   sourced by the server's domain account, value decoding to 64 bytes;
4. timebounds still satisfied (5 s clock skew tolerated);
5. signature set is exactly `{server, client}` — every signature must be
   attributable to one of those two keys over the transaction hash;
6. the nonce has never been redeemed (**replay protection**).

Failures return a uniform `401`; specifics go to logs only, so attackers
learn nothing about which check tripped.

### 4. Gate contract invocations with the session

```bash
curl -X POST http://localhost:8787/invoke \
  -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"contractId":"C…","functionName":"get_quote","args":[42],
       "signedEnvelopeXdr":"<user-signed invoke envelope>"}'
```

Two independent gates run before anything reaches Soroban RPC:

* **Policy** (`authorizeInvocation`) — allowlisted contracts/functions per
  session tier, plus a per-session invocation quota keyed by the token's
  `jti`.
* **Relayer safety** (`verifyUserAuthorization`) — because the backend pays
  fees, it refuses to submit unless the *envelope source equals the SEP-10
  session subject* and *that account actually signed these exact
  operations*. A compromised or malicious client cannot get the relayer to
  fund arbitrary executions.

See `docs/SOROBAN-INTEGRATION.md` for the on-chain mirror of this check
(verifying a SEP-10-bound payload inside a Soroban contract via
`ed25519_verify`).

## Security properties tested

| Threat | Test |
| --- | --- |
| Tampered challenge bytes | rejected (signature mismatch or decode failure) |
| Wrong signer / impostor | `UNKNOWN_SIGNER` |
| Missing server signature (forged challenge) | `MISSING_SIGNATURE` |
| Expired challenge | `TIMEBOUNDS` |
| Future-dated challenge | `TIMEBOUNDS` |
| Replayed (already-redeemed) challenge | `NONCE_REPLAY` |
| Non-zero sequence number | `BAD_SEQUENCE` |
| Smuggled extra operations | `UNEXPECTED_OPERATION` |
| Garbage input | rejected |
| Expired / forged session token | rejected (constant-time MAC compare) |
| Off-policy contract call | `FORBIDDEN_TARGET` |
| Session quota exhaustion | `SESSION_QUOTA` |
| Relayer tricked into unsigned submission | rejected |

## Production checklist

- Load `sessionSecret` and `SEP10_SERVER_SECRET` from a secret manager.
- Replace the in-memory `NonceStore` with a shared store (Redis) when you
  run more than one backend replica.
- Anchor domain TOML: publish your auth server under `AUTH_SERVER` /
  `WEB_AUTH_FOR_CONTRACTS` so wallets can discover it.
- Consider rotating the HMAC secret with overlapping validity windows.
