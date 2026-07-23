// qa-render-sweep.mjs — render-robustness sweep across many maps + a regression
// guard for the WebGPU -> WebGL2 fallback (PR #127).
//
// Two independent phases, each driving the REAL web client over the Chrome
// DevTools Protocol (the default renderer is Bevy, so render health is judged on
// the CANVAS — screenshot luma + asset readiness — never the DOM scene layers):
//
//   1. MAP SWEEP (real GPU when --headed). Log in once, then transfer across a
//      list of Crystal maps (town / field / interior / cave / dungeon / HellFire).
//      For EACH map assert it actually renders:
//        - state.sceneInteractionReady === true (or sceneAssetReadiness.ready)
//        - canvas is not black (screenshot luma mean above BLACK_LUMA_MEAN)
//        - no stuck "Loading map…" overlay
//        - record net::ERR / 4xx for /original-map tiles (the known uncovered-tile
//          gap — recorded as a WARNING, not a hard fail, so the gate stays green
//          on a known gap; see project memory "bevy-map-uncovered-tile-gap").
//      Emits a per-map pass/fail table.
//
//   2. WEBGPU FALLBACK regression. Launch a SEPARATE Chrome with WebGPU made
//      unusable (`--disable-gpu` over localhost -> navigator.gpu present but
//      requestAdapter() -> null, plus `--enable-unsafe-swiftshader` for a software
//      WebGL2 device — the real-flag repro from project memory
//      "bevy-runtime-backend-verify"). Assert the client falls back to WebGL2 and
//      STILL renders a non-black map — i.e. NOT a black "boot-error" screen. The
//      runtime exposes its choice on window.__mir2BevyRuntimeDebug.selectedBackend
//      (set in apps/web/app/page.tsx wireAndBootRuntime); PR #127 guards exactly
//      this fallback (module-load throw at page.tsx:3463 and boot-time
//      panic-recovery both end at selectedBackend === "webgl2").
//
// This file is deliberately SELF-CONTAINED — it copies the proven CdpClient,
// luma helpers (captureLumaGrid / lumaStats / assessRender), login flow, and
// Chrome lifecycle from scripts/qa-playthrough.mjs + scripts/smoke-bevy-runtime-
// backends.mjs so it stays consistent with the house QA harnesses with zero new
// dependencies and without touching the existing scripts.
//
// Usage:
//   node ./scripts/qa-render-sweep.mjs --headed [--baseUrl http://127.0.0.1:3025]
//        [--maps "0@330,270;1@315,82;HF1"]   # ; between maps, @X,Y optional
//        [--limit N] [--skipSweep] [--skipFallback]
//        [--account NAME --password PW] [--createAccount true]
//
// The backend (gateway + embedded Crystal sim) must already be reachable at the
// client's configured gateway WS (ws://127.0.0.1:7110/ws in local dev). Exit code
// is 0 only when every swept map passed AND the fallback rendered under WebGL2.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const APP_DIR = path.resolve(SCRIPT_DIR, ".."); // apps/web — its docs/generated/player-qa/ is gitignored

const args = parseArgs(process.argv.slice(2));

const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3025";
// ?mir2Debug=1 keeps parity with the other harnesses (it gates extra debug hooks);
// __mir2BevyRuntimeDebug is set regardless, but the flag is harmless and consistent.
const RUN_URL = `${baseUrl}${baseUrl.includes("?") ? "&" : "?"}mir2Debug=1`;
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultName("QA", 12);
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultName("QR", 10);
const skipSweep = booleanArg(args.skipSweep, false);
const skipFallback = booleanArg(args.skipFallback, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 250));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(APP_DIR, "docs", "generated", "player-qa", `render-sweep-${runId}`),
);

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Tunables.
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const FALLBACK_SCENE_READY_TIMEOUT_MS = numberArg(args.fallbackSceneReadyTimeoutMs, 75_000); // swiftshader is slow
const TRANSFER_TIMEOUT_MS = numberArg(args.transferTimeoutMs, 20_000);
const SETTLE_MS = numberArg(args.settleMs, 1_600); // let the new map's tiles fetch/fail + frames settle
const BLACK_LUMA_MEAN = 8; // mean luma below this = effectively black (nothing renders — even player/HUD invisible)
const FLAT_LUMA_VARIANCE = 6; // variance below this = near-uniform (blank) frame
// Healthy maps render their floor at luma ~50-80; a map at ~10-13 is showing only
// the player sprite + HUD chrome over an unrendered (black) floor — the known
// uncovered-tile gap (project memory "bevy-map-uncovered-tile-gap"). That is
// DEGRADED, not a hard fail: surfaced loudly, but it does not break the gate
// (else the gate is perpetually red on a documented gap rather than a regression).
const DARK_FLOOR_LUMA = 30;
// The fallback "still renders" assertion runs against this known-floor-rendering
// map (the spawn town) so a degraded interior can never spuriously fail it.
const FALLBACK_RENDER_MAP = { map: "0", x: 330, y: 270 };

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

