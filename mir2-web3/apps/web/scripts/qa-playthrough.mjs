// qa-playthrough.mjs — AI-driven "real player" end-to-end walkthrough.
//
// Drives the REAL web client over Chrome DevTools Protocol through the full
// journey — register -> login -> create character -> enter world -> move ->
// find an NPC -> open dialog -> accept a quest -> travel -> wrap — and records
// EVERY problem it sees (rendering / movement / quest / network / console) into
// a structured report under docs/generated/player-qa/.
//
// Why a browser (not a protocol bot): rendering bugs (black canvas, missing
// sprites, render lag) only exist in a real Bevy canvas. The default renderer is
// Bevy, so render-health here targets the CANVAS (screenshot luma + asset
// readiness), not the DOM scene layers.
//
// This file deliberately reuses the patterns proven in
// capture-web-movement-jitter.mjs / smoke-stage5-ui.mjs (CDP client, chrome
// launch, login flow, tile clicking) so it stays consistent with the house QA
// harnesses, with zero new dependencies.
//
// Usage:
//   node ./scripts/qa-playthrough.mjs --headed [--baseUrl http://127.0.0.1:3001]
//        [--account NAME --password PW] [--createAccount true]
//        [--runId my-run] [--startMap 0 --startX 330 --startY 330]
//
// Each beat is best-effort: a failing beat is recorded as an issue and the loop
// keeps going so it captures as much of the journey as possible.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));

const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3001";
// Load with scene-motion diagnostics enabled so the camera update-rate probe can
// read window.__mir2SceneMotionDebug (motionNow clock + playerCameraMotionOffset).
const RUN_URL = `${baseUrl}${baseUrl.includes("?") ? "&" : "?"}mir2Debug=1`;
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultAccountName();
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultCharacterName();
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9600 + (process.pid % 300));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "player-qa", `playthrough-${runId}`),
);
const framesDir = path.join(outputDir, "frames");

// Optional forced spawn point (a "real" player just spawns wherever; only forced
// when --startMap is given).
const startMap = args.startMap ?? null;
const startX = numberArg(args.startX, NaN);
const startY = numberArg(args.startY, NaN);

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Tunables.
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const MOVE_SAMPLE_MS = 90; // logical-position sampling interval
const MOVE_WINDOW_MS = numberArg(args.moveWindowMs, 2600);
const NPC_DIALOG_TIMEOUT_MS = numberArg(args.npcDialogTimeoutMs, 14_000);
const QUEST_TIMEOUT_MS = numberArg(args.questTimeoutMs, 8_000);
const BLACK_LUMA_MEAN = 8; // mean luma below this = effectively black
const FLAT_LUMA_VARIANCE = 6; // variance below this = near-uniform (blank) frame
const FROZEN_FRAME_DELTA = 2.5; // visual delta below this across a move = render frozen

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (adapted from capture-web-movement-jitter.mjs) — captures console,
// network failures, and WebSocket frames (the server's authoritative truth).
// ---------------------------------------------------------------------------
class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleMessages = [];
    this.consoleErrors = [];
    this.networkFailures = [];
    this.requestUrlById = new Map();
    this.webSocketFramesReceived = [];
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
      const entry = {
        source: "console",
        type: p.type ?? "log",
        text: (p.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
        at: Date.now(),
      };
      this.consoleMessages.push(entry);
      this.consoleMessages = this.consoleMessages.slice(-400);
      if (p.type === "error" || p.type === "warning") this.consoleErrors.push(entry);
    } else if (m === "Runtime.exceptionThrown") {
      const d = p.exceptionDetails;
      this.consoleErrors.push({
        source: "exception",
        type: "error",
        text: d?.exception?.description ?? d?.text ?? "runtime exception",
        at: Date.now(),
      });
    } else if (m === "Log.entryAdded") {
      const entry = p.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        this.consoleErrors.push({
          source: entry.source ?? "log",
          type: "error",
          text: `${entry.text ?? ""}${entry.url ? ` (${entry.url})` : ""}`,
          at: Date.now(),
        });
      }
    } else if (m === "Network.requestWillBeSent") {
      if (p.requestId && p.request?.url) this.requestUrlById.set(p.requestId, p.request.url);
    } else if (m === "Network.responseReceived") {
      const r = p.response;
      const url = String(r?.url ?? "");
      if (r && r.status >= 400 && !url.includes("favicon")) {
        this.networkFailures.push({ url, status: r.status, kind: classifyAssetUrl(url), at: Date.now() });
      }
    } else if (m === "Network.loadingFailed") {
      const url = this.requestUrlById.get(p.requestId) ?? "(unknown)";
      if (!String(url).includes("favicon")) {
        this.networkFailures.push({ url, status: 0, errorText: p.errorText ?? "", kind: classifyAssetUrl(url), at: Date.now() });
      }
    } else if (m === "Network.webSocketFrameReceived") {
      this.webSocketFramesReceived.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-600);
    } else if (m === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-600);
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
// Run context + issue collection.
// ---------------------------------------------------------------------------
function createContext(client) {
  const issues = [];
  const beats = [];
  return {
    client,
    framesDir,
    issues,
    beats,
    seq: 0,
    log: (msg) => console.log(msg),
    addIssue(issue) {
      const id = `${issue.category}-${String(issues.length + 1).padStart(3, "0")}`;
      const full = { id, severity: "medium", ...issue };
      issues.push(full);
      console.log(`  ⚠ [${full.severity}/${full.category}] ${full.summary}`);
      return full;
    },
  };
}

