/**
 * Signer adapters.
 *
 * The keypair signer produces a real signature over real transaction XDR, and
 * the browser adapter is checked against both Freighter response shapes.
 */
import { Keypair, TransactionBuilder } from '@stellar/stellar-sdk';
import { describe, expect, it, vi } from 'vitest';
import {
  NETWORKS,
  SigningError,
  ValidationError,
  browserWalletSigner,
  keypairSigner,
} from '../src/index.js';
import { TEST_KEYPAIR, baseConfig, createFakeServer } from './helpers.js';
import { SwapTradeClient } from '../src/index.js';

const passphrase = NETWORKS.local.networkPassphrase;

/** Build a real unsigned transaction envelope to hand to a signer. */
async function unsignedXdr(): Promise<string> {
  const client = new SwapTradeClient(baseConfig(), { server: createFakeServer() });
  const tx = await client.buildTransaction('get_contract_version', []);
  return tx.toXDR();
}

describe('keypairSigner', () => {
  it('rejects a malformed secret key up front', () => {
    expect(() => keypairSigner('not-a-secret')).toThrow(ValidationError);
    // A public key is not a signing key.
    expect(() => keypairSigner(TEST_KEYPAIR.publicKey())).toThrow(/secret seed/);
  });

  it('produces a signature the network passphrase verifies against', async () => {
    const signer = keypairSigner(TEST_KEYPAIR.secret());
    const signed = await signer(await unsignedXdr(), {
      networkPassphrase: passphrase,
      address: TEST_KEYPAIR.publicKey(),
    });

    const tx = TransactionBuilder.fromXDR(signed, passphrase);
    expect(tx.signatures).toHaveLength(1);
    // Verifying against the transaction hash proves this is a real signature,
    // not just an envelope with a signature-shaped blob attached.
    const hash = tx.hash();
    expect(TEST_KEYPAIR.verify(hash, tx.signatures[0]!.signature())).toBe(true);
  });

  it('reports unparseable XDR as a SigningError', () => {
    const signer = keypairSigner(TEST_KEYPAIR.secret());
    expect(() =>
      signer('this-is-not-xdr', { networkPassphrase: passphrase, address: '' }),
    ).toThrow(SigningError);
  });

  it('signs over the passphrase it is given, so a mismatch invalidates the signature', async () => {
    // The passphrase is not carried in the envelope; it feeds the transaction
    // hash. Signing under the wrong one therefore succeeds locally and is only
    // rejected by the network, which is why the client passes its resolved
    // config passphrase rather than letting the signer choose.
    const signer = keypairSigner(TEST_KEYPAIR.secret());
    const signed = await signer(await unsignedXdr(), {
      networkPassphrase: NETWORKS.testnet.networkPassphrase,
      address: TEST_KEYPAIR.publicKey(),
    });

    const wrongNetwork = TransactionBuilder.fromXDR(
      signed,
      NETWORKS.testnet.networkPassphrase,
    );
    expect(TEST_KEYPAIR.verify(wrongNetwork.hash(), wrongNetwork.signatures[0]!.signature()))
      .toBe(true);

    const localNetwork = TransactionBuilder.fromXDR(signed, passphrase);
    expect(TEST_KEYPAIR.verify(localNetwork.hash(), localNetwork.signatures[0]!.signature()))
      .toBe(false);
  });
});

describe('browserWalletSigner', () => {
  it('accepts the { signedTxXdr } shape returned by current Freighter', async () => {
    const wallet = {
      signTransaction: vi.fn(async () => ({ signedTxXdr: 'SIGNED_XDR' })),
    };
    const signer = browserWalletSigner(wallet);

    await expect(
      signer('UNSIGNED', { networkPassphrase: passphrase, address: 'GABC' }),
    ).resolves.toBe('SIGNED_XDR');
    expect(wallet.signTransaction).toHaveBeenCalledWith('UNSIGNED', {
      networkPassphrase: passphrase,
      address: 'GABC',
    });
  });

  it('accepts the bare-string shape returned by older wallets', async () => {
    const signer = browserWalletSigner({
      signTransaction: vi.fn(async () => 'SIGNED_XDR'),
    });
    await expect(
      signer('UNSIGNED', { networkPassphrase: passphrase, address: 'GABC' }),
    ).resolves.toBe('SIGNED_XDR');
  });

  it.each([
    ['an empty string', ''],
    ['whitespace only', '   '],
    ['an object with no XDR', {} as never],
  ])('treats %s as a rejected signature request', async (_label, result) => {
    const signer = browserWalletSigner({ signTransaction: vi.fn(async () => result) });
    await expect(
      signer('UNSIGNED', { networkPassphrase: passphrase, address: 'GABC' }),
    ).rejects.toThrow(/may have been rejected/);
  });

  it('propagates a wallet-level rejection', async () => {
    const signer = browserWalletSigner({
      signTransaction: vi.fn(async () => {
        throw new Error('User declined access');
      }),
    });
    await expect(
      signer('UNSIGNED', { networkPassphrase: passphrase, address: 'GABC' }),
    ).rejects.toThrow(/User declined access/);
  });
});

describe('end-to-end signing through the client', () => {
  it('submits an envelope carrying a verifiable signature', async () => {
    const server = createFakeServer();
    const client = new SwapTradeClient(baseConfig(), { server });

    await client.mint('XLM', TEST_KEYPAIR.publicKey(), 1n);

    const submitted = server.sendTransaction.mock.calls[0]![0] as { toXDR(): string };
    const tx = TransactionBuilder.fromXDR(submitted.toXDR(), passphrase);
    expect(tx.signatures).toHaveLength(1);
    expect(
      Keypair.fromPublicKey(TEST_KEYPAIR.publicKey()).verify(
        tx.hash(),
        tx.signatures[0]!.signature(),
      ),
    ).toBe(true);
  });
});
