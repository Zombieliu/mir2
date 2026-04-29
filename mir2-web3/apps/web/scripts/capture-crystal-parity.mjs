import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const DEFAULT_BASE_URL = "http://127.0.0.1:3002";
const DEFAULT_OUTPUT_DIR = path.resolve(process.cwd(), "..", "..", "docs", "generated", "player-qa");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };
const DEFAULT_ACCOUNT = "QA0429A";
const DEFAULT_PASSWORD = "Mir2test1";
const DEFAULT_MAP = "0";
const DEFAULT_X = 287;
const DEFAULT_Y = 618;

const args = parseArgs(process.argv.slice(2));
const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL;
const outputDir = path.resolve(args.output ?? DEFAULT_OUTPUT_DIR);
const prefix = args.prefix ?? `crystal-parity-${Date.now()}`;
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? DEFAULT_ACCOUNT;
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? DEFAULT_PASSWORD;
const map = args.map ?? DEFAULT_MAP;
const x = numberArg(args.x, DEFAULT_X);
const y = numberArg(args.y, DEFAULT_Y);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9400 + (process.pid % 1000));

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
      this.consoleErrors.push({
        source: "exception",
        text: message.params?.exceptionDetails?.text ?? "runtime exception",
      });
    }

    if (message.method === "Log.entryAdded") {
      const entry = message.params?.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        const url = entry.url ? ` (${entry.url})` : "";
        this.consoleErrors.push({ source: entry.source ?? "log", text: `${entry.text ?? ""}${url}` });
      }
    }

    if (message.method === "Network.responseReceived") {
      const response = message.params?.response;
      if (response?.status === 404 && !String(response.url ?? "").includes("favicon")) {
        this.network404s.push(response.url);
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
      throw new Error(`Evaluation failed: ${result.exceptionDetails.text ?? JSON.stringify(result.exceptionDetails)}`);
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
    const wsUrl = await createPageTarget();
    client = new CdpClient(wsUrl);
    await client.connect();
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await client.send("Page.enable");
    await setViewport(client, DEFAULT_VIEWPORT);
    await navigate(client, baseUrl);

    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'login'", "login screen", 15_000);
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    await click(client, ".login-button.ok button");
    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'select'", "select screen", 15_000);
    await click(client, ".select-action.start button");
    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'game'", "game screen", 20_000);
    await transferIfNeeded(client, map, x, y);
    await waitUntil(client, "!document.querySelector('.login-transition-overlay')", "login transition cleared", 5_000);
    await delay(750);

    const state = await readState(client);
    const screenshotPath = path.join(outputDir, `${prefix}.png`);
    const statePath = path.join(outputDir, `${prefix}-state.json`);
    const screenshot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    await fs.writeFile(screenshotPath, Buffer.from(screenshot.data, "base64"));
    await fs.writeFile(
      statePath,
      `${JSON.stringify(
        {
          ...state,
          network404Count: client.network404s.length,
          consoleErrorCount: client.consoleErrors.length,
          nonFaviconNetwork404s: [...new Set(client.network404s)],
          consoleErrors: client.consoleErrors,
          screenshot: path.relative(process.cwd(), screenshotPath).replaceAll("\\", "/"),
        },
        null,
        2,
      )}\n`,
    );

    console.log(JSON.stringify({ ok: true, screenshotPath, statePath }, null, 2));
  } finally {
    client?.close();
    await stopChrome(chrome);
    await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => undefined);
  }
}

async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-crystal-parity-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      "--headless=new",
      "--disable-gpu",
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

