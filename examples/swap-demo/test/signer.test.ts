/**
 * Signer resolution.
 *
 * These tests pin the security property the demo depends on: the only browser
 * signing path is an injected wallet, and no environment variable can introduce
 * a signing key. A regression here would silently reintroduce a secret into the
 * shipped bundle, so it is asserted rather than assumed.
 */
import { describe, expect, it, vi } from 'vitest';
import { detectBrowserWallet, resolveSigner } from '../src/signer.js';
import { DEMO_ACCOUNT, DEMO_PASSPHRASE, fakeWallet } from './fakeClient.js';

/**
 * Stand-in for a key someone might try to supply through the environment.
 *
 * Deliberately not a valid — or even well-formed — Stellar seed. Its content is
 * irrelevant to what the test proves: no code path reads these variables, so
 * nothing inspects the value. Using a real seed here would add a
 * credential-shaped literal to the repository and prove nothing extra.
 */
const ATTEMPTED_KEY = 'not-a-key-and-never-read';

describe('detectBrowserWallet', () => {
  it('finds an injected wallet that can sign', () => {
    const wallet = fakeWallet();
    expect(detectBrowserWallet({ freighterApi: wallet })).toBe(wallet);
  });

  it('treats a missing global as no wallet', () => {
    expect(detectBrowserWallet({})).toBeUndefined();
    expect(detectBrowserWallet(undefined)).toBeUndefined();
    expect(detectBrowserWallet(null)).toBeUndefined();
  });

  it('rejects a half-injected wallet that cannot sign', () => {
    // Extensions inject incrementally. A stub without `signTransaction` must not
    // be reported as usable, or the failure would land after the user clicks.
    expect(detectBrowserWallet({ freighterApi: {} })).toBeUndefined();
    expect(detectBrowserWallet({ freighterApi: { signTransaction: 'nope' } })).toBeUndefined();
  });
});

describe('resolveSigner', () => {
  it('adapts an injected wallet into a signing callback', async () => {
    const wallet = fakeWallet();
    const { signer, kind } = resolveSigner({ freighterApi: wallet });

    expect(kind).toBe('browser-wallet');
    const signed = await signer!('AAAA-envelope', {
      networkPassphrase: DEMO_PASSPHRASE,
      address: DEMO_ACCOUNT,
    });

    // The adapter forwards network and address so the wallet can warn about a
    // wrong chain or a wrong account before the user approves.
    expect(wallet.requests).toEqual([
      { xdr: 'AAAA-envelope', networkPassphrase: DEMO_PASSPHRASE, address: DEMO_ACCOUNT },
    ]);
    expect(signed).toBe('signed:AAAA-envelope');
  });

  it('reports no signer when no wallet is present', () => {
    const { signer, kind } = resolveSigner({});
    expect(signer).toBeUndefined();
    expect(kind).toBe('none');
  });

  it('surfaces a declined signature as an error rather than a silent failure', async () => {
    const { signer } = resolveSigner({ freighterApi: fakeWallet({ reject: true }) });
    await expect(
      signer!('AAAA-envelope', {
        networkPassphrase: DEMO_PASSPHRASE,
        address: DEMO_ACCOUNT,
      }),
    ).rejects.toThrow(/declined/i);
  });

  it('never produces a signer from environment variables', () => {
    // The guarantee: a key placed in the environment cannot become a signer,
    // because no code path reads one. Vite would inline any VITE_ value into the
    // public bundle, so this must stay true.
    vi.stubEnv('VITE_DEMO_SECRET_KEY', ATTEMPTED_KEY);
    vi.stubEnv('VITE_SECRET_KEY', ATTEMPTED_KEY);

    expect(resolveSigner({}).kind).toBe('none');
    expect(resolveSigner({}).signer).toBeUndefined();

    vi.unstubAllEnvs();
  });
});
