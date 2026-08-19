/**
 * `@swaptrade/sdk` — a lightweight TypeScript SDK for the SwapTrade Soroban
 * contracts.
 *
 * @example
 * ```ts
 * import { SwapTradeClient, NETWORKS, keypairSigner } from '@swaptrade/sdk';
 *
 * const client = new SwapTradeClient({
 *   ...NETWORKS.local,
 *   contractId: process.env.SWAPTRADE_CONTRACT_ID!,
 *   publicKey: process.env.SWAPTRADE_PUBLIC_KEY!,
 *   signTransaction: keypairSigner(process.env.SWAPTRADE_SECRET_KEY!),
 * });
 *
 * const { returnValue: orderId } = await client.placeLimitOrder({
 *   tokenIn: 'XLM',
 *   tokenOut: 'USDCSIM',
 *   amountIn: 1_000n,
 *   limitPrice: 1_000_000n,
 * });
 * ```
 */

export { SwapTradeClient } from './client.js';
export type { ClientOptions, RpcServerLike } from './client.js';

export {
  DEFAULT_FEE,
  DEFAULT_POLL_TIMEOUT_MS,
  DEFAULT_TIMEOUT_SECONDS,
  assertAccountId,
  assertContractId,
  assertPositiveAmount,
  assertSymbol,
  networkPreset,
  resolveConfig,
} from './config.js';

export {
  CONTRACT_ERROR_NAMES,
  ConfigError,
  ContractCallError,
  RpcError,
  SigningError,
  SimulationError,
  SwapTradeError,
  TransactionFailedError,
  TransactionTimeoutError,
  ValidationError,
  contractErrorName,
} from './errors.js';
export type { SwapTradeErrorCode } from './errors.js';

export { browserWalletSigner, keypairSigner } from './signers.js';
export type { BrowserWallet } from './signers.js';

export {
  KYC_STATUSES,
  NETWORKS,
  ORDER_STATUSES,
  ORDER_TYPES,
} from './types.js';
export type {
  KYCStatus,
  NetworkName,
  Order,
  OrderStatus,
  OrderType,
  PlaceLimitOrderParams,
  PortfolioSummary,
  ResolvedConfig,
  SignTransaction,
  SimulationResult,
  SwapTradeConfig,
  SwapParams,
  TransactionResult,
} from './types.js';

export {
  addressToScVal,
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
  u32ToScVal,
  u64ToScVal,
  unitEnumToScVal,
} from './scval.js';
