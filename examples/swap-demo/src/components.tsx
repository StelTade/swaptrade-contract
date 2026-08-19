/**
 * Presentational components.
 *
 * These render props and raise callbacks. None of them import the SDK, build a
 * transaction, or know a chain exists — that is the layering issue #254 asks
 * for: React UI -> SDK -> Stellar SDK -> contracts.
 */
import type { SignerKind } from './signer.js';
import type { AccountSnapshot, StepFailure, StepOutcome, WorkflowStep } from './workflow.js';

/** Shorten a hash for display while keeping it verifiable at a glance. */
function abbreviate(hash: string): string {
  return hash.length <= 16 ? hash : `${hash.slice(0, 8)}…${hash.slice(-8)}`;
}

export interface ConnectionPanelProps {
  publicKey: string;
  network: string;
  rpcUrl: string;
  signerKind: SignerKind;
}

/** Shows which account and network the demo is acting on. */
export function ConnectionPanel({
  publicKey,
  network,
  rpcUrl,
  signerKind,
}: ConnectionPanelProps) {
  const signerLabel = {
    'browser-wallet': 'Injected browser wallet',
    none: 'None — read-only',
  }[signerKind];

  return (
    <section aria-labelledby="connection-heading" className="panel">
      <h2 id="connection-heading">Connection</h2>
      <dl>
        <dt>Account</dt>
        <dd data-testid="account">{publicKey}</dd>
        <dt>Network</dt>
        <dd data-testid="network">{network}</dd>
        <dt>RPC</dt>
        <dd data-testid="rpc-url">{rpcUrl}</dd>
        <dt>Signer</dt>
        <dd data-testid="signer">{signerLabel}</dd>
      </dl>
    </section>
  );
}

export interface SetupChecklistProps {
  problems: { variable: string; detail: string }[];
}

/** Rendered instead of the workflow when configuration is incomplete. */
export function SetupChecklist({ problems }: SetupChecklistProps) {
  return (
    <section aria-labelledby="setup-heading" className="panel panel--warning">
      <h2 id="setup-heading">Configuration required</h2>
      <p>
        Copy <code>.env.example</code> to <code>.env.local</code> and set the following before
        the demo can reach a contract:
      </p>
      <ul data-testid="setup-problems">
        {problems.map((problem) => (
          <li key={problem.variable}>
            <code>{problem.variable}</code> — {problem.detail}
          </li>
        ))}
      </ul>
      <p>
        See <code>docs/LOCALNET.md</code> for the full walkthrough.
      </p>
    </section>
  );
}

export interface WorkflowControlsProps {
  activeStep: WorkflowStep | null;
  busy: boolean;
  canSign: boolean;
  amountIn: string;
  limitPrice: string;
  onAmountInChange(value: string): void;
  onLimitPriceChange(value: string): void;
  onPrepare(): void;
  onCreate(): void;
  onFund(): void;
  onAccept(): void;
  onRefresh(): void;
}

/** The four workflow buttons plus the two order inputs. */
export function WorkflowControls({
  activeStep,
  busy,
  canSign,
  amountIn,
  limitPrice,
  onAmountInChange,
  onLimitPriceChange,
  onPrepare,
  onCreate,
  onFund,
  onAccept,
  onRefresh,
}: WorkflowControlsProps) {
  const label = (step: WorkflowStep, text: string) =>
    activeStep === step ? `${text}…` : text;

  return (
    <section aria-labelledby="workflow-heading" className="panel">
      <h2 id="workflow-heading">Workflow</h2>

      {!canSign && (
        <p role="status" className="notice" data-testid="no-signer-notice">
          No wallet detected — install a Stellar browser wallet (such as Freighter) and reload
          to sign transactions. Read-only refresh still works. The demo never accepts a secret
          key, because anything given to the browser is public.
        </p>
      )}

      <div className="field">
        <label htmlFor="amount-in">Amount in (XLM)</label>
        <input
          id="amount-in"
          inputMode="numeric"
          value={amountIn}
          onChange={(event) => onAmountInChange(event.target.value)}
        />
      </div>

      <div className="field">
        <label htmlFor="limit-price">Limit price</label>
        <input
          id="limit-price"
          inputMode="numeric"
          value={limitPrice}
          onChange={(event) => onLimitPriceChange(event.target.value)}
        />
      </div>

      <ol className="steps">
        <li>
          <button type="button" onClick={onPrepare} disabled={busy || !canSign}>
            {label('prepare', '1. Prepare (KYC + price)')}
          </button>
        </li>
        <li>
          <button type="button" onClick={onCreate} disabled={busy || !canSign}>
            {label('create', '2. Create order')}
          </button>
        </li>
        <li>
          <button type="button" onClick={onFund} disabled={busy || !canSign}>
            {label('fund', '3. Fund account')}
          </button>
        </li>
        <li>
          <button type="button" onClick={onAccept} disabled={busy || !canSign}>
            {label('accept', '4. Accept / execute')}
          </button>
        </li>
      </ol>

      <button type="button" onClick={onRefresh} disabled={busy}>
        Refresh state
      </button>
    </section>
  );
}

