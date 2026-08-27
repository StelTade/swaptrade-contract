import { Networks } from "@stellar/stellar-sdk";

/**
 * Runtime configuration for the SEP-10 service.
 *
 * Everything is injectable so tests can drive the clock deterministically
 * and run against ephemeral keypairs instead of real secrets.
 */
export interface Sep10Config {
  /** Domain that names the ManageData operation: `<domain> auth`. */
  authDomain: string;
  /** Stellar network the challenge transactions are built for. */
  networkPassphrase: string;
  /**
   * Challenge validity window in seconds. SEP-10 requires the timebounds
   * to span at most five minutes, so keep this at or below 300.
   */
  challengeWindowSeconds: number;
  /** Session-token lifetime in seconds after successful verification. */
  sessionTtlSeconds: number;
  /** HMAC secret used to mint and verify session tokens. */
  sessionSecret: string;
}

export const defaultConfig: Sep10Config = {
  authDomain: "swaptrade.example",
  networkPassphrase: Networks.TESTNET,
  challengeWindowSeconds: 300,
  sessionTtlSeconds: 15 * 60,
  // In production load this from a secret manager — never hardcode it.
  sessionSecret: "dev-only-secret-rotate-me",
};
