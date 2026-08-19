/**
 * Live localnet check for the SDK's transaction pipeline.
 *
 * This is NOT part of the automated test suite: it needs a running localnet and
 * a deployed contract, so it is a manual verification script referenced by
 * docs/LOCALNET.md.
 *
 * It drives the deployed `soroban-ping` contract through the real SDK to prove
 * the build -> simulate -> assemble -> sign -> submit -> poll pipeline works
 * against an actual network, not just against the fake RPC server used by the
 * unit tests.
 *
 * ## Why a secret key is acceptable here and not in the browser
 *
 * This runs in Node. The key stays in the process and its environment; it is
 * never written into an asset served to anyone. The example DApp deliberately
 * has no equivalent path, because Vite inlines `VITE_`-prefixed values into the
 * public bundle — see examples/swap-demo/src/signer.ts.
 *
 * Usage:
 *   # Ephemeral: generate and fund a throwaway localnet key, use it, discard it.
 *   node --experimental-strip-types scripts/verify_localnet.ts \
 *     --contract <C...> --ephemeral
 *
 *   # Or supply an existing localnet identity.
 *   node --experimental-strip-types scripts/verify_localnet.ts \
 *     --contract <C...> --secret <S...>
 *
 * Values come from the command line or the environment; nothing is hardcoded.
 */
import { SwapTradeClient, keypairSigner, NETWORKS } from '../packages/swaptrade-sdk/dist/index.js';

/** Read `--flag value` from argv, falling back to an environment variable. */
function arg(flag: string, envVar: string): string | undefined {
  const index = process.argv.indexOf(`--${flag}`);
  if (index !== -1 && process.argv[index + 1]) return process.argv[index + 1];
  return process.env[envVar];
}

/** Whether a boolean `--flag` is present. */
function flag(name: string): boolean {
  return process.argv.includes(`--${name}`);
}

const contractId = arg('contract', 'SWAPTRADE_CONTRACT_ID');
const rpcUrl = arg('rpc', 'SOROBAN_RPC_URL') ?? NETWORKS.local.rpcUrl;
const networkPassphrase =
  arg('passphrase', 'SOROBAN_NETWORK_PASSPHRASE') ?? NETWORKS.local.networkPassphrase;

const { Keypair } = await import('@stellar/stellar-sdk');

/**
 * Fund an account with friendbot.
 *
 * The quickstart localnet exposes friendbot on the same host as RPC, so a
 * throwaway identity can be created and funded without the CLI or a stored key.
 */
async function fundWithFriendbot(publicKey: string): Promise<void> {
  const friendbot = new URL(rpcUrl);
  friendbot.pathname = '/friendbot';
  friendbot.search = `?addr=${publicKey}`;

  const response = await fetch(friendbot);
  if (!response.ok) {
    throw new Error(
      `Friendbot could not fund ${publicKey} (HTTP ${response.status}). ` +
        'Is this a localnet with friendbot enabled? Pass --secret to use an existing identity instead.',
    );
  }
}

/**
 * Resolve the signing identity.
 *
 * `--ephemeral` generates a key that exists only for this process, which keeps
 * the common case free of any stored or pasted credential.
 */
async function resolveIdentity(): Promise<{ secret: string; publicKey: string; source: string }> {
  const supplied = arg('secret', 'SWAPTRADE_SECRET_KEY');

  if (supplied) {
    return {
      secret: supplied,
      publicKey: Keypair.fromSecret(supplied).publicKey(),
      source: 'supplied identity',
    };
  }

  if (!flag('ephemeral')) {
    throw new Error('no identity');
  }

  const keypair = Keypair.random();
  console.log(`Generating an ephemeral identity: ${keypair.publicKey()}`);
  await fundWithFriendbot(keypair.publicKey());
  console.log('Funded via friendbot. It is discarded when this process exits.\n');

  return {
    secret: keypair.secret(),
    publicKey: keypair.publicKey(),
    source: 'ephemeral (generated, funded, discarded)',
  };
}

if (!contractId) {
  console.error(
    'Usage: node --experimental-strip-types scripts/verify_localnet.ts \\\n' +
      '  --contract <C...> [--ephemeral | --secret <S...>]\n\n' +
      'Or set SWAPTRADE_CONTRACT_ID and SWAPTRADE_SECRET_KEY.',
  );
  process.exit(2);
}

let identity: { secret: string; publicKey: string; source: string };
try {
  identity = await resolveIdentity();
} catch (error) {
  if (error instanceof Error && error.message === 'no identity') {
    console.error(
      'No signing identity. Either:\n' +
        '  --ephemeral            generate and fund a throwaway localnet key, or\n' +
        '  --secret <S...>        use an existing identity (or SWAPTRADE_SECRET_KEY)\n\n' +
        'A secret is safe here because this is a Node process, not a browser bundle.',
    );
    process.exit(2);
  }
  throw error;
}

const client = new SwapTradeClient({
  rpcUrl,
  networkPassphrase,
  contractId,
  publicKey: identity.publicKey,
  signTransaction: keypairSigner(identity.secret),
});

console.log(`RPC:      ${rpcUrl}`);
console.log(`Contract: ${contractId}`);
console.log(`Account:  ${identity.publicKey}`);
console.log(`Identity: ${identity.source}\n`);

// 1. Read-only path: build -> simulate -> decode, no submission.
const simulated = await client.simulate<string>('ping', []);
console.log(`simulate('ping') -> ${JSON.stringify(simulated.returnValue)}`);
if (simulated.returnValue !== 'pong') {
  throw new Error(`Expected "pong" from simulation, got ${JSON.stringify(simulated.returnValue)}`);
}

// 2. Full write path: build -> simulate -> assemble -> sign -> submit -> poll.
//    `ping` mutates nothing, but submitting it exercises every stage of the
//    pipeline against a real network, which is the point of this check.
const submitted = await client.invoke<string>('ping', []);
console.log(`invoke('ping')   -> ${JSON.stringify(submitted.returnValue)}`);
console.log(`  status: ${submitted.status}`);
console.log(`  hash:   ${submitted.hash}`);
console.log(`  ledger: ${submitted.ledger}`);

if (submitted.status !== 'SUCCESS') {
  throw new Error(`Expected SUCCESS, got ${submitted.status}`);
}
if (submitted.returnValue !== 'pong') {
  throw new Error(`Expected "pong" from invocation, got ${JSON.stringify(submitted.returnValue)}`);
}

console.log('\nOK: simulate and invoke both round-tripped through localnet.');
