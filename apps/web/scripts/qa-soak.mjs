// qa-soak.mjs — AI-driven LONG-RUNNING "play all day" stability soak.
//
// The other QA loops (qa-playthrough / qa-combat-survival / qa-social) each run a
// short journey and exit. This harness answers the one thing they cannot: does a
// SUSTAINED session stay healthy, or does it slowly leak / degrade / desync? It
// drives the REAL web client over Chrome DevTools Protocol, keeps it CONTINUOUSLY
// busy with real player activity, and samples a TIME-SERIES of memory / GPU / DOM /
// FPS / WebSocket / error health every ~10–15s for the whole run (minutes → hours).
// At the end it fits trends over the windows and emits a PASS / LEAK / DEGRADED
// verdict plus a time-series JSON and a report.md (per-window numbers + sparklines).
//
// Why a browser (not a protocol bot): leaks live in the real client — the Bevy
// entity atlas (GPU pixel bytes), the alpha-keyed scene-asset blobs, the React/DOM
// node graph, the cache-metrics transfer ledger. Only a real headed Chrome with a
// real GPU surfaces runtime/GPU memory growth and true frame cadence.
//
// ACTIVE soak (not idle): an idle client never allocates, so a leak that only grows
// under play would never show. On a fast cycle the harness alternates real activity —
//   (a) movement via BOTH click-to-move AND a sustained HELD ARROW KEY
//       (CDP Input.dispatchKeyEvent — also the only loop that exercises the keyboard
//        SEND pipeline the click-driven loops miss),
//   (b) combat (find a kind:"monster", walk adjacent, swing),
//   (c) cross-map travel (transferTo across a small ring of maps),
//   (d) open/close the inventory HUD (churns a window mount/unmount — a DOM-leak probe).
//
// MONITORS (sampled into the time-series):
//   • JS heap        — performance.memory.usedJSHeapSize / totalJSHeapSize (GC-forced
//                      each sample via HeapProfiler.collectGarbage, so the series is
//                      RETAINED heap — a monotonic climb is a real leak, not garbage).
//   • Runtime/GPU    — window.__mir2BevyEntityRendererDebug.atlasPixelBytes / atlasCount
//                      + sceneAssetRuntime.alphaKeyedBlobBytes, and
//                      window.__mir2CacheMetrics.snapshot().summary
//                      (transferBytes / cacheStorageEntryCount / domImageCount).
//   • DOM            — document.querySelectorAll("*").length + document.images.length.
//   • FPS / cadence  — an in-page rAF recorder (the rafGaps approach from
//                      qa-load-stress.mjs) drained each sample → median fps + p95
//                      frame-time + worst no-frame gap (the FREEZE signal).
//   • WS health      — gateway reconnect count (Network.webSocketCreated), frames-
//                      received rate, wsState.
//   • Errors         — console errors / exceptions (Runtime.consoleAPICalled /
//                      exceptionThrown) + network failures (≥400 / net::ERR_*),
//                      accumulated as monotonic counters and per-window deltas.
//
// DETECTORS / VERDICT:
//   • LEAK            — retained JS heap OR bevy atlasPixelBytes OR DOM node count
//                       trends monotonically up (least-squares slope over the windows)
//                       beyond a threshold GC does not reclaim.
//   • FPS DEGRADED    — last-window median fps materially below the first window.
//   • ERROR ACCUM     — console/network error rate growing across the run.
//   • RECONNECT STORM — repeated gateway reconnects.
//   • FREEZE          — a long no-frame gap (rAF starved) or a WS frame stall.
//   Verdict = LEAK (any leak detector) → else DEGRADED (any other detector) → else PASS.
//   Exit code is 1 on LEAK / FREEZE / RECONNECT-STORM (hard stability failures), else 0.
//
// ENV — the shared :7110 node-proxy sim DEPLETES over long runs (project memory), so a
// soak pointed at it measures shared-sim exhaustion, not the client. PREFER an ISOLATED
// gateway: spin one up from a prebuilt release binary on fresh ports with a temp account
// store, then point the client at it via the localhost-only `?gatewayWs=` override
// (page.tsx) — e.g.
//   MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7311 MIR2_ACCOUNT_STORE=$(mktemp -d)/acct.json \
//     cargo +1.89.0 run -p mir2-gateway --bin mir2-gateway      # isolated sim+gateway
//   node ./scripts/qa-soak.mjs --headed --baseUrl http://127.0.0.1:3001 \
//     --gatewayWs ws://127.0.0.1:7311/ws --durationMin 120
// The report records which gateway the client actually connected to and flags a run
// against the shared :7110 stack (so depletion is not mistaken for a client leak).
//
// This file deliberately reuses the patterns proven in qa-combat-survival.mjs /
// qa-playthrough.mjs / qa-load-stress.mjs (CDP client, headed chrome launch, login
// flow, tile clicking, transferTo, held-arrow-key driver, rAF recorder) so it stays
// consistent with the house QA harnesses, with zero new dependencies.
//
// Usage:
//   node ./scripts/qa-soak.mjs --headed [--durationMin 20] [--baseUrl http://127.0.0.1:3001]
//        [--gatewayWs ws://127.0.0.1:7311/ws] [--account NAME --password PW]
//        [--sampleMs 12000] [--activityIntervalMs 2500] [--runId my-run]

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));

const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3001";
// The `gatewayWs` query override is honored only on a localhost origin (page.tsx) —
// when set it points the client at an isolated gateway instead of the page default.
const gatewayWs = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? null;
const RUN_URL = buildRunUrl(baseUrl, gatewayWs);

const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultName("ACC");
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultName("SOAK").slice(0, 10);
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9200 + (process.pid % 300));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(args.output ?? path.join(process.cwd(), "docs", "generated", "soak-qa", `run-${runId}`));
const framesDir = path.join(outputDir, "frames");

// Soak cadence.
const durationMin = numberArg(args.durationMin ?? process.env.MIR2_SOAK_DURATION_MIN, 20);
const durationMs = Math.max(1, durationMin) * 60_000;
const sampleMs = numberArg(args.sampleMs ?? process.env.MIR2_SOAK_SAMPLE_MS, 12_000); // ~10–15s time-series cadence
const activityIntervalMs = numberArg(args.activityIntervalMs ?? process.env.MIR2_SOAK_ACTIVITY_MS, 2_500);
const gcEachSample = booleanArg(args.gcEachSample, true); // GC before each heap read → RETAINED-heap series
const heldKeyMs = numberArg(args.heldKeyMs, 850); // how long a held arrow key is sustained per activity tick

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);

// A small ring of maps the travel activity cycles through. mapFileName equals the
// transfer key map token ("0"/"1"/"2") — see QUICK_TRANSFER_OPTIONS (page.tsx). Town
// "0" plus the two on-demand-monster fields so travel churns distinct tile/entity sets.
const TRAVEL_RING = [
  { map: "0", x: 330, y: 270, label: "Bichon Province (0)" },
  { map: "1", x: 315, y: 82, label: "Woomyon Woods (1)" },
  { map: "2", x: 503, y: 483, label: "Serpent Valley (2)" },
];

