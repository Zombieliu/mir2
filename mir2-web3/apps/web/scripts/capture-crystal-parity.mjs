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
    if (args.openGameShop === "true") {
      await installCommandProbe(client);
      await click(client, ".hud-button.shop button");
      await waitUntil(client, "document.querySelector('.game-shop-window')", "game shop window", 5_000);
      await delay(750);
      if (args.buyGameShop === "true") {
        await click(client, ".game-shop-payment.credit");
        await delay(100);
        await click(client, ".game-shop-cell-buy .sprite-button");
        await delay(1_000);
      }
      if (args.previewGameShop === "true") {
        await click(client, ".game-shop-section.all .sprite-button");
        await delay(100);
        await clickGameShopCategory(client, "Mount");
        await delay(100);
        await click(client, ".game-shop-cell-preview .sprite-button");
        await waitUntil(client, "document.querySelector('.game-shop-viewer')", "game shop preview viewer", 5_000);
        await delay(500);
      }
    }
    if (args.openMail === "true") {
      await click(client, ".mini-map-button.mail button");
      await waitUntil(client, "document.querySelector('.mail-panel')", "mail panel", 5_000);
      await delay(500);
    }
    if (args.openBigMap === "true") {
      await click(client, ".mini-map-button.bigmap button");
      await waitUntil(client, "document.querySelector('.big-map-dialog')", "big map dialog", 5_000);
      await delay(500);
    }

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
  await fillTransferInputs(client, targetMap, String(targetX), String(targetY));
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

async function installCommandProbe(client) {
  await client.evaluate(`
    (() => {
      const sentKey = "__mir2QaSentCommands";
      window[sentKey] = [];

      if (!WebSocket.prototype.__mir2QaSendProbeWrapped) {
        const originalWebSocketSend = WebSocket.prototype.send;
        WebSocket.prototype.send = function(data) {
          try {
            const text =
              typeof data === "string"
                ? data
                : data instanceof ArrayBuffer
                  ? new TextDecoder().decode(data)
                  : "";
            const command = JSON.parse(text);
            window[sentKey].push({
              t: Date.now(),
              command,
            });
          } catch {
            // Best-effort QA probe.
          }
          return originalWebSocketSend.apply(this, arguments);
        };
        Object.defineProperty(WebSocket.prototype, "__mir2QaSendProbeWrapped", {
          value: true,
          configurable: true,
        });
      }

      const wrapStage = (stage) => {
        if (!stage || stage.__mir2QaSendProbeWrapped || typeof stage.send !== "function") return stage;
        const originalSend = stage.send;
        stage.send = function(command) {
          try {
            window[sentKey].push({
              t: Date.now(),
              command: JSON.parse(JSON.stringify(command)),
            });
          } catch {
            // Best-effort QA probe.
          }
          return originalSend.apply(this, arguments);
        };
        Object.defineProperty(stage, "__mir2QaSendProbeWrapped", {
          value: true,
          configurable: true,
        });
        return stage;
      };

      const descriptor = Object.getOwnPropertyDescriptor(window, "__mir2Stage5");
      if (descriptor?.get?.__mir2QaSendProbeAccessor) {
        wrapStage(window.__mir2Stage5);
        return true;
      }
      if (descriptor && descriptor.configurable === false) {
        wrapStage(window.__mir2Stage5);
        return false;
      }

      let currentStage = wrapStage(window.__mir2Stage5);
      const getter = function() {
        return currentStage;
      };
      getter.__mir2QaSendProbeAccessor = true;
      Object.defineProperty(window, "__mir2Stage5", {
        configurable: true,
        enumerable: true,
        get: getter,
        set: (nextStage) => {
          currentStage = wrapStage(nextStage);
        },
      });
      return true;
    })()
  `);
}

