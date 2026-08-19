/**
 * Client behaviour: argument mapping, the create -> fund -> accept path, and
 * failure handling.
 *
 * A fake RPC server is injected at the `RpcServerLike` seam. Everything above it
 * — transaction building, simulation handling, signing, submission and polling —
 * is the real implementation, so these tests exercise the SDK rather than a mock
 * of it. No network calls are made.
 */
import { Address, TransactionBuilder, scValToNative } from '@stellar/stellar-sdk';
import { describe, expect, it } from 'vitest';
import {
  ConfigError,
  ContractCallError,
  RpcError,
  SigningError,
  SimulationError,
  SwapTradeClient,
  TransactionFailedError,
  TransactionTimeoutError,
  ValidationError,
} from '../src/index.js';
import {
  TEST_CONTRACT_ID,
  TEST_PUBLIC_KEY,
  baseConfig,
  createFakeServer,
  i128Return,
  simulationFailure,
  simulationSuccess,
  u64Return,
} from './helpers.js';

/**
 * Decode the invocation the client built, so tests can assert on the exact
 * method name and arguments that would reach the contract.
 */
function decodeInvocation(tx: { toXDR(): string }, networkPassphrase: string) {
  const parsed = TransactionBuilder.fromXDR(tx.toXDR(), networkPassphrase) as never;
  const op = (parsed as { operations: unknown[] }).operations[0] as {
    func: {
      invokeContract(): {
        functionName(): { toString(): string };
        args(): unknown[];
        contractAddress(): unknown;
      };
    };
  };
  const invoke = op.func.invokeContract();
  return {
    method: invoke.functionName().toString(),
    args: invoke.args().map((a) => scValToNative(a as never)),
    contract: Address.fromScAddress(invoke.contractAddress() as never).toString(),
  };
}

describe('buildTransaction', () => {
  it('targets the configured contract and encodes arguments in ABI order', async () => {
    const server = createFakeServer();
    const client = new SwapTradeClient(baseConfig(), { server });

    const tx = await client.buildTransaction('balance_of', []);
    expect(decodeInvocation(tx, client.config.networkPassphrase).contract).toBe(
      TEST_CONTRACT_ID,
    );
    expect(server.getAccount).toHaveBeenCalledWith(TEST_PUBLIC_KEY);
  });

  it('reports a helpful error when the source account cannot be loaded', async () => {
    const server = createFakeServer({ accountError: new Error('Account not found') });
    const client = new SwapTradeClient(baseConfig(), { server });

    await expect(client.buildTransaction('initialize')).rejects.toThrow(RpcError);
    await expect(client.buildTransaction('initialize')).rejects.toThrow(
      /Could not load source account/,
    );
  });
});

describe('read-only calls use simulation', () => {
  it('balanceOf maps (token, user) and decodes an i128 without precision loss', async () => {
    const huge = 170141183460469231731687303715884105727n;
    const server = createFakeServer({ simulateResult: simulationSuccess(i128Return(huge)) });
    const client = new SwapTradeClient(baseConfig(), { server });

    await expect(client.balanceOf('USDCSIM')).resolves.toBe(huge);

    const tx = server.simulateTransaction.mock.calls[0]![0] as { toXDR(): string };
    const { method, args } = decodeInvocation(tx, client.config.networkPassphrase);
    expect(method).toBe('balance_of');
    expect(args).toEqual(['USDCSIM', TEST_PUBLIC_KEY]);
    // Read-only paths must never submit.
    expect(server.sendTransaction).not.toHaveBeenCalled();
  });

  it('getPortfolio decodes the (u32, i128) tuple', async () => {
    const server = createFakeServer({
      simulateResult: simulationSuccess(
        (await import('@stellar/stellar-sdk')).xdr.ScVal.scvVec([
          (await import('@stellar/stellar-sdk')).nativeToScVal(3, { type: 'u32' }),
          i128Return(2_500n),
        ]),
      ),
    });
    const client = new SwapTradeClient(baseConfig(), { server });

    await expect(client.getPortfolio()).resolves.toEqual({
      tradeCount: 3,
      totalVolume: 2_500n,
    });
  });

  it('surfaces a contract error code from a failed simulation', async () => {
    // 500 is KYCVerificationRequired in counter/src/errors.rs.
    const server = createFakeServer({
      simulateResult: simulationFailure('HostError: Error(Contract, #500)'),
    });
    const client = new SwapTradeClient(baseConfig(), { server });

    const error = await client.balanceOf('XLM').catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ContractCallError);
    expect((error as ContractCallError).contractCode).toBe(500);
    expect((error as ContractCallError).contractName).toBe('KYCVerificationRequired');
  });

  it('reports a non-contract simulation failure as SimulationError', async () => {
    const server = createFakeServer({
      simulateResult: simulationFailure('resource limit exceeded'),
    });
    const client = new SwapTradeClient(baseConfig(), { server });

    await expect(client.balanceOf('XLM')).rejects.toThrow(SimulationError);
  });
});

