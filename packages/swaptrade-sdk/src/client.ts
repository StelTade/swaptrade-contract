/**
 * `SwapTradeClient` — a thin, typed wrapper over the SwapTrade Soroban contract.
 *
 * The client owns the mechanical parts of a Soroban call (build -> simulate ->
 * sign -> submit -> poll) and exposes one method per contract entry point. It
 * deliberately does not re-implement anything `@stellar/stellar-sdk` already
 * does well; it only removes the boilerplate and adds typing plus a consistent
 * error taxonomy.
 *
 * Method names and argument order mirror `swaptrade-contracts/counter/src/lib.rs`.
 */
import {
  Contract,
  TransactionBuilder,
  rpc as StellarRpc,
  type Account,
  type Transaction,
  type xdr,
} from '@stellar/stellar-sdk';
import {
  assertAccountId,
  assertPositiveAmount,
  assertSymbol,
  resolveConfig,
} from './config.js';
import {
  ConfigError,
  ContractCallError,
  RpcError,
  SigningError,
  SimulationError,
  SwapTradeError,
  TransactionFailedError,
  TransactionTimeoutError,
} from './errors.js';
import {
  accountArg,
  asContractError,
  decodeKycStatus,
  decodeOrder,
  decodePortfolio,
  fromScVal,
  i128ToScVal,
  kycStatusToScVal,
  optionToScVal,
  symbolToScVal,
  tupleToScVal,
  u128ToScVal,
  u32ToScVal,
  u64ToScVal,
} from './scval.js';
import type {
  KYCStatus,
  Order,
  PlaceLimitOrderParams,
  PortfolioSummary,
  ResolvedConfig,
  SimulationResult,
  SwapTradeConfig,
  TransactionResult,
} from './types.js';

/** How long to wait between `getTransaction` polls, in milliseconds. */
const POLL_INTERVAL_MS = 1_000;

/** Minimal surface of the RPC server the client depends on. */
export interface RpcServerLike {
  getAccount(address: string): Promise<unknown>;
  simulateTransaction(tx: Transaction): Promise<unknown>;
  sendTransaction(tx: Transaction): Promise<unknown>;
  getTransaction(hash: string): Promise<unknown>;
}

/** Options for constructing a {@link SwapTradeClient}. */
export interface ClientOptions {
  /**
   * Pre-built RPC server, primarily for tests.
   * When omitted a `rpc.Server` is created from the resolved config.
   */
  server?: RpcServerLike;
}

