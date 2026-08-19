#!/usr/bin/env bash
#
# Start a Stellar localnet, build a contract, deploy it, and print the
# environment the demo needs.
#
# Deliberately does NOT write .env.local for you: the file is git-ignored and
# holds a key, so creating it is left as an explicit copy-paste step.
#
# Usage:
#   scripts/localnet_deploy.sh                 # deploy soroban-ping (builds cleanly)
#   CONTRACT=counter scripts/localnet_deploy.sh # attempt the counter contract
#
# See docs/LOCALNET.md for the full walkthrough and the current build status of
# each contract.

set -euo pipefail

CONTRACT="${CONTRACT:-soroban-ping}"
NETWORK_NAME="${NETWORK_NAME:-local}"
RPC_URL="${RPC_URL:-http://localhost:8000/soroban/rpc}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
CONTAINER="${CONTAINER:-swaptrade-localnet}"
IDENTITY="${IDENTITY:-demo}"
WASM_TARGET="wasm32v1-none"

# Crate name -> wasm file name (cargo replaces dashes with underscores).
WASM_NAME="${CONTRACT//-/_}.wasm"
WASM_PATH="target/${WASM_TARGET}/release/${WASM_NAME}"

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m warn:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# --- 1. Preconditions ------------------------------------------------------

command -v docker >/dev/null 2>&1 || die "docker is required but not on PATH."
command -v cargo  >/dev/null 2>&1 || die "cargo is required but not on PATH."
command -v stellar >/dev/null 2>&1 || die \
  "The stellar CLI is required. Install it with:
    cargo install --locked stellar-cli
  or see https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli"

# soroban-sdk's build script rejects wasm32-unknown-unknown on Rust >= 1.82.
if ! rustup target list --installed | grep -qx "${WASM_TARGET}"; then
  log "Adding Rust target ${WASM_TARGET}"
  rustup target add "${WASM_TARGET}"
fi

# --- 2. Localnet -----------------------------------------------------------

if [ "$(docker inspect -f '{{.State.Running}}' "${CONTAINER}" 2>/dev/null)" = "true" ]; then
  log "Localnet container '${CONTAINER}' is already running"
else
  docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
  log "Starting localnet container '${CONTAINER}'"
  docker run -d --name "${CONTAINER}" \
    -p 8000:8000 \
    stellar/quickstart:latest --local --enable-soroban-rpc >/dev/null
fi

log "Waiting for Soroban RPC on ${RPC_URL}"
for _ in $(seq 1 90); do
  if curl -fsS -X POST "${RPC_URL}" \
      -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null | grep -q healthy; then
    log "RPC is healthy"
    break
  fi
  sleep 2
done

curl -fsS -X POST "${RPC_URL}" -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null | grep -q healthy \
  || die "RPC did not become healthy. Check: docker logs ${CONTAINER}"

# --- 3. Network and identity --------------------------------------------------

log "Registering network '${NETWORK_NAME}' with the CLI"
stellar network add "${NETWORK_NAME}" \
  --rpc-url "${RPC_URL}" \
  --network-passphrase "${NETWORK_PASSPHRASE}" \
  --overwrite >/dev/null 2>&1 || \
stellar network add "${NETWORK_NAME}" \
  --rpc-url "${RPC_URL}" \
  --network-passphrase "${NETWORK_PASSPHRASE}" >/dev/null 2>&1 || true

if ! stellar keys address "${IDENTITY}" >/dev/null 2>&1; then
  log "Generating identity '${IDENTITY}'"
  stellar keys generate "${IDENTITY}" --network "${NETWORK_NAME}" --fund
else
  log "Identity '${IDENTITY}' already exists; funding it"
  stellar keys fund "${IDENTITY}" --network "${NETWORK_NAME}" >/dev/null 2>&1 || true
fi

PUBLIC_KEY="$(stellar keys address "${IDENTITY}")"
log "Using account ${PUBLIC_KEY}"

# --- 4. Build ---------------------------------------------------------------

log "Building ${CONTRACT} for ${WASM_TARGET}"
if ! cargo build --release --target "${WASM_TARGET}" -p "${CONTRACT}"; then
  die "Build failed for '${CONTRACT}'.
The 'counter' crate does not currently compile on a clean checkout (pre-existing,
unrelated to the SDK). Run without CONTRACT set to deploy 'soroban-ping' instead,
and see docs/LOCALNET.md for details."
fi

[ -f "${WASM_PATH}" ] || die "Expected wasm at ${WASM_PATH} but it was not produced."

# --- 5. Deploy --------------------------------------------------------------

log "Deploying ${WASM_NAME}"
CONTRACT_ID="$(stellar contract deploy \
  --wasm "${WASM_PATH}" \
  --source "${IDENTITY}" \
  --network "${NETWORK_NAME}")"

[ -n "${CONTRACT_ID}" ] || die "Deploy did not return a contract ID."

# --- 6. Report --------------------------------------------------------------

cat <<EOF

$(log "Deployed successfully")

  Contract: ${CONTRACT}
  ID:       ${CONTRACT_ID}
  Account:  ${PUBLIC_KEY}

Add the following to examples/swap-demo/.env.local (git-ignored):

  VITE_RPC_URL=${RPC_URL}
  VITE_NETWORK_PASSPHRASE=${NETWORK_PASSPHRASE}
  VITE_CONTRACT_ID=${CONTRACT_ID}
  VITE_PUBLIC_KEY=${PUBLIC_KEY}

None of those is a secret. The demo signs through a browser wallet, so install
a Stellar wallet extension and import '${IDENTITY}' there if you want to run
the write path in the browser:

  stellar keys show ${IDENTITY}      # then paste into the wallet, not into .env

To exercise the SDK's signing path without a wallet, use the Node script, which
keeps the key in the process and out of any bundle:

  npm run localnet:verify -- --contract ${CONTRACT_ID} --secret "\$(stellar keys show ${IDENTITY})"

Then start the demo:

  npm run demo

To stop the localnet:

  docker rm -f ${CONTAINER}
EOF
