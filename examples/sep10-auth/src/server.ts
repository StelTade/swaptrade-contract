import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { randomBytes } from "node:crypto";
import { Keypair, StrKey } from "@stellar/stellar-sdk";
import { defaultConfig, type Sep10Config } from "./config";
import { NonceStore } from "./nonce-store";
import {
  Sep10Error,
  buildChallenge,
  nonceFingerprint,
  verifyChallenge,
} from "./sep10";
import { issueToken } from "./token";
import { requireAuth, sendJson } from "./middleware";
import {
  authorizeInvocation,
  defaultPolicy,
  verifyUserAuthorization,
  type InvocationPlan,
} from "./contract-gate";

/**
 * Minimal runnable demonstration of the whole flow:
 *
 *   GET  /auth?account=G...   -> SEP-10 challenge transaction
 *   POST /auth                -> {transaction: <signed xdr>} => session token
 *   POST /invoke              -> gated contract invocation request
 *
 * Run with `npm run demo` and follow README.md for a scripted walkthrough.
 */

const config: Sep10Config = defaultConfig;
const serverKeypair = process.env.SEP10_SERVER_SECRET
  ? Keypair.fromSecret(process.env.SEP10_SERVER_SECRET)
  : Keypair.random();

const nonces = new NonceStore(config.challengeWindowSeconds);
const sessionUsage = new Map<string, number>();

// Swap in your deployed atomic-swap contract id here.
const ATOMIC_SWAP_CONTRACT_ID =
  process.env.ATOMIC_SWAP_CONTRACT_ID ?? StrKey.encodeContract(randomBytes(32));
const policy = defaultPolicy(ATOMIC_SWAP_CONTRACT_ID);

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    let data = "";
    req.on("data", (chunk) => (data += chunk));
    req.on("end", () => resolve(data));
    req.on("error", reject);
  });
}

async function handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
  const url = new URL(req.url ?? "/", "http://localhost");

  if (req.method === "GET" && url.pathname === "/auth") {
    const clientAccountId = url.searchParams.get("account");
    if (!clientAccountId) {
      sendJson(res, 400, { error: "query parameter `account` is required" });
      return;
    }
    try {
      const challenge = buildChallenge(serverKeypair, clientAccountId, config);
      sendJson(res, 200, challenge);
    } catch {
      sendJson(res, 400, { error: "invalid account id" });
    }
    return;
  }

  if (req.method === "POST" && url.pathname === "/auth") {
    const body = JSON.parse((await readBody(req)) || "{}") as { transaction?: string };
    if (!body.transaction) {
      sendJson(res, 400, { error: "body must be {transaction: <signed challenge xdr>}" });
      return;
    }
    try {
      const { clientAccountId, nonce } = verifyChallenge({
        signedChallengeXdr: body.transaction,
        serverKeypair,
        nonces,
        config,
      });
      const { token, payload } = issueToken(clientAccountId, nonceFingerprint(nonce), config);
      sendJson(res, 200, { token, expires_at: payload.exp });
    } catch (err) {
      if (err instanceof Sep10Error) {
        // Uniform 401s for every verification failure; details go to logs only.
        console.warn("challenge rejected:", err.code, err.message);
        sendJson(res, 401, { error: "authentication failed" });
      } else {
        throw err;
      }
    }
    return;
  }

  if (req.method === "POST" && url.pathname === "/invoke") {
    return requireAuth(config, async (authedReq, authedRes) => {
      const plan = JSON.parse((await readBody(authedReq)) || "{}") as InvocationPlan & {
        signedEnvelopeXdr?: string;
      };
      try {
        authorizeInvocation(authedReq.auth!, plan, policy, (jti) => sessionUsage.get(jti) ?? 0);
      } catch (err) {
        sendJson(authedRes, 403, { error: (err as Error).message });
        return;
      }

      // Relayer rule: never fund what the subject did not sign.
      const verdict = verifyUserAuthorization({
        envelopeXdr: plan.signedEnvelopeXdr ?? "",
        expectedSourceAccountId: authedReq.auth!.clientAccountId,
        networkPassphrase: config.networkPassphrase,
      });
      if (!verdict.ok) {
        sendJson(authedRes, 403, { error: verdict.reason });
        return;
      }

      sessionUsage.set(authedReq.auth!.jti, (sessionUsage.get(authedReq.auth!.jti) ?? 0) + 1);
      // Production: submitToRpc(plan.signedEnvelopeXdr, pass, rpcUrl)
      sendJson(authedRes, 202, {
        accepted: true,
        txHash: verdict.hash.toString("hex"),
        note: "verified against session subject — hand off to Soroban RPC here",
      });
    })(req, res);
  }

  sendJson(res, 404, { error: "not found" });
}

export function startDemoServer(port = 8787): void {
  createServer((req, res) => {
    handle(req, res).catch((err) => {
      console.error(err);
      sendJson(res, 500, { error: "internal" });
    });
  }).listen(port, () => {
    console.log(`SEP-10 demo listening on :${port}`);
    console.log(`server account: ${serverKeypair.publicKey()}`);
  });
}

if (require.main === module) {
  startDemoServer();
}