async function runBeat(ctx, title, fn) {
  ctx.seq += 1;
  const id = ctx.seq;
  const beat = { id, title, startedAt: Date.now(), ok: true };
  ctx.log(`\n▶ beat ${id}: ${title}`);
  try {
    await fn(beat);
  } catch (err) {
    beat.ok = false;
    beat.error = String(err?.message ?? err);
    ctx.addIssue({
      category: "flow",
      severity: "high",
      beat: id,
      summary: `beat blocked: ${title}`,
      detail: beat.error,
    });
    ctx.log(`  ✖ ${beat.error.slice(0, 300)}`);
  }
  const frameName = `${String(id).padStart(2, "0")}-${slug(title)}.png`;
  try {
    await captureFrame(ctx.client, path.join(framesDir, frameName));
    beat.frame = `frames/${frameName}`;
  } catch {
    /* screenshot is best-effort */
  }
  try {
    beat.state = await readGameState(ctx.client);
  } catch {
    /* state read is best-effort */
  }
  beat.endedAt = Date.now();
  beat.durationMs = beat.endedAt - beat.startedAt;
  ctx.beats.push(beat);
  return beat;
}

// ---------------------------------------------------------------------------
// In-page state reader — compact view of the authoritative client state.
// ---------------------------------------------------------------------------
async function readGameState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const entities = Array.isArray(state.entities) ? state.entities : [];
      const npcs = entities.filter((e) => e && e.kind === "npc");
      const monsters = entities.filter((e) => e && e.kind === "monster");
      const questLog = Array.isArray(state.questLog) ? state.questLog : [];
      const dialog = state.activeNpcDialog ?? null;
      const bodyText = document.body?.innerText ?? "";
      const compactBevy = (dbg) => {
        if (!dbg || typeof dbg !== "object") return dbg ?? null;
        const out = {};
        for (const k of Object.keys(dbg)) {
          const v = dbg[k];
          if (typeof v === "number" || typeof v === "boolean" || typeof v === "string") out[k] = v;
        }
        return out;
      };
      const sceneLayers = [
        ["backdrop", ".game-scene-backdrop"],
        ["spriteOverlay", ".viewport-sprite-overlay"],
        ["entityOverlay", ".viewport-entity-overlay"],
        ["dropOverlay", ".viewport-drop-overlay"],
      ].map(([name, selector]) => {
        const node = document.querySelector(selector);
        const style = node ? getComputedStyle(node) : null;
        return { name, present: Boolean(node), opacity: style?.opacity ?? null, visibility: style?.visibility ?? null, display: style?.display ?? null };
      });
      return {
        capturedAt: Date.now(),
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        sceneInteractionReady: state.sceneInteractionReady ?? null,
        sceneAssetReadiness: state.sceneAssetReadiness ?? null,
        player: state.player ? { x: state.player.x, y: state.player.y, direction: state.player.direction ?? null } : null,
        playerObjectId: state.playerObjectId ?? null,
        entityCount: entities.length,
        npcCount: npcs.length,
        monsterCount: monsters.length,
        npcs: npcs.slice(0, 40).map((e) => ({ objectId: e.objectId, name: e.name ?? null, x: e.x, y: e.y })),
        characters: Array.isArray(state.characters)
          ? state.characters.map((c) => ({ name: c?.name ?? null, index: c?.index ?? null }))
          : [],
        questLogCount: questLog.length,
        questLog: questLog.slice(0, 20).map((q) => ({
          questId: q?.questId ?? q?.index ?? null,
          name: q?.name ?? q?.title ?? null,
          state: q?.state ?? q?.status ?? null,
        })),
        activeNpcDialog: dialog
          ? {
              npcObjectId: dialog.npcObjectId ?? null,
              npcName: dialog.npcName ?? null,
              title: dialog.title ?? null,
              linkCount: Array.isArray(dialog.links) ? dialog.links.length : null,
            }
          : null,
        npcDialogDom: Boolean(document.querySelector(".npc-dialog-panel")),
        npcDialogLinkCount: document.querySelectorAll(".npc-dialog-links button, .npc-dialog-links a").length,
        loadingMapVisible: bodyText.includes("Loading map"),
        sceneLayers,
        bevyRuntime: compactBevy(window.__mir2BevyRuntimeDebug),
        bevyEntity: compactBevy(window.__mir2BevyEntityRendererDebug),
      };
    })()
  `);
}

// ---------------------------------------------------------------------------
// Render health — canvas-centric (Bevy is the renderer).
// ---------------------------------------------------------------------------
async function canvasRect(client) {
  return client.evaluate(`
    (() => {
      const c = document.querySelector("#mir2-web3-canvas") || document.querySelector("canvas");
      if (!c) return null;
      const b = c.getBoundingClientRect();
      return { x: Math.max(0, b.left), y: Math.max(0, b.top), width: b.width, height: b.height };
    })()
  `);
}

// Returns an N×N luma grid for the live canvas region by screenshotting it and
// re-decoding the raster in-page (avoids WebGL readback pitfalls).
async function captureLumaGrid(client, grid = 8) {
  const rect = await canvasRect(client);
  const clip = rect && rect.width > 4 && rect.height > 4 ? { ...rect, scale: 1 } : undefined;
  const shot = await client.send("Page.captureScreenshot", {
    format: "jpeg",
    quality: 45,
    captureBeyondViewport: false,
    ...(clip ? { clip } : {}),
  });
  return client.evaluate(`
    (async () => {
      const img = new Image();
      img.src = "data:image/jpeg;base64,${shot.data}";
      try { await img.decode(); } catch { return null; }
      const G = ${grid};
      const cv = document.createElement("canvas");
      cv.width = G; cv.height = G;
      const ctx = cv.getContext("2d");
      ctx.drawImage(img, 0, 0, G, G);
      const d = ctx.getImageData(0, 0, G, G).data;
      const lumas = [];
      for (let i = 0; i < d.length; i += 4) lumas.push(0.299 * d[i] + 0.587 * d[i + 1] + 0.114 * d[i + 2]);
      return lumas;
    })()
  `);
}

function lumaStats(lumas) {
  if (!Array.isArray(lumas) || !lumas.length) return { mean: null, variance: null };
  const mean = lumas.reduce((a, b) => a + b, 0) / lumas.length;
  const variance = lumas.reduce((a, b) => a + (b - mean) * (b - mean), 0) / lumas.length;
  return { mean, variance };
}

function lumaDelta(a, b) {
  if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length || !a.length) return null;
  let sum = 0;
  for (let i = 0; i < a.length; i += 1) sum += Math.abs(a[i] - b[i]);
  return sum / a.length;
}

async function captureFrame(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
}

// Assess and record render health of the current canvas. `phase` labels the beat.
async function assessRenderHealth(ctx, beatId, phase) {
  const lumas = await captureLumaGrid(ctx.client).catch(() => null);
  const { mean, variance } = lumaStats(lumas);
  const state = await readGameState(ctx.client).catch(() => ({}));
  if (mean !== null && mean < BLACK_LUMA_MEAN) {
    ctx.addIssue({
      category: "render",
      severity: "high",
      beat: beatId,
      summary: `canvas is effectively black during ${phase} (mean luma ${mean.toFixed(1)})`,
      detail: { mean, variance, mapFileName: state.mapFileName, sceneAssetReadiness: state.sceneAssetReadiness },
    });
  } else if (variance !== null && variance < FLAT_LUMA_VARIANCE) {
    ctx.addIssue({
      category: "render",
      severity: "medium",
      beat: beatId,
      summary: `canvas is near-uniform / blank during ${phase} (variance ${variance.toFixed(1)})`,
      detail: { mean, variance, mapFileName: state.mapFileName },
    });
  }
  if (state.loadingMapVisible) {
    ctx.addIssue({
      category: "render",
      severity: "high",
      beat: beatId,
      summary: `"Loading map…" overlay still visible during ${phase}`,
      detail: { mapFileName: state.mapFileName },
    });
  }
  return { mean, variance, lumas };
}

// ---------------------------------------------------------------------------
// Camera update-rate probe — quantifies the judder cause.
// ---------------------------------------------------------------------------
// Sample the scene-motion debug hook (window.__mir2SceneMotionDebug, gated on
// ?mir2Debug=1) from an IN-PAGE rAF loop. A ~20Hz CDP poll would alias the
// ~33Hz content rate, so the loop must run inside the browser at frame rate.
async function sampleCameraMotion(client, durationMs) {
  await client.evaluate(`
    (() => {
      window.__qaCamSamples = [];
      window.__qaCamStop = false;
      const tick = () => {
        const d = window.__mir2SceneMotionDebug;
        const o = d && d.playerCameraMotionOffset;
        window.__qaCamSamples.push({ t: performance.now(), m: d ? d.motionNow : null, x: o ? o.x : null, y: o ? o.y : null });
        if (!window.__qaCamStop) requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
      return true;
    })()
  `);
  await delay(durationMs);
  return client.evaluate(
    `(() => { window.__qaCamStop = true; const s = window.__qaCamSamples || []; window.__qaCamSamples = []; return s; })()`,
  );
}

// From per-frame samples, derive the browser frame rate, the motionNow React-clock
// rate, and the distinct camera-scroll update rate (= the content/judder rate).
function analyzeCameraMotion(samples) {
  if (!Array.isArray(samples) || samples.length < 8) return null;
  const dts = [];
  for (let i = 1; i < samples.length; i += 1) dts.push(samples[i].t - samples[i - 1].t);
  dts.sort((a, b) => a - b);
  const medDt = dts[Math.floor(dts.length / 2)] || 0;
  const rafHz = medDt > 0 ? 1000 / medDt : null;
  let motionChanges = 0;
  let camChanges = 0;
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  for (let i = 0; i < samples.length; i += 1) {
    const s = samples[i];
    if (typeof s.x === "number") { minX = Math.min(minX, s.x); maxX = Math.max(maxX, s.x); }
    if (typeof s.y === "number") { minY = Math.min(minY, s.y); maxY = Math.max(maxY, s.y); }
    if (i > 0) {
      if (samples[i].m !== samples[i - 1].m) motionChanges += 1;
      if (samples[i].x !== samples[i - 1].x || samples[i].y !== samples[i - 1].y) camChanges += 1;
    }
  }
  const windowSec = (samples[samples.length - 1].t - samples[0].t) / 1000;
  const cameraUpdateHz = windowSec > 0 ? round2(camChanges / windowSec) : null;
  return {
    frames: samples.length,
    windowSec: round2(windowSec),
    rafHz: rafHz ? round2(rafHz) : null,
    motionClockHz: windowSec > 0 ? round2(motionChanges / windowSec) : null,
    cameraUpdateHz,
    offsetRangePx: round2(Math.max(maxX - minX, maxY - minY)),
    judderFactorVsRaf: cameraUpdateHz && rafHz ? round2(rafHz / cameraUpdateHz) : null,
  };
}

// ---------------------------------------------------------------------------
// Movement + interaction primitives (adapted from the movement harness).
// ---------------------------------------------------------------------------
async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
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

async function clickTile(client, x, y, button = "left") {
  const point = await tilePoint(client, x, y);
  if (!point) return false;
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

async function transferTo(client, map, x, y) {
  await sendCommand(client, { type: "transferMap", key: `crystal:${map}:${x}:${y}` });
  await waitUntil(
    client,
    `(() => { const s = window.__mir2Stage5?.state; return s?.mapFileName === ${JSON.stringify(map)} && s?.player?.x === ${Number(x)} && s?.player?.y === ${Number(y)}; })()`,
    "forced transfer",
    20_000,
  );
}

// Heuristic: spawn-area merchants/services don't grant quests — used only to
// ORDER candidates (quest-likely first) and to keep "no quest" from a pure
// merchant set at low severity. Never used to skip an NPC entirely.
function isLikelyMerchant(name) {
  return /merchant|storage|warehouse|black\s?smith|smith|repair|trade|shop|grocer|armou?r|weapon|potion|supplies|banker|deposit/i.test(
    String(name ?? ""),
  );
}

// Open (and confirm) an NPC's dialog the way a player would — click its tile so
// the client auto-walks then interacts, with a direct-interact fallback.
async function openNpcDialog(client, npc) {
  const clicked = await clickTile(client, npc.x, npc.y, "left");
  if (!clicked) await sendCommand(client, { type: "interact", objectId: Number(npc.objectId) });
  const mine = `window.__mir2Stage5?.state?.activeNpcDialog != null && String(window.__mir2Stage5.state.activeNpcDialog.npcObjectId) === ${JSON.stringify(String(npc.objectId))}`;
  if (await waitUntilSoft(client, mine, NPC_DIALOG_TIMEOUT_MS)) return true;
  await sendCommand(client, { type: "interact", objectId: Number(npc.objectId) });
  return waitUntilSoft(client, mine, 6_000);
}

// ---------------------------------------------------------------------------
// The journey.
// ---------------------------------------------------------------------------
async function playthrough(ctx) {
  const client = ctx.client;

  // Beat 1 — open the client, reach the login screen.
  await runBeat(ctx, "open client + login screen", async () => {
    await navigate(client, RUN_URL);
    await waitUntil(
      client,
      "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)",
      "client stage ready",
      25_000,
    );
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "login") await waitForSelectorSoft(client, ".login-overlay", 8_000);
  });

  // Beat 2 — register a fresh account (so the run is reproducible from scratch).
  await runBeat(ctx, "register account", async (beat) => {
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    beat.account = account;
    if (screen !== "login") {
      beat.note = `already past login (screen=${screen}); skipping registration`;
      return;
    }
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    if (createAccount) {
      await click(client, ".login-button.account button");
      await waitUntil(client, "window.__mir2Stage5?.state?.wsState === 'open'", "account-creation socket open", 15_000);
      await delay(2000);
    }
  });

  // Beat 3 — log in -> character select.
  await runBeat(ctx, "login to character select", async () => {
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "select" || screen === "game") return;
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    await click(client, ".login-button.ok button");
    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'select'", "select screen", 30_000);
  });

  // Beat 4 — create a character and start the game.
  await runBeat(ctx, "create character + start game", async (beat) => {
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "game") return;
    beat.characterName = characterName;

    // Create OUR OWN server-backed character. We deliberately do NOT trust
    // `state.characters` to decide whether a character exists — it can contain a
    // phantom slot the server never returned (observed: LoginSuccess characters:[]
    // yet state.characters had an entry named after the account). The reliable
    // "server created it" signal is OUR named character appearing.
    const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;

    if (!(await client.evaluate(haveOurChar))) {
      await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: "warrior" });
      let created = await waitUntilSoft(client, haveOurChar, 15_000);
      if (!created) {
        // Fall back to the on-screen "new character" flow a human would use.
        await click(client, ".select-action.new button").catch(() => {});
        await fillInput(client, "input[name='characterName'], .new-character-name, .character-create-name", characterName).catch(() => {});
        await click(client, ".select-action.ok button, .new-character-confirm button, .character-create-confirm button").catch(() => {});
        created = await waitUntilSoft(client, haveOurChar, 10_000);
      }
      if (!created) {
        ctx.addIssue({
          category: "flow",
          severity: "high",
          beat: beat.id,
          summary: "character creation did not produce a server-backed character (newCharacter not reflected)",
          detail: { characterName, characters: (await readGameState(client)).characters },
        });
      }
    }

    const startIndex = await client.evaluate(
      `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
    );
    beat.startIndex = startIndex;

    const wsBefore = client.webSocketFramesReceived.length;
    await sendCommand(client, { type: "startGame", characterIndex: startIndex });
    const entered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 25_000);
    if (!entered) {
      const result = startGameResultFrom(client.webSocketFramesReceived.slice(wsBefore));
      beat.startGameResult = result;
      ctx.addIssue({
        category: "flow",
        severity: "high",
        beat: beat.id,
        summary:
          result === null
            ? "startGame sent but never entered the game (no StartGame reply seen)"
            : `startGame rejected by server (StartGame result=${result}); character likely not server-backed`,
        detail: { startIndex, result, characters: (await readGameState(client)).characters },
      });
      throw new Error(`did not enter game (StartGame result=${result})`);
    }
  });

  // Beat 5 — enter the world: scene must become ready and actually render.
  await runBeat(ctx, "enter world + map renders", async (beat) => {
    await waitUntil(client, "!document.querySelector('.login-transition-overlay')", "login transition cleared", 8_000).catch(() => {});

    let sceneReady = true;
    try {
      await waitUntil(
        client,
        "window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)",
        "scene assets ready",
        SCENE_READY_TIMEOUT_MS,
      );
    } catch (err) {
      sceneReady = false;
      ctx.addIssue({
        category: "render",
        severity: "high",
        beat: beat.id,
        summary: "scene never became interaction/asset ready after entering world",
        detail: String(err?.message ?? err).slice(0, 400),
      });
    }

    if (startMap !== null && Number.isFinite(startX) && Number.isFinite(startY)) {
      await transferTo(client, startMap, startX, startY).catch((err) =>
        ctx.addIssue({ category: "flow", severity: "medium", beat: beat.id, summary: `forced transfer failed`, detail: String(err?.message ?? err) }),
      );
    }

    await delay(1200); // let the first frames settle
    beat.render = await assessRenderHealth(ctx, beat.id, "enter world");
  });

  // Beat 6 — move and check the render keeps up with the server.
  await runBeat(ctx, "movement + render-keeps-up", async (beat) => {
    const before = await readGameState(client);
    if (!before.player) throw new Error("no player position; cannot test movement");

    const target = { x: before.player.x + 4, y: before.player.y };
    const visualBefore = await captureLumaGrid(client).catch(() => null);
    const wsBefore = client.webSocketFramesReceived.length;

    // Issue a run toward the target the way a player would (click the tile).
    const clicked = await clickTile(client, target.x, target.y, "left");
    if (!clicked) await sendCommand(client, { type: "moveTo", x: target.x, y: target.y, mode: "run" });

    // Sample logical position + a few visual frames across the move window.
    const samples = [];
    const visualFrames = [];
    const deadline = Date.now() + MOVE_WINDOW_MS;
    let lastVisualAt = 0;
    while (Date.now() < deadline) {
      const s = await client.evaluate(
        `(() => { const p = window.__mir2Stage5?.state?.player; return p ? { x: p.x, y: p.y, dir: p.direction ?? null, at: Date.now() } : null; })()`,
      );
      if (s) samples.push(s);
      if (Date.now() - lastVisualAt > 600) {
        const grid = await captureLumaGrid(client).catch(() => null);
        if (grid) visualFrames.push(grid);
        lastVisualAt = Date.now();
      }
      await delay(MOVE_SAMPLE_MS);
    }
    const after = await readGameState(client);
    const wsAfter = client.webSocketFramesReceived.length;

    const moved = after.player && (after.player.x !== before.player.x || after.player.y !== before.player.y);
    const serverFrames = client.webSocketFramesReceived
      .slice(wsBefore)
      .filter((f) => /Object(Walk|Run|Turn|Pushed)|UserLocation|Pushed/.test(String(f.payloadData ?? ""))).length;

    beat.movement = {
      before: before.player,
      after: after.player,
      moved: Boolean(moved),
      logicalSamples: samples.length,
      serverMovementFrames: serverFrames,
      wsFramesInWindow: wsAfter - wsBefore,
    };

    if (!moved) {
      ctx.addIssue({
        category: "movement",
        severity: serverFrames > 0 ? "high" : "medium",
        beat: beat.id,
        summary:
          serverFrames > 0
            ? "server sent movement frames but client position did not change (render/state desync)"
            : "movement command produced no movement (server did not move the player)",
        detail: beat.movement,
      });
    } else {
      // Render-frozen check: server moved us but the canvas never visually changed.
      let maxVisualDelta = 0;
      for (let i = 1; i < visualFrames.length; i += 1) {
        const d = lumaDelta(visualFrames[i - 1], visualFrames[i]);
        if (d !== null) maxVisualDelta = Math.max(maxVisualDelta, d);
      }
      const overallDelta = lumaDelta(visualBefore, visualFrames[visualFrames.length - 1]);
      beat.movement.maxVisualDelta = round2(maxVisualDelta);
      beat.movement.overallVisualDelta = overallDelta === null ? null : round2(overallDelta);
      if (visualFrames.length >= 2 && maxVisualDelta < FROZEN_FRAME_DELTA && (overallDelta ?? 0) < FROZEN_FRAME_DELTA) {
        ctx.addIssue({
          category: "render",
          severity: "high",
          beat: beat.id,
          summary: "player moved on the server but the canvas did not visually update (render frozen during movement)",
          detail: beat.movement,
        });
      }

      // Jank / teleport detection on logical samples.
      let maxStep = 0;
      let lastChangeAt = samples.length ? samples[0].at : Date.now();
      let maxGapMs = 0;
      for (let i = 1; i < samples.length; i += 1) {
        const step = Math.abs(samples[i].x - samples[i - 1].x) + Math.abs(samples[i].y - samples[i - 1].y);
        if (step > 0) {
          maxGapMs = Math.max(maxGapMs, samples[i].at - lastChangeAt);
          lastChangeAt = samples[i].at;
        }
        maxStep = Math.max(maxStep, step);
      }
      beat.movement.maxTileStep = maxStep;
      // The gap between *logical tile changes* is the walk/run cadence (movement
      // SPEED), not render jank — recorded for context but never flagged as an
      // issue. Real movement-feel analysis (prediction staleness, command-queue
      // latency, camera continuity, visual jumps) is the dedicated job of
      // `capture-web-movement-jitter.mjs`, which has Crystal-calibrated thresholds.
      beat.movement.tileCadenceMs = maxGapMs;
      if (maxStep > 2) {
        ctx.addIssue({
          category: "movement",
          severity: "medium",
          beat: beat.id,
          summary: `position teleported ${maxStep} tiles in one sample (jump/desync)`,
          detail: beat.movement,
        });
      }
    }

    beat.render = await assessRenderHealth(ctx, beat.id, "after movement");
  });

  // Beat 6.5 — quantify the camera scroll update rate during a sustained walk.
  // Bevy draws at display Hz, but the camera offset is pushed from the ~33Hz React
  // motionNow clock; on a high-refresh display that low content rate reads as
  // "一顿一顿" judder. This turns the judder into a number (a perf number only counts
  // if the camera actually scrolled — the human-verified rule). The real fix lives
  // Bevy-side and must still be confirmed by a human walking on a 120Hz display.
  await runBeat(ctx, "camera update-rate probe", async (beat) => {
    if (!(await client.evaluate("window.__mir2SceneMotionDebug != null"))) {
      beat.note = "scene-motion debug hook not active (need ?mir2Debug=1)";
      return;
    }
    const p = await readGameState(client);
    if (!p.player) {
      beat.note = "no player position; skipping camera probe";
      return;
    }
    // Click a far tile to trigger a sustained auto-run (continuous scroll).
    const tx = p.player.x + 9;
    const ty = p.player.y;
    if (!(await clickTile(client, tx, ty, "left"))) {
      await sendCommand(client, { type: "moveTo", x: tx, y: ty, mode: "run" });
    }
    await delay(250); // let the run start scrolling before sampling
    const rate = analyzeCameraMotion(await sampleCameraMotion(client, 2500));
    beat.cameraRate = rate;

    if (!rate || rate.offsetRangePx < 2) {
      ctx.addIssue({
        category: "movement",
        severity: "low",
        beat: beat.id,
        summary: "camera-rate sample invalid — camera did not scroll during the walk (no sustained movement)",
        detail: rate,
      });
      return;
    }
    if (rate.cameraUpdateHz && rate.rafHz && rate.cameraUpdateHz < rate.rafHz * 0.7) {
      ctx.addIssue({
        category: "movement",
        severity: "medium",
        beat: beat.id,
        summary: `camera scrolls at ~${rate.cameraUpdateHz}Hz while frames render at ~${rate.rafHz}Hz (judder factor ${rate.judderFactorVsRaf}x) — the ~33Hz motionNow clock; worse on high-refresh displays`,
        detail: rate,
      });
    }
  });

  // Beats 7–8 — find a quest-giver NPC, open its dialog, accept a quest.
  // Spawn areas are full of merchants/services, so rank NPCs quest-likely first
  // and try several rather than only the nearest one (avoids a merchant false
  // positive).
  const questCandidates = [];
  await runBeat(ctx, "find NPC + open dialog", async (beat) => {
    const state = await readGameState(client);
    if (!state.npcs.length) {
      ctx.addIssue({
        category: "quest",
        severity: "medium",
        beat: beat.id,
        summary: "no NPC visible on the current map; cannot exercise NPC/quest flow",
        detail: { mapFileName: state.mapFileName, entityCount: state.entityCount },
      });
      return;
    }
    const p = state.player ?? { x: 0, y: 0 };
    const ranked = [...state.npcs].sort((a, b) => {
      const am = isLikelyMerchant(a.name) ? 1 : 0;
      const bm = isLikelyMerchant(b.name) ? 1 : 0;
      if (am !== bm) return am - bm; // quest-likely (non-merchant) NPCs first
      return Math.abs(a.x - p.x) + Math.abs(a.y - p.y) - (Math.abs(b.x - p.x) + Math.abs(b.y - p.y));
    });
    questCandidates.push(...ranked.slice(0, 4));
    beat.candidates = questCandidates.map((n) => ({ name: n.name, merchant: isLikelyMerchant(n.name) }));

    const npc = ranked[0];
    beat.npc = npc;
    if (!(await openNpcDialog(client, npc))) {
      ctx.addIssue({
        category: "quest",
        severity: "high",
        beat: beat.id,
        summary: `clicking NPC "${npc.name ?? npc.objectId}" did not open a dialog`,
        detail: npc,
      });
      return;
    }

    // The dialog is logically open — did it actually render?
    const afterState = await readGameState(client);
    if (!afterState.npcDialogDom) {
      ctx.addIssue({
        category: "render",
        severity: "high",
        beat: beat.id,
        summary: "NPC dialog is open in state but the .npc-dialog-panel did not render",
        detail: afterState.activeNpcDialog,
      });
    }
    beat.dialog = afterState.activeNpcDialog;
    beat.dialogLinkCount = afterState.npcDialogLinkCount;
  });

  // Beat 8 — accept a quest, trying each candidate NPC until one grants one.
  await runBeat(ctx, "accept quest", async (beat) => {
    if (!questCandidates.length) {
      beat.note = "no NPC candidates from beat 7";
      return;
    }
    const startQuestCount = (await readGameState(client)).questLogCount;
    beat.questLogBefore = startQuestCount;
    const tried = [];
    let acceptedFrom = null;

    for (const npc of questCandidates) {
      if (acceptedFrom) break;
      const isOpen = await client.evaluate(
        `window.__mir2Stage5?.state?.activeNpcDialog != null && String(window.__mir2Stage5.state.activeNpcDialog.npcObjectId) === ${JSON.stringify(String(npc.objectId))}`,
      );
      if (!isOpen && !(await openNpcDialog(client, npc))) {
        tried.push({ name: npc.name, opened: false });
        continue;
      }
      const before = (await readGameState(client)).questLogCount;
      const linkCount = (await readGameState(client)).npcDialogLinkCount || 0;
      // A real player clicks dialog links looking for an "accept quest" option.
      let granted = false;
      for (let i = 0; i < Math.max(1, Math.min(linkCount, 6)) && !granted; i += 1) {
        const clicked = await client.evaluate(
          `(() => { const l = document.querySelectorAll(".npc-dialog-links button, .npc-dialog-links a"); if (!l[${i}]) return false; l[${i}].click(); return true; })()`,
        );
        if (!clicked) break;
        granted = await waitUntilSoft(client, `(window.__mir2Stage5?.state?.questLog?.length ?? 0) > ${before}`, QUEST_TIMEOUT_MS);
      }
      tried.push({ name: npc.name, merchant: isLikelyMerchant(npc.name), opened: true, grantedQuest: granted });
      if (granted) acceptedFrom = npc;
    }

    const after = await readGameState(client);
    beat.tried = tried;
    beat.questLogAfter = after.questLogCount;
    // A quest can also be granted on proximity / mid-dialog without a single link
    // click "winning", so treat any net questLog growth during this beat as success.
    if (acceptedFrom || after.questLogCount > startQuestCount) {
      beat.acceptedFrom = acceptedFrom?.name ?? acceptedFrom?.objectId ?? "(during NPC interaction)";
      beat.acceptedQuest = after.questLog[after.questLog.length - 1] ?? null;
    } else {
      const openedTries = tried.filter((entry) => entry.opened);
      const allMerchants = openedTries.length > 0 && openedTries.every((entry) => entry.merchant);
      ctx.addIssue({
        category: "quest",
        severity: allMerchants ? "low" : "medium",
        beat: beat.id,
        summary: allMerchants
          ? `no quest accepted — the ${openedTries.length} reachable NPC(s) here are merchants/services (expected near spawn)`
          : `tried ${tried.length} NPC(s) but none granted a quest`,
        detail: { tried, questLogBefore: startQuestCount, questLogAfter: after.questLogCount },
      });
    }
  });

  // Beat 9 — travel along the journey (move again, watch render the whole way).
  await runBeat(ctx, "travel + render stability", async (beat) => {
    const before = await readGameState(client);
    if (!before.player) {
      beat.note = "no player position; skipping travel";
      return;
    }
    const legs = [
      { dx: 0, dy: 4 },
      { dx: -4, dy: 0 },
      { dx: 0, dy: -4 },
    ];
    let blackouts = 0;
    for (const leg of legs) {
      const p = await client.evaluate(
        `(() => { const pl = window.__mir2Stage5?.state?.player; return pl ? { x: pl.x, y: pl.y } : null; })()`,
      );
      if (!p) break;
      const tx = p.x + leg.dx;
      const ty = p.y + leg.dy;
      const clicked = await clickTile(client, tx, ty, "left");
      if (!clicked) await sendCommand(client, { type: "moveTo", x: tx, y: ty, mode: "run" });
      await delay(1400);
      const grid = await captureLumaGrid(client).catch(() => null);
      const { mean } = lumaStats(grid);
      if (mean !== null && mean < BLACK_LUMA_MEAN) blackouts += 1;
    }
    beat.blackoutLegs = blackouts;
    if (blackouts > 0) {
      ctx.addIssue({
        category: "render",
        severity: "high",
        beat: beat.id,
        summary: `canvas went black on ${blackouts}/${legs.length} travel legs`,
      });
    }
    beat.render = await assessRenderHealth(ctx, beat.id, "after travel");
  });

  // Beat 10 — wrap: snapshot final console/network state.
  await runBeat(ctx, "wrap + collect diagnostics", async (beat) => {
    beat.final = await readGameState(client);
  });
}