function errorMessage(value: unknown): string {
  if (value instanceof Error) return value.message;
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

/**
 * Classify an unknown thrown value from the RPC layer.
 *
 * Contract errors are surfaced as {@link ContractCallError} so callers can read
 * `contractCode`; anything else is transport failure.
 */
function classifyRpcFailure(cause: unknown, action: string): SwapTradeError {
  if (cause instanceof SwapTradeError) return cause;
  const message = errorMessage(cause);
  return asContractError(message, cause) ?? new RpcError(`${action}: ${message}`, cause);
}

export class SwapTradeClient {
  readonly config: ResolvedConfig;
  private readonly contract: Contract;
  private readonly server: RpcServerLike;

  /**
   * @param config - Connection, contract and signing configuration.
   * @param options - Optional overrides, mainly for testing.
   * @throws {ConfigError} when required configuration is missing or malformed.
   * @throws {ValidationError} when the contract ID or public key is invalid.
   */
  constructor(config: SwapTradeConfig, options: ClientOptions = {}) {
    this.config = resolveConfig(config);
    this.contract = new Contract(this.config.contractId);
    this.server =
      options.server ??
      (new StellarRpc.Server(this.config.rpcUrl, {
        allowHttp: this.config.allowHttp,
      }) as unknown as RpcServerLike);
  }

  /** Convenience factory; equivalent to `new SwapTradeClient(...)`. */
  static create(config: SwapTradeConfig, options: ClientOptions = {}): SwapTradeClient {
    return new SwapTradeClient(config, options);
  }

  // ── Transaction plumbing ──────────────────────────────────────────────────

  /**
   * Build an unsigned transaction invoking `method` with `args`.
   *
   * Exposed so callers can inspect or externally sign the envelope instead of
   * using {@link invoke}.
   */
  async buildTransaction(method: string, args: xdr.ScVal[] = []): Promise<Transaction> {
    let account: Awaited<ReturnType<RpcServerLike['getAccount']>>;
    try {
      account = await this.server.getAccount(this.config.publicKey);
    } catch (cause) {
      throw classifyRpcFailure(
        cause,
        `Could not load source account ${this.config.publicKey}. Confirm it exists and is funded on this network`,
      );
    }

    return new TransactionBuilder(account as Account, {
      fee: this.config.fee,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(this.config.timeoutSeconds)
      .build();
  }

  /**
   * Simulate a call without submitting it.
   *
   * Useful for read-only contract methods and for previewing whether a
   * state-changing call would succeed. Costs nothing and needs no signature.
   *
   * @throws {SimulationError} when the host reports a simulation error.
   */
  async simulate<T = unknown>(
    method: string,
    args: xdr.ScVal[] = [],
    decode?: (raw: unknown) => T,
  ): Promise<SimulationResult<T>> {
    const tx = await this.buildTransaction(method, args);

    let sim: unknown;
    try {
      sim = await this.server.simulateTransaction(tx);
    } catch (cause) {
      throw classifyRpcFailure(cause, `Simulation of "${method}" failed`);
    }

    if (StellarRpc.Api.isSimulationError(sim as never)) {
      const raw = (sim as { error: string }).error;
      const events = ((sim as { events?: unknown[] }).events ?? []).map(errorMessage);
      throw (
        asContractError(raw) ??
        new SimulationError(`Simulation of "${method}" failed: ${raw}`, events)
      );
    }

    const result = (sim as { result?: { retval?: xdr.ScVal } }).result;
    const raw = result?.retval ? fromScVal(result.retval) : undefined;
    const minResourceFee = (sim as { minResourceFee?: string }).minResourceFee;

    return {
      returnValue: (decode ? decode(raw) : (raw as T)),
      ...(minResourceFee ? { minResourceFee } : {}),
    };
  }

  /**
   * Build, simulate, sign, submit and await a state-changing call.
   *
   * Simulation runs first so authorization and resource footprint are attached
   * before signing, and so a call that cannot succeed fails without spending a
   * fee.
   *
   * @throws {ConfigError} when no `signTransaction` was configured.
   * @throws {SigningError} when the signer rejects or returns unusable XDR.
   * @throws {SimulationError} when the call would fail on-chain.
   * @throws {TransactionFailedError} when the network rejects the transaction.
   * @throws {TransactionTimeoutError} when it does not settle in time.
   */
  async invoke<T = unknown>(
    method: string,
    args: xdr.ScVal[] = [],
    decode?: (raw: unknown) => T,
  ): Promise<TransactionResult<T>> {
    const { signTransaction } = this.config;
    if (!signTransaction) {
      throw new ConfigError(
        `Cannot invoke "${method}": no signTransaction callback was configured. Provide one to send transactions, or use simulate() for read-only calls.`,
      );
    }

    const tx = await this.buildTransaction(method, args);

    // Simulate and assemble so the transaction carries the correct Soroban
    // resource footprint and auth entries before it is signed.
    let sim: unknown;
    try {
      sim = await this.server.simulateTransaction(tx);
    } catch (cause) {
      throw classifyRpcFailure(cause, `Simulation of "${method}" failed`);
    }

    if (StellarRpc.Api.isSimulationError(sim as never)) {
      const raw = (sim as { error: string }).error;
      throw (
        asContractError(raw) ??
        new SimulationError(`Simulation of "${method}" failed: ${raw}`)
      );
    }

    const prepared = StellarRpc.assembleTransaction(tx, sim as never).build();

    let signedXdr: string;
    try {
      signedXdr = await signTransaction(prepared.toXDR(), {
        networkPassphrase: this.config.networkPassphrase,
        address: this.config.publicKey,
      });
    } catch (cause) {
      throw new SigningError(
        `Signing "${method}" was rejected or failed: ${errorMessage(cause)}`,
        cause,
      );
    }

    if (typeof signedXdr !== 'string' || signedXdr.trim() === '') {
      throw new SigningError(
        `Signing "${method}" returned no transaction XDR. The signer must return the signed envelope as a base64 string.`,
      );
    }

    let signed: Transaction;
    try {
      signed = TransactionBuilder.fromXDR(
        signedXdr,
        this.config.networkPassphrase,
      ) as Transaction;
    } catch (cause) {
      throw new SigningError(
        `Signer returned XDR that could not be parsed for "${method}": ${errorMessage(cause)}`,
        cause,
      );
    }

    let sent: { status?: string; hash?: string; errorResult?: unknown };
    try {
      sent = (await this.server.sendTransaction(signed)) as typeof sent;
    } catch (cause) {
      throw classifyRpcFailure(cause, `Submitting "${method}" failed`);
    }

    if (sent.status === 'ERROR' || sent.status === 'DUPLICATE') {
      throw new TransactionFailedError(
        `Network rejected "${method}" with status ${sent.status}: ${errorMessage(sent.errorResult)}`,
        sent.hash,
        sent.status,
      );
    }

    const hash = sent.hash;
    if (!hash) {
      throw new TransactionFailedError(
        `Submitting "${method}" returned no transaction hash.`,
        undefined,
        sent.status,
      );
    }

    return this.awaitTransaction<T>(hash, method, decode);
  }

  /**
   * Poll `getTransaction` until the transaction settles.
   *
   * @throws {TransactionTimeoutError} when the poll window elapses first.
   */
  private async awaitTransaction<T>(
    hash: string,
    method: string,
    decode?: (raw: unknown) => T,
  ): Promise<TransactionResult<T>> {
    const deadline = Date.now() + this.config.pollTimeoutMs;

    for (;;) {
      let result: {
        status?: string;
        returnValue?: xdr.ScVal;
        ledger?: number;
        resultXdr?: unknown;
      };
      try {
        result = (await this.server.getTransaction(hash)) as typeof result;
      } catch (cause) {
        throw classifyRpcFailure(cause, `Polling transaction ${hash} failed`);
      }

      const status = result.status ?? 'NOT_FOUND';

      if (status === 'SUCCESS') {
        const raw = result.returnValue ? fromScVal(result.returnValue) : undefined;
        return {
          hash,
          status,
          ...(result.ledger !== undefined ? { ledger: result.ledger } : {}),
          ...(raw !== undefined
            ? { returnValue: (decode ? decode(raw) : (raw as T)) }
            : {}),
        };
      }

      if (status === 'FAILED') {
        const detail = errorMessage(result.resultXdr);
        throw (
          asContractError(detail) ??
          new TransactionFailedError(
            `Transaction ${hash} for "${method}" failed on-chain: ${detail}`,
            hash,
            status,
          )
        );
      }

      if (Date.now() >= deadline) {
        throw new TransactionTimeoutError(hash, this.config.pollTimeoutMs);
      }

      await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
    }
  }

  // ── Contract lifecycle ────────────────────────────────────────────────────

  /** `initialize()` — set the stored contract version after deployment. */
  async initialize(): Promise<TransactionResult<void>> {
    return this.invoke<void>('initialize');
  }

  /** `get_contract_version() -> u32`. */
  async getContractVersion(): Promise<number> {
    const { returnValue } = await this.simulate<number>('get_contract_version', [], (raw) =>
      Number(raw ?? 0),
    );
    return returnValue;
  }

  // ── Balances ──────────────────────────────────────────────────────────────

  /**
   * `mint(token: Symbol, to: Address, amount: i128)`.
   *
   * The demo contract mints simulated balances, which is how a test account is
   * funded before trading.
   */
  async mint(token: string, to: string, amount: bigint): Promise<TransactionResult<void>> {
    return this.invoke<void>('mint', [
      symbolToScVal(token, 'token'),
      accountArg(to, 'recipient'),
      i128ToScVal(assertPositiveAmount(amount)),
    ]);
  }

  /**
   * `balance_of(token: Symbol, user: Address) -> i128`.
   *
   * Read-only, so it is simulated rather than submitted.
   */
  async balanceOf(token: string, user?: string): Promise<bigint> {
    const address = user ?? this.config.publicKey;
    const { returnValue } = await this.simulate<bigint>(
      'balance_of',
      [symbolToScVal(token, 'token'), accountArg(address, 'user')],
      (raw) => (typeof raw === 'bigint' ? raw : BigInt(Number(raw ?? 0))),
    );
    return returnValue;
  }

  /** `get_portfolio(user: Address) -> (u32, i128)`. */
  async getPortfolio(user?: string): Promise<PortfolioSummary> {
    const address = user ?? this.config.publicKey;
    const { returnValue } = await this.simulate<PortfolioSummary>(
      'get_portfolio',
      [accountArg(address, 'user')],
      decodePortfolio,
    );
    return returnValue;
  }

  // ── Orders: the create -> fund -> accept demo path ────────────────────────

  /**
   * `place_limit_order(token_in, token_out, amount_in, limit_price, expires_at, user) -> u64`
   *
   * This is the contract's "create an offer" primitive and returns the new
   * order ID.
   */
  async placeLimitOrder(params: PlaceLimitOrderParams): Promise<TransactionResult<bigint>> {
    const user = params.user ?? this.config.publicKey;
    return this.invoke<bigint>(
      'place_limit_order',
      [
        symbolToScVal(params.tokenIn, 'tokenIn'),
        symbolToScVal(params.tokenOut, 'tokenOut'),
        i128ToScVal(assertPositiveAmount(params.amountIn, 'amountIn')),
        u128ToScVal(assertPositiveAmount(params.limitPrice, 'limitPrice')),
        optionToScVal(params.expiresAt, u64ToScVal),
        accountArg(user, 'user'),
      ],
      (raw) => (typeof raw === 'bigint' ? raw : BigInt(Number(raw ?? 0))),
    );
  }

  /** `get_order(order_id: u64) -> Order`. */
  async getOrder(orderId: bigint): Promise<Order> {
    const { returnValue } = await this.simulate<Order>(
      'get_order',
      [u64ToScVal(orderId)],
      decodeOrder,
    );
    return returnValue;
  }

  /** `get_user_orders(user: Address) -> Vec<Order>`. */
  async getUserOrders(user?: string): Promise<Order[]> {
    const address = user ?? this.config.publicKey;
    const { returnValue } = await this.simulate<Order[]>(
      'get_user_orders',
      [accountArg(address, 'user')],
      (raw) => (Array.isArray(raw) ? raw.map(decodeOrder) : []),
    );
    return returnValue;
  }

  /**
   * `execute_due_orders() -> Vec<u64>`
   *
   * Settles every order whose conditions are met and returns the executed IDs —
   * the counterparty half of the demo flow.
   */
  async executeDueOrders(): Promise<TransactionResult<bigint[]>> {
    return this.invoke<bigint[]>('execute_due_orders', [], (raw) =>
      Array.isArray(raw) ? raw.map((id) => (typeof id === 'bigint' ? id : BigInt(Number(id)))) : [],
    );
  }

  /** `cancel_order(order_id: u64, user: Address)`. */
  async cancelOrder(orderId: bigint, user?: string): Promise<TransactionResult<void>> {
    const address = user ?? this.config.publicKey;
    return this.invoke<void>('cancel_order', [
      u64ToScVal(orderId),
      accountArg(address, 'user'),
    ]);
  }

  // ── Direct swaps ──────────────────────────────────────────────────────────

  /**
   * `swap(from: Symbol, to: Symbol, amount: i128, user: Address) -> i128`
   *
   * Reverts on any failure; see {@link safeSwap} for the non-reverting variant.
   */
  async swap(
    from: string,
    to: string,
    amount: bigint,
    user?: string,
  ): Promise<TransactionResult<bigint>> {
    const address = user ?? this.config.publicKey;
    if (assertSymbol(from, 'from') === assertSymbol(to, 'to')) {
      throw new ContractCallError('Cannot swap a token for itself: "from" and "to" must differ.');
    }
    return this.invoke<bigint>(
      'swap',
      [
        symbolToScVal(from, 'from'),
        symbolToScVal(to, 'to'),
        i128ToScVal(assertPositiveAmount(amount)),
        accountArg(address, 'user'),
      ],
      (raw) => (typeof raw === 'bigint' ? raw : BigInt(Number(raw ?? 0))),
    );
  }

  /**
   * `safe_swap(from, to, amount, user, deadline) -> i128`
   *
   * Returns `0` instead of reverting when the swap cannot proceed.
   */
  async safeSwap(
    from: string,
    to: string,
    amount: bigint,
    deadline: bigint,
    user?: string,
  ): Promise<TransactionResult<bigint>> {
    const address = user ?? this.config.publicKey;
    return this.invoke<bigint>(
      'safe_swap',
      [
        symbolToScVal(from, 'from'),
        symbolToScVal(to, 'to'),
        i128ToScVal(assertPositiveAmount(amount)),
        accountArg(address, 'user'),
        u64ToScVal(deadline),
      ],
      (raw) => (typeof raw === 'bigint' ? raw : BigInt(Number(raw ?? 0))),
    );
  }

  // ── Oracle prices ─────────────────────────────────────────────────────────

  /**
   * `set_price(token_pair: (Symbol, Symbol), price: u128)`
   *
   * Orders and swaps consult the oracle, so a localnet demo must seed a price
   * before trading.
   */
  async setPrice(
    from: string,
    to: string,
    price: bigint,
  ): Promise<TransactionResult<void>> {
    return this.invoke<void>('set_price', [
      this.tokenPair(from, to),
      u128ToScVal(assertPositiveAmount(price, 'price')),
    ]);
  }

  /** `get_current_price(token_pair: (Symbol, Symbol)) -> u128`. */
  async getCurrentPrice(from: string, to: string): Promise<bigint> {
    const { returnValue } = await this.simulate<bigint>(
      'get_current_price',
      [this.tokenPair(from, to)],
      (raw) => (typeof raw === 'bigint' ? raw : BigInt(Number(raw ?? 0))),
    );
    return returnValue;
  }

  /** Encode a `(Symbol, Symbol)` token-pair tuple argument. */
  private tokenPair(from: string, to: string): xdr.ScVal {
    return tupleToScVal([
      symbolToScVal(from, 'from'),
      symbolToScVal(to, 'to'),
    ]);
  }

  // ── KYC ───────────────────────────────────────────────────────────────────

  /** `kyc_is_verified(user: Address) -> bool`. */
  async kycIsVerified(user?: string): Promise<boolean> {
    const address = user ?? this.config.publicKey;
    const { returnValue } = await this.simulate<boolean>(
      'kyc_is_verified',
      [accountArg(address, 'user')],
      (raw) => raw === true,
    );
    return returnValue;
  }

  /** `kyc_submit(user: Address)` — user-initiated KYC submission. */
  async kycSubmit(user?: string): Promise<TransactionResult<void>> {
    const address = user ?? this.config.publicKey;
    return this.invoke<void>('kyc_submit', [accountArg(address, 'user')]);
  }

  /** `kyc_add_operator(admin: Address, operator: Address)` — admin only. */
  async kycAddOperator(admin: string, operator: string): Promise<TransactionResult<void>> {
    return this.invoke<void>('kyc_add_operator', [
      accountArg(admin, 'admin'),
      accountArg(operator, 'operator'),
    ]);
  }

  /**
   * `kyc_update_status(operator, user, new_status, reason)` — operator only.
   *
   * Trading entry points are gated on `Verified`, so the demo must walk an
   * account through `Pending -> InReview -> Verified`.
   */
  async kycUpdateStatus(
    operator: string,
    user: string,
    newStatus: KYCStatus,
    reason?: string,
  ): Promise<TransactionResult<void>> {
    return this.invoke<void>('kyc_update_status', [
      accountArg(operator, 'operator'),
      accountArg(user, 'user'),
      kycStatusToScVal(newStatus),
      optionToScVal(reason, (r) => symbolToScVal(r, 'reason', false)),
    ]);
  }

  /** `kyc_get_record(user: Address) -> KYCRecord`; returns the status field. */
  async kycGetStatus(user?: string): Promise<KYCStatus> {
    const address = user ?? this.config.publicKey;
    const { returnValue } = await this.simulate<KYCStatus>(
      'kyc_get_record',
      [accountArg(address, 'user')],
      (raw) => {
        const record = (raw ?? {}) as Record<string, unknown>;
        return decodeKycStatus(record['status']);
      },
    );
    return returnValue;
  }

  // ── Admin ─────────────────────────────────────────────────────────────────

  /** `pause_trading(caller: Address) -> bool` — admin only. */
  async pauseTrading(caller?: string): Promise<TransactionResult<boolean>> {
    const address = caller ?? this.config.publicKey;
    return this.invoke<boolean>(
      'pause_trading',
      [accountArg(address, 'caller')],
      (raw) => raw === true,
    );
  }

  /** `resume_trading(caller: Address) -> bool` — admin only. */
  async resumeTrading(caller?: string): Promise<TransactionResult<boolean>> {
    const address = caller ?? this.config.publicKey;
    return this.invoke<boolean>(
      'resume_trading',
      [accountArg(address, 'caller')],
      (raw) => raw === true,
    );
  }

  /** `get_user_tier(user: Address) -> UserTier`. */
  async getUserTier(user?: string): Promise<string> {
    const address = user ?? this.config.publicKey;
    const { returnValue } = await this.simulate<string>(
      'get_user_tier',
      [accountArg(address, 'user')],
      (raw) => (Array.isArray(raw) ? String(raw[0]) : String(raw ?? 'Unknown')),
    );
    return returnValue;
  }

  /** `set_max_slippage_bps(bps: u32)`. */
  async setMaxSlippageBps(bps: number): Promise<TransactionResult<void>> {
    if (!Number.isInteger(bps) || bps < 0 || bps > 10_000) {
      throw new ContractCallError('Slippage must be an integer between 0 and 10000 basis points.');
    }
    return this.invoke<void>('set_max_slippage_bps', [u32ToScVal(bps)]);
  }
}
