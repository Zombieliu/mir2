import { join } from 'node:path';
import { loadConfig } from './config';
import { EventSource } from './source';
import { DedupStore } from './dedup';
import { makeSink } from './sink';
import { processOnce } from './relayer';

/**
 * Relayer (WF-3) long-running entry: poll the chain for the mine package's events, dedup,
 * normalize, and inject the chain-confirmed commands. Run: `pnpm relayer` (needs
 * MINE_PACKAGE_ID in env/.env; set GATEWAY_INJECT_URL + OPERATOR_TOKEN for the M3 HTTP sink).
 */
async function main(): Promise<void> {
  const config = loadConfig();
  console.log(
    `[relayer] network=${config.network} package=${config.packageId} ` +
      `modules=${config.modules.join(',')} sink=${config.injectUrl ? 'http' : 'log'}`,
  );

  const source = new EventSource(config.network, config.packageId, config.modules, config.stateDir);
  const dedup = new DedupStore(join(config.stateDir, 'seen.json'));
  const sink = makeSink(config);

  let stopping = false;
  const stop = () => {
    stopping = true;
  };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);

  while (!stopping) {
    try {
      const result = await processOnce(source, dedup, sink);
      if (result.commands > 0 || result.scanned > 0) {
        console.log(
          `[relayer] scanned=${result.scanned} commands=${result.commands} ` +
            `dup=${result.duplicates} skipped=${result.skipped} seen=${dedup.size()}`,
        );
      }
    } catch (error) {
      console.error('[relayer] poll error (will retry):', error instanceof Error ? error.message : error);
    }
    await new Promise((r) => setTimeout(r, config.pollIntervalMs));
  }
  console.log('[relayer] stopped');
}

main().catch((error) => {
  console.error('[relayer] fatal:', error);
  process.exit(1);
});
