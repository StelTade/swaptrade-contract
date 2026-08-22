/**
 * SEP-10 challenge/response primitives for SwapTrade.
 *
 * Implements the core of SEP-10 (stellar-protocol/ecosystem/sep-0010.md)
 * without requiring network access:
 *  - server builds a challenge containing a random 64-byte nonce in a
 *    `manage_data` style payload signed with the server key,
 *  - client countersigns the exact same payload,
 *  - verification checks both signatures, the time bounds and that the
 *    nonce has not been consumed (replay protection).
 *
 * Ed25519 is Stellar's signature scheme; tweetnacl provides it.
 */
import nacl from "tweetnacl";
import { createHmac, randomBytes, timingSafeEqual } from "node:crypto";

export const CHALLENGE_TIMEOUT_SECONDS = 15 * 60;
export const SERVER_DATA_KEY = "swaptrade auth";
export const CLIENT_DOMAIN_KEY = "swaptrade client_domain";

/** Stellar strkey: version byte (6<<3), then 32-byte key, CRC16-xmodem checksum. */
const STRKEY_VERSION_ED25519 = (6 << 3) | 0; // 0x30

const BASE32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

function base32Encode(buf: Buffer): string {
  let bits = 0;
  let value = 0;
  let out = "";
  for (const byte of buf) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      out += BASE32_ALPHABET[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) out += BASE32_ALPHABET[(value << (5 - bits)) & 31];
  return out;
}

function base32Decode(s: string): Buffer {
  let bits = 0;
  let value = 0;
  const bytes: number[] = [];
  for (const c of s.toUpperCase()) {
    const idx = BASE32_ALPHABET.indexOf(c);
    if (idx === -1) throw new Error(`invalid base32 character ${c}`);
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) {
      bytes.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return Buffer.from(bytes);
}

function crc16Xmodem(buf: Buffer): number {
  let crc = 0;
  for (const b of buf) {
    crc ^= b << 8;
    for (let i = 0; i < 8; i++) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
    }
  }
  return crc;
}

/** Encode a raw 32-byte ed25519 public key as a Stellar `G...` strkey. */
export function encodeStrKey(rawPublicKey: Uint8Array): string {
  if (rawPublicKey.length !== 32) throw new Error("ed25519 public key must be 32 bytes");
  const payload = Buffer.concat([
    Buffer.from([STRKEY_VERSION_ED25519]),
    Buffer.from(rawPublicKey),
  ]);
  const checksum = Buffer.alloc(2);
  checksum.writeUInt16LE(crc16Xmodem(payload), 0);
  return base32Encode(Buffer.concat([payload, checksum]));
}

/** Decode a `G...` strkey back to the raw 32-byte key (checksum verified). */
export function decodeStrKey(strKey: string): Buffer {
  const decoded = base32Decode(strKey);
  const payload = decoded.subarray(0, decoded.length - 2);
  const checksum = decoded.readUInt16LE(decoded.length - 2);
  if (crc16Xmodem(payload) !== checksum) throw new Error("invalid strkey checksum");
  if (payload[0] !== STRKEY_VERSION_ED25519) throw new Error("not an ed25519 strkey");
  return payload.subarray(1);
}

export interface Challenge {
  serverAccount: string;
  clientAccount: string;
  nonceB64: string;
  issuedAt: number; // unix seconds
  expiresAt: number; // unix seconds
  clientDomain?: string;
}

/**
 * Server-side: build a SEP-10 challenge. In full SEP-10 this is encoded as
 * a Stellar transaction with `manage_data` operations carrying
 * `SERVER_DATA_KEY` -> nonce and `CLIENT_DOMAIN_KEY` -> client home domain.
 * Here the equivalent canonical payload is produced so the same signing and
 * verification rules apply.
 */
export function createChallenge(
  serverSecretSeedHex: string,
  clientAccount: string,
  nowSeconds: number,
  clientDomain?: string,
): { challenge: Challenge; signedPayload: string } {
  decodeStrKey(clientAccount); // validates shape early
  const nonceB64 = randomBytes(64).toString("base64");
  const expiresAt = nowSeconds + CHALLENGE_TIMEOUT_SECONDS;
  const challenge: Challenge = {
    serverAccount: encodeStrKey(nacl.sign.keyPair.fromSeed(Buffer.from(serverSecretSeedHex, "hex")).publicKey),
    clientAccount,
    nonceB64,
    issuedAt: nowSeconds,
    expiresAt,
    clientDomain,
  };
  return { challenge, signedPayload: canonicalPayload(challenge) };
}

