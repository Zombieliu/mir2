/**
 * Lightweight, always-on diagnostic ring buffer + one-shot snapshot exporter.
 *
 * Goal: when a subsystem misbehaves (map render, movement, packets, assets), a dev
 * presses Alt+D (or calls `window.__mir2Debug.dump()`) and gets ONE self-contained
 * JSON — recent packets in/out, captured "[mir2]" warnings + uncaught errors, the
 * existing movement diagnostics, asset-cache state, and a world summary — clean
 * enough to hand straight to an AI for analysis (no browser-extension noise; the
 * cross-cutting "what happened + current state" context is bundled in).
 *
 * Frontend-only; it complements the in-page movement-diagnostics harness
 * (`window.__mir2Movement*`) and the asset HUD (`window.__mir2AssetCache`).
 * Cross-layer (gateway/sim) correlation by packet seq is a separate backend phase.
 */

export type DebugEventKind = "packet-out" | "packet-in" | "error" | "warn" | "anomaly" | "note";

export type DebugEvent = {
  /** epoch ms */
  t: number;
  iso: string;
  kind: DebugEventKind;
  /** subsystem/category: net, asset, render, movement, ui, ... */
  cat: string;
  data: Record<string, unknown>;
};

export type DiagnosticSnapshot = {
  schema: "mir2-debug-snapshot/1";
  sessionId: string;
  capturedAt: string;
  subsystem: string;
  url: string;
  context: Record<string, unknown>;
  counts: Record<string, number>;
  events: DebugEvent[];
  movement?: unknown;
  assetCache?: unknown;
  renderState?: unknown;
  sceneGate?: unknown;
};

const RING_MAX = 600;
const SNAPSHOT_EVENTS = 320;
const DEBUG_SESSION_STORAGE_KEY = "mir2-debug-session-id";
const ring: DebugEvent[] = [];
const counts: Record<string, number> = {};
let installed = false;

export function recordDebugEvent(kind: DebugEventKind, cat: string, data: Record<string, unknown>): void {
  const t = Date.now();
  ring.push({ t, iso: new Date(t).toISOString(), kind, cat, data });
  if (ring.length > RING_MAX) ring.splice(0, ring.length - RING_MAX);
  const key = `${kind}:${cat}`;
  counts[key] = (counts[key] ?? 0) + 1;
}

export function recordDebugError(cat: string, error: unknown, extra?: Record<string, unknown>): void {
  const candidate = error as { message?: unknown; stack?: unknown } | undefined;
  recordDebugEvent("error", cat, {
    message: typeof candidate?.message === "string" ? candidate.message : String(error),
    stack:
      typeof candidate?.stack === "string"
        ? candidate.stack.split("\n").slice(0, 8).join("\n")
        : undefined,
    ...extra,
  });
}

type SnapshotContextProvider = () => Record<string, unknown>;
let contextProvider: SnapshotContextProvider = () => ({});

/** The host (page.tsx) registers a provider returning the current world summary. */
export function setSnapshotContext(provider: SnapshotContextProvider): void {
  contextProvider = provider;
}

type SnapshotRenderStateProvider = () => unknown;
let renderStateProvider: SnapshotRenderStateProvider | null = null;

/**
 * The host (page.tsx) registers a provider returning a compact summary of what the
 * scene renderer is currently drawing (per-layer sprite counts, libraries in use, a
 * small player-centred grid of cell layers, and failed assets). It is computed lazily
 * at capture time so there is zero per-frame cost. Bounded on purpose — see the
 * builder in original-client-scene-map-rendering.ts — because the snapshot payload is
 * already near the MCP read-size limit.
 */
export function setRenderStateProvider(provider: SnapshotRenderStateProvider): void {
  renderStateProvider = provider;
}

function readWindowGlobal(key: string): unknown {
  try {
    return (window as unknown as Record<string, unknown>)[key];
  } catch {
    return undefined;
  }
}

function createDebugSessionId() {
  const suffix =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : Math.random().toString(36).slice(2, 12);
  return `debug-${Date.now().toString(36)}-${suffix}`;
}

function debugSessionId() {
  if (typeof window === "undefined") return createDebugSessionId();
  try {
    const existing = window.sessionStorage.getItem(DEBUG_SESSION_STORAGE_KEY);
    if (existing) return existing;
    const next = createDebugSessionId();
    window.sessionStorage.setItem(DEBUG_SESSION_STORAGE_KEY, next);
    return next;
  } catch {
    return createDebugSessionId();
  }
}

function dispatchSnapshotUploadStatus(detail: Record<string, unknown>) {
  if (typeof window === "undefined") return;
  try {
    window.dispatchEvent(new CustomEvent("mir2:debug-snapshot-upload", { detail }));
  } catch {
    // Snapshot capture must remain best-effort and never break gameplay.
  }
}

