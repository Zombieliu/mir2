/**
 * mir2-probe — cross-layer data bus (P1).
 *
 * Producers for each of the six data layers. Every producer is an *adapter*
 * over an existing window global; we treat it as best-effort and return `null`
 * on any access failure so a sample() call never throws into gameplay.
 *
 * NOTE: we deliberately do NOT touch the existing global state here — read
 * only. Some fields are intentionally null in P1 because their producers live
 * on the Bevy/runtime and gateway sides (P2):
 *
 *   - bevy.systemMs / bevy.counters — requires publish_status extension
 *   - residency.idbTimings — requires wrapping idbRequest/idbTransactionDone
 *   - gateway.metrics — requires a new /metrics HTTP route
 *
 * The schema already declares these as `T | null` so P2 is additive: each
 * producer simply stops returning null and downstream consumers update.
 */

import type {
  ProbeBevyLayer,
  ProbeFrameLayer,
  ProbeGatewayLayer,
  ProbeMovementLayer,
  ProbeNetworkLayer,
  ProbeResidencyLayer,
  ProbeStageLayer,
} from "./schema";
import { readBevyStatus } from "./bevy-status-probe";
import { readFrameProbe } from "./frame-probe";
import {
  isGatewayProbeRunning,
  readGatewayLayer as readGatewayLayerFromPoller,
  startGatewayProbe,
  stopGatewayProbe,
} from "./gateway-fetch";

type Windowish = Window &
  typeof globalThis & {
    [k: string]: unknown;
  };

function readWindow<T>(key: string): T | null {
  if (typeof window === "undefined") return null;
  try {
    const v = (window as Windowish)[key];
    if (v === undefined || v === null) return null;
    return v as T;
  } catch {
    return null;
  }
}

function readWindowObj(key: string): Record<string, unknown> | null {
  const v = readWindow<unknown>(key);
  return v && typeof v === "object" ? (v as Record<string, unknown>) : null;
}

function readArr(key: string): Array<Record<string, unknown>> {
  const v = readWindow<unknown>(key);
  if (Array.isArray(v)) return v as Array<Record<string, unknown>>;
  return [];
}

function readNumberField(o: Record<string, unknown>, key: string): number | null {
  const v = o[key];
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}

function readStringField(o: Record<string, unknown>, key: string): string | null {
  const v = o[key];
  return typeof v === "string" ? v : null;
}

function readBooleanField(o: Record<string, unknown>, key: string): boolean | null {
  const v = o[key];
  return typeof v === "boolean" ? v : null;
}

function readRecordField(o: Record<string, unknown>, key: string): Record<string, unknown> | null {
  const v = o[key];
  return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
}

