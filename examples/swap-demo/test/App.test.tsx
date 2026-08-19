/**
 * Demo behaviour, from the user's point of view.
 *
 * Assertions go through rendered text and roles rather than component internals:
 * no test reaches into state, props, or the hook. The client is faked at the SDK
 * boundary, so the workflow mapping (create -> place_limit_order, fund -> mint,
 * accept -> execute_due_orders) is genuinely exercised.
 */
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ContractCallError, SigningError } from '@swaptrade/sdk';
import { describe, expect, it } from 'vitest';
import { App } from '../src/App.js';
import {
  DEMO_ACCOUNT,
  createFakeClient,
  fakeOrder,
  fakeSetup,
} from './fakeClient.js';

/** Render with a fake client and return it for call assertions. */
function renderApp(options: Parameters<typeof createFakeClient>[0] = {}) {
  const client = createFakeClient(options);
  render(<App setup={fakeSetup(client)} />);
  return client;
}

describe('configuration', () => {
  it('shows a setup checklist instead of the workflow when config is missing', () => {
    render(
      <App
        setup={{
          ok: false,
          problems: [{ variable: 'VITE_CONTRACT_ID', detail: 'Deploy first.' }],
        }}
      />,
    );

    expect(screen.getByText('Configuration required')).toBeInTheDocument();
    expect(screen.getByText('VITE_CONTRACT_ID')).toBeInTheDocument();
    // The workflow must not be offered against a contract that isn't configured.
    expect(screen.queryByRole('button', { name: /Create order/ })).not.toBeInTheDocument();
  });

  it('shows the connected account and network', () => {
    renderApp();
    expect(screen.getByTestId('account')).toHaveTextContent(DEMO_ACCOUNT);
    expect(screen.getByTestId('network')).toHaveTextContent('Standalone Network');
  });

  it('disables signing actions but keeps refresh available without a signer', () => {
    const client = createFakeClient();
    render(<App setup={fakeSetup(client, 'none')} />);

    expect(screen.getByTestId('no-signer-notice')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Create order/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Refresh state' })).toBeEnabled();
  });

  it('names the browser wallet as the signer and never offers a key input', () => {
    renderApp();

    expect(screen.getByTestId('signer')).toHaveTextContent('Injected browser wallet');

    // The security property, asserted at the surface a user touches: there is no
    // field that could accept a secret key. A regression that added one would
    // fail here before it reached a bundle.
    for (const field of screen.getAllByRole('textbox')) {
      expect(field).toHaveAttribute('id');
      expect(['amount-in', 'limit-price']).toContain(field.getAttribute('id'));
    }
    expect(document.querySelector('input[type="password"]')).toBeNull();
    expect(screen.queryByLabelText(/secret|private key|seed/i)).not.toBeInTheDocument();
  });

  it('directs a user without a wallet to install one rather than paste a key', () => {
    render(<App setup={fakeSetup(createFakeClient(), 'none')} />);

    const notice = screen.getByTestId('no-signer-notice');
    expect(notice).toHaveTextContent(/wallet/i);
    // The remedy offered must never be "supply a key" — in a variable or a field.
    expect(notice).not.toHaveTextContent(/VITE_[A-Z0-9_]*(SECRET|PRIVATE|SEED)/);
    expect(notice).not.toHaveTextContent(/paste|enter.*key/i);
  });
});

describe('create -> fund -> accept', () => {
  it('PREPARE verifies KYC and seeds the oracle price', async () => {
    const client = renderApp({ kycVerified: false });

    await userEvent.click(screen.getByRole('button', { name: /Prepare/ }));

    await waitFor(() => expect(screen.getByTestId('activity-list')).toBeInTheDocument());
    expect(client.kycSubmit).toHaveBeenCalledWith(DEMO_ACCOUNT);
    expect(client.kycUpdateStatus).toHaveBeenCalledWith(DEMO_ACCOUNT, DEMO_ACCOUNT, 'Verified');
    expect(client.setPrice).toHaveBeenCalledWith('XLM', 'USDCSIM', 1_000_000n);
    expect(screen.getByTestId('status-prepare')).toHaveTextContent('SUCCESS');
  });

  it('PREPARE skips re-verification for an already-verified account', async () => {
    const client = renderApp({ kycVerified: true });

    await userEvent.click(screen.getByRole('button', { name: /Prepare/ }));

    await waitFor(() => expect(client.setPrice).toHaveBeenCalled());
    expect(client.kycSubmit).not.toHaveBeenCalled();
    expect(screen.getByText(/already KYC-verified/)).toBeInTheDocument();
  });

  it('CREATE places a limit order with the entered amounts and shows the order ID', async () => {
    const client = renderApp({ placedOrderId: 42n });

    await userEvent.clear(screen.getByLabelText('Amount in (XLM)'));
    await userEvent.type(screen.getByLabelText('Amount in (XLM)'), '2500');
    await userEvent.click(screen.getByRole('button', { name: /Create order/ }));

    await waitFor(() => expect(screen.getByTestId('order-id')).toBeInTheDocument());
    expect(client.placeLimitOrder).toHaveBeenCalledWith({
      tokenIn: 'XLM',
      tokenOut: 'USDCSIM',
      amountIn: 2_500n,
      limitPrice: 1_000_000n,
    });
    expect(screen.getByTestId('order-id')).toHaveTextContent('#42');
  });

  it('FUND mints the entered amount to the connected account', async () => {
    const client = renderApp();

    await userEvent.click(screen.getByRole('button', { name: /Fund account/ }));

    await waitFor(() => expect(client.mint).toHaveBeenCalled());
    expect(client.mint).toHaveBeenCalledWith('XLM', DEMO_ACCOUNT, 1_000n);
  });

  it('ACCEPT executes due orders and lists the executed IDs', async () => {
    const client = renderApp({ executedIds: [7n, 8n] });

    await userEvent.click(screen.getByRole('button', { name: /Accept/ }));

    await waitFor(() => expect(client.executeDueOrders).toHaveBeenCalled());
    expect(screen.getByText(/Executed order\(s\): #7, #8/)).toBeInTheDocument();
  });

  it('reports when nothing was due rather than implying success', async () => {
    renderApp({ executedIds: [] });

    await userEvent.click(screen.getByRole('button', { name: /Accept/ }));

    await waitFor(() =>
      expect(screen.getByText(/No orders were due for execution\./)).toBeInTheDocument(),
    );
  });

  it('accumulates each step in the activity log with its transaction hash', async () => {
    renderApp({ kycVerified: true });

    await userEvent.click(screen.getByRole('button', { name: /Prepare/ }));
    await waitFor(() => expect(screen.getByTestId('hash-prepare')).toBeInTheDocument());
    await userEvent.click(screen.getByRole('button', { name: /Create order/ }));
    await waitFor(() => expect(screen.getByTestId('hash-create')).toBeInTheDocument());
    await userEvent.click(screen.getByRole('button', { name: /Fund account/ }));
    await waitFor(() => expect(screen.getByTestId('hash-fund')).toBeInTheDocument());
    await userEvent.click(screen.getByRole('button', { name: /Accept/ }));
    await waitFor(() => expect(screen.getByTestId('hash-accept')).toBeInTheDocument());

    expect(screen.getAllByRole('listitem').length).toBeGreaterThanOrEqual(4);
    // Hashes are abbreviated for display but the full value stays available.
    expect(screen.getByTestId('hash-create')).toHaveAttribute('title', 'c'.repeat(64));
  });
});

describe('reading on-chain state', () => {
  it('starts with no state and fills in after a refresh', async () => {
    renderApp({ balance: 4_200n, tradeCount: 3, totalVolume: 12_000n, kycVerified: true });

    expect(screen.getByTestId('state-empty')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Refresh state' }));

    await waitFor(() => expect(screen.getByTestId('balance')).toHaveTextContent('4200'));
    expect(screen.getByTestId('kyc')).toHaveTextContent('Verified');
    expect(screen.getByTestId('trade-count')).toHaveTextContent('3');
    expect(screen.getByTestId('total-volume')).toHaveTextContent('12000');
  });

  it('lists open orders returned by the contract', async () => {
    renderApp({ orders: [fakeOrder({ orderId: 11n, status: 'PartiallyFilled' })] });

    await userEvent.click(screen.getByRole('button', { name: 'Refresh state' }));

    await waitFor(() => expect(screen.getByTestId('order-list')).toBeInTheDocument());
    expect(screen.getByText(/#11 Limit XLM→USDCSIM 1000 — PartiallyFilled/)).toBeInTheDocument();
  });
});

describe('failure reporting', () => {
  it('names the contract error rather than showing a raw host error', async () => {
    const client = createFakeClient();
    client.placeLimitOrder.mockRejectedValueOnce(
      new ContractCallError('HostError: Error(Contract, #500)', 500, 'KYCVerificationRequired'),
    );
    render(<App setup={fakeSetup(client)} />);

    await userEvent.click(screen.getByRole('button', { name: /Create order/ }));

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
    expect(screen.getByTestId('failure-contract')).toHaveTextContent('KYCVerificationRequired');
    expect(screen.getByTestId('failure-contract')).toHaveTextContent('#500');
  });

  it('reports a rejected signature and re-enables the buttons', async () => {
    const client = createFakeClient();
    client.mint.mockRejectedValueOnce(new SigningError('User declined the request'));
    render(<App setup={fakeSetup(client)} />);

    await userEvent.click(screen.getByRole('button', { name: /Fund account/ }));

    await waitFor(() =>
      expect(screen.getByTestId('failure-message')).toHaveTextContent('User declined'),
    );
    // A failure must not leave the UI stuck mid-step.
    expect(screen.getByRole('button', { name: /Fund account/ })).toBeEnabled();
  });

  it('clears a previous failure when the next step starts', async () => {
    const client = createFakeClient({ kycVerified: true });
    client.mint.mockRejectedValueOnce(new SigningError('User declined the request'));
    render(<App setup={fakeSetup(client)} />);

    await userEvent.click(screen.getByRole('button', { name: /Fund account/ }));
    await waitFor(() => expect(screen.getByTestId('failure')).toBeInTheDocument());

    await userEvent.click(screen.getByRole('button', { name: /Accept/ }));
    await waitFor(() => expect(screen.queryByTestId('failure')).not.toBeInTheDocument());
  });

  it('rejects a non-numeric amount before calling the contract', async () => {
    const client = renderApp();

    await userEvent.clear(screen.getByLabelText('Amount in (XLM)'));
    await userEvent.type(screen.getByLabelText('Amount in (XLM)'), '12.5');
    await userEvent.click(screen.getByRole('button', { name: /Create order/ }));

    expect(screen.getByTestId('input-error')).toBeInTheDocument();
    expect(client.placeLimitOrder).not.toHaveBeenCalled();
  });

  it('rejects a zero amount', async () => {
    const client = renderApp();

    await userEvent.clear(screen.getByLabelText('Amount in (XLM)'));
    await userEvent.type(screen.getByLabelText('Amount in (XLM)'), '0');
    await userEvent.click(screen.getByRole('button', { name: /Fund account/ }));

    expect(screen.getByTestId('input-error')).toBeInTheDocument();
    expect(client.mint).not.toHaveBeenCalled();
  });
});
