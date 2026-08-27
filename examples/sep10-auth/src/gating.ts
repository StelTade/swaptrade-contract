/**
 * Gating Soroban contract invocations behind SEP-10 authentication.
 *
 * Two patterns are demonstrated:
 *
 * 1. **Session gate** — the DApp backend only relays contract calls for
 *    requests carrying a valid SEP-10 session token. The user's Stellar
 *    account is passed through as the `source_account` / signer of the
 *    Soroban authorization entry, so on-chain authorization remains
 *    anchored to the authenticated identity.
 *
 * 2. **Relayer with signed intents** — for fee-sponsored flows the client
 *    signs an *intent* payload (contract id + function + args + nonce)
 *    with their Stellar ed25519 key. The relayer verifies the signature,
 *    enforces single-use nonces (replay protection) and submits the
 *    invocation on behalf of the user.
 */
import nacl from "tweetnacl";
import {
  ReplayGuard,
  SessionToken,
  Challenge,
  decodeStrKey,
  issueSessionToken,
  verifyChallenge,
  verifySessionToken,
} from "./sep10.js";

export interface ContractIntent {
  contractId: string;
  functionName: string;
  args: unknown[];
  nonce: number;
}

export interface GateResult {
  allowed: boolean;
  reason?: string;
  account?: string;
}

export class Sep10Gate {
  private readonly nonces = new ReplayGuard();
  private readonly usedIntents = new ReplayGuard();

  constructor(
    private readonly serverPublicKeyRaw: Uint8Array,
    private readonly serverSecretHex: string,
    private readonly nowSeconds: () => number = () => Math.floor(Date.now() / 1000),
  ) {}

  /** Step 1 of the flow: verify a completed challenge and mint a session token. */
  completeChallenge(
    challenge: Challenge,
    serverSignatureB64: string,
    clientSignatureB64: string,
    ttlSeconds = 3600,
  ): GateResult & { token?: SessionToken } {
    if (
      !verifyChallenge(
        challenge,
        serverSignatureB64,
        clientSignatureB64,
        this.serverPublicKeyRaw,
        this.nowSeconds(),
      )
    ) {
      return { allowed: false, reason: "challenge verification failed" };
    }
    const replayKey = `challenge:${challenge.nonceB64}`;
    if (!this.nonces.consume(replayKey)) {
      return { allowed: false, reason: "challenge replay detected" };
    }
    const token = issueSessionToken(challenge.clientAccount, this.serverSecretHex, ttlSeconds, this.nowSeconds());
    return { allowed: true, account: challenge.clientAccount, token };
  }

  /** Step 2a: gate any contract-invoking operation on a session token. */
  authorizeWithToken(token: SessionToken, intent: ContractIntent): GateResult {
    if (!verifySessionToken(token, this.serverSecretHex, this.nowSeconds())) {
      return { allowed: false, reason: "invalid or expired session token" };
    }
    return this.checkIntentReplay(token.account, intent);
  }

  /**
   * Step 2b: fee-sponsored flow — the client signs the canonical intent
   * payload with their Stellar secret key; the relayer validates it against
   * the claimed account before submitting.
   */
  authorizeSignedIntent(accountId: string, intent: ContractIntent, signatureB64: string): GateResult {
    let clientKey: Buffer;
    try {
      clientKey = decodeStrKey(accountId);
    } catch {
      return { allowed: false, reason: "malformed client account" };
    }
    const message = Buffer.from(intentPayload(intent), "utf8");
    let ok = false;
    try {
      ok = nacl.sign.detached.verify(message, Buffer.from(signatureB64, "base64"), clientKey);
    } catch {
      ok = false;
    }
    if (!ok) {
      return { allowed: false, reason: "intent signature invalid" };
    }
    return this.checkIntentReplay(accountId, intent);
  }

  private checkIntentReplay(account: string, intent: ContractIntent): GateResult {
    const key =
      `intent:${account}:${intent.contractId}:${intent.functionName}:` +
      `${JSON.stringify(intent.args)}:${intent.nonce}`;
    if (!this.usedIntents.consume(key)) {
      return { allowed: false, reason: "intent replay detected" };
    }
    return { allowed: true, account };
  }
}

/** Canonical bytes a client signs for a fee-sponsored invocation. */
export function intentPayload(intent: ContractIntent): string {
  return [
    "swaptrade-intent-v1",
    intent.contractId,
    intent.functionName,
    JSON.stringify(intent.args),
    String(intent.nonce),
  ].join("\n");
}
