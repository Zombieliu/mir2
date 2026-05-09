import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const DEFAULT_BASE_URL = "http://127.0.0.1:3002";
const DEFAULT_OUTPUT_DIR = path.resolve(process.cwd(), "docs", "generated", "player-qa", "npc-click");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

const args = parseArgs(process.argv.slice(2));
const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL;
const outputDir = path.resolve(args.output ?? DEFAULT_OUTPUT_DIR);
const prefix = args.prefix ?? `npc-click-${Date.now()}`;
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? `npcqa${Date.now().toString().slice(-6)}`;
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "demo";
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const characterName = args.characterName ?? `NpcQa${Date.now().toString().slice(-5)}`;
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 800));
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const map = args.map ?? "0";
const x = numberArg(args.x, 326);
const y = numberArg(args.y, 270);
const npcPattern = args.npcPattern ?? "村庄向导|Village Guide";

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

    if (message.method === "Runtime.consoleAPICalled" && ["error", "warning"].includes(message.params?.type)) {
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
    await navigate(client, `${baseUrl}${baseUrl.includes("?") ? "&" : "?"}npcSmoke=${Date.now()}`);
    await login(client);
    await clearCommandHistory(client);
    await transferTo(client, map, x, y);
    await waitUntil(
      client,
      `
        (() => {
          const pattern = new RegExp(${JSON.stringify(npcPattern)});
          return window.__mir2Stage5?.state?.entities?.some?.((entity) =>
            entity?.kind === "npc" && pattern.test(entity?.name ?? "")
          );
        })()
      `,
      "NPC entity visible after transfer",
      20_000,
    );

    let before = await readNpcSmokeState(client, npcPattern);
    if (!before.npc) {
      throw new Error(`NPC not found before click: ${JSON.stringify(before)}`);
    }
    if (tileDistance(before.player, before.npc) > 1) {
      const adjacent = adjacentToNpc(before.player, before.npc);
      await transferTo(client, map, adjacent.x, adjacent.y);
      await waitUntil(
        client,
        `
          (() => {
            const pattern = new RegExp(${JSON.stringify(npcPattern)});
            return window.__mir2Stage5?.state?.entities?.some?.((entity) =>
              entity?.kind === "npc" && pattern.test(entity?.name ?? "")
            );
          })()
        `,
        "NPC entity visible after adjacent transfer",
        20_000,
      );
      before = await readNpcSmokeState(client, npcPattern);
    }
    await clearCommandHistory(client);

    const clicked = await clickNpc(client, npcPattern);
    if (!clicked) {
      throw new Error(`NPC DOM click failed for ${npcPattern}`);
    }

    await waitUntil(
      client,
      "window.__mir2Stage5?.state?.activeNpcDialog?.npcObjectId != null",
      "NPC dialog opened",
      10_000,
    );

    const after = await readNpcSmokeState(client, npcPattern);
    const result = {
      ok:
        Boolean(after.activeNpcDialog?.npcObjectId) &&
        after.interactCount === 1 &&
        after.moveCount === 0 &&
        client.consoleErrors.length === 0 &&
        client.network404s.length === 0,
      baseUrl,
      account,
      password,
      characterName,
      target: { map, x, y, npcPattern },
      before,
      after,
      consoleErrors: client.consoleErrors,
      nonFaviconNetwork404s: client.network404s,
    };
    const statePath = path.join(outputDir, `${prefix}.json`);
    await fs.writeFile(statePath, JSON.stringify(result, null, 2));
    console.log(
      JSON.stringify(
        {
          ok: result.ok,
          statePath,
          account,
          password,
          characterName,
          dialogTitle: after.activeNpcDialog?.title ?? null,
          interactCount: after.interactCount,
          moveCount: after.moveCount,
          consoleErrors: client.consoleErrors,
          nonFaviconNetwork404s: client.network404s,
        },
        null,
        2,
      ),
    );
    if (!result.ok) {
      process.exitCode = 1;
    }
  } finally {
    client?.close();
    await stopChrome(chrome).catch(() => undefined);
  }
}

async function login(client) {
  await waitUntil(
    client,
    "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)",
    "client stage ready",
    20_000,
  );

  let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  if (screen === "login") {
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);

    if (createAccount) {
      await click(client, ".login-button.account button");
      await waitUntil(client, "window.__mir2Stage5?.state?.wsState === 'open'", "account creation socket", 15_000);
      await delay(800);
    }

    await click(client, ".login-button.ok button");
    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'select'", "select screen", 15_000);
    screen = "select";
  }

  if (screen === "select" && createAccount) {
    const created = await client.evaluate(`
      window.__mir2Stage5?.send?.(${JSON.stringify({
        type: "newCharacter",
        name: characterName,
        gender: "male",
        class: "warrior",
      })}) === true
    `);
    if (!created) throw new Error(`Failed to create NPC QA character ${characterName}`);

    await waitUntil(
      client,
      `
        Array.isArray(window.__mir2Stage5?.state?.characters)
          && window.__mir2Stage5.state.characters.some((character) => character?.name === ${JSON.stringify(characterName)})
      `,
      "NPC QA character creation",
      15_000,
    );

    const started = await client.evaluate(`
      (() => {
        const state = window.__mir2Stage5?.state;
        const character = state?.characters?.find((entry) => entry?.name === ${JSON.stringify(characterName)});
        if (!character) return false;
        return window.__mir2Stage5?.send?.({ type: "startGame", characterIndex: character.index ?? 0 }) === true;
      })()
    `);
    if (!started) throw new Error(`Failed to start NPC QA character ${characterName}`);
  } else if (screen === "select") {
    await click(client, ".select-action.start button");
  }

  await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'game'", "game screen", 20_000);
  await waitUntil(client, "!document.querySelector('.login-transition-overlay')", "login transition cleared", 5_000);
}

