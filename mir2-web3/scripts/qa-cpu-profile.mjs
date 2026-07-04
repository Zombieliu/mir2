// qa-cpu-profile.mjs — attribute the held-run main-thread hitch to specific functions.
//
// The run-stutter residual ("奔跑两步一卡") is a recurring ~80ms main-thread long task that the
// in-page longTask API (?perfDiag=1) can SEE but cannot ATTRIBUTE: every ServerPacket handler is
// <=0.4ms, so the hitch is in the render/flush layer, not packet handling. This script captures a
// real CDP CPU profile (V8 Profiler domain) while a REAL held Shift+arrow run is driven over CDP
// Input.dispatchKeyEvent (synthetic KeyboardEvents are unreliable for held movement — CDP input
// is a real key), then aggregates self-time by function so the dominant cost (React commit / Bevy
// WASM frame / atlas decode / GC) is named. Also cross-references Chrome's [Violation] "handler
// took Nms" log lines (rAF vs message vs setInterval) for coarse corroboration.
//
// Defaults target the local verify rig: prod build on :3080, gateway :7141, char Walko704 (so it
// does not fight a Scout lease held by another open client). Judge on a PROD build only
// (memory client-perf-judge-on-prod-build) — `next build --webpack` + `next start -p 3080`.
//
// Usage:
//   node scripts/qa-cpu-profile.mjs
//   node scripts/qa-cpu-profile.mjs --baseUrl http://127.0.0.1:3080 --char Walko704 --holdMs 10000 --dir Down
//   MIR2_CHROME_HEADED=0 node scripts/qa-cpu-profile.mjs   # headless (WebGPU may fall back to WebGL2)

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a.startsWith("--")) {
      const key = a.slice(2);
      const next = argv[i + 1];
      if (next === undefined || next.startsWith("--")) out[key] = true;
      else {
        out[key] = next;
        i += 1;
      }
    }
  }
  return out;
}
const args = parseArgs(process.argv.slice(2));
const bool = (v, d) => (v === undefined ? d : v === true || v === "1" || v === "true");
const num = (v, d) => (v === undefined ? d : Number(v));

const gatewayWs = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS ?? "ws://127.0.0.1:7141/ws";
const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3080";
const RUN_URL = `${baseUrl}${baseUrl.includes("?") ? "&" : "?"}mir2Debug=1&perfDiag=1&gatewayWs=${encodeURIComponent(gatewayWs)}`;
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? "demo";
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "demo";
const characterName = args.char ?? args.characterName ?? "Walko704";
const holdMs = num(args.holdMs, 10_000);
const dir = args.dir ?? "Down";
const settleMs = num(args.settleMs, 2_500);
const samplingIntervalUs = num(args.samplingIntervalUs, 120); // fine-grained V8 sampling
const headed = bool(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const debugPort = num(args.debugPort, 9700 + (process.pid % 250));
const VIEWPORT = { width: 1024, height: 768 };

function findChromePath() {
  if (process.env.MIR2_CHROME_PATH) return process.env.MIR2_CHROME_PATH;
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium-browser",
  ];
  return candidates.find((p) => existsSync(p)) ?? null;
}
const chromePath = findChromePath();
if (!chromePath) throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");

const delay = (ms) => new Promise((r) => setTimeout(r, ms));

const ARROW = {
  Up: { key: "ArrowUp", code: "ArrowUp", vk: 38 },
  Right: { key: "ArrowRight", code: "ArrowRight", vk: 39 },
  Down: { key: "ArrowDown", code: "ArrowDown", vk: 40 },
  Left: { key: "ArrowLeft", code: "ArrowLeft", vk: 37 },
};
const SHIFT = { key: "Shift", code: "ShiftLeft", vk: 16 };

class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.violations = [];
    this.violationCounts = {};
    this.wsSent = []; // outbound WS frames — ground-truth proof the run actually moved
  }
  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (e) => this.onMessage(e.data));
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }
  onMessage(raw) {
    const msg = JSON.parse(raw);
    if (msg.id && this.pending.has(msg.id)) {
      const { resolve, reject } = this.pending.get(msg.id);
      this.pending.delete(msg.id);
      if (msg.error) reject(new Error(`${msg.error.message}: ${msg.error.data ?? ""}`));
      else resolve(msg.result ?? {});
      return;
    }
    if (msg.method === "Network.webSocketFrameSent") {
      this.wsSent.push({ payload: msg.params?.response?.payloadData ?? "", at: Date.now() });
      if (this.wsSent.length > 5000) this.wsSent = this.wsSent.slice(-4000);
    }
    if (msg.method === "Log.entryAdded") {
      const text = String(msg.params?.entry?.text ?? "");
      if (/handler took|\[Violation\]/i.test(text)) {
        const ms = Number((text.match(/took\s+(\d+(?:\.\d+)?)\s*ms/i) ?? [])[1] ?? 0);
        const bucket = (text.match(/'([^']+)'\s+handler/i) ?? [])[1]?.toLowerCase() ?? (/forced reflow/i.test(text) ? "reflow" : "other");
        this.violations.push({ bucket, ms });
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
    const r = await this.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true, userGesture: true });
    if (r.exceptionDetails) throw new Error(`eval failed: ${r.exceptionDetails.text ?? ""}`);
    return r.result?.value;
  }
  close() {
    try {
      this.ws?.close();
    } catch {
      /* ignore */
    }
  }
}

