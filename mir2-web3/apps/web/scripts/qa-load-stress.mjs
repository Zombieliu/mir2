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
const RUN_URL = `${baseUrl}${baseUrl.includes("?") ? "&" : "?"}mir2Debug=1`;

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

if (!chromePath) throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (same shape as qa-playthrough.mjs) — captures console, network
// failures, and WebSocket frames (the server's authoritative truth).
// ---------------------------------------------------------------------------
class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
    this.networkFailures = [];
    this.requestUrlById = new Map();
    this.webSocketFramesReceived = [];
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
  const { phases, recording, recorderSource, verdict, allSamples } = ctx;
  await fs.mkdir(outputDir, { recursive: true });

  // Canonical flight-recorder dump (scrubbable in /mir2-replay.html).
  await fs.writeFile(path.join(outputDir, "recording.json"), JSON.stringify(recording));
  // Structured findings.
  const findings = {
    runId,
    capturedAt: new Date().toISOString(),
    field: field.label,
    config: { totalDurationMs: TOTAL_DURATION_MS, spawnCount: SPAWN_COUNT, monster: MONSTER_NAME, hogPhase: RUN_HOG_PHASE, hogMs: HOG_MS, hogPeriodMs: HOG_PERIOD_MS, hitchMs: HITCH_MS, leadCapTiles: LEAD_CAP_TILES },
    gmSpawnWorks: ctx.gmSpawnWorks ?? null,
    recorderSource,
    verdict,
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
    await client.evaluate("window.__mir2Recorder.start(); window.__mir2LoadProbe.start();");
    console.log(`· recorder armed (${recorderInfo.source}) + probe running`);

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

    const verdict = buildVerdict(phases);
    const ctx = { phases, recording, recorderSource: recorderInfo.source, verdict, allSamples, consoleErrors: client.consoleErrors, gmSpawnWorks };
    await writeReport(ctx);

    console.log(`\n===== VERDICT =====`);
    console.log(`reproduced: ${verdict.reproduced ? "YES" : "no"}  ·  trigger: ${verdict.trigger}`);
    for (const n of verdict.notes) console.log(` - ${n}`);
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