describe('create -> fund -> accept', () => {
  it('CREATE: placeLimitOrder maps every argument and returns the new order ID', async () => {
    const server = createFakeServer({
      simulateResult: simulationSuccess(u64Return(7n)),
      getTransactionResults: [{ status: 'SUCCESS', ledger: 42, returnValue: u64Return(7n) }],
    });
    const client = new SwapTradeClient(baseConfig(), { server });

    const result = await client.placeLimitOrder({
      tokenIn: 'XLM',
      tokenOut: 'USDCSIM',
      amountIn: 1_000n,
      limitPrice: 1_000_000n,
      expiresAt: 1_800_000_000n,
    });

    expect(result.returnValue).toBe(7n);
    expect(result.status).toBe('SUCCESS');
    expect(result.ledger).toBe(42);

    const tx = server.simulateTransaction.mock.calls[0]![0] as { toXDR(): string };
    const { method, args } = decodeInvocation(tx, client.config.networkPassphrase);
    expect(method).toBe('place_limit_order');
    // Order matches place_limit_order in counter/src/lib.rs.
    expect(args).toEqual([
      'XLM',
      'USDCSIM',
      1_000n,
      1_000_000n,
      1_800_000_000n,
      TEST_PUBLIC_KEY,
    ]);
  });

  it('CREATE: omitting expiresAt encodes Option::None as void', async () => {
    const server = createFakeServer({ simulateResult: simulationSuccess(u64Return(1n)) });
    const client = new SwapTradeClient(baseConfig(), { server });

    await client.placeLimitOrder({
      tokenIn: 'XLM',
      tokenOut: 'USDCSIM',
      amountIn: 5n,
      limitPrice: 10n,
    });

    const tx = server.simulateTransaction.mock.calls[0]![0] as { toXDR(): string };
    const { args } = decodeInvocation(tx, client.config.networkPassphrase);
    // scValToNative maps ScVal::Void to null.
    expect(args[4]).toBeNull();
  });

  it('FUND: mint maps (token, to, amount)', async () => {
    const server = createFakeServer();
    const client = new SwapTradeClient(baseConfig(), { server });

    await client.mint('USDCSIM', TEST_PUBLIC_KEY, 5_000n);

    const tx = server.simulateTransaction.mock.calls[0]![0] as { toXDR(): string };
    const { method, args } = decodeInvocation(tx, client.config.networkPassphrase);
    expect(method).toBe('mint');
    expect(args).toEqual(['USDCSIM', TEST_PUBLIC_KEY, 5_000n]);
  });

  it('ACCEPT: executeDueOrders decodes the returned Vec<u64>', async () => {
    const { xdr, nativeToScVal } = await import('@stellar/stellar-sdk');
    const ids = xdr.ScVal.scvVec([
      nativeToScVal(7n, { type: 'u64' }),
      nativeToScVal(8n, { type: 'u64' }),
    ]);
    const server = createFakeServer({
      simulateResult: simulationSuccess(ids),
      getTransactionResults: [{ status: 'SUCCESS', ledger: 43, returnValue: ids }],
    });
    const client = new SwapTradeClient(baseConfig(), { server });

    const result = await client.executeDueOrders();
    expect(result.returnValue).toEqual([7n, 8n]);

    const tx = server.simulateTransaction.mock.calls[0]![0] as { toXDR(): string };
    expect(decodeInvocation(tx, client.config.networkPassphrase).method).toBe(
      'execute_due_orders',
    );
  });

  it('signs, submits and polls exactly once for a successful call', async () => {
    const server = createFakeServer();
    const client = new SwapTradeClient(baseConfig(), { server });

    await client.mint('XLM', TEST_PUBLIC_KEY, 1n);

    expect(server.simulateTransaction).toHaveBeenCalledTimes(1);
    expect(server.sendTransaction).toHaveBeenCalledTimes(1);
    expect(server.getTransaction).toHaveBeenCalledTimes(1);
  });

  it('polls until the transaction leaves NOT_FOUND', async () => {
    const server = createFakeServer({
      getTransactionResults: [
        { status: 'NOT_FOUND' },
        { status: 'SUCCESS', ledger: 44 },
      ],
    });
    const client = new SwapTradeClient(baseConfig({ pollTimeoutMs: 5_000 }), { server });

    await expect(client.mint('XLM', TEST_PUBLIC_KEY, 1n)).resolves.toMatchObject({
      status: 'SUCCESS',
    });
    expect(server.getTransaction).toHaveBeenCalledTimes(2);
  });
});

