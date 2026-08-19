/**
 * Workflow state for the demo.
 *
 * All mutation lives here; components read this hook's return value and call its
 * actions. Keeping the state machine in one place is what lets the components
 * stay free of chain logic.
 */
import { useCallback, useMemo, useState } from 'react';
import type { SwapTradeClient } from '@swaptrade/sdk';
import {
  type AccountSnapshot,
  type StepFailure,
  type StepOutcome,
  type WorkflowStep,
  acceptOrders,
  createOrder,
  fundAccount,
  prepareAccount,
  readSnapshot,
  toStepFailure,
} from './workflow.js';

/** Default demo amounts. Small values keep localnet ledgers readable. */
export const DEFAULT_AMOUNT = 1_000n;
export const DEFAULT_LIMIT_PRICE = 1_000_000n;
export const DEFAULT_ORACLE_PRICE = 1_000_000n;

export interface WorkflowState {
  /** Step currently running, or `null` when idle. */
  activeStep: WorkflowStep | null;
  /** Completed step outcomes, oldest first. */
  outcomes: StepOutcome[];
  /** Most recent failure, cleared when a new step starts. */
  failure: StepFailure | null;
  /** ID of the order created in this session, when known. */
  orderId: bigint | null;
  /** Latest on-chain snapshot, or `null` before the first refresh. */
  snapshot: AccountSnapshot | null;
  /** True while any step or refresh is in flight. */
  busy: boolean;
}

export interface WorkflowActions {
  prepare(): Promise<void>;
  create(amountIn: bigint, limitPrice: bigint): Promise<void>;
  fund(amount: bigint): Promise<void>;
  accept(): Promise<void>;
  refresh(): Promise<void>;
  reset(): void;
}

const INITIAL: WorkflowState = {
  activeStep: null,
  outcomes: [],
  failure: null,
  orderId: null,
  snapshot: null,
  busy: false,
};

/**
 * Drive the create -> fund -> accept workflow.
 *
 * @param client - A configured client, or `null` when configuration is invalid.
 */
export function useSwapWorkflow(client: SwapTradeClient | null): WorkflowState & WorkflowActions {
  const [state, setState] = useState<WorkflowState>(INITIAL);

  /**
   * Run one step with uniform bookkeeping.
   *
   * Marking the step active before awaiting and clearing it in `finally` means a
   * thrown error can never leave the UI stuck on a spinner.
   */
  const run = useCallback(
    async (step: WorkflowStep, action: (client: SwapTradeClient) => Promise<void>) => {
      if (!client) return;

      setState((prev) => ({ ...prev, activeStep: step, failure: null, busy: true }));
      try {
        await action(client);
      } catch (error) {
        setState((prev) => ({ ...prev, failure: toStepFailure(step, error) }));
      } finally {
        setState((prev) => ({ ...prev, activeStep: null, busy: false }));
      }
    },
    [client],
  );

  const record = useCallback((outcome: StepOutcome, orderId?: bigint) => {
    setState((prev) => ({
      ...prev,
      outcomes: [...prev.outcomes, outcome],
      ...(orderId !== undefined ? { orderId } : {}),
    }));
  }, []);

  const actions = useMemo<WorkflowActions>(
    () => ({
      prepare: () =>
        run('prepare', async (c) => {
          record(await prepareAccount(c, DEFAULT_ORACLE_PRICE));
        }),

      create: (amountIn, limitPrice) =>
        run('create', async (c) => {
          const result = await createOrder(c, { amountIn, limitPrice });
          record(result.outcome, result.orderId);
        }),

      fund: (amount) =>
        run('fund', async (c) => {
          record(await fundAccount(c, amount));
        }),

      accept: () =>
        run('accept', async (c) => {
          const result = await acceptOrders(c);
          record(result.outcome);
        }),

      // Refresh is read-only, so it reports failures without claiming a step.
      refresh: async () => {
        if (!client) return;
        setState((prev) => ({ ...prev, busy: true }));
        try {
          const snapshot = await readSnapshot(client);
          setState((prev) => ({ ...prev, snapshot, failure: null }));
        } catch (error) {
          setState((prev) => ({ ...prev, failure: toStepFailure('prepare', error) }));
        } finally {
          setState((prev) => ({ ...prev, busy: false }));
        }
      },

      reset: () => setState(INITIAL),
    }),
    [client, record, run],
  );

  return { ...state, ...actions };
}