// ---------------------------------------------------------------------------
// Map sweep list. Landing tiles are taken from QUICK_TRANSFER_OPTIONS
// (apps/web/app/page.tsx:972) and from the worldSnapshot mapTransfers' toPosition
// (each is a guaranteed-valid spawn tile on the destination map). The debug
// crystal transfer lands the player EXACTLY at (x,y) with no walkability clamp
// (relocate_player_to_map in apps/simulation/src/runtime/map.rs), so these tiles
// place the camera on real content. Spread across town / field / interior / cave /
// dungeon / HellFire chain to exercise render breadth, not just the spawn town.
// mustRenderFloor: maps that are atlas-covered and MUST render their floor (the
// canonical town + fields every QA harness relies on). If one of these comes back
// with an unrendered floor it is a hard FAIL (atlas-coverage regression), not the
// tolerated "degraded" of the obscure uncovered maps.
const DEFAULT_SWEEP_MAPS = [
  { map: "0", x: 330, y: 270, label: "Bichon Province (town)", mustRenderFloor: true },
  { map: "1", x: 315, y: 82, label: "Woomyon Woods S (field)", mustRenderFloor: true },
  { map: "2", x: 503, y: 483, label: "Serpent Valley (field)", mustRenderFloor: true },
  { map: "0122", x: 13, y: 40, label: "Palace (interior)" },
  { map: "0103", x: 5, y: 12, label: "Weapon Store (interior)" },
  { map: "0101", x: 9, y: 22, label: "Tavern (interior)" },
  { map: "0125", x: 8, y: 16, label: "Inn (interior)" },
  { map: "D011", x: 154, y: 368, label: "Natural Cave (dungeon)" },
  { map: "D401", x: 25, y: 181, label: "Dead Mine Entrance (dungeon)" },
  { map: "D001", x: 151, y: 362, label: "Oma Cave 1F (dungeon)" },
  { map: "HELL00", x: 61, y: 82, label: "Waste Lands (HellFire approach)" },
  { map: "HF1", x: 200, y: 200, label: "HellFire 1F" },
  { map: "HF2", x: 200, y: 200, label: "HellFire 2F" },
  { map: "HF3", x: 200, y: 200, label: "HellFire 3F" },
  { map: "D1801", x: 200, y: 200, label: "Penal Cavern" },
  { map: "HKR", x: 200, y: 200, label: "HellFire Kings Room" },
  { map: "n0", x: 200, y: 200, label: "n0 (QA map)" },
];

const sweepMaps = resolveSweepMaps();

