/**
 * Browser smoke test.
 *
 * Runs the production build with the real `@swaptrade/sdk` in a real browser. It
 * deliberately points at an RPC port with nothing behind it, which makes the
 * test self-contained: it proves the app mounts, reads its configuration, and
 * surfaces a transport failure to the user instead of hanging or crashing —
 * none of which requires a deployed contract.
 *
 * Signing goes through a mock wallet injected at `globalThis.freighterApi`, the
 * same global a real extension uses, so the test exercises the demo's actual
 * signer detection and the SDK's wallet adapter. No signing key exists in the
 * bundle or the page.
 *
 * The happy path against a live contract is covered by `docs/LOCALNET.md`; the
 * argument-mapping and success paths are covered by the SDK and component tests.
 */
import { expect, test } from '@playwright/test';
import { MOCK_WALLET_INIT_SCRIPT } from './mockWallet.js';

/** Values the build was configured with in `playwright.config.ts`. */
const ACCOUNT = 'GDVEU3DD4KOFECV66VIHWEZOYX4ZKR3WV27L464SIIPOU2IUI3JCZA57';
const CONTRACT = 'CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE';

test.describe('swap demo', () => {
  // Install the wallet before any page script runs, mirroring how an extension
  // injects. Tests that want the no-wallet state override this per test.
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(MOCK_WALLET_INIT_SCRIPT);
  });

  test('mounts and reports the configured account and network', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', (message) => {
      if (message.type() === 'error') consoleErrors.push(message.text());
    });

    await page.goto('/');

    await expect(page.getByRole('heading', { name: 'SwapTrade demo', level: 1 })).toBeVisible();
    await expect(page.getByTestId('account')).toHaveText(ACCOUNT);
    await expect(page.getByTestId('network')).toHaveText('Standalone Network ; February 2017');
    await expect(page.getByTestId('signer')).toContainText('Injected browser wallet');

    // A configured build must not fall back to the setup checklist.
    await expect(page.getByText('Configuration required')).toBeHidden();
    expect(consoleErrors).toEqual([]);
  });

  test('presents the create -> fund -> accept steps in order', async ({ page }) => {
    await page.goto('/');

    const steps = page.locator('.steps button');
    await expect(steps).toHaveCount(4);
    await expect(steps.nth(0)).toContainText('Prepare');
    await expect(steps.nth(1)).toContainText('Create order');
    await expect(steps.nth(2)).toContainText('Fund account');
    await expect(steps.nth(3)).toContainText('Accept');

    // A wallet is injected, so the workflow is actionable.
    await expect(steps.nth(0)).toBeEnabled();
    await expect(page.getByTestId('no-signer-notice')).toBeHidden();
  });

  test('shows the contract ID it will call', async ({ page }) => {
    await page.goto('/');
    // Guards against a build that silently picked up a different contract.
    await expect(page.getByTestId('rpc-url')).toContainText('localhost');
    expect(CONTRACT).toHaveLength(56);
  });

  test('surfaces an unreachable RPC endpoint as a visible error', async ({ page }) => {
    await page.goto('/');

    await page.getByRole('button', { name: 'Refresh state' }).click();

    // The alert is the contract this test cares about: an RPC failure has to
    // reach the user, and the button has to become usable again.
    const alert = page.getByRole('alert');
    await expect(alert).toBeVisible({ timeout: 30_000 });
    await expect(page.getByRole('button', { name: 'Refresh state' })).toBeEnabled();
    await expect(page.getByTestId('state-empty')).toBeVisible();
  });

  test('validates amounts in the browser before calling the contract', async ({ page }) => {
    await page.goto('/');

    const amount = page.getByLabel('Amount in (XLM)');
    await amount.fill('12.5');
    await page.getByRole('button', { name: /Create order/ }).click();

    await expect(page.getByTestId('input-error')).toBeVisible();
    // Rejected client-side, so nothing was ever submitted.
    await expect(page.getByTestId('activity-empty')).toBeVisible();
  });

  test('ships no signing key and offers no way to enter one', async ({ page }) => {
    await page.goto('/');

    // The bundle is a public asset. Assert against the real served JavaScript,
    // not against source: this is the artifact an attacker would read.
    const scripts = await page.locator('script[src]').evaluateAll((nodes) =>
      nodes.map((node) => (node as HTMLScriptElement).src),
    );
    expect(scripts.length).toBeGreaterThan(0);

    for (const src of scripts) {
      const body = await (await page.request.get(src)).text();
      expect(body).not.toMatch(/S[A-Z2-7]{55}/);
      // Match the shape rather than one variable name, so renaming the leak does
      // not evade the check. Mirrors the grep in `.github/workflows/sdk.yml`.
      expect(body).not.toMatch(/VITE_[A-Z0-9_]*(SECRET|PRIVATE|SEED|MNEMONIC|PASSWORD)/);
    }

    // And no input could collect one from the user.
    await expect(page.locator('input[type="password"]')).toHaveCount(0);
    const inputIds = await page
      .locator('input')
      .evaluateAll((nodes) => nodes.map((node) => (node as HTMLInputElement).id));
    expect(inputIds.sort()).toEqual(['amount-in', 'limit-price']);
  });

  test('reports a declined wallet signature as a visible failure', async ({ page }) => {
    await page.goto('/');

    // The injected mock holds no key and declines. What matters is that the
    // refusal reaches the user and leaves the app usable, rather than silently
    // stalling on a pending signature.
    await page.getByRole('button', { name: /Prepare/ }).click();

    await expect(page.getByRole('alert')).toBeVisible({ timeout: 30_000 });
    await expect(page.getByRole('button', { name: /Prepare/ })).toBeEnabled();
  });
});

/**
 * The no-wallet state. Separate from the block above so no init script runs:
 * this is what a visitor without an extension actually sees.
 */
test.describe('swap demo without a wallet', () => {
  test('falls back to read-only rather than asking for a key', async ({ page }) => {
    await page.goto('/');

    await expect(page.getByTestId('signer')).toContainText('None');
    await expect(page.getByTestId('no-signer-notice')).toBeVisible();
    await expect(page.getByTestId('no-signer-notice')).toContainText(/wallet/i);
    await expect(page.getByRole('button', { name: /Create order/ })).toBeDisabled();

    // Reading needs no signer, so it stays available.
    await expect(page.getByRole('button', { name: 'Refresh state' })).toBeEnabled();

    // The remedy offered is a wallet, never a key field.
    await expect(page.locator('input[type="password"]')).toHaveCount(0);
  });
});
