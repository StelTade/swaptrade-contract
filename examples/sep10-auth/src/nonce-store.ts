/**
 * Single-use nonce registry.
 *
 * SEP-10 challenges embed a 64-byte random nonce. Verification must
 * enforce that each nonce is consumed exactly once: an attacker who
 * observes a signed challenge (or replays a captured POST) must not be
 * able to mint a second session from it. Entries expire with the same
 * TTL as the challenge window so the map cannot grow without bound.
 */
export class NonceStore {
  private readonly used = new Map<string, number>();
  private readonly ttlSeconds: number;

  constructor(
    ttlSeconds = 300,
    private readonly now: () => number = () => Math.floor(Date.now() / 1000),
  ) {
    this.ttlSeconds = ttlSeconds;
  }

  /** Record a nonce as consumed. Returns false if it was already used. */
  consume(nonceBase64: string): boolean {
    this.sweep();
    if (this.used.has(nonceBase64)) {
      return false;
    }
    this.used.set(nonceBase64, Math.floor(this.now()));
    return true;
  }

  has(nonceBase64: string): boolean {
    return this.used.has(nonceBase64);
  }

  size(): number {
    return this.used.size;
  }

  private sweep(): void {
    const now = Math.floor(this.now());
    for (const [nonce, seenAt] of this.used) {
      if (now - seenAt > this.ttlSeconds) {
        this.used.delete(nonce);
      }
    }
  }
}
