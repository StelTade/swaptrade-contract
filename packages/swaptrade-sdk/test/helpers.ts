/**
 * Shared test helpers.
 *
 * A fake RPC server is injected at the `RpcServerLike` boundary so tests cover
 * the SDK's own build/simulate/sign/submit logic without any network access.
 */
import { Account, Keypair, SorobanDataBuilder, nativeToScVal, xdr } from '@stellar/stellar-sdk';
import { vi } from 'vitest';
import type { RpcServerLike } from '../src/client.js';
import { keypairSigner } from '../src/signers.js';
import { NETWORKS, type SwapTradeConfig } from '../src/types.js';

/** A deterministic, syntactically valid contract ID for tests. */
export const TEST_CONTRACT_ID = 'CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE';

/**
 * Stable test keypair, derived from a fixed 32-byte seed rather than written out
 * as a literal secret. It controls no real funds on any network, and deriving it
 * keeps anything that looks like a credential out of the repository.
 */
export const TEST_KEYPAIR = Keypair.fromRawEd25519Seed(
  Buffer.alloc(32, 7),
);

export const TEST_PUBLIC_KEY = TEST_KEYPAIR.publicKey();

/** Options for {@link createFakeServer}. */
export interface FakeServerOptions {
  /** Value returned by `simulateTransaction`. */
  simulateResult?: unknown;
  /** Value returned by `sendTransaction`. */
  sendResult?: unknown;
  /** Sequence of values returned by successive `getTransaction` calls. */
  getTransactionResults?: unknown[];
  /** Force `getAccount` to reject. */
  accountError?: Error;
  /** Force `simulateTransaction` to reject. */
  simulateError?: Error;
  /** Force `sendTransaction` to reject. */
  sendError?: Error;
}

/**
 * Build a successful simulation response.
 *
 * This mirrors the *raw* JSON-RPC shape (`results[].xdr` as base64), because
 * `assembleTransaction` re-parses the response internally. Using the real shape
 * means the client's actual assemble/sign path is exercised rather than stubbed.
 */
export function simulationSuccess(retval?: xdr.ScVal): Record<string, unknown> {
  const returned = retval ?? xdr.ScVal.scvVoid();
  return {
    transactionData: new SorobanDataBuilder().build().toXDR('base64'),
    minResourceFee: '12345',
    latestLedger: 100,
    events: [],
    results: [{ xdr: returned.toXDR('base64'), auth: [] }],
    // The parsed `result.retval` the client reads for its return value.
    result: { retval: returned, auth: [] },
  };
}

/** Build a failed simulation response. */
export function simulationFailure(error: string): Record<string, unknown> {
  return { error, events: [], latestLedger: 100 };
}

/**
 * Create a fake RPC server.
 *
 * `getAccount` returns a real `Account` so `TransactionBuilder` behaves exactly
 * as it would in production.
 */
export function createFakeServer(options: FakeServerOptions = {}): RpcServerLike & {
  getAccount: ReturnType<typeof vi.fn>;
  simulateTransaction: ReturnType<typeof vi.fn>;
  sendTransaction: ReturnType<typeof vi.fn>;
  getTransaction: ReturnType<typeof vi.fn>;
} {
  const getTransactionResults = options.getTransactionResults ?? [
    { status: 'SUCCESS', ledger: 101 },
  ];
  let pollIndex = 0;

  return {
    getAccount: vi.fn(async (address: string) => {
      if (options.accountError) throw options.accountError;
      return new Account(address, '1');
    }),
    simulateTransaction: vi.fn(async () => {
      if (options.simulateError) throw options.simulateError;
      return options.simulateResult ?? simulationSuccess();
    }),
    sendTransaction: vi.fn(async () => {
      if (options.sendError) throw options.sendError;
      return options.sendResult ?? { status: 'PENDING', hash: 'a'.repeat(64) };
    }),
    getTransaction: vi.fn(async () => {
      const result =
        getTransactionResults[Math.min(pollIndex, getTransactionResults.length - 1)];
      pollIndex += 1;
      return result;
    }),
  };
}

/** A valid base config for the local network, with a working local signer. */
export function baseConfig(overrides: Partial<SwapTradeConfig> = {}): SwapTradeConfig {
  return {
    rpcUrl: NETWORKS.local.rpcUrl,
    networkPassphrase: NETWORKS.local.networkPassphrase,
    contractId: TEST_CONTRACT_ID,
    publicKey: TEST_PUBLIC_KEY,
    signTransaction: keypairSigner(TEST_KEYPAIR.secret()),
    ...overrides,
  };
}

/** Encode an `i128` return value for a simulation response. */
export function i128Return(value: bigint): xdr.ScVal {
  return nativeToScVal(value, { type: 'i128' });
}

/** Encode a `u64` return value for a simulation response. */
export function u64Return(value: bigint): xdr.ScVal {
  return nativeToScVal(value, { type: 'u64' });
}
