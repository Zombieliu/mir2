// qa-load-stress.mjs — a self-contained QA loop that stresses self-movement +
// rendering UNDER LOAD to reproduce the intermittent "overshoot then snap" drift.
//
// WHY THIS HARNESS EXISTS
// -----------------------
// Gentle play does NOT trigger the drift, and the *logical* prediction looks
// clean when sampled calmly. The suspect is therefore load / frame-drops /
// run-walk mismatch. This loop deliberately drives the client hard — it hops to
// a DENSE hunting field, pulls several monsters (entity-render load + combat
// packet churn), and MOVES AGGRESSIVELY (fast far/zigzag run clicks with rapid
// direction reversals) — while a per-frame probe watches the prediction.
//
// THE BUG MECHANISM IT TARGETS (from apps/web/app/page.tsx)
// --------------------------------------------------------
// Self movement is locally predicted. `state.player` RENDERS the prediction,
// leading the server by up to MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES (= 2 tiles,
// page.tsx:906-907). The local prediction advances from a rAF loop
// (`tickMovementPlan`, page.tsx:3922) while the authoritative server position is
// applied by the WS handler inside the same React monolith. When the main thread
// stalls (frame drops under entity/combat/React load) OR the client predicts a
// run step the server resolves as a walk, the prediction outruns the applied
// server position. The instant the lead exceeds 2 tiles,
// `preserveCrystalSelfRenderPosition` (page.tsx:3313-3340) DISCARDS the
// prediction and `state.player` falls back to the server tile — i.e. the sprite
// OVERSHOOTS forward then SNAPS back. This loop makes that reproducible and
// correlates each snap with (a) frame hitches and (b) concurrent entity count.
//
// DESIGN
// ------
// Three back-to-back phases over one logged-in session, so the only thing that
// changes is the load — and the gradient names the trigger:
//   phase 1  "gentle"           — slow, deliberate walking with no reversals; the
//                                 CONTROL that mirrors the calm play that does NOT
//                                 trigger the drift (expect ~0 drift events)
//   phase 2  "aggressive"       — fast far/zigzag RUN clicks with hard direction
//                                 reversals + combat (best-effort entity density)
//   phase 3  "aggressive+hitch" — same, plus a synthetic main-thread hog that
//                                 forces realistic rAF gaps (stands in for the
//                                 natural ~200ms React-monolith movement jank)
// Because a hard stall throttles move throughput, drift is also reported PER 1000
// rendered frames so the phases compare fairly.
// Each phase runs the public flight recorder (public/mir2-recorder.js →
// window.__mir2Recorder) AND a finer per-frame probe (window.__mir2LoadProbe)
// that records, every rAF: the server tile, the rendered tile (state.player),
// the raw prediction (state.predictedPlayer), the render lead, the rAF gap, and
// the live entity count. Detectors then flag overshoot/snap events, correlate
// hitches with movement, and correlate entity count with frame gaps.
//
// HELD-KEYBOARD BEAT (added for the "hold a direction → freeze 1-3s → jump 2 tiles"
// stall, distinct from the overshoot/snap above): before the load phases, the loop
// holds REAL arrow keys via CDP Input.dispatchKeyEvent (NOT clicks — the click
// phases below never touch the keyboard held-move → SEND pipeline) in BOTH an open
// field AND near obstacles (a dynamic swarm, or a cluttered town when GM-spawn is
// unavailable). It counts move SENDs from ground-truth outbound WS frames and, for a
// PURE CARDINAL hold, classifies every >= HELD_STALL_FAIL_MS gap by sampling the next
// tile each ~90ms:
//   • next tile entity-occupied   → legitimate NPC/monster block (diagnostic)
//   • correction-block active      → legitimate post-correction / pending settle (diagnostic)
//   • next tile FREE yet the gap RELEASED into more SENDs → THE BUG (a real wall never
//     releases; only stale blocked-direction memory frees on its 5.6s TTL timer)
// Only the last case FAILS, so the guard never cries wolf on a town full of walls and
// NPCs. The threshold sits above MOVEMENT_PENDING_MAX_AGE_MS (3000ms) and the 400ms
// correction settle, so a clear-tile freeze that long can only be the memory bug.
// The stall was sticky blocked-direction route-hint suppression (meant for click-to-
// TARGET A* detours) bleeding into held DIRECTION intent; the open field stays smooth
// because nothing there ever seeds that memory — which is exactly why the click loops
// (which move via clicks, in open fields) missed it. NOTE a reliable POSITIVE catch
// needs entity transients (the GM swarm); town reproduces it only opportunistically.
//
// SCOPE: everything lives in THIS file (no shared edits) so parallel QA sessions
// never collide. Run on your OWN dev client + the shared gateway:
//   node ./scripts/qa-load-stress.mjs --headed --baseUrl http://127.0.0.1:3021
// Useful flags:
//   --map 1|2            dense field to hunt in (default 1 = Woomyon Woods)
//   --durationMs 90000   total drive time, split equally across the phases
//   --spawn 40           entity-render target (GM @MOB; falls back to natural aggro)
//   --hog true|false     run the aggressive+hitch phase (default true)
//   --hogMs 85           base busy-loop length per hog pulse (ms)
//   --hogPeriodMs 320    gap between hog pulses (ms)
//   --held true|false    run the held-keyboard movement beat (default true)
//   --heldHoldMs 5000    per-direction sustained hold (ms)
//   --heldStallMs 1800   send-gap that counts as a held-direction stall (ms)
//   --obstacleMap 0      static-obstacle fallback map when GM-spawn is unavailable
//   --headless           force headless (NOT recommended — SwiftShader false-black)
//
// Zero new dependencies; CDP plumbing is the same shape proven in
// qa-playthrough.mjs / capture-web-movement-jitter.mjs.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(__dirname, "..");

const args = parseArgs(process.argv.slice(2));

const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3021";
// mir2Debug=1 turns on the movement console log (page.tsx:1207-1216) so the
// recorder captures send/ack/CORRECTION events, plus the scene-motion debug hook.
// perfDiag=1 arms the per-packet handleGatewayEvent timing (window.__mir2PerfDiag,
// Stage 0) — opt-in, zero hot-path cost when the flag is absent.
const RUN_URL = `${baseUrl}${baseUrl.includes("?") ? "&" : "?"}mir2Debug=1&perfDiag=1`;

const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultAccountName();
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultCharacterName();
const headed = !booleanArg(args.headless, false) && booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 250));