// ---- Detector thresholds (tunable). A healthy stack should land all of these well
// inside their bounds → PASS. Kept conservative so warm-up noise doesn't trip them. ----
const WARMUP_SAMPLES = numberArg(args.warmupSamples, 2); // floor: skip at least N samples from trend fits
// LEAK slopes fit the STEADY STATE, past warm-up. One-time costs (loading each map's
// assets/entities into the heap+atlas the first time the travel ring visits it) are
// bounded and plateau, so they must not read as a leak — discard the first fraction of
// the run and fit the remainder (default: the second half), where a real (unbounded)
// leak is STILL climbing. This large warm-up applies ONLY to the leak-slope fits; the
// FPS / reconnect / freeze detectors deliberately span the whole run (minus a tiny
// settle) so a regression in the first half is never skipped.
const WARMUP_FRACTION = clampNumber(numberArg(args.warmupFraction, 0.5), 0, 0.8);
const LEAK_HEAP_SLOPE_MB_PER_MIN = numberArg(args.leakHeapSlope, 2.0);
const LEAK_HEAP_MIN_GROWTH_MB = numberArg(args.leakHeapGrowth, 15);
const LEAK_ATLAS_SLOPE_MB_PER_MIN = numberArg(args.leakAtlasSlope, 1.0);
const LEAK_ATLAS_MIN_GROWTH_MB = numberArg(args.leakAtlasGrowth, 8);
const LEAK_DOM_SLOPE_PER_MIN = numberArg(args.leakDomSlope, 40);
const LEAK_DOM_MIN_GROWTH = numberArg(args.leakDomGrowth, 400);
const LEAK_MIN_R2 = 0.30; // trend must be reasonably linear (not a single spike)
const FPS_DEGRADE_RATIO = numberArg(args.fpsDegradeRatio, 0.7); // last < first*ratio = degraded
const FPS_MIN_HEALTHY = numberArg(args.fpsMinHealthy, 20); // only judge degradation if it started healthy
const ERROR_GROWTH_RATIO = numberArg(args.errorGrowthRatio, 1.5);
const ERROR_MIN_TOTAL = numberArg(args.errorMinTotal, 6);
const RECONNECT_MAX = numberArg(args.reconnectMax, 2);
const FREEZE_GAP_MS = numberArg(args.freezeGapMs, 3_000);

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (adapted from qa-combat-survival.mjs). Built for a LONG run: every
// captured array is capped, but each event also bumps a MONOTONIC counter so the
// time-series reads rates/totals from the counters, never from the rotating buffers.
// ---------------------------------------------------------------------------
class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
    this.consoleErrorCount = 0; // monotonic
    this.networkFailures = [];
    this.networkFailureCount = 0; // monotonic
    this.requestUrlById = new Map();
    this.webSocketFramesReceived = [];
    this.webSocketFramesSent = [];
    this.wsRxCount = 0; // monotonic frames received
    this.wsTxCount = 0; // monotonic frames sent
    this.gatewayUrls = [];
    this.gatewaySocketCount = 0; // monotonic gateway (/ws) socket opens → reconnects = count-1
    this.crashed = null;
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (event) => this.handleMessage(event.data));
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }

  handleMessage(raw) {
    const message = JSON.parse(raw);
    if (message.id && this.pending.has(message.id)) {
      const { resolve, reject } = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) reject(new Error(`${message.error.message}: ${message.error.data ?? ""}`));
      else resolve(message.result ?? {});
      return;
    }

    const m = message.method;
    const p = message.params ?? {};

    if (m === "Runtime.consoleAPICalled") {
      if (p.type === "error" || p.type === "warning") {
        this.consoleErrorCount += 1;
        this.consoleErrors.push({
          source: "console",
          type: p.type,
          text: (p.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
          at: Date.now(),
        });
        this.consoleErrors = this.consoleErrors.slice(-500);
      }
    } else if (m === "Runtime.exceptionThrown") {
      const d = p.exceptionDetails;
      this.consoleErrorCount += 1;
      this.consoleErrors.push({
        source: "exception",
        type: "error",
        text: d?.exception?.description ?? d?.text ?? "runtime exception",
        at: Date.now(),
      });
      this.consoleErrors = this.consoleErrors.slice(-500);
    } else if (m === "Log.entryAdded") {
      const entry = p.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        this.consoleErrorCount += 1;
        this.consoleErrors.push({
          source: entry.source ?? "log",
          type: "error",
          text: `${entry.text ?? ""}${entry.url ? ` (${entry.url})` : ""}`,
          at: Date.now(),
        });
        this.consoleErrors = this.consoleErrors.slice(-500);
      }
    } else if (m === "Network.requestWillBeSent") {
      if (p.requestId && p.request?.url) this.requestUrlById.set(p.requestId, p.request.url);
    } else if (m === "Network.responseReceived") {
      const r = p.response;
      const url = String(r?.url ?? "");
      if (r && r.status >= 400 && !url.includes("favicon")) {
        this.networkFailureCount += 1;
        this.networkFailures.push({ url, status: r.status, kind: classifyAssetUrl(url), at: Date.now() });
        this.networkFailures = this.networkFailures.slice(-500);
      }
    } else if (m === "Network.loadingFailed") {
      const url = this.requestUrlById.get(p.requestId) ?? "(unknown)";
      if (!String(url).includes("favicon")) {
        this.networkFailureCount += 1;
        this.networkFailures.push({ url, status: 0, errorText: p.errorText ?? "", kind: classifyAssetUrl(url), at: Date.now() });
        this.networkFailures = this.networkFailures.slice(-500);
      }
    } else if (m === "Network.webSocketCreated") {
      // The gateway socket the client opened. The FIRST is the initial connect; each
      // subsequent /ws socket open is a reconnect (the storm signal).
      if (p.url) {
        this.gatewayUrls.push({ url: String(p.url), at: Date.now() });
        if (/\/ws(\?|$)/.test(String(p.url))) this.gatewaySocketCount += 1;
      }
    } else if (m === "Network.webSocketFrameReceived") {
      this.wsRxCount += 1;
      this.webSocketFramesReceived.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-200);
    } else if (m === "Network.webSocketFrameSent") {
      this.wsTxCount += 1;
      this.webSocketFramesSent.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-200);
    } else if (m === "Inspector.targetCrashed") {
      this.crashed = { at: Date.now() };
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (result.exceptionDetails) {
      throw new Error(`Evaluation failed: ${result.exceptionDetails.text ?? JSON.stringify(result.exceptionDetails)}`);
    }
    return result.result?.value;
  }

  // Best gateway URL guess: the first ws:// socket on the mir2 gateway path ("/ws").
  gatewayUrl() {
    const hit = this.gatewayUrls.find((g) => /\/ws(\?|$)/.test(g.url));
    return hit?.url ?? this.gatewayUrls[0]?.url ?? null;
  }

  close() {
    this.ws?.close();
  }
}

// ---------------------------------------------------------------------------
// Run context.
// ---------------------------------------------------------------------------
function createContext(client) {
  const issues = [];
  return {
    client,
    issues,
    samples: [],
    activityLog: [],
    activityCounts: {},
    running: false,
    startedAt: 0,
    lastSampleAt: 0,
    prev: null,
    enteredWorld: false,
    log: (msg) => console.log(msg),
    addIssue(issue) {
      const id = `${issue.category}-${String(issues.length + 1).padStart(3, "0")}`;
      const full = { id, severity: "medium", beat: null, ...issue };
      issues.push(full);
      console.log(`  ⚠ [${full.severity}/${full.category}] ${full.summary}`);
      return full;
    },
  };
}

function snapshotCounters(client) {
  return {
    wsRx: client.wsRxCount,
    wsTx: client.wsTxCount,
    wsCreated: client.gatewaySocketCount,
    consoleErrors: client.consoleErrorCount,
    networkFailures: client.networkFailureCount,
  };
}

// ---------------------------------------------------------------------------
// In-page FPS recorder — a continuous rAF loop (the rafGaps approach from
// qa-load-stress.mjs). It records each frame delta into a ring buffer; the Node
// side drains the deltas every sample and computes median fps + p95 frame-time +
// the worst no-frame gap (a long gap = rAF starved = a visible freeze). Lives on
// `window` so it survives WS reconnects / React remounts.
// ---------------------------------------------------------------------------
function installFpsProbeSrc() {
  return `
(function () {
  var W = window;
  if (W.__mir2SoakFps && W.__mir2SoakFps.running) return W.__mir2SoakFps.status();
  var F = {
    running: false, _raf: 0, _last: 0, gaps: [], cap: 8000, lastDrainAt: 0, totalFrames: 0,
    start: function () {
      if (F.running) return; F.running = true; F._last = performance.now(); F.lastDrainAt = performance.now();
      var step = function () {
        var t = performance.now(), dt = t - F._last; F._last = t;
        F.totalFrames++;
        if (F.gaps.length >= F.cap) F.gaps.shift();
        F.gaps.push(dt);
        F._raf = requestAnimationFrame(step);
      };
      F._raf = requestAnimationFrame(step);
    },
    // Frame deltas since the last drain + the wall window, then reset.
    drain: function () {
      var now = performance.now();
      var arr = F.gaps; F.gaps = [];
      var windowMs = now - F.lastDrainAt; F.lastDrainAt = now;
      var maxGap = 0; for (var i = 0; i < arr.length; i++) { if (arr[i] > maxGap) maxGap = arr[i]; }
      // A wall window much longer than the painted frames implies a tail gap where rAF
      // never fired (the tab was starved at drain time); fold that into maxGap.
      var painted = 0; for (var j = 0; j < arr.length; j++) painted += arr[j];
      var tailGap = windowMs - painted;
      if (tailGap > maxGap) maxGap = tailGap;
      return { dts: arr, windowMs: Math.round(windowMs), frames: arr.length, maxGapMs: Math.round(maxGap), totalFrames: F.totalFrames };
    },
    status: function () { return { running: F.running, buffered: F.gaps.length, totalFrames: F.totalFrames }; },
  };
  W.__mir2SoakFps = F; F.start(); return F.status();
})();
`;
}

