# Impact on the Stellar ecosystem

What this contribution changes in concrete terms. Every number below is a count
of something in this repository or a result observed while building it — there
are no adoption figures, download counts or user metrics here, because none have
been measured.

## The gap this closes

Before this change, the repository contained Soroban contracts and no client.
Anyone wanting to call `place_limit_order` from an application had to read
`swaptrade-contracts/counter/src/lib.rs`, work out the argument order, and hand-encode
each value into an `ScVal` — including the cases where the naive mapping is
wrong (`Option::None` is `ScVal::Void`, a fieldless enum is a one-element
vector, `i128` overflows a JavaScript `number` above 2^53). There was no
reference for the build → simulate → assemble → sign → submit → poll sequence, and
no runnable end-to-end example.

That work is now done once, in a typed and tested layer, instead of once per
integrator.

## Measurable engineering output

### Code

| Item | Count |
| --- | --- |
| SDK source | 1,672 lines across 7 modules |
| SDK public client methods | 26 — three generic (`buildTransaction`, `simulate`, `invoke`) plus 23 mapped to real contract entry points |
| Contract error codes catalogued with names | 46 |
| Typed error classes | 9, all extending one base |
| Example DApp source | 931 lines of TypeScript across 7 files, plus 168 lines of CSS |
| Contracts modified | **0** |

The SDK wraps only entry points that exist. Where the issue's vocabulary had no
counterpart in the contracts — there is no `create_swap`, `fund_swap` or
`accept_swap` — the workflow was mapped onto the real primitives
(`place_limit_order`, `mint`, `execute_due_orders`) and the mismatch documented,
rather than adding contract functions to make the example read more neatly.

### Tests

| Suite | Tests | Network access |
| --- | --- | --- |
| SDK unit (`packages/swaptrade-sdk/test`) | 95 | none — fake injected at the `RpcServerLike` seam |
| Demo component (`examples/swap-demo/test`) | 26 | none — fake client at the SDK boundary, fake wallet at the signer seam |
| Browser smoke (`examples/swap-demo/e2e`) | 8 | real Chromium, production build, deliberately unreachable RPC, mock wallet |
| **Total** | **129** | |

1,758 lines of test code against 2,603 lines of TypeScript source. The mocks sit
at boundaries, not in the middle: SDK tests exercise the real encoder, the real
transaction builder, the real signer and the real polling loop, then decode the
built XDR to assert on the exact method name and argument list that would reach
the contract. A test that mocked the encoder could not have caught an argument
in the wrong position; these can.

The same rule applies to signing. The browser tests inject a wallet at
`globalThis.freighterApi` — the global a real extension uses — so the demo's own
detection code and the SDK's wallet adapter both run for real. The mock holds no
key; it records the request and declines, which is enough to prove the path is
wired and that a refusal surfaces to the user.

Signature tests verify cryptographically rather than structurally — they parse
the signed envelope, recompute the transaction hash, and check the signature
against the public key.

### Automation

| Before | After |
| --- | --- |
| 12 manual localnet steps | `npm run localnet:deploy` — one command |
| No way to check the SDK against a live chain | `npm run localnet:verify` — exercises both SDK paths and exits non-zero if either fails |
| 0 active CI checks on client code | 2 jobs — build, typecheck, 121 unit tests, 8 browser smoke tests, three-part secret scan |

The three existing Rust workflows (`ci.yml`, `format.yml`,
`formal_verification.yml`) are fully commented out upstream and were left
exactly as they are. The new `sdk.yml` is additive and path-filtered to
`packages/**` and `examples/**`, so it does not run on contract-only changes.

One CI step is a supply-chain check rather than a build check. It asserts three
things: no Stellar secret-key-shaped string (`S[A-Z2-7]{55}`) appears in the built
browser bundle; no secret-shaped `VITE_` variable is referenced by the bundle; and
no source file under `examples/swap-demo` reads one. The third check exists
because the first two would pass by accident if the variable were simply unset in
CI — the leak would only appear once a developer set it locally. Vite inlines
every `VITE_`-prefixed variable into the bundle, which makes this a realistic
mistake rather than a theoretical one.

### Security posture

The browser demo has **no secret-key code path at all**, which is a stronger
property than having one that is discouraged:

- `examples/swap-demo/src/signer.ts` is the only place that decides how a
  transaction is signed, and it imports `browserWalletSigner` only. The SDK's
  `keypairSigner` is never imported into anything that Vite bundles, so setting a
  `VITE_*_SECRET_KEY` has no effect — there is nothing to read it.
