import type { IncomingMessage, ServerResponse } from "node:http";
import { TokenError, verifyToken } from "./token";
import type { Sep10Config } from "./config";

/**
 * HTTP bearer-token gate.
 *
 * Every contract-invoking endpoint sits behind this: no valid SEP-10
 * session token, no invocation. `contract-gate.ts` builds on it to show
 * how the authenticated identity then drives a Soroban operation.
 */

export interface AuthedRequest extends IncomingMessage {
  auth?: {
    clientAccountId: string;
    jti: string;
  };
}

export function extractBearerToken(req: IncomingMessage): string | null {
  const header = req.headers.authorization;
  if (!header || !header.startsWith("Bearer ")) return null;
  return header.slice("Bearer ".length).trim() || null;
}

export type Handler = (req: AuthedRequest, res: ServerResponse) => void | Promise<void>;

/**
 * Wrap an endpoint handler so it only executes for requests carrying a
 * currently-valid session token. Rejections are uniform on purpose —
 * they do not reveal whether the token was malformed, tampered with,
 * or merely expired.
 */
export function requireAuth(
  config: Sep10Config,
  handler: Handler,
  now: () => number = () => Math.floor(Date.now() / 1000),
): Handler {
  return async (req: AuthedRequest, res: ServerResponse) => {
    const token = extractBearerToken(req);
    if (!token) {
      res.writeHead(401, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "missing bearer token" }));
      return;
    }
    try {
      const payload = verifyToken(token, config, now);
      req.auth = { clientAccountId: payload.sub, jti: payload.jti };
    } catch (err) {
      void err;
      res.writeHead(401, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "invalid or expired session token" }));
      return;
    }
    await handler(req, res);
  };
}

export function sendJson(res: ServerResponse, status: number, body: unknown): void {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(body));
}
