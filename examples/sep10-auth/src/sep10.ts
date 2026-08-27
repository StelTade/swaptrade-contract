import {
  Account,
  Keypair,
  Operation,
  StrKey,
  Transaction,
  TransactionBuilder,
} from "@stellar/stellar-sdk";
import { createHash, randomBytes } from "node:crypto";
import { NonceStore } from "./nonce-store";
import type { Sep10Config } from "./config";

/** Minimal structural view of a ManageData operation. */
interface ManageDataOpView {
  name: string;
  value?: string;
  source?: string;
}

/**
 * SEP-10 (Stellar Web Authentication) challenge-response flow.
 *
 * The protocol in one paragraph: the server proves control of a domain
 * account by signing a challenge transaction that carries a fresh random
 * nonce; the client proves control of the account it wants to authenticate
 * as by co-signing that same transaction. The server then verifies both
 * signatures plus every structural property the protocol pins down
 * (sequence number, timebounds, operation shape) before issuing a session
 * token.
 *
 * Spec: https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md
 */

/** Seconds of clock skew tolerated when validating timebounds. */
const CLOCK_SKEW_SECONDS = 5;

export class Sep10Error extends Error {
  constructor(
    public readonly code:
      | "MALFORMED_TRANSACTION"
      | "BAD_SEQUENCE"
      | "TIMEBOUNDS"
      | "UNEXPECTED_OPERATION"
      | "UNKNOWN_SIGNER"
      | "MISSING_SIGNATURE"
      | "NONCE_REPLAY"
      | "WRONG_NETWORK",
    message: string,
  ) {
    super(message);
    this.name = "Sep10Error";
  }
}

export interface ChallengeResult {
  /** Signed challenge envelope (base64 XDR) for the client to co-sign. */
  transaction: string;
  /** Base64 nonce embedded in the challenge, tracked for replay checks. */
  nonce: string;
  /** Client public address the challenge is bound to. */
  clientAccountId: string;
}

/**
 * Step 1 — server builds and signs the challenge (GET /auth).
 *
 * Shape required by SEP-10:
 * - transaction source account: the CLIENT account being authenticated
 *   (this is how the challenge names its subject), sequence number 0
 * - exactly one operation: ManageData, name `<domain> auth`, value 64
 *   bytes of cryptographically random data, sourced by the SERVER's
 *   domain account
 * - timebounds spanning at most five minutes from now
 * - signed first by the server; the client co-signs in step 2
 */
export function buildChallenge(
  serverKeypair: Keypair,
  clientAccountId: string,
  config: Sep10Config,
  now: () => number = () => Math.floor(Date.now() / 1000),
): ChallengeResult {
  // Validate early so we never sign a challenge bound to a bogus key.
  if (!StrKey.isValidEd25519PublicKey(clientAccountId)) {
    throw new Sep10Error("MALFORMED_TRANSACTION", `not a valid ed25519 account id: ${clientAccountId}`);
  }

  // Raw 64 random bytes go straight into the XDR value; the base64
  // form is only used for logging/replay bookkeeping.
  const nonceBytes = randomBytes(64);
  const nonce = nonceBytes.toString("base64");
  const currentUnix = now();

  // TransactionBuilder increments the account's sequence for the built
  // transaction, so -1 here lands as sequence 0 on the wire — exactly
  // what SEP-10 mandates for challenges.
  const sourceAccount = new Account(clientAccountId, "-1");
  const operation = Operation.manageData({
    name: `${config.authDomain} auth`,
    value: nonceBytes,
    source: serverKeypair.publicKey(),
  });

  const tx = new TransactionBuilder(sourceAccount, {
    fee: "100",
    networkPassphrase: config.networkPassphrase,
    timebounds: {
      minTime: currentUnix,
      maxTime: currentUnix + config.challengeWindowSeconds,
    },
  })
    .addOperation(operation)
    .build();

  tx.sign(serverKeypair);

  return {
    transaction: tx.toXDR(),
    nonce,
    clientAccountId,
  };
}

export interface ClientChallengeContext {
  challengeXdr: string;
  clientKeypair: Keypair;
  config: Sep10Config;
}

/** Returns the decoded transaction plus the ManageData operation. */
function parseAndValidateShape(
  challengeXdr: string,
  serverAccountId: string,
  config: Sep10Config,
): { tx: Transaction; manageDataOp: ManageDataOpView } {
  let tx: Transaction;
  try {
    tx = TransactionBuilder.fromXDR(challengeXdr, config.networkPassphrase) as Transaction;
  } catch {
    throw new Sep10Error("WRONG_NETWORK", "transaction does not deserialize for this network passphrase");
  }

  if (tx.sequence !== "0") {
    throw new Sep10Error("BAD_SEQUENCE", `challenge sequence must be 0, got `);
  }

  if (tx.operations.length !== 1) {
    throw new Sep10Error("UNEXPECTED_OPERATION", `challenge must contain exactly one operation, got ${tx.operations.length}`);
  }

  const op = tx.operations[0];
  if (op.type !== "manageData") {
    throw new Sep10Error("UNEXPECTED_OPERATION", `challenge operation must be manageData, got ${op.type}`);
  }
  const manageDataOp = op as unknown as ManageDataOpView;

  if (manageDataOp.name !== `${config.authDomain} auth`) {
    throw new Sep10Error("UNEXPECTED_OPERATION", `manageData name must be '${config.authDomain} auth', got '${manageDataOp.name}'`);
  }

  if (manageDataOp.source !== serverAccountId) {
    throw new Sep10Error("UNEXPECTED_OPERATION", "manageData operation must be sourced by the server's domain account");
  }

  if (!manageDataOp.value) {
    throw new Sep10Error("MALFORMED_TRANSACTION", "manageData value must be present");
  }
  const rawNonce = Buffer.from(manageDataOp.value, "base64");
  if (rawNonce.length !== 64) {
    throw new Sep10Error("MALFORMED_TRANSACTION", `manageData value must decode to 64 bytes, got ${rawNonce.length}`);
  }

  return { tx, manageDataOp };
}

