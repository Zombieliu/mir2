import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_BASE_URL = "http://127.0.0.1:13010";
const DEFAULT_GATEWAY_WS_URL = "ws://127.0.0.1:7210/ws";
const DEFAULT_OUTPUT_DIR = path.resolve(REPO_ROOT, "docs", "generated", "player-qa", "reconnect");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

const args = parseArgs(process.argv.slice(2));
const gatewayWsUrl = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? DEFAULT_GATEWAY_WS_URL;
const runId = args.runId ?? new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
const prefix = args.prefix ?? `reconnect-resume-${runId}`;
const baseUrl = buildBaseUrl(args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL, gatewayWsUrl);
const outputDir = path.resolve(args.output ?? process.env.MIR2_RECONNECT_SMOKE_OUTPUT ?? DEFAULT_OUTPUT_DIR);
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9900 + (process.pid % 500));
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const waitTimeoutMs = numberArg(args.waitTimeoutMs ?? process.env.MIR2_RECONNECT_SMOKE_WAIT_MS, 45_000);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
    this.network404s = [];
    this.packetFrames = [];
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
      if (message.error) {
        reject(new Error(`${message.error.message}: ${message.error.data ?? ""}`));
      } else {
        resolve(message.result ?? {});
      }
      return;
    }

    if (message.method === "Runtime.consoleAPICalled" && message.params?.type === "error") {
      this.consoleErrors.push({
        source: "console",
        text: (message.params.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
      });
    }

    if (message.method === "Runtime.exceptionThrown") {
      const details = message.params?.exceptionDetails;
      this.consoleErrors.push({
        source: "exception",
        text: details?.exception?.description ?? details?.text ?? "runtime exception",
      });
    }

    if (message.method === "Log.entryAdded") {
      const entry = message.params?.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        this.consoleErrors.push({ source: entry.source ?? "log", text: entry.text ?? "" });
      }
    }

    if (message.method === "Network.responseReceived") {
      const response = message.params?.response;
      if (response?.status === 404 && !String(response.url ?? "").includes("favicon")) {
        this.network404s.push(response.url);
      }
    }

    if (
      message.method === "Network.webSocketFrameReceived" ||
      message.method === "Network.webSocketFrameSent"
    ) {
      const payloadData = message.params?.response?.payloadData ?? "";
      if (typeof payloadData === "string" && isReconnectRelevantPayload(payloadData)) {
        this.packetFrames.push({
          direction: message.method.endsWith("Sent") ? "sent" : "received",
          payloadData: payloadData.slice(0, 800),
          at: Date.now(),
        });
        this.packetFrames = this.packetFrames.slice(-80);
      }
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.text ?? JSON.stringify(result.exceptionDetails));
    }
    return result.result?.value;
  }

  close() {
    this.ws?.close();
  }
}

