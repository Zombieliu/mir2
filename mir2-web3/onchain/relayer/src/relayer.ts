import { eventIdOf } from './types';
import { normalize } from './normalize';
import type { EventSource } from './source';
import type { DedupStore } from './dedup';
import type { Sink } from './sink';

export interface ProcessResult {
  scanned: number;
  duplicates: number;
  commands: number;
  skipped: number;
}

/**
 * One poll cycle: fetch new events, dedup by `(tx_digest, event_seq)`, normalize, inject.
 * Persists the dedup set + source cursors at the end so a crash/restart resumes without
 * loss or duplicates (ROADMAP §5 exit). The three idempotency layers compose here: the
 * chain's nonce (already settled), the relayer's dedup (this function), and the Sim's
 * idempotency key (carried on every command for place #3).
 */
export async function processOnce(
  source: EventSource,
  dedup: DedupStore,
  sink: Sink,
): Promise<ProcessResult> {
  const events = await source.poll();
  let duplicates = 0;
  let commands = 0;
  let skipped = 0;
  for (const event of events) {
    if (!dedup.claim(eventIdOf(event))) {
      duplicates += 1; // idempotency place #2
      continue;
    }
    const command = normalize(event);
    if (!command) {
      skipped += 1; // internal storage SetRecord, or a dry (zero-ore) batch
      continue;
    }
    await sink.inject(command);
    commands += 1;
  }
  dedup.persist();
  source.persistCursors();
  return { scanned: events.length, duplicates, commands, skipped };
}