- Signing goes through the SDK's `SignTransaction` callback to an injected
  Freighter-style wallet. The key stays in the extension, outside the page, and
  the user approves each signature.
- There is no private-key input field in the UI, and two independent layers
  assert its absence: a component test enumerating every rendered input, and a
  browser test that fetches the served JavaScript and checks the artifact rather
  than the source.
- With no wallet installed the app degrades to read-only — the workflow buttons
  disable and refresh still works, because reads are simulate-only. A missing
  extension is reported as something to install, never as a prompt for a key.
- Keypair signing remains available where it is safe: `scripts/verify_localnet.ts`
  runs in Node, where the key stays in the process and never enters an asset
  served to anyone. Its documented default is `--ephemeral`, which generates a
  throwaway identity, funds it via friendbot and discards it, so the happy path
  involves no stored or pasted credential at all.
- No credential-shaped literal exists anywhere in the tree. Tests that need a
  keypair derive one: `Keypair.fromRawEd25519Seed(Buffer.alloc(32, 7))`.
- `rpcUrl`, `networkPassphrase`, `contractId` and `publicKey` are required with
  no defaults. A default network passphrase is a signing accident waiting to
  happen — a transaction signed for the wrong network is a valid transaction for
  that network.
- Plain HTTP is rejected except for loopback, unless explicitly opted into.
- 4 dependency vulnerabilities (1 critical, 1 high, 2 moderate — Vite path
  traversal, `server.fs.deny` bypass, esbuild request forgery, Vitest UI
  arbitrary file read) were resolved by upgrading rather than suppressed with
  audit exceptions. `npm audit` reports 0.

### Verified on-chain, not just in tests

The full SDK pipeline was executed against a real Stellar network, and the
resulting transaction independently confirmed:

| | |
| --- | --- |
| Network | `stellar/quickstart:latest` localnet, `Standalone Network ; February 2017` |
| Contract deployed | `CBKMKKR73AOFQBJSOY55BJVDBMPDIY2XHI5WDHEFX6LVXAFJ3TK4DINH` |
| SDK `simulate()` | returned `"pong"` |
| SDK `invoke()` | returned `"pong"`, status `SUCCESS`, ledger 1281 |
| Transaction hash | `55d1ddabefb46aa4810cdb1c7d41b48bcb64f08ed8b8ac762401c7c987a94c15` |
| Independent check | raw `getTransaction` on that hash → `SUCCESS`, ledger 1281 |

This matters because a fake RPC server cannot catch a malformed resource
footprint, a wrong network passphrase, or a signature over the wrong hash. Those
only fail against a real node, and they did not fail here.

The scope of that verification is bounded and stated plainly: it ran against
`soroban-ping`, the one workspace member that compiles. The `counter` crate —
which holds every method the SDK wraps — has 152 pre-existing compilation errors
on a clean checkout at `1b76016`. Its methods are verified against the contract
source and by unit tests, not on-chain. See
[LOCALNET.md](LOCALNET.md#what-was-actually-verified).

## Why this is useful beyond this repository

**The encoding work transfers.** The Rust-to-ScVal mismatches handled in
`scval.ts` are not SwapTrade-specific; they are Soroban-wide. Any contract with
an `Option` field, a fieldless enum or an `i128` amount has the same traps, and
the table in [CONTRIBUTING_SDK.md](CONTRIBUTING_SDK.md#encoding-gotchas) is a
reusable reference.

**The test seam is a pattern, not a fixture.** Injecting a fake at
`RpcServerLike` — one narrow interface, four methods — keeps the entire
transaction pipeline under test without a network. Other Soroban projects can
copy the shape.

**Documented failure is more useful than hidden failure.** The `counter` build
break, the `perform_swap` stub returning `Ok(0)`, the dormant Rust CI and the
`cargo install stellar-cli` failure on Rust 1.97.1 are all written down with the
exact errors. A newcomer hitting any of them now finds an explanation instead of
concluding the setup is their fault.

**Onboarding cost drops.** A developer new to the repository can go from clone to
a working transaction with `npm install`, `npm run localnet:deploy`,
`npm run localnet:verify` — and read the demo to see how the pieces connect.

## What this does not claim

- No adoption, usage or download figures. None have been measured.
- No performance claim. Nothing was benchmarked.
- No claim that the full trading workflow runs on-chain. It cannot until
  `counter` compiles, and that is a pre-existing condition this change does not
  address.
- No claim that the SDK is production-ready against a public network. It has been
  exercised against localnet only.
