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
const DEFAULT_OUTPUT_DIR = path.resolve(REPO_ROOT, "docs", "generated", "player-qa", "two-client-zone");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

const args = parseArgs(process.argv.slice(2));
const gatewayWsUrl = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? DEFAULT_GATEWAY_WS_URL;
const baseUrl = buildBaseUrl(args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL, gatewayWsUrl);
const outputDir = path.resolve(args.output ?? process.env.MIR2_TWO_CLIENT_ZONE_OUTPUT ?? DEFAULT_OUTPUT_DIR);
const runId = args.runId ?? new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
const prefix = args.prefix ?? `two-client-zone-${runId}`;
const password = args.password ?? process.env.MIR2_TWO_CLIENT_ZONE_PASSWORD ?? "zone-pass";
const map = args.map ?? process.env.MIR2_TWO_CLIENT_ZONE_MAP ?? "0";
const accountA = args.accountA ?? process.env.MIR2_TWO_CLIENT_ZONE_ACCOUNT_A ?? `zonea${runId}`;
const accountB = args.accountB ?? process.env.MIR2_TWO_CLIENT_ZONE_ACCOUNT_B ?? `zoneb${runId}`;
const characterA = (args.characterA ?? process.env.MIR2_TWO_CLIENT_ZONE_CHARACTER_A ?? `ZA${runId.slice(-6)}`).slice(0, 10);
const characterB = (args.characterB ?? process.env.MIR2_TWO_CLIENT_ZONE_CHARACTER_B ?? `ZB${runId.slice(-6)}`).slice(0, 10);
const startAx = numberArg(args.ax ?? process.env.MIR2_TWO_CLIENT_ZONE_AX, 330);
const startAy = numberArg(args.ay ?? process.env.MIR2_TWO_CLIENT_ZONE_AY, 270);
const startBx = numberArg(args.bx ?? process.env.MIR2_TWO_CLIENT_ZONE_BX, 332);
const startBy = numberArg(args.by ?? process.env.MIR2_TWO_CLIENT_ZONE_BY, 270);
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 500));
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