// ---------------------------------------------------------------------------
// CDP client — captures console errors and network failures (the only signals
// this harness needs). Adapted from scripts/qa-playthrough.mjs.
// ---------------------------------------------------------------------------
class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
    this.networkFailures = [];
    this.requestUrlById = new Map();
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
          source: "console",
          type: p.type,
          text: (p.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
          at: Date.now(),
        });
      }
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
// In-page state reader — compact view of the authoritative client state plus the
// Bevy runtime backend choice.
// ---------------------------------------------------------------------------
async function readState(client) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const body = document.body?.innerText ?? "";
      const compact = (d) => {
        if (!d || typeof d !== "object") return null;
        const o = {};
        for (const k of Object.keys(d)) {
          const v = d[k];
          if (typeof v === "number" || typeof v === "string" || typeof v === "boolean") o[k] = v;
        }
        return o;
      };
      return {
        screen: s.screen ?? null,
        wsState: s.wsState ?? null,
        mapFileName: s.mapFileName ?? null,
        mapTitle: s.mapTitle ?? null,
        sceneInteractionReady: s.sceneInteractionReady ?? null,
        sceneAssetReady: s.sceneAssetReadiness?.ready ?? null,
        player: s.player ? { x: s.player.x, y: s.player.y } : null,
        loadingMapVisible: body.includes("Loading map"),
        bevyRuntime: compact(window.__mir2BevyRuntimeDebug),
        bodyText: body.slice(0, 240),
      };
    })()
  `);
}

// ---------------------------------------------------------------------------
// Render health — canvas-centric (Bevy is the renderer). captureLumaGrid /
// lumaStats are the proven helpers from scripts/qa-playthrough.mjs.
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

// Classify the canvas luma into render-health flags shared by both phases.
async function assessRender(client) {
  const lumas = await captureLumaGrid(client).catch(() => null);
  const { mean, variance } = lumaStats(lumas);
  const black = mean !== null && mean < BLACK_LUMA_MEAN;
  const blank = !black && variance !== null && variance < FLAT_LUMA_VARIANCE;
  return { mean, variance, black, blank };
}

async function captureFrame(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
}

// ---------------------------------------------------------------------------
// Login / enter-world — distilled from scripts/qa-playthrough.mjs (register ->
// login -> create our OWN named character -> startGame -> scene ready). Throws on
// a hard failure so a broken stack aborts loudly instead of running N failing maps.
// ---------------------------------------------------------------------------
async function enterWorld(client, url, sceneReadyTimeoutMs) {
  await navigate(client, url);
  await waitUntil(client, "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)", "client stage ready", 30_000);
  let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");

  if (screen === "login") {
    await waitForSelectorSoft(client, ".login-overlay", 8_000);
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    if (createAccount) {
      await clickSelector(client, ".login-button.account button").catch(() => {});
      await waitUntilSoft(client, "window.__mir2Stage5?.state?.wsState === 'open'", 15_000);
      await delay(1_800);
      await fillInput(client, ".login-input.account", account).catch(() => {});
      await fillInput(client, ".login-input.password", password).catch(() => {});
    }
    await clickSelector(client, ".login-button.ok button");
    await waitUntil(client, "['select', 'game'].includes(window.__mir2Stage5?.state?.screen)", "character select / game", 30_000);
    screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  }

  if (screen !== "game") {
    // Create OUR OWN server-backed character. Do NOT trust state.characters to
    // decide existence — it can hold a phantom slot the server never returned
    // (observed: LoginSuccess characters:[] yet state.characters had an entry).
    const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;
    if (!(await client.evaluate(haveOurChar))) {
      await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: "warrior" });
      await waitUntilSoft(client, haveOurChar, 15_000);
    }
    const startIndex = await client.evaluate(
      `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
    );
    await sendCommand(client, { type: "startGame", characterIndex: startIndex });
    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'game'", "enter game", 30_000);
  }

  await waitUntil(
    client,
    "window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)",
    "scene assets ready",
    sceneReadyTimeoutMs,
  );
  await delay(1_200); // let the first frames settle
}

// Forced debug transfer (crystal:<map>:<x>:<y> -> WorldCommand::TransferMap, lands
// the player exactly at (x,y) and sets state.mapFileName). Soft — never throws;
// returns whether the client landed on the requested map.
async function transferToMap(client, map, x, y, timeoutMs) {
  await sendCommand(client, { type: "transferMap", key: `crystal:${map}:${x}:${y}` });
  const arrived = await waitUntilSoft(client, `window.__mir2Stage5?.state?.mapFileName === ${JSON.stringify(map)}`, timeoutMs);
  return arrived;
}