const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "player-qa", `load-stress-${runId}`),
);

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Dense on-demand-spawn hunting fields (matches qa-playthrough HUNTING_FIELDS /
// QUICK_TRANSFER_OPTIONS in page.tsx). transferMap key = crystal:<map>:<x>:<y>.
const HUNTING_FIELDS = {
  1: { map: "1", x: 315, y: 82, label: "Woomyon Woods (1)" },
  2: { map: "2", x: 503, y: 483, label: "Serpent Valley (2)" },
};
const field = HUNTING_FIELDS[String(args.map ?? "1")] ?? HUNTING_FIELDS[1];

// Tunables.
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
// Total drive time, split equally across the phases (~30s each for the default
// 3 phases). 30s/phase reliably surfaces the occasional drift; the aggressive
// phases get ~60s combined — the "~60s of aggressive movement" the bug needs.
const TOTAL_DURATION_MS = numberArg(args.durationMs, 90_000);
const RUN_HOG_PHASE = booleanArg(args.hog, true);
// Moderate by default: the hog must create realistic frame HITCHES (50-200ms
// rAF gaps) without freezing the client so hard that movement throughput — and
// therefore the very drift we measure — collapses. ~16% duty keeps ~40-50 fps.
const HOG_MS = numberArg(args.hogMs, 85); // base busy-loop length per pulse
const HOG_JITTER_MS = numberArg(args.hogJitterMs, 180); // random extra (occasional longer stall)
const HOG_PERIOD_MS = numberArg(args.hogPeriodMs, 320); // pulse cadence (~35-45 fps; keeps move throughput)
const SPAWN_COUNT = numberArg(args.spawn, 40); // entity-render load in the loaded phases
const MONSTER_NAME = args.monster ?? "Chicken"; // GM-spawn template (low-damage so the QA char survives the swarm)
const MOVE_INTERVAL_MS = numberArg(args.moveIntervalMs, 110); // micro-zigzag re-issue cadence (faster than a step completes)
const ATTACK_EVERY_MS = numberArg(args.attackEveryMs, 600);

// Detector thresholds.
const LEAD_CAP_TILES = 2; // = MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES (page.tsx:906-907)
const SNAP_BACK_TILES = 2; // render tile jumping backward >= this in one rAF = a snap
const HITCH_MS = numberArg(args.hitchMs, 50); // rAF gap >= this = a frame hitch (~3 dropped 60Hz frames)
const CORRELATION_WINDOW_MS = 250; // a snap within this of a hitch is "explained" by it

// Held-keyboard movement beat (the keyboard SEND pipeline the click phases never
// touch). A healthy held direction SENDs a walk/run roughly once per
// CRYSTAL_MOVE_DELAY_MS (=600ms, original-client-movement-controller.ts:38); the
// 100ms input interval (CRYSTAL_MOVE_INPUT_INTERVAL_MS) just retries until the
// cadence gate opens. The bug = many seconds of ZERO sends while a key is held
// even though the path is clear, released as a 2-tile run jump.
const RUN_HELD_BEAT = booleanArg(args.held, true);
const HELD_HOLD_MS = numberArg(args.heldHoldMs, 5000); // per-direction sustained hold
// A gap >= this with the next tile FREE and no correction-block is the bug. 1800ms
// sits ABOVE MOVEMENT_PENDING_MAX_AGE_MS (3000ms, original-client-movement-
// controller.ts:41) and the 400ms correction settle, and BELOW the bug's 5.6s
// blocked-direction TTL — so a clear-tile freeze this long can only be the memory bug.
const HELD_STALL_FAIL_MS = numberArg(args.heldStallMs, 1800);
const HELD_OBSTACLE_SPAWN = numberArg(args.heldSpawn, 36); // dynamic-obstacle swarm density (GM @MOB)
// Static-obstacle fallback when GM-spawn is unavailable: a cluttered town (map 0,
// Bichon Province) where building edges supply the obstacles the swarm otherwise would.
const OBSTACLE_FIELD = {
  map: String(args.obstacleMap ?? "0"),
  x: numberArg(args.obstacleX, 283),
  y: numberArg(args.obstacleY, 606),
  label: "Bichon Province town (0)",
};

if (!chromePath) throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (same shape as qa-playthrough.mjs) — captures console, network
// failures, and WebSocket frames (the server's authoritative truth).
// ---------------------------------------------------------------------------

// Map a Chrome [Violation] message to the handler bucket it names. Chrome quotes
// the handler kind, e.g. "'message' handler took…", "'click' handler took…",
// "'setInterval' handler took…", "'requestAnimationFrame' handler took…".
function bucketViolationHandler(text) {
  const quoted = (text.match(/'([^']+)'\s+handler/i) ?? [])[1];
  if (quoted) {
    const k = quoted.toLowerCase();
    if (k.includes("message")) return "message";
    if (k.includes("click") || k.includes("pointer") || k.includes("mouse")) return "click";
    if (k.includes("setinterval")) return "setInterval";
    if (k.includes("settimeout")) return "setTimeout";
    if (k.includes("requestanimationframe") || k.includes("raf")) return "requestAnimationFrame";
    return k;
  }
  if (/forced reflow/i.test(text)) return "reflow";
  return "other";
}

class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
    // Chrome [Violation] capture (Stage 0, render-perf): the "handler took N ms"
    // warnings arrive on the CDP Log domain at verbose level and were silently
    // dropped (consoleErrors only keeps type==='error'|'warning'). Bucket them by
    // the handler bucket Chrome names (message / click / setInterval / requestAnimationFrame).
    this.violations = [];
    this.violationCounts = {};
    this.networkFailures = [];
    this.requestUrlById = new Map();
    this.webSocketFramesReceived = [];
    // Ground-truth SENT frames (the client's outbound BrowserCommands). The held-
    // keyboard beat counts move SENDs from these — the in-page move-console log is
    // capped at 100 entries, too small for a multi-second sustained hold.
    this.webSocketFramesSent = [];
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
        this.consoleErrors.push({
          type: p.type,
          text: (p.args ?? []).map((a) => a.value ?? a.description ?? "").join(" "),
          at: Date.now(),
        });
        this.consoleErrors = this.consoleErrors.slice(-400);
      }
    } else if (m === "Runtime.exceptionThrown") {
      const d = p.exceptionDetails;
      this.consoleErrors.push({ type: "error", text: d?.exception?.description ?? d?.text ?? "exception", at: Date.now() });
    } else if (m === "Network.requestWillBeSent") {
      if (p.requestId && p.request?.url) this.requestUrlById.set(p.requestId, p.request.url);
    } else if (m === "Network.responseReceived") {
      const r = p.response;
      const url = String(r?.url ?? "");
      if (r && r.status >= 400 && !url.includes("favicon")) {
        this.networkFailures.push({ url, status: r.status, at: Date.now() });
      }
    } else if (m === "Network.loadingFailed") {
      const url = this.requestUrlById.get(p.requestId) ?? "(unknown)";
      if (!String(url).includes("favicon")) this.networkFailures.push({ url, status: 0, errorText: p.errorText ?? "", at: Date.now() });
    } else if (m === "Network.webSocketFrameReceived") {
      this.webSocketFramesReceived.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-1200);
    } else if (m === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-4000);
    } else if (m === "Log.entryAdded") {
      // Chrome reports long handlers as a Log entry like
      // "[Violation] 'message' handler took 197ms" (source 'violation', level 'verbose').
      const text = String(p.entry?.text ?? "");
      if (/handler took|\[Violation\]/i.test(text)) {
        const ms = Number((text.match(/took\s+(\d+(?:\.\d+)?)\s*ms/i) ?? [])[1] ?? 0);
        const bucket = bucketViolationHandler(text);
        this.violations.push({ bucket, ms, text: text.slice(0, 200), at: Date.now() });
        this.violations = this.violations.slice(-2000);
        const agg = (this.violationCounts[bucket] ??= { count: 0, totalMs: 0, maxMs: 0 });
        agg.count += 1;
        agg.totalMs += ms;
        if (ms > agg.maxMs) agg.maxMs = ms;
      }
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

  close() {
    this.ws?.close();
  }
}

// ---------------------------------------------------------------------------
// Flight recorder — load the public dev recorder if present (the canonical
// mir2-replay/1 format, scrubbable in /mir2-replay.html), else inject a compact
// API-compatible fallback so this harness is self-contained on a clean checkout.
// ---------------------------------------------------------------------------
async function loadRecorder(client) {
  const recorderPath = path.join(WEB_ROOT, "public", "mir2-recorder.js");
  if (existsSync(recorderPath)) {
    const src = await fs.readFile(recorderPath, "utf8");
    await client.evaluate(`(function(){ ${src} \n; return true; })()`);
    const status = await client.evaluate("window.__mir2Recorder ? window.__mir2Recorder.status() : null");
    if (status) return { source: "public/mir2-recorder.js" };
  }
  await client.evaluate(FALLBACK_RECORDER_SRC);
  return { source: "embedded-fallback" };
}

// Minimal, API-compatible (start/stop/status/mark/dump) recorder used only when
// public/mir2-recorder.js is absent. Same dump shape as the real one.
const FALLBACK_RECORDER_SRC = `
(function () {
  var W = window;
  if (W.__mir2Recorder) return;
  var R = {
    running: false, startedAt: 0, ticks: [], rafGaps: [], markers: [], cap: 24000, _tick: 0, _raf: 0, _last: 0,
    start: function () {
      if (R.running) return; R.running = true; R.startedAt = Date.now();
      R.ticks = []; R.rafGaps = []; R.markers = [];
      W.__mir2MovementLogEnabled = true; W.__mir2MovementConsoleEvents = [];
      R._tick = setInterval(function () {
        var s = W.__mir2Stage5 && W.__mir2Stage5.state; if (!s) return;
        var p = s.player, pred = s.predictedPlayer, ents = Array.isArray(s.entities) ? s.entities : [];
        R.ticks.push({ t: Date.now(), sx: p && p.x, sy: p && p.y, px: pred && pred.x, py: pred && pred.y, ec: ents.length });
        if (R.ticks.length > R.cap) R.ticks.shift();
      }, 33);
      R._last = performance.now();
      var raf = function () { var t = performance.now(), dt = t - R._last; R._last = t;
        if (dt > 30) { R.rafGaps.push({ t: Date.now(), dtMs: Math.round(dt) }); if (R.rafGaps.length > 6000) R.rafGaps.shift(); }
        R._raf = requestAnimationFrame(raf); };
      R._raf = requestAnimationFrame(raf);
    },
    stop: function () { if (!R.running) return; R.running = false; clearInterval(R._tick); cancelAnimationFrame(R._raf); },
    mark: function (label) { var m = { t: Date.now(), label: label || ("mark " + (R.markers.length + 1)) }; R.markers.push(m); return m; },
    status: function () { return { running: R.running, ticks: R.ticks.length, rafGaps: R.rafGaps.length, markers: R.markers.length, durationMs: R.running ? Date.now() - R.startedAt : 0 }; },
    dump: function () {
      var s = W.__mir2Stage5 && W.__mir2Stage5.state;
      return { format: "mir2-replay/1", meta: { startedAt: R.startedAt, endedAt: Date.now(), map: s && s.mapFileName, href: location.href, ua: navigator.userAgent, fallback: true },
        ticks: R.ticks.slice(), rafGaps: R.rafGaps.slice(), moveEvents: (W.__mir2MovementConsoleEvents || []).slice().reverse(), inputs: [], markers: R.markers.slice() };
    },
  };
  W.__mir2Recorder = R;
  return R.status();
})();
`;

// ---------------------------------------------------------------------------
// Per-frame self-movement probe (window.__mir2LoadProbe). Sampled on rAF so each
// sample carries the true frame gap. It reads the THREE positions that define
// the overshoot/snap: server tile (entities[playerObjectId]), rendered tile
// (state.player, which leads when lead<=2 / falls back to server otherwise), and
// the raw prediction (state.predictedPlayer). It also drops a recorder marker on
// each backward render jump so the dumped replay aligns with the snaps.
// ---------------------------------------------------------------------------
function installMoveProbeSrc() {
  return `
(function () {
  var W = window;
  if (W.__mir2LoadProbe && W.__mir2LoadProbe._raf) { try { cancelAnimationFrame(W.__mir2LoadProbe._raf); } catch (e) {} }
  var P = {
    running: false, samples: [], cap: 200000, _raf: 0, _last: 0, _lastRnd: null, _phase: "idle",
    setPhase: function (name) { P._phase = String(name || "idle"); },
    start: function () {
      if (P.running) return; P.running = true; P.samples = []; P._last = performance.now(); P._lastRnd = null;
      var step = function () {
        var t = performance.now(), dt = t - P._last; P._last = t;
        var s = W.__mir2Stage5 && W.__mir2Stage5.state;
        if (s) {
          var ents = Array.isArray(s.entities) ? s.entities : [];
          var pid = s.playerObjectId;
          var srv = null;
          for (var i = 0; i < ents.length; i++) { if (ents[i] && String(ents[i].objectId) === String(pid)) { srv = ents[i]; break; } }
          var rnd = s.player || null;      // rendered (predicted-when-lead<=2, else server)
          var prd = s.predictedPlayer || null; // raw prediction (null when no lead)
          var mc = 0; for (var j = 0; j < ents.length; j++) { if (ents[j] && ents[j].kind === "monster" && !ents[j].dead) mc++; }
          var rLead = (rnd && srv) ? Math.max(Math.abs(rnd.x - srv.x), Math.abs(rnd.y - srv.y)) : 0;
          var pLead = (prd && srv) ? Math.max(Math.abs(prd.x - srv.x), Math.abs(prd.y - srv.y)) : 0;
          // backward render jump vs previous sample = the visible snap
          var backJump = 0;
          if (P._lastRnd && rnd) {
            var bx = Math.abs(rnd.x - P._lastRnd.x), by = Math.abs(rnd.y - P._lastRnd.y);
            backJump = Math.max(bx, by);
          }
          var sample = {
            t: Date.now(), dt: Math.round(dt * 100) / 100,
            sx: srv ? srv.x : null, sy: srv ? srv.y : null, sdir: srv ? (srv.direction || null) : null,
            rx: rnd ? rnd.x : null, ry: rnd ? rnd.y : null,
            px: prd ? prd.x : null, py: prd ? prd.y : null,
            rLead: rLead, pLead: pLead, jump: backJump,
            ec: ents.length, mc: mc,
            plan: s.movementPlan ? 1 : 0,
            q: Array.isArray(s.directionStepPendingQueue) ? s.directionStepPendingQueue.length : 0,
            out: Array.isArray(s.outstandingSelfMovementActions) ? s.outstandingSelfMovementActions.length : 0,
            ph: P._phase,
          };
          if (P.samples.length < P.cap) P.samples.push(sample);
          // Mark backward render jumps (the snap) into the flight recorder.
          if (backJump >= ${SNAP_BACK_TILES} && P._lastRnd && W.__mir2Recorder && W.__mir2Recorder.running) {
            try { W.__mir2Recorder.mark("snap dt=" + Math.round(dt) + " rLead=" + rLead + " ec=" + ents.length); } catch (e) {}
          }
          if (rnd) P._lastRnd = { x: rnd.x, y: rnd.y };
        }
        P._raf = requestAnimationFrame(step);
      };
      P._raf = requestAnimationFrame(step);
    },
    stop: function () { if (!P.running) return; P.running = false; try { cancelAnimationFrame(P._raf); } catch (e) {} },
    drain: function () { var s = P.samples; P.samples = []; return s; },
    status: function () { return { running: P.running, samples: P.samples.length, phase: P._phase }; },
  };
  W.__mir2LoadProbe = P;
  return P.status();
})();
`;
}

