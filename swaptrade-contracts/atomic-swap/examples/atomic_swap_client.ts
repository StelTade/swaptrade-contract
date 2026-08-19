/**
 * Atomic Swap Client — TypeScript example
 *
 * Demonstrates the full create → fund → accept cycle for a Soroban
 * atomic swap contract on localnet (standalone network).
 *
 * Prerequisites:
 *   - soroban-cli installed (`stellar`)
 *   - localnet running (`stellar network start standalone`)
 *   - Contract deployed (see README)
 *
 * Run:
 *   npx tsx examples/atomic_swap_client.ts
 */

import {
  Contract,
  Keypair,
  Networks,
  SorobanRpc,
  TransactionBuilder,
  xdr,
  Address,
  nativeToScVal,
  scValToNative,
} from "@stellar/stellar-sdk";

// ── Configuration ────────────────────────────────────────────
const RPC_URL = "http://localhost:8000/soroban/rpc";
const NETWORK_PASSPHRASE = Networks.STANDALONE;

// Replace with your deployed contract ID (hex or C… address)
const SWAP_CONTRACT_ID = process.env.SWAP_CONTRACT_ID ?? "";

// Replace with deployed Stellar Asset Contract addresses
const ASSET_A_ID = process.env.ASSET_A_ID ?? "";
const ASSET_B_ID = process.env.ASSET_B_ID ?? "";

// ── Helpers ──────────────────────────────────────────────────

const server = new SorobanRpc.Server(RPC_URL);

async function sendTx(txn: string, signer: Keypair): Promise<string> {
  const tx = TransactionBuilder.fromXDR(txn, NETWORK_PASSPHRASE);
  tx.sign(signer);
  const result = await server.sendTransaction(tx);
  if (result.status === "ERROR") {
    throw new Error(`Transaction failed: ${JSON.stringify(result.errorResult)}`);
  }
  // Wait for confirmation
  const hash = result.hash;
  for (let i = 0; i < 10; i++) {
    const get = await server.getTransaction(hash);
    if (get.status === "SUCCESS") return hash;
    if (get.status === "FAILED") throw new Error("Transaction failed on-chain");
    await new Promise((r) => setTimeout(r, 1000));
  }
  throw new Error("Transaction timed out");
}

function toAddress(address: string): xdr.ScVal {
  return nativeToScVal(address, { type: "address" });
}

function toU64(n: number): xdr.ScVal {
  return nativeToScVal(n, { type: "u64" });
}

function toI128(n: number): xdr.ScVal {
  return nativeToScVal(n, { type: "i128" });
}

// ── Contract client functions ────────────────────────────────

/**
 * Build a create_swap transaction (does NOT send it).
 */
