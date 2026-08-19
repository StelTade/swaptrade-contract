/**
 * Test double for the SDK client.
 *
 * The demo's seam is the client object itself: `src/workflow.ts` only ever calls
 * its methods, so replacing it here exercises the real workflow and hook logic
 * without a network, a signer, or the Stellar SDK. Tests assert on what the user
 * sees, not on how the component is wired.
 */
import type { Order, SwapTradeClient } from '@swaptrade/sdk';
import { vi } from 'vitest';
import type { ClientSetup } from '../src/config.js';
import type { SignerKind } from '../src/signer.js';

export const DEMO_ACCOUNT = 'GDVEU3DD4KOFECV66VIHWEZOYX4ZKR3WV27L464SIIPOU2IUI3JCZA57';
export const DEMO_CONTRACT = 'CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE';
export const DEMO_PASSPHRASE = 'Standalone Network ; February 2017';
export const DEMO_RPC = 'http://localhost:8000/soroban/rpc';

/** A representative order, matching the field names the SDK decodes to. */
export function fakeOrder(overrides: Partial<Order> = {}): Order {
  return {
    orderId: 7n,
    owner: DEMO_ACCOUNT,
    orderType: 'Limit',
    tokenIn: 'XLM',
    tokenOut: 'USDCSIM',
    amountIn: 1_000n,
    amountFilled: 0n,
    limitPrice: 1_000_000n,
    status: 'Pending',
    createdAt: 1_700_000_000n,
    ...overrides,
  };
}

export interface FakeClientOptions {
  kycVerified?: boolean;
  balance?: bigint;
  orders?: Order[];
  tradeCount?: number;
  totalVolume?: bigint;
  placedOrderId?: bigint;
  executedIds?: bigint[];
}

export type FakeClient = SwapTradeClient & {
  kycIsVerified: ReturnType<typeof vi.fn>;
  kycSubmit: ReturnType<typeof vi.fn>;
  kycUpdateStatus: ReturnType<typeof vi.fn>;
  setPrice: ReturnType<typeof vi.fn>;
  placeLimitOrder: ReturnType<typeof vi.fn>;
  mint: ReturnType<typeof vi.fn>;
  executeDueOrders: ReturnType<typeof vi.fn>;
  balanceOf: ReturnType<typeof vi.fn>;
  getPortfolio: ReturnType<typeof vi.fn>;
  getUserOrders: ReturnType<typeof vi.fn>;
};

/** Build a fake client whose methods resolve like a healthy contract would. */
export function createFakeClient(options: FakeClientOptions = {}): FakeClient {
  const tx = (hash: string, returnValue?: unknown) => ({
    hash,
    status: 'SUCCESS',
    ledger: 101,
    ...(returnValue !== undefined ? { returnValue } : {}),
  });

  const fake = {
    config: {
      publicKey: DEMO_ACCOUNT,
      contractId: DEMO_CONTRACT,
      networkPassphrase: DEMO_PASSPHRASE,
      rpcUrl: DEMO_RPC,
      allowHttp: true,
      fee: '1000000',
      timeoutSeconds: 60,
      pollTimeoutMs: 30_000,
    },
    kycIsVerified: vi.fn(async () => options.kycVerified ?? false),
    kycSubmit: vi.fn(async () => tx('k'.repeat(64))),
    kycUpdateStatus: vi.fn(async () => tx('u'.repeat(64))),
    setPrice: vi.fn(async () => tx('p'.repeat(64))),
    placeLimitOrder: vi.fn(async () => tx('c'.repeat(64), options.placedOrderId ?? 7n)),
    mint: vi.fn(async () => tx('f'.repeat(64))),
    executeDueOrders: vi.fn(async () => tx('a'.repeat(64), options.executedIds ?? [7n])),
    balanceOf: vi.fn(async () => options.balance ?? 5_000n),
    getPortfolio: vi.fn(async () => ({
      tradeCount: options.tradeCount ?? 2,
      totalVolume: options.totalVolume ?? 9_000n,
    })),
    getUserOrders: vi.fn(async () => options.orders ?? [fakeOrder()]),
  };

  return fake as unknown as FakeClient;
}

/** Wrap a fake client in the `ClientSetup` shape `App` accepts. */
export function fakeSetup(
  client: SwapTradeClient,
  signerKind: SignerKind = 'browser-wallet',
): ClientSetup {
  return { ok: true, client, signerKind };
}

/**
 * A wallet double that signs nothing.
 *
 * Matches the `BrowserWallet` shape the SDK adapts, so `resolveSigner` accepts
 * it. It records what it was asked to sign and returns a placeholder envelope;
 * pass `{ reject: true }` to simulate a user declining. No key is involved —
 * real signing is covered by the SDK's own signer tests.
 */
export function fakeWallet(options: { reject?: boolean } = {}) {
  const requests: { xdr: string; networkPassphrase?: string; address?: string }[] = [];

  return {
    requests,
    signTransaction: vi.fn(
      async (xdr: string, opts: { networkPassphrase?: string; address?: string }) => {
        requests.push({ xdr, ...opts });
        if (options.reject) throw new Error('User declined the signature request.');
        return { signedTxXdr: `signed:${xdr}` };
      },
    ),
  };
}
