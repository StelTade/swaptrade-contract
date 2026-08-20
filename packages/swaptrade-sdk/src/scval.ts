/**
 * Conversion between TypeScript values and Soroban `ScVal`s.
 *
 * This is the only module that needs to know how the contract encodes its
 * arguments and return values, so the mapping stays reviewable in one place.
 */
import { Address, nativeToScVal, scValToNative, xdr } from '@stellar/stellar-sdk';
import { assertAccountId, assertSymbol } from './config.js';
import { ContractCallError, contractErrorName } from './errors.js';
import {
  KYC_STATUSES,
  ORDER_STATUSES,
  ORDER_TYPES,
  type KYCStatus,
  type Order,
  type OrderStatus,
  type OrderType,
  type PortfolioSummary,
} from './types.js';

/** Encode an account or contract address as an `ScVal`. */
export function addressToScVal(value: string): xdr.ScVal {
  return new Address(value).toScVal();
}

/** Encode a Soroban `Symbol`. */
export function symbolToScVal(value: string, label = 'symbol', short = true): xdr.ScVal {
  return nativeToScVal(assertSymbol(value, label, short), { type: 'symbol' });
}

/** Encode a signed 128-bit integer (`i128`). */
export function i128ToScVal(value: bigint): xdr.ScVal {
  return nativeToScVal(value, { type: 'i128' });
}

/** Encode an unsigned 128-bit integer (`u128`). */
export function u128ToScVal(value: bigint): xdr.ScVal {
  return nativeToScVal(value, { type: 'u128' });
}

/** Encode an unsigned 64-bit integer (`u64`). */
export function u64ToScVal(value: bigint): xdr.ScVal {
  return nativeToScVal(value, { type: 'u64' });
}

/** Encode an unsigned 32-bit integer (`u32`). */
export function u32ToScVal(value: number): xdr.ScVal {
  return nativeToScVal(value, { type: 'u32' });
}

/**
 * Encode a Rust `Option<T>`.
 *
 * `Some(v)` is the inner value and `None` is `ScVal::Void`, which is how the
 * Soroban host represents optionals.
 */
export function optionToScVal<T>(
  value: T | undefined | null,
  encode: (inner: T) => xdr.ScVal,
): xdr.ScVal {
  return value === undefined || value === null ? xdr.ScVal.scvVoid() : encode(value);
}

/**
 * Encode a fieldless Rust enum variant.
 *
 * `#[contracttype]` encodes these as a single-element vector holding the variant
 * name as a symbol.
 */
export function unitEnumToScVal(variant: string): xdr.ScVal {
  return xdr.ScVal.scvVec([nativeToScVal(variant, { type: 'symbol' })]);
}

/**
 * Encode a Rust tuple, e.g. the `(Symbol, Symbol)` token pair used by the
 * oracle entry points. Tuples are encoded as a vector of their elements.
 */
export function tupleToScVal(elements: xdr.ScVal[]): xdr.ScVal {
  return xdr.ScVal.scvVec(elements);
}

/** Encode a `KYCStatus` argument. */
export function kycStatusToScVal(status: KYCStatus): xdr.ScVal {
  if (!KYC_STATUSES.includes(status)) {
    throw new ContractCallError(
      `Unknown KYC status "${status}". Expected one of: ${KYC_STATUSES.join(', ')}.`,
    );
  }
  return unitEnumToScVal(status);
}

/** Decode an `ScVal` into a plain JavaScript value. */
export function fromScVal<T = unknown>(value: xdr.ScVal): T {
  return scValToNative(value) as T;
}

function asBigInt(value: unknown): bigint {
  if (typeof value === 'bigint') return value;
  if (typeof value === 'number') return BigInt(value);
  if (typeof value === 'string' && value !== '') return BigInt(value);
  return 0n;
}

function optionalBigInt(value: unknown): bigint | undefined {
  return value === undefined || value === null ? undefined : asBigInt(value);
}

/**
 * Normalise a decoded unit-enum value to one of `allowed`.
 *
 * `scValToNative` yields either the variant name or a single-element array
 * depending on the encoding, and an index when the enum carries explicit
 * discriminants — all three are handled here.
 */
function decodeUnitEnum<T extends string>(
  value: unknown,
  allowed: readonly T[],
  label: string,
): T {
  const raw = Array.isArray(value) ? value[0] : value;

  if (typeof raw === 'string' && (allowed as readonly string[]).includes(raw)) {
    return raw as T;
  }
  if (typeof raw === 'number' || typeof raw === 'bigint') {
    const variant = allowed[Number(raw)];
    if (variant) return variant;
  }
  throw new ContractCallError(
    `Could not decode ${label} from contract value ${JSON.stringify(String(raw))}.`,
  );
}

