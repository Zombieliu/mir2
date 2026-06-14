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
  const { intervalMs = 16, onSnapshot } = options;

  let timerId: ReturnType<typeof setInterval> | null = null;
  let lastJson: string | null = null;

  function tick(): void {
    // Stamp the snapshot — additive field, backward-compatible.
    const snapshot = { ...store.getSnapshot(), clientTimeMs: Date.now() };
    const json = JSON.stringify(snapshot);
    if (json === lastJson) return;
    lastJson = json;
    onSnapshot(json);
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
