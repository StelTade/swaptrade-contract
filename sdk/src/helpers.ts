/**
 * Helper functions for Stellar SDK integration
 */

import {
  nativeToScVal,
  xdr,
} from "@stellar/stellar-sdk";

/**
 * Convert a string address to ScVal
 */
export function toAddress(address: string): xdr.ScVal {
  return nativeToScVal(address, { type: "address" });
}

/**
 * Convert a number to u64 ScVal
 */
export function toU64(n: number): xdr.ScVal {
  return nativeToScVal(n, { type: "u64" });
}

/**
 * Convert a number or bigint to i128 ScVal
 */
export function toI128(n: number | bigint): xdr.ScVal {
  return nativeToScVal(n, { type: "i128" });
}

/**
 * Convert a boolean to ScVal
 */
export function toBool(b: boolean): xdr.ScVal {
  return nativeToScVal(b, { type: "bool" });
}

/**
 * Calculate expiry timestamp (seconds from epoch)
 */
export function calculateExpiry(secondsFromNow: number): number {
  return Math.floor(Date.now() / 1000) + secondsFromNow;
}

/**
 * Generate a random nonce
 */
export function generateNonce(): number {
  return Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
}

/**
 * Validate address format (basic check)
 */
export function isValidAddress(address: string): boolean {
  // Stellar addresses are 56 characters starting with 'G'
  return /^G[A-Z0-9]{55}$/.test(address) || /^C[A-Z0-9]{55}$/.test(address);
}

/**
 * Validate amount is positive
 */
export function isValidAmount(amount: number | bigint): boolean {
  const num = typeof amount === "bigint" ? Number(amount) : amount;
  return num > 0;
}

/**
 * Validate expiry is in the future
 */
export function isValidExpiry(expiry: number, minExpirySeconds: number = 300): boolean {
  const now = Math.floor(Date.now() / 1000);
  return expiry > now + minExpirySeconds;
}