/** Decode an `OrderStatus`. */
export function decodeOrderStatus(value: unknown): OrderStatus {
  return decodeUnitEnum(value, ORDER_STATUSES, 'order status');
}

/** Decode an `OrderType`. */
export function decodeOrderType(value: unknown): OrderType {
  return decodeUnitEnum(value, ORDER_TYPES, 'order type');
}

/** Decode a `KYCStatus`. */
export function decodeKycStatus(value: unknown): KYCStatus {
  return decodeUnitEnum(value, KYC_STATUSES, 'KYC status');
}

/**
 * Decode the contract's `Order` struct into an {@link Order}.
 *
 * Accepts an already-native object so it can be unit-tested without building
 * XDR by hand.
 */
export function decodeOrder(raw: unknown): Order {
  if (raw === null || typeof raw !== 'object') {
    throw new ContractCallError('Expected an order struct from the contract.');
  }
  const o = raw as Record<string, unknown>;

  const owner = o['owner'];
  const order: Order = {
    orderId: asBigInt(o['order_id']),
    owner: typeof owner === 'string' ? owner : String(owner ?? ''),
    orderType: decodeOrderType(o['order_type']),
    tokenIn: String(o['token_in'] ?? ''),
    tokenOut: String(o['token_out'] ?? ''),
    amountIn: asBigInt(o['amount_in']),
    amountFilled: asBigInt(o['amount_filled']),
    status: decodeOrderStatus(o['status']),
    createdAt: asBigInt(o['created_at']),
  };

  // Optional contract fields are only set when present, so consumers can rely
  // on `undefined` meaning "None" rather than "zero".
  const limitPrice = optionalBigInt(o['limit_price']);
  if (limitPrice !== undefined) order.limitPrice = limitPrice;
  const triggerPrice = optionalBigInt(o['trigger_price']);
  if (triggerPrice !== undefined) order.triggerPrice = triggerPrice;
  const expiresAt = optionalBigInt(o['expires_at']);
  if (expiresAt !== undefined) order.expiresAt = expiresAt;
  const filledAt = optionalBigInt(o['filled_at']);
  if (filledAt !== undefined) order.filledAt = filledAt;
  const intervalSecs = optionalBigInt(o['interval_secs']);
  if (intervalSecs !== undefined) order.intervalSecs = intervalSecs;
  const remaining = optionalBigInt(o['remaining_occurrences']);
  if (remaining !== undefined) order.remainingOccurrences = remaining;
  const nextRun = optionalBigInt(o['next_run']);
  if (nextRun !== undefined) order.nextRun = nextRun;

  return order;
}

/** Decode the `(u32, i128)` tuple returned by `get_portfolio`. */
export function decodePortfolio(raw: unknown): PortfolioSummary {
  if (!Array.isArray(raw) || raw.length < 2) {
    throw new ContractCallError(
      'Expected get_portfolio to return a (trade_count, total_volume) tuple.',
    );
  }
  return {
    tradeCount: Number(asBigInt(raw[0])),
    totalVolume: asBigInt(raw[1]),
  };
}

/**
 * Extract a contract error code from an RPC/simulation error payload.
 *
 * The host reports these as `Error(Contract, #N)`; returns `undefined` when the
 * message is not a contract error.
 */
export function parseContractErrorCode(message: string): number | undefined {
  const match = /Error\(Contract,\s*#(\d+)\)/.exec(message);
  if (match?.[1]) return Number(match[1]);
  const alt = /ContractError\((\d+)\)/.exec(message);
  return alt?.[1] ? Number(alt[1]) : undefined;
}

/**
 * Turn a raw host error message into a {@link ContractCallError} when it encodes
 * a contract error, otherwise return `undefined` so the caller can classify it.
 */
export function asContractError(message: string, cause?: unknown): ContractCallError | undefined {
  const code = parseContractErrorCode(message);
  if (code === undefined) return undefined;
  const name = contractErrorName(code);
  return new ContractCallError(
    name
      ? `Contract returned ${name} (code ${code}).`
      : `Contract returned error code ${code}.`,
    code,
    name,
    cause,
  );
}

/** Validate and encode an account argument in one step. */
export function accountArg(value: string, label = 'address'): xdr.ScVal {
  return addressToScVal(assertAccountId(value, label));
}
