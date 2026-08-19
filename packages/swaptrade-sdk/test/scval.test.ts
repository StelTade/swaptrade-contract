/**
 * ScVal encoding/decoding.
 *
 * These assertions round-trip through the real `@stellar/stellar-sdk` codec, so
 * they verify the SDK's mapping against the contract's actual wire format rather
 * than against a re-implementation of it.
 */
import { nativeToScVal, xdr } from '@stellar/stellar-sdk';
import { describe, expect, it } from 'vitest';
import {
  ContractCallError,
  contractErrorName,
  decodeKycStatus,
  decodeOrder,
  decodeOrderStatus,
  decodeOrderType,
  decodePortfolio,
  fromScVal,
  i128ToScVal,
  kycStatusToScVal,
  optionToScVal,
  parseContractErrorCode,
  symbolToScVal,
  tupleToScVal,
  u128ToScVal,
  u64ToScVal,
  unitEnumToScVal,
} from '../src/index.js';

describe('scalar encoding round-trips', () => {
  it('encodes symbols', () => {
    expect(fromScVal(symbolToScVal('USDCSIM', 'token'))).toBe('USDCSIM');
  });

  it('preserves the full i128 range', () => {
    const max = 170141183460469231731687303715884105727n;
    expect(fromScVal(i128ToScVal(max))).toBe(max);
    expect(fromScVal(i128ToScVal(-max))).toBe(-max);
  });

  it('preserves large u128 values', () => {
    const value = 340282366920938463463374607431768211455n;
    expect(fromScVal(u128ToScVal(value))).toBe(value);
  });

  it('encodes u64', () => {
    expect(fromScVal(u64ToScVal(1_800_000_000n))).toBe(1_800_000_000n);
  });
});

describe('Option encoding', () => {
  it('encodes None as void', () => {
    expect(optionToScVal(undefined, u64ToScVal).switch().name).toBe('scvVoid');
    expect(optionToScVal(null, u64ToScVal).switch().name).toBe('scvVoid');
  });

  it('encodes Some as the inner value', () => {
    expect(fromScVal(optionToScVal(9n, u64ToScVal))).toBe(9n);
  });

  it('treats 0 as Some(0) rather than None', () => {
    // A falsy-but-present value must not collapse to None.
    expect(fromScVal(optionToScVal(0n, u64ToScVal))).toBe(0n);
  });
});

describe('enum and tuple encoding', () => {
  it('encodes a unit enum variant as a single-element vector', () => {
    expect(fromScVal(unitEnumToScVal('Verified'))).toEqual(['Verified']);
  });

  it('encodes a token pair tuple', () => {
    const pair = tupleToScVal([symbolToScVal('XLM', 'a'), symbolToScVal('USDCSIM', 'b')]);
    expect(fromScVal(pair)).toEqual(['XLM', 'USDCSIM']);
  });

  it('encodes a valid KYC status and rejects an invalid one', () => {
    expect(fromScVal(kycStatusToScVal('Verified'))).toEqual(['Verified']);
    expect(() => kycStatusToScVal('Approved' as never)).toThrow(ContractCallError);
  });
});

describe('unit enum decoding', () => {
  it('decodes from a variant name', () => {
    expect(decodeOrderStatus('Filled')).toBe('Filled');
    expect(decodeOrderType('Limit')).toBe('Limit');
    expect(decodeKycStatus('Rejected')).toBe('Rejected');
  });

  it('decodes from the single-element vector form', () => {
    expect(decodeOrderStatus(['Cancelled'])).toBe('Cancelled');
  });

  it('decodes from a numeric discriminant', () => {
    // KYCStatus assigns Verified = 4 in counter/src/kyc.rs.
    expect(decodeKycStatus(4)).toBe('Verified');
    expect(decodeOrderStatus(0)).toBe('Pending');
  });

  it('rejects an unrecognised variant instead of guessing', () => {
    expect(() => decodeOrderStatus('Bogus')).toThrow(ContractCallError);
    expect(() => decodeKycStatus(99)).toThrow(/Could not decode/);
  });
});

describe('decodeOrder', () => {
  const raw = {
    order_id: 7n,
    owner: 'GDVEU3DD4KOFECV66VIHWEZOYX4ZKR3WV27L464SIIPOU2IUI3JCZA57',
    order_type: 'Limit',
    token_in: 'XLM',
    token_out: 'USDCSIM',
    amount_in: 1_000n,
    amount_filled: 0n,
    limit_price: 1_000_000n,
    trigger_price: null,
    status: 'Pending',
    created_at: 1_700_000_000n,
    expires_at: null,
    filled_at: null,
    interval_secs: null,
    remaining_occurrences: null,
    next_run: null,
  };

  it('maps snake_case contract fields to camelCase', () => {
    const order = decodeOrder(raw);
    expect(order.orderId).toBe(7n);
    expect(order.tokenIn).toBe('XLM');
    expect(order.tokenOut).toBe('USDCSIM');
    expect(order.amountIn).toBe(1_000n);
    expect(order.orderType).toBe('Limit');
    expect(order.status).toBe('Pending');
  });

  it('represents contract None as undefined, not zero', () => {
    // Collapsing None to 0 would make "no expiry" look like "expired at epoch".
    const order = decodeOrder(raw);
    expect(order.expiresAt).toBeUndefined();
    expect(order.triggerPrice).toBeUndefined();
    expect(order.limitPrice).toBe(1_000_000n);
  });

  it('decodes present optional fields', () => {
    const order = decodeOrder({ ...raw, expires_at: 1_800_000_000n });
    expect(order.expiresAt).toBe(1_800_000_000n);
  });

  it('rejects a non-object payload', () => {
    expect(() => decodeOrder(null)).toThrow(ContractCallError);
  });
});

describe('decodePortfolio', () => {
  it('decodes the (u32, i128) tuple returned by get_portfolio', () => {
    expect(decodePortfolio([4, 9_999n])).toEqual({ tradeCount: 4, totalVolume: 9_999n });
  });

  it('round-trips through real XDR', () => {
    const encoded = tupleToScVal([
      nativeToScVal(2, { type: 'u32' }),
      i128ToScVal(500n),
    ]);
    expect(decodePortfolio(fromScVal(encoded))).toEqual({
      tradeCount: 2,
      totalVolume: 500n,
    });
  });

  it('rejects a malformed tuple', () => {
    expect(() => decodePortfolio([1])).toThrow(/tuple/);
    expect(() => decodePortfolio('nope')).toThrow(ContractCallError);
  });
});

describe('contract error parsing', () => {
  it.each([
    ['HostError: Error(Contract, #500)', 500],
    ['... Error(Contract, #300) ...', 300],
    ['ContractError(104)', 104],
  ])('extracts a code from %s', (message, expected) => {
    expect(parseContractErrorCode(message)).toBe(expected);
  });

  it('returns undefined for a non-contract error', () => {
    expect(parseContractErrorCode('connection refused')).toBeUndefined();
  });

  it('resolves codes to the names declared in errors.rs', () => {
    expect(contractErrorName(500)).toBe('KYCVerificationRequired');
    expect(contractErrorName(301)).toBe('SlippageExceeded');
    expect(contractErrorName(10)).toBe('TradingPaused');
    expect(contractErrorName(4242)).toBeUndefined();
  });
});

describe('void decoding', () => {
  it('decodes scvVoid to null', () => {
    expect(fromScVal(xdr.ScVal.scvVoid())).toBeNull();
  });
});