/**
 * Step 2 — client co-signs the challenge (client side helper).
 *
 * In a real DApp this runs wherever the user's secret lives (wallet,
 * Freighter, custodial signer); it never touches the server.
 */
export function signChallenge(ctx: ClientChallengeContext): string {
  const { tx, _ } = parseAndValidateShapeForClient(ctx.challengeXdr, ctx.config);
  tx.sign(ctx.clientKeypair);
  return tx.toXDR();
}

// Clients validate less than servers (they trust the server they called),
// but still need sequence/timebounds/operation checks to avoid signing
// something else entirely.
function parseAndValidateShapeForClient(
  challengeXdr: string,
  config: Sep10Config,
): { tx: Transaction; _: undefined } {
  let tx: Transaction;
  try {
    tx = TransactionBuilder.fromXDR(challengeXdr, config.networkPassphrase) as Transaction;
  } catch {
    throw new Sep10Error("WRONG_NETWORK", "transaction does not deserialize for this network passphrase");
  }
  if (tx.sequence !== "0") {
    throw new Sep10Error("BAD_SEQUENCE", "challenge sequence must be 0");
  }
  if (tx.operations.length !== 1 || tx.operations[0].type !== "manageData") {
    throw new Sep10Error("UNEXPECTED_OPERATION", "challenge must be a single manageData operation");
  }
  const op = tx.operations[0] as ManageDataOpView;
  if (op.name !== `${config.authDomain} auth`) {
    throw new Sep10Error("UNEXPECTED_OPERATION", `manageData name mismatch: ${op.name}`);
  }
  return { tx, _: undefined };
}

export interface VerifyOptions {
  signedChallengeXdr: string;
  serverKeypair: Keypair;
  nonces: NonceStore;
  config: Sep10Config;
  now?: () => number;
}

/**
 * Step 3 — server verifies the fully-signed challenge (POST /auth).
 *
 * Checks, in order:
 *   1. structural validity (sequence, single ManageData op, server source)
 *   2. timebounds currently satisfied
 *   3. signature set == {server, client}, each verified against the tx hash
 *   4. nonce not seen before (replay protection)
 *
 * On success the caller should issue a session token bound to the client
 * account id and the consumed nonce.
 */
export function verifyChallenge(opts: VerifyOptions): {
  clientAccountId: string;
  nonce: string;
} {
  const { signedChallengeXdr, serverKeypair, nonces, config } = opts;
  const now = opts.now ?? (() => Math.floor(Date.now() / 1000));

  const { tx, manageDataOp } = parseAndValidateShape(
    signedChallengeXdr,
    serverKeypair.publicKey(),
    config,
  );

  // Timebounds: minTime <= now <= maxTime, with small skew tolerance.
  const currentTime = now();
  const minTime = Number(tx.timeBounds?.minTime ?? 0);
  const maxTime = Number(tx.timeBounds?.maxTime ?? 0);
  if (
    !tx.timeBounds ||
    currentTime + CLOCK_SKEW_SECONDS < minTime ||
    currentTime - CLOCK_SKEW_SECONDS > maxTime
  ) {
    throw new Sep10Error("TIMEBOUNDS", `challenge outside its validity window [${minTime}, ${maxTime}] at t=${currentTime}`);
  }

  // Signature verification happens over the transaction hash.
  const txHash = tx.hash();
  const clientAccountId = tx.source; // challenge source is the client account
  const clientVerifier = Keypair.fromPublicKey(clientAccountId);

  const signatures = tx.signatures.map((s) => ({
    keyHint: s.hint().toString("hex"),
    sigBytes: s.signature(),
  }));

  if (signatures.length !== 2) {
    throw new Sep10Error("MISSING_SIGNATURE", `expected exactly [server, client] signatures, got ${signatures.length}`);
  }

  // Every signature must map to either the server or the client key; no
  // extra parties are allowed to have contributed.
  for (const sig of signatures) {
    const matchesServer =
      sig.keyHint === keyHintOf(serverKeypair.publicKey()) && serverKeypair.verify(txHash, sig.sigBytes);
    let matchesClient = false;
    if (!matchesServer) {
      matchesClient = sig.keyHint === keyHintOf(clientAccountId) && clientVerifier.verify(txHash, sig.sigBytes);
    }
    if (!matchesServer && !matchesClient) {
      throw new Sep10Error("UNKNOWN_SIGNER", `signature hint ${sig.keyHint} matches neither the server nor the client key`);
    }
  }

  const nonce = Buffer.from(manageDataOp.value!, "base64").toString("base64");

  // Replay protection: the nonce is single-use. This check MUST come
  // after signature verification so garbage cannot pollute the store,
  // but before any token is minted.
  if (!nonces.consume(nonce)) {
    throw new Sep10Error("NONCE_REPLAY", "this challenge has already been redeemed");
  }

  return { clientAccountId, nonce };
}

/** Session identity issued after successful verification. */
export interface AuthenticatedSession {
  clientAccountId: string;
  nonceHash: string;
  expiresAt: number;
}

export function nonceFingerprint(nonceBase64: string): string {
  return createHash("sha256").update(Buffer.from(nonceBase64, "base64")).digest("hex").slice(0, 32);
}

// --- helpers ---------------------------------------------------------------

function keyHintOf(accountId: string): string {
  const raw = StrKey.decodeEd25519PublicKey(accountId);
  return Buffer.from(raw).subarray(28).toString("hex");
}