// ---------------------------------------------------------------------------
// Phase 1 — map sweep.
// ---------------------------------------------------------------------------
async function runMapSweep(client) {
  const rows = [];
  const readyExpr =
    "window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true";

  for (let i = 0; i < sweepMaps.length; i += 1) {
    const m = sweepMaps[i];
    console.log(`\n▶ [${i + 1}/${sweepMaps.length}] sweep map "${m.map}" — ${m.label} @ (${m.x},${m.y})`);
    const netBaseline = client.networkFailures.length;
    const consoleBaseline = client.consoleErrors.filter(isCriticalConsoleError).length;

    const arrived = await transferToMap(client, m.map, m.x, m.y, TRANSFER_TIMEOUT_MS).catch(() => false);
    const ready = await waitUntilSoft(client, readyExpr, SCENE_READY_TIMEOUT_MS);
    await delay(SETTLE_MS); // give tiles time to fetch/fail and frames to settle

    const state = await readState(client).catch(() => ({}));
    const render = await assessRender(client);

    const newFailures = client.networkFailures.slice(netBaseline);
    const tile404 = newFailures.filter((f) => f.kind === "map");
    const runtimeFail = newFailures.filter((f) => f.kind === "runtime");
    // ui / sound / entity / atlas / asset 404s are known dev + R2 asset gaps (NPC
    // libs, dev Sound/*.wav) — recorded for completeness, never a hard fail.
    const assetGap = newFailures.filter((f) => ["ui", "sound", "entity", "asset", "map-atlas"].includes(f.kind));
    const unexpectedFail = newFailures.filter((f) => f.kind === "other");
    const consoleDelta = client.consoleErrors.filter(isCriticalConsoleError).length - consoleBaseline;
    const loadingStuck = Boolean(state.loadingMapVisible);

    // Hard-fail reasons = the map is unreachable or truly broken.
    const reasons = [];
    if (!arrived) reasons.push("did-not-arrive");
    if (!ready) reasons.push("scene-not-ready");
    if (loadingStuck) reasons.push("stuck-loading");
    if (render.black) reasons.push("black-canvas");
    if (runtimeFail.length) reasons.push(`runtime-asset-404(${runtimeFail.length})`); // a missing Bevy runtime module breaks rendering

    // Floor not visibly rendered (only player + HUD over a black floor) — the known
    // uncovered-tile gap. DEGRADED on an obscure map, but a hard FAIL on a core map
    // expected to render its floor (atlas-coverage regression guard).
    const floorMissing = reasons.length === 0 && render.mean !== null && render.mean < DARK_FLOOR_LUMA;
    if (floorMissing && m.mustRenderFloor) reasons.push("core-map-floor-not-rendered");

    const hardFail = reasons.length > 0;
    const status = hardFail ? "fail" : floorMissing ? "degraded" : "pass";

    const warnings = [];
    if (status === "degraded") warnings.push(tile404.length ? `map-floor-missing(${tile404.length}×tile-404)` : "map-floor-missing(dark,no-404)");
    if (render.blank) warnings.push("near-uniform-canvas");
    if (tile404.length) warnings.push(`${tile404.length}×/original-map 404`);
    if (assetGap.length) warnings.push(`${assetGap.length}×asset-gap`);
    if (unexpectedFail.length) warnings.push(`${unexpectedFail.length}×other-fail`);

    const row = {
      map: m.map,
      label: m.label,
      x: m.x,
      y: m.y,
      status,
      arrived,
      mapFileName: state.mapFileName ?? null,
      sceneReady: Boolean(ready),
      loadingStuck,
      lumaMean: render.mean,
      lumaVariance: render.variance,
      black: render.black,
      blank: render.blank,
      floorMissing,
      tile404Count: tile404.length,
      tile404Sample: tile404.slice(0, 5).map((f) => `${f.status || f.errorText || "ERR"} ${stripBase(f.url)}`),
      runtimeFailCount: runtimeFail.length,
      assetGapCount: assetGap.length,
      unexpectedFailCount: unexpectedFail.length,
      consoleErrorDelta: consoleDelta,
      reasons,
      warnings,
    };
    rows.push(row);

    try {
      await captureFrame(client, path.join(outputDir, "frames", `sweep-${String(i + 1).padStart(2, "0")}-${slug(m.map)}.png`));
    } catch {
      /* screenshot is best-effort */
    }

    const badge = status === "pass" ? "✅ PASS" : status === "degraded" ? "🟡 DEGRADED" : "❌ FAIL";
    console.log(
      `   ${badge}  arrived=${arrived} ready=${row.sceneReady} ` +
        `luma=${fmt(render.mean)}/${fmt(render.variance)} tile404=${tile404.length}` +
        `${reasons.length ? `  reasons=${reasons.join(",")}` : ""}${warnings.length ? `  warn=${warnings.join(",")}` : ""}`,
    );
  }
  return rows;
}

