// qa-button-sounds.mjs — loop-checks that every original-UI "function button" plays the CORRECT
// click sound, driving the REAL <SpriteButton> over the Chrome DevTools Protocol.
//
// THE BUG THIS GUARDS
//   Every SpriteButton (the single shared component behind the HUD function bar, login, select,
//   and every dialog button) used to play a hard-coded sound id `10100`. In Crystal's
//   SoundList.cs that id is `LoginEffect` (the login-screen whoosh) — NOT a button click. The
//   real interface click is `ButtonA = 10103` (103.wav), which Crystal's MirControl.OnMouseClick
//   plays per-button (the de-facto default: 328 of 338 explicit buttons use ButtonA). So in the
//   port EVERY button fired the wrong, identical sound — exactly the "all button SFX sound the
//   same, and wrong" symptom. The fix points SpriteButton at ORIGINAL_SOUND_IDS.uiButtonClick =
//   10103. See lib/original-sound-events.ts + app/components/original-client-overlays.tsx.
//
// HOW IT CHECKS (no gateway, no login, no web3, no R2 bytes required)
//   The audio path is: SpriteButton.onClick -> playOriginalSoundId(id) -> crystalSoundPath(id) ->
//   new Audio(path).play(). We patch HTMLMediaElement.prototype.play (installed before page load
//   via Page.addScriptToEvaluateOnNewDocument) to record every played src, then map the wav
//   basename back to its Crystal sound id via public/original-ui/sound-index.generated.json. If a
//   sound id is NOT present locally it never reaches Audio — playOriginalSoundId records it on
//   window.__mir2AudioDiagnostics.missingSoundIds instead, which we also read, so the fired id is
//   observable either way. Target is the deterministic harness route app/qa/button-sounds, which
//   mounts the real SpriteButton wired to the real ORIGINAL_UI sprites.
//
//   The "loop": R rounds x every .sprite-button on the page, each click verified independently.
//   PASS iff every click fires exactly the expected ButtonA (10103) and NEVER LoginEffect (10100).
//
//   NOTE ON "they're all the same": that part is actually Crystal-faithful — the original
//   overwhelmingly uses ButtonA for buttons. The defect was the *wrong* id, not the uniformity.
//   This loop therefore asserts correctness of the id, and reports the distribution for the record.
//
// Usage:
//   node ./scripts/qa-button-sounds.mjs [--headed] [--baseUrl http://127.0.0.1:3025]
//        [--rounds 3] [--expectId 10103] [--debugPort N]
//   A Next dev server must already serve <baseUrl> (e.g. `next dev --webpack -p 3025`). No backend
//   gateway is needed — the harness route renders without one. Exit code 0 only when every click
//   across every round fired the expected button sound.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const APP_DIR = path.resolve(SCRIPT_DIR, ".."); // apps/web

const args = parseArgs(process.argv.slice(2));
const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3025";
const HARNESS_URL = `${baseUrl.replace(/\/$/, "")}/qa/button-sounds`;
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const rounds = Math.max(1, numberArg(args.rounds, 3));
const expectId = numberArg(args.expectId, 10103); // ButtonA — the canonical interface click
const FORBIDDEN_LOGIN_EFFECT = 10100; // SoundList.LoginEffect — the pre-fix bug; must NEVER fire
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9760 + (process.pid % 200));
const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Reverse map: wav basename (e.g. "103.wav") -> Crystal sound id (e.g. 10103).
const basenameToSoundId = await loadSoundIndex();

const PROBE_SOURCE = `
  (() => {
    if (window.__mir2SoundProbeInstalled) return;
    window.__mir2SoundProbeInstalled = true;
    window.__mir2SoundProbe = { plays: [] };
    try {
      const proto = HTMLMediaElement.prototype;
      const origPlay = proto.play;
      proto.play = function () {
        try {
          window.__mir2SoundProbe.plays.push({ src: this.currentSrc || this.src || "", t: Date.now() });
        } catch (e) {}
        try { return origPlay.apply(this, arguments); } catch (e) { return Promise.reject(e); }
      };
    } catch (e) {}
  })();
`;

