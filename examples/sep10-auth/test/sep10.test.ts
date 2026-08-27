import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { Keypair, Account, Operation, StrKey, TransactionBuilder } from "@stellar/stellar-sdk";
import { randomBytes } from "node:crypto";

import { defaultConfig, type Sep10Config } from "../src/config";
import { NonceStore } from "../src/nonce-store";
import {
  Sep10Error,
  buildChallenge,
  signChallenge,
  verifyChallenge,
} from "../src/sep10";
import { TokenError, issueToken, verifyToken } from "../src/token";
import {
  PolicyError,
  authorizeInvocation,
  createInvocationForSigning,
  defaultPolicy,
  verifyUserAuthorization,
} from "../src/contract-gate";

// Deterministic clock: tests pin `now` so timebounds behave predictably.
const T0 = 1_700_000_000;
const at = (t: number) => () => t;

function makeHarness() {
  const serverKp = Keypair.random();
  const clientKp = Keypair.random();
  const config: Sep10Config = { ...defaultConfig, sessionSecret: "test-secret" };
  const nonces = new NonceStore(300, at(T0));
  return { serverKp, clientKp, config, nonces };
}

/** Full happy-path run: challenge -> client co-sign -> verify. */
function completeAuth(
  h: ReturnType<typeof makeHarness>,
  now = at(T0 + 1),
) {
  const challenge = buildChallenge(h.serverKp, h.clientKp.publicKey(), h.config, at(T0));
  const signed = signChallenge({
    challengeXdr: challenge.transaction,
    clientKeypair: h.clientKp,
    config: h.config,
  });
  return verifyChallenge({
    signedChallengeXdr: signed,
    serverKeypair: h.serverKp,
    nonces: h.nonces,
    config: h.config,
    now,
  });
}

describe("SEP-10 happy path", () => {
  it("authenticates the client account and consumes the nonce exactly once", () => {
    const h = makeHarness();

    const result = completeAuth(h);
    assert.equal(result.clientAccountId, h.clientKp.publicKey());

    // Nonce is now burned in the store.
    assert.equal(h.nonces.has(result.nonce), true);
  });

  it("issues a session token that verifies and carries the subject", () => {
    const h = makeHarness();
    const session = completeAuth(h);

    const { token } = issueToken(session.clientAccountId, "jti-abc", h.config, at(T0 + 2));
    const payload = verifyToken(token, h.config, at(T0 + 3));
    assert.equal(payload.sub, h.clientKp.publicKey());
    assert.equal(payload.jti, "jti-abc");
    assert.ok(payload.exp > payload.iat);
  });
});

