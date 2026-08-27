/**
 * Tests for the SEP-10 example (issue #260): accepted flows, rejected
 * flows and replay protections.
 *
 * Run with: npm test
 */
import { describe, expect, it } from "vitest";
import nacl from "tweetnacl";
import {
  Challenge,
  ReplayGuard,
  canonicalPayload,
  createChallenge,
  decodeStrKey,
  encodeStrKey,
  issueSessionToken,
  signBytes,
  verifyChallenge,
  verifySessionToken,
} from "../src/sep10.js";
import { ContractIntent, Sep10Gate, intentPayload } from "../src/gating.js";

const T0 = 1_700_000_000;

function serverKeyPair() {
  const seed = Buffer.alloc(32, 7); // deterministic test seed
  return nacl.sign.keyPair.fromSeed(seed);
}

function makeClient() {
  const pair = nacl.sign.keyPair();
  return { accountId: encodeStrKey(pair.publicKey), secret: pair.secretKey };
}

function clientSign(secret: Uint8Array, payload: string): string {
  return Buffer.from(nacl.sign.detached(Buffer.from(payload, "utf8"), secret)).toString("base64");
}

describe("strkey codec", () => {
  it("round-trips ed25519 keys as G... addresses", () => {
    const raw = serverKeyPair().publicKey;
    const str = encodeStrKey(raw);
    expect(str[0]).toBe("G");
    expect(str.length).toBe(56);
    expect(Buffer.from(decodeStrKey(str)).equals(Buffer.from(raw))).toBe(true);
  });

  it("rejects corrupted checksums", () => {
    const str = encodeStrKey(serverKeyPair().publicKey);
    const flipped = (str[10] === "A" ? "B" : "A") + str.slice(11);
    expect(() => decodeStrKey(flipped)).toThrow(/checksum/);
  });
});

describe("challenge verification", () => {
  it("accepts a correctly signed challenge", () => {
    const server = serverKeyPair();
    const client = makeClient();
    const { challenge } = createChallenge(
      Buffer.from(server.secretKey.slice(0, 32)).toString("hex"),
      client.accountId,
      T0,
    );
    const payload = canonicalPayload(challenge);
    const serverSig = clientSign(server.secretKey, payload);
    const clientSig = clientSign(client.secret, payload);
    expect(verifyChallenge(challenge, serverSig, clientSig, server.publicKey, T0 + 1)).toBe(true);
  });

  it("rejects a client signature from a different key", () => {
    const server = serverKeyPair();
    const client = makeClient();
    const impostor = makeClient();
    const { challenge } = createChallenge(
      Buffer.from(server.secretKey.slice(0, 32)).toString("hex"),
      client.accountId,
      T0,
    );
    const payload = canonicalPayload(challenge);
    const serverSig = clientSign(server.secretKey, payload);
    const badClientSig = clientSign(impostor.secret, payload);
    expect(verifyChallenge(challenge, serverSig, badClientSig, server.publicKey, T0 + 1)).toBe(false);
  });

  it("rejects tampered challenge payloads", () => {
    const server = serverKeyPair();
    const client = makeClient();
    const { challenge } = createChallenge(
      Buffer.from(server.secretKey.slice(0, 32)).toString("hex"),
      client.accountId,
      T0,
    );
    const serverSig = clientSign(server.secretKey, canonicalPayload(challenge));
    const tampered: Challenge = { ...challenge, nonceB64: Buffer.alloc(64, 9).toString("base64") };
    const clientSig = clientSign(client.secret, canonicalPayload(tampered));
    // Client signed the *tampered* copy but the server signature is over the
    // original bytes -> verification must fail.
    expect(
      verifyChallenge(tampered, serverSig, clientSig, server.publicKey, T0 + 1),
    ).toBe(false);
  });

  it("rejects challenges outside the validity window", () => {
    const server = serverKeyPair();
    const client = makeClient();
    const { challenge } = createChallenge(
      Buffer.from(server.secretKey.slice(0, 32)).toString("hex"),
      client.accountId,
      T0,
    );
    const payload = canonicalPayload(challenge);
    const serverSig = clientSign(server.secretKey, payload);
    const clientSig = clientSign(client.secret, payload);

    expect(verifyChallenge(challenge, serverSig, clientSig, server.publicKey, T0 - 1)).toBe(false);
    expect(
      verifyChallenge(challenge, serverSig, clientSig, server.publicKey, challenge.expiresAt),
    ).toBe(false);
    expect(
      verifyChallenge(challenge, serverSig, clientSig, server.publicKey, challenge.expiresAt - 1),
    ).toBe(true);
  });
});

