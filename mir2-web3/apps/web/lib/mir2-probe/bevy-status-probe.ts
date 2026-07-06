/**
 * mir2-probe — Bevy status bridge (L2).
 *
 * The wasm runtime already emits `{ phase, message }` events through
 * `set_mir2_status_sink(callback)` in lib.rs:1957. We do NOT touch the Rust
 * side; instead this module exposes a tiny ring buffer + a wrapCallback helper
 * that the page.tsx side calls when it installs its own status sink.
 *
 * Result: `readBevyStatus()` returns the most recently published phase + the
 * recent phase/message ring; a probe sample() reads this synchronously.
 *
 * Why keep this separate from frame-probe.ts: frame-probe is purely
 * passive PerformanceObserver/rAF and will run unconditionally when the SDK is
 * started. The status bridge is purely passive too but its data source is
 * callback-shaped and conceptually crosses the wasm→JS boundary. Keeping them
 * in separate modules prevents the frame probe from becoming a dumping ground
 * for every cross-layer reader.
 */

export type BevyStatusEntry = {
  /** Performance.now() at the moment the status was received. */
  atMs: number;
  /** Date.now() epoch — for cross-session correlation. */
  epoch: number;
  phase: string;
  message: string;
};

const RING_MAX = 64;
const ring: BevyStatusEntry[] = [];
let lastEntry: BevyStatusEntry | null = null;

export function recordBevyStatus(payload: unknown): void {
  if (!payload || typeof payload !== "object") return;
  const p = payload as { phase?: unknown; message?: unknown };
  const phase = typeof p.phase === "string" ? p.phase : "";
  const message = typeof p.message === "string" ? p.message : "";
  const entry: BevyStatusEntry = {
    atMs: typeof performance !== "undefined" ? performance.now() : Date.now(),
    epoch: typeof Date !== "undefined" ? Date.now() : 0,
    phase,
    message,
  };
  ring.push(entry);
  if (ring.length > RING_MAX) ring.splice(0, ring.length - RING_MAX);
  lastEntry = entry;
}

/**
 * Wrap a caller's status sink callback so that every published status also
 * feeds the probe ring. The original callback is invoked unchanged so existing
 * page.tsx behaviour (setRuntimePhase / setRuntimeMessage) is preserved.
 */
export function wrapBevyStatusSink<T>(original: (status: { phase: string; message: string }) => T) {
  return function (this: unknown, status: { phase: string; message: string }) {
    try {
      recordBevyStatus(status);
    } catch {
      /* never let the probe break the real sink */
    }
    return original.call(this, status);
  };
}

export function readBevyStatus(): {
  lastPhase: string | null;
  lastMessage: string | null;
  lastAtMs: number | null;
  ring: BevyStatusEntry[];
} {
  return {
    lastPhase: lastEntry?.phase ?? null,
    lastMessage: lastEntry?.message ?? null,
    lastAtMs: lastEntry?.atMs ?? null,
    ring: ring.slice(),
  };
}