// ---------------------------------------------------------------------------
// Phase 2 — WebGPU -> WebGL2 fallback regression (PR #127). Runs in its OWN
// Chrome launched with WebGPU made unusable.
// ---------------------------------------------------------------------------
async function runFallbackRegression(client) {
  const result = { phase: "webgpu-unusable-fallback" };
  let enterError = null;
  let entered = false;
  try {
    await enterWorld(client, RUN_URL, FALLBACK_SCENE_READY_TIMEOUT_MS);
    entered = true;
  } catch (err) {
    enterError = String(err?.message ?? err).slice(0, 300);
  }
  // Assert "still renders" against a known-floor-rendering map (the spawn town) so
  // a degraded interior — the same account may resume on the sweep's last map —
  // can't spoof a black canvas. Best-effort; if it fails we still assess what drew.
  if (entered) {
    await transferToMap(client, FALLBACK_RENDER_MAP.map, FALLBACK_RENDER_MAP.x, FALLBACK_RENDER_MAP.y, TRANSFER_TIMEOUT_MS).catch(() => {});
    await waitUntilSoft(
      client,
      "window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true",
      SCENE_READY_TIMEOUT_MS,
    );
    await delay(SETTLE_MS);
  }
  await delay(1_200);

  const state = await readState(client).catch(() => ({}));
  const rt = state.bevyRuntime ?? {};
  const render = await assessRender(client);
  const bodyText = state.bodyText ?? "";
  // The regression PR #127 guards against this: a black "boot-error" / runtime-skipped
  // screen instead of a clean WebGL2 fallback.
  const bootError = /boot error|boot-error|runtime skipped|neither WebGPU nor WebGL2/i.test(bodyText);
  const criticalConsole = client.consoleErrors.filter(isCriticalConsoleError);

  result.enterError = enterError;
  result.runtimePresent = Boolean(rt && rt.selectedBackend);
  result.requestedBackend = rt.requestedBackend ?? null;
  result.selectedBackend = rt.selectedBackend ?? null;
  result.compiledBackend = rt.compiledBackend ?? null;
  result.fallbackFrom = rt.fallbackFrom ?? null;
  result.webgpuSupported = rt.webgpuSupported ?? null;
  result.webgl2Supported = rt.webgl2Supported ?? null;
  result.enteredGame = state.screen === "game";
  result.sceneReady = state.sceneInteractionReady === true || state.sceneAssetReady === true;
  result.mapFileName = state.mapFileName ?? null;
  result.lumaMean = render.mean;
  result.lumaVariance = render.variance;
  result.black = render.black;
  result.bootError = bootError;
  result.criticalConsoleErrorCount = criticalConsole.length;
  result.criticalConsoleErrorSample = criticalConsole.slice(0, 6).map((e) => e.text?.slice(0, 160));
  // The repro is "inconclusive" (not a client regression) if WebGPU stayed usable
  // despite --disable-gpu — surface it distinctly so a green run isn't mistaken for
  // a verified fallback.
  result.webgpuStillUsable = rt.selectedBackend === "webgpu";

  const reasons = [];
  if (!result.runtimePresent) reasons.push("runtime-debug-absent");
  if (result.selectedBackend !== "webgl2") reasons.push(`selected-backend=${result.selectedBackend ?? "none"}`);
  if (!result.enteredGame) reasons.push("did-not-enter-game");
  if (render.black) reasons.push("black-canvas");
  if (bootError) reasons.push("boot-error-screen");
  result.reasons = reasons;
  result.pass = reasons.length === 0;

  try {
    await captureFrame(client, path.join(outputDir, "frames", "fallback-webgl2.png"));
  } catch {
    /* best-effort */
  }
  return result;
}

// ---------------------------------------------------------------------------
// Chrome lifecycle.
// ---------------------------------------------------------------------------
async function openClient(port, extraFlags) {
  const userDataDir = path.join(os.tmpdir(), `mir2-render-sweep-${process.pid}-${port}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${port}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      "--disable-background-timer-throttling",
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
      "--no-proxy-server",
      "--proxy-bypass-list=*",
      "--no-first-run",
      "--no-default-browser-check",
      `--window-size=${VIEWPORT.width},${VIEWPORT.height}`,
      ...extraFlags,
      "about:blank",
    ],
    // detached -> Chrome leads its own process group so closeClient can reap the
    // WHOLE tree (helper / GPU / renderer children); a bare kill of the parent
    // leaves them alive and they starve the GPU of the next headed run.
    { stdio: "ignore", detached: true },
  );
  chrome.userDataDir = userDataDir;
  await waitForChrome(port);

  const response = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent("about:blank")}`, { method: "PUT" });
  if (!response.ok) throw new Error(`Chrome target creation failed: ${response.status}`);
  const target = await response.json();
  const client = new CdpClient(target.webSocketDebuggerUrl);
  await client.connect();
  await client.send("Runtime.enable");
  await client.send("Log.enable");
  await client.send("Network.enable");
  await client.send("Page.enable");
  await client.send("Page.bringToFront"); // a backgrounded tab pauses rAF -> the canvas never repaints
  await client.send("Emulation.setDeviceMetricsOverride", VIEWPORT);
  await client.send("Emulation.setVisibleSize", { width: VIEWPORT.width, height: VIEWPORT.height }).catch(() => {});
  return { chrome, client };
}

async function closeClient(handle) {
  if (!handle) return;
  handle.client?.close();
  const chrome = handle.chrome;
  if (chrome && chrome.pid && !chrome.killed) {
    // Kill the whole process group (negative pid) so no Chrome child outlives us.
    const killGroup = (signal) => {
      try {
        process.kill(-chrome.pid, signal);
      } catch {
        try {
          chrome.kill(signal);
        } catch {
          /* already gone */
        }
      }
    };
    killGroup("SIGTERM");
    const exited = await Promise.race([
      new Promise((resolve) => chrome.once("exit", () => resolve(true))),
      delay(4_000).then(() => false),
    ]);
    if (!exited) killGroup("SIGKILL");
  }
  if (chrome?.userDataDir) await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => {});
}

async function waitForChrome(port) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) return;
    } catch {
      /* not up yet */
    }
    await delay(100);
  }
  throw new Error(`Timed out waiting for Chrome debug endpoint on ${port}.`);
}