async function main() {
  if (!chromePath) {
    console.error("No Chrome found. Set MIR2_CHROME_PATH to a Chrome/Chromium/Edge binary.");
    process.exit(2);
  }
  console.log(`qa-button-sounds: target=${HARNESS_URL} headed=${headed} rounds=${rounds} expect=${expectId}(ButtonA) forbid=${FORBIDDEN_LOGIN_EFFECT}(LoginEffect)`);

  const handle = await openClient(debugPort);
  const records = [];
  try {
    await handle.client.send("Page.addScriptToEvaluateOnNewDocument", { source: PROBE_SOURCE });
    await navigate(handle.client, HARNESS_URL);

    // Wait for the harness React tree to mount and expose its readiness flag + buttons.
    await waitUntil(
      handle.client,
      "window.__mir2ButtonHarness && window.__mir2ButtonHarness.ready === true && document.querySelectorAll('.sprite-button').length > 0",
      "harness mount",
      20_000,
    );

    const buttonKeys = await handle.client.evaluate(
      "Array.from(document.querySelectorAll('.sprite-button')).map((b) => b.getAttribute('aria-label'))",
    );
    if (!Array.isArray(buttonKeys) || buttonKeys.length === 0) {
      throw new Error("No .sprite-button elements found on the harness page.");
    }
    console.log(`qa-button-sounds: ${buttonKeys.length} buttons x ${rounds} rounds = ${buttonKeys.length * rounds} clicks\n`);

    for (let round = 1; round <= rounds; round += 1) {
      for (const key of buttonKeys) {
        records.push(await clickAndCapture(handle.client, key, round));
      }
    }
  } finally {
    await closeClient(handle);
  }

  report(records);
}

// Click one button and capture exactly the sound id(s) its click fired.
async function clickAndCapture(client, key, round) {
  const before = await client.evaluate(
    `(() => ({
       plays: (window.__mir2SoundProbe?.plays?.length) || 0,
       missing: (window.__mir2AudioDiagnostics?.missingSoundIds || []).slice(),
       clicks: (window.__mir2ButtonHarness?.clicks?.[${JSON.stringify(key)}]) || 0,
     }))()`,
  );

  const clicked = await client.evaluate(
    `(() => {
       const n = document.querySelector('.sprite-button[aria-label=' + ${JSON.stringify(JSON.stringify(key))} + ']');
       if (!n) return false;
       n.click();
       return true;
     })()`,
  );
  if (!clicked) {
    return { round, key, clicked: false, firedIds: [], handlerRan: false, basenames: [], missingNew: [] };
  }

  await delay(140); // onClick -> new Audio(...).play() is synchronous, but give React a beat

  const after = await client.evaluate(
    `(() => ({
       plays: (window.__mir2SoundProbe?.plays || []),
       missing: (window.__mir2AudioDiagnostics?.missingSoundIds || []).slice(),
       clicks: (window.__mir2ButtonHarness?.clicks?.[${JSON.stringify(key)}]) || 0,
     }))()`,
  );

  const newPlays = (after.plays || []).slice(before.plays);
  const basenames = newPlays
    .map((p) => basenameOf(p.src))
    .filter(Boolean);
  const playedIds = basenames.map((b) => basenameToSoundId.get(b)).filter((id) => id !== undefined);
  const missingNew = (after.missing || []).filter((id) => !(before.missing || []).includes(id));
  const firedIds = [...new Set([...playedIds, ...missingNew])];

  return {
    round,
    key,
    clicked: true,
    handlerRan: after.clicks > before.clicks,
    firedIds,
    basenames,
    missingNew,
  };
}

