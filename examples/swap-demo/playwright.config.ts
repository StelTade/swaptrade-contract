/**
 * Playwright configuration.
 *
 * Playwright is the only browser-test tool in this repository; Cypress is
 * deliberately not used. The smoke test runs against the production build via
 * `vite preview`, so it validates the artifact CI ships rather than the dev
 * server.
 *
 * Note what is absent from `env` below: there is no signing key. The demo has no
 * secret-key code path, so a browser test signs the way a user does — through an
 * injected wallet, mocked per-test in `e2e/mockWallet.ts`. Nothing
 * credential-shaped is built into the bundle.
 */
import { Keypair } from '@stellar/stellar-sdk';
import { defineConfig, devices } from '@playwright/test';

const PORT = 4173;

/**
 * Source account for the smoke test.
 *
 * Only the public half is used, derived from a fixed seed so no literal address
 * is written into the repository. It controls no funds on any network, and the
 * matching secret is never generated here or handed to the browser.
 */
const TEST_PUBLIC_KEY = Keypair.fromRawEd25519Seed(Buffer.alloc(32, 7)).publicKey();

/** Syntactically valid contract ID. Nothing is deployed at it. */
const TEST_CONTRACT_ID = 'CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? 'list' : [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: 'on-first-retry',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    // Build first so the smoke test can never pass against a stale bundle.
    command: 'npm run build && npm run preview',
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
    // Vite inlines these at build time, which is exactly why no key appears
    // here. The RPC port is intentionally one with nothing behind it: the smoke
    // test asserts that a transport failure reaches the user, which needs no
    // deployed contract.
    env: {
      VITE_RPC_URL: 'http://localhost:8999/soroban/rpc',
      VITE_NETWORK_PASSPHRASE: 'Standalone Network ; February 2017',
      VITE_CONTRACT_ID: TEST_CONTRACT_ID,
      VITE_PUBLIC_KEY: TEST_PUBLIC_KEY,
    },
  },
});