class CdpClient {
  constructor(wsUrl, label) {
    this.wsUrl = wsUrl;
    this.label = label;
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
      if (typeof payloadData === "string" && isZonePacketPayload(payloadData)) {
        this.packetFrames.push({
          direction: message.method.endsWith("Sent") ? "sent" : "received",
          payloadData: payloadData.slice(0, 800),
          at: Date.now(),
        });
        this.packetFrames = this.packetFrames.slice(-60);
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
      throw new Error(`${this.label} evaluate failed: ${result.exceptionDetails.text ?? JSON.stringify(result.exceptionDetails)}`);
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
  const clients = [];

  try {
    clients.push(await makeClient("A"), await makeClient("B"));
    const accounts = [
      { account: accountA, character: characterA, x: startAx, y: startAy },
      { account: accountB, character: characterB, x: startBx, y: startBy },
    ];

    await Promise.all(clients.map((client, index) => loginAndStart(client, accounts[index])));
    await Promise.all(clients.map((client, index) => transferTo(client, accounts[index].x, accounts[index].y)));
    await pulseBoth(clients, 4);

    await waitUntilClient(
      clients[0],
      `(() => (window.__mir2Stage5?.state?.entities ?? []).some((entity) => entity?.name === ${JSON.stringify(characterB)} && entity?.kind === "player"))()`,
      "A sees B",
      15_000,
    );
    await waitUntilClient(
      clients[1],
      `(() => (window.__mir2Stage5?.state?.entities ?? []).some((entity) => entity?.name === ${JSON.stringify(characterA)} && entity?.kind === "player"))()`,
      "B sees A",
      15_000,
    );

    await sendClientCommand(clients[0], { type: "walk", direction: "Right" }, "A walk right");
    await pulseBoth(clients, 5);
    await waitUntilClient(
      clients[1],
      `(() => (window.__mir2GatewayEventHistory ?? []).some((event) => event?.packet === "ObjectWalk" || event?.packet === "ObjectRun"))()`,
      "B receives A movement broadcast",
      15_000,
    );

    const chatMessage = `zone smoke ${runId}`;
    await sendClientCommand(clients[1], { type: "chat", message: chatMessage }, "B chat");
    await pulseBoth(clients, 3);
    await waitUntilClient(
      clients[0],
      `(() => (window.__mir2GatewayEventHistory ?? []).some((event) => event?.packet === "ObjectChat" && JSON.stringify(event?.payload ?? {}).includes(${JSON.stringify(chatMessage)})))()`,
      "A receives B chat broadcast",
      15_000,
    );

    const summaries = await Promise.all(clients.map((client) => readSummary(client)));
    const screenshots = [];
    for (let index = 0; index < clients.length; index += 1) {
      screenshots.push(await captureScreenshot(clients[index], `${prefix}-${index === 0 ? "a" : "b"}.png`));
    }

    const report = {
      ok: false,
      runId,
      baseUrl,
      gatewayWsUrl,
      map,
      accounts,
      summaries,
      screenshots,
      consoleErrors: clients.flatMap((client) =>
        client.consoleErrors.map((entry) => ({ client: client.label, ...entry })),
      ),
      nonFaviconNetwork404s: clients.flatMap((client) =>
        client.network404s.map((url) => ({ client: client.label, url })),
      ),
      packetFrames: clients.map((client) => ({
        client: client.label,
        frames: client.packetFrames.slice(-16),
      })),
    };
    report.assertions = {
      bothGame: summaries.every((summary) => summary.screen === "game"),
      aSeesB: summaries[0].entities.some((entity) => entity.name === characterB && entity.kind === "player"),
      bSeesA: summaries[1].entities.some((entity) => entity.name === characterA && entity.kind === "player"),
      bSawMovementBroadcast: clients[1].packetFrames.some((frame) => /ObjectWalk|ObjectRun/.test(frame.payloadData)),
      aSawChatBroadcast: clients[0].packetFrames.some((frame) => frame.payloadData.includes("ObjectChat") && frame.payloadData.includes(chatMessage)),
      noConsoleErrors: report.consoleErrors.length === 0,
      noNonFavicon404s: report.nonFaviconNetwork404s.length === 0,
    };
    report.ok = Object.values(report.assertions).every(Boolean);

    const reportPath = path.join(outputDir, `${prefix}.json`);
    await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
    console.log(
      JSON.stringify(
        {
          ok: report.ok,
          reportPath,
          assertions: report.assertions,
          screenshots,
        },
        null,
        2,
      ),
    );
    if (!report.ok) {
      process.exitCode = 1;
    }
  } finally {
    for (const client of clients) {
      client.close();
    }
    await stopChrome(chrome);
  }
}

async function makeClient(label) {
  const target = await createPageTarget();
  const client = new CdpClient(target.webSocketDebuggerUrl, label);
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
    `${label} page load`,
    20_000,
  );
  await waitUntilClient(
    client,
    `["login", "select", "game"].includes(window.__mir2Stage5?.state?.screen)`,
    `${label} stage ready`,
    25_000,
  );
  return client;
}

async function loginAndStart(client, accountInfo) {
  const screen = await client.evaluate(`window.__mir2Stage5?.state?.screen ?? null`);
  if (screen === "login") {
    await fillInput(client, ".login-input.account", accountInfo.account);
    await fillInput(client, ".login-input.password", password);
    await click(client, ".login-button.account button");
    await waitUntilClient(
      client,
      `window.__mir2Stage5?.state?.wsState === "open"`,
      `${client.label} account socket open`,
      15_000,
    );
    await delay(1_000);
    await click(client, ".login-button.ok button");
    await waitUntilClient(client, `window.__mir2Stage5?.state?.screen === "select"`, `${client.label} select`, 20_000);
  }

  await sendClientCommand(
    client,
    { type: "newCharacter", name: accountInfo.character, gender: "male", class: "warrior" },
    `${client.label} new character`,
  );
  await waitUntilClient(
    client,
    `(window.__mir2Stage5?.state?.characters ?? []).some((character) => character?.name === ${JSON.stringify(accountInfo.character)})`,
    `${client.label} character created`,
    15_000,
  );
  await client.evaluate(`
    (() => {
      const character = (window.__mir2Stage5?.state?.characters ?? []).find((entry) => entry?.name === ${JSON.stringify(accountInfo.character)});
      return window.__mir2Stage5?.send?.({ type: "startGame", characterIndex: character?.index ?? 0 }) === true;
    })()
  `);
  await waitUntilClient(
    client,
    `window.__mir2Stage5?.state?.screen === "game" && Boolean(window.__mir2Stage5?.state?.player)`,
    `${client.label} game`,
    25_000,
  );
}

async function transferTo(client, x, y) {
  await sendClientCommand(client, { type: "transferMap", key: `crystal:${map}:${x}:${y}` }, `${client.label} transfer`);
  await waitUntilClient(
    client,
    `(() => {
      const state = window.__mir2Stage5?.state;
      return state?.mapFileName === ${JSON.stringify(map)} && state?.player?.x === ${x} && state?.player?.y === ${y};
    })()`,
    `${client.label} transfer ${x},${y}`,
    20_000,
  );
}

async function pulseBoth(clients, count) {
  for (let index = 0; index < count; index += 1) {
    await Promise.all(clients.map((client) => sendClientCommand(client, { type: "tick" }, `${client.label} tick`)));
    await delay(450);
  }
}

async function sendClientCommand(client, command, label) {
  const ok = await client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
  if (!ok) {
    throw new Error(`${label} command was not accepted by the Web client.`);
  }
}

async function readSummary(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      return {
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        accountId: state.accountId ?? null,
        player: state.player ?? null,
        playerObjectId: state.playerObjectId ?? null,
        mapFileName: state.mapFileName ?? null,
        worldTick: state.worldTick ?? null,
        logsTail: (state.logs ?? []).slice(-8).map((line) => line?.text ?? String(line)),
        entities: (state.entities ?? [])
          .map((entity) => ({
            kind: entity.kind,
            name: entity.name,
            objectId: entity.objectId,
            x: entity.x,
            y: entity.y,
            direction: entity.direction,
          }))
          .sort((left, right) => String(left.name).localeCompare(String(right.name))),
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
  if (!ok) {
    throw new Error(`${client.label} could not fill ${selector}`);
  }
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
    throw new Error(`${client.label} could not click ${selector}`);
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
        bodyText: document.body?.innerText?.slice(0, 500) ?? "",
        screen: window.__mir2Stage5?.state?.screen ?? null,
        wsState: window.__mir2Stage5?.state?.wsState ?? null,
        player: window.__mir2Stage5?.state?.player ?? null,
        entities: (window.__mir2Stage5?.state?.entities ?? []).map((entity) => ({
          kind: entity.kind,
          name: entity.name,
          x: entity.x,
          y: entity.y,
        })),
      }))()
    `)
    .catch((error) => ({ debugError: String(error) }));
  throw new Error(`Timed out waiting for ${label}; last=${JSON.stringify(lastValue)}; debug=${JSON.stringify(debug).slice(0, 2_000)}`);
}

async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-two-client-zone-${process.pid}-${Date.now()}`);
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
    url.searchParams.set("codexBust", String(Date.now()));
  }
  return url.toString();
}

function isZonePacketPayload(payloadData) {
  return /ObjectPlayer|ObjectWalk|ObjectRun|ObjectTurn|ObjectChat|ObjectRemove|UserLocation/.test(payloadData);
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
