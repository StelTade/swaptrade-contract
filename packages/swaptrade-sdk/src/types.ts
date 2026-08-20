/**
 * Public types for the SwapTrade SDK.
 *
 * Shapes here mirror the on-chain types in `swaptrade-contracts/counter`:
 *   - `Order` / `OrderType` / `OrderStatus` -> `counter/src/orders.rs`
 *   - `KYCStatus`                           -> `counter/src/kyc.rs`
 *   - network defaults                      -> `soroban.toml`
 */

/** Well-known network presets, taken verbatim from `soroban.toml`. */
export const NETWORKS = Object.freeze({
  local: Object.freeze({
    rpcUrl: 'http://localhost:8000/soroban/rpc',
    networkPassphrase: 'Standalone Network ; February 2017',
  }),
  testnet: Object.freeze({
    rpcUrl: 'https://soroban-testnet.stellar.org',
    networkPassphrase: 'Test SDF Network ; September 2015',
  }),
  mainnet: Object.freeze({
    rpcUrl: 'https://soroban.stellar.org',
    networkPassphrase: 'Public Global Stellar Network ; September 2015',
  }),
});

/** Name of a preset in {@link NETWORKS}. */
export type NetworkName = keyof typeof NETWORKS;

/**
 * Signs a transaction envelope.
 *
 * The XDR string in and out keeps the SDK agnostic about *how* signing happens,
 * so a Freighter-style browser wallet and a local keypair satisfy the same
 * interface. Implementations must reject rather than return unsigned XDR.
 *
 * @param xdr - Base64 transaction envelope to sign.
 * @param context - Network the transaction is bound to.
 * @returns The signed envelope as base64 XDR.
 */
export type SignTransaction = (
  xdr: string,
  context: { networkPassphrase: string; address: string },
) => Promise<string> | string;

/** Configuration for {@link SwapTradeClient}. */
export interface SwapTradeConfig {
  /** Soroban RPC endpoint. Required; never defaulted to a public network. */
  rpcUrl: string;
  /** Network passphrase the RPC server is running. */
  networkPassphrase: string;
  /** Contract ID (`C...`) of the deployed SwapTrade contract. */
  contractId: string;
  /** Public key (`G...`) used as source account for calls. */
  publicKey: string;
  /** Signer callback. Read-only queries work without it. */
  signTransaction?: SignTransaction;
  /**
   * Whether to allow plain-HTTP RPC URLs.
   * Defaults to `true` only for loopback hosts, so localnet works out of the
   * box while a non-TLS remote endpoint is rejected.
   */
  allowHttp?: boolean;
  /** Fee in stroops offered per operation. Defaults to 1_000_000 (0.1 XLM). */
  fee?: string;
  /** Seconds the built transaction stays valid. Defaults to 60. */
  timeoutSeconds?: number;
  /** Milliseconds to wait for a submitted transaction to settle. Defaults to 30_000. */
  pollTimeoutMs?: number;
}

/** Fully-resolved configuration, after defaults are applied and validated. */
export interface ResolvedConfig {
  readonly rpcUrl: string;
  readonly networkPassphrase: string;
  readonly contractId: string;
  readonly publicKey: string;
  readonly signTransaction?: SignTransaction;
  readonly allowHttp: boolean;
  readonly fee: string;
  readonly timeoutSeconds: number;
  readonly pollTimeoutMs: number;
}

/** Result of a state-changing contract call. */
export interface TransactionResult<T = unknown> {
  /** Transaction hash, present once submitted. */
  hash: string;
  /** Final RPC status, e.g. `SUCCESS`. */
  status: string;
  /** Decoded contract return value, when the call returns one. */
  returnValue?: T;
  /** Ledger the transaction was applied in, when reported. */
  ledger?: number;
}

/** Outcome of a simulate-only call. */
export interface SimulationResult<T = unknown> {
  /** Decoded return value produced by simulation. */
  returnValue: T;
  /** Minimum resource fee the RPC server computed, in stroops. */
  minResourceFee?: string;
}

/**
 * Order lifecycle states, mirroring `OrderStatus` in `counter/src/orders.rs`.
 *
 * Declared in the contract's variant order because Soroban encodes unit enum
 * variants by index.
 */
export const ORDER_STATUSES = [
  'Pending',
  'Filled',
  'Cancelled',
  'Expired',
  'PartiallyFilled',
  'Scheduled',
] as const;

export type OrderStatus = (typeof ORDER_STATUSES)[number];

/** Order kinds, mirroring `OrderType` in `counter/src/orders.rs`. */
export const ORDER_TYPES = ['Market', 'Limit', 'StopLoss', 'StopLimit'] as const;

export type OrderType = (typeof ORDER_TYPES)[number];

/**
 * An on-chain order, mirroring `Order` in `counter/src/orders.rs`.
 *
 * `i128` and `u128` contract fields are surfaced as `bigint` to avoid the
 * precision loss a `number` would introduce.
 */
export interface Order {
  orderId: bigint;
  owner: string;
  orderType: OrderType;
  tokenIn: string;
  tokenOut: string;
  amountIn: bigint;
  amountFilled: bigint;
  limitPrice?: bigint;
  triggerPrice?: bigint;
  status: OrderStatus;
  createdAt: bigint;
  expiresAt?: bigint;
  filledAt?: bigint;
  intervalSecs?: bigint;
  remainingOccurrences?: bigint;
  nextRun?: bigint;
}

/**
 * KYC states, mirroring `KYCStatus` in `counter/src/kyc.rs`.
 *
 * The contract assigns explicit discriminants `0..=5`; this array is ordered to
 * match so the index is the discriminant. `Verified` and `Rejected` are terminal
 * states in the contract's state machine.
 */
export const KYC_STATUSES = [
  'Unverified',
  'Pending',
  'InReview',
  'AdditionalInfoRequired',
  'Verified',
  'Rejected',
] as const;

export type KYCStatus = (typeof KYC_STATUSES)[number];

/** Portfolio summary returned by `get_portfolio` as a `(u32, i128)` tuple. */
export interface PortfolioSummary {
  /** Number of trades recorded for the account. */
  tradeCount: number;
  /** Total traded volume. */
  totalVolume: bigint;
}

/** Parameters for placing a limit order via `place_limit_order`. */
export interface PlaceLimitOrderParams {
  /** Symbol being sold, e.g. `XLM`. Max 9 characters (Soroban `Symbol`). */
  tokenIn: string;
  /** Symbol being bought, e.g. `USDCSIM`. */
  tokenOut: string;
  /** Amount of `tokenIn` to sell. Must be positive. */
  amountIn: bigint;
  /** Minimum acceptable price, scaled by the contract's `PRECISION`. */
  limitPrice: bigint;
  /** Optional expiry as a unix timestamp; omit for no expiry. */
  expiresAt?: bigint;
  /** Account placing the order. Defaults to the configured public key. */
  user?: string;
}

/** Parameters for `swap` / `safe_swap`. */
export interface SwapParams {
  from: string;
  to: string;
  amount: bigint;
  user?: string;
  /** Only used by `safe_swap`: unix timestamp after which the swap is void. */
  deadline?: bigint;
}