async function navigate(client, url) {
  let lastError;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      await client.send("Page.navigate", { url });
      await waitUntil(client, "document.readyState === 'complete' || document.readyState === 'interactive'", "page load", 20_000);
      const currentUrl = await client.evaluate("window.location.href");
      if (typeof currentUrl !== "string" || !currentUrl.startsWith("chrome-error://")) return;
      lastError = new Error(`Chrome landed on ${currentUrl}`);
    } catch (error) {
      lastError = error;
    }
    await delay(500);
  }
  throw lastError ?? new Error("Page navigation failed.");
}

// ---------------------------------------------------------------------------
// DOM plumbing.
// ---------------------------------------------------------------------------
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

// SpriteButtons / overlay buttons react to a direct programmatic .click().
async function clickSelector(client, selector) {
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

async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
}

async function waitUntil(client, expression, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (v) return;
    await delay(120);
  }
  const debug = await client
    .evaluate(
      `(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, map: window.__mir2Stage5?.state?.mapFileName ?? null, sceneReady: window.__mir2Stage5?.state?.sceneInteractionReady ?? null, body: document.body?.innerText?.slice(0, 240) ?? "" }))()`,
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

// ---------------------------------------------------------------------------
// Reporting.
// ---------------------------------------------------------------------------
function printSweepTable(rows) {
  if (!rows.length) return;
  console.log("\n================ MAP SWEEP — per-map render result ================");
  const header = ["MAP".padEnd(7), "RESULT".padEnd(9), "ARR", "RDY", "LOAD", "LUMA(mean/var)".padEnd(16), "TILE404", "LABEL"];
  console.log(header.join("  "));
  for (const r of rows) {
    const result = r.status === "pass" ? "PASS" : r.status === "degraded" ? "DEGRADED" : "FAIL";
    console.log(
      [
        String(r.map).padEnd(7),
        result.padEnd(9),
        r.arrived ? " ✓ " : " ✗ ",
        r.sceneReady ? " ✓ " : " ✗ ",
        r.loadingStuck ? "STUCK" : " ok ",
        `${fmt(r.lumaMean)}/${fmt(r.lumaVariance)}`.padEnd(16),
        String(r.tile404Count).padStart(5),
        `  ${r.label}`,
      ].join("  "),
    );
  }
  const healthy = rows.filter((r) => r.status === "pass").length;
  const degraded = rows.filter((r) => r.status === "degraded").length;
  const failed = rows.filter((r) => r.status === "fail").length;
  console.log(`------------------------------------------------------------------`);
  console.log(
    `MAP SWEEP: ${healthy} healthy, ${degraded} degraded (floor/tiles not rendered — known gap), ${failed} failed, of ${rows.length}.`,
  );
}

async function writeReport(sweepRows, fallback) {
  const counts = (status) => (sweepRows ? sweepRows.filter((r) => r.status === status).length : 0);
  const summary = {
    runId,
    baseUrl,
    headed,
    startedAt: globalStartedAt,
    finishedAt: Date.now(),
    sweep: sweepRows
      ? {
          total: sweepRows.length,
          healthy: counts("pass"),
          degraded: counts("degraded"),
          failed: counts("fail"),
          mapsWithTile404: sweepRows.filter((r) => r.tile404Count > 0).length,
          ok: sweepRows.length > 0 && counts("fail") === 0, // degraded (known gap) does not break the gate
        }
      : null,
    fallback: fallback
      ? { selectedBackend: fallback.selectedBackend, pass: fallback.pass, reasons: fallback.reasons, webgpuStillUsable: fallback.webgpuStillUsable }
      : null,
  };

  await fs.writeFile(path.join(outputDir, "report.json"), JSON.stringify({ summary, sweep: sweepRows, fallback }, null, 2));

  const statusBadge = (s) => (s === "pass" ? "✅ PASS" : s === "degraded" ? "🟡 DEGRADED" : "❌ FAIL");
  const md = [];
  md.push(`# Render sweep + WebGPU→WebGL2 fallback report — ${runId}`);
  md.push("");
  md.push(`- Target: \`${baseUrl}\`  ·  headed \`${headed}\``);
  if (sweepRows && sweepRows.length) {
    md.push(
      `- Map sweep: **${counts("pass")} healthy / ${counts("degraded")} degraded / ${counts("fail")} failed** of ${sweepRows.length}  ·  ${summary.sweep.mapsWithTile404} map(s) hit /original-map 404s`,
    );
    md.push(`- _Degraded = map reached and client alive, but the floor did not render (only player + HUD) — the known uncovered-tile gap. Not a hard fail._`);
  }
  if (fallback) {
    md.push(`- WebGPU fallback: **${fallback.pass ? "PASS" : "FAIL"}** (selectedBackend \`${fallback.selectedBackend ?? "none"}\`${fallback.fallbackFrom ? `, fell back from \`${fallback.fallbackFrom}\`` : ""})`);
  }
  md.push("");

  if (sweepRows && sweepRows.length) {
    md.push("## Map sweep");
    md.push("");
    md.push("| Map | Result | Arrived | SceneReady | Loading | Luma mean/var | /original-map 404 | Notes | Label |");
    md.push("|---|---|---|---|---|---|---|---|---|");
    for (const r of sweepRows) {
      const notes = [...r.reasons.map((x) => `❌${x}`), ...r.warnings.map((x) => `⚠${x}`)].join(" ");
      md.push(
        `| \`${r.map}\` | ${statusBadge(r.status)} | ${r.arrived ? "✓" : "✗"} | ${r.sceneReady ? "✓" : "✗"} | ${r.loadingStuck ? "STUCK" : "ok"} | ${fmt(r.lumaMean)} / ${fmt(r.lumaVariance)} | ${r.tile404Count} | ${notes || "—"} | ${r.label} |`,
      );
    }
    md.push("");
    const offenders = sweepRows.filter((r) => r.tile404Count > 0);
    if (offenders.length) {
      md.push("### /original-map tile 404 samples (known uncovered-tile gap — warning only)");
      md.push("");
      for (const r of offenders) {
        md.push(`- \`${r.map}\` (${r.tile404Count}): ${r.tile404Sample.map((s) => `\`${s}\``).join(", ")}`);
      }
      md.push("");
    }
  }

  if (fallback) {
    md.push("## WebGPU → WebGL2 fallback regression (PR #127)");
    md.push("");
    md.push("Launched a separate Chrome with `--disable-gpu --enable-unsafe-swiftshader` (WebGPU unusable: `navigator.gpu` present but `requestAdapter()` → null; software WebGL2). The client must fall back to WebGL2 and still render a non-black map — not a black boot-error.");
    md.push("");
    md.push("```json");
    md.push(JSON.stringify(fallback, null, 2));
    md.push("```");
    md.push("");
  }

  md.push("## Reproduce");
  md.push("");
  md.push("```bash");
  md.push(`cd apps/web && node ./scripts/qa-render-sweep.mjs --headed --baseUrl ${baseUrl}`);
  md.push("```");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Small utilities.
