# @swaptrade/sdk

A lightweight TypeScript wrapper around the SwapTrade Soroban contracts.

The SDK does one thing: it turns typed JavaScript calls into correctly-encoded
Soroban contract invocations, and turns contract responses back into typed
JavaScript. It does not wrap the parts of `@stellar/stellar-sdk` that are already
clean — keypairs, `StrKey`, XDR primitives — and it holds no state beyond its
configuration.

```ts
import { SwapTradeClient } from '@swaptrade/sdk';

const client = new SwapTradeClient({
  rpcUrl: process.env.SOROBAN_RPC_URL!,
  networkPassphrase: process.env.SOROBAN_NETWORK_PASSPHRASE!,
  contractId: process.env.SWAPTRADE_CONTRACT_ID!,
  publicKey: process.env.SWAPTRADE_PUBLIC_KEY!,
  signTransaction: myWalletSigner,
});

const balance = await client.balanceOf('XLM');       // simulate only, no fee
const result = await client.mint('XLM', account, 1_000n); // signs and submits
console.log(result.hash, result.status);
```

## Install

The package is not published; it is consumed through the npm workspace at the
repository root.

```bash
npm install          # from the repository root
npm run build --workspace @swaptrade/sdk
```

## Configuration

Every field that determines *which chain and contract you are talking to* is
required. The SDK never falls back to a public network, because a silent default
here means signing a real transaction against the wrong chain.

| Field | Required | Notes |
| --- | --- | --- |
| `rpcUrl` | yes | Soroban RPC endpoint. |
| `networkPassphrase` | yes | Must match the RPC server's network. |
| `contractId` | yes | `C...`, validated with `StrKey`. |
| `publicKey` | yes | `G...`, used as the source account. |
| `signTransaction` | no | Omit for read-only use. |
| `allowHttp` | no | Defaults to `true` only for loopback hosts. |
| `fee` | no | Stroops per operation. Default `1000000`. |
| `timeoutSeconds` | no | Transaction validity window. Default `60`. |
| `pollTimeoutMs` | no | How long to wait for settlement. Default `30000`. |

Read these from the environment. Do not hardcode contract IDs, endpoints, keys or
passphrases:

```ts
import { NETWORKS, networkPreset } from '@swaptrade/sdk';

const { rpcUrl, networkPassphrase } = networkPreset('local'); // or 'testnet'
```

`NETWORKS` mirrors the values declared in `soroban.toml`.

### Signers

`signTransaction` is a callback, so the SDK stays agnostic about wallets:

```ts
type SignTransaction = (
  xdr: string,
  context: { networkPassphrase: string; address: string },
) => Promise<string> | string;
```

Two adapters ship with the SDK:

- **`browserWalletSigner(wallet)`** — Freighter-style injected wallets. Accepts
  both the current `{ signedTxXdr }` response and the older bare-string form.
  **This is the only correct choice for a browser app.**
- **`keypairSigner(secret)`** — signs locally from a secret seed. For Node
  scripts, CLI tools and tests, where the key stays in the process.

> **Never use `keypairSigner` in code that ships to a browser.** Bundlers inline
> environment variables into the output — Vite does this for anything
> `VITE_`-prefixed — so a key read from the environment at build time becomes a
> string in a public asset, recoverable from devtools or a plain `fetch`. The
> example DApp in `examples/swap-demo` therefore does not import
> `keypairSigner` at all, and its CI job fails if a secret-shaped variable
> appears in the bundle or is read by its source.

## What the SDK calls

The methods map onto `swaptrade-contracts/counter`. Read-only methods simulate
and return a decoded value; state-changing methods simulate, sign, submit and
poll, returning a `TransactionResult`.

| Category | Methods |
| --- | --- |
| Setup | `initialize`, `getContractVersion` |
| Balances | `mint`, `balanceOf`, `getPortfolio` |
| Orders | `placeLimitOrder`, `getOrder`, `getUserOrders`, `executeDueOrders`, `cancelOrder` |
| Swaps | `swap`, `safeSwap`, `setMaxSlippageBps` |
| Oracle | `setPrice`, `getCurrentPrice` |
| KYC | `kycSubmit`, `kycIsVerified`, `kycGetStatus`, `kycUpdateStatus`, `kycAddOperator` |
| Admin | `pauseTrading`, `resumeTrading`, `getUserTier` |

