/**
 * mir2-probe — cross-layer game probe system.
 *
 * Schema for the unified aggregated sample. Each sub-record maps to one of the
 * probe data layers (movement / stage / bevy / residency / network / gateway /
 * frame). Fields that are not yet wired are typed as `T | null` and slide from
 * `null` to `T` as later phases add hooks; producers must never throw on a
 * missing window global, they must return `null`.
 *
 * Lifecycle:
 *   P1 (this file): schema + bus + frame probe + SDK shell. Most layers are
 *     read-only adapters over existing window hooks; only L6 (frame timing) is a
 *     genuinely new passive observer.
 *   P2: Bevy publish_status extension, gateway /metrics, dev overlay UI.
 *
 * Compatibility: the type intentionally mirrors names already exported by
 * `apps/web/app/page.tsx` (`MovementDiagnosticSample`), `apps/web/lib/debug-snapshot.ts`
 * (`DebugEvent`), and `apps/web/lib/asset-residency/types.ts` (`AssetResidencyStats`)
 * so producers are pure shapers, not new sources of truth.
 */

export type ProbeSchemaVersion = "mir2-probe/1";

export type MovementAckOutcome = "confirmed" | "correction" | "accepted";

export type ProbeMovementLayer = {
  /** True if `?movementLog=1` is active (existing __mir2MovementLogEnabled). */
  logEnabled: boolean;
  /** sessionId from __mir2MovementDiagnostics, or null if not active. */
  sessionId: string | null;
  /** Diagnostic event buffer depth out of 1200; not movement ack backlog. */
  pendingEvents: number | null;
  /** Alias for pendingEvents with clearer semantics for new overlay rows. */
  diagnosticEventCount: number | null;
  /** Current client-side movement queue/action depth derived from MovementDiagnosticSample. */
  movementQueueDepth: number | null;
  /** Last captured sample, or null when movement diagnostics inactive. */
  lastSample: unknown | null;
  /** Last movement ring buffer slice (most recent 50), for packet-in/out correlation. */
  sentCommands: Array<Record<string, unknown>>;
  receivedPackets: Array<Record<string, unknown>>;
  keyboardEvents: Array<Record<string, unknown>>;
  shellRenderPerf: Array<Record<string, unknown>>;
  /** Derived deltas (in ms) versus "now"; positive = ahead, negative = overdue. */
  nextMoveSendAtDelta: number | null;
  runPrimedUntilDelta: number | null;
  inputBlockedUntilDelta: number | null;
  /** Set when a correction was observed in the last received packet (heuristic). */
  lastAckOutcome: MovementAckOutcome | null;
};

export type ProbeBevyLayer = {
  /** Runtime backend reported by __mir2BevyRuntimeBackend. */
  backend: "webgpu" | "webgl2" | null;
  /** Last `{phase, message}` emitted by publish_status from wasm. */
  lastPhase: string | null;
  lastMessage: string | null;
  lastPhaseAtMs: number | null;
  /** P2 will populate these from the wasm-side counters/timings extension. */
  systemMs: Record<string, number> | null;
  counters: Record<string, number> | null;
  entityRenderer: Record<string, unknown> | null;
};

export type ProbeStagePlayer = {
  /** Render-facing x/y, kept for compatibility with older stage diagnostics. */
  x: number | null;
  y: number | null;
  /** Authoritative self entity tile from the latest world snapshot. */
  serverX: number | null;
  serverY: number | null;
  /** Local render/prediction tile after crystal render-position preservation. */
  renderX: number | null;
  renderY: number | null;
  objectId: string | null;
  kind: string | null;
  name: string | null;
  direction: string | null;
  dead: boolean | null;
  hp: number | null;
  maxHp: number | null;
  movementAnimation: string | null;
  movementStartedAt: number | null;
  movementUntil: number | null;
};

export type ProbeStagePredictedPlayer = {
  x: number | null;
  y: number | null;
  direction: string | null;
  mode: string | null;
  sentAt: number | null;
  visualUntil: number | null;
};

export type ProbeSceneAssetReadiness = {
  key: string | null;
  ready: boolean | null;
  interactionReady: boolean | null;
  visualReady: boolean | null;
  status: string | null;
  total: number | null;
  loaded: number | null;
  failed: number | null;
  pending: number | null;
  durationMs: number | null;
  failedUrlCount: number | null;
};

export type ProbeStageLayer = {
  /** Client screen and socket state, used to prove StartGame reached the game scene. */
  screen: string | null;
  wsState: string | null;
  mapFileName: string | null;
  mapTitle: string | null;
  playerObjectId: string | null;
  selectedObjectId: string | null;
  player: ProbeStagePlayer | null;
  predictedPlayer: ProbeStagePredictedPlayer | null;
  worldSnapshotVersion: number | null;
  worldSnapshotRealtimeMode: string | null;
  worldTick: number | null;
  sceneInteractionReady: boolean | null;
  sceneAssetReadiness: ProbeSceneAssetReadiness | null;
  entityCount: number | null;
  groundDropCount: number | null;
};

export type ProbeResidencyLayer = {
  /** Latest stats() snapshot from bevyAtlasResidency, or null when not initialized. */
  stats: Record<string, number> | null;
  /** Entity atlas build stats resolved on the shell side. */
  resolveStats: Record<string, unknown> | null;
  /** CacheMetrics.summary if available. */
  cacheSummary: Record<string, unknown> | null;
  /** P2 will add real IDB timing in ms. */
  idbTimings: { lastGetMs: number | null; lastPutMs: number | null; lastListByAgeMs: number | null };
};

