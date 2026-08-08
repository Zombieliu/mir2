/**
 * Fixed-cadence snapshot emitter.
 *
 * `createSnapshotEmitter` wraps a `WorldStore` and calls `onSnapshot(json)`
 * at a steady interval (`intervalMs`), deduplicating by JSON string so the
 * downstream consumer (Bevy / WASM `setMir2WorldState`) is not called when
 * nothing has changed.
 *
 * This replaces the React-useEffect rAF pattern in `page.tsx` (lines
 * 3526-3547). It runs independent of any React render cycle so the push rate
 * is not throttled by React reconciliation.
 *
 * No React imports — pure TS with no external dependencies.
 */

import type { WorldStore } from "./store";
import type { WorldState } from "./types";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export type SnapshotEmitterOptions = {
  /**
   * How often to check for changes and potentially push a new snapshot (ms).
   * Defaults to 16 (≈60 fps). The emitter always pushes the LATEST state
   * at each tick — it does not interpolate.
   */
  intervalMs?: number;

  /**
   * Optional projection applied before serialization. Use it to push only the
   * fields the consumer needs — e.g. Bevy ignores the large originalMapRegion /
   * inventory / UI state, so projecting to its slice keeps the per-tick
   * stringify cheap instead of re-serializing the whole world 30x/s.
   */
  select?: (state: WorldState) => Record<string, unknown>;

  /**
   * Called with the serialized JSON string whenever the snapshot has changed
   * since the last call.
   */
  onSnapshot: (json: string) => void;
};

export type SnapshotEmitter = {
  /** Start the polling loop. Idempotent — safe to call multiple times. */
  start(): void;
  /** Stop the polling loop. The current interval (if any) is cleared. */
  stop(): void;
  /** True when the emitter is running. */
  readonly running: boolean;
};

/**
 * Create a snapshot emitter bound to `store`.
 *
 * The emitter stamps `clientTimeMs` onto each outbound snapshot (additive,
 * ignored by existing consumers) so Bevy can detect stale pushes.
 */
export function createSnapshotEmitter(
  store: WorldStore,
  options: SnapshotEmitterOptions,
): SnapshotEmitter {
  const { intervalMs = 16, select, onSnapshot } = options;

  let timerId: ReturnType<typeof setInterval> | null = null;
  let lastJson: string | null = null;

  function tick(): void {
    // Dedupe on the world state itself — NOT on a timestamped copy, or the
    // ever-changing clientTimeMs would make every tick look "changed" and the
    // emitter would push a full serialize + WASM call every interval even when
    // nothing moved. We stamp clientTimeMs only when we actually push.
    const snapshot = store.getSnapshot();
    const view = select ? select(snapshot) : snapshot;
    const json = JSON.stringify(view);
    if (json === lastJson) return;
    lastJson = json;
    // Stamp clientTimeMs on the outbound snapshot — additive field, ignored by
    // existing consumers; lets Bevy timestamp/interpolate this push.
    onSnapshot(JSON.stringify({ ...view, clientTimeMs: Date.now() }));
  }

  const emitter: SnapshotEmitter = {
    start() {
      if (timerId !== null) return;
      timerId = setInterval(tick, intervalMs);
    },

    stop() {
      if (timerId === null) return;
      clearInterval(timerId);
      timerId = null;
    },

    get running() {
      return timerId !== null;
    },
  };

  return emitter;
}
