/**
 * Signer helpers.
 *
 * The SDK accepts any {@link SignTransaction} callback, which keeps wallet
 * choice out of the client. These helpers cover the two cases the example app
 * and the localnet scripts need.
 */
import { Keypair, TransactionBuilder } from '@stellar/stellar-sdk';
import { SigningError, ValidationError } from './errors.js';
import type { SignTransaction } from './types.js';

/**
 * Build a signer from a Stellar secret key.
 *
 * Intended for localnet demos, scripts and tests. Never ship a secret key to a
 * browser bundle — use {@link browserWalletSigner} for user-facing apps.
 *
 * @param secretKey - A Stellar secret seed (`S...`).
 * @throws {ValidationError} when the secret key is malformed.
 */
export function keypairSigner(secretKey: string): SignTransaction {
  let keypair: Keypair;
  try {
    keypair = Keypair.fromSecret(secretKey);
  } catch (cause) {
    throw new ValidationError(
      'ADDRESS_INVALID',
      'Invalid secret key: expected a Stellar secret seed starting with "S".',
      cause,
    );
  }

  return (xdr, { networkPassphrase }) => {
    try {
      const tx = TransactionBuilder.fromXDR(xdr, networkPassphrase);
      tx.sign(keypair);
      return tx.toXDR();
    } catch (cause) {
      throw new SigningError(`Local keypair could not sign the transaction: ${String(cause)}`, cause);
    }
  };
}

/** Minimal shape of a Freighter-style injected browser wallet. */
export interface BrowserWallet {
  signTransaction(
    xdr: string,
    opts: { networkPassphrase?: string; address?: string },
  ): Promise<string | { signedTxXdr: string }>;
}

/**
 * Adapt a Freighter-style browser wallet to {@link SignTransaction}.
 *
 * Recent Freighter versions resolve to `{ signedTxXdr }` while older ones
 * resolve to a bare string; both are accepted.
 */
export function browserWalletSigner(wallet: BrowserWallet): SignTransaction {
  return async (xdr, context) => {
    const result = await wallet.signTransaction(xdr, {
      networkPassphrase: context.networkPassphrase,
      address: context.address,
    });

    const signed = typeof result === 'string' ? result : result?.signedTxXdr;
    if (typeof signed !== 'string' || signed.trim() === '') {
      throw new SigningError(
        'Wallet did not return signed transaction XDR. The request may have been rejected.',
      );
    }
    return signed;
  };
}
