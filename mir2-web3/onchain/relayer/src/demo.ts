import { loadConfig } from './config';
import { EventSource } from './source';
import { DedupStore } from './dedup';
import { processOnce } from './relayer';
import type { NormalizedCommand } from './types';
import type { Sink } from './sink';

/**
 * One-shot demo (the M2 deliverable): normalize all of the package's events to date into
 * commands, then re-scan with the SAME dedup store to prove a full replay yields ZERO new
 * commands (idempotency place #2). Runs read-only against testnet — no key, no injection.
 *
 * Run: `pnpm relayer:demo` (needs MINE_PACKAGE_ID; .env already has the testnet package).
 */
async function main(): Promise<void> {
  const config = loadConfig();
  console.log(`[demo] network=${config.network} package=${config.packageId}`);

  const collected: NormalizedCommand[] = [];
  const sink: Sink = {
    async inject(command) {
      collected.push(command);
      console.log(`  -> ${command.kind} ${JSON.stringify(command)}`);
    },
  };

  // Fresh in-memory dedup + ephemeral cursors so the demo is reproducible.
  const dedup = new DedupStore();

  console.log('[demo] pass 1 — normalize all events to date:');
  const pass1 = await processOnce(new EventSource(config.network, config.packageId, config.modules), dedup, sink);
  console.log(
    `[demo] pass1: scanned=${pass1.scanned} commands=${pass1.commands} ` +
      `skipped=${pass1.skipped} dup=${pass1.duplicates}`,
  );

  console.log('[demo] pass 2 — full replay against the same dedup store (expect commands=0):');
  const pass2 = await processOnce(new EventSource(config.network, config.packageId, config.modules), dedup, sink);
  console.log(`[demo] pass2: scanned=${pass2.scanned} commands=${pass2.commands} dup=${pass2.duplicates}`);

  if (pass2.commands !== 0) {
    console.error('❌ dedup FAILED: replay produced new commands');
    process.exit(1);
  }
  console.log(`✅ ${collected.length} normalized command(s); replay produced 0 (idempotent).`);
}

main().catch((error) => {
  console.error('[demo] fatal:', error);
  process.exit(1);
});