describe('argument validation happens before any network call', () => {
  it('rejects a non-positive amount', async () => {
    const server = createFakeServer();
    const client = new SwapTradeClient(baseConfig(), { server });

    await expect(client.mint('XLM', TEST_PUBLIC_KEY, 0n)).rejects.toThrow(ValidationError);
    expect(server.simulateTransaction).not.toHaveBeenCalled();
  });

  it('rejects an over-length token symbol', async () => {
    const client = new SwapTradeClient(baseConfig(), { server: createFakeServer() });
    await expect(client.balanceOf('WAYTOOLONGSYMBOL')).rejects.toThrow(/at most 9/);
  });

  it('rejects an invalid recipient address', async () => {
    const client = new SwapTradeClient(baseConfig(), { server: createFakeServer() });
    await expect(client.mint('XLM', 'not-an-address', 1n)).rejects.toThrow(
      /not a valid Stellar account ID/,
    );
  });

  it('rejects swapping a token for itself', async () => {
    const client = new SwapTradeClient(baseConfig(), { server: createFakeServer() });
    await expect(client.swap('XLM', 'XLM', 10n)).rejects.toThrow(/for itself/);
  });

  it('rejects out-of-range slippage', async () => {
    const client = new SwapTradeClient(baseConfig(), { server: createFakeServer() });
    await expect(client.setMaxSlippageBps(10_001)).rejects.toThrow(ContractCallError);
  });
});

