# SwapTrade demo DApp

A minimal React app demonstrating the **create → fund → accept** workflow against
the SwapTrade Soroban contract, driven entirely through `@swaptrade/sdk`.

This is a demonstration of the SDK, not a product UI. It is intentionally small:
one screen, four buttons, plain CSS, no component library, no router, no state
management dependency.

## Layering

```
React components  ->  workflow.ts  ->  @swaptrade/sdk  ->  @stellar/stellar-sdk  ->  contract
```

The rule this app enforces is that **no component contains chain logic**. Nothing
under `src/components.tsx` imports the SDK, builds a transaction, encodes an
ScVal, or knows a blockchain exists — it renders props and raises callbacks.

| File | Responsibility |
| --- | --- |
| `src/config.ts` | The only place that reads `import.meta.env`. Returns a client or a list of configuration problems. |
| `src/signer.ts` | The only place that decides how a transaction gets signed. |
| `src/workflow.ts` | The whole SDK boundary. Each workflow step is one function. |
| `src/useSwapWorkflow.ts` | All mutable state and the step state machine. |
| `src/components.tsx` | Presentational only. Props in, callbacks out. |
| `src/App.tsx` | Wires configuration to the hook and lays out the panels. |

That split is also what makes the tests meaningful: `test/App.test.tsx` swaps in a
fake client at the SDK boundary, so the real workflow mapping and hook logic run
while assertions stay on rendered output.

## Which contract methods the steps use

`swaptrade-contracts/counter` has no `create_swap` / `fund_swap` / `accept_swap`
trio. The workflow from issue #254 is therefore mapped onto the primitives the
contract actually exposes:

| Demo step | Contract method(s) | Why |
| --- | --- | --- |
| **Prepare** | `kyc_submit`, `kyc_update_status`, `set_price` | Trading entry points are gated by `require_authenticated_verified_user`, and limit orders need an oracle price. |
| **Create** | `place_limit_order` | Creates the order and returns its `u64` ID. |
| **Fund** | `mint` | Credits the simulated asset the order spends. |
| **Accept** | `execute_due_orders` | Settles every order whose conditions are met, returning the executed IDs. |

## Configuration

```bash
cp .env.example .env.local     # .env.local is git-ignored
```

| Variable | Required | Notes |
| --- | --- | --- |
| `VITE_CONTRACT_ID` | yes | Printed by `scripts/localnet_deploy.sh`. No default. |
| `VITE_PUBLIC_KEY` | yes | Source account (`G...`). An address, not a credential. |
| `VITE_RPC_URL` | no | Defaults to the localnet endpoint from `soroban.toml`. |
| `VITE_NETWORK_PASSPHRASE` | no | Defaults to the localnet passphrase. |

**None of these is a secret, and there is deliberately no variable for one.**

> **Vite inlines every `VITE_`-prefixed variable into the browser bundle.** A
> secret key in `.env.local` is therefore not a secret in an environment
> variable — it is a secret pasted into a public asset, readable by anyone who
> opens devtools or fetches the JavaScript. That is true on localnet too, and an
> example that demonstrates the pattern teaches it.
>
> So this demo has no secret-key code path at all. `src/signer.ts` imports only
> `browserWalletSigner`; `keypairSigner` is never imported, and setting a
> `VITE_*_SECRET_KEY` has no effect. CI enforces this three ways: no
> secret-key-shaped string in the bundle, no secret-shaped `VITE_` variable in
> the bundle, and no source file reading one.

If a required variable is missing, the app renders a setup checklist naming the
variable instead of failing on first click.

### Signing

Signing goes through the SDK's `SignTransaction` callback, and the demo supplies
exactly one implementation of it in the browser:

```
components  ->  useSwapWorkflow  ->  workflow.ts  ->  SwapTradeClient
                                                            │
                                          signTransaction ◄─┘
                                                │
                                    browserWalletSigner(wallet)
                                                │
                                  globalThis.freighterApi  (extension holds the key)
```

`src/signer.ts` detects an injected Freighter-style wallet and adapts it. The key
stays in the extension, outside the page, and the user approves each signature.

With no wallet, the app runs read-only: the four workflow buttons are disabled
and "Refresh state" still works, because reads are simulate-only. A missing
extension is reported as something to install — never as a prompt for a key.

There is **no private-key input field** in the UI, and adding one would fail
tests in both `test/App.test.tsx` and `e2e/smoke.spec.ts`.

To sign on localnet without installing a wallet extension, use the Node script
instead, where the key stays in the process and out of any bundle:

```bash
npm run localnet:verify -- --contract <C...> --ephemeral
```

## Run

```bash
# From the repository root
npm install
npm run build --workspace @swaptrade/sdk   # the demo imports the built SDK
npm run demo                               # or: npm run dev --workspace @swaptrade/swap-demo
```

Then open <http://localhost:5173>.

For a working end-to-end run against a live contract, follow
[`docs/LOCALNET.md`](../../docs/LOCALNET.md).

## Tests

```bash
npm run test --workspace @swaptrade/swap-demo      # 26 component tests
npm run typecheck --workspace @swaptrade/swap-demo
npm run test:e2e --workspace @swaptrade/swap-demo  # 8 Playwright smoke tests
```

**Component tests** (`test/`) use Testing Library and assert on roles and
rendered text, never on component internals. The client is faked at the SDK
boundary via `test/fakeClient.ts`, and the wallet via `fakeWallet()`.
`test/signer.test.ts` pins the security property directly: an injected wallet is
adapted, a half-injected one is rejected, and no environment variable can produce
a signer.

**Smoke tests** (`e2e/`) run the *production build* in real Chromium with the
*real* SDK. They deliberately point at an RPC port with nothing behind it, which
makes them self-contained: they prove the app mounts, reads its configuration,
and surfaces a transport failure to the user rather than hanging. One test
fetches the served JavaScript and asserts no secret-key-shaped string is in it —
checking the artifact, not the source.

Signing in the browser test uses a mock wallet installed at
`globalThis.freighterApi` via `addInitScript` (`e2e/mockWallet.ts`), the same
global a real extension uses. It holds no key: it records the request and
declines, which is enough to prove the wallet path is wired and that a refusal
reaches the user. Playwright is the only browser-test tool in this repository —
do not add Cypress.

## Adding a step

1. Add the SDK call as a function in `src/workflow.ts`, returning a `StepOutcome`.
2. Add its name to `WORKFLOW_STEPS`.
3. Add an action to `useSwapWorkflow.ts` that calls it through `run()`.
4. Add a button to `WorkflowControls`.
5. Add a test asserting the SDK method was called with the right arguments.

If step 1 requires a contract method the SDK does not wrap yet, add it to
`packages/swaptrade-sdk/src/client.ts` first — components must not reach past the
SDK.