/** Deterministic byte payload both parties sign (SEP-10 transaction hash analogue). */
export function canonicalPayload(ch: Challenge): string {
  return [
    SERVER_DATA_KEY,
    ch.serverAccount,
    ch.clientAccount,
    ch.nonceB64,
    String(ch.issuedAt),
    String(ch.expiresAt),
    ch.clientDomain ? `${CLIENT_DOMAIN_KEY}:${ch.clientDomain}` : "",
  ].join("\n");
}

/**
 * Verification rules (mirrors SEP-10 `verify_challenge_tx_hash`):
 *  1. exactly two signatures expected: server then client
 *  2. server signature must match the advertised server account
 *  3. client signature must match `challenge.clientAccount`
 *  4. challenge must still be within its validity window
 */
export function verifyChallenge(
  challenge: Challenge,
  serverSignatureB64: string,
  clientSignatureB64: string,
  serverPublicKeyRaw: Uint8Array,
  nowSeconds: number,
): boolean {
  if (nowSeconds < challenge.issuedAt || nowSeconds >= challenge.expiresAt) return false;

  const message = Buffer.from(canonicalPayload(challenge), "utf8");
  const serverSig = Buffer.from(serverSignatureB64, "base64");
  const clientSig = Buffer.from(clientSignatureB64, "base64");

  const serverOk = nacl.sign.detached.verify(message, serverSig, serverPublicKeyRaw);
  if (!serverOk) return false;

  let clientKey: Buffer;
  try {
    clientKey = decodeStrKey(challenge.clientAccount);
  } catch {
    return false;
  }
  return nacl.sign.detached.verify(message, clientSig, clientKey);
}

export function signBytes(payload: string, secretSeedOrKeyHex: string): string {
  const keyPair =
    secretSeedOrKeyHex.length === 64
      ? nacl.sign.keyPair.fromSeed(Buffer.from(secretSeedOrKeyHex, "hex"))
      : nacl.sign.keyPair.fromSecretKey(Buffer.from(secretSeedOrKeyHex, "hex"));
  const sig = nacl.sign.detached(Buffer.from(payload, "utf8"), keyPair.secretKey);
  return Buffer.from(sig).toString("base64");
}

// ---------------------------------------------------------------------------
// Session tokens (the JWT analogue SEP-10 servers hand out after a successful
// challenge). HMAC-SHA256 with explicit expiry keeps this dependency-free.
// ---------------------------------------------------------------------------

export interface SessionToken {
  account: string;
  issuedAt: number;
  expiresAt: number;
  sig: string;
}

export function issueSessionToken(
  account: string,
  serverSecretHex: string,
  ttlSeconds: number,
  nowSeconds: number,
): SessionToken {
  const body = JSON.stringify({ account, issuedAt: nowSeconds, expiresAt: nowSeconds + ttlSeconds });
  const sig = createHmac("sha256", Buffer.from(serverSecretHex, "hex")).update(body).digest("base64");
  return { ...JSON.parse(body), sig };
}

export function verifySessionToken(token: SessionToken, serverSecretHex: string, nowSeconds: number): boolean {
  const { account, issuedAt, expiresAt, sig } = token;
  if (nowSeconds < issuedAt || nowSeconds >= expiresAt) return false;
  const body = JSON.stringify({ account, issuedAt, expiresAt });
  const expected = createHmac("sha256", Buffer.from(serverSecretHex, "hex")).update(body).digest();
  const given = Buffer.from(sig, "base64");
  return expected.length === given.length && timingSafeEqual(expected, given);
}

// ---------------------------------------------------------------------------
// Replay protection: consumed nonces and raw signed payloads.
// ---------------------------------------------------------------------------

export class ReplayGuard {
  private readonly seen = new Set<string>();

  /** Returns true on first sight of `key`, false afterwards. */
  consume(key: string): boolean {
    if (this.seen.has(key)) return false;
    this.seen.add(key);
    return true;
  }

  get size(): number {
    return this.seen.size;
  }
}