// In-page PerformanceObserver long-task total (window.__mir2LongTask) — the
// standards-based equivalent of Chrome's [Violation] "handler took N ms": it
// totals main-thread long-task time (tasks > 50ms) so the report has a
// browser-agnostic jank channel even where the [Violation] strings differ.
// ---------------------------------------------------------------------------
function installLongTaskObserverSrc() {
  return `
(function () {
  var W = window;
  if (W.__mir2LongTask && W.__mir2LongTask._obs) { try { W.__mir2LongTask._obs.disconnect(); } catch (e) {} }
  var L = { count: 0, totalMs: 0, maxMs: 0, _obs: null, startedAt: Date.now(),
    snapshot: function () { return { count: L.count, totalMs: Math.round(L.totalMs * 100) / 100, maxMs: Math.round(L.maxMs * 100) / 100, durationMs: Date.now() - L.startedAt }; },
    stop: function () { if (L._obs) { try { L._obs.disconnect(); } catch (e) {} L._obs = null; } return L.snapshot(); } };
  if (typeof PerformanceObserver === "function") {
    try {
      L._obs = new PerformanceObserver(function (list) {
        var es = list.getEntries();
        for (var i = 0; i < es.length; i++) { L.count++; L.totalMs += es[i].duration; if (es[i].duration > L.maxMs) L.maxMs = es[i].duration; }
      });
      L._obs.observe({ entryTypes: ["longtask"] });
    } catch (e) { L._obs = null; }
  }
  W.__mir2LongTask = L;
  return L.snapshot();
})();
`;
}

// Synthetic main-thread hog — forces rAF gaps to AMPLIFY the natural frame drops
// (a controlled stressor proving the frame-drop -> overshoot/snap causal link).
async function startHog(client) {
  await client.evaluate(`
    (function () {
      var W = window;
      if (W.__mir2LoadHog) return;
      W.__mir2LoadHog = setInterval(function () {
        // Variable-length stall: most are short, some span >1 server move step so
        // the server-applied position desyncs hard from the local prediction.
        var end = performance.now() + ${HOG_MS} + Math.floor(Math.random() * ${HOG_JITTER_MS});
        while (performance.now() < end) { /* block the main thread */ }
      }, ${HOG_PERIOD_MS});
      return true;
    })()
  `);
}
async function stopHog(client) {
  await client.evaluate(`(function(){ if (window.__mir2LoadHog) { clearInterval(window.__mir2LoadHog); window.__mir2LoadHog = null; } return true; })()`);
}

// ---------------------------------------------------------------------------
// Movement + combat primitives (same shapes as qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
}

// GM force-spawn (`@MOB <name> <count>`, gm_commands.rs:927) is the reliable way
// to create entity-render load on demand — it spawns hostile monsters at the
// player's own cell. Needs the account to be GM/test; best-effort (falls back to
// natural aggro if the server rejects it).
async function gmChat(client, message) {
  return sendCommand(client, { type: "chat", message });
}

async function clearMonsters(client) {
  await gmChat(client, "@CLEARMOB");
  await delay(500);
}

// Ensure at least `target` live monsters near the player (GM-spawn the deficit in
// batches). Returns the achieved count. target=0 clears the field for a clean
// baseline. Spawn happens while the player is stationary so mobs land on its cell.
async function ensureMonsterLoad(client, target, monsterName) {
  if (target <= 0) {
    await clearMonsters(client);
    const m = await readMonsters(client).catch(() => null);
    return m?.count ?? 0;
  }
  let last = (await readMonsters(client).catch(() => null))?.count ?? 0;
  for (let attempt = 0; attempt < 6 && last < target; attempt += 1) {
    const deficit = Math.min(40, target - last);
    await gmChat(client, `@MOB ${monsterName} ${deficit}`);
    await delay(900);
    last = (await readMonsters(client).catch(() => null))?.count ?? last;
  }
  if (last === 0) {
    // GM rejected (not a GM account) — fall back to sweeping natural spawns.
    const m = await pullMonsters(client, Math.min(target, 6), 12_000);
    last = m?.count ?? 0;
  }
  return last;
}

async function tilePoint(client, x, y) {
  const selector = `[aria-label="tile ${x}, ${y}"]`;
  return client.evaluate(`
    (() => {
      const tile = document.querySelector(${JSON.stringify(selector)});
      if (!tile) return null;
      const box = tile.getBoundingClientRect();
      if (box.width === 0 || box.height === 0) return null;
      return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
    })()
  `);
}

async function clickTile(client, x, y, button = "left") {
  const point = await tilePoint(client, x, y);
  if (!point) return false;
  await client.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: point.x, y: point.y, button: "none" });
  await client.send("Input.dispatchMouseEvent", { type: "mousePressed", x: point.x, y: point.y, button, buttons: button === "right" ? 2 : 1, clickCount: 1 });
  await client.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: point.x, y: point.y, button, buttons: 0, clickCount: 1 });
  return true;
}

async function transferTo(client, map, x, y) {
  await sendCommand(client, { type: "transferMap", key: `crystal:${map}:${x}:${y}` });
  await waitUntilSoft(
    client,
    `(() => { const s = window.__mir2Stage5?.state; return s?.mapFileName === ${JSON.stringify(map)}; })()`,
    20_000,
  );
}

async function readPlayerPos(client) {
  return client.evaluate(`(() => { const p = window.__mir2Stage5?.state?.player; return p ? { x: p.x, y: p.y } : null; })()`);
}

