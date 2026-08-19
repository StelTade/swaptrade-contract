/**
 * App shell.
 *
 * Wires configuration to the workflow hook and lays out the panels. The client
 * is built once per mount so a config mistake surfaces as a checklist rather
 * than a runtime crash on first click.
 */
import { useMemo, useState } from 'react';
import {
  ActivityLog,
  ConnectionPanel,
  SetupChecklist,
  StatePanel,
  WorkflowControls,
} from './components.js';
import { createClientFromEnv } from './config.js';
import { DEFAULT_AMOUNT, DEFAULT_LIMIT_PRICE, useSwapWorkflow } from './useSwapWorkflow.js';
import type { ClientSetup } from './config.js';

/** Parse a decimal string to a positive bigint, or `null` when unusable. */
function parseAmount(raw: string): bigint | null {
  if (!/^\d+$/.test(raw.trim())) return null;
  const value = BigInt(raw.trim());
  return value > 0n ? value : null;
}

export interface AppProps {
  /**
   * Pre-built setup, injected by tests.
   * Production reads the environment instead.
   */
  setup?: ClientSetup;
}

export function App({ setup }: AppProps) {
  // `useMemo` with no deps: read the environment once per mount.
  const resolved = useMemo<ClientSetup>(() => setup ?? createClientFromEnv(), [setup]);

  const [amountIn, setAmountIn] = useState(DEFAULT_AMOUNT.toString());
  const [limitPrice, setLimitPrice] = useState(DEFAULT_LIMIT_PRICE.toString());
  const [inputError, setInputError] = useState<string | null>(null);

  const client = resolved.ok ? resolved.client : null;
  const workflow = useSwapWorkflow(client);

  const handleCreate = () => {
    const amount = parseAmount(amountIn);
    const price = parseAmount(limitPrice);
    if (amount === null || price === null) {
      setInputError('Amount and limit price must be whole numbers greater than zero.');
      return;
    }
    setInputError(null);
    void workflow.create(amount, price);
  };

  const handleFund = () => {
    const amount = parseAmount(amountIn);
    if (amount === null) {
      setInputError('Amount must be a whole number greater than zero.');
      return;
    }
    setInputError(null);
    void workflow.fund(amount);
  };

  return (
    <main className="app">
      <header>
        <h1>SwapTrade demo</h1>
        <p className="muted">
          Create → fund → accept against the SwapTrade Soroban contract, driven entirely
          through <code>@swaptrade/sdk</code>.
        </p>
      </header>

      {!resolved.ok ? (
        <SetupChecklist problems={resolved.problems} />
      ) : (
        <>
          <ConnectionPanel
            publicKey={resolved.client.config.publicKey}
            network={resolved.client.config.networkPassphrase}
            rpcUrl={resolved.client.config.rpcUrl}
            signerKind={resolved.signerKind}
          />

          {inputError && (
            <p role="alert" className="alert" data-testid="input-error">
              {inputError}
            </p>
          )}

          <WorkflowControls
            activeStep={workflow.activeStep}
            busy={workflow.busy}
            canSign={resolved.signerKind !== 'none'}
            amountIn={amountIn}
            limitPrice={limitPrice}
            onAmountInChange={setAmountIn}
            onLimitPriceChange={setLimitPrice}
            onPrepare={() => void workflow.prepare()}
            onCreate={handleCreate}
            onFund={handleFund}
            onAccept={() => void workflow.accept()}
            onRefresh={() => void workflow.refresh()}
          />

          <ActivityLog outcomes={workflow.outcomes} failure={workflow.failure} />
          <StatePanel snapshot={workflow.snapshot} orderId={workflow.orderId} />
        </>
      )}
    </main>
  );
}
