# Contributing to the SwapTrade SDK and examples

This guide covers the **TypeScript SDK** (`packages/swaptrade-sdk`) and the
**example DApp** (`examples/swap-demo`). For the Rust contracts, see the root
[`README.md`](../README.md) and [`SECURITY.md`](../SECURITY.md).

## Setup

```bash
git clone <your fork>
cd swaptrade-contract
npm install                                # installs both workspaces
npm run build --workspace @swaptrade/sdk    # the demo imports the built SDK
```

Requires Node.js 20+. For contract work you also need Rust and the
`wasm32v1-none` target — see [`docs/LOCALNET.md`](LOCALNET.md).

### Repository layout

```
packages/swaptrade-sdk/     TypeScript SDK
  src/client.ts             build / simulate / sign / submit; one method per contract entry point
  src/config.ts             validation and defaults -> frozen ResolvedConfig
  src/scval.ts              ScVal encoding and decoding
  src/errors.ts             error classes + contract error-code catalogue
  src/types.ts              public types mirroring the on-chain structs
  src/signers.ts            wallet and keypair adapters
  test/                     95 tests, no network access

examples/swap-demo/         React demo
  src/config.ts             the only reader of import.meta.env
  src/signer.ts             the only signing decision; browser wallet only
  src/workflow.ts           the entire SDK boundary
  src/useSwapWorkflow.ts    all mutable state
  src/components.tsx        presentational only
  test/                     26 component tests
  e2e/                      8 Playwright smoke tests

scripts/localnet_deploy.sh  localnet + build + deploy
scripts/verify_localnet.ts  drives the SDK against a live localnet
```

## Architecture rules

These are the constraints that keep the layering honest. A change that breaks one
of them will be asked to change.

**1. Layering is one-directional.**

```
React components -> workflow.ts -> @swaptrade/sdk -> @stellar/stellar-sdk -> contract
```

No component imports the SDK, builds a transaction, or encodes an ScVal. If a
component needs new chain data, add it to `workflow.ts`.

**2. The contract is the source of truth.** Every SDK method must match a real
entry point in `swaptrade-contracts/counter`. Do not add a wrapper for a method
that does not exist, and do not change contract behaviour to make a wrapper
simpler. If a contract limitation blocks something, document it rather than
working around it silently.

**3. Never guess a network, contract, or identity.** `rpcUrl`,
`networkPassphrase`, `contractId` and `publicKey` are required with no defaults.
A wrong default means signing against the wrong chain.

**4. Amounts are `bigint`.** Contract `i128` / `u128` / `u64` values map to
`bigint` in and out. Accepting a `number` would silently lose precision above
2^53, so the SDK rejects one.

**5. No secrets in the repository — and none in the browser.** No private keys,
seed phrases, RPC credentials, contract IDs or user wallet data — not in source,
tests, fixtures, or docs. Tests derive keypairs from a fixed seed
(`Keypair.fromRawEd25519Seed(Buffer.alloc(32, 7))`) so no credential-shaped
literal appears anywhere.

Beyond the repository, a browser bundle is a public asset. Vite inlines every
`VITE_`-prefixed variable into the JavaScript it ships, so a key supplied that
way is published, not configured. The example DApp consequently has **no
secret-key code path**: `src/signer.ts` imports `browserWalletSigner` only, and
`keypairSigner` is reserved for Node scripts where the key stays in the process.
Do not add a `VITE_*_SECRET_KEY`, and do not add a key input field — CI fails on
the first, and tests fail on the second.

**6. The SDK stays thin.** It exists to encode arguments and decode results. It
does not re-wrap what `@stellar/stellar-sdk` already does cleanly — keypairs,
`StrKey`, XDR primitives — and holds no state beyond its configuration.

## Adding an SDK method

Take `mint` as the model:

1. **Read the contract signature** in `swaptrade-contracts/counter/src/lib.rs`.
   Argument order matters; it is positional on the wire.
2. **Validate first.** Use `assertSymbol`, `assertAccountId`,
   `assertPositiveAmount` so bad input fails before any network call.
3. **Encode each argument** with the helper matching the Rust type
   (`symbolToScVal`, `i128ToScVal`, `optionToScVal`, `unitEnumToScVal`,
   `tupleToScVal`, …).
4. **Pick the right path.** Read-only → `simulate()` (no fee, no signer).
   State-changing → `invoke()`.
5. **Decode the result** into a type from `types.ts`, adding a decoder to
   `scval.ts` if the shape is new.
6. **Add a test** that decodes the built XDR and asserts the exact method name
   and argument list.

