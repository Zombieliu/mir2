import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { SuiClient, getFullnodeUrl } from '@mysten/sui/client';
import type { ChainEvent } from './types';
import type { SuiNetwork } from './config';

type SuiCursor = { txDigest: string; eventSeq: string } | null;

/**
 * EventSource — reads the package's on-chain events directly from Sui via `queryEvents`
 * (`MoveModule { package, module }`), which is precise (only this package's modules) and
 * robust. The Dubhe indexer (sqlite + GraphQL) runs as the WF-2 indexer, but its GraphQL
 * endpoint is undocumented in this SDK version, so the bridge reads Sui directly — the
 * indexer does the same internally.
 *
 * A per-module cursor is persisted so a restart resumes exactly where it left off (no loss);
 * combined with the DedupStore this guarantees no-loss + no-dup across reconnects (ROADMAP
 * §5 exit). `order: 'ascending'` yields events oldest-first so the cursor advances safely.
 */
export class EventSource {
  private readonly client: SuiClient;
  private readonly cursorFile: string;
  private cursors: Record<string, SuiCursor> = {};

  constructor(
    network: SuiNetwork,
    private readonly packageId: string,
    private readonly modules: string[],
    stateDir?: string,
  ) {
    this.client = new SuiClient({ url: getFullnodeUrl(network) });
    this.cursorFile = stateDir ? join(stateDir, 'cursors.json') : '';
    if (this.cursorFile && existsSync(this.cursorFile)) {
      try {
        this.cursors = JSON.parse(readFileSync(this.cursorFile, 'utf8'));
      } catch {
        this.cursors = {};
      }
    }
  }

  /** Fetch all new events across the configured modules since the saved cursors. */
  async poll(): Promise<ChainEvent[]> {
    const out: ChainEvent[] = [];
    for (const module of this.modules) {
      let cursor: SuiCursor = this.cursors[module] ?? null;
      for (;;) {
        const res = await this.client.queryEvents({
          query: { MoveModule: { package: this.packageId, module } },
          cursor,
          order: 'ascending',
          limit: 50,
        });
        for (const e of res.data) {
          out.push({
            id: { txDigest: e.id.txDigest, eventSeq: e.id.eventSeq },
            type: e.type,
            parsedJson: (e.parsedJson as ChainEvent['parsedJson']) ?? null,
            timestampMs: e.timestampMs ?? null,
          });
        }
        if (res.data.length > 0) cursor = res.nextCursor as SuiCursor;
        if (!res.hasNextPage) break;
      }
      this.cursors[module] = cursor;
    }
    return out;
  }

  persistCursors(): void {
    if (!this.cursorFile) return;
    mkdirSync(join(this.cursorFile, '..'), { recursive: true });
    writeFileSync(this.cursorFile, JSON.stringify(this.cursors), 'utf8');
  }
}
