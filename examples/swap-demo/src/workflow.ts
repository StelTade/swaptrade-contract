/**
 * The demo workflow, expressed as SDK calls.
 *
 * This module is the whole boundary between the UI and the chain: components
 * import from here and never touch `@swaptrade/sdk`'s transaction plumbing or
 * `@stellar/stellar-sdk` at all.
 *
 * ## Why these contract methods
 *
 * `swaptrade-contracts/counter` has no `create_swap` / `fund_swap` /
 * `accept_swap` trio. The create -> fund -> accept shape from issue #254 is
 * therefore mapped onto the primitives the contract actually exposes:
 *
 * | Demo step | Contract method       |
 * |-----------|-----------------------|
 * | Prepare   | `kyc_submit`, `kyc_update_status`, `set_price` |
 * | Create    | `place_limit_order`   |
 * | Fund      | `mint`                |
 * | Accept    | `execute_due_orders`  |
 *
 * Trading entry points are gated by `require_authenticated_verified_user`, so
 * the prepare step is a precondition rather than decoration.
 */
import {
  SwapTradeError,
  type Order,
  type SwapTradeClient,
  type TransactionResult,
} from '@swaptrade/sdk';

/** Steps the UI can run, in the order they must happen. */
export const WORKFLOW_STEPS = ['prepare', 'create', 'fund', 'accept'] as const;

export type WorkflowStep = (typeof WORKFLOW_STEPS)[number];

/** Outcome of one step, in terms the UI can render directly. */
export interface StepOutcome {
  step: WorkflowStep;
  /** Human-readable summary of what happened on success. */
  summary: string;
  /** Transaction hash, when the step submitted one. */
  hash?: string;
  /** Final RPC status, when the step submitted a transaction. */
  status?: string;
  /** Ledger the transaction landed in, when reported. */
  ledger?: number;
}

/** A failure, already translated out of SDK internals. */
export interface StepFailure {
  step: WorkflowStep;
  message: string;
  /** SDK error discriminator, e.g. `CONTRACT_ERROR`. */
  code?: string;
  /** Contract error name from `errors.rs`, when the chain rejected the call. */
  contractName?: string;
  /** Numeric contract error code, when present. */
  contractCode?: number;
}

/** Token pair the demo trades. Both are simulated assets minted by the contract. */
export const TOKEN_IN = 'XLM';
export const TOKEN_OUT = 'USDCSIM';

/** Parameters for the create step. */
export interface CreateOrderInput {
  amountIn: bigint;
  limitPrice: bigint;
}

/**
 * Convert any thrown value into a renderable failure.
 *
 * Contract errors carry the name from `errors.rs`, which is far more actionable
 * than the raw `Error(Contract, #500)` the host produces.
 */
export function toStepFailure(step: WorkflowStep, error: unknown): StepFailure {
  if (error instanceof SwapTradeError) {
    const withContract = error as SwapTradeError & {
      contractName?: string;
      contractCode?: number;
    };
    return {
      step,
      message: error.message,
      code: error.code,
      ...(withContract.contractName ? { contractName: withContract.contractName } : {}),
      ...(withContract.contractCode !== undefined
        ? { contractCode: withContract.contractCode }
        : {}),
    };
  }
  return { step, message: error instanceof Error ? error.message : String(error) };
}

/** Shape a `TransactionResult` into a `StepOutcome`. */
function outcome(
  step: WorkflowStep,
  summary: string,
  result: TransactionResult<unknown>,
): StepOutcome {
  return {
    step,
    summary,
    hash: result.hash,
    status: result.status,
    ...(result.ledger !== undefined ? { ledger: result.ledger } : {}),
  };
}

/**
 * PREPARE: satisfy the contract's preconditions.
 *
 * Verifies KYC for the demo account and seeds the oracle price for the pair.
 * Both are idempotent enough to re-run: an already-verified account short-
 * circuits, and `set_price` overwrites.
 */
export async function prepareAccount(
  client: SwapTradeClient,
  price: bigint,
): Promise<StepOutcome> {
  const account = client.config.publicKey;

  const alreadyVerified = await client.kycIsVerified(account);
  if (!alreadyVerified) {
    await client.kycSubmit(account);
    // The demo account is its own KYC operator on localnet, where it is also
    // the contract admin. On a shared network an operator would do this.
    await client.kycUpdateStatus(account, account, 'Verified');
  }

  const result = await client.setPrice(TOKEN_IN, TOKEN_OUT, price);
  return outcome(
    'prepare',
    alreadyVerified
      ? `Account already KYC-verified; oracle price set to ${price}.`
      : `Account KYC-verified and oracle price set to ${price}.`,
    result,
  );
}

/** CREATE: place a limit order and return its ID alongside the outcome. */
export async function createOrder(
  client: SwapTradeClient,
  input: CreateOrderInput,
): Promise<{ outcome: StepOutcome; orderId?: bigint }> {
  const result = await client.placeLimitOrder({
    tokenIn: TOKEN_IN,
    tokenOut: TOKEN_OUT,
    amountIn: input.amountIn,
    limitPrice: input.limitPrice,
  });

  const orderId = result.returnValue;
  return {
    outcome: outcome(
      'create',
      orderId === undefined
        ? `Limit order placed for ${input.amountIn} ${TOKEN_IN}.`
        : `Limit order #${orderId} placed for ${input.amountIn} ${TOKEN_IN}.`,
      result,
    ),
    ...(orderId !== undefined ? { orderId } : {}),
  };
}

/** FUND: mint the input token to the account so the order can settle. */
export async function fundAccount(
  client: SwapTradeClient,
  amount: bigint,
): Promise<StepOutcome> {
  const result = await client.mint(TOKEN_IN, client.config.publicKey, amount);
  return outcome('fund', `Minted ${amount} ${TOKEN_IN} to the demo account.`, result);
}

/** ACCEPT: execute every order whose conditions are met. */
export async function acceptOrders(
  client: SwapTradeClient,
): Promise<{ outcome: StepOutcome; executedIds: bigint[] }> {
  const result = await client.executeDueOrders();
  const executedIds = result.returnValue ?? [];

  return {
    outcome: outcome(
      'accept',
      executedIds.length === 0
        ? 'No orders were due for execution.'
        : `Executed order(s): ${executedIds.map((id) => `#${id}`).join(', ')}.`,
      result,
    ),
    executedIds,
  };
}

/** Read-only snapshot of on-chain state, for the status panel. */
export interface AccountSnapshot {
  balance: bigint;
  tradeCount: number;
  totalVolume: bigint;
  kycVerified: boolean;
  orders: Order[];
}

/**
 * Read current state without submitting anything.
 *
 * Every call here is simulate-only, so refreshing costs no fee and needs no
 * signer.
 */
export async function readSnapshot(client: SwapTradeClient): Promise<AccountSnapshot> {
  const [balance, portfolio, kycVerified, orders] = await Promise.all([
    client.balanceOf(TOKEN_IN),
    client.getPortfolio(),
    client.kycIsVerified(),
    client.getUserOrders(),
  ]);

  return {
    balance,
    tradeCount: portfolio.tradeCount,
    totalVolume: portfolio.totalVolume,
    kycVerified,
    orders,
  };
}
