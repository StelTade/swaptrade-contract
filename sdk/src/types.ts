/**
 * Type definitions for SwapTrade Atomic Swap SDK
 */

import { Address } from "@stellar/stellar-sdk";

/**
 * Lifecycle states for an atomic swap
 */
export enum SwapState {
  Created = "Created",
  Funded = "Funded",
  Accepted = "Accepted",
  Cancelled = "Cancelled",
  Refunded = "Refunded",
}

/**
 * Error codes from the contract
 */
export enum SwapError {
  SwapNotFound = 1,
  Unauthorized = 2,
  MissingTrustline = 3,
  InvalidState = 4,
  InvalidAmount = 5,
  InvalidExpiry = 6,
  Expired = 7,
  SameAsset = 8,
  TransferMismatch = 9,
  TrustlineCheckFailed = 10,
}

/**
 * Swap metadata structure
 */
export interface Swap {
  id: number;
  nonce: number;
  creator: string;
  counterparty: string;
  asset_a: string;
  amount_a: bigint;
  asset_b: string;
  amount_b: bigint;
  expiry: number;
  state: SwapState;
  creator_funded: boolean;
  counterparty_funded: boolean;
  created_at: number;
}

/**
 * Parameters for creating a swap
 */
export interface CreateSwapParams {
  creator: string;
  counterparty: string;
  asset_a: string;
  amount_a: number | bigint;
  asset_b: string;
  amount_b: number | bigint;
  expiry: number;
  nonce: number;
}

/**
 * Parameters for funding a swap
 */
export interface FundSwapParams {
  swap_id: number;
  funder: string;
}

/**
 * Parameters for accepting a swap
 */
export interface AcceptSwapParams {
  swap_id: number;
  acceptor: string;
}

/**
 * Parameters for cancelling a swap
 */
export interface CancelSwapParams {
  swap_id: number;
}

/**
 * Parameters for refunding a swap
 */
export interface RefundSwapParams {
  swap_id: number;
}

/**
 * Network configuration
 */
export interface NetworkConfig {
  rpcUrl: string;
  networkPassphrase: string;
}

/**
 * Transaction result
 */
export interface TransactionResult {
  hash: string;
  status: "SUCCESS" | "FAILED" | "PENDING";
}

/**
 * SDK configuration
 */
export interface SwapTradeSDKConfig {
  contractId: string;
  network: NetworkConfig;
}
