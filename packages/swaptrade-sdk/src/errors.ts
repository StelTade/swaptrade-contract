/**
 * Error taxonomy for the SwapTrade SDK.
 *
 * Every failure the SDK can produce is one of these classes, so callers can
 * branch on `err.code` instead of matching on message strings. The contract
 * error codes mirror `swaptrade-contracts/counter/src/errors.rs`.
 */

/** Discriminator for {@link SwapTradeError} subclasses. */
export type SwapTradeErrorCode =
  | 'CONFIG_INVALID'
  | 'ADDRESS_INVALID'
  | 'CONTRACT_ID_INVALID'
  | 'AMOUNT_INVALID'
  | 'SYMBOL_INVALID'
  | 'SIGNING_FAILED'
  | 'SIMULATION_FAILED'
  | 'RPC_FAILED'
  | 'TRANSACTION_FAILED'
  | 'TRANSACTION_TIMEOUT'
  | 'CONTRACT_ERROR';

/** Base class for all SDK errors. */
export class SwapTradeError extends Error {
  readonly code: SwapTradeErrorCode;
  /** Underlying error, when this wraps a lower-level failure. */
  override readonly cause?: unknown;

  constructor(code: SwapTradeErrorCode, message: string, cause?: unknown) {
    super(message);
    this.name = new.target.name;
    this.code = code;
    this.cause = cause;
    // Keeps `instanceof` reliable when the package is consumed as ES2022 output.
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/** Missing or malformed {@link SwapTradeConfig}. */
export class ConfigError extends SwapTradeError {
  constructor(message: string, cause?: unknown) {
    super('CONFIG_INVALID', message, cause);
  }
}

/** A supplied value is not a valid Stellar account / contract identifier. */
export class ValidationError extends SwapTradeError {
  constructor(
    code: Extract<
      SwapTradeErrorCode,
      'ADDRESS_INVALID' | 'CONTRACT_ID_INVALID' | 'AMOUNT_INVALID' | 'SYMBOL_INVALID'
    >,
    message: string,
    cause?: unknown,
  ) {
    super(code, message, cause);
  }
}

/** The signer rejected the request or returned an unusable signature. */
export class SigningError extends SwapTradeError {
  constructor(message: string, cause?: unknown) {
    super('SIGNING_FAILED', message, cause);
  }
}

/**
 * `simulateTransaction` reported an error.
 *
 * Simulation runs before signing, so this means nothing was submitted and no
 * fee was charged.
 */
export class SimulationError extends SwapTradeError {
  /** Diagnostic events returned by the RPC server, when present. */
  readonly events: readonly string[];

  constructor(message: string, events: readonly string[] = [], cause?: unknown) {
    super('SIMULATION_FAILED', message, cause);
    this.events = events;
  }
}

/** Transport-level failure talking to the Soroban RPC server. */
export class RpcError extends SwapTradeError {
  constructor(message: string, cause?: unknown) {
    super('RPC_FAILED', message, cause);
  }
}

/** The transaction was submitted but did not reach `SUCCESS`. */
export class TransactionFailedError extends SwapTradeError {
  readonly hash?: string;
  readonly status?: string;

  constructor(message: string, hash?: string, status?: string, cause?: unknown) {
    super('TRANSACTION_FAILED', message, cause);
    this.hash = hash;
    this.status = status;
  }
}

/** The transaction did not settle within the configured polling window. */
export class TransactionTimeoutError extends SwapTradeError {
  readonly hash: string;

  constructor(hash: string, timeoutMs: number) {
    super(
      'TRANSACTION_TIMEOUT',
      `Transaction ${hash} did not settle within ${timeoutMs}ms. It may still be applied; re-check the hash before retrying.`,
    );
    this.hash = hash;
  }
}

/**
 * The contract itself returned an `Err(...)`.
 *
 * `contractCode` is the numeric discriminant from the contract's error enum and
 * `contractName` is the resolved name when the code is one the SDK knows.
 */
export class ContractCallError extends SwapTradeError {
  readonly contractCode?: number;
  readonly contractName?: string;

  constructor(message: string, contractCode?: number, contractName?: string, cause?: unknown) {
    super('CONTRACT_ERROR', message, cause);
    this.contractCode = contractCode;
    this.contractName = contractName;
  }
}

/**
 * `SwapTradeError` codes as defined in `counter/src/errors.rs`.
 *
 * Kept in sync manually; the contract is the source of truth. Used only to turn
 * an opaque numeric failure into a readable message.
 */
export const CONTRACT_ERROR_NAMES: Readonly<Record<number, string>> = Object.freeze({
  1: 'NotAdmin',
  2: 'NotAuthorized',
  3: 'InvalidAddress',
  4: 'InvalidMultiSigConfig',
  5: 'MultiSigNotConfigured',
  10: 'TradingPaused',
  11: 'UserFrozen',
  12: 'CircuitBreakerTripped',
  13: 'InvalidPrivateTransaction',
  90: 'ProposalNotFound',
  91: 'ProposalAlreadyExecuted',
  92: 'AlreadyApproved',
  93: 'InsufficientApprovals',
  94: 'TimelockNotElapsed',
  95: 'AlreadyVoted',
  96: 'InsufficientSignatures',
  97: 'QuorumNotReached',
  98: 'ProposalFailed',
  99: 'ProposalCanceled',
  100: 'InvalidAmount',
  101: 'AmountOverflow',
  102: 'InvalidTokenSymbol',
  103: 'InvalidSwapPair',
  104: 'InsufficientBalance',
  105: 'ZeroAmountSwap',
  200: 'InvariantViolation',
  201: 'StalePrice',
  202: 'InvalidPrice',
  203: 'PriceNotSet',
  204: 'OracleNotConfigured',
  205: 'OracleNotActive',
  206: 'CircuitBreakerActive',
  207: 'CircuitBreakerTriggered',
  208: 'InvalidConfig',
  300: 'RateLimitExceeded',
  301: 'SlippageExceeded',
  302: 'Expired',
  400: 'LPPositionNotFound',
  401: 'InsufficientLPTokens',
  500: 'KYCVerificationRequired',
  501: 'NotKYCOperator',
  502: 'InvalidKYCStateTransition',
  503: 'KYCTerminalStateImmutable',
  504: 'SelfVerificationNotAllowed',
  505: 'KYCOverrideNotFound',
  506: 'KYCTimelockNotElapsed',
});

/** Resolve a numeric contract error code to its declared name, if known. */
export function contractErrorName(code: number): string | undefined {
  return CONTRACT_ERROR_NAMES[code];
}