async function main() {
  await fs.mkdir(outputDir, { recursive: true });
  const chrome = await launchChrome();
  let client;

  try {
    client = await makeClient();
    await quickEnterWorld(client);
    const before = await readSummary(client);

    const forcedCloseAccepted = await client.evaluate(
      `window.__mir2Stage5?.closeGatewayForReconnectSmoke?.() === true`,
    );
    await waitUntilClient(
      client,
      `(() => {
        const mode = window.__mir2Stage5?.state?.reconnectStatus?.mode;
        return mode === "scheduled" || mode === "connecting" || mode === "resuming";
      })()`,
      "reconnect status to leave idle",
      10_000,
    );
    const during = await readSummary(client);
    const overlayText = await client.evaluate(
      `document.body?.innerText?.split("\\n").filter((line) => /Reconnect|Connection lost|restored/i.test(line)).slice(-8) ?? []`,
    );

    await waitUntilClient(
      client,
      `(() => {
        const state = window.__mir2Stage5?.state;
        return state?.screen === "game" &&
          state?.wsState === "open" &&
          state?.reconnectStatus?.mode === "idle" &&
          Boolean(state?.player);
      })()`,
      "game to recover after reconnect",
      waitTimeoutMs,
    );
    const after = await readSummary(client);
    const screenshot = await captureScreenshot(client, `${prefix}.png`);

    const report = {
      ok: false,
      runId,
      baseUrl,
      gatewayWsUrl,
      before,
      during,
      after,
      overlayText,
      screenshot,
      consoleErrors: client.consoleErrors,
      nonFaviconNetwork404s: client.network404s,
      packetFrames: client.packetFrames.slice(-24),
    };
    report.allowedNetwork404s = report.nonFaviconNetwork404s.filter(isAllowedNetwork404);
    report.unexpectedNetwork404s = report.nonFaviconNetwork404s.filter((url) => !isAllowedNetwork404(url));
    report.criticalConsoleErrors = client.consoleErrors.filter(isCriticalConsoleError);
    report.assertions = {
      enteredGame: before.screen === "game" && before.wsState === "open" && Boolean(before.player),
      forcedCloseAccepted,
      sawReconnectStatus: ["scheduled", "connecting", "resuming"].includes(during.reconnectStatus?.mode),
      recoveredGame: after.screen === "game",
      reconnectIdle: after.reconnectStatus?.mode === "idle",
      wsOpen: after.wsState === "open",
      playerStayedPresent: Boolean(before.player) && Boolean(after.player),
      mapStayedSame: Boolean(before.mapFileName) && before.mapFileName === after.mapFileName,
      noCriticalConsoleErrors: report.criticalConsoleErrors.length === 0,
      noUnexpectedNetwork404s: report.unexpectedNetwork404s.length === 0,
    };
    report.ok = Object.values(report.assertions).every(Boolean);

    const reportPath = path.join(outputDir, `${prefix}.json`);
    const latestPath = path.join(outputDir, "latest-reconnect-resume.json");
    await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
    await fs.writeFile(latestPath, `${JSON.stringify(report, null, 2)}\n`);
    console.log(
      JSON.stringify(
        {
          ok: report.ok,
          reportPath,
          latestPath,
          screenshot,
          assertions: report.assertions,
        },
        null,
        2,
      ),
    );
    if (!report.ok) {
      process.exitCode = 1;
    }
  } finally {
    client?.close();
    await stopChrome(chrome);
  }
}

async function makeClient() {
  const target = await createPageTarget();
  const client = new CdpClient(target.webSocketDebuggerUrl);
  await client.connect();
  await client.send("Page.enable");
  await client.send("Runtime.enable");
  await client.send("Log.enable");
  await client.send("Network.enable");
  await setViewport(client);
  await client.send("Page.navigate", { url: baseUrl });
  await waitUntilClient(
    client,
    `document.readyState === "complete" || document.readyState === "interactive"`,
    "page load",
    20_000,
  );
  await waitUntilClient(
    client,
    `["login", "select", "game"].includes(window.__mir2Stage5?.state?.screen)`,
    "stage ready",
    25_000,
  );
  return client;
}

async function quickEnterWorld(client) {
  const screen = await client.evaluate(`window.__mir2Stage5?.state?.screen ?? null`);
  if (screen === "game") {
    return;
  }
  if (screen === "login") {
    await click(client, ".login-button.password button");
  } else if (screen === "select") {
    await client.evaluate(`window.__mir2Stage5?.send?.({ type: "startGame", characterIndex: 0 }) === true`);
  }
  await waitUntilClient(
    client,
    `(() => {
      const state = window.__mir2Stage5?.state;
      return state?.screen === "game" && state?.wsState === "open" && Boolean(state?.player);
    })()`,
    "quick enter game",
    waitTimeoutMs,
  );
}