describe("SEP-10 rejection paths", () => {
  it("rejects a tampered nonce (signature no longer matches)", () => {
    const h = makeHarness();
    const challenge = buildChallenge(h.serverKp, h.clientKp.publicKey(), h.config, at(T0));
    let signed = signChallenge({
      challengeXdr: challenge.transaction,
      clientKeypair: h.clientKp,
      config: h.config,
    });

    // Flip one character in the middle of the base64 XDR: the signed
    // bytes no longer match what was signed. Depending on where the
    // corruption lands the envelope may fail to decode at all or merely
    // lose its signature validity — either way verification must reject.
    const mid = Math.floor(signed.length / 2);
    const victim = signed[mid] === "A" ? "B" : "A";
    signed = signed.slice(0, mid) + victim + signed.slice(mid + 1);

    assert.throws(
      () =>
        verifyChallenge({
          signedChallengeXdr: signed,
          serverKeypair: h.serverKp,
          nonces: h.nonces,
          config: h.config,
          now: at(T0 + 1),
        }),
      (err: unknown) => err instanceof Sep10Error,
    );
  });

  it("rejects when the wrong client signs", () => {
    const h = makeHarness();
    const impostor = Keypair.random();
    const challenge = buildChallenge(h.serverKp, h.clientKp.publicKey(), h.config, at(T0));

    // Impostor signs a challenge bound to someone else's account: the
    // signature set is still [server, signer] but the second signature
    // does not belong to the subject named by the transaction source.
    const tx = TransactionBuilder.fromXDR(challenge.transaction, h.config.networkPassphrase);
    tx.sign(impostor);

    assert.throws(
      () =>
        verifyChallenge({
          signedChallengeXdr: tx.toXDR(),
          serverKeypair: h.serverKp,
          nonces: h.nonces,
          config: h.config,
          now: at(T0 + 1),
        }),
      (err: unknown) => err instanceof Sep10Error && err.code === "UNKNOWN_SIGNER",
    );
  });

  it("rejects challenges without the server signature", () => {
    const h = makeHarness();
    const challenge = buildChallenge(h.serverKp, h.clientKp.publicKey(), h.config, at(T0));

    // Strip the server's signature: rebuild from raw envelope ops.
    const tx = TransactionBuilder.fromXDR(challenge.transaction, h.config.networkPassphrase);
    tx.signatures.length = 0;
    tx.sign(h.clientKp);

    assert.throws(
      () =>
        verifyChallenge({
          signedChallengeXdr: tx.toXDR(),
          serverKeypair: h.serverKp,
          nonces: h.nonces,
          config: h.config,
          now: at(T0 + 1),
        }),
      (err: unknown) => err instanceof Sep10Error && err.code === "MISSING_SIGNATURE",
    );
  });

  it("rejects expired challenges", () => {
    const h = makeHarness();
    // Challenge minted at T0 with a 300s window; verified an hour later.
    const challenge = buildChallenge(h.serverKp, h.clientKp.publicKey(), h.config, at(T0));
    const signed = signChallenge({
      challengeXdr: challenge.transaction,
      clientKeypair: h.clientKp,
      config: h.config,
    });

    assert.throws(
      () =>
        verifyChallenge({
          signedChallengeXdr: signed,
          serverKeypair: h.serverKp,
          nonces: h.nonces,
          config: h.config,
          now: at(T0 + 3600),
        }),
      (err: unknown) => err instanceof Sep10Error && err.code === "TIMEBOUNDS",
    );
  });

  it("rejects future-dated challenges (minTime not reached)", () => {
    const h = makeHarness();
    const challenge = buildChallenge(h.serverKp, h.clientKp.publicKey(), h.config, at(T0));
    const signed = signChallenge({
      challengeXdr: challenge.transaction,
      clientKeypair: h.clientKp,
      config: h.config,
    });

    assert.throws(
      () =>
        verifyChallenge({
          signedChallengeXdr: signed,
          serverKeypair: h.serverKp,
          nonces: h.nonces,
          config: h.config,
          now: at(T0 - 60),
        }),
      (err: unknown) => err instanceof Sep10Error && err.code === "TIMEBOUNDS",
    );
  });

  it("blocks replay of a redeemed challenge", () => {
    const h = makeHarness();
    const challenge = buildChallenge(h.serverKp, h.clientKp.publicKey(), h.config, at(T0));
    const signed = signChallenge({
      challengeXdr: challenge.transaction,
      clientKeypair: h.clientKp,
      config: h.config,
    });

    const first = verifyChallenge({
      signedChallengeXdr: signed,
      serverKeypair: h.serverKp,
      nonces: h.nonces,
      config: h.config,
      now: at(T0 + 1),
    });
    assert.equal(first.clientAccountId, h.clientKp.publicKey());

    // The exact same envelope must fail on the second attempt even
    // though every signature is still valid.
    assert.throws(
      () =>
        verifyChallenge({
          signedChallengeXdr: signed,
          serverKeypair: h.serverKp,
          nonces: h.nonces,
          config: h.config,
          now: at(T0 + 1),
        }),
      (err: unknown) => err instanceof Sep10Error && err.code === "NONCE_REPLAY",
    );
  });

  it("rejects envelopes whose sequence number is not zero", () => {
    const h = makeHarness();

    // Forge a structurally-plausible challenge that differs only in its
    // sequence number — SEP-10 pins it to 0 so challenges can never be
    // replayed as real ledger transactions.
    // Builder bumps the account sequence into the transaction, so an
    // account at 41 yields a challenge carrying sequence 42 instead of
    // the mandated 0.
    const forged = new TransactionBuilder(
      new Account(h.serverKp.publicKey(), "41"),
      {
        fee: "100",
        networkPassphrase: h.config.networkPassphrase,
        timebounds: { minTime: T0, maxTime: T0 + 300 },
      },
    )
      .addOperation(
        Operation.manageData({
          name: `${h.config.authDomain} auth`,
          value: Buffer.alloc(64),
        }),
      )
      .build();
    assert.equal(forged.sequence, "42");
    forged.sign(h.serverKp);
    forged.sign(h.clientKp);

    assert.throws(
      () =>
        verifyChallenge({
          signedChallengeXdr: forged.toXDR(),
          serverKeypair: h.serverKp,
          nonces: h.nonces,
          config: h.config,
        }),
      (err: unknown) => err instanceof Sep10Error && err.code === "BAD_SEQUENCE",
    );
  });

  it("rejects extra operations smuggled into the challenge", () => {
    const h = makeHarness();

    // A second manageData op appended to an otherwise valid challenge.
    const smuggled = new TransactionBuilder(
      new Account(h.serverKp.publicKey(), "-1"),
      {
        fee: "100",
        networkPassphrase: h.config.networkPassphrase,
        timebounds: { minTime: T0, maxTime: T0 + 300 },
      },
    )
      .addOperation(
        Operation.manageData({
          name: `${h.config.authDomain} auth`,
          value: Buffer.alloc(64),
        }),
      )
      .addOperation(
        Operation.manageData({
          name: `${h.config.authDomain} smuggled`,
          value: Buffer.from("attacker-controlled"),
        }),
      )
      .build();
    smuggled.sign(h.serverKp);
    smuggled.sign(h.clientKp);

    assert.throws(
      () =>
        verifyChallenge({
          signedChallengeXdr: smuggled.toXDR(),
          serverKeypair: h.serverKp,
          nonces: h.nonces,
          config: h.config,
        }),
      (err: unknown) => err instanceof Sep10Error && err.code === "UNEXPECTED_OPERATION",
    );
  });

  it("rejects garbage input outright", () => {
    const h = makeHarness();
    assert.throws(
      () =>
        verifyChallenge({
          signedChallengeXdr: "not-an-xdr",
          serverKeypair: h.serverKp,
          nonces: h.nonces,
          config: h.config,
        }),
      Sep10Error,
    );
  });
});