// ---------------------------------------------------------------------------
// Cross-cutting collectors (run after the journey).
// ---------------------------------------------------------------------------
function collectNetworkIssues(ctx) {
  const failures = ctx.client.networkFailures;
  if (!failures.length) return;
  // Group by kind so one issue summarises many identical sprite 404s.
  const byKind = new Map();
  for (const f of failures) {
    const key = f.kind ?? "other";
    if (!byKind.has(key)) byKind.set(key, []);
    byKind.get(key).push(f);
  }
  for (const [kind, list] of byKind) {
    const severity = kind === "runtime" ? "high" : kind === "map" || kind === "ui" || kind === "entity" ? "medium" : "low";
    const sample = list.slice(0, 6).map((f) => `${f.status || f.errorText || "ERR"} ${f.url}`);
    ctx.addIssue({
      category: "network",
      severity,
      beat: null,
      summary: `${list.length} ${kind} asset request(s) failed (e.g. sprite/atlas not served)`,
      detail: { kind, count: list.length, sample, note: "If files exist in git, the R2 release may be stale (see ASSET-RELEASE-RUNBOOK)." },
    });
  }
}

function collectConsoleIssues(ctx) {
  const errors = ctx.client.consoleErrors.filter(isCriticalConsoleError);
  if (!errors.length) return;
  // Dedup by message text.
  const seen = new Map();
  for (const e of errors) {
    const key = (e.text ?? "").slice(0, 160);
    if (!seen.has(key)) seen.set(key, { ...e, count: 0 });
    seen.get(key).count += 1;
  }
  for (const e of seen.values()) {
    ctx.addIssue({
      category: "console",
      severity: e.type === "error" ? "medium" : "low",
      beat: null,
      summary: `console ${e.type}: ${(e.text ?? "").slice(0, 140)}`,
      detail: { count: e.count, source: e.source },
    });
  }
}

