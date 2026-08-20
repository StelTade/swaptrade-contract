/**
 * Development-only mock wallet for the browser smoke test.
 *
 * The demo has exactly one browser signing path — an injected wallet at
 * `globalThis.freighterApi` — so an automated browser test needs a wallet, not a
 * secret key. This installs one at that same global before the app boots, which
 * means the smoke test exercises the real `browserWalletSigner` adapter and the
 * real `resolveSigner` detection rather than a bypass.
 *
 * ## It holds no key
 *
 * A real wallet signs. This one records the request and rejects it with the
 * message a user-declined signature produces. That is enough for the smoke
 * test's actual assertions — that the workflow is enabled, that a signature
 * request reaches the wallet, and that a refusal surfaces as a visible error —
 * and it means no signing key exists anywhere in the browser context.
 *
 * Loaded via Playwright's `addInitScript`, so it is never part of the production
 * bundle and never imported by `src/`.
 */

/** Requests the mock received, readable from a test via `window`. */
export interface MockWalletLog {
  requests: { xdr: string; networkPassphrase?: string; address?: string }[];
}

/**
 * Source of the init script, as a string.
 *
 * Playwright serialises `addInitScript` functions into the page, so this is
 * written as a self-contained expression with no imports or closure captures.
 */
export const MOCK_WALLET_INIT_SCRIPT = `
(() => {
  const log = { requests: [] };
  globalThis.__mockWalletLog = log;
  globalThis.freighterApi = {
    signTransaction(xdr, opts) {
      log.requests.push({
        xdr,
        networkPassphrase: opts && opts.networkPassphrase,
        address: opts && opts.address,
      });
      // A wallet that holds no key can only decline. The demo must render this
      // as a failure the user can act on, which is what the smoke test asserts.
      return Promise.reject(new Error('User declined the signature request.'));
    },
  };
})();
`;