async function drainFps(client) {
  const r = await client
    .evaluate(`(window.__mir2SoakFps && window.__mir2SoakFps.drain) ? window.__mir2SoakFps.drain() : null`)
    .catch(() => null);
  if (!r) {
    // Probe missing (first sample / lost) — (re)install and report an empty window.
    await client.evaluate(installFpsProbeSrc()).catch(() => {});
    return { dts: [], frames: 0, maxGapMs: 0, windowMs: 0, totalFrames: 0 };
  }
  return r;
}

// ---------------------------------------------------------------------------
// One time-series sample — reads heap + runtime/GPU + DOM + WS context in a single
// in-page evaluate (minimize round-trips), then merges the FPS drain + the Node-side
// monotonic counters.
// ---------------------------------------------------------------------------
async function readSampleState(client) {
  return client.evaluate(`
    (() => {
      var W = window;
      var s = (W.__mir2Stage5 && W.__mir2Stage5.state) || {};
      var ents = Array.isArray(s.entities) ? s.entities : [];
      var mem = (performance && performance.memory) || {};
      var er = W.__mir2BevyEntityRendererDebug || null;
      var sar = (er && er.sceneAssetRuntime) || null;
      var cache = null;
      try { cache = (W.__mir2CacheMetrics && W.__mir2CacheMetrics.snapshot) ? W.__mir2CacheMetrics.snapshot().summary : null; } catch (e) { cache = null; }
      var num = function (v) { return (typeof v === "number" && isFinite(v)) ? v : null; };
      var pick = function (a, b) { return a != null ? a : (b != null ? b : null); };
      return {
        jsHeapUsed: num(mem.usedJSHeapSize), jsHeapTotal: num(mem.totalJSHeapSize), jsHeapLimit: num(mem.jsHeapSizeLimit),
        atlasPixelBytes: er ? num(er.atlasPixelBytes) : null,
        atlasCount: er ? num(er.atlasCount) : null,
        alphaKeyedBlobBytes: pick(sar ? num(sar.alphaKeyedBlobBytes) : null, cache ? num(cache.alphaKeyedBlobBytes) : null),
        entityRendererEntities: er ? num(er.entityCount) : null,
        entityRendererLayers: er ? num(er.layerCount) : null,
        transferBytes: cache ? num(cache.transferBytes) : null,
        cacheStorageEntryCount: cache ? num(cache.cacheStorageEntryCount) : null,
        cacheDomImageCount: cache ? num(cache.domImageCount) : null,
        domNodes: document.querySelectorAll("*").length,
        domImages: document.images.length,
        damageFloaters: document.querySelectorAll(".scene-damage-floater").length,
        inventoryOpen: Boolean(document.querySelector(".inventory-window")),
        screen: s.screen || null, wsState: s.wsState || null, mapFileName: s.mapFileName || null,
        player: s.player ? { x: s.player.x, y: s.player.y } : null,
        entityCount: ents.length,
        monsterCount: ents.filter(function (e) { return e && e.kind === "monster" && !e.dead; }).length,
      };
    })()
  `);
}

async function takeSample(ctx, idx) {
  const client = ctx.client;
  if (gcEachSample) await client.send("HeapProfiler.collectGarbage").catch(() => {});
  const fps = await drainFps(client);
  const st = await readSampleState(client);
  const now = Date.now();

  const cur = snapshotCounters(client);
  const prev = ctx.prev ?? cur;
  ctx.prev = cur;
  const dtSec = Math.max(0.001, (now - (ctx.lastSampleAt || ctx.startedAt)) / 1000);
  ctx.lastSampleAt = now;

  const dts = (fps.dts || []).filter((d) => typeof d === "number" && d > 0 && d < 60_000);
  const fpsMedian = dts.length ? round1(1000 / median(dts)) : null;
  const frameTimeP95 = dts.length ? round1(percentile(dts, 95)) : null;
  const reconnects = Math.max(0, cur.wsCreated - 1);

  return {
    index: idx,
    atMs: now - ctx.startedAt,
    atMin: round2((now - ctx.startedAt) / 60_000),
    wallClock: new Date(now).toISOString(),
    // memory
    jsHeapUsed: st.jsHeapUsed,
    jsHeapUsedMB: toMB(st.jsHeapUsed),
    jsHeapTotalMB: toMB(st.jsHeapTotal),
    jsHeapLimitMB: toMB(st.jsHeapLimit),
    // runtime / GPU
    atlasPixelBytes: st.atlasPixelBytes,
    atlasPixelMB: toMB(st.atlasPixelBytes),
    atlasCount: st.atlasCount,
    alphaKeyedBlobBytes: st.alphaKeyedBlobBytes,
    alphaKeyedBlobMB: toMB(st.alphaKeyedBlobBytes),
    entityRendererEntities: st.entityRendererEntities,
    entityRendererLayers: st.entityRendererLayers,
    transferBytes: st.transferBytes,
    transferMB: toMB(st.transferBytes),
    cacheStorageEntryCount: st.cacheStorageEntryCount,
    // DOM
    domNodes: st.domNodes,
    domImages: st.domImages,
    cacheDomImageCount: st.cacheDomImageCount,
    damageFloaters: st.damageFloaters,
    // FPS / cadence
    fpsMedian,
    frameTimeP95,
    frames: fps.frames,
    maxFrameGapMs: fps.maxGapMs,
    totalFrames: fps.totalFrames,
    // WS health
    wsRxTotal: cur.wsRx,
    wsTxTotal: cur.wsTx,
    wsRxRate: round1((cur.wsRx - prev.wsRx) / dtSec),
    wsTxRate: round1((cur.wsTx - prev.wsTx) / dtSec),
    wsState: st.wsState,
    reconnects,
    // errors
    consoleErrorsTotal: cur.consoleErrors,
    consoleErrorsDelta: cur.consoleErrors - prev.consoleErrors,
    networkFailuresTotal: cur.networkFailures,
    networkFailuresDelta: cur.networkFailures - prev.networkFailures,
    // context
    screen: st.screen,
    mapFileName: st.mapFileName,
    player: st.player,
    entityCount: st.entityCount,
    monsterCount: st.monsterCount,
    inventoryOpen: st.inventoryOpen,
  };
}

function printSampleLine(sample) {
  const f = (v, suffix = "") => (v == null ? "·" : `${v}${suffix}`);
  console.log(
    `  · [${String(sample.index).padStart(3, "0")} | ${String(sample.atMin).padStart(5)}m] ` +
      `heap ${f(sample.jsHeapUsedMB, "MB")} atlas ${f(sample.atlasPixelMB, "MB")} dom ${f(sample.domNodes)} ` +
      `fps ${f(sample.fpsMedian)} p95 ${f(sample.frameTimeP95, "ms")} gap ${f(sample.maxFrameGapMs, "ms")} ` +
      `wsRx ${f(sample.wsRxRate, "/s")} err+${sample.consoleErrorsDelta + sample.networkFailuresDelta} ` +
      `reconn ${sample.reconnects} map ${f(sample.mapFileName)}`,
  );
}

async function runSampleLoop(ctx, deadline) {
  let idx = 0;
  while (Date.now() < deadline && ctx.running) {
    await delay(sampleMs);
    if (!ctx.running) break;
    try {
      const sample = await takeSample(ctx, idx++);
      ctx.samples.push(sample);
      printSampleLine(sample);
      // Incrementally persist the time-series so a multi-hour run that dies at hour N
      // still leaves everything up to N on disk.
      if (idx % 5 === 0) await writeTimeseries(ctx).catch(() => {});
    } catch (err) {
      ctx.addIssue({ category: "soak", severity: "low", summary: "time-series sample failed", detail: String(err?.message ?? err) });
    }
  }
}

// ---------------------------------------------------------------------------
// Activity driver — keep the client continuously busy. Each tick fires ONE real
// activity, rotating through the cycle; every action is best-effort and never aborts
// the soak. (a) click-move + held-key, (b) combat, (c) travel, (d) inventory churn.
// ---------------------------------------------------------------------------
const ACTIVITIES = [
  { name: "click-move", fn: activityClickMove },
  { name: "held-key", fn: activityHeldKey },
  { name: "combat", fn: activityCombat },
  { name: "travel", fn: activityTravel },
  { name: "inventory", fn: activityInventory },
];