describe('signing and submission failures', () => {
  it('requires a signer for state-changing calls', async () => {
    const client = new SwapTradeClient(
      baseConfig({ signTransaction: undefined }),
      { server: createFakeServer() },
    );

    await expect(client.mint('XLM', TEST_PUBLIC_KEY, 1n)).rejects.toThrow(ConfigError);
    await expect(client.mint('XLM', TEST_PUBLIC_KEY, 1n)).rejects.toThrow(
      /no signTransaction callback/,
    );
  });

  it('still allows read-only calls without a signer', async () => {
    const server = createFakeServer({ simulateResult: simulationSuccess(i128Return(9n)) });
    const client = new SwapTradeClient(baseConfig({ signTransaction: undefined }), { server });

    await expect(client.balanceOf('XLM')).resolves.toBe(9n);
  });

  it('wraps a signer rejection as SigningError', async () => {
    const client = new SwapTradeClient(
      baseConfig({
        signTransaction: () => {
          throw new Error('User declined the request');
        },
      }),
      { server: createFakeServer() },
    );

    const error = await client.mint('XLM', TEST_PUBLIC_KEY, 1n).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(SigningError);
    expect((error as SigningError).message).toMatch(/User declined/);
  });

  it('rejects a signer that returns empty XDR', async () => {
    const client = new SwapTradeClient(
      baseConfig({ signTransaction: () => '' }),
      { server: createFakeServer() },
    );

    await expect(client.mint('XLM', TEST_PUBLIC_KEY, 1n)).rejects.toThrow(
      /must return the signed envelope/,
    );
  });

  it('rejects a signer that returns unparseable XDR', async () => {
    const client = new SwapTradeClient(
      baseConfig({ signTransaction: () => 'this-is-not-xdr' }),
      { server: createFakeServer() },
    );

    await expect(client.mint('XLM', TEST_PUBLIC_KEY, 1n)).rejects.toThrow(
      /could not be parsed/,
    );
  });

  it('reports a network-level rejection', async () => {
    const server = createFakeServer({
      sendResult: { status: 'ERROR', hash: 'b'.repeat(64), errorResult: 'txInsufficientFee' },
    });
    const client = new SwapTradeClient(baseConfig(), { server });

    const error = await client.mint('XLM', TEST_PUBLIC_KEY, 1n).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(TransactionFailedError);
    expect((error as TransactionFailedError).status).toBe('ERROR');
  });

  it('reports an on-chain failure after submission', async () => {
    const server = createFakeServer({
      getTransactionResults: [{ status: 'FAILED', resultXdr: 'txFailed' }],
    });
    const client = new SwapTradeClient(baseConfig(), { server });

    await expect(client.mint('XLM', TEST_PUBLIC_KEY, 1n)).rejects.toThrow(
      TransactionFailedError,
    );
  });

  it('maps an on-chain contract error to ContractCallError', async () => {
    const server = createFakeServer({
      getTransactionResults: [
        { status: 'FAILED', resultXdr: 'HostError: Error(Contract, #300)' },
      ],
    });
    const client = new SwapTradeClient(baseConfig(), { server });

    const error = await client.mint('XLM', TEST_PUBLIC_KEY, 1n).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ContractCallError);
    // 300 is RateLimitExceeded.
    expect((error as ContractCallError).contractName).toBe('RateLimitExceeded');
  });

  it('times out rather than polling forever, and reports the hash', async () => {
    const server = createFakeServer({ getTransactionResults: [{ status: 'NOT_FOUND' }] });
    const client = new SwapTradeClient(baseConfig({ pollTimeoutMs: 1 }), { server });

    const error = await client.mint('XLM', TEST_PUBLIC_KEY, 1n).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(TransactionTimeoutError);
    expect((error as TransactionTimeoutError).hash).toBe('a'.repeat(64));
  });

  it('wraps an RPC transport failure as RpcError', async () => {
    const server = createFakeServer({ sendError: new Error('ECONNREFUSED') });
    const client = new SwapTradeClient(baseConfig(), { server });

    await expect(client.mint('XLM', TEST_PUBLIC_KEY, 1n)).rejects.toThrow(RpcError);
  });
});

describe('KYC helpers', () => {
  it('encodes a KYCStatus enum variant and an optional reason', async () => {
    const server = createFakeServer();
    const client = new SwapTradeClient(baseConfig(), { server });

    await client.kycUpdateStatus(TEST_PUBLIC_KEY, TEST_PUBLIC_KEY, 'Verified');

    const tx = server.simulateTransaction.mock.calls[0]![0] as { toXDR(): string };
    const { method, args } = decodeInvocation(tx, client.config.networkPassphrase);
    expect(method).toBe('kyc_update_status');
    // Unit enum variants decode to a single-element array.
    expect(args[2]).toEqual(['Verified']);
    expect(args[3]).toBeNull();
  });

  it('rejects an unknown KYC status', async () => {
    const client = new SwapTradeClient(baseConfig(), { server: createFakeServer() });
    await expect(
      client.kycUpdateStatus(TEST_PUBLIC_KEY, TEST_PUBLIC_KEY, 'Approved' as never),
    ).rejects.toThrow(/Unknown KYC status/);
  });
});

describe('oracle price helpers', () => {
  it('encodes the (Symbol, Symbol) token pair as a tuple', async () => {
    const server = createFakeServer();
    const client = new SwapTradeClient(baseConfig(), { server });

    await client.setPrice('XLM', 'USDCSIM', 1_000_000n);

    const tx = server.simulateTransaction.mock.calls[0]![0] as { toXDR(): string };
    const { method, args } = decodeInvocation(tx, client.config.networkPassphrase);
    expect(method).toBe('set_price');
    expect(args[0]).toEqual(['XLM', 'USDCSIM']);
    expect(args[1]).toBe(1_000_000n);
  });
});