async function clearCommandHistory(client) {
  await client.evaluate("window.__mir2CommandHistory = []");
}

async function transferTo(client, mapFileName, tileX, tileY) {
  const alreadyThere = await client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state;
      return state?.mapFileName === ${JSON.stringify(mapFileName)}
        && state?.player?.x === ${Number(tileX)}
        && state?.player?.y === ${Number(tileY)};
    })()
  `);
  if (alreadyThere) return;

  await client.evaluate(`
    window.__mir2Stage5?.send?.(${JSON.stringify({ type: "transferMap", key: `crystal:${mapFileName}:${tileX}:${tileY}` })}) === true
  `);
  await waitUntil(
    client,
    `
      (() => {
        const state = window.__mir2Stage5?.state;
        return state?.mapFileName === ${JSON.stringify(mapFileName)}
          && state?.player?.x === ${Number(tileX)}
          && state?.player?.y === ${Number(tileY)};
      })()
    `,
    "NPC smoke transfer",
    20_000,
  );
}

async function readNpcSmokeState(client, patternSource) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state;
      const pattern = new RegExp(${JSON.stringify(patternSource)});
      const commands = window.__mir2CommandHistory?.slice?.(0, 30) ?? [];
      const npc = state?.entities?.find?.((entity) => entity?.kind === "npc" && pattern.test(entity?.name ?? "")) ?? null;
      const icon = document.querySelector(".entity-quest-icon");
      const nameplate = [...document.querySelectorAll("button.entity-nameplate")]
        .find((node) => pattern.test(node.textContent ?? ""));
      return {
        player: state?.player ?? null,
        npc,
        activeNpcDialog: state?.activeNpcDialog ?? null,
        commands,
        interactCount: commands.filter((entry) => entry?.type === "interact").length,
        moveCount: commands.filter((entry) => ["moveTo", "walk", "run", "turn"].includes(entry?.type)).length,
        questIconCount: document.querySelectorAll(".entity-quest-icon").length,
        villageIconBounds:
          icon && nameplate
            ? {
                icon: rectToJson(icon.getBoundingClientRect()),
                nameplate: rectToJson(nameplate.getBoundingClientRect()),
              }
            : null,
      };

      function rectToJson(rect) {
        return {
          x: Math.round(rect.x * 100) / 100,
          y: Math.round(rect.y * 100) / 100,
          width: Math.round(rect.width * 100) / 100,
          height: Math.round(rect.height * 100) / 100,
        };
      }
    })()
  `);
}

async function clickNpc(client, patternSource) {
  return client.evaluate(`
    (() => {
      const pattern = new RegExp(${JSON.stringify(patternSource)});
      const nodes = [...document.querySelectorAll("button.entity-nameplate, button.entity-sprite-hit")];
      const node = nodes.find((entry) => pattern.test(entry.textContent ?? entry.getAttribute("aria-label") ?? ""));
      if (!node) return false;
      const rect = node.getBoundingClientRect();
      const clientX = rect.left + rect.width / 2;
      const clientY = rect.top + rect.height / 2;
      node.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true, button: 0, buttons: 1, clientX, clientY }));
      node.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true, button: 0, buttons: 0, clientX, clientY }));
      node.click();
      return true;
    })()
  `);
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

async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-npc-click-${process.pid}-${Date.now()}`);
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

async function waitUntil(client, expression, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastValue = null;
  while (Date.now() < deadline) {
    lastValue = await client.evaluate(`Boolean(${expression})`);
    if (lastValue) return;
    await delay(100);
  }
  const debug = await client
    .evaluate(`
      (() => ({
        url: location.href,
        readyState: document.readyState,
        title: document.title,
        stageScreen: window.__mir2Stage5?.state?.screen ?? null,
        stageKeys: window.__mir2Stage5?.state ? Object.keys(window.__mir2Stage5.state).slice(0, 20) : [],
        bodyText: document.body?.innerText?.slice(0, 500) ?? "",
        player: window.__mir2Stage5?.state?.player ?? null,
        entityNames: window.__mir2Stage5?.state?.entities?.map?.((entity) => entity?.name).slice(0, 20) ?? [],
        activeNpcDialog: window.__mir2Stage5?.state?.activeNpcDialog ?? null,
        commandHistory: window.__mir2CommandHistory?.slice?.(0, 20) ?? [],
      }))()
    `)
    .catch((error) => ({ debugError: String(error) }));
  throw new Error(`Timed out waiting for ${label}; last=${JSON.stringify(lastValue)}; debug=${JSON.stringify(debug)}`);
}

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) return;
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve));
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function tileDistance(left, right) {
  if (!left || !right) return Number.POSITIVE_INFINITY;
  return Math.max(Math.abs(Number(left.x) - Number(right.x)), Math.abs(Number(left.y) - Number(right.y)));
}

function adjacentToNpc(player, npc) {
  const dx = Math.sign(Number(player?.x ?? npc.x + 1) - Number(npc.x));
  const dy = Math.sign(Number(player?.y ?? npc.y) - Number(npc.y));
  if (dx !== 0 || dy !== 0) {
    return { x: Number(npc.x) + dx, y: Number(npc.y) + dy };
  }
  return { x: Number(npc.x) + 1, y: Number(npc.y) };
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = "1";
    } else {
      parsed[key] = next;
      index += 1;
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
  ];
  return candidates.find((candidate) => path.isAbsolute(candidate));
}

await main();
