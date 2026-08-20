import { StrKey } from '@stellar/stellar-sdk';
import { ConfigError, ValidationError } from './errors.js';
import { NETWORKS, type NetworkName, type ResolvedConfig, type SwapTradeConfig } from './types.js';

/** Default fee offered per operation, in stroops (0.1 XLM). */
export const DEFAULT_FEE = '1000000';
/** Default transaction validity window, in seconds. */
export const DEFAULT_TIMEOUT_SECONDS = 60;
/** Default time to wait for a submitted transaction to settle, in milliseconds. */
export const DEFAULT_POLL_TIMEOUT_MS = 30_000;

/** Soroban `Symbol` values are limited to 32 characters. */
const MAX_SYMBOL_LENGTH = 32;
/** `symbol_short!` values — used for asset codes in this contract — allow 9. */
const MAX_SHORT_SYMBOL_LENGTH = 9;

/** Hosts for which plain HTTP is considered safe (local development). */
const LOOPBACK_HOSTS = new Set(['localhost', '127.0.0.1', '::1', '0.0.0.0']);

/** Assert a value is a valid Stellar public key (`G...`). */
export function assertAccountId(value: unknown, label = 'public key'): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new ValidationError('ADDRESS_INVALID', `Invalid ${label}: expected a non-empty string.`);
  }
  if (!StrKey.isValidEd25519PublicKey(value)) {
    throw new ValidationError(
      'ADDRESS_INVALID',
      `Invalid ${label}: "${value}" is not a valid Stellar account ID (expected a G... address).`,
    );
  }
  return value;
}

/** Assert a value is a valid Soroban contract ID (`C...`). */
export function assertContractId(value: unknown): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new ValidationError(
      'CONTRACT_ID_INVALID',
      'Invalid contract ID: expected a non-empty string.',
    );
  }
  if (!StrKey.isValidContract(value)) {
    throw new ValidationError(
      'CONTRACT_ID_INVALID',
      `Invalid contract ID: "${value}" is not a valid Soroban contract ID (expected a C... address).`,
    );
  }
  return value;
}

/**
 * Assert a value is usable as a Soroban `Symbol`.
 *
 * The contract stores asset codes with `symbol_short!`, which caps length at 9;
 * pass `short: false` for the general 32-character `Symbol` limit.
 */
export function assertSymbol(value: unknown, label = 'symbol', short = true): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new ValidationError('SYMBOL_INVALID', `Invalid ${label}: expected a non-empty string.`);
  }
  const limit = short ? MAX_SHORT_SYMBOL_LENGTH : MAX_SYMBOL_LENGTH;
  if (value.length > limit) {
    throw new ValidationError(
      'SYMBOL_INVALID',
      `Invalid ${label}: "${value}" is ${value.length} characters but the contract allows at most ${limit}.`,
    );
  }
  if (!/^[A-Za-z0-9_]+$/.test(value)) {
    throw new ValidationError(
      'SYMBOL_INVALID',
      `Invalid ${label}: "${value}" must contain only letters, digits and underscores.`,
    );
  }
  return value;
}

/** Assert an amount is a strictly positive integer, as the contract requires. */
export function assertPositiveAmount(value: unknown, label = 'amount'): bigint {
  if (typeof value !== 'bigint') {
    throw new ValidationError(
      'AMOUNT_INVALID',
      `Invalid ${label}: expected a bigint, received ${typeof value}. Use BigInt(...) to avoid precision loss on i128 values.`,
    );
  }
  if (value <= 0n) {
    throw new ValidationError('AMOUNT_INVALID', `Invalid ${label}: must be greater than zero.`);
  }
  return value;
}

/** Resolve a {@link NetworkName} to its RPC URL and passphrase. */
export function networkPreset(name: NetworkName): { rpcUrl: string; networkPassphrase: string } {
  const preset = NETWORKS[name];
  if (!preset) {
    throw new ConfigError(
      `Unknown network "${name}". Expected one of: ${Object.keys(NETWORKS).join(', ')}.`,
    );
  }
  return { rpcUrl: preset.rpcUrl, networkPassphrase: preset.networkPassphrase };
}

function isLoopback(rpcUrl: string): boolean {
  try {
    return LOOPBACK_HOSTS.has(new URL(rpcUrl).hostname);
  } catch {
    return false;
  }
}

/**
 * Validate a {@link SwapTradeConfig} and apply defaults.
 *
 * Nothing is silently defaulted that would send traffic to a network the caller
 * did not name: `rpcUrl`, `networkPassphrase` and `contractId` are all required.
 *
 * @throws {ConfigError} when a required field is missing or malformed.
 * @throws {ValidationError} when the contract ID or public key is invalid.
 */
export function resolveConfig(config: SwapTradeConfig): ResolvedConfig {
  if (config === null || typeof config !== 'object') {
    throw new ConfigError('Missing configuration: expected a SwapTradeConfig object.');
  }

  const { rpcUrl, networkPassphrase } = config;

  if (typeof rpcUrl !== 'string' || rpcUrl.trim() === '') {
    throw new ConfigError(
      'Missing "rpcUrl". Set it explicitly (e.g. from NETWORKS.local.rpcUrl or a VITE_SOROBAN_RPC_URL env var).',
    );
  }

  let parsed: URL;
  try {
    parsed = new URL(rpcUrl);
  } catch (cause) {
    throw new ConfigError(`Invalid "rpcUrl": "${rpcUrl}" is not a valid URL.`, cause);
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new ConfigError(
      `Invalid "rpcUrl": protocol "${parsed.protocol}" is not supported, expected http: or https:.`,
    );
  }

  if (typeof networkPassphrase !== 'string' || networkPassphrase.trim() === '') {
    throw new ConfigError(
      'Missing "networkPassphrase". A wrong passphrase produces signatures the network rejects, so it is never defaulted.',
    );
  }

  const allowHttp = config.allowHttp ?? isLoopback(rpcUrl);
  if (parsed.protocol === 'http:' && !allowHttp) {
    throw new ConfigError(
      `Refusing to use plain HTTP for non-local RPC URL "${rpcUrl}". Use https, or set allowHttp: true to override.`,
    );
  }

  const fee = config.fee ?? DEFAULT_FEE;
  if (!/^\d+$/.test(fee)) {
    throw new ConfigError(`Invalid "fee": "${fee}" must be a whole number of stroops.`);
  }

  const timeoutSeconds = config.timeoutSeconds ?? DEFAULT_TIMEOUT_SECONDS;
  if (!Number.isInteger(timeoutSeconds) || timeoutSeconds <= 0) {
    throw new ConfigError('Invalid "timeoutSeconds": must be a positive integer.');
  }

  const pollTimeoutMs = config.pollTimeoutMs ?? DEFAULT_POLL_TIMEOUT_MS;
  if (!Number.isInteger(pollTimeoutMs) || pollTimeoutMs <= 0) {
    throw new ConfigError('Invalid "pollTimeoutMs": must be a positive integer.');
  }

  if (config.signTransaction !== undefined && typeof config.signTransaction !== 'function') {
    throw new ConfigError('Invalid "signTransaction": must be a function when provided.');
  }

  const resolved: ResolvedConfig = {
    rpcUrl,
    networkPassphrase,
    contractId: assertContractId(config.contractId),
    publicKey: assertAccountId(config.publicKey),
    allowHttp,
    fee,
    timeoutSeconds,
    pollTimeoutMs,
    ...(config.signTransaction ? { signTransaction: config.signTransaction } : {}),
  };

  return Object.freeze(resolved);
}
