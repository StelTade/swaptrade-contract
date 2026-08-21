# SwapTrade SDK

A lightweight TypeScript/JavaScript SDK for interacting with SwapTrade Soroban atomic swap contracts on the Stellar network.

## Features

- **Typed Interfaces**: Full TypeScript support with comprehensive type definitions
- **Transaction Helpers**: Build, sign, and submit transactions with ease
- **Account Management**: Helper functions for keypair generation and validation
- **Best Practices**: Built-in validation, error handling, and gas optimization
- **Network Support**: Works with localnet, testnet, and mainnet

## Installation

```bash
npm install @swaptrade/sdk
```

Or with yarn:

```bash
yarn add @swaptrade/sdk
```

## Quick Start

```typescript
import { AtomicSwapClient, Keypair } from "@swaptrade/sdk";

// Initialize the client
const client = new AtomicSwapClient({
  contractId: "YOUR_CONTRACT_ID",
  network: {
    rpcUrl: "https://rpc-futurenet.stellar.org",
    networkPassphrase: "Test SDF Network ; September 2015",
  },
});

// Create a keypair
const creator = Keypair.random();
const counterparty = Keypair.random();

// Create a swap
const result = await client.createSwap(
  creator,
  counterparty.publicKey(),
  "ASSET_A_CONTRACT_ID",
  100,
  "ASSET_B_CONTRACT_ID",
  200,
  3600, // 1 hour expiry
);

console.log(`Swap created: ${result.swapId}`);
```

## API Reference

### AtomicSwapClient

The main class for interacting with the atomic swap contract.

#### Constructor

```typescript
constructor(config: SwapTradeSDKConfig)
```

**Parameters:**
- `contractId`: The deployed contract ID
- `network`: Network configuration with `rpcUrl` and `networkPassphrase`

#### Methods

##### `buildCreateSwap(params, signerPublicKey)`

Build a create_swap transaction without signing.

```typescript
const xdrTx = await client.buildCreateSwap(
  {
    creator: "G...",
    counterparty: "G...",
    asset_a: "C...",
    amount_a: 100,
    asset_b: "C...",
    amount_b: 200,
    expiry: 1234567890,
    nonce: 1,
  },
  signerPublicKey
);
```

##### `buildFundSwap(params, signerPublicKey)`

Build a fund_swap transaction.

```typescript
const xdrTx = await client.buildFundSwap(
  { swap_id: 1, funder: "G..." },
  signerPublicKey
);
```

##### `buildAcceptSwap(params, signerPublicKey)`

Build an accept_swap transaction.

```typescript
const xdrTx = await client.buildAcceptSwap(
  { swap_id: 1, acceptor: "G..." },
  signerPublicKey
);
```

##### `buildCancelSwap(params, signerPublicKey)`

Build a cancel_swap transaction.

```typescript
const xdrTx = await client.buildCancelSwap(
  { swap_id: 1 },
  signerPublicKey
);
```

##### `buildRefundSwap(params, signerPublicKey)`

Build a refund_swap transaction.

```typescript
const xdrTx = await client.buildRefundSwap(
  { swap_id: 1 },
  signerPublicKey
);
```

##### `signAndSubmitTransaction(xdrTx, signer)`

Sign and submit a transaction to the network.

```typescript
const result = await client.signAndSubmitTransaction(xdrTx, keypair);
console.log(`Transaction hash: ${result.hash}`);
```

##### `getSwap(swapId)`

Fetch swap details from the contract.

```typescript
const swap = await client.getSwap(1);
console.log(`Swap state: ${swap.state}`);
```

##### `checkTrustline(address, asset)`

Check if an address has a trustline for an asset.

```typescript
const hasTrustline = await client.checkTrustline("G...", "C...");
```

##### `getMinExpiry()`

Get the minimum expiry window configured in the contract.

```typescript
const minExpiry = await client.getMinExpiry();
```

### Helper Functions

#### `toAddress(address)`

Convert a string address to ScVal.

#### `toU64(n)`

Convert a number to u64 ScVal.

#### `toI128(n)`

Convert a number or bigint to i128 ScVal.

#### `calculateExpiry(secondsFromNow)`

Calculate expiry timestamp (seconds from epoch).

#### `generateNonce()`

Generate a random nonce for idempotency.

#### `isValidAddress(address)`

Validate Stellar address format.

#### `isValidAmount(amount)`

Validate amount is positive.

#### `isValidExpiry(expiry, minExpirySeconds)`

Validate expiry is in the future.

## Network Configuration

### Localnet (Standalone)

```typescript
const client = new AtomicSwapClient({
  contractId: "YOUR_CONTRACT_ID",
  network: {
    rpcUrl: "http://localhost:8000/soroban/rpc",
    networkPassphrase: "Standalone Network ; February 2017",
  },
});
```

### Testnet (Futurenet)

```typescript
const client = new AtomicSwapClient({
  contractId: "YOUR_CONTRACT_ID",
  network: {
    rpcUrl: "https://rpc-futurenet.stellar.org",
    networkPassphrase: "Test SDF Network ; September 2015",
  },
});
```

### Mainnet

```typescript
const client = new AtomicSwapClient({
  contractId: "YOUR_CONTRACT_ID",
  network: {
    rpcUrl: "https://rpc.mainnet.stellar.org",
    networkPassphrase: "Public Global Stellar Network ; September 2015",
  },
});
```

## Complete Example

```typescript
import { AtomicSwapClient, Keypair } from "@swaptrade/sdk";

async function fullSwapCycle() {
  // Initialize client
  const client = new AtomicSwapClient({
    contractId: process.env.SWAP_CONTRACT_ID!,
    network: {
      rpcUrl: "http://localhost:8000/soroban/rpc",
      networkPassphrase: "Standalone Network ; February 2017",
    },
  });

  // Generate keypairs (in production, use existing wallets)
  const creator = Keypair.random();
  const counterparty = Keypair.random();

  // Fund accounts on localnet (using stellar-cli)
  // stellar keys fund <address> --network standalone

  // Create swap
  const { swapId, txHash } = await client.createSwap(
    creator,
    counterparty.publicKey(),
    process.env.ASSET_A_ID!,
    100,
    process.env.ASSET_B_ID!,
    200,
    3600,
  );
  console.log(`Swap created: ${swapId}`);

  // Fund creator's side
  const fundA = await client.buildFundSwap(
    { swap_id: swapId, funder: creator.publicKey() },
    creator.publicKey(),
  );
  await client.signAndSubmitTransaction(fundA, creator);
  console.log("Creator funded");

  // Fund counterparty's side
  const fundB = await client.buildFundSwap(
    { swap_id: swapId, funder: counterparty.publicKey() },
    counterparty.publicKey(),
  );
  await client.signAndSubmitTransaction(fundB, counterparty);
  console.log("Counterparty funded");

  // Accept swap (atomic execution)
  const accept = await client.buildAcceptSwap(
    { swap_id: swapId, acceptor: counterparty.publicKey() },
    counterparty.publicKey(),
  );
  await client.signAndSubmitTransaction(accept, counterparty);
  console.log("Swap accepted - assets transferred atomically");
}

fullSwapCycle().catch(console.error);
```

## Development

```bash
# Install dependencies
npm install

# Build
npm run build

# Watch mode
npm run watch
```

## License

MIT