For anything not listed, `buildTransaction`, `simulate` and `invoke` are public,
so you can call an arbitrary method without waiting for a wrapper:

```ts
import { symbolToScVal, u64ToScVal } from '@swaptrade/sdk';

await client.invoke('some_new_method', [symbolToScVal('XLM', 'token'), u64ToScVal(1n)]);
```

## Amounts are `bigint`

Contract `i128` / `u128` / `u64` values are `bigint` in and out. A `number` would
silently lose precision above 2^53, so the SDK rejects one rather than truncating:

```ts
await client.mint('XLM', account, 1000);   // throws ValidationError
await client.mint('XLM', account, 1000n);  // correct
```

## Errors

Every failure is a `SwapTradeError` subclass with a `code`, so callers branch on
a discriminator instead of matching message strings.

| Class | `code` | Means |
| --- | --- | --- |
| `ConfigError` | `CONFIG_INVALID` | Missing or malformed configuration. |
| `ValidationError` | `ADDRESS_INVALID`, `CONTRACT_ID_INVALID`, `AMOUNT_INVALID`, `SYMBOL_INVALID` | A bad argument, caught before any network call. |
| `SimulationError` | `SIMULATION_FAILED` | Simulation failed. Nothing was submitted; no fee was charged. |
| `SigningError` | `SIGNING_FAILED` | Signer rejected the request or returned unusable XDR. |
| `RpcError` | `RPC_FAILED` | Transport-level failure reaching the RPC server. |
| `TransactionFailedError` | `TRANSACTION_FAILED` | Rejected by the network, or failed on-chain. |
| `TransactionTimeoutError` | `TRANSACTION_TIMEOUT` | Did not settle within `pollTimeoutMs`. Carries `hash` so you can keep checking. |
| `ContractCallError` | `CONTRACT_ERROR` | The contract returned an error. Carries `contractCode` and `contractName`. |

`ContractCallError` resolves the numeric code against the catalogue in
`counter/src/errors.rs`, which turns an opaque host error into something
actionable:

```ts
try {
  await client.placeLimitOrder({ /* ... */ });
} catch (error) {
  if (error instanceof ContractCallError) {
    // "KYCVerificationRequired (#500)" rather than "Error(Contract, #500)"
    console.error(error.contractName, error.contractCode);
  }
}
```

## Module layout

| File | Responsibility |
| --- | --- |
| `src/client.ts` | `SwapTradeClient`: build, simulate, sign, submit, poll; one method per contract entry point. |
| `src/config.ts` | Validation and defaults. Produces a frozen `ResolvedConfig`. |
| `src/scval.ts` | ScVal encoding and decoding, including `Option`, unit enums and tuples. |
| `src/errors.ts` | Error classes and the contract error-code catalogue. |
| `src/types.ts` | Public types mirroring the on-chain structs. |
| `src/signers.ts` | Wallet and keypair signer adapters. |

## Encoding notes

These are the places where the Rust and JavaScript type systems do not line up,
and where a naive mapping would be wrong:

- **`Option::None`** encodes to `ScVal::Void` and decodes to `undefined`, not `0`.
  Collapsing it would make "no expiry" indistinguishable from "expired at epoch".
- **Fieldless enums** encode as a single-element vector of the variant name,
  e.g. `KYCStatus::Verified` → `scvVec([Symbol("Verified")])`.
- **Tuples** encode as vectors: `(Symbol, Symbol)` → `scvVec([sym, sym])`.
- **`symbol_short!`** is capped at 9 characters; general `Symbol` at 32. The SDK
  validates against the correct limit per argument.

## Testing

```bash
npm run test --workspace @swaptrade/sdk
```

95 tests, no network access. The `RpcServerLike` interface is the injection
point: tests supply a fake server, and everything above it — argument encoding,
transaction building, simulation handling, signing, submission and polling — is
the real implementation. Tests decode the built XDR to assert on the exact
method name and argument list that would reach the contract.

```ts
const client = new SwapTradeClient(config, { server: myFakeServer });
```