describe("session tokens", () => {
  it("refuses expired tokens", () => {
    const h = makeHarness();
    const { token } = issueToken(h.clientKp.publicKey(), "jti-x", h.config, at(T0));
    assert.throws(
      () => verifyToken(token, h.config, at(T0 + h.config.sessionTtlSeconds + 1)),
      (err: unknown) => err instanceof TokenError && err.code === "EXPIRED",
    );
  });

  it("refuses tokens signed with a different secret", () => {
    const h = makeHarness();
    const { token } = issueToken(h.clientKp.publicKey(), "jti-y", h.config, at(T0));
    const rogueConfig = { ...h.config, sessionSecret: "attacker-secret" };
    assert.throws(
      () => verifyToken(token, rogueConfig, at(T0 + 1)),
      (err: unknown) => err instanceof TokenError && err.code === "BAD_SIGNATURE",
    );
  });

  it("binds distinct sessions to distinct jtis", () => {
    const h = makeHarness();
    const s1 = completeAuth(h);
    const challenge2 = buildChallenge(h.serverKp, h.clientKp.publicKey(), h.config, at(T0 + 10));
    const signed2 = signChallenge({
      challengeXdr: challenge2.transaction,
      clientKeypair: h.clientKp,
      config: h.config,
    });
    const s2 = verifyChallenge({
      signedChallengeXdr: signed2,
      serverKeypair: h.serverKp,
      nonces: h.nonces,
      config: h.config,
      now: at(T0 + 11),
    });
    assert.notEqual(s1.nonce, s2.nonce);
  });
});