export function captureSnapshot(subsystem: string, context?: Record<string, unknown>): DiagnosticSnapshot {
  let ctx: Record<string, unknown>;
  try {
    ctx = { ...contextProvider(), ...(context ?? {}) };
  } catch (error) {
    ctx = { contextError: String(error), ...(context ?? {}) };
  }
  const sessionId = debugSessionId();
  const movementDiagnostics = readWindowGlobal("__mir2MovementDiagnostics");
  const movementRecent = readWindowGlobal("__mir2MovementConsoleEvents");
  let renderState: unknown;
  try {
    renderState = renderStateProvider ? renderStateProvider() : undefined;
  } catch (error) {
    renderState = { error: String(error) };
  }
  return {
    schema: "mir2-debug-snapshot/1",
    sessionId,
    capturedAt: new Date().toISOString(),
    subsystem,
    url: typeof location !== "undefined" ? location.href : "",
    context: { sessionId, ...ctx },
    counts: { ...counts, ringSize: ring.length },
    events: ring.slice(-SNAPSHOT_EVENTS),
    movement:
      movementDiagnostics || movementRecent
        ? { diagnostics: movementDiagnostics, recent: movementRecent }
        : undefined,
    assetCache: readWindowGlobal("__mir2AssetCache"),
    renderState,
    sceneGate: readWindowGlobal("__mir2SceneGate"),
  };
}

async function uploadSnapshot(snapshot: DiagnosticSnapshot): Promise<void> {
  if (typeof fetch === "undefined") return;
  dispatchSnapshotUploadStatus({
    status: "uploading",
    sessionId: snapshot.sessionId,
    message: "诊断快照上传中...",
  });

  try {
    const response = await fetch("/api/debug-snapshots", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        source: "alt-d",
        pageUrl: snapshot.url,
        userAgent: typeof navigator !== "undefined" ? navigator.userAgent : null,
        snapshot,
      }),
      cache: "no-store",
    });
    const result = (await response.json().catch(() => ({}))) as Record<string, unknown>;
    if (!response.ok || result.ok !== true) {
      throw new Error(typeof result.error === "string" ? result.error : `upload failed (${response.status})`);
    }
    const snapshotId = typeof result.id === "number" || typeof result.id === "string" ? result.id : null;
    dispatchSnapshotUploadStatus({
      status: "uploaded",
      sessionId: snapshot.sessionId,
      snapshotId,
      message: snapshotId ? `诊断快照已上传 Debug DB #${snapshotId}` : "诊断快照已上传 Debug DB",
    });
    recordDebugEvent("note", "snapshot", {
      action: "upload",
      status: "ok",
      snapshotId,
      sessionId: snapshot.sessionId,
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    dispatchSnapshotUploadStatus({
      status: "failed",
      sessionId: snapshot.sessionId,
      message: `上传失败，仅已保存本地：${message}`,
    });
    recordDebugEvent("warn", "snapshot", {
      action: "upload",
      status: "failed",
      message,
      sessionId: snapshot.sessionId,
    });
  }
}

/** Capture + download a snapshot JSON, and log it to the console for quick copy. */
export function downloadSnapshot(subsystem: string, context?: Record<string, unknown>): DiagnosticSnapshot {
  const snapshot = captureSnapshot(subsystem, context);
  try {
    console.log("[mir2] diagnostic snapshot", snapshot);
    if (typeof document !== "undefined") {
      const json = JSON.stringify(snapshot, null, 2);
      const blob = new Blob([json], { type: "application/json" });
      const href = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = href;
      anchor.download = `mir2-snapshot-${subsystem}-${new Date()
        .toISOString()
        .replace(/[:.]/g, "-")}.json`;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      setTimeout(() => URL.revokeObjectURL(href), 1000);
      dispatchSnapshotUploadStatus({
        status: "saved",
        sessionId: snapshot.sessionId,
        message: "诊断快照已保存本地",
      });
    }
  } catch (error) {
    console.warn("[mir2] snapshot download failed", error);
  }
  void uploadSnapshot(snapshot);
  return snapshot;
}

/**
 * Install global capture: uncaught errors / rejections, and "[mir2]"-tagged
 * console warnings + errors (so existing diagnostics like the scene-asset-missing
 * warning flow into the ring without touching those call sites). Browser-extension
 * console noise is excluded by the "[mir2]" gate, keeping snapshots clean.
 * Idempotent (safe under React strict-mode double effects).
 */
export function installDebugCapture(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;

  window.addEventListener("error", (ev) => {
    recordDebugError("window", (ev as ErrorEvent).error ?? (ev as ErrorEvent).message, {
      filename: (ev as ErrorEvent).filename,
      line: (ev as ErrorEvent).lineno,
    });
  });
  window.addEventListener("unhandledrejection", (ev) => {
    recordDebugError("promise", (ev as PromiseRejectionEvent).reason);
  });

  const isMir2 = (value: unknown): value is string => typeof value === "string" && value.includes("[mir2]");
  const originalWarn = console.warn.bind(console);
  const originalError = console.error.bind(console);
  console.warn = (...args: unknown[]) => {
    if (isMir2(args[0])) {
      try {
        recordDebugEvent("warn", "console", { message: args[0], detail: args[1] });
      } catch {
        /* never let capture break logging */
      }
    }
    originalWarn(...args);
  };
  console.error = (...args: unknown[]) => {
    if (isMir2(args[0])) {
      try {
        recordDebugEvent("error", "console", { message: args[0], detail: args[1] });
      } catch {
        /* never let capture break logging */
      }
    }
    originalError(...args);
  };

  (window as unknown as Record<string, unknown>).__mir2Debug = {
    dump: (subsystem = "manual", context?: Record<string, unknown>) => downloadSnapshot(subsystem, context),
    snapshot: (subsystem = "manual", context?: Record<string, unknown>) => captureSnapshot(subsystem, context),
    events: () => ring.slice(),
    counts: () => ({ ...counts }),
    clear: () => {
      ring.length = 0;
    },
    record: recordDebugEvent,
  };
}