function report(records) {
  const idCounts = new Map();
  const failures = [];
  for (const r of records) {
    for (const id of r.firedIds) idCounts.set(id, (idCounts.get(id) ?? 0) + 1);

    const firedExpected = r.firedIds.includes(expectId);
    const firedForbidden = r.firedIds.includes(FORBIDDEN_LOGIN_EFFECT);
    const exactlyOne = r.firedIds.length === 1;
    if (!r.clicked) failures.push({ r, why: "button not found / not clicked" });
    else if (r.firedIds.length === 0) failures.push({ r, why: "no sound fired" });
    else if (firedForbidden) failures.push({ r, why: `fired LoginEffect ${FORBIDDEN_LOGIN_EFFECT} (the bug)` });
    else if (!firedExpected) failures.push({ r, why: `fired ${r.firedIds.join(",")} not ${expectId}` });
    else if (!exactlyOne) failures.push({ r, why: `fired multiple ids ${r.firedIds.join(",")}` });
    else if (!r.handlerRan) failures.push({ r, why: "onClick handler did not run" });
  }

  // Per-button summary (collapsed across rounds).
  const byKey = new Map();
  for (const r of records) {
    if (!byKey.has(r.key)) byKey.set(r.key, { ids: new Set(), ok: 0, n: 0, handler: 0 });
    const e = byKey.get(r.key);
    e.n += 1;
    r.firedIds.forEach((id) => e.ids.add(id));
    if (r.firedIds.length === 1 && r.firedIds[0] === expectId) e.ok += 1;
    if (r.handlerRan) e.handler += 1;
  }

  console.log("button                         rounds  fired-id(s)         handler  verdict");
  console.log("------                         ------  -----------         -------  -------");
  for (const [key, e] of byKey) {
    const ids = [...e.ids].map((id) => labelId(id)).join(",") || "—";
    const ok = e.ok === e.n && e.handler === e.n;
    console.log(
      `${key.padEnd(30)} ${String(e.ok + "/" + e.n).padStart(6)}  ${ids.padEnd(19)} ${String(e.handler + "/" + e.n).padStart(7)}  ${ok ? "✅" : "❌"}`,
    );
  }

  console.log("\nfired-id distribution (all clicks):");
  for (const [id, count] of [...idCounts.entries()].sort((a, b) => b[1] - a[1])) {
    console.log(`   ${labelId(id)} : ${count}`);
  }

  const total = records.length;
  const passed = total - failures.length;
  console.log(`\n================ BUTTON SOUND LOOP ================`);
  if (failures.length === 0) {
    console.log(`✅ PASS — ${passed}/${total} clicks fired ButtonA (${expectId}); zero LoginEffect (${FORBIDDEN_LOGIN_EFFECT}).`);
    process.exit(0);
  } else {
    console.log(`❌ FAIL — ${passed}/${total} clicks correct; ${failures.length} bad:`);
    for (const f of failures.slice(0, 20)) {
      console.log(`   round ${f.r.round} ${f.r.key}: ${f.why} [fired=${f.r.firedIds.join(",") || "none"}]`);
    }
    process.exit(1);
  }
}

function labelId(id) {
  if (id === 10100) return "10100(LoginEffect)";
  if (id === 10103) return "10103(ButtonA)";
  if (id === 10104) return "10104(ButtonB)";
  if (id === 10105) return "10105(ButtonC)";
  return String(id);
}

function basenameOf(src) {
  if (!src) return null;
  try {
    const clean = String(src).split("?")[0].split("#")[0];
    const base = clean.split("/").pop();
    return base ? base.toLowerCase() : null;
  } catch {
    return null;
  }
}

async function loadSoundIndex() {
  const file = path.join(APP_DIR, "public", "original-ui", "sound-index.generated.json");
  const json = JSON.parse(await fs.readFile(file, "utf8"));
  const map = new Map();
  for (const [id, entry] of Object.entries(json.sounds ?? {})) {
    const name = entry?.fileName ? String(entry.fileName).toLowerCase() : null;
    if (name) map.set(name, Number(id));
    const pathBase = entry?.path ? basenameOf(entry.path) : null;
    if (pathBase) map.set(pathBase, Number(id));
  }
  return map;
}

// ---------------------------------------------------------------------------
// CDP client + Chrome lifecycle (self-contained; mirrors scripts/qa-render-sweep.mjs).
// ---------------------------------------------------------------------------
class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
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
    if (m === "Runtime.exceptionThrown") {
      const d = p.exceptionDetails;
      this.consoleErrors.push(d?.exception?.description ?? d?.text ?? "runtime exception");
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

async function openClient(port) {
  const userDataDir = path.join(os.tmpdir(), `mir2-button-sounds-${process.pid}-${port}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${port}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      "--autoplay-policy=no-user-gesture-required", // so Audio.play() proceeds instead of NotAllowedError
      "--disable-background-timer-throttling",
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
      "--mute-audio", // we observe play() calls, not actual sound; keep the CI host quiet
      "--no-proxy-server",
      "--no-first-run",
      "--no-default-browser-check",
      `--window-size=${VIEWPORT.width},${VIEWPORT.height}`,
      "about:blank",
    ],
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
  await client.send("Page.enable");
  await client.send("Page.bringToFront");
  await client.send("Emulation.setDeviceMetricsOverride", VIEWPORT);
  return { chrome, client };
}

async function closeClient(handle) {
  if (!handle) return;
  handle.client?.close();
  const chrome = handle.chrome;
  if (chrome && chrome.pid && !chrome.killed) {
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
  throw lastError ?? new Error(`Failed to navigate to ${url}`);
}

async function waitUntil(client, expression, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (v === true) return;
    await delay(150);
  }
  throw new Error(`Timed out waiting for ${label}.`);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const [rawKey, inlineValue] = arg.slice(2).split("=", 2);
    const key = rawKey.trim();
    if (!key) continue;
    if (inlineValue !== undefined) parsed[key] = inlineValue;
    else {
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

main().catch((error) => {
  console.error("qa-button-sounds: fatal", error?.stack ?? error);
  process.exit(2);
});