describe("session tokens", () => {
  it("verifies genuine tokens and rejects forgeries/expiry", () => {
    const secret = "ab".repeat(32);
    const token = issueSessionToken("GAAA", secret, 60, T0);
    expect(verifySessionToken(token, secret, T0 + 30)).toBe(true);
    expect(verifySessionToken(token, secret, T0 + 60)).toBe(false); // expired
    const forged = { ...token, sig: Buffer.alloc(32).toString("base64") };
    expect(verifySessionToken(forged, secret, T0 + 1)).toBe(false);
    expect(verifySessionToken(token, "cd".repeat(32), T0 + 1)).toBe(false);
  });
});

describe("replay guard", () => {
  it("consumes each key exactly once", () => {
    const guard = new ReplayGuard();
    expect(guard.consume("n1")).toBe(true);
    expect(guard.consume("n1")).toBe(false);
    expect(guard.consume("n2")).toBe(true);
    expect(guard.size).toBe(2);
  });
});

describe("SEP-10 gated contract invocations", () => {
  function setupGate() {
    const server = serverKeyPair();
    const gate = new Sep10Gate(
      server.publicKey,
      Buffer.from(server.secretKey.slice(0, 32)).toString("hex"),
      () => T0,
    );
    return { gate, server };
  }

  function completeFlow(gate: Sep10Gate, server: nacl.SignKeyPair, client: ReturnType<typeof makeClient>) {
    const { challenge } = createChallenge(
      Buffer.from(server.secretKey.slice(0, 32)).toString("hex"),
      client.accountId,
      T0,
    );
    const payload = canonicalPayload(challenge);
    return gate.completeChallenge(
      challenge,
      clientSign(server.secretKey, payload),
      clientSign(client.secret, payload),
    );
  }

  const intent: ContractIntent = {
    contractId: "CAS3J7GYLGXMF6TDJBBYYE3JNNFRVLDDTT6E8B2LNL4N25Q6YVGB72PI",
    functionName: "execute_swap",
    args: [{ amount: "100000000" }],
    nonce: 1,
  };

  it("mints a session token after a valid challenge and gates calls", () => {
    const { gate, server } = setupGate();
    const result = completeFlow(gate, server, makeClient());

    expect(result.allowed).toBe(true);
    expect(result.token).toBeDefined();

    const authorized = gate.authorizeWithToken(result.token!, intent);
    expect(authorized.allowed).toBe(true);
    expect(authorized.account).toBeDefined();
  });

  it("rejects replayed challenges (same nonce reused)", () => {
    const { gate, server } = setupGate();
    const client = makeClient();
    const { challenge } = createChallenge(
      Buffer.from(server.secretKey.slice(0, 32)).toString("hex"),
      client.accountId,
      T0,
    );
    const payload = canonicalPayload(challenge);
    const serverSig = clientSign(server.secretKey, payload);
    const clientSig = clientSign(client.secret, payload);

    const first = gate.completeChallenge(challenge, serverSig, clientSig);
    expect(first.allowed).toBe(true);

    // The exact same challenge presented again -> replay.
    const second = gate.completeChallenge(challenge, serverSig, clientSig);
    expect(second.allowed).toBe(false);
    expect(second.reason).toMatch(/replay/i);
  });

  it("rejects the same intent twice even under a fresh token", () => {
    const { gate, server } = setupGate();
    const client = makeClient();
    const token1 = completeFlow(gate, server, client).token!;
    const token2 = completeFlow(gate, server, client).token!;

    expect(gate.authorizeWithToken(token1, intent).allowed).toBe(true);
    const again = gate.authorizeWithToken(token2, intent);
    expect(again.allowed).toBe(false);
    expect(again.reason).toMatch(/replay/i);
  });

  it("rejects expired or forged session tokens", () => {
    const { gate, server } = setupGate();
    const token = completeFlow(gate, server, makeClient()).token!;
    const expired = { ...token, expiresAt: T0 - 1 };
    expect(gate.authorizeWithToken(expired, intent).reason).toMatch(/invalid or expired/i);

    const forged = { ...token, account: encodeStrKey(nacl.sign.keyPair().publicKey) };
    const res = gate.authorizeWithToken(forged, intent);
    expect(res.allowed).toBe(false);
    expect(res.reason).toMatch(/invalid or expired/i);
  });

  it("relayer flow verifies signed intents and enforces nonces", () => {
    const { gate } = setupGate();
    const client = makeClient();
    const payload = intentPayload(intent);
    const sig = clientSign(client.secret, payload);

    expect(gate.authorizeSignedIntent(client.accountId, intent, sig).allowed).toBe(true);
    const replayed = gate.authorizeSignedIntent(client.accountId, intent, sig);
    expect(replayed.allowed).toBe(false);
    expect(replayed.reason).toMatch(/replay/i);
  });

  it("relayer rejects intents signed by another key", () => {
    const { gate } = setupGate();
    const attacker = makeClient();
    const sig = clientSign(attacker.secret, intentPayload(intent));
    const victim = makeClient();
    const res = gate.authorizeSignedIntent(victim.accountId, intent, sig);
    expect(res.allowed).toBe(false);
    expect(res.reason).toMatch(/signature invalid/i);
  });
});
