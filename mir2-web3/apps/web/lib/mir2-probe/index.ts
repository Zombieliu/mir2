/**
 * mir2-probe — top-level SDK exposed at window.__mir2Probe (P1).
 *
 * P1 contract:
 *   - install() is idempotent and safe under React strict-mode double-mount.
 *   - snapshot() never throws; missing layers fall back to null/empty arrays.
 *   - start()/stop() toggle the passive frame probe (longtask + rAF).
 *   - No overlay UI in P1 — that is P2.
 *   - No mutation of any existing window.__mir2* global.
 *
 * Producers live in ./bus.ts and read existing window globals. The single
 * genuinely new observation here is the frame probe (./frame-probe.ts), which
 * is purely additive (its own PerformanceObserver + rAF loop, no coupling to
 * the 30Hz motion clock in original-client-shell.tsx).
 *
 * Gate: the SDK is always installed in the browser (the existence of even a
 * `null` `__mir2Probe` reference is useful for live console debugging and for
 * downstream QA scripts). A label is captured at install time from the URL
 * query `?probe=label` or `?probeLabel=label` for correlation with a repro run;
 * `?probe=label` also enables the visual overlay, while `probeLabel` is useful
 * for headless runs that should not mount overlay UI.
 */

import {
  isFrameProbeRunning,
  startFrameProbe,
  stopFrameProbe,
} from "./frame-probe";
import { wrapBevyStatusSink } from "./bevy-status-probe";
import {
  readBevyLayer,
  readFrameLayer,
  readGatewayLayer,
  readMovementLayer,
  readNetworkLayer,
  readResidencyLayer,
  readStageLayer,
  startGatewayMetricsPoller,
  stopGatewayMetricsPoller,
} from "./bus";
import type { ProbeHandle, ProbeSample, ProbeSnapshotOptions, ProbeSnapshotProfile } from "./schema";

const SCHEMA = "mir2-probe/1" as const;

let installed = false;
let sessionLabel = "";
let sessionId = "probe-uninitialized";
let started = false;

function createSessionId(): string {
  try {
    const rnd =
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : Math.random().toString(36).slice(2, 12);
    return `probe-${Date.now().toString(36)}-${rnd}`;
  } catch {
    return `probe-${Date.now().toString(36)}`;
  }
}

function readUrlLabel(): string {
  if (typeof window === "undefined" || typeof URLSearchParams === "undefined") return "";
  try {
    const p = new URLSearchParams(window.location.search);
    const v = p.get("probe") ?? p.get("probeLabel") ?? "";
    if (typeof v !== "string") return "";
    if (v === "1" || v === "true" || v === "on" || v === "yes") return "on";
    if (v.length > 0 && v.length <= 64) return v;
    return "";
  } catch {
    return "";
  }
}

export function installProbe(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  sessionId = createSessionId();
  sessionLabel = readUrlLabel();
  started = false;
  const handle: ProbeHandle = {
    schema: SCHEMA,
    sessionId,
    snapshot,
    start,
    stop,
    running: () => isFrameProbeRunning(),
    label: () => sessionLabel,
  };
  (window as unknown as { __mir2Probe?: ProbeHandle }).__mir2Probe = handle;
}

function snapshot(options?: ProbeSnapshotOptions): ProbeSample {
  const now = perfNow();
  const epoch = epochNow();
  const label = (options?.label && options.label.length > 0 && options.label.length <= 64
    ? options.label
    : sessionLabel);
  const profile = normalizeProfile(options?.profile);
  const movementMaxRecords = profile === "movement" ? 16 : 50;
  return {
    schema: SCHEMA,
    t: now,
    epoch,
    sessionId,
    label,
    movement: readMovementLayer(epoch, {
      maxRecords: movementMaxRecords,
      maxKeyboardEvents: profile === "movement" ? 24 : 100,
      maxDiagnosticEvents: profile === "movement" ? 120 : 240,
    }),
    stage: readStageLayer(),
    bevy: readBevyLayer(),
    residency: profile === "movement" ? emptyResidencyLayer() : readResidencyLayer(),
    network: profile === "movement" ? emptyNetworkLayer() : readNetworkLayer(epoch),
    gateway: readGatewayLayer(),
    frame: readFrameLayer(),
  };
}

function normalizeProfile(profile: ProbeSnapshotProfile | undefined): ProbeSnapshotProfile {
  if (profile === "overlay" || profile === "movement") return profile;
  return "full";
}

function emptyResidencyLayer(): ProbeSample["residency"] {
  return {
    stats: null,
    resolveStats: null,
    cacheSummary: null,
    idbTimings: { lastGetMs: null, lastPutMs: null, lastListByAgeMs: null },
  };
}

function emptyNetworkLayer(): ProbeSample["network"] {
  return {
    gatewayEventHistory: [],
    commandHistory: [],
    counts: {},
    wsReadyState: null,
    msSinceLastTxMovement: null,
  };
}

function start(): void {
  if (started) return;
  started = true;
  startFrameProbe();
  startGatewayMetricsPoller();
}

function stop(): void {
  if (!started) return;
  started = false;
  stopFrameProbe();
  stopGatewayMetricsPoller();
}

function perfNow(): number {
  return typeof performance !== "undefined" ? performance.now() : 0;
}

function epochNow(): number {
  return typeof Date !== "undefined" ? Date.now() : 0;
}

export function getProbeHandle(): ProbeHandle | null {
  if (typeof window === "undefined") return null;
  return ((window as unknown as { __mir2Probe?: ProbeHandle | undefined }).__mir2Probe) ?? null;
}

export { wrapBevyStatusSink } from "./bevy-status-probe";
export { getResidencyIdbTimings } from "../asset-residency/browser-adapters";
