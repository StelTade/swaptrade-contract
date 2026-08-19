# Running the SDK and demo against Stellar localnet

A reproducible, from-scratch walkthrough: local network → deployed contract →
working demo in a browser.

`scripts/localnet_deploy.sh` automates steps 3–9. This document spells them out
so you can run them individually and understand what each one does.

> **Read this first — contract build status.** On a clean checkout of `main`, the
> `counter` crate (which holds every method the SDK wraps) **does not compile**:
> `cargo check --workspace` reports 152 pre-existing errors. This is unrelated to
> the SDK and was verified on an untouched tree at commit `1b76016`. The
> consequence is concrete and stated plainly: the localnet walkthrough below was
> executed and verified against `soroban-ping`, the one workspace member that
> builds. The full create → fund → accept path against `counter` cannot be
> executed until that crate compiles. See
> [What was actually verified](#what-was-actually-verified).

## Prerequisites

| Tool | Check | Install |
| --- | --- | --- |
| Docker | `docker --version` | <https://docs.docker.com/get-docker/> |
| Rust | `cargo --version` | <https://rustup.rs> |
| Node.js 20+ | `node --version` | <https://nodejs.org> |
| Stellar CLI | `stellar --version` | see note below |
| `curl` | `curl --version` | usually preinstalled |

> **Installing the Stellar CLI.** `cargo install --locked stellar-cli` failed on
> Rust 1.97.1 during this work: the transitive dependency `ethnum 1.5.2` does not
> compile (`error[E0512]: cannot transmute between types of different sizes`).
> Use a prebuilt release binary instead, which works:
>
> ```bash
> # Linux/macOS: pick the matching asset from
> # https://github.com/stellar/stellar-cli/releases
> curl -sL https://github.com/stellar/stellar-cli/releases/download/v27.1.0/stellar-cli-27.1.0-x86_64-unknown-linux-gnu.tar.gz | tar xz
> ```
>
> On Windows, use the `-x86_64-pc-windows-msvc.tar.gz` asset or the `-installer-`
> `.exe`. CLI v27.1.0 was used for the verification below.

## The 12 steps

### 1. Install the Rust wasm target

Soroban requires `wasm32v1-none`. `soroban-sdk`'s build script **rejects**
`wasm32-unknown-unknown` on Rust 1.82+, so this is not optional:

```bash
rustup target add wasm32v1-none
```

### 2. Install JavaScript dependencies

```bash
npm install        # from the repository root
```

This installs both workspaces: `packages/swaptrade-sdk` and `examples/swap-demo`.

### 3. Start the local network

```bash
docker run -d --name swaptrade-localnet \
  -p 8000:8000 \
  stellar/quickstart:latest --local --enable-soroban-rpc
```

### 4. Wait for RPC to become healthy

The container needs to close its first ledgers before it will answer:

```bash
curl -s -X POST http://localhost:8000/soroban/rpc \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
```

Repeat until you see `"status":"healthy"`:

```json
{"jsonrpc":"2.0","id":1,"result":{"status":"healthy","latestLedger":157,...}}
```

If it never becomes healthy, check `docker logs swaptrade-localnet`.

### 5. Register the network with the CLI

The passphrase must match `soroban.toml` exactly, including spaces around the
semicolon:

```bash
stellar network add local \
  --rpc-url http://localhost:8000/soroban/rpc \
  --network-passphrase 'Standalone Network ; February 2017'
```

CLI v27 has no `--overwrite` flag; if `local` already exists the command errors
harmlessly and the existing entry is used.

### 6. Create and fund an identity

```bash
stellar keys generate demo --network local --fund
stellar keys address demo          # prints the G... public key
```

### 7. Build the contract

```bash
cargo build --release --target wasm32v1-none -p soroban-ping
```

The wasm lands at `target/wasm32v1-none/release/soroban_ping.wasm`.

To attempt the full contract instead — expect the 152 pre-existing errors
described above:

```bash
cargo build --release --target wasm32v1-none -p counter
```

### 8. Deploy

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/soroban_ping.wasm \
  --source demo \
  --network local
```

This prints the contract ID (`C...`). Keep it.

### 9. Verify the deployment with a direct call

Confirm the contract is live before involving the SDK, so a later failure is
unambiguous:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source demo \
  --network local \
  -- ping
```

Expected output: `"pong"`.

### 9b. Verify the SDK's transaction pipeline against the live network

This is the step that proves the SDK itself works on a real network rather than
only against the fake RPC server used by the unit tests.

The simplest form generates a throwaway identity, funds it via friendbot, uses it
and discards it — no key is stored or pasted anywhere:

```bash
npm run build --workspace @swaptrade/sdk
node --experimental-strip-types scripts/verify_localnet.ts \
  --contract <CONTRACT_ID> --ephemeral
```

Or reuse the identity from step 6:

```bash
node --experimental-strip-types scripts/verify_localnet.ts \
  --contract <CONTRACT_ID> \
  --secret "$(stellar keys show demo)"
```

It runs both SDK paths — `simulate()` (read-only) and `invoke()` (build →
simulate → assemble → sign → submit → poll) — and fails loudly if either does
not return `"pong"`.

> A secret key is fine here and not in the browser. This is a Node process: the
> key stays in memory and its environment, and is never written into an asset
> served to anyone. The demo has no equivalent path, because Vite inlines
> `VITE_`-prefixed values into the public bundle.

### 10. Configure the demo

```bash
cp examples/swap-demo/.env.example examples/swap-demo/.env.local
```

Edit `.env.local`:

```
VITE_RPC_URL=http://localhost:8000/soroban/rpc
VITE_NETWORK_PASSPHRASE=Standalone Network ; February 2017
VITE_CONTRACT_ID=<CONTRACT_ID from step 8>
VITE_PUBLIC_KEY=<output of `stellar keys address demo`>
```

**None of those is a secret, and there is no variable for one.** `.env.local` is
git-ignored, but that is not the protection that matters: Vite inlines every
`VITE_`-prefixed value into the browser bundle, so anything in this file is
public in the built app. The demo has no code path that reads a signing key, so
adding one would have no effect.

To sign in the browser, install a Stellar wallet extension (such as Freighter)
and import the `demo` identity into it:

```bash
stellar keys show demo      # paste into the wallet — not into .env.local
```

To exercise signing without a wallet extension, use step 9b instead.

### 11. Build the SDK and start the demo

The demo imports the SDK's compiled output, so build it first:

```bash
npm run build --workspace @swaptrade/sdk
npm run demo
```

### 12. Run the workflow

Open <http://localhost:5173> and confirm the Connection panel shows your account,
`Standalone Network ; February 2017`, and `Injected browser wallet`. If it shows
`None — read-only`, no wallet was detected: the four workflow buttons stay
disabled and only **Refresh state** works. Then, in order:

| Button | Contract call | Expected |
| --- | --- | --- |
| **1. Prepare** | `kyc_submit`, `kyc_update_status`, `set_price` | Wallet prompts for each signature; Activity shows `SUCCESS` and a transaction hash. |
| **2. Create order** | `place_limit_order` | "On-chain state" shows the new order ID. |
| **3. Fund account** | `mint` | Balance increases after **Refresh state**. |
| **4. Accept / execute** | `execute_due_orders` | Activity lists the executed order IDs. |

Every hash shown is real: check any of them with

```bash
stellar events --network local --start-ledger <LEDGER>
```

### Tear down

```bash
docker rm -f swaptrade-localnet
```

## What was actually verified

Separating observed results from wired-up-but-unexecuted behaviour, so nothing
above reads as a stronger claim than it is.

### Executed and confirmed

Steps 1–9b were run on Windows 11 with Docker, Rust 1.97.1 and Stellar CLI
v27.1.0:

| What | Result |
| --- | --- |
| Localnet started | `stellar/quickstart:latest`, RPC `"status":"healthy"` |
| Identity funded | `GCW7OFUICLXFKFAHJHW4LSBBES7I74NC3Z3XWICBJMWT56L4VO2UUUKT` |
| Contract built | `soroban_ping.wasm`, 654 bytes, target `wasm32v1-none` |
| Contract deployed | `CBKMKKR73AOFQBJSOY55BJVDBMPDIY2XHI5WDHEFX6LVXAFJ3TK4DINH` |
| CLI invoke | `ping` → `"pong"` |
| **SDK `simulate()`** | `ping` → `"pong"` |
| **SDK `invoke()`** | `"pong"`, status `SUCCESS`, ledger `1281`, hash `55d1ddabefb46aa4810cdb1c7d41b48bcb64f08ed8b8ac762401c7c987a94c15` |
| Independent confirmation | `getTransaction` on that hash → `status: SUCCESS`, `ledger: 1281`, `applicationOrder: 1` |

That last pair is the meaningful result: the SDK's full pipeline — argument
encoding, transaction building, simulation, `assembleTransaction`, signing,
submission and polling — completed against a real network and the resulting
transaction was independently confirmed on-chain.

### Not executed

1. **`counter` does not compile.** 152 pre-existing errors on a clean checkout,
   confirmed at commit `1b76016` with an empty `git status`. Step 12's table
   therefore describes the demo's wiring, not an execution observed against a
   deployed `counter`. Every `counter`-specific method the SDK exposes is
   verified against the contract source and by unit tests, not on-chain.
2. **No `create_swap` / `fund_swap` / `accept_swap` exists.** The workflow is
   mapped onto real primitives (`place_limit_order`, `mint`,
   `execute_due_orders`). The contracts were not modified to make the demo
   simpler.
3. **`perform_swap` is a stub.** `counter/src/swap.rs` performs its safety checks
   and then returns `Ok(0)` with a `// ... rest of swap code` comment. `swap()`
   and `safeSwap()` are bound in the SDK, but the contract does not yet move
   balances.

### Automated coverage

| Suite | Count | Network |
| --- | --- | --- |
| SDK unit tests | 95 | none — fake RPC at the `RpcServerLike` seam |
| Demo component tests | 26 | none — fake client at the SDK boundary, fake wallet at the signer seam |
| Playwright smoke tests | 8 | real Chromium, production build, unreachable RPC, mock wallet |


## Troubleshooting

**`error: unsupported target 'wasm32-unknown-unknown'`**
You are on Rust 1.82+. Use `wasm32v1-none` (step 1).

**Demo shows "Configuration required"**
`VITE_CONTRACT_ID` or `VITE_PUBLIC_KEY` is unset in `.env.local`. Vite only reads
env files at startup — restart the dev server after editing.

**`Refusing to use plain HTTP`**
The SDK allows plain HTTP only for loopback. Use `localhost`, or set
`allowHttp: true` explicitly for a remote endpoint.

**`Error(Contract, #500)` / `KYCVerificationRequired`**
Trading entry points require a verified account. Run **Prepare** first.

**`txInsufficientFee` or a timeout on submit**
The localnet was still catching up. Re-check `getHealth` (step 4) and retry.

**Port 8000 already in use**
`docker rm -f swaptrade-localnet`, or map a different port and update
`VITE_RPC_URL` to match.
