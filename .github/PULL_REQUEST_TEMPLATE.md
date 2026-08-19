<!--
Delete any section that does not apply. A short, accurate PR is better than a
long one padded with boilerplate.
-->

## Summary

<!-- What changed and why, in two or three sentences. -->

Closes #

## Type of change

- [ ] Contract change (Rust / Soroban)
- [ ] SDK change (`packages/swaptrade-sdk`)
- [ ] Example DApp change (`examples/swap-demo`)
- [ ] CI / tooling
- [ ] Documentation

## What changed

<!--
List the files that matter and why. Skip mechanical churn.

  - `packages/swaptrade-sdk/src/client.ts` — added `cancelOrder`
  - `packages/swaptrade-sdk/test/client.test.ts` — argument-mapping test
-->

## Contract compatibility

<!-- For SDK changes. Delete if not applicable. -->

- [ ] Every method added or changed matches a real entry point in `swaptrade-contracts/`
- [ ] Argument order matches the Rust signature (positional on the wire)
- [ ] `i128` / `u128` / `u64` values are `bigint`, not `number`
- [ ] `Option::None` decodes to `undefined`, not `0`
- [ ] No contract behaviour was changed to make the SDK or demo simpler

Contract entry point(s) this relies on:

<!-- e.g. `counter/src/lib.rs::cancel_order` (line 1795) -->

## Validation

Paste **real output**. If something failed, say so and classify it as caused by
this change, pre-existing, or environmental.

```
$ npm run typecheck

$ npm run test

$ npm run build --workspace @swaptrade/sdk

$ npm run test:e2e --workspace @swaptrade/swap-demo
```

### Localnet

<!--
If you touched signing, submission or encoding, run the live check in
docs/LOCALNET.md — a fake RPC server cannot catch a malformed footprint or a
wrong passphrase. Paste the contract ID and transaction hash.
-->

- [ ] Verified against localnet (`scripts/verify_localnet.ts`)
- [ ] Not applicable

### Not verified

<!--
State the gaps explicitly. An honest gap is fine; an unverified claim is not.
"The counter contract does not compile on a clean checkout, so X could not be
exercised on-chain" is a good entry.
-->

## Tests

- [ ] Added or updated tests for this change
- [ ] No unit test makes a real network call
- [ ] Component tests assert on rendered output, not component internals
- [ ] No Cypress was added (Playwright is the only browser-test tool here)

## Checklist

- [ ] No secrets: no private keys, seed phrases, RPC credentials, contract IDs or wallet data in source, tests, fixtures or docs
- [ ] No secret reaches the browser: no `VITE_*` secret variable, no key input field, no `keypairSigner` import under `examples/`
- [ ] No `.env` file is committed (`.env.example` with placeholders is fine)
- [ ] Configuration is read from the environment, not hardcoded
- [ ] No generated artifacts (`node_modules/`, `dist/`, `playwright-report/`, `target/`)
- [ ] No editor/IDE files
- [ ] No debugging statements left behind
- [ ] No unrelated reformatting or lockfile churn
- [ ] Layering respected: no chain logic inside React components
- [ ] Docs updated if behaviour or setup changed
- [ ] Reviewed my own `git diff` before requesting review

## Notes for reviewers

<!-- Anything non-obvious: a trade-off, something you were unsure about, a follow-up. -->
