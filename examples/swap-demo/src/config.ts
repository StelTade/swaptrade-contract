/**
 * Environment -> SDK configuration.
 *
 * Everything the demo needs to reach a network comes from `import.meta.env`, so
 * no endpoint or contract ID is baked into the source. This module is the only
 * place that reads the environment; components receive a finished client.
 *
 * No variable read here is a secret. Signing keys never pass through the
 * environment, because Vite inlines `VITE_`-prefixed values into the browser
 * bundle — see `signer.ts`.
 */
import {
  NETWORKS,
  SwapTradeClient,
  type SwapTradeConfig,
} from '@swaptrade/sdk';
import { resolveSigner, type SignerKind } from './signer.js';

export type { SignerKind } from './signer.js';

/** Missing configuration, described in terms of what the operator must do. */
export interface ConfigProblem {
  variable: string;
  detail: string;
}

/** Either a usable client or the list of things preventing one. */
export type ClientSetup =
  | { ok: true; client: SwapTradeClient; signerKind: SignerKind }
  | { ok: false; problems: ConfigProblem[] };

/** Read a variable, treating blank strings as absent. */
function envVar(name: string): string | undefined {
  const raw = (import.meta.env as Record<string, string | undefined>)[name];
  const trimmed = raw?.trim();
  return trimmed === '' ? undefined : trimmed;
}

/**
 * Build the client from the environment.
 *
 * Returns problems rather than throwing so the UI can render a setup checklist
 * instead of a blank screen — a missing contract ID is a configuration mistake,
 * not a crash.
 */
export function createClientFromEnv(): ClientSetup {
  const problems: ConfigProblem[] = [];

  const contractId = envVar('VITE_CONTRACT_ID');
  if (!contractId) {
    problems.push({
      variable: 'VITE_CONTRACT_ID',
      detail: 'Contract ID printed by `npm run localnet:deploy`.',
    });
  }

  const publicKey = envVar('VITE_PUBLIC_KEY');
  if (!publicKey) {
    problems.push({
      variable: 'VITE_PUBLIC_KEY',
      detail: 'Public key (G...) to use as the source account.',
    });
  }

  if (!contractId || !publicKey) return { ok: false, problems };

  // Only the localnet endpoint is defaulted; a public network must be explicit.
  const config: SwapTradeConfig = {
    rpcUrl: envVar('VITE_RPC_URL') ?? NETWORKS.local.rpcUrl,
    networkPassphrase: envVar('VITE_NETWORK_PASSPHRASE') ?? NETWORKS.local.networkPassphrase,
    contractId,
    publicKey,
  };

  const { signer, kind } = resolveSigner();
  if (signer) config.signTransaction = signer;

  try {
    return { ok: true, client: new SwapTradeClient(config), signerKind: kind };
  } catch (error) {
    return {
      ok: false,
      problems: [
        {
          variable: 'configuration',
          detail: error instanceof Error ? error.message : String(error),
        },
      ],
    };
  }
}