async function createPageTarget() {
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/new?about:blank`, { method: "PUT" });
  if (!response.ok) throw new Error(`Chrome target creation failed: ${response.status}`);
  const target = await response.json();
  return target.webSocketDebuggerUrl;
}

async function waitForChrome() {
  const deadline = Date.now() + 10_000;
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
  await client.send("Page.navigate", { url });
  await waitUntil(client, "document.readyState === 'complete' || document.readyState === 'interactive'", "page load", 15_000);
}

async function transferIfNeeded(client, targetMap, targetX, targetY) {
  const alreadyThere = await client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state;
      return state?.mapFileName === ${JSON.stringify(targetMap)}
        && state?.player?.x === ${Number(targetX)}
        && state?.player?.y === ${Number(targetY)};
    })()
  `);
  if (alreadyThere) return;

  await click(client, ".hud-button.menu button");
  await waitUntil(client, "document.querySelector('.system-menu-qa-transfer')", "qa transfer panel", 5_000);
  await fillInput(client, ".system-menu-qa-transfer input:nth-of-type(1)", targetMap);
  await fillInput(client, ".system-menu-qa-transfer input:nth-of-type(2)", String(targetX));
  await fillInput(client, ".system-menu-qa-transfer input:nth-of-type(3)", String(targetY));
  await click(client, ".system-menu-qa-transfer button[type='submit']");
  await waitUntil(
    client,
    `
      (() => {
        const state = window.__mir2Stage5?.state;
        return state?.mapFileName === ${JSON.stringify(targetMap)}
          && state?.player?.x === ${Number(targetX)}
          && state?.player?.y === ${Number(targetY)};
      })()
    `,
    "target scene",
    20_000,
  );
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
    lastValue = await client.evaluate(`Boolean(${expression})`);
    if (lastValue) return;
    await delay(100);
  }
  throw new Error(`Timed out waiting for ${label}; last=${JSON.stringify(lastValue)}`);
}

async function readState(client) {
  return client.evaluate(`
    (() => {
      const stage = document.querySelector(".client-stage-frame")?.getBoundingClientRect();
      const miniMap = document.querySelector(".mini-map-panel")?.getBoundingClientRect();
      const hud = document.querySelector(".main-hud-shell")?.getBoundingClientRect();
      const chat = document.querySelector(".chat-frame")?.getBoundingClientRect();
      const duraPanel = document.querySelector(".dura-panel")?.getBoundingClientRect();
      const chatLines = Array.from(document.querySelectorAll(".chat-feed-line")).map((node) => node.textContent ?? "");
      const nextDevPortals = Array.from(document.querySelectorAll("nextjs-portal"));
      const visibleNextDevPortals = nextDevPortals.filter((node) => {
        const box = node.getBoundingClientRect();
        const style = getComputedStyle(node);
        return style.display !== "none" && style.visibility !== "hidden" && box.width > 0 && box.height > 0;
      });
      const rect = (value) => value ? ({
        left: value.left,
        top: value.top,
        right: value.right,
        bottom: value.bottom,
        width: value.width,
        height: value.height,
      }) : null;
      const state = window.__mir2Stage5?.state ?? {};
      const entities = state.entities ?? [];
      const nameplateNodes = Array.from(document.querySelectorAll(".entity-nameplate"));
      return {
        screen: state.screen ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        player: state.player ?? null,
        logs: state.logs ?? [],
        transitionOverlayVisible: Boolean(document.querySelector(".login-transition-overlay")),
        entityCount: entities.length,
        npcCount: entities.filter((entity) => entity.kind === "npc").length,
        monsterCount: entities.filter((entity) => entity.kind === "monster").length,
        visibleNameplates: nameplateNodes.map((node) => node.innerText?.trim() ?? node.textContent?.trim() ?? ""),
        visibleNameplateDetails: nameplateNodes.map((node) => ({
          text: node.innerText?.trim() ?? node.textContent?.trim() ?? "",
          color: getComputedStyle(node.querySelector("strong") ?? node).color,
          secondaryColor: node.querySelector(".entity-subname")
            ? getComputedStyle(node.querySelector(".entity-subname")).color
            : null,
          classes: node.className,
          rect: rect(node.getBoundingClientRect()),
        })),
        questMarkerCount: document.querySelectorAll(".entity-quest-icon").length,
        entityHealthBarCount: document.querySelectorAll(".entity-health-bar").length,
        visibleChatLines: chatLines,
        nextDevPortalCount: nextDevPortals.length,
        nextDevIndicatorVisible: visibleNextDevPortals.length > 0,
        stage: rect(stage),
        hud: rect(hud),
        miniMap: rect(miniMap),
        chat: rect(chat),
        duraPanel: rect(duraPanel),
      };
    })()
  `);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const value = argv[index + 1]?.startsWith("--") || argv[index + 1] === undefined ? "true" : argv[++index];
    parsed[key] = value;
  }
  return parsed;
}

function numberArg(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function findChromePath() {
  const candidates = [
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    path.join(process.env.LOCALAPPDATA ?? "", "Google\\Chrome\\Application\\chrome.exe"),
  ];
  return candidates.find(Boolean);
}

async function stopChrome(chrome) {
  if (chrome.exitCode !== null) return;
  chrome.kill();
  await new Promise((resolve) => {
    const timer = setTimeout(resolve, 2_000);
    chrome.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

await main();