```ts
async mint(token: string, to: string, amount: bigint): Promise<TransactionResult<void>> {
  return this.invoke('mint', [
    symbolToScVal(assertSymbol(token), 'token'),
    addressToScVal(assertAccountId(to, 'to')),
    i128ToScVal(assertPositiveAmount(amount)),
  ]);
}
```

Not every method needs a wrapper: `buildTransaction`, `simulate` and `invoke` are
public, so callers can reach a new contract method immediately.

### Encoding gotchas

Places where Rust and JavaScript do not line up, and a naive mapping is wrong:

| Rust | Wire form | Notes |
| --- | --- | --- |
| `Option::None` | `ScVal::Void` | Decodes to `undefined`, never `0`. |
| `Option::Some(0)` | the inner value | A falsy value must not collapse to `None`. |
| fieldless enum | `scvVec([Symbol(name)])` | Variant **order** matters: Soroban also encodes by index. |
| `(A, B)` tuple | `scvVec([a, b])` | |
| `symbol_short!` | `Symbol` | Capped at **9** chars; general `Symbol` at 32. |

## Adding an example

Add a new workspace under `examples/`; it is picked up automatically by the
`examples/*` glob. It must depend only on `@swaptrade/sdk`, read all
configuration from the environment, ship a `.env.example` with placeholders, and
add `typecheck`, `test` and `build` scripts so CI covers it.

Prefer extending `swap-demo` when you are demonstrating another *workflow* rather
than another *framework*.

## Testing

```bash
npm run test                                       # both workspaces
npm run test --workspace @swaptrade/sdk             # 95 SDK tests
npm run test --workspace @swaptrade/swap-demo       # 26 component tests
npm run test:e2e --workspace @swaptrade/swap-demo   # 8 Playwright smoke tests
npm run typecheck                                   # both workspaces
```

**No unit test may touch the network.** Mock at a boundary, not in the middle:

- **SDK tests** inject a fake at `RpcServerLike`. Everything above it — encoding,
  building, simulation handling, signing, submitting, polling — is the real
  implementation. Tests decode the built XDR to assert on the exact wire call.
- **Component tests** inject a fake client at the SDK boundary and assert on
  roles and rendered text. Do not assert on component internals, props or state.
- **Smoke tests** run the production build in real Chromium with the real SDK,
  pointed at an unreachable RPC port so they stay self-contained. Signing uses a
  mock wallet installed at `globalThis.freighterApi` via `addInitScript`, so the
  real signer-detection and wallet-adapter code runs. Never give a browser test a
  secret key — it would end up in the bundle the test builds.

**Playwright is the only browser-test tool here. Do not add Cypress.**

For changes to signing, submission or encoding, also run the live check in
[`docs/LOCALNET.md`](LOCALNET.md) — a fake RPC server cannot catch a malformed
footprint or a wrong passphrase.

## Before opening a pull request

```bash
npm run typecheck
npm run test
npm run build --workspace @swaptrade/sdk
npm run build --workspace @swaptrade/swap-demo
npm run test:e2e --workspace @swaptrade/swap-demo
git diff --stat && git diff
```

Check that the diff contains no secrets or `.env` files, no `node_modules/`,
`dist/` or `playwright-report/`, no editor/IDE files, no debugging statements, no
unrelated reformatting, and no unnecessary lockfile churn.

### Pull request expectations

Use [`.github/PULL_REQUEST_TEMPLATE.md`](../.github/PULL_REQUEST_TEMPLATE.md).

- **Scope one PR to one concern.** Do not mix an SDK method, a UI change and a CI
  tweak.
- **Link the issue** it closes.
- **Paste real command output** for what you ran. If something fails, say so and
  classify it: caused by this change, pre-existing, or environmental. Do not
  modify unrelated code to hide a pre-existing failure.
- **State what you did not verify.** An honest gap is fine; an unverified claim
  is not.
- **Note the pre-existing `counter` build failure** if it blocked you: 152 errors
  on a clean checkout at `1b76016`, unrelated to the SDK.

## Known repository state

Worth knowing before you file a bug:

| Issue | Detail |
| --- | --- |
| `counter` does not compile | 152 errors on a clean checkout at `1b76016`. |
| `perform_swap` is a stub | `counter/src/swap.rs` runs its checks then returns `Ok(0)`. |
| No swap lifecycle | No `create_swap` / `fund_swap` / `accept_swap`; the demo maps onto `place_limit_order`, `mint`, `execute_due_orders`. |
| Rust CI is dormant | `ci.yml`, `format.yml` and `formal_verification.yml` are fully commented out. `sdk.yml` covers only `packages/` and `examples/`. |
| `.gitignore` ignores `*.json` | A broad rule for generated artifacts. Scoped negations re-include the JSON files needed to install the workspace — add one if you introduce another. |