// ---------------------------------------------------------------------------
// Reporting.
// ---------------------------------------------------------------------------
async function writeReport(ctx) {
  const bySeverity = { high: 0, medium: 0, low: 0 };
  const byCategory = {};
  for (const issue of ctx.issues) {
    bySeverity[issue.severity] = (bySeverity[issue.severity] ?? 0) + 1;
    byCategory[issue.category] = (byCategory[issue.category] ?? 0) + 1;
  }
  const summary = {
    runId,
    baseUrl,
    account,
    characterName,
    startedAt: ctx.startedAt,
    finishedAt: Date.now(),
    beats: ctx.beats.length,
    beatsOk: ctx.beats.filter((b) => b.ok).length,
    issueCount: ctx.issues.length,
    bySeverity,
    byCategory,
  };

  await fs.writeFile(path.join(outputDir, "summary.json"), JSON.stringify(summary, null, 2));
  await fs.writeFile(
    path.join(outputDir, "report.json"),
    JSON.stringify({ summary, issues: ctx.issues, beats: ctx.beats }, null, 2),
  );
  await fs.writeFile(
    path.join(outputDir, "console.json"),
    JSON.stringify({ errors: ctx.client.consoleErrors, messages: ctx.client.consoleMessages.slice(-200) }, null, 2),
  );
  await fs.writeFile(path.join(outputDir, "network-failures.json"), JSON.stringify(ctx.client.networkFailures, null, 2));
  await fs.writeFile(
    path.join(outputDir, "ws-timeline.json"),
    JSON.stringify(
      {
        sent: ctx.client.webSocketFramesSent.slice(-200),
        received: ctx.client.webSocketFramesReceived.slice(-200),
      },
      null,
      2,
    ),
  );

  const order = { high: 0, medium: 1, low: 2 };
  const sortedIssues = [...ctx.issues].sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9));
  const md = [];
  md.push(`# Player-QA playthrough report — ${runId}`);
  md.push("");
  md.push(`- Target: \`${baseUrl}\`  ·  account \`${account}\`  ·  character \`${characterName}\``);
  md.push(`- Beats: ${summary.beatsOk}/${summary.beats} ok  ·  Issues: ${summary.issueCount} (high ${bySeverity.high}, medium ${bySeverity.medium}, low ${bySeverity.low})`);
  md.push(`- By category: ${Object.entries(byCategory).map(([k, v]) => `${k} ${v}`).join(", ") || "none"}`);
  md.push("");
  md.push("## Issues (by severity)");
  md.push("");
  if (!sortedIssues.length) {
    md.push("_No issues detected._");
  } else {
    md.push("| # | Severity | Category | Beat | Summary |");
    md.push("|---|---|---|---|---|");
    for (const i of sortedIssues) {
      md.push(`| ${i.id} | ${i.severity} | ${i.category} | ${i.beat ?? "—"} | ${String(i.summary).replace(/\|/g, "\\|")} |`);
    }
  }
  md.push("");
  md.push("## Journey (beats)");
  md.push("");
  for (const b of ctx.beats) {
    md.push(`### Beat ${b.id} — ${b.title} ${b.ok ? "✅" : "❌"}  (${b.durationMs}ms)`);
    if (b.error) md.push(`- error: \`${b.error.slice(0, 300)}\``);
    if (b.note) md.push(`- note: ${b.note}`);
    if (b.state) {
      md.push(
        `- state: screen=\`${b.state.screen}\` map=\`${b.state.mapFileName ?? "?"}\` pos=${b.state.player ? `(${b.state.player.x},${b.state.player.y})` : "—"} npcs=${b.state.npcCount} quests=${b.state.questLogCount} sceneReady=${b.state.sceneInteractionReady}`,
      );
    }
    if (b.movement) md.push(`- movement: ${JSON.stringify(b.movement)}`);
    if (b.cameraRate) md.push(`- cameraRate: ${JSON.stringify(b.cameraRate)}`);
    if (b.render) md.push(`- render: mean=${fmt(b.render.mean)} variance=${fmt(b.render.variance)}`);
    if (b.frame) md.push(`- frame: [\`${b.frame}\`](${b.frame})`);
    md.push("");
  }
  md.push("## Detailed issue evidence");
  md.push("");
  for (const i of sortedIssues) {
    md.push(`### ${i.id} — [${i.severity}/${i.category}] ${i.summary}`);
    if (i.beat) md.push(`- beat: ${i.beat}`);
    if (i.detail !== undefined) md.push("```json\n" + JSON.stringify(i.detail, null, 2) + "\n```");
    md.push("");
  }
  md.push("## Reproduce");
  md.push("");
  md.push("```bash");
  md.push(`cd apps/web && node ./scripts/qa-playthrough.mjs --headed --baseUrl ${baseUrl} --account ${account}`);
  md.push("```");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from capture-web-movement-jitter.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-playthrough-${process.pid}-${Date.now()}`);
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
  let lastValue = null;
  while (Date.now() < deadline) {
    lastValue = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (lastValue) return;
    await delay(120);
  }
  const debug = await client
    .evaluate(
      `(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, sceneReady: window.__mir2Stage5?.state?.sceneInteractionReady ?? null, body: document.body?.innerText?.slice(0, 300) ?? "" }))()`,
    )
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

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) return;
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve));
}

// ---------------------------------------------------------------------------
// Small utilities.
// ---------------------------------------------------------------------------
function classifyAssetUrl(url) {
  const t = String(url ?? "");
  if (t.includes("/bevy-runtime/")) return "runtime";
  if (t.includes("/original-ui/")) return "ui";
  if (t.includes("/original-map/")) return "map";
  if (t.includes("entity") || t.includes("atlas")) return "entity";
  if (/\.(png|webp|ktx2|basis|wav|mp3|ogg)(\?|$)/i.test(t)) return "asset";
  return "other";
}

// Scan recently-received WS frames for the server's StartGame reply and return
// its result code (null if no StartGame frame was seen).
function startGameResultFrom(frames) {
  for (let i = frames.length - 1; i >= 0; i -= 1) {
    try {
      const o = JSON.parse(frames[i].payloadData);
      if (o && o.packet === "StartGame") return o.payload?.result ?? null;
    } catch {
      /* non-JSON frame */
    }
  }
  return null;
}

function isCriticalConsoleError(error) {
  const text = String(error?.text ?? "");
  if (error?.source === "network" && text.includes("net::ERR_FAILED")) return false;
  if (text.includes("favicon")) return false;
  return true;
}

function slug(text) {
  return String(text).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40);
}

function round2(n) {
  return Math.round(n * 100) / 100;
}

function fmt(n) {
  return n === null || n === undefined ? "—" : round2(n);
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

function defaultCharacterName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `QA${suffix}`.slice(0, 10).toUpperCase();
}

function defaultAccountName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `QA${suffix}`.slice(0, 12).toUpperCase();
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
  console.log(`qa-playthrough: target=${baseUrl} account=${account} char=${characterName} headed=${headed}`);
  console.log(`qa-playthrough: output=${outputDir}`);

  const chrome = await launchChrome();
  let client;
  let ctx;
  try {
    const wsUrl = await createPageTarget();
    client = new CdpClient(wsUrl);
    await client.connect();
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await client.send("Page.enable");
    await client.send("Page.bringToFront");
    await setViewport(client, VIEWPORT);

    ctx = createContext(client);
    ctx.startedAt = Date.now();

    await playthrough(ctx);

    collectNetworkIssues(ctx);
    collectConsoleIssues(ctx);
    await writeReport(ctx);

    const s = ctx.issues.reduce((acc, i) => ((acc[i.severity] = (acc[i.severity] ?? 0) + 1), acc), {});
    console.log(`\nqa-playthrough done: ${ctx.beats.filter((b) => b.ok).length}/${ctx.beats.length} beats ok, ${ctx.issues.length} issues (high ${s.high ?? 0}, medium ${s.medium ?? 0}, low ${s.low ?? 0}).`);
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
  } catch (error) {
    console.error("qa-playthrough fatal:", error);
    if (ctx) {
      try {
        ctx.addIssue({ category: "flow", severity: "high", beat: null, summary: "fatal harness error", detail: String(error?.message ?? error) });
        await writeReport(ctx);
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