async function runActivityLoop(ctx, deadline) {
  let i = 0;
  ctx.travelIndex = 0;
  ctx.dirIndex = 0;
  while (Date.now() < deadline && ctx.running) {
    const act = ACTIVITIES[i % ACTIVITIES.length];
    i += 1;
    const t0 = Date.now();
    let result = null;
    let error = null;
    try {
      result = await act.fn(ctx);
    } catch (err) {
      error = String(err?.message ?? err);
    }
    ctx.activityCounts[act.name] = (ctx.activityCounts[act.name] ?? 0) + 1;
    ctx.activityLog.push({ at: Date.now(), name: act.name, result, error, ms: Date.now() - t0 });
    if (ctx.activityLog.length > 400) ctx.activityLog.shift();
    const elapsed = Date.now() - t0;
    if (elapsed < activityIntervalMs) await delay(activityIntervalMs - elapsed);
  }
}

const STEP_DIRS = [
  [4, 0], [0, 4], [-4, 0], [0, -4],
  [3, 3], [-3, -3], [3, -3], [-3, 3],
];
const ARROW_CYCLE = ["Up", "Right", "Down", "Left"];

// (a1) Click-to-move: click an in-viewport tile a few steps away in a rotating
// direction (the production movement path → handleViewportTileAction → moveToTile).
async function activityClickMove(ctx) {
  const client = ctx.client;
  const p = await readPlayerPos(client);
  if (!p) return "no-player";
  const [dx, dy] = STEP_DIRS[ctx.dirIndex++ % STEP_DIRS.length];
  const tx = p.x + dx;
  const ty = p.y + dy;
  const ok = await clickTile(client, tx, ty, "left");
  return ok ? `click ${tx},${ty}` : "offscreen";
}

// (a2) Held keyboard: sustain ONE arrow key for heldKeyMs via CDP Input — the only
// activity that exercises the keyboard SEND pipeline (shell handleViewportDirectionIntent
// → trySendQueuedCrystalMove) the click loops never touch.
async function activityHeldKey(ctx) {
  const dir = ARROW_CYCLE[ctx.dirIndex++ % ARROW_CYCLE.length];
  await holdArrowKey(ctx.client, dir, heldKeyMs);
  return `held ${dir} ${heldKeyMs}ms`;
}

// (b) Combat: if a monster is in view, either swing (adjacent) or take ONE step toward
// it (never walk+attack the same tick — an attack arms movementInputBlockedUntil).
async function activityCombat(ctx) {
  const client = ctx.client;
  const m = await readNearestMonster(client, 12);
  if (!m) return "no-monster";
  const p = await readPlayerPos(client);
  if (!p) return "no-player";
  const cheb = Math.max(Math.abs(m.x - p.x), Math.abs(m.y - p.y));
  if (cheb <= 1) {
    if (!(await clickTile(client, m.x, m.y, "left"))) await sendCommand(client, { type: "attack", objectId: Number(m.objectId) });
    return `swing ${m.name ?? m.objectId}`;
  }
  const step = stepTowardTile(p, m);
  const ok = await clickTile(client, step.x, step.y, "left");
  return ok ? `approach ${m.name ?? m.objectId}` : "approach-offscreen";
}

// (c) Cross-map travel: rotate through the map ring (transferMap → ClientPacket).
async function activityTravel(ctx) {
  const client = ctx.client;
  const dest = TRAVEL_RING[ctx.travelIndex++ % TRAVEL_RING.length];
  const here = await client.evaluate(`window.__mir2Stage5?.state?.mapFileName ?? null`).catch(() => null);
  if (here === dest.map) return `already on ${dest.map}`;
  await sendCommand(client, { type: "transferMap", key: `crystal:${dest.map}:${dest.x}:${dest.y}` });
  const arrived = await waitUntilSoft(
    client,
    `window.__mir2Stage5?.state?.mapFileName === ${JSON.stringify(dest.map)}`,
    9_000,
  );
  return `travel ${here ?? "?"}→${dest.map} ${arrived ? "ok" : "pending"}`;
}

// (d) Inventory churn: open then close the inventory HUD window (mount/unmount) — a
// window that leaks DOM nodes per open/close shows up in the domNodes trend.
async function activityInventory(ctx) {
  const client = ctx.client;
  const before = await client.evaluate(`Boolean(document.querySelector(".inventory-window"))`).catch(() => false);
  await click(client, ".hud-button.inventory button").catch(() => {});
  await delay(450);
  const opened = await client.evaluate(`Boolean(document.querySelector(".inventory-window"))`).catch(() => false);
  // Toggle back to the state we found it in (default: leave it closed) so the HUD churns.
  if (opened !== before) await click(client, ".hud-button.inventory button").catch(() => {});
  return `inventory ${before}->${opened}`;
}

// ---------------------------------------------------------------------------
// Movement / combat primitives (from qa-combat-survival.mjs).
// ---------------------------------------------------------------------------
async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
}

async function readPlayerPos(client) {
  return client.evaluate(`(() => { const p = window.__mir2Stage5?.state?.player; return p ? { x: p.x, y: p.y } : null; })()`);
}

// Nearest live, killable (non-guard / non-prop / non-invulnerable) monster within range.
async function readNearestMonster(client, maxRange) {
  const range = Number.isFinite(maxRange) ? maxRange : 9999;
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const p = s.player ?? { x: 0, y: 0 };
      const cheb = (e) => Math.max(Math.abs(e.x - p.x), Math.abs(e.y - p.y));
      const isGuard = (e) => /guard/i.test(e.name || "");
      const invuln = (e) => typeof e.maxHp === "number" && e.maxHp >= 1000;
      const isProp = (e) => /tree|chestnut|herb|mushroom|stump|flower|sprout|sapling|shrub|\\bbush\\b|\\brock\\b|\\bore\\b|vein|plant|\\blog\\b|grass|reed|fungus/i.test(e.name || "");
      const mons = ents.filter((e) => e && e.kind === "monster" && !e.dead && !isGuard(e) && !invuln(e) && !isProp(e) && cheb(e) <= ${range});
      if (!mons.length) return null;
      mons.sort((a, b) => cheb(a) - cheb(b));
      const m = mons[0];
      return { objectId: String(m.objectId), name: m.name ?? null, x: m.x, y: m.y };
    })()
  `);
}

function stepTowardTile(player, monster) {
  const dx = Math.sign(monster.x - player.x);
  const dy = Math.sign(monster.y - player.y);
  return { x: monster.x - dx, y: monster.y - dy };
}

async function tilePoint(client, x, y) {
  const selector = `[aria-label="tile ${x}, ${y}"]`;
  return client.evaluate(`
    (() => {
      const tile = document.querySelector(${JSON.stringify(selector)});
      if (!tile) return null;
      const box = tile.getBoundingClientRect();
      return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
    })()
  `);
}

// Click a tile. Returns false when off-screen (its aria-label node exists even when
// scrolled out of view), so callers know the click did nothing.
async function clickTile(client, x, y, button = "left") {
  const point = await tilePoint(client, x, y);
  if (!point) return false;
  if (point.x < 2 || point.y < 2 || point.x > VIEWPORT.width - 2 || point.y > VIEWPORT.height - 2) return false;
  await client.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: point.x, y: point.y, button: "none" });
  await client.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: point.x,
    y: point.y,
    button,
    buttons: button === "right" ? 2 : 1,
    clickCount: 1,
  });
  await client.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: point.x, y: point.y, button, buttons: 0, clickCount: 1 });
  return true;
}

// Held-arrow-key driver (from qa-load-stress.mjs). A single keyDown adds the direction
// to the shell's held-key set (the shell drives held SENDs from its own 100ms interval
// and ignores event.repeat), so keeping the key down without a keyUp is a faithful
// "key held"; the sparse autoRepeat keydowns just shape it like real OS key-repeat.
const ARROW_KEYS = {
  Up: { key: "ArrowUp", code: "ArrowUp", vk: 38 },
  Right: { key: "ArrowRight", code: "ArrowRight", vk: 39 },
  Down: { key: "ArrowDown", code: "ArrowDown", vk: 40 },
  Left: { key: "ArrowLeft", code: "ArrowLeft", vk: 37 },
};

async function dispatchKey(client, type, k, autoRepeat = false) {
  await client.send("Input.dispatchKeyEvent", {
    type,
    key: k.key,
    code: k.code,
    windowsVirtualKeyCode: k.vk,
    nativeVirtualKeyCode: k.vk,
    autoRepeat,
    isKeypad: false,
  });
}

async function holdArrowKey(client, dirName, holdMs) {
  const k = ARROW_KEYS[dirName];
  if (!k) throw new Error(`unknown arrow direction ${dirName}`);
  // Keyboard movement is ignored while a text input is focused — make sure none is.
  await client.evaluate("(() => { const a = document.activeElement; if (a && typeof a.blur === 'function') a.blur(); return true; })()").catch(() => {});
  await dispatchKey(client, "keyDown", k, false);
  const deadline = Date.now() + holdMs;
  let nextRepeat = Date.now() + 200;
  try {
    while (Date.now() < deadline) {
      const now = Date.now();
      if (now >= nextRepeat) {
        await dispatchKey(client, "keyDown", k, true);
        nextRepeat = now + 200;
      }
      await delay(20);
    }
  } finally {
    await dispatchKey(client, "keyUp", k, false);
  }
}

// ---------------------------------------------------------------------------
// Enter world (register → login → create character → start game → scene ready).
// Compact merge of the qa-playthrough / qa-social login flow.
// ---------------------------------------------------------------------------
async function enterWorld(ctx) {
  const client = ctx.client;
  await navigate(client, RUN_URL);
  await waitUntil(client, "['login','select','game'].includes(window.__mir2Stage5?.state?.screen)", "client stage ready", 30_000);

  let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  if (screen === "login") {
    if (!(await waitForSelectorSoft(client, ".login-input.account", 12_000))) {
      throw new Error("login form (.login-input.account) never rendered");
    }
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    if (createAccount) {
      await click(client, ".login-button.account button");
      await waitUntilSoft(client, "window.__mir2Stage5?.state?.wsState === 'open'", 15_000);
      await delay(1_500);
    }
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    await click(client, ".login-button.ok button");
    await waitUntil(client, "['select','game'].includes(window.__mir2Stage5?.state?.screen)", "select/game screen", 30_000);
  }

  screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  if (screen !== "game") {
    // Trust OUR named character (a phantom slot can exist — qa-playthrough documents it).
    const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;
    if (!(await client.evaluate(haveOurChar))) {
      await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: "warrior" });
      await waitUntilSoft(client, haveOurChar, 15_000);
    }
    const startIndex = await client.evaluate(
      `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
    );
    await sendCommand(client, { type: "startGame", characterIndex: startIndex });
    await waitUntil(
      client,
      "window.__mir2Stage5?.state?.screen === 'game' && Boolean(window.__mir2Stage5?.state?.player)",
      "enter game",
      30_000,
    );
  }

  await waitUntilSoft(
    client,
    "window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true",
    SCENE_READY_TIMEOUT_MS,
  );
  await delay(1_200);
  ctx.enteredWorld = true;
}

