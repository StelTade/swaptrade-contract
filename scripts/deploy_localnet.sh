#!/bin/bash

# Localnet Deployment Script for SwapTrade Contracts
#
# This script deploys all SwapTrade contracts to a Soroban localnet instance.
# Prerequisites:
#   - Stellar quickstart node running (docker run --rm -it -p 8000:8000 stellar/quickstart:soroban-dev)
#   - soroban-cli installed
#   - Rust toolchain with wasm32-unknown-unknown target
#
# Usage: ./scripts/deploy_localnet.sh [--clean]

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SOROBAN_RPC_URL="${SOROBAN_RPC_URL:-http://localhost:8000/soroban/rpc}"
SOROBAN_NETWORK_PASSPHRASE="${SOROBAN_NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
IDENTITY_NAME="${IDENTITY_NAME:-deployer}"

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       SwapTrade Localnet Deployment Script                   ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Parse arguments
CLEAN=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --clean) CLEAN=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# Clean if requested
if [ "$CLEAN" = true ]; then
    echo -e "${YELLOW}Cleaning build artifacts...${NC}"
    cargo clean
fi

# Step 1: Build contracts
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Step 1: Building contracts for WASM target${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

rustup target add wasm32-unknown-unknown 2>/dev/null || true

echo "Building counter contract..."
cargo build \
    --manifest-path swaptrade-contracts/counter/Cargo.toml \
    --target wasm32-unknown-unknown \
    --release

echo "Building soroban-ping contract..."
cargo build \
    --manifest-path swaptrade-contracts/soroban-ping/Cargo.toml \
    --target wasm32-unknown-unknown \
    --release

echo -e "${GREEN}✓ Contracts built successfully${NC}"
echo ""

# Step 2: Check localnet connectivity
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Step 2: Checking localnet connectivity${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

if curl -s "$SOROBAN_RPC_URL" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Localnet is reachable at $SOROBAN_RPC_URL${NC}"
else
    echo -e "${RED}✗ Cannot reach localnet at $SOROBAN_RPC_URL${NC}"
    echo -e "${YELLOW}Start localnet with:${NC}"
    echo "  docker run --rm -it -p 8000:8000 stellar/quickstart:soroban-dev"
    exit 1
fi
echo ""

# Step 3: Generate or use existing identity
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Step 3: Setting up deployer identity${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

if soroban keys ls 2>/dev/null | grep -q "$IDENTITY_NAME"; then
    echo -e "${GREEN}✓ Identity '$IDENTITY_NAME' already exists${NC}"
else
    echo "Generating deployer identity..."
    soroban keys generate --network standalone --output json "$IDENTITY_NAME"
    echo -e "${GREEN}✓ Identity '$IDENTITY_NAME' generated${NC}"
fi
echo ""

# Step 4: Deploy contracts
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Step 4: Deploying contracts${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

COUNTER_WASM="target/wasm32-unknown-unknown/release/counter.wasm"
PING_WASM="target/wasm32-unknown-unknown/release/soroban_ping.wasm"

DEPLOYED_CONTRACTS=""

# Deploy counter contract
if [ -f "$COUNTER_WASM" ]; then
    echo "Deploying counter contract..."
    COUNTER_ID=$(soroban contract deploy \
        --wasm "$COUNTER_WASM" \
        --network standalone \
        --source "$IDENTITY_NAME" 2>&1)
    echo -e "${GREEN}✓ Counter contract deployed: $COUNTER_ID${NC}"
    DEPLOYED_CONTRACTS="$DEPLOYED_CONTRACTS\ncounter: $COUNTER_ID"
else
    echo -e "${YELLOW}⚠ Counter WASM not found at $COUNTER_WASM, skipping${NC}"
fi

# Deploy ping contract
if [ -f "$PING_WASM" ]; then
    echo "Deploying soroban-ping contract..."
    PING_ID=$(soroban contract deploy \
        --wasm "$PING_WASM" \
        --network standalone \
        --source "$IDENTITY_NAME" 2>&1)
    echo -e "${GREEN}✓ Soroban-ping contract deployed: $PING_ID${NC}"
    DEPLOYED_CONTRACTS="$DEPLOYED_CONTRACTS\nsoroban-ping: $PING_ID"
else
    echo -e "${YELLOW}⚠ soroban-ping WASM not found at $PING_WASM, skipping${NC}"
fi
echo ""

# Step 5: Initialize contracts
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Step 5: Initializing contracts${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

if [ -n "${COUNTER_ID:-}" ]; then
    echo "Initializing counter contract..."
    soroban contract invoke \
        --id "$COUNTER_ID" \
        --network standalone \
        --source "$IDENTITY_NAME" \
        -- initialize --admin "$(soroban keys address "$IDENTITY_NAME")" 2>&1 || \
        echo -e "${YELLOW}⚠ Counter initialization skipped (may already be initialized)${NC}"
fi

echo -e "${GREEN}✓ Contract initialization complete${NC}"
echo ""

# Summary
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    DEPLOYMENT SUMMARY                        ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "Network:     ${GREEN}Standalone Localnet${NC}"
echo -e "RPC URL:     ${GREEN}$SOROBAN_RPC_URL${NC}"
echo -e "Deployer:    ${GREEN}$IDENTITY_NAME${NC}"
echo ""
echo -e "${YELLOW}Deployed Contracts:${NC}"
echo -e "$DEPLOYED_CONTRACTS"
echo ""
echo -e "${GREEN}Deployment complete!${NC}"
echo ""
echo -e "${YELLOW}Useful commands:${NC}"
echo "  soroban contract invoke --id <CONTRACT_ID> --network standalone -- <function> <args>"
echo "  soroban contract read --id <CONTRACT_ID> --network standalone"
echo ""