export type ProbeNetworkLayer = {
  /** Ring of last 50 entries from __mir2GatewayEventHistory (raw decoded). */
  gatewayEventHistory: Array<Record<string, unknown>>;
  /** Ring of last 50 from __mir2CommandHistory. */
  commandHistory: Array<Record<string, unknown>>;
  /** Per-kind counts from debug-snapshot ring (counts[kind:cat]). */
  counts: Record<string, number>;
  /** WebSocket readyState if available. */
  wsReadyState: number | null;
  /** Inferred time (in ms) since last tx movement command; null when never sent. */
  msSinceLastTxMovement: number | null;
};

export type ProbeGatewayLayer = {
  /** Result of GET /health if reached (Rust-side), null on dev without gateway. */
  health: Record<string, unknown> | null;
  /** P2 will populate from GET /metrics. */
  metrics: Record<string, unknown> | null;
  /** Wall time (ms) of the last successful /health fetch. */
  healthFetchedAtMs: number | null;
  healthFetchMs: number | null;
};

export type ProbeFrameLayer = {
  /** rAF frame delta avg over last ~30 frames. */
  rafDeltaAvg: number | null;
  /** Worst rAF frame delta over the last ~30 frames. */
  rafDeltaMax: number | null;
  /** Frame rate estimated from rAF deltas in last ~1s. */
  fpsInLastSecond: number | null;
  /** setInterval drift avg over the last ~30 timer samples. */
  timerDriftAvg: number | null;
  /** Worst setInterval drift over the last ~30 timer samples. */
  timerDriftMax: number | null;
  /** Count of LongTask entries (>50ms) in the last 1s. */
  longtasksInLastSecond: number | null;
  /** Longest single longtask duration in the last 1s. */
  longestLongtaskMs: number | null;
  /** Monotonic count of LongTask entries observed since frame probe start. */
  longtaskCountTotal: number;
  /** Monotonic count of rAF deltas >= 120ms since frame probe start. */
  rafDeltaSpikeCountTotal: number;
  /** Monotonic count of 100ms timer drifts >= 120ms since frame probe start. */
  timerDriftSpikeCountTotal: number;
  /** Performance.now() timestamp of the latest LongTask, if any. */
  lastLongtaskAtMs: number | null;
  /** Performance.now() timestamp of the latest rAF spike, if any. */
  lastRafDeltaSpikeAtMs: number | null;
  /** Performance.now() timestamp of the latest timer drift spike, if any. */
  lastTimerDriftSpikeAtMs: number | null;
  /** Recent LongTask entries, used to correlate action-local stalls. */
  recentLongtasks: Array<{
    atMs: number;
    duration: number;
    name: string | null;
    attribution: Array<{
      name: string | null;
      entryType: string | null;
      containerType: string | null;
      containerName: string | null;
      containerSrc: string | null;
      containerId: string | null;
      scriptUrl: string | null;
    }>;
  }>;
  /** Recent resource completions near the current sample. */
  recentResources: Array<{
    atMs: number;
    startTime: number;
    responseEnd: number;
    duration: number;
    path: string;
    initiatorType: string | null;
    transferSize: number | null;
    encodedBodySize: number | null;
    decodedBodySize: number | null;
    deliveryType: string | null;
  }>;
  /** Histogram buckets: <50 / 50-100 / 100-200 / 200+. */
  longtaskBuckets: { lt50: number; b50to100: number; b100to200: number; ge200: number } | null;
};

export type ProbeSample = {
  schema: ProbeSchemaVersion;
  /** Performance.now() clock — used to derive deltas; monotonic per page load. */
  t: number;
  /** Date.now() epoch ms — for cross-session correlation. */
  epoch: number;
  /** Probe session id — stable across samples within one page session. */
  sessionId: string;
  /** Unchanging per-sample label set by the runner (e.g. "shift-run-bug"). */
  label: string;
  movement: ProbeMovementLayer;
  stage: ProbeStageLayer;
  bevy: ProbeBevyLayer;
  residency: ProbeResidencyLayer;
  network: ProbeNetworkLayer;
  gateway: ProbeGatewayLayer;
  frame: ProbeFrameLayer;
};

export type ProbeSnapshotProfile = "full" | "overlay" | "movement";

export type ProbeSnapshotOptions = {
  /** Optional label stamped into the sample (correlation with a repro run). */
  label?: string;
  /**
   * Sampling profile. `movement` intentionally skips heavy cache/network
   * detail so high-frequency movement timelines cannot perturb gameplay.
   */
  profile?: ProbeSnapshotProfile;
};

/** Shape of the SDK exposed at window.__mir2Probe; P1 only requires snapshot(). */
export type ProbeHandle = {
  schema: ProbeSchemaVersion;
  sessionId: string;
  /** Capture a point-in-time cross-layer sample. Always returns; never throws. */
  snapshot: (options?: ProbeSnapshotOptions) => ProbeSample;
  /** Begin passive frame observations (longtask + rAF). Idempotent. */
  start: () => void;
  /** Stop passive frame observations. Idempotent. */
  stop: () => void;
  /** True when frame probe is currently observing. */
  running: () => boolean;
  /** Session label set at install time. */
  label: () => string;
};