// ---------------------------------------------------------------------------
// Detectors — fit trends over the time-series and decide PASS / LEAK / DEGRADED.
// ---------------------------------------------------------------------------
// Trend over post-warmup samples: least-squares slope (per minute) of `key`, plus
// denoised first/last (median of the first / last few post-warmup samples).
function trend(samples, key) {
  // fullSeries (all samples) drives the sparkline so the warm-up climb stays visible;
  // the slope / first / last / r² are fit over the post-warm-up steady-state window only.
  const fullSeries = samples.map((s) => (typeof s[key] === "number" && isFinite(s[key]) ? s[key] : null));
  const usable = samples.slice(warmupCount(samples.length)).filter((s) => typeof s[key] === "number" && isFinite(s[key]));
  if (usable.length < 3) return { n: usable.length, slopePerMin: null, r2: null, first: null, last: null, min: null, max: null, series: usable.map((s) => s[key]), fullSeries };
  const xs = usable.map((s) => s.atMin);
  const ys = usable.map((s) => s[key]);
  const { slope, r2 } = linregress(xs, ys);
  const headN = Math.min(3, Math.floor(usable.length / 2));
  const first = median(ys.slice(0, headN));
  const last = median(ys.slice(-headN));
  return { n: usable.length, slopePerMin: round2(slope), r2: round2(r2), first: round2(first), last: round2(last), min: Math.min(...ys), max: Math.max(...ys), series: ys, fullSeries };
}

function detectLeaks(samples) {
  const heap = trend(samples, "jsHeapUsedMB");
  const atlas = trend(samples, "atlasPixelMB");
  const dom = trend(samples, "domNodes");

  const detectors = [];
  const tooShort = samples.length - warmupCount(samples.length) < 3;

  const heapGrowth = heap.first != null && heap.last != null ? round2(heap.last - heap.first) : null;
  const heapFired =
    !tooShort && heap.slopePerMin != null && heap.slopePerMin >= LEAK_HEAP_SLOPE_MB_PER_MIN && heapGrowth >= LEAK_HEAP_MIN_GROWTH_MB && heap.r2 >= LEAK_MIN_R2;
  detectors.push({
    key: "leak-heap",
    label: "JS heap leak (retained, GC-forced)",
    fired: Boolean(heapFired),
    severity: "high",
    summary: heapFired
      ? `retained JS heap climbs ${heap.slopePerMin}MB/min (+${heapGrowth}MB, r²=${heap.r2}) and GC does not reclaim it`
      : `retained JS heap stable (slope ${fmt(heap.slopePerMin)}MB/min, Δ${fmt(heapGrowth)}MB)`,
    detail: { ...heap, growthMB: heapGrowth, thresholds: { slope: LEAK_HEAP_SLOPE_MB_PER_MIN, growth: LEAK_HEAP_MIN_GROWTH_MB, r2: LEAK_MIN_R2 } },
  });

  const atlasGrowth = atlas.first != null && atlas.last != null ? round2(atlas.last - atlas.first) : null;
  const atlasFired =
    !tooShort && atlas.slopePerMin != null && atlas.slopePerMin >= LEAK_ATLAS_SLOPE_MB_PER_MIN && atlasGrowth >= LEAK_ATLAS_MIN_GROWTH_MB && atlas.r2 >= LEAK_MIN_R2;
  detectors.push({
    key: "leak-atlas",
    label: "Bevy entity-atlas GPU-mem leak",
    fired: Boolean(atlasFired),
    severity: "high",
    summary: atlasFired
      ? `entity-atlas pixel bytes climb ${atlas.slopePerMin}MB/min (+${atlasGrowth}MB, r²=${atlas.r2})`
      : `entity-atlas pixel bytes stable (slope ${fmt(atlas.slopePerMin)}MB/min, Δ${fmt(atlasGrowth)}MB)`,
    detail: { ...atlas, growthMB: atlasGrowth, thresholds: { slope: LEAK_ATLAS_SLOPE_MB_PER_MIN, growth: LEAK_ATLAS_MIN_GROWTH_MB, r2: LEAK_MIN_R2 } },
  });

  const domGrowth = dom.first != null && dom.last != null ? Math.round(dom.last - dom.first) : null;
  const domFired =
    !tooShort && dom.slopePerMin != null && dom.slopePerMin >= LEAK_DOM_SLOPE_PER_MIN && domGrowth >= LEAK_DOM_MIN_GROWTH && dom.r2 >= LEAK_MIN_R2;
  detectors.push({
    key: "leak-dom",
    label: "DOM node-count leak",
    fired: Boolean(domFired),
    severity: "high",
    summary: domFired
      ? `DOM node count climbs ${dom.slopePerMin}/min (+${domGrowth} nodes, r²=${dom.r2})`
      : `DOM node count stable (slope ${fmt(dom.slopePerMin)}/min, Δ${fmt(domGrowth)} nodes)`,
    detail: { ...dom, growth: domGrowth, thresholds: { slope: LEAK_DOM_SLOPE_PER_MIN, growth: LEAK_DOM_MIN_GROWTH, r2: LEAK_MIN_R2 } },
  });

  return { detectors, trends: { heap, atlas, dom }, tooShort };
}

function detectFpsDegradation(samples) {
  // Spans the WHOLE run (tiny fixed settle) — first window vs last window is the point.
  const usable = samples.slice(WARMUP_SAMPLES).filter((s) => typeof s.fpsMedian === "number" && s.fpsMedian > 0);
  if (usable.length < 4) {
    return { key: "fps-degraded", label: "FPS degradation", fired: false, severity: "medium", summary: "too few FPS samples to judge", detail: { n: usable.length } };
  }
  const headN = Math.min(3, Math.floor(usable.length / 3));
  const firstFps = median(usable.slice(0, headN).map((s) => s.fpsMedian));
  const lastFps = median(usable.slice(-headN).map((s) => s.fpsMedian));
  const fired = firstFps >= FPS_MIN_HEALTHY && lastFps < firstFps * FPS_DEGRADE_RATIO && lastFps < firstFps - 5;
  return {
    key: "fps-degraded",
    label: "FPS degradation",
    fired,
    severity: "medium",
    summary: fired
      ? `median fps fell ${round1(firstFps)} → ${round1(lastFps)} (last < ${Math.round(FPS_DEGRADE_RATIO * 100)}% of first)`
      : `median fps held ${round1(firstFps)} → ${round1(lastFps)}`,
    detail: { firstFps: round1(firstFps), lastFps: round1(lastFps), ratio: round2(lastFps / firstFps) },
  };
}

