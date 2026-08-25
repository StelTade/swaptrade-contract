import { createHmac, timingSafeEqual } from "node:crypto";
import type { Sep10Config } from "./config";

/**
 * Stateless HMAC session tokens issued after SEP-10 verification.
 *
 * Deliberately dependency-free: the format is
 *
 *   base64url(payload).base64url(HMAC-SHA256(payload, secret))
 *
 * with the payload carrying the authenticated account, issuance time,
 * expiry, and a jti bound to the consumed challenge nonce. Binding the
 * token to the nonce is what lets revocation ride on the same single-use
 * registry the protocol already requires: once a challenge nonce is spent,
 * any token derived from it is identifiable and rejectable.
 */

export interface TokenPayload {
  /** Stellar account id of the authenticated user. */
  sub: string;
  /** Issued-at (unix seconds). */
  iat: number;
  /** Expiry (unix seconds). */
  exp: number;
  /** Unique token id — sha256 prefix of the consumed challenge nonce. */
  jti: string;
}

function b64url(input: Buffer): string {
  return input.toString("base64url");
}

export function issueToken(
  clientAccountId: string,
  nonceHash: string,
  config: Sep10Config,
  now: () => number = () => Math.floor(Date.now() / 1000),
): { token: string; payload: TokenPayload } {
  const iat = now();
  const payload: TokenPayload = {
    sub: clientAccountId,
    iat,
    exp: iat + config.sessionTtlSeconds,
    jti: nonceHash,
  };
  const body = b64url(Buffer.from(JSON.stringify(payload), "utf8"));
  const mac = createHmac("sha256", config.sessionSecret).update(body).digest();
  return { token: `${body}.${b64url(mac)}`, payload };
}

export class TokenError extends Error {
  constructor(
    public readonly code: "MALFORMED" | "BAD_SIGNATURE" | "EXPIRED",
    message: string,
  ) {
    super(message);
    this.name = "TokenError";
  }
}

export function verifyToken(
  token: string,
  config: Sep10Config,
  now: () => number = () => Math.floor(Date.now() / 1000),
): TokenPayload {
  const parts = token.split(".");
  if (parts.length !== 2) {
    throw new TokenError("MALFORMED", "token must be `<payload>.<signature>`");
  }
  const [body, sigB64] = parts;

  let expected: Buffer;
  let provided: Buffer;
  try {
    expected = createHmac("sha256", config.sessionSecret).update(body).digest();
    provided = Buffer.from(sigB64, "base64url");
  } catch {
    throw new TokenError("MALFORMED", "token signature is not valid base64url");
  }

  // Constant-time comparison so token MAC checks do not leak timing.
  if (provided.length !== expected.length || !timingSafeEqual(provided, expected)) {
    throw new TokenError("BAD_SIGNATURE", "token signature mismatch");
  }

  let payload: TokenPayload;
  try {
    payload = JSON.parse(Buffer.from(body, "base64url").toString("utf8")) as TokenPayload;
  } catch {
    throw new TokenError("MALFORMED", "token payload is not valid JSON");
  }

  if (typeof payload.exp !== "number" || now() >= payload.exp) {
    throw new TokenError("EXPIRED", `token expired at ${payload.exp}`);
  }
  return payload;
}
