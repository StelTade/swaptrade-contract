/**
 * Configuration validation.
 *
 * These tests pin the contract that the SDK never silently guesses a network,
 * contract or identity, because a wrong default here means signing against the
 * wrong chain.
 */
import { describe, expect, it } from 'vitest';
import {
  ConfigError,
  DEFAULT_FEE,
  DEFAULT_POLL_TIMEOUT_MS,
  DEFAULT_TIMEOUT_SECONDS,
  NETWORKS,
  SwapTradeClient,
  ValidationError,
  assertPositiveAmount,
  assertSymbol,
  networkPreset,
  resolveConfig,
} from '../src/index.js';
import { TEST_CONTRACT_ID, TEST_PUBLIC_KEY, baseConfig } from './helpers.js';

describe('resolveConfig', () => {
  it('applies documented defaults for optional fields', () => {
    const resolved = resolveConfig({
      rpcUrl: NETWORKS.local.rpcUrl,
      networkPassphrase: NETWORKS.local.networkPassphrase,
      contractId: TEST_CONTRACT_ID,
      publicKey: TEST_PUBLIC_KEY,
    });

    expect(resolved.fee).toBe(DEFAULT_FEE);
    expect(resolved.timeoutSeconds).toBe(DEFAULT_TIMEOUT_SECONDS);
    expect(resolved.pollTimeoutMs).toBe(DEFAULT_POLL_TIMEOUT_MS);
  });

  it('returns a frozen object so config cannot drift after construction', () => {
    const resolved = resolveConfig(baseConfig());
    expect(Object.isFrozen(resolved)).toBe(true);
  });

  it.each([
    ['missing config entirely', undefined],
    ['a null config', null],
  ])('rejects %s', (_label, value) => {
    expect(() => resolveConfig(value as never)).toThrow(ConfigError);
  });

  it('rejects a missing rpcUrl instead of defaulting to a public network', () => {
    expect(() => resolveConfig(baseConfig({ rpcUrl: undefined as never }))).toThrow(
      /Missing "rpcUrl"/,
    );
  });

  it('rejects a malformed rpcUrl', () => {
    expect(() => resolveConfig(baseConfig({ rpcUrl: 'not-a-url' }))).toThrow(
      /not a valid URL/,
    );
  });

  it('rejects a non-HTTP protocol', () => {
    expect(() => resolveConfig(baseConfig({ rpcUrl: 'ftp://example.org' }))).toThrow(
      /is not supported/,
    );
  });

  it('rejects a missing networkPassphrase, which would produce invalid signatures', () => {
    expect(() =>
      resolveConfig(baseConfig({ networkPassphrase: undefined as never })),
    ).toThrow(/Missing "networkPassphrase"/);
  });

  it('allows plain HTTP for loopback so localnet works without extra flags', () => {
    const resolved = resolveConfig(baseConfig({ rpcUrl: 'http://localhost:8000/soroban/rpc' }));
    expect(resolved.allowHttp).toBe(true);
  });

  it('refuses plain HTTP for a remote host unless explicitly allowed', () => {
    expect(() =>
      resolveConfig(baseConfig({ rpcUrl: 'http://rpc.example.org' })),
    ).toThrow(/Refusing to use plain HTTP/);

    const forced = resolveConfig(
      baseConfig({ rpcUrl: 'http://rpc.example.org', allowHttp: true }),
    );
    expect(forced.allowHttp).toBe(true);
  });

  it('rejects an invalid contract ID', () => {
    expect(() => resolveConfig(baseConfig({ contractId: 'not-a-contract' }))).toThrow(
      ValidationError,
    );
    expect(() => resolveConfig(baseConfig({ contractId: TEST_PUBLIC_KEY }))).toThrow(
      /not a valid Soroban contract ID/,
    );
  });

  it('rejects an invalid public key', () => {
    expect(() => resolveConfig(baseConfig({ publicKey: 'GBADKEY' }))).toThrow(
      /not a valid Stellar account ID/,
    );
    // A contract ID is not a valid source account.
    expect(() => resolveConfig(baseConfig({ publicKey: TEST_CONTRACT_ID }))).toThrow(
      ValidationError,
    );
  });

  it.each([
    ['a non-numeric fee', { fee: '10.5' }],
    ['a zero timeout', { timeoutSeconds: 0 }],
    ['a negative poll timeout', { pollTimeoutMs: -1 }],
    ['a non-function signer', { signTransaction: 'nope' as never }],
  ])('rejects %s', (_label, overrides) => {
    expect(() => resolveConfig(baseConfig(overrides))).toThrow(ConfigError);
  });
});

describe('networkPreset', () => {
  it('exposes the values declared in soroban.toml', () => {
    expect(networkPreset('local')).toEqual({
      rpcUrl: 'http://localhost:8000/soroban/rpc',
      networkPassphrase: 'Standalone Network ; February 2017',
    });
    expect(networkPreset('testnet').networkPassphrase).toBe('Test SDF Network ; September 2015');
  });

  it('rejects an unknown network name', () => {
    expect(() => networkPreset('staging' as never)).toThrow(ConfigError);
  });
});

describe('assertSymbol', () => {
  it('accepts a valid short symbol', () => {
    expect(assertSymbol('USDCSIM')).toBe('USDCSIM');
  });

  it('rejects symbols longer than the contract allows', () => {
    // The contract stores asset codes with `symbol_short!`, capped at 9 chars.
    expect(() => assertSymbol('TOOLONGSYMBOL')).toThrow(/at most 9/);
    // The general Symbol limit is 32.
    expect(() => assertSymbol('A'.repeat(33), 'reason', false)).toThrow(/at most 32/);
  });

  it('rejects symbols with characters Soroban does not permit', () => {
    expect(() => assertSymbol('BAD-SYM')).toThrow(/letters, digits and underscores/);
  });
});

describe('assertPositiveAmount', () => {
  it('requires bigint to avoid silent i128 precision loss', () => {
    expect(() => assertPositiveAmount(100 as never)).toThrow(/expected a bigint/);
  });

  it('rejects zero and negative amounts', () => {
    expect(() => assertPositiveAmount(0n)).toThrow(/greater than zero/);
    expect(() => assertPositiveAmount(-5n)).toThrow(/greater than zero/);
  });

  it('preserves large i128 values exactly', () => {
    const large = 170141183460469231731687303715884105727n;
    expect(assertPositiveAmount(large)).toBe(large);
  });
});

describe('SwapTradeClient construction', () => {
  it('exposes the resolved config', () => {
    const client = new SwapTradeClient(baseConfig());
    expect(client.config.contractId).toBe(TEST_CONTRACT_ID);
    expect(client.config.publicKey).toBe(TEST_PUBLIC_KEY);
  });

  it('supports the static factory', () => {
    expect(SwapTradeClient.create(baseConfig())).toBeInstanceOf(SwapTradeClient);
  });

  it('fails fast on invalid configuration rather than at first call', () => {
    expect(() => new SwapTradeClient(baseConfig({ contractId: 'nope' }))).toThrow(
      ValidationError,
    );
  });
});