function detectErrorAccumulation(samples, client) {
  if (samples.length < 4) {
    return { key: "error-accum", label: "Error accumulation", fired: false, severity: "medium", summary: "too few samples to judge", detail: {} };
  }
  const mid = Math.floor(samples.length / 2);
  const sum = (arr, k) => arr.reduce((a, s) => a + (s[k] ?? 0), 0);
  const minutes = (arr) => Math.max(1 / 60, (arr[arr.length - 1].atMs - arr[0].atMs) / 60_000 || 1 / 60);
  const early = samples.slice(0, mid);
  const late = samples.slice(mid);
  const earlyErr = sum(early, "consoleErrorsDelta") + sum(early, "networkFailuresDelta");
  const lateErr = sum(late, "consoleErrorsDelta") + sum(late, "networkFailuresDelta");
  const earlyRate = earlyErr / minutes(early);
  const lateRate = lateErr / minutes(late);
  const total = client.consoleErrorCount + client.networkFailureCount;
  const fired = total >= ERROR_MIN_TOTAL && lateRate > earlyRate * ERROR_GROWTH_RATIO && lateErr > earlyErr;
  return {
    key: "error-accum",
    label: "Error accumulation",
    fired,
    severity: "medium",
    summary: fired
      ? `error rate growing: ${round2(earlyRate)}/min early → ${round2(lateRate)}/min late (${total} total)`
      : `error rate flat/low (${round2(earlyRate)}→${round2(lateRate)}/min, ${total} total)`,
    detail: { earlyErr, lateErr, earlyRate: round2(earlyRate), lateRate: round2(lateRate), total, consoleErrors: client.consoleErrorCount, networkFailures: client.networkFailureCount },
  };
}

function detectReconnectStorm(samples, client) {
  const reconnects = Math.max(0, client.gatewaySocketCount - 1);
  // Non-open WS states observed across the run (the initial connect aside). Whole-run.
  const notOpen = samples.slice(WARMUP_SAMPLES).filter((s) => s.wsState && s.wsState !== "open").length;
  const fired = reconnects > RECONNECT_MAX || notOpen >= Math.max(3, Math.ceil(samples.length * 0.2));
  return {
    key: "reconnect-storm",
    label: "Reconnect storm",
    fired,
    severity: "high",
    summary: fired
      ? `${reconnects} gateway reconnect(s) and ${notOpen} non-open wsState sample(s)`
      : `${reconnects} reconnect(s), ${notOpen} non-open wsState sample(s)`,
    detail: { reconnects, notOpenSamples: notOpen, threshold: RECONNECT_MAX, gatewaySocketCount: client.gatewaySocketCount },
  };
}

function detectFreeze(samples, client) {
  let worst = { maxFrameGapMs: 0, index: null, atMin: null };
  for (const s of samples) {
    if ((s.maxFrameGapMs ?? 0) > (worst.maxFrameGapMs ?? 0)) worst = { maxFrameGapMs: s.maxFrameGapMs, index: s.index, atMin: s.atMin };
  }
  // A frame-bearing window with zero painted frames while the soak was active = stall.
  // Whole-run (tiny fixed settle) — a freeze anywhere must count.
  const zeroFrameWindows = samples.slice(WARMUP_SAMPLES).filter((s) => s.frames === 0 && s.totalFrames != null).length;
  const fired = worst.maxFrameGapMs >= FREEZE_GAP_MS || zeroFrameWindows >= 2 || Boolean(client.crashed);
  return {
    key: "freeze",
    label: "Freeze / render stall",
    fired,
    severity: "high",
    summary: client.crashed
      ? "renderer target CRASHED during the soak"
      : fired
        ? `worst no-frame gap ${worst.maxFrameGapMs}ms at ${worst.atMin}m${zeroFrameWindows ? `, ${zeroFrameWindows} zero-frame window(s)` : ""}`
        : `worst no-frame gap ${worst.maxFrameGapMs}ms (threshold ${FREEZE_GAP_MS}ms)`,
    detail: { worst, zeroFrameWindows, threshold: FREEZE_GAP_MS, crashed: client.crashed },
  };
}

function computeVerdict(ctx) {
  const { detectors: leakDetectors, trends, tooShort } = detectLeaks(ctx.samples);
  const fps = detectFpsDegradation(ctx.samples);
  const errors = detectErrorAccumulation(ctx.samples, ctx.client);
  const reconnect = detectReconnectStorm(ctx.samples, ctx.client);
  const freeze = detectFreeze(ctx.samples, ctx.client);

  const all = [...leakDetectors, fps, errors, reconnect, freeze];
  const leakFired = leakDetectors.some((d) => d.fired);
  const otherFired = [fps, errors, reconnect, freeze].some((d) => d.fired);
  const verdict = leakFired ? "LEAK" : otherFired ? "DEGRADED" : "PASS";
  // Hard failures gate the exit code; soft degradation is recorded but exits 0.
  const hardFail = leakFired || freeze.fired || reconnect.fired;

  // Surface each fired detector as an issue for the report's issue table.
  for (const d of all) {
    if (d.fired) ctx.addIssue({ category: "soak", severity: d.severity, summary: `${d.label}: ${d.summary}`, detail: d.detail });
  }

  return { verdict, hardFail, detectors: all, trends, tooShort };
}

// ---------------------------------------------------------------------------
// Reporting.
// ---------------------------------------------------------------------------
function classifyGateway(url) {
  const u = String(url ?? "");
  if (u.includes(":7110")) return { kind: "shared-proxy", shared: true, note: "shared :7110 node proxy — DEPLETES over long runs (project memory); a leak/degrade here may be shared-sim exhaustion, not the client" };
  if (u.includes(":7111")) return { kind: "rust-7111", shared: false, note: "Rust gateway :7111 (authoritative numerics)" };
  if (!u) return { kind: "unknown", shared: null, note: "gateway socket URL not observed" };
  return { kind: "isolated", shared: false, note: "isolated gateway (non-:7110) — measures the client, not the shared sim" };
}

async function writeTimeseries(ctx) {
  await fs.writeFile(
    path.join(outputDir, "timeseries.json"),
    JSON.stringify({ runId, baseUrl, gatewayWs, startedAt: ctx.startedAt, sampleMs, durationMin, samples: ctx.samples }, null, 2),
  );
}

