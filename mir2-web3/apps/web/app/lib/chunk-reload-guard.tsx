"use client";

import { useEffect } from "react";

/**
 * Next.js ChunkLoadError resilience.
 *
 * When a deployment ships new hashed chunks, an already-open tab can fail a
 * dynamic `import()` because the old chunk URL is gone (404) or the network
 * blipped. Next surfaces this as a `ChunkLoadError` / "Loading chunk failed" /
 * "Failed to fetch dynamically imported module". Left unhandled it floods the
 * console and hard-fails subsystems that lazy-load (e.g. the mobile joystick).
 *
 * Strategy:
 *   1. First time we see a chunk failure, schedule a single full reload after a
 *      short delay so the page picks up the fresh manifest. The browser will
 *      naturally retry the failed dynamic import after reload.
 *   2. Guard the reload via sessionStorage so a chunk that is genuinely broken
 *      (not just stale) can never put us in an infinite reload loop.
 *
 * The guard is idempotent and SSR-safe; install it once from a client
 * component (see `installChunkReloadGuard`).
 */

const RELOAD_GUARD_KEY = "mir2:chunk-reload-guard";
// If we reloaded for a chunk error within this window, do NOT reload again —
// the chunk is broken, not stale, and another reload would loop forever.
const RELOAD_GUARD_WINDOW_MS = 30_000;
// Small delay so multiple near-simultaneous chunk failures collapse into one
// reload instead of racing each other.
const RELOAD_DELAY_MS = 600;

type GuardWindow = Window & {
  __mir2ChunkReloadGuardInstalled?: boolean;
};

function isChunkLoadError(value: unknown): boolean {
  if (!value) return false;

  // Native ChunkLoadError carries a `name`.
  const name = (value as { name?: unknown }).name;
  if (typeof name === "string" && name === "ChunkLoadError") return true;

  const message =
    (value as { message?: unknown }).message ??
    (typeof value === "string" ? value : undefined);
  if (typeof message !== "string") return false;

  return (
    message.includes("ChunkLoadError") ||
    /Loading( CSS)? chunk [\w-]+ failed/i.test(message) ||
    message.includes("Failed to fetch dynamically imported module") ||
    message.includes("error loading dynamically imported module") ||
    message.includes("Importing a module script failed")
  );
}

function recentlyReloaded(): boolean {
  try {
    const raw = window.sessionStorage.getItem(RELOAD_GUARD_KEY);
    if (!raw) return false;
    const at = Number.parseInt(raw, 10);
    if (!Number.isFinite(at)) return false;
    return Date.now() - at < RELOAD_GUARD_WINDOW_MS;
  } catch {
    // sessionStorage can throw (privacy mode / disabled storage). Without it we
    // cannot guard against loops, so refuse to auto-reload at all.
    return true;
  }
}

function markReloaded(): boolean {
  try {
    window.sessionStorage.setItem(RELOAD_GUARD_KEY, String(Date.now()));
    return true;
  } catch {
    return false;
  }
}

/**
 * Installs the global chunk-load failure handler. Safe to call multiple times;
 * only the first call wires up listeners. Returns a cleanup function that
 * removes them (used by React effect teardown).
 */
export function installChunkReloadGuard(): () => void {
  if (typeof window === "undefined") return () => {};

  const guardWindow = window as GuardWindow;
  if (guardWindow.__mir2ChunkReloadGuardInstalled) return () => {};
  guardWindow.__mir2ChunkReloadGuardInstalled = true;

  let reloadTimer = 0;

  const triggerReloadOnce = () => {
    if (reloadTimer) return; // A reload is already pending for this page life.
    if (recentlyReloaded()) {
      // We already reloaded recently for a chunk error and it failed again —
      // the chunk is genuinely broken. Stop here so we don't loop, but keep the
      // rejection handled (no console flood, no crashed subsystem).
      console.warn("[mir2] chunk load failed again after reload; not reloading");
      return;
    }
    if (!markReloaded()) {
      console.warn("[mir2] chunk load failed; cannot guard reload (no sessionStorage)");
      return;
    }
    console.warn("[mir2] chunk load failed; reloading once to fetch fresh assets");
    reloadTimer = window.setTimeout(() => {
      window.location.reload();
    }, RELOAD_DELAY_MS);
  };

  const onError = (event: ErrorEvent) => {
    if (!isChunkLoadError(event.error) && !isChunkLoadError(event.message)) return;
    triggerReloadOnce();
  };

  const onRejection = (event: PromiseRejectionEvent) => {
    if (!isChunkLoadError(event.reason)) return;
    // Mark handled so it does not surface as an uncaught rejection (the console
    // flood we are trying to stop), then recover via a single reload.
    event.preventDefault();
    triggerReloadOnce();
  };

  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);

  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
    if (reloadTimer) window.clearTimeout(reloadTimer);
    guardWindow.__mir2ChunkReloadGuardInstalled = false;
  };
}

/**
 * Renderless client component that installs the chunk-reload guard for the app
 * lifetime. Mount it once near the root (alongside other app-level effects).
 */
export function ChunkReloadGuard() {
  useEffect(() => installChunkReloadGuard(), []);
  return null;
}
