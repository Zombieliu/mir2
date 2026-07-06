"use client";

/**
 * mir2-probe — dev overlay (P2-5).
 *
 * A small, fixed-position semi-transparent panel that periodically calls
 * `window.__mir2Probe.snapshot()` and renders a compact summary across the six
 * layers. Shown only when the URL opt-in `?probe=1` (or `?probe=<label>`) is
 * set, so the per-sample polling cost is opt-in.
 *
 * Constraints:
 *   - Panel state lives entirely inside this component; it pushes nothing
 *     into the parent page's React tree (it renders through a portal into a
 *     sibling div appended to <body>).
 *   - Polling is 1 Hz. The overlay is for human inspection; faster polling can
 *     itself contribute to the main-thread churn the probe is trying to measure.
 *   - Zero gameplay effects: no button dispatches movement commands, no
 *     router navigation, no clipboard interaction. A single [JSON] button
 *     downloads the latest ProbeSample; nothing else.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import type { ProbeSample } from "./schema";

const OVERLAY_POLL_INTERVAL_MS = 1000;

function readWindowProbe(): {
  snapshot?: (options?: { profile?: "full" | "overlay" | "movement" }) => ProbeSample;
  label?: () => string;
  start?: () => void;
  stop?: () => void;
  running?: () => boolean;
} | null {
  if (typeof window === "undefined") return null;
  const handle = (window as unknown as { __mir2Probe?: unknown }).__mir2Probe;
  if (!handle || typeof handle !== "object") return null;
  return handle as {
    snapshot?: () => ProbeSample;
    label?: () => string;
    start?: () => void;
    stop?: () => void;
    running?: () => boolean;
  };
}

function isProbeEnabledInQuery(): boolean {
  if (typeof window === "undefined") return false;
  const p = new URLSearchParams(window.location.search).get("probe");
  return p !== null && p !== "0" && p !== "false" && p !== "off";
}

function fmtMs(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "—";
  if (ms < 1) return `${Math.round(ms * 1000)}μs`;
  return `${Math.round(ms)}ms`;
}

function fmtInt(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  return String(Math.round(n));
}

function colorForMs(value: number | null | undefined, warn: number, bad: number): string {
  if (value === null || value === undefined) return "var(--mir2-probe-c-dim)";
  if (value >= bad) return "var(--mir2-probe-c-bad)";
  if (value >= warn) return "var(--mir2-probe-c-warn)";
  return "var(--mir2-probe-c-ok)";
}

function colorForFps(value: number | null, okAt: number, warnAt: number): string {
  if (value === null) return "var(--mir2-probe-c-dim)";
  if (value < warnAt) return "var(--mir2-probe-c-bad)";
  if (value < okAt) return "var(--mir2-probe-c-warn)";
  return "var(--mir2-probe-c-ok)";
}

type OverlayRowProps = {
  label: string;
  value: string;
  valueColor?: string;
  hint?: string;
};

type MetricSummary = {
  count?: number | null;
  msMax?: number | null;
  msLast?: number | null;
  msAvg?: number | null;
};

type GatewayMetricsView = {
  tick?: MetricSummary | null;
  save?: MetricSummary | null;
  worldSnapshotCount?: number | null;
  sharedTick?: {
    pending?: MetricSummary | null;
    expire?: MetricSummary | null;
    command?: MetricSummary | null;
    autoPickup?: MetricSummary | null;
    observer?: MetricSummary | null;
    observerDetail?: {
      npc?: MetricSummary | null;
      apply?: MetricSummary | null;
      shared?: MetricSummary | null;
      drops?: MetricSummary | null;
      quest?: MetricSummary | null;
      zoneObserver?: MetricSummary | null;
    } | null;
    zone?: MetricSummary | null;
    pendingMsLast?: number | null;
    expireMsLast?: number | null;
    commandMsLast?: number | null;
    autoPickupMsLast?: number | null;
    observerMsLast?: number | null;
    zoneMsLast?: number | null;
  } | null;
};

function Row({ label, value, valueColor, hint }: OverlayRowProps) {
  return (
    <div className="mir2-probe-row">
      <span className="mir2-probe-row-label" title={hint}>{label}</span>
      <span
        className="mir2-probe-row-value"
        style={valueColor ? { color: valueColor } : undefined}
      >
        {value}
      </span>
    </div>
  );
}

function downloadSampleJson(sample: ProbeSample | null, label: string) {
  if (!sample || typeof document === "undefined") return;
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const blob = new Blob([JSON.stringify(sample, null, 2)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `mir2-probe-${label || "unlabelled"}-${stamp}.json`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 1500);
}

function metricHasSamples(metric: MetricSummary | null | undefined): boolean {
  return (metric?.count ?? 0) > 0;
}

function metricLastForColor(metric: MetricSummary | null | undefined): number | null | undefined {
  return metricHasSamples(metric) ? metric?.msLast : undefined;
}

function metricMaxForColor(metric: MetricSummary | null | undefined): number | null | undefined {
  return metricHasSamples(metric) ? metric?.msMax : undefined;
}

function fmtMetricLastAvg(metric: MetricSummary | null | undefined): string {
  if (!metric) return "—";
  const count = fmtInt(metric.count);
  if (!metricHasSamples(metric)) return `— ×${count}`;
  return `${fmtMs(metric.msLast)}/${fmtMs(metric.msAvg)} ×${count}`;
}

function fmtMetricMax(metric: MetricSummary | null | undefined): string {
  if (!metric) return "—";
  const count = fmtInt(metric.count);
  if (!metricHasSamples(metric)) return `— ×${count}`;
  return `${fmtMs(metric.msMax)} ×${count}`;
}

function sharedTickPhaseMax(
  phase: MetricSummary | null | undefined,
  legacyLast: number | null | undefined,
): number | null | undefined {
  if (phase) return phase.msMax ?? phase.msLast;
  return legacyLast;
}

function recordFromUnknown(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" ? value as Record<string, unknown> : null;
}

function numFromRecord(record: Record<string, unknown> | null, key: string): number | null {
  const value = record?.[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function strFromRecord(record: Record<string, unknown> | null, key: string): string | null {
  const value = record?.[key];
  return typeof value === "string" && value.length > 0 ? value : null;
}

export function Mir2ProbeOverlay() {
  const [sample, setSample] = useState<ProbeSample | null>(null);
  const [mounted, setMounted] = useState(false);
  const sampleRef = useRef<ProbeSample | null>(null);

  useEffect(() => {
    if (!isProbeEnabledInQuery()) return;
    setMounted(true);

    const tick = () => {
      const handle = readWindowProbe();
      if (!handle?.snapshot) return;
      try {
        const s = handle.snapshot({ profile: "overlay" });
        sampleRef.current = s;
        setSample(s);
      } catch {
        /* never let the overlay break gameplay */
      }
    };
    tick();
    const id = window.setInterval(tick, OVERLAY_POLL_INTERVAL_MS);
    return () => {
      window.clearInterval(id);
    };
  }, []);

  const label = useMemo(() => {
    const handle = readWindowProbe();
    return handle?.label?.() ?? "";
  }, [sample]);

  // Mount portal as a direct child of <body> so the overlay never participates
  // in the scene/game layout transformations.
  const host = useMemo(() => {
    if (typeof document === "undefined") return null;
    if (!isProbeEnabledInQuery()) return null;
    let node = document.querySelector("#mir2-probe-overlay-host");
    if (!node) {
      node = document.createElement("div");
      node.id = "mir2-probe-overlay-host";
      document.body.appendChild(node);
    }
    return node;
  }, []);

  if (!mounted) return null;
  if (!host) return null;

  const f = sample?.frame;
  const m = sample?.movement;
  const s = sample?.stage;
  const b = sample?.bevy;
  const r = sample?.residency;
  const n = sample?.network;
  const g = sample?.gateway;
  const gatewayMetrics = g?.metrics as GatewayMetricsView | null | undefined;

  const fpsColor = f ? colorForFps(f.fpsInLastSecond, 50, 30) : undefined;
  const rafColor = f ? colorForMs(f.rafDeltaAvg, 20, 33) : undefined;
  const rafMaxColor = f ? colorForMs(f.rafDeltaMax, 50, 120) : undefined;
  const driftColor = f ? colorForMs(f.timerDriftMax, 50, 120) : undefined;
  const ltColor = f ? colorForMs(f.longestLongtaskMs, 100, 200) : undefined;
  const tickMsColor = gatewayMetrics
    ? colorForMs(metricLastForColor(gatewayMetrics.tick), 50, 200)
    : undefined;
  const saveMsColor = gatewayMetrics
    ? colorForMs(metricLastForColor(gatewayMetrics.save), 50, 250)
    : undefined;
  const lastMovementRx = m?.receivedPackets[0];
  const lastMovementTx = m?.sentCommands[0];
  const movementAckValue =
    m?.lastAckOutcome ??
    (lastMovementRx ? `rx ${String(lastMovementRx.packet ?? "?")}` : null) ??
    (lastMovementTx ? `tx ${String(lastMovementTx.type ?? "?")}` : null) ??
    "—";
  const movementQueueValue =
    m?.movementQueueDepth !== null && m?.movementQueueDepth !== undefined
      ? `${fmtInt(m.movementQueueDepth)} d${fmtInt(m.diagnosticEventCount)}`
      : m
        ? `tx${m.sentCommands.length}/rx${m.receivedPackets.length}`
        : "—";
  const gatewaySharedTick = gatewayMetrics?.sharedTick;
  const gatewaySharedTickCommandMax = sharedTickPhaseMax(
    gatewaySharedTick?.command,
    gatewaySharedTick?.commandMsLast,
  );
  const gatewaySharedTickExpireMax = sharedTickPhaseMax(
    gatewaySharedTick?.expire,
    gatewaySharedTick?.expireMsLast,
  );
  const gatewaySharedTickZoneMax = sharedTickPhaseMax(
    gatewaySharedTick?.zone,
    gatewaySharedTick?.zoneMsLast,
  );
  const gatewaySharedTickAutoPickupMax = sharedTickPhaseMax(
    gatewaySharedTick?.autoPickup,
    gatewaySharedTick?.autoPickupMsLast,
  );
  const gatewaySharedTickValue = gatewaySharedTick
    ? `${fmtMs(gatewaySharedTickCommandMax)}/${fmtMs(gatewaySharedTickExpireMax)}/${fmtMs(gatewaySharedTickZoneMax)}/${fmtMs(gatewaySharedTickAutoPickupMax)}`
    : null;
  const gatewayObserverDetail = gatewaySharedTick?.observerDetail;
  const gatewayObserverDetailValue = gatewayObserverDetail
    ? [
        gatewayObserverDetail.npc,
        gatewayObserverDetail.apply,
        gatewayObserverDetail.shared,
        gatewayObserverDetail.drops,
        gatewayObserverDetail.quest,
        gatewayObserverDetail.zoneObserver,
      ].map((phase) => fmtMs(phase?.msMax ?? phase?.msLast)).join("/")
    : null;
  const tickMaxColor = gatewayMetrics
    ? colorForMs(metricMaxForColor(gatewayMetrics.tick), 50, 200)
    : undefined;
  const resolveStats = recordFromUnknown(r?.resolveStats);
  const cacheSnapshot = recordFromUnknown(r?.cacheSummary);
  const cacheSummary = recordFromUnknown(cacheSnapshot?.summary) ?? cacheSnapshot;
  const atlasResolveValue = resolveStats
    ? `${strFromRecord(resolveStats, "lastSource") ?? "—"} ${fmtMs(numFromRecord(resolveStats, "lastBuildMs"))} n${fmtInt(numFromRecord(resolveStats, "lastSourceCount"))}`
    : "—";
  const resourceCacheValue = cacheSummary
    ? `${fmtInt(numFromRecord(cacheSummary, "resourceCount"))}/${fmtInt(numFromRecord(cacheSummary, "cachedLikeCount"))} s${fmtInt(numFromRecord(cacheSummary, "sceneHits"))}/${fmtInt(numFromRecord(cacheSummary, "sceneMisses"))}`
    : "—";
  const stageValue = s
    ? `${s.screen ?? "—"} ${s.wsState ?? "—"} p=${s.playerObjectId ?? "—"}`
    : "—";
  const stagePositionValue = s?.player
    ? `s${fmtInt(s.player.serverX)},${fmtInt(s.player.serverY)} r${fmtInt(s.player.renderX)},${fmtInt(s.player.renderY)}`
    : "—";
  const sceneReadyValue = s?.sceneAssetReadiness
    ? `${s.sceneAssetReadiness.status ?? "—"} ${fmtInt(s.sceneAssetReadiness.loaded)}/${fmtInt(s.sceneAssetReadiness.total)} p${fmtInt(s.sceneAssetReadiness.pending)}`
    : s
      ? `${s.sceneInteractionReady === true ? "ready" : "wait"}`
      : "—";

  return createPortal(
    <section aria-label="mir2-probe dev overlay" className="mir2-probe-overlay" role="status">
      <header className="mir2-probe-head">
        <span>mir2-probe</span>
        <span className="mir2-probe-label">{label || "(unlabelled)"}</span>
      </header>
      <Row
        label="fps"
        hint="Estimated frames/sec from rAF deltas (last ~1s)"
        value={f?.fpsInLastSecond !== null && f?.fpsInLastSecond !== undefined ? `${f.fpsInLastSecond}` : "—"}
        valueColor={fpsColor}
      />
      <Row
        label="raf"
        hint="Average rAF frame delta in ms"
        value={fmtMs(f?.rafDeltaAvg)}
        valueColor={rafColor}
      />
      <Row
        label="raf max"
        hint="Worst rAF frame delta over the last ~30 frames"
        value={fmtMs(f?.rafDeltaMax)}
        valueColor={rafMaxColor}
      />
      <Row
        label="drift"
        hint="Worst 100ms timer drift over the last ~30 samples; catches main-thread stalls even when LongTask is unavailable"
        value={fmtMs(f?.timerDriftMax)}
        valueColor={driftColor}
      />
      <Row
        label="lt/s"
        hint="LongTasks (>50ms) in last 1s"
        value={fmtInt(f?.longtasksInLastSecond)}
      />
      <Row
        label="lt max"
        hint="Longest single LongTask in last 1s"
        value={fmtMs(f?.longestLongtaskMs)}
        valueColor={ltColor}
      />
      <Row
        label="ack"
        hint="Most recent movement ack outcome; falls back to latest tx/rx movement packet when movementDiag is off"
        value={movementAckValue}
      />
      <Row
        label="mQ"
        hint="Client movement queue depth; dN is diagnostic event buffer depth, not unacked movement"
        value={movementQueueValue}
      />
      <Row
        label="stage"
        hint="Client screen, WebSocket state, and current player object id"
        value={stageValue}
      />
      <Row
        label="pos"
        hint="Self player tile: server-authoritative sX,Y and render/predicted rX,Y"
        value={stagePositionValue}
      />
      <Row
        label="ready"
        hint="Scene asset readiness status: loaded/total and pending"
        value={sceneReadyValue}
      />
      <Row
        label="bevy"
        hint="Runtime backend and last publish_status phase"
        value={b ? `${b.backend ?? "—"} ${b.lastPhase ?? "—"}` : "—"}
      />
      <Row
        label="mem"
        hint="Entity-atlas hot cache size / evictions; not total scene-resource residency"
        value={
          r?.stats
            ? `${(r.stats as { memoryCacheSize?: number }).memoryCacheSize ?? "—"}/ev=${(r.stats as { memoryEvictions?: number }).memoryEvictions ?? 0}`
            : "—"
        }
      />
      <Row
        label="atlas"
        hint="Entity-atlas resolve source, last build/load ms, and source frame count"
        value={atlasResolveValue}
      />
      <Row
        label="res"
        hint="Resource count / cached-like count; scene cache hits/misses"
        value={resourceCacheValue}
      />
      <Row
        label="idb"
        hint="Last IDB get/put/listByAge ms"
        value={
          r?.idbTimings
            ? `g=${fmtMs(r.idbTimings.lastGetMs)} p=${fmtMs(r.idbTimings.lastPutMs)} l=${fmtMs(r.idbTimings.lastListByAgeMs)}`
            : "—"
        }
      />
      <Row
        label="net"
        hint="sent/received packet rings / ws state"
        value={
          n
            ? `↑${n.commandHistory.length}/↓${n.gatewayEventHistory.length}`
            : "—"
        }
      />
      <Row
        label="gTick"
        hint="Gateway session.tick() wall time: last/avg from /metrics, not tick interval or input latency"
        value={
          gatewayMetrics
            ? fmtMetricLastAvg(gatewayMetrics.tick)
            : "—"
        }
        valueColor={tickMsColor}
      />
      <Row
        label="gMax"
        hint="Gateway session.tick() worst observed wall time since gateway startup"
        value={
          gatewayMetrics
            ? fmtMetricMax(gatewayMetrics.tick)
            : "—"
        }
        valueColor={tickMaxColor}
      />
      {gatewaySharedTickValue ? (
        <Row
          label="gPart"
          hint="Gateway shared tick max parts: command/expire/zone/auto-pickup"
          value={gatewaySharedTickValue}
          valueColor={tickMaxColor}
        />
      ) : null}
      {gatewayObserverDetailValue ? (
        <Row
          label="gObs"
          hint="Gateway observer max parts: npc/apply/shared/drops/quest/zone-observer"
          value={gatewayObserverDetailValue}
          valueColor={tickMaxColor}
        />
      ) : null}
      <Row
        label="save"
        hint="Gateway save_active_character wall time: last/avg from /metrics; — ×0 means no measured saves yet"
        value={
          gatewayMetrics
            ? fmtMetricLastAvg(gatewayMetrics.save)
            : "—"
        }
        valueColor={saveMsColor}
      />
      <Row
        label="snap"
        hint="worldSnapshot pushes since gateway startup (cumulative count)"
        value={
          gatewayMetrics
            ? fmtInt(gatewayMetrics.worldSnapshotCount)
            : "—"
        }
      />
      <footer className="mir2-probe-foot">
        <button
          type="button"
          onClick={() => downloadSampleJson(sampleRef.current, label)}
          className="mir2-probe-btn"
        >
          JSON
        </button>
        <button
          type="button"
          onClick={() => {
            const handle = readWindowProbe();
            if (!handle) return;
            if (handle.running?.() ?? false) handle.stop?.();
            else handle.start?.();
            // trigger immediate re-render
            setSample({ ...(sampleRef.current as ProbeSample) });
          }}
          className="mir2-probe-btn"
        >
          {/* toggled by start/stop; the actual running state is read on re-render */}
          {readWindowProbe()?.running?.() ? "Stop" : "Start"}
        </button>
      </footer>
    </section>,
    host,
  );
}