export interface ActivityLogProps {
  outcomes: StepOutcome[];
  failure: StepFailure | null;
}

/** Transaction hashes, statuses and the most recent failure. */
export function ActivityLog({ outcomes, failure }: ActivityLogProps) {
  return (
    <section aria-labelledby="activity-heading" className="panel">
      <h2 id="activity-heading">Activity</h2>

      {failure && (
        <div role="alert" className="alert" data-testid="failure">
          <strong>{failure.step} failed</strong>
          <p data-testid="failure-message">{failure.message}</p>
          {failure.contractName && (
            <p data-testid="failure-contract">
              Contract error: {failure.contractName}
              {failure.contractCode !== undefined ? ` (#${failure.contractCode})` : ''}
            </p>
          )}
          {failure.code && <p className="muted">Code: {failure.code}</p>}
        </div>
      )}

      {outcomes.length === 0 ? (
        <p className="muted" data-testid="activity-empty">
          Nothing submitted yet.
        </p>
      ) : (
        <ul data-testid="activity-list">
          {outcomes.map((outcome, index) => (
            <li key={`${outcome.step}-${outcome.hash ?? index}`}>
              <strong>{outcome.step}</strong> — {outcome.summary}
              {outcome.status && (
                <span data-testid={`status-${outcome.step}`} className="badge">
                  {outcome.status}
                </span>
              )}
              {outcome.hash && (
                <code title={outcome.hash} data-testid={`hash-${outcome.step}`}>
                  {abbreviate(outcome.hash)}
                </code>
              )}
              {outcome.ledger !== undefined && (
                <span className="muted"> ledger {outcome.ledger}</span>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export interface StatePanelProps {
  snapshot: AccountSnapshot | null;
  orderId: bigint | null;
}

/** On-chain state read back through simulation. */
export function StatePanel({ snapshot, orderId }: StatePanelProps) {
  return (
    <section aria-labelledby="state-heading" className="panel">
      <h2 id="state-heading">On-chain state</h2>

      {orderId !== null && (
        <p data-testid="order-id">
          Order created this session: <strong>#{orderId.toString()}</strong>
        </p>
      )}

      {snapshot === null ? (
        <p className="muted" data-testid="state-empty">
          Press “Refresh state” to read the contract.
        </p>
      ) : (
        <dl>
          <dt>KYC</dt>
          <dd data-testid="kyc">{snapshot.kycVerified ? 'Verified' : 'Not verified'}</dd>
          <dt>XLM balance</dt>
          <dd data-testid="balance">{snapshot.balance.toString()}</dd>
          <dt>Trades</dt>
          <dd data-testid="trade-count">{snapshot.tradeCount}</dd>
          <dt>Volume</dt>
          <dd data-testid="total-volume">{snapshot.totalVolume.toString()}</dd>
          <dt>Open orders</dt>
          <dd data-testid="order-count">{snapshot.orders.length}</dd>
        </dl>
      )}

      {snapshot !== null && snapshot.orders.length > 0 && (
        <ul data-testid="order-list">
          {snapshot.orders.map((order) => (
            <li key={order.orderId.toString()}>
              #{order.orderId.toString()} {order.orderType} {order.tokenIn}→{order.tokenOut}{' '}
              {order.amountIn.toString()} — {order.status}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
