/**
 * The demo's signing abstraction.
 *
 * This module is the only place the demo decides *how* a transaction gets
 * signed, and it deliberately offers exactly one browser mechanism: an injected
 * Stellar wallet, adapted to the SDK's {@link SignTransaction} callback.
 *
 * ## Why there is no secret-key path here
 *
 * Vite inlines every `VITE_`-prefixed variable into the JavaScript it ships to
 * the browser. A signing key supplied that way is therefore not "a secret in an
 * environment variable" — it is a secret pasted into a public asset, recoverable
 * by anyone who opens devtools or fetches the bundle. That is true on localnet
 * too, and an example that demonstrates the pattern teaches it.
 *
 * So the demo cannot sign from a secret key even if someone sets one: no code
 * path reads a key, and `keypairSigner` is not imported. Signing authority stays
 * with the wallet, which holds the key outside the page and asks the user before
 * every signature.
 *
 * (This file deliberately does not spell out a `VITE_…SECRET…` variable name.
 * CI greps the demo's source and bundle for that shape, and a mention in a
 * comment would trip it — the sourcemap ships the comment too.)
 *
 * Node scripts are a different setting — the process is not a public asset — so
 * `scripts/verify_localnet.ts` uses `keypairSigner` with an ephemeral key. See
 * `docs/LOCALNET.md`.
 */
import { browserWalletSigner, type BrowserWallet, type SignTransaction } from '@swaptrade/sdk';

/** Which signing mechanism the demo resolved, so the UI can report it. */
export type SignerKind = 'browser-wallet' | 'none';

/**
 * Global shape a Freighter-style extension injects.
 *
 * Typed as a partial so a half-initialised injection is treated as absent
 * rather than trusted.
 */
interface WalletGlobals {
  freighterApi?: Partial<BrowserWallet>;
}

/**
 * Find an injected wallet that can actually sign.
 *
 * Presence of the global is not enough: extensions inject incrementally, and a
 * stub without `signTransaction` would fail at the worst moment — after the user
 * has already filled in an amount and clicked.
 */
export function detectBrowserWallet(scope: unknown = globalThis): BrowserWallet | undefined {
  const candidate = (scope as WalletGlobals | null)?.freighterApi;
  return typeof candidate?.signTransaction === 'function'
    ? (candidate as BrowserWallet)
    : undefined;
}

/**
 * Resolve the signer for this page load.
 *
 * Returns `kind: 'none'` rather than throwing when no wallet is present: the
 * read-only half of the demo (balances, orders, prices) needs no signer, and
 * losing it would make a missing extension look like a broken app.
 */
export function resolveSigner(scope: unknown = globalThis): {
  signer?: SignTransaction;
  kind: SignerKind;
} {
  const wallet = detectBrowserWallet(scope);
  return wallet ? { signer: browserWalletSigner(wallet), kind: 'browser-wallet' } : { kind: 'none' };
}