async function writeReport(ctx, result) {
  const client = ctx.client;
  const gatewayUrl = client.gatewayUrl();
  const gateway = classifyGateway(gatewayUrl);
  const verdict = result.verdict;
  const samples = ctx.samples;

  const summary = {
    runId,
    verdict,
    hardFail: result.hardFail,
    baseUrl,
    gatewayWsRequested: gatewayWs ?? "(client default)",
    gatewayUrlConnected: gatewayUrl,
    gateway,
    account,
    characterName,
    enteredWorld: ctx.enteredWorld,
    startedAt: ctx.startedAt,
    finishedAt: Date.now(),
    durationMin,
    sampleMs,
    activityIntervalMs,
    plannedSamples: Math.round(durationMs / sampleMs),
    sampleCount: samples.length,
    activityCounts: ctx.activityCounts,
    activityTotal: Object.values(ctx.activityCounts).reduce((a, b) => a + b, 0),
    consoleErrors: client.consoleErrorCount,
    networkFailures: client.networkFailureCount,
    reconnects: Math.max(0, client.gatewaySocketCount - 1),
    detectors: result.detectors.map((d) => ({ key: d.key, label: d.label, fired: d.fired, severity: d.severity, summary: d.summary })),
    issueCount: ctx.issues.length,
  };

  await fs.writeFile(path.join(outputDir, "summary.json"), JSON.stringify(summary, null, 2));
  await fs.writeFile(path.join(outputDir, "report.json"), JSON.stringify({ summary, detectors: result.detectors, trends: result.trends, issues: ctx.issues, activityLog: ctx.activityLog.slice(-120) }, null, 2));
  await fs.writeFile(path.join(outputDir, "console.json"), JSON.stringify({ count: client.consoleErrorCount, errors: client.consoleErrors }, null, 2));
  await fs.writeFile(path.join(outputDir, "network-failures.json"), JSON.stringify({ count: client.networkFailureCount, failures: client.networkFailures }, null, 2));
  await writeTimeseries(ctx);

  const verdictBanner = verdict === "PASS" ? "✅ PASS" : verdict === "LEAK" ? "🔴 LEAK" : "🟠 DEGRADED";
  const md = [];
  md.push(`# Soak QA report — ${runId}`);
  md.push("");
  md.push(`## Verdict: ${verdictBanner}`);
  md.push("");
  md.push(`- Duration: **${durationMin} min** · ${samples.length}/${summary.plannedSamples} samples @ ${sampleMs}ms · activity @ ${activityIntervalMs}ms (${summary.activityTotal} actions)`);
  md.push(`- Target: \`${baseUrl}\` · account \`${account}\` · character \`${characterName}\` · enteredWorld=${ctx.enteredWorld}`);
  md.push(`- Gateway: requested \`${summary.gatewayWsRequested}\` · connected \`${gatewayUrl ?? "?"}\` → **${gateway.kind}**`);
  if (gateway.shared) md.push(`  - ⚠️ ${gateway.note}`);
  else md.push(`  - ${gateway.note}`);
  md.push(`- Totals: ${summary.consoleErrors} console error(s) · ${summary.networkFailures} network failure(s) · ${summary.reconnects} reconnect(s)`);
  md.push("");

  md.push("## Detectors");
  md.push("");
  md.push("| Detector | Result | Detail |");
  md.push("|---|---|---|");
  for (const d of result.detectors) {
    md.push(`| ${d.label} | ${d.fired ? "🔴 FIRED" : "✅ ok"} | ${String(d.summary).replace(/\|/g, "\\|")} |`);
  }
  md.push("");

  md.push(`## Trends (slope fit over steady-state tail, past ${Math.round(WARMUP_FRACTION * 100)}% warm-up; sparkline = full run)`);
  md.push("");
  md.push("| Metric | First | Last | Δ | Slope/min | r² | Sparkline |");
  md.push("|---|---|---|---|---|---|---|");
  md.push(trendRow("JS heap (MB)", result.trends.heap));
  md.push(trendRow("Atlas pixels (MB)", result.trends.atlas));
  md.push(trendRow("DOM nodes", result.trends.dom));
  md.push(metricRow(samples, "Median FPS", "fpsMedian"));
  md.push(metricRow(samples, "Frame p95 (ms)", "frameTimeP95"));
  md.push(metricRow(samples, "alphaKeyed blob (MB)", "alphaKeyedBlobMB"));
  md.push(metricRow(samples, "transfer (MB)", "transferMB"));
  md.push(metricRow(samples, "cacheStorage entries", "cacheStorageEntryCount"));
  md.push(metricRow(samples, "WS rx/s", "wsRxRate"));
  md.push("");

  md.push("## Per-window samples");
  md.push("");
  const rows = downsample(samples, 60);
  if (rows.length < samples.length) md.push(`_Showing ${rows.length} of ${samples.length} samples (downsampled)._`);
  md.push("| # | min | heapMB | atlasMB | dom | imgs | fps | p95ms | gapMs | wsRx/s | wsState | err+ | reconn | map | mons |");
  md.push("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
  for (const s of rows) {
    md.push(
      `| ${s.index} | ${s.atMin} | ${fmt(s.jsHeapUsedMB)} | ${fmt(s.atlasPixelMB)} | ${fmt(s.domNodes)} | ${fmt(s.domImages)} | ${fmt(s.fpsMedian)} | ${fmt(s.frameTimeP95)} | ${fmt(s.maxFrameGapMs)} | ${fmt(s.wsRxRate)} | ${s.wsState ?? "?"} | ${s.consoleErrorsDelta + s.networkFailuresDelta} | ${s.reconnects} | ${s.mapFileName ?? "?"} | ${fmt(s.monsterCount)} |`,
    );
  }
  md.push("");

  md.push("## Activity mix");
  md.push("");
  for (const [name, count] of Object.entries(ctx.activityCounts)) md.push(`- ${name}: ${count}`);
  md.push("");

  const order = { high: 0, medium: 1, low: 2 };
  const sortedIssues = [...ctx.issues].sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9));
  md.push("## Issues");
  md.push("");
  if (!sortedIssues.length) {
    md.push("_No issues detected._");
  } else {
    md.push("| # | Severity | Category | Summary |");
    md.push("|---|---|---|---|");
    for (const i of sortedIssues) md.push(`| ${i.id} | ${i.severity} | ${i.category} | ${String(i.summary).replace(/\|/g, "\\|")} |`);
    md.push("");
    md.push("### Evidence");
    md.push("");
    for (const i of sortedIssues) {
      md.push(`#### ${i.id} — [${i.severity}/${i.category}] ${i.summary}`);
      if (i.detail !== undefined) md.push("```json\n" + JSON.stringify(i.detail, null, 2) + "\n```");
      md.push("");
    }
  }
  md.push("");

  md.push("## Reproduce");
  md.push("");
  md.push("```bash");
  md.push(`cd apps/web && node ./scripts/qa-soak.mjs --headed --durationMin ${durationMin} --baseUrl ${baseUrl}${gatewayWs ? ` --gatewayWs ${gatewayWs}` : ""}`);
  md.push("```");
  md.push("");
  md.push("> Files: `timeseries.json` (full series) · `report.json` · `summary.json` · `console.json` · `network-failures.json` · `frames/`.");

  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

function trendRow(label, t) {
  const spark = t ? sparkline(t.fullSeries ?? t.series) : "";
  if (!t || t.slopePerMin == null) return `| ${label} | — | — | — | — | — | \`${spark}\` |`;
  const delta = t.first != null && t.last != null ? round2(t.last - t.first) : null;
  return `| ${label} | ${fmt(t.first)} | ${fmt(t.last)} | ${fmt(delta)} | ${fmt(t.slopePerMin)} | ${fmt(t.r2)} | \`${spark}\` |`;
}

function metricRow(samples, label, key) {
  const t = trend(samples, key);
  return trendRow(label, t);
}

// ---------------------------------------------------------------------------
// Math / formatting helpers.
// ---------------------------------------------------------------------------
function linregress(xs, ys) {
  let n = 0, sx = 0, sy = 0, sxx = 0, sxy = 0, syy = 0;
  for (let i = 0; i < xs.length; i += 1) {
    const x = xs[i];
    const y = ys[i];
    if (typeof y !== "number" || !isFinite(y)) continue;
    n += 1; sx += x; sy += y; sxx += x * x; sxy += x * y; syy += y * y;
  }
  if (n < 2) return { slope: 0, intercept: ys[0] ?? 0, r2: 0, n };
  const d = n * sxx - sx * sx;
  if (d === 0) return { slope: 0, intercept: sy / n, r2: 0, n };
  const slope = (n * sxy - sx * sy) / d;
  const intercept = (sy - slope * sx) / n;
  const rDen = Math.sqrt((n * sxx - sx * sx) * (n * syy - sy * sy));
  const r = rDen === 0 ? 0 : (n * sxy - sx * sy) / rDen;
  return { slope, intercept, r2: r * r, n };
}

function median(arr) {
  const a = arr.filter((v) => typeof v === "number" && isFinite(v)).slice().sort((x, y) => x - y);
  if (!a.length) return null;
  const mid = Math.floor(a.length / 2);
  return a.length % 2 ? a[mid] : (a[mid - 1] + a[mid]) / 2;
}

function percentile(arr, p) {
  const a = arr.filter((v) => typeof v === "number" && isFinite(v)).slice().sort((x, y) => x - y);
  if (!a.length) return null;
  const idx = Math.min(a.length - 1, Math.max(0, Math.ceil((p / 100) * a.length) - 1));
  return a[idx];
}

function sparkline(values) {
  const bars = "▁▂▃▄▅▆▇█";
  const nums = (values ?? []).filter((v) => typeof v === "number" && isFinite(v));
  if (!nums.length) return "";
  const min = Math.min(...nums);
  const max = Math.max(...nums);
  const range = max - min || 1;
  return (values ?? [])
    .map((v) => (typeof v === "number" && isFinite(v) ? bars[Math.min(7, Math.max(0, Math.floor(((v - min) / range) * 7.999)))] : " "))
    .join("");
}

function downsample(arr, maxRows) {
  if (arr.length <= maxRows) return arr;
  const step = Math.ceil(arr.length / maxRows);
  const out = [];
  for (let i = 0; i < arr.length; i += step) out.push(arr[i]);
  if (out[out.length - 1] !== arr[arr.length - 1]) out.push(arr[arr.length - 1]);
  return out;
}

function clampNumber(v, lo, hi) {
  return Math.min(hi, Math.max(lo, v));
}

// How many leading samples to discard as warm-up before fitting trends: the larger of
// the absolute floor and the warm-up fraction, but never so many that <3 remain.
function warmupCount(n) {
  const want = Math.max(WARMUP_SAMPLES, Math.floor(n * WARMUP_FRACTION));
  return clampNumber(want, 0, Math.max(0, n - 3));
}