async function fillTransferInputs(client, map, x, y) {
  const ok = await client.evaluate(`
    (() => {
      const form = document.querySelector(".system-menu-qa-transfer");
      const inputs = Array.from(form?.querySelectorAll("input") ?? []);
      if (inputs.length < 3) return false;
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      const values = ${JSON.stringify([map, x, y])};
      inputs.slice(0, 3).forEach((input, index) => {
        setter.call(input, values[index]);
        input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText" }));
        input.dispatchEvent(new Event("change", { bubbles: true }));
      });
      return true;
    })()
  `);
  if (!ok) throw new Error("Could not fill QA transfer inputs");
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

async function clickGameShopCategory(client, categoryName) {
  const ok = await client.evaluate(`
    (() => {
      const category = ${JSON.stringify(categoryName)};
      const node = Array.from(document.querySelectorAll(".game-shop-categories button"))
        .find((button) => (button.textContent ?? "").trim() === category);
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!ok) throw new Error(`Could not click game shop category ${categoryName}`);
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
      const gameShop = document.querySelector(".game-shop-window")?.getBoundingClientRect();
      const mailPanel = document.querySelector(".mail-panel")?.getBoundingClientRect();
      const bigMap = document.querySelector(".big-map-dialog")?.getBoundingClientRect();
      const hudHealthOnlyLabel = document.querySelector(".hud-health-only-label");
      const chatLines = Array.from(document.querySelectorAll(".chat-feed-line")).map((node) => node.textContent ?? "");
      const state = window.__mir2Stage5?.state ?? {};
      const inventoryItems = state.world?.inventoryItems ?? state.inventoryItems ?? [];
      const beltItems = state.world?.beltItems ?? state.beltItems ?? [];
      const storageItems = state.world?.storageItems ?? state.storageItems ?? [];
      const equipmentItems = state.world?.equipmentItems ?? state.equipmentItems ?? [];
      const questLog = state.world?.questLog ?? state.questLog ?? [];
      const knownSkills = state.world?.knownSkills ?? state.knownSkills ?? [];
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
      const entities = state.entities ?? [];
      const nameplateNodes = Array.from(document.querySelectorAll(".entity-nameplate"));
      const stageCursor = stage ? getComputedStyle(document.querySelector(".client-stage-frame")).cursor : null;
      return {
        screen: state.screen ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        player: state.player ?? null,
        playerHp: state.world?.playerHp ?? state.playerHp ?? null,
        playerMaxHp: state.world?.playerMaxHp ?? state.playerMaxHp ?? null,
        playerMp: state.world?.playerMp ?? state.playerMp ?? null,
          gold: state.world?.gold ?? state.gold ?? null,
          credit: state.world?.credit ?? state.credit ?? null,
          inventoryItemCount: inventoryItems.length,
        beltItemCount: beltItems.length,
        storageItemCount: storageItems.length,
        equipmentItemCount: equipmentItems.length,
        questCount: questLog.length,
        skillCount: knownSkills.length,
        hudHealthOnlyLabel: hudHealthOnlyLabel?.textContent ?? null,
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
          gameShop: readGameShopState(rect, gameShop),
        mailPanel: readMailState(rect, mailPanel),
        bigMap: readBigMapState(rect, bigMap),
        cursor: {
          stage: stageCursor,
          npcHit: document.querySelector(".entity-sprite-stack.npc .entity-sprite-hit")
            ? getComputedStyle(document.querySelector(".entity-sprite-stack.npc .entity-sprite-hit")).cursor
            : null,
          monsterHit: document.querySelector(".entity-sprite-stack.monster .entity-sprite-hit")
            ? getComputedStyle(document.querySelector(".entity-sprite-stack.monster .entity-sprite-hit")).cursor
            : null,
        },
      };

      function readGameShopState(rect, gameShopBounds) {
        const cells = Array.from(document.querySelectorAll(".game-shop-cell-frame"));
        const icons = Array.from(document.querySelectorAll(".game-shop-cell-icon"));
        const sentCommands = Array.isArray(window.__mir2QaSentCommands) ? window.__mir2QaSentCommands : [];
        return {
          visible: Boolean(document.querySelector(".game-shop-window")),
          inventoryVisible: Boolean(document.querySelector(".inventory-window")),
          bounds: rect(gameShopBounds),
          cellCount: cells.length,
          firstCellBounds: rect(cells[0]?.getBoundingClientRect()),
          firstCellName: document.querySelector(".game-shop-cell-name")?.textContent ?? null,
          firstCellCreditPrice: document.querySelector(".game-shop-cell-credit-price")?.textContent ?? null,
          firstCellGoldPrice: document.querySelector(".game-shop-cell-gold-price")?.textContent ?? null,
          categoryCount: document.querySelectorAll(".game-shop-categories button").length,
          categories: Array.from(document.querySelectorAll(".game-shop-categories button")).map((node) => node.textContent ?? ""),
          pageLabel: document.querySelector(".game-shop-page")?.textContent ?? null,
          loadedIconCount: icons.filter((icon) => icon.complete && icon.naturalWidth > 0).length,
          iconSources: icons.slice(0, 8).map((icon) => icon.getAttribute("src")),
          buyButtonCount: document.querySelectorAll(".game-shop-cell-buy .sprite-button").length,
          previewButtonCount: document.querySelectorAll(".game-shop-cell-preview .sprite-button").length,
          previewViewerVisible: Boolean(document.querySelector(".game-shop-viewer")),
          previewViewerBounds: rect(document.querySelector(".game-shop-viewer")?.getBoundingClientRect()),
          previewViewerItemName: document.querySelector(".game-shop-viewer")?.getAttribute("data-item-name") ?? null,
          previewViewerDirection: document.querySelector(".game-shop-viewer")?.getAttribute("data-direction") ?? null,
          paymentGoldBox: document.querySelector(".game-shop-payment.gold img")?.getAttribute("src") ?? null,
          paymentCreditBox: document.querySelector(".game-shop-payment.credit img")?.getAttribute("src") ?? null,
          sentCommandTail: sentCommands.slice(-8),
          oldPlaceholderCellCount: document.querySelectorAll(".game-shop-cell").length,
        };
      }

      function readMailState(rect, mailBounds) {
        const rowNodes = Array.from(document.querySelectorAll(".mail-row"));
        const overlayHead = document.querySelector(".mail-panel > .overlay-panel-head");
        return {
          visible: Boolean(document.querySelector(".mail-panel")),
          bounds: rect(mailBounds),
          rowCount: rowNodes.length,
          rowTexts: rowNodes.slice(0, 10).map((node) => node.textContent?.trim() ?? ""),
          pageLabel: document.querySelector(".mail-page-label")?.textContent ?? null,
          hasFrame: Boolean(document.querySelector(".mail-frame")),
          emptyVisible: Boolean(document.querySelector(".mail-empty")),
          visibleOverlayHead: overlayHead ? (() => {
            const box = overlayHead.getBoundingClientRect();
            const style = getComputedStyle(overlayHead);
            return style.display !== "none" && style.visibility !== "hidden" && box.width > 0 && box.height > 0 && box.left >= 0 && box.top >= 0;
          })() : false,
          oldOverlayRowCount: document.querySelectorAll(".mail-panel .overlay-panel-list > .overlay-panel-row").length,
        };
      }

      function readBigMapState(rect, bigMapBounds) {
        return {
          visible: Boolean(document.querySelector(".big-map-dialog")),
          bounds: rect(bigMapBounds),
          viewport: rect(document.querySelector(".big-map-viewport")?.getBoundingClientRect()),
          npcRowCount: document.querySelectorAll(".big-map-npc-row").length,
          npcRows: Array.from(document.querySelectorAll(".big-map-npc-row")).slice(0, 10).map((node) => ({
            text: node.textContent?.trim() ?? "",
            icon: node.querySelector(".big-map-npc-icon")?.getAttribute("src") ?? null,
          })),
          dotCount: document.querySelectorAll(".big-map-dot").length,
          hasFrame: Boolean(document.querySelector(".big-map-frame")),
          hasRaster: Boolean(document.querySelector(".big-map-raster")),
          title: document.querySelector(".big-map-title")?.textContent ?? null,
          coordinate: document.querySelector(".big-map-coordinate")?.textContent ?? null,
        };
      }
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
