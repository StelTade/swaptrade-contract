import {
  Account,
  Keypair,
  Operation,
  SorobanRpc,
  StrKey,
  Transaction,
  TransactionBuilder,
  nativeToScVal,
} from "@stellar/stellar-sdk";
import type { AuthedRequest } from "./middleware";

/**
 * Gating Soroban contract invocations behind SEP-10 sessions.
 *
 * The pattern solves two problems at once:
 *
 * 1. Access control — only holders of a freshly verified SEP-10 session
 *    reach any endpoint that can move contract state.
 *
 * 2. Relayer safety — users rarely hold XLM for fees in DApps, so the
 *    backend often *submits* transactions on their behalf. A careless
 *    relayer becomes an arbitrary-execution oracle: whatever bytes the
 *    client posts get funded. The rule enforced here is that the relayer
 *    refuses to pay for anything the session owner did not sign, with
 *    the envelope's source account matching the SEP-10 subject exactly.
 */

export interface InvocationPlan {
  /** Contract ID (C... string) the call targets. */
  contractId: string;
  /** Function name inside the contract. */
  functionName: string;
  /** Arguments as JSON-compatible values; converted to ScVals below. */
  args: unknown[];
}

/** Operations a session may request. Keep this list tight. */
export interface InvocationPolicy {
  allowedContracts: Record<string, string[]>; // contractId -> allowed fns
  maxInvocationsPerSession: number;
}

export function defaultPolicy(atomicSwapContractId: string): InvocationPolicy {
  return {
    allowedContracts: {
      // Example: sessions may drive the atomic swap contract's user-side
      // entry points, never administrative functions.
      [atomicSwapContractId]: ["get_quote", "get_escrow", "lock_asset"],
    },
    maxInvocationsPerSession: 10,
  };
}

export class PolicyError extends Error {
  constructor(public readonly reason: "FORBIDDEN_TARGET" | "SESSION_QUOTA", message: string) {
    super(message);
    this.name = "PolicyError";
  }
}

/** Pure decision function — trivially unit-testable, no network involved. */
export function authorizeInvocation(
  auth: NonNullable<AuthedRequest["auth"]>,
  plan: InvocationPlan,
  policy: InvocationPolicy,
  usageCountFor: (jti: string) => number,
): void {
  const allowedFns = policy.allowedContracts[plan.contractId];
  if (!allowedFns || !allowedFns.includes(plan.functionName)) {
    throw new PolicyError("FORBIDDEN_TARGET", `${plan.functionName} on ${plan.contractId} is not permitted for sessions`);
  }
  const used = usageCountFor(auth.jti);
  if (used >= policy.maxInvocationsPerSession) {
    throw new PolicyError("SESSION_QUOTA", `session ${auth.jti} already performed ${used} invocations`);
  }
}

/**
 * Build an unsigned Soroban invocation whose source account is the
 * session's subject. The user signs this envelope wherever their key
 * material lives; the server never sees secrets.
 *
 * Sequence number 0 is a placeholder — re-fetch the real sequence from
 * the ledger right before submission.
 */
export function createInvocationForSigning(
  sessionSubjectAccountId: string,
  plan: InvocationPlan,
  networkPassphrase: string,
  now: () => number = () => Math.floor(Date.now() / 1000),
): string {
  const scArgs = plan.args.map((a) => nativeToScVal(a));
  const builder = new TransactionBuilder(new Account(sessionSubjectAccountId, "0"), {
    fee: "100",
    networkPassphrase,
    timebounds: { minTime: now(), maxTime: now() + 120 },
  });
  const tx = builder
    .addOperation(
      Operation.invokeContractFunction({
        contract: plan.contractId,
        function: plan.functionName,
        args: scArgs,
        source: sessionSubjectAccountId,
      }),
    )
    .build();
  return tx.toXDR();
}

export interface SubmissionCheckOptions {
  envelopeXdr: string;
  /** Must equal the SEP-10 session subject. */
  expectedSourceAccountId: string;
  networkPassphrase: string;
}

/**
 * The relayer's critical check: before funding submission, prove that
 * (a) the envelope's source account is the session subject and
 * (b) that account actually signed these exact operations.
 */
export function verifyUserAuthorization(
  opts: SubmissionCheckOptions,
): { ok: true; hash: Buffer } | { ok: false; reason: string } {
  let tx: Transaction;
  try {
    tx = TransactionBuilder.fromXDR(opts.envelopeXdr, opts.networkPassphrase) as Transaction;
  } catch {
    return { ok: false, reason: "undecodable transaction envelope" };
  }

  if (!StrKey.isValidEd25519PublicKey(tx.source)) {
    return { ok: false, reason: "envelope source is not an ed25519 account" };
  }
  if (tx.source !== opts.expectedSourceAccountId) {
    return { ok: false, reason: "envelope source does not match the authenticated session" };
  }

  const hash = tx.hash();
  const clientKp = Keypair.fromPublicKey(opts.expectedSourceAccountId);
  const clientHint = keyHint(opts.expectedSourceAccountId);
  const hasClientSignature = tx.signatures.some(
    (s) =>
      s.hint().toString("hex") === clientHint && clientKp.verify(hash, s.signature()),
  );
  if (!hasClientSignature) {
    return { ok: false, reason: "session subject did not sign this transaction" };
  }
  return { ok: true, hash };
}

/** Hand-off point to the network. Injectable so tests stay offline. */
export async function submitToRpc(
  envelopeXdr: string,
  networkPassphrase: string,
  rpcUrl: string,
): Promise<{ status: string; hash: string }> {
  const server = new SorobanRpc.Server(rpcUrl);
  const tx = TransactionBuilder.fromXDR(envelopeXdr, networkPassphrase);
  const response = await server.sendTransaction(tx);
  return { status: response.status, hash: response.hash };
}

function keyHint(accountId: string): string {
  // Ed25519 hint = last 4 bytes of the raw key.
  const raw = Buffer.from(StrKey.decodeEd25519PublicKey(accountId));
  return raw.subarray(raw.length - 4).toString("hex");
}