function recordFromUnknown(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

function readArrayField(o: Record<string, unknown>, key: string): Array<Record<string, unknown>> {
  const v = o[key];
  return Array.isArray(v) ? (v as Array<Record<string, unknown>>) : [];
}

function readStringishField(o: Record<string, unknown>, key: string): string | null {
  const v = o[key];
  if (typeof v === "string") return v;
  if (typeof v === "number" && Number.isFinite(v)) return String(v);
  return null;
}

export function readMovementLayer(
  now: number,
  options: { maxRecords?: number; maxKeyboardEvents?: number; maxDiagnosticEvents?: number } = {},
): ProbeMovementLayer {
  const maxRecords = options.maxRecords ?? 50;
  const maxKeyboardEvents = options.maxKeyboardEvents ?? 100;
  const maxDiagnosticEvents = options.maxDiagnosticEvents ?? 240;
  const logEnabled = readWindow<boolean>("__mir2MovementLogEnabled") === true;
  const diag = readWindowObj("__mir2MovementDiagnostics");
  const sessionId = diag ? readStringField(diag, "sessionId") : null;
  const pendingEvents = diag ? readNumberField(diag, "pendingEvents") : null;
  const lastEvent = diag ? (diag.lastEvent as Record<string, unknown> | undefined) : undefined;
  const diagnosticEvents = diag ? readArrayField(diag, "events").slice(-maxDiagnosticEvents) : [];

  const sentCommands = mergeMovementRecords(
    readArr("__mir2MovementSentCommands"),
    sentCommandsFromDiagnosticEvents(diagnosticEvents),
  );
  const receivedPackets = mergeMovementRecords(
    readArr("__mir2MovementReceivedPackets"),
    receivedPacketsFromDiagnosticEvents(diagnosticEvents),
  );
  const keyboardEvents = readArr("__mir2KeyboardMoveEvents");
  const shellRenderPerf = readArr("__mir2ShellRenderPerf");
  const gatewayEventHistory = readArr("__mir2GatewayEventHistory");
  void gatewayEventHistory; // correlated in network layer

  const lastSample = diag ? ((diag.lastSample as Record<string, unknown> | undefined) ?? null) : null;
  const fallbackSample = movementSampleFromEvent(lastEvent);
  const sample = lastSample ?? fallbackSample;
  const deltas = deriveMovementDeltas(now, sample);
  const lastAckOutcome =
    deriveLastAckOutcome(lastEvent) ?? deriveLastAckOutcome(receivedPackets[0]);

  return {
    logEnabled,
    sessionId,
    pendingEvents,
    diagnosticEventCount: pendingEvents,
    movementQueueDepth: deriveMovementQueueDepth(sample),
    lastSample: lastSample,
    sentCommands: sentCommands.slice(-maxRecords),
    receivedPackets: receivedPackets.slice(-maxRecords),
    keyboardEvents: keyboardEvents.slice(-maxKeyboardEvents),
    shellRenderPerf: shellRenderPerf.slice(-maxRecords),
    nextMoveSendAtDelta: deltas.nextMoveSendAtDelta,
    runPrimedUntilDelta: deltas.runPrimedUntilDelta,
    inputBlockedUntilDelta: deltas.inputBlockedUntilDelta,
    lastAckOutcome,
  };
}

function sentCommandsFromDiagnosticEvents(events: Array<Record<string, unknown>>): Array<Record<string, unknown>> {
  const entries: Array<Record<string, unknown>> = [];
  for (const event of events) {
    if (event.type !== "tx:movementCommand") continue;
    const payload = readRecordField(event, "payload");
    const command = payload ? readRecordField(payload, "command") : null;
    if (!command) continue;
    entries.push({
      ...command,
      at: readNumberField(event, "at") ?? readNumberField(command, "at") ?? Date.now(),
    });
  }
  return entries;
}

function receivedPacketsFromDiagnosticEvents(events: Array<Record<string, unknown>>): Array<Record<string, unknown>> {
  const entries: Array<Record<string, unknown>> = [];
  for (const event of events) {
    if (event.type !== "rx:movementPacket") continue;
    const payload = readRecordField(event, "payload");
    if (!payload) continue;
    entries.push({
      packet: readStringField(payload, "packet"),
      payload: payload.payload ?? null,
      at: readNumberField(event, "at") ?? Date.now(),
    });
  }
  return entries;
}

function mergeMovementRecords(
  primary: Array<Record<string, unknown>>,
  secondary: Array<Record<string, unknown>>,
): Array<Record<string, unknown>> {
  const records = [...primary, ...secondary];
  const byKey = new Map<string, Record<string, unknown>>();
  for (const record of records) {
    byKey.set(movementRecordKey(record), record);
  }
  return [...byKey.values()]
    .sort((left, right) => (readNumberField(left, "at") ?? 0) - (readNumberField(right, "at") ?? 0))
    .slice(-50);
}

function movementRecordKey(record: Record<string, unknown>): string {
  const payload = readRecordField(record, "payload");
  return [
    readNumberField(record, "at") ?? "",
    readStringField(record, "type") ?? "",
    readStringField(record, "direction") ?? "",
    readNumberField(record, "movementSeq") ?? "",
    readStringField(record, "packet") ?? "",
    payload ? readNumberField(payload, "x") ?? "" : "",
    payload ? readNumberField(payload, "y") ?? "" : "",
  ].join(":");
}

function movementSampleFromEvent(event: Record<string, unknown> | undefined): Record<string, unknown> | null {
  if (!event || typeof event !== "object") return null;
  const payload = event.payload as Record<string, unknown> | undefined;
  if (!payload || typeof payload !== "object") return null;
  const sample = payload.sample;
  if (sample && typeof sample === "object") return sample as Record<string, unknown>;
  const before = payload.before;
  if (before && typeof before === "object") return before as Record<string, unknown>;
  return payload.queues && typeof payload.queues === "object" ? payload : null;
}

function deriveLastAckOutcome(event: Record<string, unknown> | undefined):
  | "confirmed"
  | "correction"
  | "accepted"
  | null {
  if (!event || typeof event !== "object") return null;
  const type = event.type;
  const payload = (event.payload as Record<string, unknown> | undefined) ?? {};
  const nestedPayload = (payload.payload as Record<string, unknown> | undefined) ?? {};
  if (
    type === "rx:movementPacket" ||
    type === "apply:selfMovementPacket" ||
    readStringField(event, "packet") !== null
  ) {
    const outcome =
      payload.outcome ??
      payload.crystalAckDisposition ??
      nestedPayload.outcome;
    if (outcome === "confirmed" || outcome === "correction" || outcome === "accepted") {
      return outcome;
    }
    const packetName =
      readStringField(payload, "packetName") ??
      readStringField(payload, "packet") ??
      readStringField(nestedPayload, "packetName") ??
      readStringField(nestedPayload, "packet") ??
      readStringField(event, "packetName") ??
      readStringField(event, "packet");
    return movementAckOutcomeFromPacketName(packetName);
  }
  return null;
}

function movementAckOutcomeFromPacketName(packetName: string | null):
  | "confirmed"
  | "correction"
  | "accepted"
  | null {
  if (!packetName) return null;
  if (packetName === "UserDashFail" || packetName === "ObjectDashFail") return "correction";
  if (
    packetName === "UserLocation" ||
    packetName === "Pushed" ||
    packetName === "UserDash" ||
    packetName === "UserDashAttack" ||
    packetName === "UserAttackMove"
  ) {
    return "confirmed";
  }
  if (
    packetName === "ObjectWalk" ||
    packetName === "ObjectRun" ||
    packetName === "ObjectTurn" ||
    packetName === "ObjectPushed" ||
    packetName === "ObjectDash" ||
    packetName === "ObjectDashAttack" ||
    packetName === "ObjectBackStep" ||
    packetName === "ObjectSitDown"
  ) {
    return "accepted";
  }
  return null;
}

function readBoolLikeQueue(o: Record<string, unknown>, key: string): number {
  const v = o[key];
  return v && typeof v === "object" ? 1 : 0;
}

function deriveMovementQueueDepth(sample: Record<string, unknown> | null): number | null {
  const queues = (sample?.queues as Record<string, unknown> | undefined) ?? null;
  if (!queues) return null;
  return (
    readBoolLikeQueue(queues, "movementPlan") +
    readBoolLikeQueue(queues, "pendingSelfMove") +
    readBoolLikeQueue(queues, "queuedMoveIntent") +
    readBoolLikeQueue(queues, "queuedDirectionStep") +
    readBoolLikeQueue(queues, "directionStepPending") +
    (readNumberField(queues, "directionStepPendingQueueLength") ?? 0) +
    (readNumberField(queues, "crystalSelfActionFeedLength") ?? 0) +
    (readNumberField(queues, "outstandingSelfMovementActionsLength") ?? 0)
  );
}

function deriveMovementDeltas(_now: number, sample: Record<string, unknown> | null): {
  nextMoveSendAtDelta: number | null;
  runPrimedUntilDelta: number | null;
  inputBlockedUntilDelta: number | null;
} {
  if (!sample) {
    return {
      nextMoveSendAtDelta: null,
      runPrimedUntilDelta: null,
      inputBlockedUntilDelta: null,
    };
  }
  const queues = (sample.queues as Record<string, unknown> | undefined) ?? null;
  return {
    nextMoveSendAtDelta: queues ? readNumberField(queues as Record<string, unknown>, "nextMoveWaitMs") : null,
    runPrimedUntilDelta: null,
    inputBlockedUntilDelta: queues ? readNumberField(queues as Record<string, unknown>, "movementInputBlockedForMs") : null,
  };
}

export function readBevyLayer(): ProbeBevyLayer {
  const backend = readWindow<string>("__mir2BevyRuntimeBackend");
  const status = readBevyStatus();
  const entityRendererDebug = readWindowObj("__mir2BevyEntityRendererDebug");
  return {
    backend: backend === "webgpu" || backend === "webgl2" ? backend : null,
    lastPhase: status.lastPhase,
    lastMessage: status.lastMessage,
    lastPhaseAtMs: status.lastAtMs,
    // systemMs/counters are intentionally null in P2-3 — the wasm runtime has
    // not yet been extended to publish per-system timing. The JS status ring
    // captures every publish_status phase emitted by lib.rs, so future work
    // only needs to add fields to publish_status and shape them here; no other
    // bridge work is required.
    systemMs: null,
    counters: null,
    entityRenderer: entityRendererDebug
      ? {
          enabled: entityRendererDebug.enabled ?? null,
          domEntityFallback: entityRendererDebug.domEntityFallback ?? null,
          runtimePending: entityRendererDebug.runtimePending ?? null,
          useBevyEntityRenderer: entityRendererDebug.useBevyEntityRenderer ?? null,
          useWebGl2EntityAtlasRenderer: entityRendererDebug.useWebGl2EntityAtlasRenderer ?? null,
          hideDomEntitySpritesForBevy: entityRendererDebug.hideDomEntitySpritesForBevy ?? null,
          atlasKey: entityRendererDebug.atlasKey ?? null,
          atlasReady: entityRendererDebug.atlasReady ?? null,
          textureReady: entityRendererDebug.textureReady ?? null,
          entityCount: entityRendererDebug.entityCount ?? null,
          layerCount: entityRendererDebug.layerCount ?? null,
        }
      : null,
  };
}

export function readStageLayer(): ProbeStageLayer {
  const empty: ProbeStageLayer = {
    screen: null,
    wsState: null,
    mapFileName: null,
    mapTitle: null,
    playerObjectId: null,
    selectedObjectId: null,
    player: null,
    predictedPlayer: null,
    worldSnapshotVersion: null,
    worldSnapshotRealtimeMode: null,
    worldTick: null,
    sceneInteractionReady: null,
    sceneAssetReadiness: null,
    entityCount: null,
    groundDropCount: null,
  };

  try {
    const stage5 = readWindowObj("__mir2Stage5");
    const state = stage5 ? readRecordField(stage5, "state") : null;
    if (!state) return empty;
    const entities = state.entities;
    const groundDrops = state.groundDrops;
    return {
      screen: readStringField(state, "screen"),
      wsState: readStringField(state, "wsState"),
      mapFileName: readStringField(state, "mapFileName"),
      mapTitle: readStringField(state, "mapTitle"),
      playerObjectId: readStringishField(state, "playerObjectId"),
      selectedObjectId: readStringishField(state, "selectedObjectId"),
      player: shapeStagePlayer(readRecordField(state, "player")),
      predictedPlayer: shapePredictedPlayer(readRecordField(state, "predictedPlayer")),
      worldSnapshotVersion: readNumberField(state, "worldSnapshotVersion"),
      worldSnapshotRealtimeMode: readStringField(state, "worldSnapshotRealtimeMode"),
      worldTick: readNumberField(state, "worldTick"),
      sceneInteractionReady: readBooleanField(state, "sceneInteractionReady"),
      sceneAssetReadiness: shapeSceneAssetReadiness(readRecordField(state, "sceneAssetReadiness")),
      entityCount: Array.isArray(entities) ? entities.length : null,
      groundDropCount: Array.isArray(groundDrops) ? groundDrops.length : null,
    };
  } catch {
    return empty;
  }
}

function shapeStagePlayer(player: Record<string, unknown> | null): ProbeStageLayer["player"] {
  if (!player) return null;
  return {
    x: readNumberField(player, "x"),
    y: readNumberField(player, "y"),
    serverX: readNumberField(player, "serverX"),
    serverY: readNumberField(player, "serverY"),
    renderX: readNumberField(player, "renderX"),
    renderY: readNumberField(player, "renderY"),
    objectId: readStringishField(player, "objectId"),
    kind: readStringField(player, "kind"),
    name: readStringField(player, "name"),
    direction: readStringField(player, "direction"),
    dead: readBooleanField(player, "dead"),
    hp: readNumberField(player, "hp"),
    maxHp: readNumberField(player, "maxHp"),
    movementAnimation: readStringField(player, "movementAnimation"),
    movementStartedAt: readNumberField(player, "movementStartedAt"),
    movementUntil: readNumberField(player, "movementUntil"),
  };
}

function shapePredictedPlayer(predicted: Record<string, unknown> | null): ProbeStageLayer["predictedPlayer"] {
  if (!predicted) return null;
  return {
    x: readNumberField(predicted, "x"),
    y: readNumberField(predicted, "y"),
    direction: readStringField(predicted, "direction"),
    mode: readStringField(predicted, "mode"),
    sentAt: readNumberField(predicted, "sentAt"),
    visualUntil: readNumberField(predicted, "visualUntil"),
  };
}

function shapeSceneAssetReadiness(readiness: Record<string, unknown> | null): ProbeStageLayer["sceneAssetReadiness"] {
  if (!readiness) return null;
  const failedUrls = readiness.failedUrls;
  return {
    key: readStringField(readiness, "key"),
    ready: readBooleanField(readiness, "ready"),
    interactionReady: readBooleanField(readiness, "interactionReady"),
    visualReady: readBooleanField(readiness, "visualReady"),
    status: readStringField(readiness, "status"),
    total: readNumberField(readiness, "total"),
    loaded: readNumberField(readiness, "loaded"),
    failed: readNumberField(readiness, "failed"),
    pending: readNumberField(readiness, "pending"),
    durationMs: readNumberField(readiness, "durationMs"),
    failedUrlCount: Array.isArray(failedUrls) ? failedUrls.length : null,
  };
}

export function readResidencyLayer(): ProbeResidencyLayer {
  const residency = readWindowObj("__mir2Residency");
  let stats: Record<string, number> | null = null;
  let resolveStats: Record<string, unknown> | null = null;
  let idbTimings: ProbeResidencyLayer["idbTimings"] = {
    lastGetMs: null,
    lastPutMs: null,
    lastListByAgeMs: null,
  };
  if (residency) {
    try {
      const statsFn = (residency as { stats?: () => Record<string, number> }).stats;
      if (typeof statsFn === "function") {
        const s = statsFn.call(residency);
        if (s && typeof s === "object") {
          stats = s as Record<string, number>;
        }
      }
    } catch {
      /* best-effort */
    }
    try {
      const resolveFn = (residency as { resolveStats?: () => Record<string, unknown> }).resolveStats;
      if (typeof resolveFn === "function") {
        const s = resolveFn.call(residency);
        if (s && typeof s === "object") {
          resolveStats = s as Record<string, unknown>;
        }
      }
    } catch {
      /* best-effort */
    }
    try {
      const idbFn = (residency as { idbTimings?: () => unknown }).idbTimings;
      if (typeof idbFn === "function") {
        const r = idbFn.call(residency);
        if (r && typeof r === "object") {
          const obj = r as { lastGetMs?: unknown; lastPutMs?: unknown; lastListByAgeMs?: unknown };
          idbTimings = {
            lastGetMs: typeof obj.lastGetMs === "number" ? obj.lastGetMs : null,
            lastPutMs: typeof obj.lastPutMs === "number" ? obj.lastPutMs : null,
            lastListByAgeMs: typeof obj.lastListByAgeMs === "number" ? obj.lastListByAgeMs : null,
          };
        }
      }
    } catch {
      /* best-effort */
    }
  }

  const cacheMetrics = readWindowObj("__mir2CacheMetrics");
  const cacheSummary = cacheMetrics
    ? (() => {
        try {
          const fn = (cacheMetrics as { snapshot?: () => Record<string, unknown> }).snapshot;
          return typeof fn === "function" ? compactCacheMetricsSnapshot(fn.call(cacheMetrics)) : null;
        } catch {
          return null;
        }
      })()
    : null;

  return {
    stats,
    resolveStats,
    cacheSummary,
    idbTimings,
  };
}

function compactCacheMetricsSnapshot(snapshot: Record<string, unknown> | null): Record<string, unknown> | null {
  if (!snapshot || typeof snapshot !== "object") return null;
  const summary = readRecordField(snapshot, "summary");
  const resources = snapshot.resources;
  const apiRequests = snapshot.apiRequests;
  const milestones = snapshot.milestones;
  const prewarmRuns = snapshot.prewarmRuns;
  const cacheStorage = readRecordField(snapshot, "cacheStorage");
  return {
    startedAt: readNumberField(snapshot, "startedAt"),
    debug: readBooleanField(snapshot, "debug"),
    summary: summary ? compactCacheSummaryRecord(summary) : null,
    resourceCount: Array.isArray(resources) ? resources.length : null,
    apiRequestCount: Array.isArray(apiRequests) ? apiRequests.length : null,
    milestoneCount: Array.isArray(milestones) ? milestones.length : null,
    prewarmRuns: Array.isArray(prewarmRuns)
      ? prewarmRuns.slice(-8).map((run) => compactPrewarmRun(recordFromUnknown(run)))
      : [],
    cacheStorage: cacheStorage
      ? {
          supported: readBooleanField(cacheStorage, "supported"),
          updatedAtMs: readNumberField(cacheStorage, "updatedAtMs"),
          cacheCount: readNumberField(cacheStorage, "cacheCount"),
          entryCount: readNumberField(cacheStorage, "entryCount"),
          usageBytes: readNumberField(cacheStorage, "usageBytes"),
          quotaBytes: readNumberField(cacheStorage, "quotaBytes"),
          persisted: readBooleanField(cacheStorage, "persisted"),
          persistGranted: readBooleanField(cacheStorage, "persistGranted"),
        }
      : null,
  };
}

function compactCacheSummaryRecord(summary: Record<string, unknown>): Record<string, unknown> {
  const slowest = summary.slowest;
  return {
    ...summary,
    slowest: Array.isArray(slowest)
      ? slowest.slice(0, 5).map((entry) => {
          const record = recordFromUnknown(entry);
          return record
            ? {
                path: readStringField(record, "path"),
                kind: readStringField(record, "kind"),
                durationMs: readNumberField(record, "durationMs"),
                transferSize: readNumberField(record, "transferSize"),
              }
            : null;
        }).filter(Boolean)
      : [],
  };
}

function compactPrewarmRun(run: Record<string, unknown> | null): Record<string, unknown> | null {
  if (!run) return null;
  const failedUrls = run.failedUrls;
  return {
    name: readStringField(run, "name"),
    status: readStringField(run, "status"),
    requested: readNumberField(run, "requested"),
    ok: readNumberField(run, "ok"),
    failed: readNumberField(run, "failed"),
    failedUrlCount: Array.isArray(failedUrls) ? failedUrls.length : null,
    durationMs: readNumberField(run, "durationMs"),
    sceneCacheHits: readNumberField(run, "sceneCacheHits"),
    sceneCacheMisses: readNumberField(run, "sceneCacheMisses"),
  };
}

export function readNetworkLayer(now: number): ProbeNetworkLayer {
  const gatewayEventHistory = readArr("__mir2GatewayEventHistory");
  const commandHistory = readArr("__mir2CommandHistory");
  void now;
  const counts = readWindowObj("__mir2Debug")
    ? (() => {
        try {
          const fn = (readWindowObj("__mir2Debug") as { counts?: () => Record<string, number> } | null)
            ?.counts;
          return typeof fn === "function" ? fn.call(null) : null;
        } catch {
          return null;
        }
      })()
    : null;
  const lastCmd = readWindow<Record<string, unknown>>("__mir2LastCommand");
  const wsReadyState = lastCmd && typeof (lastCmd as { ws?: { readyState?: number } }).ws === "object"
    ? null
    : null;
  void wsReadyState;
  const lastTxTs = pickLastMovementCommandTs(commandHistory);
  return {
    gatewayEventHistory: gatewayEventHistory.slice(-50),
    commandHistory: commandHistory.slice(-50),
    counts: counts ?? {},
    wsReadyState: null,
    msSinceLastTxMovement: lastTxTs !== null ? Math.max(0, now - lastTxTs) : null,
  };
}

function pickLastMovementCommandTs(history: Array<Record<string, unknown>>): number | null {
  for (let i = history.length - 1; i >= 0; i -= 1) {
    const e = history[i];
    if (e && e.type && typeof e.type === "string" && e.type.toLowerCase().includes("move")) {
      const at = e.at;
      if (typeof at === "number" && at > 0) return at;
      const t = e.t;
      if (typeof t === "number" && t > 0) return t;
    }
  }
  return null;
}

export function readGatewayLayer(): ProbeGatewayLayer {
  return readGatewayLayerFromPoller();
}

export function startGatewayMetricsPoller(): void {
  startGatewayProbe();
}

export function stopGatewayMetricsPoller(): void {
  stopGatewayProbe();
}

export function isGatewayMetricsPollerRunning(): boolean {
  return isGatewayProbeRunning();
}

export function readFrameLayer(): ProbeFrameLayer {
  const r = readFrameProbe();
  return {
    rafDeltaAvg: r.rafDeltaAvg,
    rafDeltaMax: r.rafDeltaMax,
    fpsInLastSecond: r.fpsInLastSecond,
    timerDriftAvg: r.timerDriftAvg,
    timerDriftMax: r.timerDriftMax,
    longtasksInLastSecond: r.longtasksInLastSecond,
    longestLongtaskMs: r.longestLongtaskMs,
    longtaskCountTotal: r.longtaskCountTotal,
    rafDeltaSpikeCountTotal: r.rafDeltaSpikeCountTotal,
    timerDriftSpikeCountTotal: r.timerDriftSpikeCountTotal,
    lastLongtaskAtMs: r.lastLongtaskAtMs,
    lastRafDeltaSpikeAtMs: r.lastRafDeltaSpikeAtMs,
    lastTimerDriftSpikeAtMs: r.lastTimerDriftSpikeAtMs,
    recentLongtasks: r.recentLongtasks,
    recentResources: r.recentResources,
    longtaskBuckets: r.longtaskBuckets,
  };
}