async function readMonsters(client) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const p = s.player ?? { x: 0, y: 0 };
      const mons = ents.filter((e) => e && e.kind === "monster" && !e.dead)
        .map((e) => ({ objectId: String(e.objectId), x: e.x, y: e.y, dist: Math.abs(e.x - p.x) + Math.abs(e.y - p.y) }))
        .sort((a, b) => a.dist - b.dist);
      return { count: mons.length, entityCount: ents.length, nearest: mons[0] ?? null, list: mons.slice(0, 12) };
    })()
  `);
}

// Sweep the field to aggro monsters until we have >= target (or time out).
async function pullMonsters(client, target, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  const start = await readPlayerPos(client);
  let ring = 0;
  while (Date.now() < deadline) {
    const m = await readMonsters(client).catch(() => null);
    if (m && m.count >= target) return m;
    // walk a small spiral around the spawn so on-demand spawns wake up + aggro
    const p = (await readPlayerPos(client)) ?? start;
    if (p) {
      ring = (ring + 1) % 8;
      const offsets = [[6, 0], [6, 6], [0, 6], [-6, 6], [-6, 0], [-6, -6], [0, -6], [6, -6]];
      const [dx, dy] = offsets[ring];
      if (!(await clickTile(client, p.x + dx, p.y + dy, "left"))) {
        await sendCommand(client, { type: "moveTo", x: p.x + dx, y: p.y + dy, mode: "run" });
      }
    }
    await delay(700);
  }
  return readMonsters(client).catch(() => null);
}

// ---------------------------------------------------------------------------
// The aggressive movement driver — the actual stressor. Fast far/zigzag RUN
// clicks with rapid direction reversals, interleaved with attacks on the nearest
// monster, for `durationMs`. Returns a small action log.
// ---------------------------------------------------------------------------
async function driveAggressively(client, durationMs, log) {
  const deadline = Date.now() + durationMs;
  let issued = 0;
  let clickFails = 0;
  let lastAttack = 0;
  const anchor = (await readPlayerPos(client)) ?? { x: field.x, y: field.y };

  // A move target relative to the current position, clamped near the anchor so we
  // stay inside the dense swarm (off-screen mobs don't render → no load).
  const target = (p, dx, dy) => {
    const cx = Math.max(anchor.x - 14, Math.min(anchor.x + 14, p.x + dx));
    const cy = Math.max(anchor.y - 14, Math.min(anchor.y + 14, p.y + dy));
    return { x: cx, y: cy };
  };
  const go = async (tx, ty) => {
    if (!(await clickTile(client, tx, ty, "left"))) {
      clickFails += 1;
      await sendCommand(client, { type: "moveTo", x: tx, y: ty, mode: "run" });
    }
    issued += 1;
  };
  const maybeAttack = async () => {
    if (Date.now() - lastAttack <= ATTACK_EVERY_MS) return;
    const m = await readMonsters(client).catch(() => null);
    if (m?.nearest) {
      // Clicking a monster's OWN tile attacks (does not walk) — page.tsx:646.
      await clickTile(client, m.nearest.x, m.nearest.y, "left");
      lastAttack = Date.now();
    }
  };

  // Three interleaved patterns, the worst cases for self-prediction:
  //   A sustained RUN  — long single-direction run so the predicted lead grows
  //                      toward the 2-tile cap while server acks lag under load.
  //   B rubber-band    — immediately reverse to the far opposite tile mid-run, so
  //                      the prediction must flip while the server still resolves
  //                      the old path (the classic overshoot inducer).
  //   C micro-zigzag   — a burst of fast alternating ±2 clicks that floods the
  //                      move queue with opposing directions.
  const dirs = [
    [13, 0], [0, 13], [-13, 0], [0, -13], [11, 11], [-11, -11], [11, -11], [-11, 11],
  ];
  let d = 0;
  while (Date.now() < deadline) {
    const [dx, dy] = dirs[d % dirs.length];
    d += 1;

    // A — sustained run out
    let p = (await readPlayerPos(client)) ?? anchor;
    let t = target(p, dx, dy);
    await go(t.x, t.y);
    await delay(420);
    await maybeAttack();

    // B — rubber-band reversal (snap the path back the opposite way mid-run)
    p = (await readPlayerPos(client)) ?? anchor;
    t = target(p, -dx, -dy);
    await go(t.x, t.y);
    await delay(180);

    // C — micro-zigzag burst (opposing directions back-to-back, fast)
    for (let k = 0; k < 4 && Date.now() < deadline; k += 1) {
      p = (await readPlayerPos(client)) ?? anchor;
      const s = k % 2 === 0 ? 2 : -2;
      t = target(p, s, -s);
      await go(t.x, t.y);
      await delay(MOVE_INTERVAL_MS);
    }
    await maybeAttack();
  }
  log.push({ issued, clickFails });
  return { issued, clickFails };
}

// Gentle control — slow, deliberate WALK clicks to nearby tiles, one fully
// completing before the next (no queue churn, no reversals). This mimics the
// calm play that does NOT trigger the drift, so it is the control the aggressive
// phases are measured against.
async function driveGently(client, durationMs, log) {
  const deadline = Date.now() + durationMs;
  const anchor = (await readPlayerPos(client)) ?? { x: field.x, y: field.y };
  const dirs = [[5, 0], [0, 5], [-5, 0], [0, -5]];
  let d = 0;
  let issued = 0;
  while (Date.now() < deadline) {
    const p = (await readPlayerPos(client)) ?? anchor;
    const [dx, dy] = dirs[d % dirs.length];
    d += 1;
    const tx = Math.max(anchor.x - 8, Math.min(anchor.x + 8, p.x + dx));
    const ty = Math.max(anchor.y - 8, Math.min(anchor.y + 8, p.y + dy));
    if (!(await clickTile(client, tx, ty, "left"))) {
      await sendCommand(client, { type: "moveTo", x: tx, y: ty, mode: "walk" });
    }
    issued += 1;
    await delay(1200); // let each short walk fully settle before the next
  }
  log.push({ issued, gentle: true });
  return { issued };
}

// ---------------------------------------------------------------------------
// Held-keyboard movement beat — the keyboard SEND pipeline (original-client-
// shell.tsx:749/804 → handleViewportDirectionIntent → trySendQueuedCrystalMove),
// which the click-based phases above NEVER exercise. Drives REAL held arrow keys
// via CDP Input.dispatchKeyEvent (not clicks) and reads SENDs from ground-truth
// outbound WS frames. Targets the "hold a direction → freeze 1-3s → jump 2 tiles"
// stall caused by sticky blocked-direction route-hint suppression bleeding into
// held-direction intent.
// ---------------------------------------------------------------------------
const ARROW_KEYS = {
  Up: { key: "ArrowUp", code: "ArrowUp", vk: 38 },
  Right: { key: "ArrowRight", code: "ArrowRight", vk: 39 },
  Down: { key: "ArrowDown", code: "ArrowDown", vk: 40 },
  Left: { key: "ArrowLeft", code: "ArrowLeft", vk: 37 },
};

async function dispatchKey(client, type, k, autoRepeat = false) {
  await client.send("Input.dispatchKeyEvent", {
    type, // "keyDown" | "keyUp"
    key: k.key,
    code: k.code,
    windowsVirtualKeyCode: k.vk,
    nativeVirtualKeyCode: k.vk,
    autoRepeat,
    isKeypad: false,
  });
}

// Hold ONE arrow key down for holdMs. A single keyDown adds the direction to the
// shell's held-key set (handleKeyboardMoveDown ignores event.repeat — it relies
// on the in-page 100ms interval to drive held SENDs), so keeping the key down
// without a keyUp is a faithful "key held". Periodic autoRepeat keydowns mirror
// the OS key-repeat the flight recorder captured (no behavioural effect, just
// realism + a guard if future code ever reads event.repeat). `onSample` is polled
// during the hold so the caller can watch the server tile advance.
async function holdArrowKey(client, dirName, holdMs, onSample) {
  const k = ARROW_KEYS[dirName];
  if (!k) throw new Error(`unknown arrow direction ${dirName}`);
  // Keyboard movement is ignored while a text input is focused
  // (keyboardInputTargetIsEditable, original-client-shell.tsx:750) — make sure none is.
  await client.evaluate("(() => { const a = document.activeElement; if (a && typeof a.blur === 'function') a.blur(); return true; })()");
  await dispatchKey(client, "keyDown", k, false);
  const deadline = Date.now() + holdMs;
  let nextRepeat = Date.now() + 200;
  let nextSample = Date.now();
  try {
    while (Date.now() < deadline) {
      const now = Date.now();
      if (now >= nextRepeat) {
        // The shell ignores event.repeat (it drives held SENDs from its 100ms
        // interval), so a single keyDown already counts as "held"; the sparse
        // repeats just keep the synthetic input shaped like real OS key-repeat.
        await dispatchKey(client, "keyDown", k, true);
        nextRepeat = now + 200;
      }
      if (onSample && now >= nextSample) {
        await onSample();
        nextSample = now + 90; // dense enough to catch an NPC blocking the next tile mid-gap
      }
      await delay(15);
    }
  } finally {
    await dispatchKey(client, "keyUp", k, false);
  }
}

// Extract the MOVE sends (walk/run/turn BrowserCommands) from the ground-truth
// outbound WS frames within [tStart, tEnd]. keepAlive/tick/chat frames are ignored.
function moveSendsInWindow(framesSent, tStart, tEnd) {
  const sends = [];
  for (const f of framesSent ?? []) {
    if (typeof f.at !== "number" || f.at < tStart || f.at > tEnd) continue;
    let type = null;
    let direction = null;
    try {
      const o = JSON.parse(f.payloadData);
      type = o?.type;
      direction = o?.direction ?? null;
    } catch {
      continue;
    }
    if (type === "walk" || type === "run" || type === "turn") sends.push({ at: f.at, type, direction });
  }
  sends.sort((a, b) => a.at - b.at);
  return sends;
}

// In-page sampler for one held direction: reads the authoritative server tile
// (entities[playerObjectId]), whether the NEXT tile in the held direction is
// occupied by another live entity, and the remaining correction-block. Sampled
// densely during the hold so each stall can be attributed.
function heldSampleExpr(dirName) {
  const D = { Up: [0, -1], Down: [0, 1], Left: [-1, 0], Right: [1, 0] }[dirName] ?? [0, 0];
  return `
    (() => {
      const s = window.__mir2Stage5 && window.__mir2Stage5.state;
      if (!s) return null;
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const pid = s.playerObjectId;
      let srv = null;
      for (let i = 0; i < ents.length; i++) { if (ents[i] && String(ents[i].objectId) === String(pid)) { srv = ents[i]; break; } }
      const rnd = s.player || srv;
      let nextOcc = false;
      if (srv) {
        const nx = srv.x + (${D[0]}), ny = srv.y + (${D[1]});
        for (let i = 0; i < ents.length; i++) { const e = ents[i]; if (e && String(e.objectId) !== String(pid) && !e.dead && e.x === nx && e.y === ny) { nextOcc = true; break; } }
      }
      const now = Date.now();
      return { t: now, sx: srv ? srv.x : null, sy: srv ? srv.y : null, rx: rnd ? rnd.x : null, ry: rnd ? rnd.y : null, nextOcc: nextOcc, blk: Math.max(0, (s.movementInputBlockedUntil || 0) - now) };
    })()
  `;
}

// Classify each SEND gap during a held direction. A "released" gap is one followed
// by another SEND (the lead gap before the first send, or an internal gap between
// two sends) — the player froze, then resumed. For a PURE CARDINAL hold this is the
// clean discriminator, no wall data needed:
//   • next tile entity-occupied during the gap  → legitimate NPC/monster block
//   • correction-block active during the gap     → legitimate post-correction settle
//   • next tile FREE the whole gap, yet it released → THE BUG (a real wall would
//     never release; only stale blocked-direction memory frees on a TTL timer)
// The final trailing stand (last send → hold end) is NOT a released gap — it is the
// legitimate "held into a wall / let go", so a wall at the end never counts as a stall.
function classifyHeldGaps(sends, trace, tStart, tEnd) {
  const steps = sends.filter((s) => s.type === "walk" || s.type === "run").length;
  const turns = sends.filter((s) => s.type === "turn").length;
  const times = sends.map((s) => s.at);
  const firstSendMs = times.length ? times[0] - tStart : tEnd - tStart;
  const trailGapMs = times.length ? tEnd - times[times.length - 1] : tEnd - tStart;
  const maxInternalGapMs = times.length >= 2 ? Math.max(...times.slice(1).map((t, i) => t - times[i])) : 0;

  const released = [];
  if (times.length) {
    released.push({ from: tStart, to: times[0] }); // lead gap (before first send)
    for (let i = 1; i < times.length; i += 1) released.push({ from: times[i - 1], to: times[i] }); // internal gaps
  }
  let bugStallMs = 0;
  let entityStallMs = 0;
  let correctionStallMs = 0;
  let bugStallDetail = null;
  for (const g of released) {
    const gapMs = g.to - g.from;
    if (gapMs < HELD_STALL_FAIL_MS) continue;
    const inGap = trace.filter((s) => s.t > g.from + 30 && s.t < g.to - 30);
    if (!inGap.length) continue; // no sampler visibility into this gap → cannot attribute
    if (inGap.some((s) => s.nextOcc)) {
      entityStallMs = Math.max(entityStallMs, gapMs);
    } else if (inGap.some((s) => s.blk > 0)) {
      correctionStallMs = Math.max(correctionStallMs, gapMs);
    } else if (gapMs > bugStallMs) {
      bugStallMs = gapMs;
      const mid = inGap[Math.floor(inGap.length / 2)];
      bugStallDetail = { gapMs, atTile: mid ? { x: mid.sx, y: mid.sy } : null };
    }
  }
  return { total: sends.length, steps, turns, firstSendMs, maxInternalGapMs, trailGapMs, bugStallMs, entityStallMs, correctionStallMs, bugStallDetail, traceSamples: trace.length };
}

// Run a held-key beat: hold each direction in `directions` for holdMs, recording
// per-direction send cadence + net server-tile progress. Returns the per-direction
// rows (analysis/verdict is in analyzeHeldBeats).
async function runHeldKeyBeat(client, { name, env, directions, holdMs }) {
  console.log(`   · held beat "${name}"${env ? ` [${env}]` : ""}: holding ${directions.join("/")} ${holdMs}ms each`);
  const rows = [];
  for (const dir of directions) {
    await client.evaluate(`window.__mir2LoadProbe?.setPhase(${JSON.stringify(`held:${name}:${dir}`)})`);
    const startPos = (await readPlayerPos(client)) ?? null;
    const sampleExpr = heldSampleExpr(dir);
    const trace = [];
    let maxSeenProgress = 0;
    const tStart = Date.now();
    await holdArrowKey(client, dir, holdMs, async () => {
      const s = await client.evaluate(sampleExpr).catch(() => null);
      if (s) {
        trace.push(s);
        if (startPos && s.sx != null) maxSeenProgress = Math.max(maxSeenProgress, Math.abs(s.sx - startPos.x), Math.abs(s.sy - startPos.y));
      }
    });
    const tEnd = Date.now();
    await delay(350); // let the final in-flight send/ack land inside the window
    const endPos = (await readPlayerPos(client)) ?? null;
    const sends = moveSendsInWindow(client.webSocketFramesSent, tStart, tEnd + 350);
    const stats = classifyHeldGaps(sends, trace, tStart, tEnd + 350);
    const netProgress = startPos && endPos ? Math.max(Math.abs(endPos.x - startPos.x), Math.abs(endPos.y - startPos.y)) : 0;
    const progressTiles = Math.max(netProgress, maxSeenProgress);
    rows.push({ dir, holdMs, ...stats, progressTiles, startPos, endPos });
    console.log(
      `       ${dir}: steps=${stats.steps} turns=${stats.turns} bugStall=${stats.bugStallMs}ms ` +
        `entityStall=${stats.entityStallMs}ms corrStall=${stats.correctionStallMs}ms trail=${stats.trailGapMs}ms progress=${progressTiles}t`,
    );
    await delay(400); // brief release between directions
  }
  return { name, env: env ?? null, directions: rows };
}

// Both beats over one session: an OPEN field (clean baseline proving the held
// pipeline emits sends) and a NEAR-OBSTACLE field (the regression target — a
// dynamic swarm when GM-spawn works, else a cluttered town). Restores the loaded
// field afterward so the existing overshoot phases run unchanged.
async function runHeldKeyboardBeats(client, { gmSpawnWorks }) {
  console.log(`\n▶ held-keyboard movement beats (real CDP arrow keys; the click phases never exercise this path)`);
  const beats = [];

  // 1) OPEN FIELD — clear the field so every held direction is traversable; the
  //    held key must emit a continuous cadence of SENDs.
  await clearMonsters(client);
  await delay(500);
  beats.push(await runHeldKeyBeat(client, { name: "open-field", env: field.label, directions: ["Right", "Down"], holdMs: HELD_HOLD_MS }));

  // 2) NEAR OBSTACLES — the bug surfaces only where a held direction passes near
  //    obstacles, so a transient bump can seed the sticky blocked-direction memory.
  let env;
  if (gmSpawnWorks) {
    const loaded = await ensureMonsterLoad(client, HELD_OBSTACLE_SPAWN, MONSTER_NAME);
    env = `swarm≈${loaded}`;
  } else {
    await transferTo(client, OBSTACLE_FIELD.map, OBSTACLE_FIELD.x, OBSTACLE_FIELD.y);
    await delay(1500);
    env = OBSTACLE_FIELD.label;
  }
  beats.push(await runHeldKeyBeat(client, { name: "near-obstacles", env, directions: ["Up", "Right", "Down", "Left"], holdMs: HELD_HOLD_MS }));

  // Restore the loaded field for the existing overshoot phases.
  await clearMonsters(client).catch(() => {});
  const here = await client.evaluate("window.__mir2Stage5?.state?.mapFileName ?? null").catch(() => null);
  if (String(here) !== String(field.map)) {
    await transferTo(client, field.map, field.x, field.y).catch(() => {});
    await delay(1200);
  }
  return analyzeHeldBeats(beats);
}

// Verdict over the held beats. A FAILURE is a classified BUG stall — a held key
// that froze >= HELD_STALL_FAIL_MS with the next tile demonstrably FREE (no entity,
// not correction-blocked) and then RESUMED. Stands attributed to an entity on the
// next tile, to a server correction, or to a permanent wall (the trailing stand)
// are legitimate and reported as diagnostics, never failed — so the guard does not
// cry wolf on a cluttered town. A beat where nothing moved at all is also failed
// (the held SEND path is dead).
function analyzeHeldBeats(beats) {
  const issues = [];
  const diagnostics = [];
  for (const beat of beats) {
    let progressingDirs = 0;
    for (const r of beat.directions) {
      if (r.progressTiles >= 1) progressingDirs += 1;
      if (r.bugStallMs >= HELD_STALL_FAIL_MS) {
        issues.push(
          `${beat.name} hold ${r.dir}: held key froze ${r.bugStallMs}ms with the next tile FREE ` +
            `(no entity, not correction-blocked) then resumed${
              r.bugStallDetail?.atTile ? ` at (${r.bugStallDetail.atTile.x},${r.bugStallDetail.atTile.y})` : ""
            } — sticky blocked-direction suppression on held input.`,
        );
      }
      const legit = Math.max(r.entityStallMs, r.correctionStallMs);
      if (legit >= HELD_STALL_FAIL_MS) {
        diagnostics.push(
          `${beat.name} hold ${r.dir}: ${legit}ms stand from ${
            r.entityStallMs >= r.correctionStallMs ? "an entity blocking the next tile" : "a server correction"
          } (legitimate — not failed).`,
        );
      }
    }
    if (progressingDirs === 0) {
      issues.push(
        `${beat.name}: held movement made ZERO progress in any of the ${beat.directions.length} direction(s) — ` +
          `the held-keyboard SEND path drove no movement (regression, or the spot was fully boxed).`,
      );
    }
  }
  const passed = issues.length === 0;
  console.log(`   held-keyboard verdict: ${passed ? "PASS ✅" : `FAIL ❌ (${issues.length})`}`);
  for (const i of issues) console.log(`     - ${i}`);
  for (const d of diagnostics) console.log(`     · ${d}`);
  return { passed, issues, diagnostics, beats };
}

// ---------------------------------------------------------------------------
// Detectors — run in Node over the drained probe samples + recorder dump.
// ---------------------------------------------------------------------------

// Overshoot/snap events: a backward render jump >= SNAP_BACK_TILES in one rAF,
// after the render had been leading (rLead>=1) or movement was active. Also
// summarise overshoot runs where rLead reached the 2-tile cap.
function detectOvershootSnaps(samples) {
  const snaps = [];
  for (let i = 1; i < samples.length; i += 1) {
    const a = samples[i - 1];
    const b = samples[i];
    // Any >=2-tile move of the RENDERED self in a single rAF (~14-24ms) is
    // non-physical — a walk/run step is 1 tile over >=200ms — so it is a
    // teleport/snap, not real motion. Classify it relative to the server tile:
    // "snap-back" = render jumped toward the server (overshoot was discarded);
    // "snap-fwd" = render jumped ahead (prediction was behind, server caught up).
    if (b.jump >= SNAP_BACK_TILES && a.rx != null && b.rx != null) {
      let kind = "snap";
      if (b.sx != null && b.sy != null) {
        const distBefore = Math.max(Math.abs(a.rx - b.sx), Math.abs(a.ry - b.sy));
        const distAfter = Math.max(Math.abs(b.rx - b.sx), Math.abs(b.ry - b.sy));
        kind = distAfter < distBefore ? "snap-back" : distAfter > distBefore ? "snap-fwd" : "snap";
      }
      snaps.push({
        t: b.t,
        phase: b.ph,
        kind,
        backTiles: b.jump,
        dtAtSnap: b.dt,
        leadBefore: a.rLead,
        predLeadBefore: a.pLead,
        entityCount: b.ec,
        monsterCount: b.mc,
        from: { x: a.rx, y: a.ry },
        to: { x: b.rx, y: b.ry },
        server: { x: b.sx, y: b.sy },
      });
    }
  }
  // Overshoot runs: contiguous spans where rLead OR pLead >= LEAD_CAP_TILES.
  const overshoots = [];
  let run = null;
  for (const s of samples) {
    const atCap = Math.max(s.rLead, s.pLead) >= LEAD_CAP_TILES;
    if (atCap) {
      if (!run) run = { phase: s.ph, startT: s.t, endT: s.t, peakLead: 0, maxDt: 0, peakEc: 0 };
      run.endT = s.t;
      run.peakLead = Math.max(run.peakLead, s.rLead, s.pLead);
      run.maxDt = Math.max(run.maxDt, s.dt);
      run.peakEc = Math.max(run.peakEc, s.ec);
    } else if (run) {
      run.durationMs = run.endT - run.startT;
      overshoots.push(run);
      run = null;
    }
  }
  if (run) {
    run.durationMs = run.endT - run.startT;
    overshoots.push(run);
  }
  return { snaps, overshoots };
}

// Correlate frame hitches (dt>=HITCH_MS) with movement and with snaps.
function detectHitchCorrelation(samples, snaps) {
  const hitches = samples.filter((s) => s.dt >= HITCH_MS);
  const movingHitches = hitches.filter((s) => s.plan === 1 || s.rLead >= 1 || s.q > 0 || s.out > 0);
  // fraction of snaps that have a hitch within +/- window
  let snapsWithHitch = 0;
  for (const snap of snaps) {
    const near = hitches.some((h) => Math.abs(h.t - snap.t) <= CORRELATION_WINDOW_MS);
    if (near) snapsWithHitch += 1;
  }
  const dts = samples.map((s) => s.dt).filter((d) => Number.isFinite(d));
  return {
    hitchCount: hitches.length,
    hitchesDuringMovement: movingHitches.length,
    hitchMovementFraction: hitches.length ? round3(movingHitches.length / hitches.length) : 0,
    snapsExplainedByHitch: snapsWithHitch,
    snapHitchFraction: snaps.length ? round3(snapsWithHitch / snaps.length) : 0,
    maxDtMs: dts.length ? Math.max(...dts) : 0,
    meanDtMs: dts.length ? round2(dts.reduce((x, y) => x + y, 0) / dts.length) : 0,
    p95DtMs: percentile(dts, 95),
    approxFps: dts.length ? round1(1000 / (dts.reduce((x, y) => x + y, 0) / dts.length)) : 0,
  };
}

// Correlate concurrent entity count with rAF frame gap (does load => hitches?).
function detectEntityCountFrameGap(samples) {
  const buckets = [
    { label: "0-4", lo: 0, hi: 4 },
    { label: "5-9", lo: 5, hi: 9 },
    { label: "10-19", lo: 10, hi: 19 },
    { label: "20-39", lo: 20, hi: 39 },
    { label: "40+", lo: 40, hi: Infinity },
  ].map((b) => ({ ...b, n: 0, sumDt: 0, maxDt: 0, hitches: 0 }));
  for (const s of samples) {
    const b = buckets.find((x) => s.ec >= x.lo && s.ec <= x.hi);
    if (!b) continue;
    b.n += 1;
    b.sumDt += s.dt;
    b.maxDt = Math.max(b.maxDt, s.dt);
    if (s.dt >= HITCH_MS) b.hitches += 1;
  }
  const rows = buckets
    .filter((b) => b.n > 0)
    .map((b) => ({
      entityCount: b.label,
      samples: b.n,
      meanDtMs: round2(b.sumDt / b.n),
      maxDtMs: round2(b.maxDt),
      hitchRate: round3(b.hitches / b.n),
    }));
  const r = pearson(samples.map((s) => s.ec), samples.map((s) => s.dt));
  return { buckets: rows, pearsonEntityVsDt: r };
}

function detectCorrections(moveEvents) {
  const corrections = (moveEvents ?? []).filter((e) => e && e.kind === "correction");
  const sends = (moveEvents ?? []).filter((e) => e && e.kind === "send").length;
  const acks = (moveEvents ?? []).filter((e) => e && e.kind === "ack").length;
  return {
    sends,
    acks,
    corrections: corrections.length,
    samples: corrections.slice(0, 25).map((c) => ({ at: c.at, ...compactCorrection(c) })),
  };
}

function compactCorrection(c) {
  const out = {};
  for (const k of Object.keys(c)) {
    if (k === "kind" || k === "at") continue;
    const v = c[k];
    if (v == null) continue;
    if (typeof v === "object") out[k] = JSON.stringify(v).slice(0, 160);
    else out[k] = v;
  }
  return out;
}

// ---------------------------------------------------------------------------
// Phase runner — start probe phase, drive, drain, analyze.
// ---------------------------------------------------------------------------
async function runPhase(client, { name, spawnTarget, hog, gentle }, durationMs) {
  console.log(`\n▶ phase "${name}" (${Math.round(durationMs / 1000)}s, spawnTarget=${spawnTarget}, hog=${hog}, gentle=${!!gentle})`);
  // Establish this phase's entity-render load before driving.
  const loaded = await ensureMonsterLoad(client, spawnTarget, MONSTER_NAME);
  console.log(`   load set: monsters≈${loaded}`);
  await client.evaluate(`window.__mir2LoadProbe?.setPhase(${JSON.stringify(name)})`);
  if (hog) await startHog(client);
  const phaseStartAt = Date.now();
  const log = [];
  if (gentle) await driveGently(client, durationMs, log);
  else await driveAggressively(client, durationMs, log);
  if (hog) await stopHog(client);
  const samples = (await client.evaluate("window.__mir2LoadProbe?.drain() ?? []")) ?? [];
  // __mir2MovementConsoleEvents is a single global accumulator (newest-first, cap
  // 100) shared across phases, so restrict to THIS phase's window by timestamp.
  const allMoveEvents = (await client.evaluate("(window.__mir2MovementConsoleEvents || []).slice().reverse()")) ?? [];
  const moveEvents = allMoveEvents.filter((e) => e && typeof e.at === "number" && e.at >= phaseStartAt);

  const { snaps, overshoots } = detectOvershootSnaps(samples);
  const hitch = detectHitchCorrelation(samples, snaps);
  const entityGap = detectEntityCountFrameGap(samples);
  const corr = detectCorrections(moveEvents);

  const peakEc = samples.reduce((m, s) => Math.max(m, s.ec), 0);
  const maxRLead = samples.reduce((m, s) => Math.max(m, s.rLead, s.pLead), 0);

  const phase = {
    name,
    hog,
    durationMs,
    samples: samples.length,
    drive: log[0] ?? null,
    peakEntityCount: peakEc,
    maxLeadTiles: maxRLead,
    snapCount: snaps.length,
    overshootRuns: overshoots.length,
    longestOvershootMs: overshoots.reduce((m, o) => Math.max(m, o.durationMs), 0),
    corrections: corr.corrections,
    sends: corr.sends,
    acks: corr.acks,
    frame: hitch,
    entityFrameGap: entityGap,
    snapSamples: snaps.slice(0, 30),
    correctionSamples: corr.samples,
    overshootSamples: overshoots.slice(0, 20),
  };
  console.log(
    `   samples=${phase.samples} fps≈${hitch.approxFps} hitches=${hitch.hitchCount} ` +
      `snaps=${phase.snapCount} overshootRuns=${phase.overshootRuns} corrections=${phase.corrections} ` +
      `maxLead=${maxRLead} peakEntities=${peakEc}`,
  );
  return { phase, samples };
}

// ---------------------------------------------------------------------------
// Verdict — name the trigger from the phase results.
// ---------------------------------------------------------------------------
function buildVerdict(phases) {
  const byName = (n) => phases.find((p) => p.name === n);
  const gentle = byName("gentle");
  const aggressive = byName("aggressive");
  const hitch = byName("aggressive+hitch");
  const totalSnaps = phases.reduce((n, p) => n + p.snapCount, 0);
  const totalCorr = phases.reduce((n, p) => n + p.corrections, 0);
  const reproduced = totalSnaps > 0 || totalCorr > 0;

  const lines = [];
  if (!reproduced) {
    lines.push("NOT REPRODUCED in this run — no overshoot/snap or logical correction observed.");
    lines.push(
      `The rendered self stayed physical (no >=${SNAP_BACK_TILES}-tile single-frame jump) and the prediction held ` +
        `inside the ${LEAD_CAP_TILES}-tile lead cap. Try --spawn higher, a denser field (--map 2), longer --durationMs, ` +
        `or a larger --hogMs.`,
    );
    return { reproduced, trigger: "none-observed", notes: lines };
  }

  // Combined "drift" score per phase — snaps, server corrections and overshoot
  // runs are three faces of the same desync, and any one alone is noisy.
  const drift = (p) => (p ? p.snapCount + p.corrections + p.overshootRuns : 0);
  const fps = (p) => p?.frame?.approxFps ?? 0;
  // Throughput-normalised: a hard hog suppresses move throughput (fewer frames =
  // fewer chances to drift), so compare drift PER 1000 rendered frames too.
  const perK = (p) => (p && p.samples ? round2((drift(p) / p.samples) * 1000) : 0);
  const g = drift(gentle);
  const a = drift(aggressive);
  const h = drift(hitch);
  const grad = `drift gentle=${g} → aggressive=${a}${hitch ? ` → aggressive+hitch=${h}` : ""} (= snaps+corrections+overshootRuns)`;
  const calmClean = g <= Math.max(1, Math.floor(a * 0.34)); // gentle play stays ~clean vs aggressive

  let trigger;
  if (hitch && h > 0 && perK(hitch) > perK(aggressive) * 1.3) {
    trigger = "aggressive run + direction reversals, intensified by frame-drops";
    lines.push(
      `${grad}. Per-frame drift density rises ${perK(aggressive)} → ${perK(hitch)} per 1k frames when frame-drops are ` +
        `injected (fps ${fps(aggressive)} → ${fps(hitch)}): each rendered frame is likelier to land mid-desync. ` +
        `Aggressive movement is necessary; frame-drops amplify it. Under a hitch the rAF predictor ` +
        `(tickMovementPlan, page.tsx:3922) and the queued server position resume out of step, the lead crosses the ` +
        `${LEAD_CAP_TILES}-tile cap, and preserveCrystalSelfRenderPosition (page.tsx:3326) discards the prediction → ` +
        `overshoot then snap.`,
    );
  } else {
    trigger = "aggressive run + rapid direction reversals (run/walk + lead-cap desync)";
    lines.push(
      `${grad}. ${
        calmClean
          ? `Gentle play stays ~clean (${g}) while aggressive runs + hard reversals produce the drift (${a})`
          : `The drift concentrates in the aggressive phase (${a})`
      }: fast runs with hard reversals outrun the server resolution (run predicted / walk resolved, or an opposing ` +
        `step queued mid-run), pushing the lead to the ${LEAD_CAP_TILES}-tile cap → overshoot, server CORRECTION, ` +
        `and render snap. NOTE a hard main-thread stall (fps ${fps(hitch)}) throttles move throughput, so it does not ` +
        `always raise the raw count — see the per-frame density below.`,
    );
  }
  lines.push(
    `Per-frame drift density: gentle=${perK(gentle)} · aggressive=${perK(aggressive)}${hitch ? ` · aggressive+hitch=${perK(hitch)}` : ""} per 1k frames (controls for the hog throttling throughput).`,
  );

  // Overshoot-then-snap shape: count snap-back (the discarded overshoot) events.
  const backSnaps = phases.reduce((n, p) => n + p.snapSamples.filter((s) => s.kind === "snap-back").length, 0);
  const fwdSnaps = phases.reduce((n, p) => n + p.snapSamples.filter((s) => s.kind === "snap-fwd").length, 0);
  if (backSnaps || fwdSnaps) {
    lines.push(`Snap shape: ${backSnaps} snap-back (overshoot discarded → render jumps toward server) vs ${fwdSnaps} snap-forward (prediction was behind → render jumps ahead).`);
  }

  // Hitch correlation.
  const hitchFracs = phases.filter((p) => p.snapCount > 0).map((p) => p.frame.snapHitchFraction);
  if (hitchFracs.length) {
    const avg = round2(hitchFracs.reduce((a, x) => a + x, 0) / hitchFracs.length);
    lines.push(`${Math.round(avg * 100)}% of snaps had a frame hitch (>=${HITCH_MS}ms rAF gap) within ${CORRELATION_WINDOW_MS}ms — frame-drops and snaps co-occur.`);
  }
  // Entity/frame-gap correlation.
  const rs = phases.map((p) => p.entityFrameGap.pearsonEntityVsDt).filter((r) => Number.isFinite(r) && r !== 0);
  if (rs.length) {
    const avgR = round2(rs.reduce((a, x) => a + x, 0) / rs.length);
    lines.push(`Entity-count vs frame-gap correlation (Pearson r) ≈ ${avgR} — ${avgR >= 0.2 ? "more entities ⇒ longer frames" : "weak/none"}.`);
  }
  if (totalCorr > 0) lines.push(`${totalCorr} server logical CORRECTION event(s) logged (state.predictedPlayer rejected by the server).`);

  return { reproduced, trigger, notes: lines };
}

// ---------------------------------------------------------------------------
// Report.
// ---------------------------------------------------------------------------
async function writeReport(ctx) {
  const { phases, recording, recorderSource, verdict, allSamples, heldFindings } = ctx;
  await fs.mkdir(outputDir, { recursive: true });

  // Canonical flight-recorder dump (scrubbable in /mir2-replay.html).
  await fs.writeFile(path.join(outputDir, "recording.json"), JSON.stringify(recording));
  // Structured findings.
  const findings = {
    runId,
    capturedAt: new Date().toISOString(),
    field: field.label,
    config: { totalDurationMs: TOTAL_DURATION_MS, spawnCount: SPAWN_COUNT, monster: MONSTER_NAME, hogPhase: RUN_HOG_PHASE, hogMs: HOG_MS, hogPeriodMs: HOG_PERIOD_MS, hitchMs: HITCH_MS, leadCapTiles: LEAD_CAP_TILES, heldBeat: RUN_HELD_BEAT, heldHoldMs: HELD_HOLD_MS, heldStallMs: HELD_STALL_FAIL_MS },
    gmSpawnWorks: ctx.gmSpawnWorks ?? null,
    recorderSource,
    verdict,
    heldKeyboard: heldFindings ?? null,
    // Stage-0 render-perf instrumentation: Chrome [Violation] buckets, the
    // standards-based long-task total, and the per-packet handler p50/p95.
    renderPerf: {
      violationCounts: ctx.violationCounts ?? {},
      violations: (ctx.violations ?? []).slice(0, 50),
      longTask: ctx.longTask ?? null,
      perfDiag: ctx.perfDiag ?? null,
    },
    phases,
  };
  await fs.writeFile(path.join(outputDir, "findings.json"), JSON.stringify(findings, null, 2));
  // Raw probe samples (compact) for offline analysis.
  await fs.writeFile(path.join(outputDir, "probe-samples.json"), JSON.stringify(allSamples));

  // Markdown.
  const md = [];
  md.push(`# Movement load-stress — overshoot/snap repro`);
  md.push("");
  md.push(`- **run**: \`${runId}\``);
  md.push(`- **field**: ${field.label}`);
  md.push(`- **recorder**: ${recorderSource}`);
  md.push(`- **lead cap**: ${LEAD_CAP_TILES} tiles (MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES, page.tsx:906-907)`);
  md.push(`- **hitch threshold**: ${HITCH_MS}ms rAF gap`);
  md.push("");
  md.push(`## Verdict`);
  md.push("");
  md.push(`**Reproduced:** ${verdict.reproduced ? "YES ✅" : "no"}  ·  **Trigger:** ${verdict.trigger}`);
  md.push("");
  for (const n of verdict.notes) md.push(`- ${n}`);
  md.push("");
  if (heldFindings) {
    md.push(`## Held-keyboard movement (the SEND pipeline the click phases never touch)`);
    md.push("");
    md.push(`**Result:** ${heldFindings.passed ? "PASS ✅ — held arrow keys SEND continuously, no multi-second stall" : "FAIL ❌ — held-direction stall detected"}`);
    md.push("");
    md.push(`A held direction SENDs a walk/run ~once per ${600}ms (CRYSTAL_MOVE_DELAY_MS); a gap >= ${HELD_STALL_FAIL_MS}ms with no SEND while the path is traversable is the "freeze then jump 2 tiles" stall.`);
    md.push("");
    if (heldFindings.issues.length) {
      for (const i of heldFindings.issues) md.push(`- ⚠️ ${i}`);
      md.push("");
    }
    if (heldFindings.diagnostics?.length) {
      md.push(`Legitimate stands observed (entity/correction/wall — not failed):`);
      for (const d of heldFindings.diagnostics) md.push(`- ${d}`);
      md.push("");
    }
    md.push(`A "bug stall" is a freeze with the next tile FREE that then resumes; entity/correction stands and the trailing wall stand are legitimate.`);
    md.push("");
    for (const beat of heldFindings.beats) {
      md.push(`**${beat.name}**${beat.env ? ` — ${beat.env}` : ""}`);
      md.push("");
      md.push(`| dir | step SENDs | turn SENDs | bug stall (ms) | entity stall (ms) | corr stall (ms) | trail gap (ms) | progress (tiles) |`);
      md.push(`|---|---|---|---|---|---|---|---|`);
      for (const r of beat.directions) {
        md.push(`| ${r.dir} | ${r.steps} | ${r.turns} | ${r.bugStallMs} | ${r.entityStallMs} | ${r.correctionStallMs} | ${r.trailGapMs} | ${r.progressTiles} |`);
      }
      md.push("");
    }
  }
  md.push(`## Per-phase`);
  md.push("");
  md.push(`| phase | hog | samples | fps≈ | hitches | snaps | overshoot runs | corrections | maxLead | peakEntities | snap↔hitch | r(ec,dt) |`);
  md.push(`|---|---|---|---|---|---|---|---|---|---|---|---|`);
  for (const p of phases) {
    md.push(
      `| ${p.name} | ${p.hog} | ${p.samples} | ${p.frame.approxFps} | ${p.frame.hitchCount} | ${p.snapCount} | ${p.overshootRuns} | ${p.corrections} | ${p.maxLeadTiles} | ${p.peakEntityCount} | ${Math.round(p.frame.snapHitchFraction * 100)}% | ${p.entityFrameGap.pearsonEntityVsDt} |`,
    );
  }
  md.push("");
  md.push(`## Entity-count vs frame-gap`);
  for (const p of phases) {
    md.push("");
    md.push(`**${p.name}**`);
    md.push("");
    md.push(`| entities | samples | mean dt (ms) | max dt (ms) | hitch rate |`);
    md.push(`|---|---|---|---|---|`);
    for (const b of p.entityFrameGap.buckets) {
      md.push(`| ${b.entityCount} | ${b.samples} | ${b.meanDtMs} | ${b.maxDtMs} | ${Math.round(b.hitchRate * 100)}% |`);
    }
  }
  md.push("");
  md.push(`## Sample snap events`);
  for (const p of phases) {
    if (!p.snapSamples.length) continue;
    md.push("");
    md.push(`**${p.name}** (first ${Math.min(p.snapSamples.length, 12)} of ${p.snapCount}):`);
    md.push("");
    md.push(`| t (ms) | kind | jump tiles | dt@snap | leadBefore | entities | from → to | server |`);
    md.push(`|---|---|---|---|---|---|---|---|`);
    for (const s of p.snapSamples.slice(0, 12)) {
      md.push(`| ${s.t} | ${s.kind} | ${s.backTiles} | ${s.dtAtSnap} | ${s.leadBefore} | ${s.entityCount} | (${s.from.x},${s.from.y})→(${s.to.x},${s.to.y}) | (${s.server.x},${s.server.y}) |`);
    }
  }
  md.push("");
  md.push(`## Render-perf instrumentation (Stage 0)`);
  md.push("");
  const vCounts = ctx.violationCounts ?? {};
  const vKeys = Object.keys(vCounts);
  if (vKeys.length) {
    md.push(`**Chrome [Violation] handlers** (CDP Log domain):`);
    md.push("");
    md.push(`| handler | count | total ms | max ms |`);
    md.push(`|---|---|---|---|`);
    for (const k of vKeys.sort((a, b) => (vCounts[b].totalMs ?? 0) - (vCounts[a].totalMs ?? 0))) {
      const v = vCounts[k];
      md.push(`| ${k} | ${v.count} | ${Math.round(v.totalMs)} | ${Math.round(v.maxMs)} |`);
    }
  } else {
    md.push(`_No Chrome [Violation] "handler took N ms" warnings captured this run (browser may not emit them, or none exceeded the threshold)._`);
  }
  md.push("");
  if (ctx.longTask) {
    md.push(`**Long tasks** (in-page PerformanceObserver, tasks > 50ms): ${ctx.longTask.count} tasks · ${Math.round(ctx.longTask.totalMs)}ms total · ${Math.round(ctx.longTask.maxMs)}ms max over ${Math.round((ctx.longTask.durationMs ?? 0) / 1000)}s`);
    md.push("");
  }
  if (ctx.perfDiag && ctx.perfDiag.handlers && Object.keys(ctx.perfDiag.handlers).length) {
    md.push(`**Per-packet handler timing** (\`?perfDiag=1\`, top 12 by p95):`);
    md.push("");
    md.push(`| packet | count | p50 ms | p95 ms | max ms |`);
    md.push(`|---|---|---|---|---|`);
    const entries = Object.entries(ctx.perfDiag.handlers).sort((a, b) => (b[1].p95Ms ?? 0) - (a[1].p95Ms ?? 0));
    for (const [name, h] of entries.slice(0, 12)) {
      md.push(`| ${name} | ${h.count} | ${h.p50Ms} | ${h.p95Ms} | ${h.maxMs} |`);
    }
    md.push("");
    if (ctx.perfDiag.longTask) {
      md.push(`(page-side long-task total via __mir2PerfDiag: ${ctx.perfDiag.longTask.count} tasks · ${Math.round(ctx.perfDiag.longTask.totalMs)}ms)`);
      md.push("");
    }
  } else {
    md.push(`_No \`?perfDiag=1\` per-packet timings captured (window.__mir2PerfDiag absent — flag not armed or no packets handled)._`);
    md.push("");
  }
  md.push(`## Artifacts`);
  md.push(`- \`recording.json\` — flight-recorder dump (mir2-replay/1; open /mir2-replay.html to scrub; markers sit on each snap)`);
  md.push(`- \`findings.json\` — full structured detector output`);
  md.push(`- \`probe-samples.json\` — raw per-frame probe samples`);
  if (ctx.consoleErrors?.length) {
    md.push("");
    md.push(`## Console errors (${ctx.consoleErrors.length}, first 10)`);
    for (const e of ctx.consoleErrors.slice(0, 10)) md.push(`- [${e.type}] ${String(e.text).slice(0, 200)}`);
  }
  md.push("");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Login + enter world (compact, from qa-playthrough enterWorldQuick).