async function buildCreateSwap(
  contractId: string,
  creator: Keypair,
  counterparty: string,
  assetA: string,
  amountA: number,
  assetB: string,
  amountB: number,
  expiry: number,
  nonce: number,
): Promise<string> {
  const contract = new Contract(contractId);
  const account = await server.getAccount(creator.publicKey());
  const txn = new TransactionBuilder(account, {
    fee: "100000",
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      contract.call(
        "create_swap",
        toAddress(creator.publicKey()),
        toAddress(counterparty),
        toAddress(assetA),
        toI128(amountA),
        toAddress(assetB),
        toI128(amountB),
        toU64(expiry),
        toU64(nonce),
      ),
    )
    .setTimeout(300)
    .build();

  return txn.toXDR();
}

/**
 * Build a fund_swap transaction.
 */
async function buildFundSwap(
  contractId: string,
  funder: Keypair,
  swapId: number,
): Promise<string> {
  const contract = new Contract(contractId);
  const account = await server.getAccount(funder.publicKey());
  const txn = new TransactionBuilder(account, {
    fee: "100000",
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call("fund_swap", toU64(swapId), toAddress(funder.publicKey())))
    .setTimeout(300)
    .build();

  return txn.toXDR();
}

/**
 * Build an accept_swap transaction.
 */
async function buildAcceptSwap(
  contractId: string,
  acceptor: Keypair,
  swapId: number,
): Promise<string> {
  const contract = new Contract(contractId);
  const account = await server.getAccount(acceptor.publicKey());
  const txn = new TransactionBuilder(account, {
    fee: "100000",
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      contract.call("accept_swap", toU64(swapId), toAddress(acceptor.publicKey())),
    )
    .setTimeout(300)
    .build();

  return txn.toXDR();
}

/**
 * Build a cancel_swap transaction.
 */
async function buildCancelSwap(
  contractId: string,
  creator: Keypair,
  swapId: number,
): Promise<string> {
  const contract = new Contract(contractId);
  const account = await server.getAccount(creator.publicKey());
  const txn = new TransactionBuilder(account, {
    fee: "100000",
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call("cancel_swap", toU64(swapId)))
    .setTimeout(300)
    .build();

  return txn.toXDR();
}

/**
 * Build a refund_swap transaction.
 */
async function buildRefundSwap(
  contractId: string,
  creator: Keypair,
  swapId: number,
): Promise<string> {
  const contract = new Contract(contractId);
  const account = await server.getAccount(creator.publicKey());
  const txn = new TransactionBuilder(account, {
    fee: "100000",
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call("refund_swap", toU64(swapId)))
    .setTimeout(300)
    .build();

  return txn.toXDR();
}

// ── Main: Full swap cycle ────────────────────────────────────

async function main() {
  console.log("═══════════════════════════════════════════════════");
  console.log("  Soroban Atomic Swap — Full Lifecycle Demo");
  console.log("═══════════════════════════════════════════════════\n");

  // Generate deterministic keypairs for demo
  const creator = Keypair.random();
  const counterparty = Keypair.random();

  console.log(`Creator:       ${creator.publicKey()}`);
  console.log(`Counterparty:  ${counterparty.publicKey()}`);
  console.log(`Swap Contract: ${SWAP_CONTRACT_ID}\n`);

  // In a real scenario, you'd fund these accounts on the localnet
  // using `stellar keys fund <address> --network standalone`
  // For this demo, assume accounts are already funded.

  // ── Step 1: Create Swap ──────────────────────────────────
  const now = Math.floor(Date.now() / 1000);
  const expiry = now + 3600; // 1 hour from now
  const nonce = 1;

  console.log("Step 1: Creating swap offer...");
  const createTxn = await buildCreateSwap(
    SWAP_CONTRACT_ID,
    creator,
    counterparty.publicKey(),
    ASSET_A_ID,
    100,
    ASSET_B_ID,
    200,
    expiry,
    nonce,
  );
  const createHash = await sendTxn(createTxn, creator);
  console.log(`  ✓ Swap created (tx: ${createHash})\n`);

  // ── Step 2: Creator funds side A ─────────────────────────
  console.log("Step 2: Creator funding side A...");
  const fundATxn = await buildFundSwap(SWAP_CONTRACT_ID, creator, 1);
  const fundAHash = await sendTxn(fundATxn, creator);
  console.log(`  ✓ Side A funded (tx: ${fundAHash})\n`);

  // ── Step 3: Counterparty funds side B ────────────────────
  console.log("Step 3: Counterparty funding side B...");
  const fundBTxn = await buildFundSwap(SWAP_CONTRACT_ID, counterparty, 1);
  const fundBHash = await sendTxn(fundBTxn, counterparty);
  console.log(`  ✓ Side B funded (tx: ${fundBHash})\n`);

  // ── Step 4: Counterparty accepts (atomic execution) ──────
  console.log("Step 4: Counterparty accepting swap (atomic transfer)...");
  const acceptTxn = await buildAcceptSwap(SWAP_CONTRACT_ID, counterparty, 1);
  const acceptHash = await sendTxn(acceptTxn, counterparty);
  console.log(`  ✓ Swap accepted — assets transferred atomically (tx: ${acceptHash})\n`);

  console.log("═══════════════════════════════════════════════════");
  console.log("  ✅ Full atomic swap lifecycle completed!");
  console.log("═══════════════════════════════════════════════════\n");

  // ── Demo: Cancel flow ────────────────────────────────────
  console.log("Bonus: Demonstrating cancel flow...");
  const cancelTxn = await buildCreateSwap(
    SWAP_CONTRACT_ID,
    creator,
    counterparty.publicKey(),
    ASSET_A_ID,
    50,
    ASSET_B_ID,
    75,
    expiry,
    nonce + 1,
  );
  await sendTxn(cancelTxn, creator);
  console.log("  ✓ Second swap created");

  const cancelCall = await buildCancelSwap(SWAP_CONTRACT_ID, creator, 2);
  await sendTxn(cancelCall, creator);
  console.log("  ✓ Swap cancelled before funding\n");
}

main().catch(console.error);