// ---------------------------------------------------------------------------
function classifyAssetUrl(url) {
  const t = String(url ?? "");
  if (t.includes("/bevy-runtime/")) return "runtime";
  if (t.includes("/original-map/")) return "map";
  if (t.includes("/original-ui/") || t.includes("/api/original-ui-meta")) return "ui"; // NPC/sprite libs (known gap)
  if (t.includes("/generated/map-atlas/")) return "map-atlas";
  if (t.includes("/Sound/") || /\.(wav|mp3|ogg)(\?|$)/i.test(t)) return "sound"; // dev Sound/*.wav 404s are expected
  if (t.includes("entity") || t.includes("atlas")) return "entity";
  if (/\.(png|webp|ktx2|basis)(\?|$)/i.test(t)) return "asset";
  return "other";
}

// Filter the documented noise: favicon, ResizeObserver, the expected WebGPU-not-
// supported log during fallback, dev-server net::ERR_FAILED on existing files, and
// the known /original-map + /original-ui asset-gap 4xx (so consoleErrors aren't aliased
// into false criticals — see project memory "mir2 Chrome-MCP verify gotchas").
function isCriticalConsoleError(error) {
  const text = String(error?.text ?? "");
  if (/favicon|ResizeObserver loop|WebGPU is not supported|net::ERR_FAILED|net::ERR_ABORTED/i.test(text)) return false;
  // Known dev + R2 asset gaps (uncovered map tiles, NPC sprite libs incl. the
  // /api/original-ui-meta meta route, dev Sound/*.wav) — not real console errors.
  if (/(\/(original-map|original-ui|generated\/map-atlas)\/|\/api\/original-ui-meta|\/Sound\/|\.wav)/.test(text) && /(40\d|failed|ERR)/i.test(text)) return false;
  return true;
}

function stripBase(url) {
  return String(url ?? "").replace(baseUrl, "").slice(0, 120);
}

// Parse --maps "0@330,270;1@315,82;HF1" -> [{map,x,y,label}]. ';' between maps,
// optional '@X,Y' position (falls back to a known landing tile, else 200,200).
function resolveSweepMaps() {
  const limit = numberArg(args.limit, NaN);
  let list = DEFAULT_SWEEP_MAPS;
  if (typeof args.maps === "string" && args.maps.trim()) {
    const known = new Map(DEFAULT_SWEEP_MAPS.map((m) => [m.map, m]));
    list = args.maps
      .split(";")
      .map((entry) => entry.trim())
      .filter(Boolean)
      .map((entry) => {
        const [token, pos] = entry.split("@");
        const map = token.trim();
        if (pos) {
          const [px, py] = pos.split(",").map((n) => Number(n.trim()));
          if (Number.isFinite(px) && Number.isFinite(py)) return { map, x: px, y: py, label: known.get(map)?.label ?? map };
        }
        const fallback = known.get(map);
        return fallback ?? { map, x: 200, y: 200, label: map };
      });
  }
  if (Number.isFinite(limit) && limit > 0) list = list.slice(0, limit);
  return list;
}