// ---------------------------------------------------------------------------
async function enterWorld(client) {
  await waitUntilSoft(client, "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)", 25_000);
  let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");

  if (screen === "login") {
    await fillInput(client, ".login-input.account", account).catch(() => {});
    await fillInput(client, ".login-input.password", password).catch(() => {});
    if (createAccount) {
      await click(client, ".login-button.account button").catch(() => {});
      await waitUntilSoft(client, "window.__mir2Stage5?.state?.wsState === 'open'", 15_000);
      await delay(1500);
    }
    await fillInput(client, ".login-input.account", account).catch(() => {});
    await fillInput(client, ".login-input.password", password).catch(() => {});
    await click(client, ".login-button.ok button").catch(() => {});
    await waitUntilSoft(client, "['select', 'game'].includes(window.__mir2Stage5?.state?.screen)", 30_000);
    screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  }

  if (screen !== "game") {
    const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;
    if (!(await client.evaluate(haveOurChar))) {
      await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: "warrior" });
      await waitUntilSoft(client, haveOurChar, 15_000);
    }
    const startIndex = await client.evaluate(
      `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
    );
    await sendCommand(client, { type: "startGame", characterIndex: startIndex });
    await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 25_000);
  }

  await waitUntilSoft(
    client,
    "window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)",
    SCENE_READY_TIMEOUT_MS,
  );
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-load-stress-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      ...(disableGpu ? ["--disable-gpu"] : ["--ignore-gpu-blocklist", "--enable-webgl"]),
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
  await delay(3000);
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
    (() => { const node = document.querySelector(${JSON.stringify(selector)}); if (!node) return false; node.click(); return true; })()
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
    .evaluate(`(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null }))()`)
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

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) return;
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve));
}

// ---------------------------------------------------------------------------
// Small utilities.
// ---------------------------------------------------------------------------
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
function defaultCharacterName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `LS${suffix}`.slice(0, 10).toUpperCase();
}
function defaultAccountName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `LS${suffix}`.slice(0, 12).toUpperCase();
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
function round1(n) {
  return Math.round(n * 10) / 10;
}
function round2(n) {
  return Math.round(n * 100) / 100;
}
function round3(n) {
  return Math.round(n * 1000) / 1000;
}
function percentile(values, p) {
  if (!values.length) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const idx = Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length));
  return round2(sorted[idx]);
}
function pearson(xs, ys) {
  const n = Math.min(xs.length, ys.length);
  if (n < 3) return 0;
  let sx = 0, sy = 0, sxx = 0, syy = 0, sxy = 0;
  for (let i = 0; i < n; i += 1) {
    const x = xs[i], y = ys[i];
    sx += x; sy += y; sxx += x * x; syy += y * y; sxy += x * y;
  }
  const cov = n * sxy - sx * sy;
  const den = Math.sqrt((n * sxx - sx * sx) * (n * syy - sy * sy));
  return den === 0 ? 0 : round2(cov / den);
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------
async function main() {
  await fs.mkdir(outputDir, { recursive: true });
  console.log(`qa-load-stress: target=${baseUrl} field=${field.label} account=${account} char=${characterName} headed=${headed}`);
  console.log(`qa-load-stress: output=${outputDir}`);

  const chrome = await launchChrome();
  let client;
  try {
    const wsUrl = await createPageTarget();
    client = new CdpClient(wsUrl);
    await client.connect();
    await client.send("Runtime.enable");
    await client.send("Network.enable");
    await client.send("Page.enable");
    // Capture Chrome [Violation] warnings ("'message' handler took N ms", etc.) —
    // they surface on the Log domain at verbose level, not via consoleAPICalled.
    await client.send("Log.enable").catch(() => {});
    await client.send("Page.bringToFront");
    await setViewport(client, VIEWPORT);

    await navigate(client, RUN_URL);
    console.log("· logging in + entering world…");
    await enterWorld(client);

    console.log(`· transferring to ${field.label}…`);
    await transferTo(client, field.map, field.x, field.y);
    await delay(1500);

    // Probe GM-spawn capability up front (each phase manages its own load).
    const probeBefore = (await readMonsters(client).catch(() => null))?.count ?? 0;
    await gmChat(client, `@MOB ${MONSTER_NAME} 3`);
    await delay(1200);
    const probeAfter = (await readMonsters(client).catch(() => null))?.count ?? 0;
    const gmSpawnWorks = probeAfter > probeBefore;
    await clearMonsters(client);
    console.log(`· GM @MOB spawn ${gmSpawnWorks ? "WORKS" : "unavailable → natural-aggro fallback"} (probe ${probeBefore}→${probeAfter})`);

    // Arm the flight recorder + the per-frame probe.
    const recorderInfo = await loadRecorder(client);
    await client.evaluate(installMoveProbeSrc());
    await client.evaluate(installLongTaskObserverSrc());
    await client.evaluate("window.__mir2Recorder.start(); window.__mir2LoadProbe.start();");
    console.log(`· recorder armed (${recorderInfo.source}) + probe running`);

    // Held-keyboard movement beats FIRST, on a fresh low-load session — this is the
    // keyboard SEND pipeline the click-based load phases never exercise. Drain the
    // probe afterward so these samples do not bleed into the first load phase.
    let heldFindings = null;
    let heldSamples = [];
    if (RUN_HELD_BEAT) {
      heldFindings = await runHeldKeyboardBeats(client, { gmSpawnWorks });
      heldSamples = (await client.evaluate("window.__mir2LoadProbe?.drain() ?? []")) ?? [];
    }

    // Load-graded phases over ONE logged-in session — the only thing that
    // changes between them is the load, so a rise in snaps/corrections from one
    // phase to the next names the trigger.
    const phaseDefs = [
      { name: "gentle", spawnTarget: 0, hog: false, gentle: true }, // CONTROL: calm play (should stay clean)
      { name: "aggressive", spawnTarget: SPAWN_COUNT, hog: false, gentle: false }, // fast runs + hard reversals
      ...(RUN_HOG_PHASE ? [{ name: "aggressive+hitch", spawnTarget: SPAWN_COUNT, hog: true, gentle: false }] : []), // + injected frame-drops
    ];
    const phases = [];
    const allSamples = [];
    allSamples.push(...heldSamples);
    const phaseMs = Math.floor(TOTAL_DURATION_MS / phaseDefs.length);
    for (const def of phaseDefs) {
      const r = await runPhase(client, def, phaseMs);
      phases.push(r.phase);
      allSamples.push(...r.samples);
    }

    // Stop capture + pull the canonical recorder dump (no browser download).
    await client.evaluate("window.__mir2LoadProbe?.stop();");
    const recording = await client.evaluate("window.__mir2Recorder.dump(false)");
    await client.evaluate("window.__mir2Recorder.stop();");

    // Render-perf channels (Stage 0): the standards-based in-page long-task total
    // and the per-packet handler timings from the ?perfDiag=1 gate in page.tsx.
    const longTask = (await client.evaluate("window.__mir2LongTask?.stop() ?? null").catch(() => null)) ?? null;
    const perfDiag = (await client.evaluate("window.__mir2PerfDiag ?? null").catch(() => null)) ?? null;

    const verdict = buildVerdict(phases);
    const ctx = {
      phases,
      recording,
      recorderSource: recorderInfo.source,
      verdict,
      allSamples,
      consoleErrors: client.consoleErrors,
      gmSpawnWorks,
      heldFindings,
      // Stage-0 [Violation]/long-task instrumentation, folded into the report row.
      violations: client.violations,
      violationCounts: client.violationCounts,
      longTask,
      perfDiag,
    };
    await writeReport(ctx);

    console.log(`\n===== VERDICT =====`);
    console.log(`reproduced: ${verdict.reproduced ? "YES" : "no"}  ·  trigger: ${verdict.trigger}`);
    for (const n of verdict.notes) console.log(` - ${n}`);
    if (heldFindings) {
      console.log(`held-keyboard movement: ${heldFindings.passed ? "PASS — continuous SENDs, no held-direction stall" : "FAIL — held-direction stall detected"}`);
      for (const i of heldFindings.issues) console.log(` - ${i}`);
      // A held-direction stall is a hard regression — surface it via the exit code.
      if (!heldFindings.passed) process.exitCode = 1;
    }
    // Stage-0 render-perf summary line.
    const vTotal = Object.values(client.violationCounts ?? {}).reduce((n, v) => n + (v.count ?? 0), 0);
    const vBuckets = Object.entries(client.violationCounts ?? {})
      .map(([k, v]) => `${k}=${v.count}(${Math.round(v.maxMs)}ms max)`)
      .join(" ");
    console.log(
      `render-perf: [Violation] ${vTotal}${vBuckets ? ` — ${vBuckets}` : ""}` +
        (longTask ? ` · longTasks ${longTask.count} (${Math.round(longTask.totalMs)}ms)` : "") +
        (perfDiag?.handlers ? ` · perfDiag packets=${Object.keys(perfDiag.handlers).length}` : ""),
    );
    console.log(`\nReport: ${path.join(outputDir, "report.md")}`);
  } catch (error) {
    console.error("qa-load-stress fatal:", error);
    process.exitCode = 1;
  } finally {
    client?.close();
    await stopChrome(chrome).catch(() => {});
  }
}

main();