async function waitForChrome() {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const r = await fetch(`http://127.0.0.1:${debugPort}/json/version`);
      if (r.ok) return;
    } catch {
      await delay(100);
    }
  }
  throw new Error("Timed out waiting for Chrome debug endpoint.");
}

async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-cpu-profile-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      "--ignore-gpu-blocklist",
      "--enable-webgl",
      "--autoplay-policy=no-user-gesture-required",
      "--disable-background-timer-throttling",
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
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
  const r = await fetch(`http://127.0.0.1:${debugPort}/json/new?${encodeURIComponent(RUN_URL)}`, { method: "PUT" });
  if (!r.ok) throw new Error(`target creation failed: ${r.status}`);
  const t = await r.json();
  await delay(3000);
  return t.webSocketDebuggerUrl;
}

async function waitUntilSoft(client, expression, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      if (await client.evaluate(`Boolean(${expression})`)) return true;
    } catch {
      /* not ready */
    }
    await delay(250);
  }
  return false;
}

async function fillInput(client, selector, value) {
  await client.evaluate(`
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
}

async function click(client, selector) {
  await client.evaluate(`
    (() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return false;
      el.click();
      return true;
    })()
  `);
}

async function sendCommand(client, command) {
  await client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)})`);
}

async function enterWorld(client) {
  await waitUntilSoft(client, "['login','select','game'].includes(window.__mir2Stage5?.state?.screen)", 25_000);
  let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  if (screen === "login") {
    await fillInput(client, ".login-input.account", account).catch(() => {});
    await fillInput(client, ".login-input.password", password).catch(() => {});
    await click(client, ".login-button.ok button").catch(() => {});
    await waitUntilSoft(client, "['select','game'].includes(window.__mir2Stage5?.state?.screen)", 30_000);
    screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  }
  if (screen !== "game") {
    const startIndex = await client.evaluate(
      `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
    );
    await sendCommand(client, { type: "startGame", characterIndex: startIndex });
    await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 25_000);
  }
  await waitUntilSoft(
    client,
    "window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)",
    30_000,
  );
}

async function dispatchKey(client, type, k, autoRepeat, modifiers) {
  await client.send("Input.dispatchKeyEvent", {
    type,
    key: k.key,
    code: k.code,
    windowsVirtualKeyCode: k.vk,
    nativeVirtualKeyCode: k.vk,
    autoRepeat,
    modifiers: modifiers ?? 0,
    isKeypad: false,
  });
}

// Hold Shift + an arrow for holdMs = a real sustained RUN. The shell sets run mode on the Shift
// keydown (heldKeyboardRunModeRef) and drives held SENDs off its 100ms interval (it ignores
// event.repeat), so a single arrow keydown held with periodic autoRepeats is a faithful held run.
async function holdRun(client, dirName, ms) {
  const k = ARROW[dirName];
  if (!k) throw new Error(`unknown dir ${dirName}`);
  await client.evaluate("(() => { const a = document.activeElement; if (a && a.blur) a.blur(); return true; })()");
  const SHIFT_MOD = 8;
  await dispatchKey(client, "keyDown", SHIFT, false, SHIFT_MOD);
  await dispatchKey(client, "keyDown", k, false, SHIFT_MOD);
  const deadline = Date.now() + ms;
  let nextRepeat = Date.now() + 200;
  while (Date.now() < deadline) {
    if (Date.now() >= nextRepeat) {
      await dispatchKey(client, "keyDown", k, true, SHIFT_MOD);
      nextRepeat = Date.now() + 200;
    }
    await delay(50);
  }
  await dispatchKey(client, "keyUp", k, false, SHIFT_MOD);
  await dispatchKey(client, "keyUp", SHIFT, false, 0);
}

// Aggregate V8 CPU profile self-time by call frame (functionName @ url:line).
function analyzeProfile(profile) {
  const { nodes, samples, timeDeltas, startTime, endTime } = profile;
  const byId = new Map(nodes.map((n) => [n.id, n]));
  // self-time per node id (us), from the sample stream + timeDeltas (precise self attribution).
  const selfUsById = new Map();
  for (let i = 0; i < samples.length; i += 1) {
    const id = samples[i];
    const dt = timeDeltas[i] ?? 0;
    selfUsById.set(id, (selfUsById.get(id) ?? 0) + dt);
  }
  const agg = new Map();
  for (const [id, us] of selfUsById) {
    const n = byId.get(id);
    if (!n) continue;
    const f = n.callFrame ?? {};
    const name = f.functionName || "(anonymous)";
    const url = (f.url || "").split("/").slice(-1)[0] || (f.url ? f.url : "(native)");
    const key = `${name}  @ ${url}${f.lineNumber >= 0 ? ":" + (f.lineNumber + 1) : ""}`;
    agg.set(key, (agg.get(key) ?? 0) + us);
  }
  const totalUs = (endTime - startTime) || samples.reduce((s, _, i) => s + (timeDeltas[i] ?? 0), 0);
  const ranked = [...agg.entries()].sort((a, b) => b[1] - a[1]);
  return { totalMs: totalUs / 1000, sampleCount: samples.length, ranked };
}

async function main() {
  console.log(`[cpu-profile] launching Chrome (${headed ? "headed" : "headless"}) → ${RUN_URL}`);
  const chrome = await launchChrome();
  let client;
  try {
    const wsUrl = await createPageTarget();
    client = new CdpClient(wsUrl);
    await client.connect();
    await client.send("Runtime.enable");
    await client.send("Page.enable");
    await client.send("Network.enable");
    await client.send("Log.enable");
    await client.send("Profiler.enable");
    await client.send("Profiler.setSamplingInterval", { interval: samplingIntervalUs });
    await client.send("Page.bringToFront").catch(() => {});

    console.log(`[cpu-profile] entering world as ${characterName} …`);
    await enterWorld(client);
    const diag = await client.evaluate(`(() => {
      const s = window.__mir2Stage5?.state ?? {};
      return JSON.stringify({
        screen: s.screen, wsState: s.wsState,
        chars: Array.isArray(s.characters) ? s.characters.map(c => c?.name) : null,
        sceneReady: s.sceneInteractionReady ?? s.sceneAssetReadiness?.ready ?? null,
        hasSend: typeof window.__mir2Stage5?.send === 'function',
        self: (() => { const p = window.__mir2SceneMotionDebug?.renderPlayer; return p ? p.x+','+p.y : null; })(),
      });
    })()`).catch((e) => "diag-failed: " + e.message);
    console.log(`[cpu-profile] post-enter state: ${diag}`);
    await delay(settleMs); // let prewarm / first paint settle so the profile is the RUN, not load
    client.violations = [];
    client.violationCounts = {};

    const readPos = () =>
      client
        .evaluate("(() => { const p = window.__mir2SceneMotionDebug?.renderPlayer; return p ? p.x + ',' + p.y : null; })()")
        .catch(() => null);
    const posBefore = await readPos();

    const sentBefore = client.wsSent.length;
    console.log(`[cpu-profile] profiling a ${holdMs}ms held Shift+${dir} run … (start tile ${posBefore})`);
    await client.send("Profiler.start");
    await holdRun(client, dir, holdMs);
    const { profile } = await client.send("Profiler.stop");
    const moveFrames = client.wsSent
      .slice(sentBefore)
      .filter((f) => /"(run|walk)"|run\b|walk\b/i.test(f.payload)).length;

    const posAfter = await readPos();
    const tilesMoved = (() => {
      if (!posBefore || !posAfter) return null;
      const [ax, ay] = posBefore.split(",").map(Number);
      const [bx, by] = posAfter.split(",").map(Number);
      return Math.max(Math.abs(bx - ax), Math.abs(by - ay));
    })();

    const { totalMs, sampleCount, ranked } = analyzeProfile(profile);
    console.log(`\n=== CPU profile: ${totalMs.toFixed(0)}ms wall, ${sampleCount} samples ===`);
    console.log(`top self-time by function:`);
    for (const [key, us] of ranked.slice(0, 25)) {
      const ms = us / 1000;
      const pct = ((us / (totalMs * 1000)) * 100).toFixed(1);
      console.log(`  ${ms.toFixed(1).padStart(7)}ms  ${pct.padStart(5)}%  ${key}`);
    }

    console.log(`\n=== Chrome [Violation] handlers during the run (coarse attribution) ===`);
    const vc = Object.entries(client.violationCounts).sort((a, b) => b[1].maxMs - a[1].maxMs);
    if (vc.length === 0) console.log("  (none)");
    for (const [bucket, agg] of vc) {
      console.log(`  ${bucket.padEnd(24)} count=${agg.count} maxMs=${agg.maxMs} totalMs=${agg.totalMs.toFixed(0)}`);
    }

    // Save the raw profile for offline inspection (Chrome DevTools "Load profile").
    const outPath = path.join(os.tmpdir(), `mir2-cpu-profile-${Date.now()}.cpuprofile`);
    await fs.writeFile(outPath, JSON.stringify(profile));
    console.log(`\nraw .cpuprofile: ${outPath}  (load in DevTools › Performance › Load profile)`);
    console.log(`run distance: ${posBefore} → ${posAfter}  (renderTilesMoved=${tilesMoved ?? "n/a"})`);
    console.log(`move packets SENT during profile: ${moveFrames}  ← ground-truth that the run was actually moving`);
    if (moveFrames === 0) console.log(`⚠️  no move packets — profile is a STANDING scene, not a run (stuck at spawn / wall / not in game?).`);
  } finally {
    client?.close();
    try {
      chrome.kill();
    } catch {
      /* ignore */
    }
    if (chrome.userDataDir) {
      await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => {});
    }
  }
}

main().catch((err) => {
  console.error("[cpu-profile] failed:", err);
  process.exit(1);
});