describe("contract invocation gating", () => {
  const contractId = StrKey.encodeContract(randomBytes(32));

  it("allows permitted functions within quota", () => {
    const policy = defaultPolicy(contractId);
    authorizeInvocation(
      { clientAccountId: "GABC", jti: "sess-1" },
      { contractId, functionName: "lock_asset", args: [] },
      policy,
      () => 0,
    ); // must not throw
  });

  it("blocks functions outside the allowlist", () => {
    const policy = defaultPolicy(contractId);
    assert.throws(
      () =>
        authorizeInvocation(
          { clientAccountId: "GABC", jti: "sess-1" },
          { contractId, functionName: "set_admin", args: [] },
          policy,
          () => 0,
        ),
      (err: unknown) => err instanceof PolicyError && err.reason === "FORBIDDEN_TARGET",
    );
  });

  it("blocks contracts outside the allowlist entirely", () => {
    const policy = defaultPolicy(contractId);
    assert.throws(
      () =>
        authorizeInvocation(
          { clientAccountId: "GABC", jti: "sess-1" },
          { contractId: "COTHER000000000000000000000000000000000000000000000000000", functionName: "get_quote", args: [] },
          policy,
          () => 0,
        ),
      PolicyError,
    );
  });

  it("enforces per-session invocation quota", () => {
    const policy = defaultPolicy(contractId);
    assert.throws(
      () =>
        authorizeInvocation(
          { clientAccountId: "GABC", jti: "sess-9" },
          { contractId, functionName: "get_quote", args: [] },
          policy,
          () => policy.maxInvocationsPerSession,
        ),
      (err: unknown) => err instanceof PolicyError && err.reason === "SESSION_QUOTA",
    );
  });

  it("relayer refuses envelopes not signed by the session subject", () => {
    const h = makeHarness();
    const stranger = Keypair.random();
    const plan = { contractId, functionName: "get_quote", args: [42] };
    const unsigned = createInvocationForSigning(h.clientKp.publicKey(), plan, h.config.networkPassphrase, at(T0));

    // Stranger signs instead of the subject.
    const tx = TransactionBuilder.fromXDR(unsigned, h.config.networkPassphrase);
    tx.sign(stranger);

    const verdict = verifyUserAuthorization({
      envelopeXdr: tx.toXDR(),
      expectedSourceAccountId: h.clientKp.publicKey(),
      networkPassphrase: h.config.networkPassphrase,
    });
    assert.equal(verdict.ok, false);
    if (!verdict.ok) assert.match(verdict.reason, /did not sign|does not match/);
  });

  it("relayer accepts properly-signed invocations from the subject", () => {
    const h = makeHarness();
    const plan = { contractId, functionName: "get_quote", args: [42] };
    const unsigned = createInvocationForSigning(h.clientKp.publicKey(), plan, h.config.networkPassphrase, at(T0));

    const tx = TransactionBuilder.fromXDR(unsigned, h.config.networkPassphrase);
    tx.sign(h.clientKp);

    const verdict = verifyUserAuthorization({
      envelopeXdr: tx.toXDR(),
      expectedSourceAccountId: h.clientKp.publicKey(),
      networkPassphrase: h.config.networkPassphrase,
    });
    assert.equal(verdict.ok, true);
  });

  it("relayer rejects envelopes whose source differs from the session", () => {
    const h = makeHarness();
    const plan = { contractId, functionName: "get_quote", args: [] };
    // Envelope built for the stranger but presented under the client's session.
    const unsigned = createInvocationForSigning(strangerPublicKey(), plan, h.config.networkPassphrase, at(T0));
    const tx = TransactionBuilder.fromXDR(unsigned, h.config.networkPassphrase);
    tx.sign(strangerKeypair());

    const verdict = verifyUserAuthorization({
      envelopeXdr: tx.toXDR(),
      expectedSourceAccountId: h.clientKp.publicKey(),
      networkPassphrase: h.config.networkPassphrase,
    });
    assert.equal(verdict.ok, false);
  });
});

let _strangerKp: Keypair | null = null;
function strangerKeypair(): Keypair {
  return (_strangerKp ??= Keypair.random());
}
function strangerPublicKey(): string {
  return strangerKeypair().publicKey();
}
