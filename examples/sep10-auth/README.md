# SEP-10 Authentication for SwapTrade (Reference Implementation)

Issue #260 — SEP-10 / Soroban-compatible authentication patterns and
examples for secure user onboarding into swaps and approvals.

This package is a dependency-light, fully offline reference implementation of
the [SEP-10](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md)
challenge/response flow, plus two integration patterns showing how an
authenticated identity gates **Soroban contract invocations**. It is written
in TypeScript with no runtime dependencies other than `tweetnacl` (ed25519),
so it runs anywhere the SwapTrade backend runs.

```
src/sep10.ts     challenge creation + verification, strkey codec,
                 session tokens (JWT analogue), replay guard
src/gating.ts    Sep10Gate: session-gated contract intents and a
                 fee-sponsored relayer flow with signed intents
__tests__/       14 tests: accepted flows, rejected flows, replay protection
```

## Quick start

```bash
cd examples/sep10-auth
npm install        # --ignore-scripts is fine; tweetnacl has none anyway
npm test           # 14 passing tests
npx tsc --noEmit   # strict typecheck
```

## The flow, step by step

### 1. Server issues a challenge

```ts
import { createChallenge } from "./src/sep10.js";

const serverSeedHex = "...64 hex chars...";            // server signing key seed
const { challenge, signedPayload } = createChallenge(
  serverSeedHex,
  clientAccountIdGStrKey,
  Math.floor(Date.now() / 1000),
  "dapp.example.com",                                   // optional client_domain
);
// `signedPayload` is the canonical byte string both parties sign.
// Sign it with the SERVER key and return { challenge, serverSignature }
// to the client.
```

In full SEP-10 this payload is carried inside a Stellar transaction with
`manage_data` operations (`swaptrade auth` -> nonce,
`swaptrade client_domain` -> domain). The signing and verification rules are
identical; swap this layer for `@stellar/stellar-sdk`'s transaction builder
when wiring against a real anchor/domain server.

### 2. Client countersigns

```ts
import { signBytes } from "./src/sep10.js";
const clientSignature = signBytes(signedPayload, clientSeedHex);
```

The client signs with the ed25519 key matching its `G...` account — exactly
what Stellar wallets (Freighter, etc.) do during SEP-10 web auth.

### 3. Server verifies and mints a session token

```ts
import nacl from "tweetnacl";
import { Sep10Gate } from "./src/gating.js";

const gate = new Sep10Gate(serverPublicKeyRaw, serverSeedHex);

const result = gate.completeChallenge(challenge, serverSig, clientSig);
if (!result.allowed) throw new Error(result.reason);
const sessionToken = result.token!;   // HMAC-signed, expiring JWT analogue
```

Verification enforces all SEP-10 core rules:

| Rule | Where |
| --- | --- |
| Both signatures present and valid | `verifyChallenge` |
| Server signature binds the advertised server account | step 1 of `verifyChallenge` |
| Client signature matches `challenge.clientAccount` | step 2 of `verifyChallenge` |
| Challenge used within its validity window | time check in `verifyChallenge` |
| Challenge nonce single-use (replay protection) | `ReplayGuard` in `Sep10Gate.completeChallenge` |

### 4a. Gate contract-invoking operations on the session

```ts
const intent = {
  contractId: "CAS3J7GYLGXMF6TDJBBYYE3JNNFRVLDDTT6E8B2LNL4N25Q6YVGB72PI",
  functionName: "execute_swap",
  args: [{ amount: "100000000" }],
  nonce: nextNonce(),
};

const verdict = gate.authorizeWithToken(sessionToken, intent);
if (!verdict.allowed) return res.status(401).json({ error: verdict.reason });

// Safe to relay: build & submit the Soroban invocation here, using
// verdict.account as the authorization entry signer / source account.
await sorobanServer.submitTransaction(buildInvocation(verdict.account!, intent));
```

On-chain authorization stays anchored to the authenticated identity because
the relayer uses the verified account in the Soroban auth entry rather than
trusting a client-supplied address.

### 4b. Fee-sponsored relayer with signed intents

For flows where the backend pays fees, clients sign the *intent* itself:

```ts
import { intentPayload } from "./src/gating.js";
const signature = signBytes(intentPayload(intent), clientSeedHex);

const verdict = gate.authorizeSignedIntent(clientAccountId, intent, signature);
```

The relayer validates the ed25519 signature against the claimed account and
enforces per-account single-use `(contract, function, args, nonce)` tuples,
preventing both forgery and replay.

## Adopting SEP-10 in your DApp (GrantFox / Stellar support eligibility)

Stellar and GrantFox look for standards-compliant authentication when
evaluating DApps. To adopt:

1. Run an auth endpoint implementing steps 1–3 above (or use a hosted
   anchor's `/auth`).
2. Treat the resulting token as your session credential; never accept raw
   account ids from request bodies for privileged operations.
3. Gate every contract-invoking route through pattern 4a or 4b.
4. Keep nonces single-use and challenges short-lived (15 min default) —
   both are enforced by this module.

## Test coverage

`npm test` exercises:

- accepted end-to-end challenge -> token -> gated invocation
- rejected flows: wrong client key, tampered payloads, expired/out-of-window
  challenges, forged/expired session tokens, malformed accounts
- replay protections: reused challenge nonce, repeated intent under a fresh
  token, repeated signed intent at the relayer