async function readSummary(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      return {
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        reconnectStatus: state.reconnectStatus ?? null,
        accountId: state.accountId ?? null,
        selectedCharacterIndex: state.selectedCharacterIndex ?? null,
        player: state.player ?? null,
        playerObjectId: state.playerObjectId ?? null,
        mapFileName: state.mapFileName ?? null,
        worldTick: state.worldTick ?? null,
        logsTail: (state.logs ?? []).slice(-12).map((line) => line?.text ?? String(line)),
        gatewayEventsTail: (window.__mir2GatewayEventHistory ?? []).slice(-16),
      };
    })()
  `);
}

async function captureScreenshot(client, fileName) {
  const result = await client.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: false,
  });
  const screenshotPath = path.join(outputDir, fileName);
  await fs.writeFile(screenshotPath, Buffer.from(result.data, "base64"));
  return screenshotPath;
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
  if (!ok) {
    throw new Error(`Could not click ${selector}`);
  }
}

async function waitUntilClient(client, expression, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastValue = null;
  while (Date.now() < deadline) {
    lastValue = await client.evaluate(`Boolean(${expression})`).catch((error) => ({ error: String(error) }));
    if (lastValue === true) {
      return;
    }
    await delay(100);
  }
  const debug = await client
    .evaluate(`
      (() => ({
        url: location.href,
        readyState: document.readyState,
        bodyText: document.body?.innerText?.slice(0, 700) ?? "",
        screen: window.__mir2Stage5?.state?.screen ?? null,
        wsState: window.__mir2Stage5?.state?.wsState ?? null,
        reconnectStatus: window.__mir2Stage5?.state?.reconnectStatus ?? null,
        player: window.__mir2Stage5?.state?.player ?? null,
        logsTail: (window.__mir2Stage5?.state?.logs ?? []).slice(-8).map((line) => line?.text ?? String(line)),
      }))()
    `)
    .catch((error) => ({ debugError: String(error) }));
  throw new Error(`Timed out waiting for ${label}; last=${JSON.stringify(lastValue)}; debug=${JSON.stringify(debug).slice(0, 2_000)}`);
}

async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-reconnect-smoke-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      "--disable-gpu",
      "--no-proxy-server",
      "--proxy-bypass-list=*",
      "--no-first-run",
      "--no-default-browser-check",
      `--window-size=${DEFAULT_VIEWPORT.width},${DEFAULT_VIEWPORT.height}`,
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  chrome.userDataDir = userDataDir;
  await waitForChrome();
  return chrome;
}

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) {
    return;
  }
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve)).catch(() => undefined);
  if (chrome.userDataDir) {
    await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => undefined);
  }
}

async function createPageTarget() {
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/new?about:blank`, { method: "PUT" });
  if (!response.ok) {
    throw new Error(`Chrome target creation failed: ${response.status}`);
  }
  return response.json();
}

async function waitForChrome() {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/version`);
      if (response.ok) {
        return;
      }
    } catch {
      await delay(100);
    }
  }
  throw new Error("Timed out waiting for Chrome debug endpoint.");
}

async function setViewport(client) {
  await client.send("Emulation.setDeviceMetricsOverride", DEFAULT_VIEWPORT);
  await client.send("Emulation.setVisibleSize", {
    width: DEFAULT_VIEWPORT.width,
    height: DEFAULT_VIEWPORT.height,
  });
}

function buildBaseUrl(rawBaseUrl, wsUrl) {
  const url = new URL(rawBaseUrl);
  if (!url.searchParams.has("gatewayWs")) {
    url.searchParams.set("gatewayWs", wsUrl);
  }
  if (!url.searchParams.has("autoTick")) {
    url.searchParams.set("autoTick", "0");
  }
  if (!url.searchParams.has("codexBust")) {
    url.searchParams.set("codexBust", `reconnect-${runId}-${Date.now()}`);
  }
  return url.toString();
}

function isReconnectRelevantPayload(payloadData) {
  return /clientVersion|login|passkeyLogin|startGame|LoginSuccess|StartGame|UserInformation|Disconnect/i.test(
    payloadData,
  );
}

function isCriticalConsoleError(entry) {
  const text = String(entry.text ?? "");
  if (entry.source === "network" && /Failed to load resource/i.test(text)) {
    return false;
  }
  return !/favicon|ResizeObserver loop/i.test(text);
}

function isAllowedNetwork404(url) {
  return (
    /\/original-ui\/NPC\/94\/meta\.json(?:$|\?)/.test(url) ||
    /\/api\/original-ui-meta\?library=NPC%2F94(?:&|$)/.test(url)
  );
}

function findChromePath() {
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  ];
  return candidates.find((candidate) => fsSync.existsSync(candidate)) ?? null;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) {
      continue;
    }
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
  if (value === undefined || value === null || value === "") {
    return fallback;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") {
    return fallback;
  }
  if (typeof value === "boolean") {
    return value;
  }
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
