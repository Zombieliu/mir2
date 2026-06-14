import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import type { EventId } from './types';

/**
 * Dedup store — idempotency place #2 (DESIGN §0.2/§4). Tracks processed `(tx_digest,
 * event_seq)` ids so a replayed/reordered event stream (reconnect, restart, RPC retry) never
 * produces a duplicate command. Persists to a JSON file so dedup survives restarts.
 *
 * M2 keeps the full seen-set (testnet volume is tiny). Production (M6) swaps this for a DB
 * keyed on (tx_digest, event_seq) with a retention window tied to the source cursor.
 */
export class DedupStore {
  private seen = new Set<EventId>();

  /** @param path optional JSON file to persist to; omit for an in-memory store (tests). */
  constructor(private readonly path?: string) {
    if (path && existsSync(path)) {
      try {
        const data = JSON.parse(readFileSync(path, 'utf8')) as { seen?: EventId[] };
        for (const id of data.seen ?? []) this.seen.add(id);
      } catch {
        // Corrupt state file -> start empty; the source cursor still bounds re-scan.
      }
    }
  }

  /** True iff `id` has not been processed before. */
  isNew(id: EventId): boolean {
    return !this.seen.has(id);
  }

  /**
   * Atomically claim an id: returns true the FIRST time (and records it), false on any
   * subsequent call. This is the dedup gate the relayer calls before normalizing.
   */
  claim(id: EventId): boolean {
    if (this.seen.has(id)) return false;
    this.seen.add(id);
    return true;
  }

  size(): number {
    return this.seen.size;
  }

  persist(): void {
    if (!this.path) return;
    mkdirSync(dirname(this.path), { recursive: true });
    writeFileSync(this.path, JSON.stringify({ seen: [...this.seen] }), 'utf8');
  }
}