function slug(text) {
  return String(text).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40);
}

function fmt(n) {
  return n === null || n === undefined ? "—" : Math.round(n * 10) / 10;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const [rawKey, inlineValue] = arg.slice(2).split("=", 2);
    const key = rawKey.trim();
    if (!key) continue;
    if (inlineValue !== undefined) {
      parsed[key] = inlineValue;
    } else {
      const next = argv[index + 1];
      parsed[key] = next && !next.startsWith("--") ? argv[++index] : "true";
    }
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

function defaultName(prefix, max) {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `${prefix}${suffix}`.slice(0, max).toUpperCase();
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
let globalStartedAt = 0;

async function main() {
  await fs.mkdir(path.join(outputDir, "frames"), { recursive: true });
  globalStartedAt = Date.now();
  console.log(`qa-render-sweep: target=${baseUrl} headed=${headed} account=${account}`);
  console.log(`qa-render-sweep: ${sweepMaps.length} map(s)${skipSweep ? " (sweep skipped)" : ""}${skipFallback ? " (fallback skipped)" : ""}`);
  console.log(`qa-render-sweep: output=${outputDir}`);

  let sweepRows = null;
  let fallback = null;

  // Phase 1 — map sweep with the real/default GPU backend.
  if (!skipSweep) {
    const handle = await openClient(debugPort, ["--ignore-gpu-blocklist", "--enable-unsafe-webgpu"]);
    try {
      console.log("\n— Phase 1: map sweep —");
      await enterWorld(handle.client, RUN_URL, SCENE_READY_TIMEOUT_MS);
      sweepRows = await runMapSweep(handle.client);
    } catch (error) {
      console.error("map sweep aborted:", String(error?.message ?? error).slice(0, 400));
      sweepRows = sweepRows ?? [];
    } finally {
      await closeClient(handle);
    }
  }

  // Phase 2 — WebGPU made unusable; assert WebGL2 fallback still renders.
  if (!skipFallback) {
    const handle = await openClient(debugPort + 1, ["--disable-gpu", "--enable-unsafe-swiftshader"]);
    try {
      console.log("\n— Phase 2: WebGPU → WebGL2 fallback regression —");
      fallback = await runFallbackRegression(handle.client);
    } catch (error) {
      console.error("fallback regression error:", String(error?.message ?? error).slice(0, 400));
      fallback = fallback ?? { phase: "webgpu-unusable-fallback", pass: false, reasons: ["harness-error"], error: String(error?.message ?? error).slice(0, 300) };
    } finally {
      await closeClient(handle);
    }
  }

  if (sweepRows) printSweepTable(sweepRows);
  if (fallback) {
    console.log("\n================ WEBGPU → WEBGL2 FALLBACK ================");
    console.log(
      `${fallback.pass ? "✅ PASS" : "❌ FAIL"}  selectedBackend=${fallback.selectedBackend ?? "none"}` +
        `${fallback.fallbackFrom ? ` (from ${fallback.fallbackFrom})` : ""}  enteredGame=${fallback.enteredGame}` +
        `  luma=${fmt(fallback.lumaMean)}  black=${fallback.black}  bootError=${fallback.bootError}` +
        `${fallback.reasons?.length ? `  reasons=${fallback.reasons.join(",")}` : ""}`,
    );
    if (fallback.webgpuStillUsable) {
      console.log("   ⚠ WebGPU stayed usable despite --disable-gpu — fallback path NOT exercised (repro inconclusive on this machine).");
    }
  }

  await writeReport(sweepRows, fallback);

  // Degraded maps (known uncovered-tile gap) don't break the gate — only a hard
  // FAIL (unreachable / not-ready / stuck / black-void / runtime-404) does.
  const sweepOk = skipSweep || (sweepRows && sweepRows.length > 0 && sweepRows.every((r) => r.status !== "fail"));
  const fallbackOk = skipFallback || (fallback && fallback.pass);
  const ok = Boolean(sweepOk && fallbackOk);
  console.log(`\nqa-render-sweep ${ok ? "PASSED" : "FAILED"}.  Report: ${path.join(outputDir, "report.md")}`);
  process.exitCode = ok ? 0 : 1;
}

main().catch((error) => {
  console.error("qa-render-sweep fatal:", error);
  process.exitCode = 1;
});