function toMB(bytes) {
  return typeof bytes === "number" && isFinite(bytes) ? Math.round((bytes / 1_048_576) * 10) / 10 : null;
}
function round1(v) {
  return typeof v === "number" && isFinite(v) ? Math.round(v * 10) / 10 : null;
}
function round2(v) {
  return typeof v === "number" && isFinite(v) ? Math.round(v * 100) / 100 : null;
}
function fmt(v) {
  return v == null ? "—" : String(v);
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-combat-survival.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-soak-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      ...(disableGpu ? ["--disable-gpu"] : ["--ignore-gpu-blocklist", "--enable-webgl"]),
      // Exact (non-bucketed) performance.memory.usedJSHeapSize → accurate leak slopes.
      "--enable-precise-memory-info",
      // Keep rAF + timers running at full rate even when the window is occluded — a soak
      // must not stall just because the OS backgrounds the window (would false-FREEZE).
      "--disable-background-timer-throttling",
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
      "--no-proxy-server",
      "--proxy-bypass-list=*",
      "--no-first-run",
      "--no-default-browser-check",
      `--window-size=${VIEWPORT.width},${VIEWPORT.height}`,
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  chrome.userDataDir = userDataDir;
  await waitForChrome();
  return chrome;
}

async function createPageTarget() {
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/new?${encodeURIComponent(RUN_URL)}`, { method: "PUT" });
  if (!response.ok) throw new Error(`Chrome target creation failed: ${response.status}`);
  const target = await response.json();
  targetAlreadyNavigated = true;
  await delay(3_000);
  return target.webSocketDebuggerUrl;
}

async function waitForChrome() {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/version`);
      if (response.ok) return;
    } catch {
      await delay(100);
    }
  }
  throw new Error("Timed out waiting for Chrome debug endpoint.");
}

async function setViewport(client, viewport) {
  await client.send("Emulation.setDeviceMetricsOverride", viewport);
  await client.send("Emulation.setVisibleSize", { width: viewport.width, height: viewport.height });
}

async function navigate(client, url) {
  if (targetAlreadyNavigated) {
    try {
      await waitUntil(client, "document.readyState === 'complete' || document.readyState === 'interactive'", "page load", 15_000);
      const currentUrl = await client.evaluate("window.location.href");
      if (typeof currentUrl !== "string" || !currentUrl.startsWith("chrome-error://")) return;
    } catch {
      targetAlreadyNavigated = false;
    }
  }
  let lastError;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      await client.send("Page.navigate", { url });
      await waitUntil(client, "document.readyState === 'complete' || document.readyState === 'interactive'", "page load", 15_000);
      const currentUrl = await client.evaluate("window.location.href");
      if (typeof currentUrl !== "string" || !currentUrl.startsWith("chrome-error://")) return;
      lastError = new Error(`Chrome landed on ${currentUrl}`);
    } catch (error) {
      lastError = error;
    }
    await delay(400);
  }
  throw lastError ?? new Error("Page navigation failed.");
}

async function fillInput(client, selector, value) {
  const ok = await client.evaluate(`
    (() => {
      const input = document.querySelector(${JSON.stringify(selector)});
      if (!input) return false;
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      setter.call(input, ${JSON.stringify(value)});
      input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText" }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    })()
  `);
  if (!ok) throw new Error(`Could not fill ${selector}`);
}

async function click(client, selector) {
  const ok = await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!ok) throw new Error(`Could not click ${selector}`);
}

async function waitUntil(client, expression, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (v) return;
    await delay(120);
  }
  const debug = await client
    .evaluate(`(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, body: document.body?.innerText?.slice(0, 200) ?? "" }))()`)
    .catch((e) => ({ debugError: String(e) }));
  throw new Error(`Timed out waiting for ${label}; debug=${JSON.stringify(debug)}`);
}

async function waitUntilSoft(client, expression, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (v) return true;
    await delay(120);
  }
  return false;
}

async function waitForSelectorSoft(client, selector, timeoutMs) {
  return waitUntilSoft(client, `document.querySelector(${JSON.stringify(selector)}) != null`, timeoutMs);
}

async function captureFrame(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
}

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) return;
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve));
  if (chrome.userDataDir) await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => {});
}

// ---------------------------------------------------------------------------
// Small utilities.
// ---------------------------------------------------------------------------
function buildRunUrl(base, gw) {
  const params = ["mir2Debug=1"];
  if (gw) params.push(`gatewayWs=${encodeURIComponent(gw)}`);
  return `${base}${base.includes("?") ? "&" : "?"}${params.join("&")}`;
}

function classifyAssetUrl(url) {
  const t = String(url ?? "");
  if (t.includes("/bevy-runtime/")) return "runtime";
  if (t.includes("/original-ui/")) return "ui";
  if (t.includes("/original-map/")) return "map";
  if (t.includes("entity") || t.includes("atlas")) return "entity";
  if (/\.(png|webp|ktx2|basis|wav|mp3|ogg)(\?|$)/i.test(t)) return "asset";
  return "other";
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = "true";
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

function numberArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function defaultName(prefix) {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `${prefix}${suffix}`.slice(0, 12).toUpperCase();
}

function findChromePath() {
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------
async function main() {
  await fs.mkdir(framesDir, { recursive: true });
  console.log(`qa-soak: target=${baseUrl} gateway=${gatewayWs ?? "(client default)"} duration=${durationMin}min sample=${sampleMs}ms activity=${activityIntervalMs}ms headed=${headed}`);
  console.log(`qa-soak: output=${outputDir}`);

  const chrome = await launchChrome();
  let client;
  let ctx;
  let finalized = false;
  try {
    const wsUrl = await createPageTarget();
    client = new CdpClient(wsUrl);
    await client.connect();
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await client.send("Page.enable");
    await client.send("Inspector.enable").catch(() => {});
    await client.send("HeapProfiler.enable").catch(() => {});
    await client.send("Page.bringToFront").catch(() => {});
    await setViewport(client, VIEWPORT);

    ctx = createContext(client);
    ctx.startedAt = Date.now();

    // Graceful flush on Ctrl-C so a long soak still leaves a report.
    const onSigint = () => {
      if (!ctx.running) return;
      console.log("\nqa-soak: SIGINT — stopping and writing a partial report…");
      ctx.running = false;
    };
    process.once("SIGINT", onSigint);

    try {
      await enterWorld(ctx);
    } catch (err) {
      ctx.enteredWorld = false;
      ctx.addIssue({ category: "flow", severity: "high", summary: "could not enter world; soaking from current screen", detail: String(err?.message ?? err) });
    }

    await client.evaluate(installFpsProbeSrc()).catch(() => {});
    await captureFrame(client, path.join(framesDir, "00-start.png")).catch(() => {});
    ctx.lastSampleAt = Date.now();
    ctx.prev = snapshotCounters(client);
    ctx.running = true;

    const deadline = Date.now() + durationMs;
    console.log(`\nqa-soak: soaking for ${durationMin} min (~${Math.round(durationMs / sampleMs)} samples planned)…\n`);
    await Promise.all([runSampleLoop(ctx, deadline), runActivityLoop(ctx, deadline)]);
    ctx.running = false;

    // Final GC + a closing sample so the trend ends on a settled, garbage-free heap.
    await client.send("HeapProfiler.collectGarbage").catch(() => {});
    try {
      const closing = await takeSample(ctx, ctx.samples.length);
      ctx.samples.push(closing);
      printSampleLine(closing);
    } catch {
      /* best-effort */
    }
    await captureFrame(client, path.join(framesDir, "99-end.png")).catch(() => {});

    const result = computeVerdict(ctx);
    await writeReport(ctx, result);
    finalized = true;

    const fired = result.detectors.filter((d) => d.fired).map((d) => d.key);
    console.log(`\nqa-soak done: verdict ${result.verdict}${fired.length ? ` (fired: ${fired.join(", ")})` : ""}. ${ctx.samples.length} samples, ${ctx.issues.length} issue(s).`);
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
    if (result.hardFail) process.exitCode = 1;
  } catch (error) {
    console.error("qa-soak fatal:", error);
    if (ctx && !finalized) {
      try {
        ctx.running = false;
        ctx.addIssue({ category: "flow", severity: "high", summary: "fatal harness error", detail: String(error?.message ?? error) });
        const result = computeVerdict(ctx);
        await writeReport(ctx, result);
      } catch {
        /* ignore */
      }
    }
    process.exitCode = 1;
  } finally {
    client?.close();
    await stopChrome(chrome).catch(() => {});
  }
}

main();
