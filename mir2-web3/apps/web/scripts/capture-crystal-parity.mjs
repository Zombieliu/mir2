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
const DEFAULT_SETTLE_MS = 5_000;
const DEFAULT_VISUAL_READY_TIMEOUT_MS = 30_000;

const args = parseArgs(process.argv.slice(2));
const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL;
const outputDir = path.resolve(args.output ?? DEFAULT_OUTPUT_DIR);
const prefix = args.prefix ?? `crystal-parity-${Date.now()}`;
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? DEFAULT_ACCOUNT;
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? DEFAULT_PASSWORD;
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, false);
const characterName = args.characterName ?? account;
const map = args.map ?? DEFAULT_MAP;
const x = numberArg(args.x, DEFAULT_X);
const y = numberArg(args.y, DEFAULT_Y);
const targetTolerance = Math.max(0, numberArg(args.targetTolerance, 0));
const settleMs = numberArg(args.settleMs ?? process.env.MIR2_CAPTURE_SETTLE_MS, DEFAULT_SETTLE_MS);
const visualReadyTimeoutMs = numberArg(
  args.visualReadyTimeoutMs ?? process.env.MIR2_CAPTURE_VISUAL_READY_TIMEOUT_MS,
  DEFAULT_VISUAL_READY_TIMEOUT_MS,
);
const suppressTutorial = args.suppressTutorial !== "false";
const qaControlToken = args.qaControlToken ?? process.env.MIR2_QA_CONTROL_TOKEN ?? null;
const qaCharacterStatePath = args.qaCharacterState ?? args.qaState ?? process.env.MIR2_QA_CHARACTER_STATE ?? null;
const cdpCommandTimeoutMs = numberArg(
  args.cdpCommandTimeoutMs ?? process.env.MIR2_CDP_COMMAND_TIMEOUT_MS,
  15_000,
);
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
    this.webSocketFramesSent = [];
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

    if (message.method === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({
        requestId: message.params?.requestId ?? null,
        payloadData: message.params?.response?.payloadData ?? "",
        at: Date.now(),
      });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-50);
    }

    if (message.method === "Network.webSocketFrameReceived") {
      this.webSocketFramesReceived.push({
        requestId: message.params?.requestId ?? null,
        payloadData: message.params?.response?.payloadData ?? "",
        at: Date.now(),
      });
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-50);
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        if (!this.pending.has(id)) return;
        this.pending.delete(id);
        reject(new Error(`CDP command timed out after ${cdpCommandTimeoutMs}ms: ${method}`));
      }, cdpCommandTimeoutMs);
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
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

  criticalConsoleErrors() {
    return this.consoleErrors.filter(isCriticalConsoleError);
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
    await seedCaptureLocalStorage(client);

    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'login'", "login screen", 15_000);
    await waitUntil(client, "document.querySelector('.login-input.account') && document.querySelector('.login-input.password')", "login inputs", 10_000);
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    if (createAccount) {
      await click(client, ".login-button.account button");
      await waitUntil(client, "window.__mir2Stage5?.state?.wsState === 'open'", "account creation socket", 15_000);
      await delay(2_000);
    }
    const loginEvidence = await loginWithPassword(client, account, password);
    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'select'", "select screen", 15_000);
    await waitUntil(client, "window.__mir2Stage5?.state?.wsState === 'open'", "select socket open", 15_000);
    if (createAccount) {
      await ensureCharacter(client, characterName);
      await startCharacterByName(client, characterName);
    } else {
      await startSelectedCharacter(client);
    }
    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'game'", "game screen", 60_000);
    const stateAlignmentEvidence = await applyQaCharacterStateIfConfigured(client);
    const transferEvidence = await transferIfNeeded(client, map, x, y);
    await waitUntil(client, "!document.querySelector('.login-transition-overlay')", "login transition cleared", 5_000);
    await waitForGameVisualReadiness(client, visualReadyTimeoutMs);
    await waitForAnimationFrames(client, 2);
    await delay(settleMs);
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
    const captureEvidence = redactCaptureSecrets({
      ...state,
      network404Count: client.network404s.length,
      consoleErrorCount: client.consoleErrors.length,
      criticalConsoleErrorCount: client.criticalConsoleErrors().length,
      nonFaviconNetwork404s: [...new Set(client.network404s)],
      consoleErrors: client.consoleErrors,
      criticalConsoleErrors: client.criticalConsoleErrors(),
      captureControl: {
        login: loginEvidence,
        stateAlignment: stateAlignmentEvidence,
        transfer: transferEvidence,
      },
      screenshot: path.relative(process.cwd(), screenshotPath).replaceAll("\\", "/"),
    });
    await fs.writeFile(
      statePath,
      `${JSON.stringify(captureEvidence, null, 2)}\n`,
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
      "--ignore-gpu-blocklist",
      "--enable-unsafe-webgpu",
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
  const existing = await fetch(`http://127.0.0.1:${debugPort}/json/list`)
    .then((response) => (response.ok ? response.json() : []))
    .catch(() => []);
  const pageTarget = existing.find((target) => target.type === "page" && target.webSocketDebuggerUrl);
  if (pageTarget?.webSocketDebuggerUrl) {
    return pageTarget.webSocketDebuggerUrl;
  }

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

async function seedCaptureLocalStorage(client) {
  if (!suppressTutorial) return;
  await client.evaluate(`
    (() => {
      try {
        window.localStorage.setItem("mir2:tutorialCompleted", "1");
      } catch {
        // Non-fatal in opaque/headless storage contexts.
      }
      return true;
    })()
  `);
}

async function applyQaCharacterStateIfConfigured(client) {
  if (!qaCharacterStatePath) {
    return {
      ok: true,
      mode: "skipped",
      note: "No QA character state path was provided.",
    };
  }
  if (!qaControlToken) {
    throw new Error("--qaCharacterState/--qaState requires MIR2_QA_CONTROL_TOKEN or --qaControlToken.");
  }

  const rawPayload = await readJsonFile(path.resolve(qaCharacterStatePath));
  const payload = rawPayload.qaCharacterState ?? rawPayload.characterState ?? rawPayload;
  const expected = {
    mapFileName: payload.mapFileName ?? payload.map_file_name ?? null,
    x: payload.position?.x ?? payload.x ?? null,
    y: payload.position?.y ?? payload.y ?? null,
    hp: Number(payload.hp),
    maxHp: Number(payload.maxHp ?? payload.max_hp),
    mp: Number(payload.mp),
    maxMp: payload.maxMp == null && payload.max_mp == null ? null : Number(payload.maxMp ?? payload.max_mp),
    experience: payload.experience == null ? null : Number(payload.experience),
    maxExperience: payload.maxExperience == null && payload.max_experience == null
      ? null
      : Number(payload.maxExperience ?? payload.max_experience),
    gold: Number(payload.gold),
    inventoryItemCount: (payload.inventoryItemsJson ?? payload.inventory_items_json ?? []).length,
    beltItemCount: (payload.beltItemsJson ?? payload.belt_items_json ?? []).length,
    storageItemCount: (payload.storageItemsJson ?? payload.storage_items_json ?? []).length,
    equipmentItemCount: (payload.equipmentItemsJson ?? payload.equipment_items_json ?? []).length,
  };
  const before = await readQaAlignmentState(client);
  const probe = await sendCommandProbe(client, {
    type: "qaControl",
    token: qaControlToken,
    action: {
      type: "stage5Command",
      action: "qa.applyNativeState",
      args: [JSON.stringify(payload)],
    },
  });
  if (!probe?.sent) {
    throw new Error(`qaControl qa.applyNativeState was not accepted by the browser bridge: ${JSON.stringify(probe)}`);
  }

  await waitUntil(
    client,
    `
      (() => {
        const state = window.__mir2Stage5?.state ?? {};
        const world = state.world ?? {};
        const inventoryItems = world.inventoryItems ?? state.inventoryItems ?? [];
        const beltItems = world.beltItems ?? state.beltItems ?? [];
        const storageItems = world.storageItems ?? state.storageItems ?? [];
        const equipmentItems = world.equipmentItems ?? state.equipmentItems ?? [];
        const player = state.player ?? world.player ?? {};
        const playerHp = world.playerHp ?? state.playerHp ?? null;
        const playerMaxHp = world.playerMaxHp ?? state.playerMaxHp ?? null;
        const playerMp = world.playerMp ?? state.playerMp ?? null;
        const playerMaxMp = world.playerMaxMp ?? state.playerMaxMp ?? null;
        const gold = world.gold ?? state.gold ?? null;
        const expected = ${JSON.stringify(expected)};
        const positionMatches = expected.mapFileName == null || (
          state.mapFileName === String(expected.mapFileName)
            && Number(player?.x) === Number(expected.x)
            && Number(player?.y) === Number(expected.y)
        );
        return state.screen === "game"
          && positionMatches
          && Number(playerHp) === expected.hp
          && Number(playerMaxHp) === expected.maxHp
          && Number(playerMp) === expected.mp
          && (expected.maxMp == null || Number(playerMaxMp) === expected.maxMp)
          && Number(gold) === expected.gold
          && inventoryItems.length === expected.inventoryItemCount
          && beltItems.length === expected.beltItemCount
          && storageItems.length === expected.storageItemCount
          && equipmentItems.length === expected.equipmentItemCount;
      })()
    `,
    "QA native character state applied",
    30_000,
  );

  return {
    ok: true,
    mode: "qaControl.stage5Command",
    action: "qa.applyNativeState",
    qaCharacterStatePath: path.resolve(qaCharacterStatePath),
    expected,
    before,
    after: await readQaAlignmentState(client),
    probe,
  };
}

async function transferIfNeeded(client, targetMap, targetX, targetY) {
  const startedAt = Date.now();
  const before = await readPositionState(client);
  if (isTargetPosition(before, targetMap, targetX, targetY, targetTolerance)) {
    return {
      ok: true,
      mode: "alreadyAtTarget",
      target: { map: String(targetMap), x: Number(targetX), y: Number(targetY) },
      before,
      after: before,
      settleMs: 0,
    };
  }

  let probe = null;
  let mode = "uiTransferPanel";

  if (qaControlToken) {
    mode = "qaControl.transferMap";
    probe = await sendCommandProbe(client, {
      type: "qaControl",
      token: qaControlToken,
      action: {
        type: "transferMap",
        key: `crystal:${targetMap}:${Number(targetX)}:${Number(targetY)}`,
      },
    });
    if (!probe?.sent) {
      throw new Error(`qaControl transferMap was not accepted by the browser bridge: ${JSON.stringify(probe)}`);
    }
  } else {
    probe = { sent: true, qaControl: false, note: "MIR2_QA_CONTROL_TOKEN is not configured; used legacy UI transfer panel" };
    await click(client, ".hud-button.menu button");
    await waitUntil(client, "document.querySelector('.system-menu-qa-transfer')", "qa transfer panel", 5_000);
    await fillTransferInputs(client, targetMap, String(targetX), String(targetY));
    await click(client, ".system-menu-qa-transfer button[type='submit']");
  }

  await waitUntil(
    client,
    `
      (() => {
        const state = window.__mir2Stage5?.state;
        return state?.mapFileName === ${JSON.stringify(targetMap)}
          && Math.abs((state?.player?.x ?? NaN) - ${Number(targetX)}) <= ${targetTolerance}
          && Math.abs((state?.player?.y ?? NaN) - ${Number(targetY)}) <= ${targetTolerance};
      })()
    `,
    "target scene",
    20_000,
  );
  const after = await waitForStableTargetPosition(client, targetMap, targetX, targetY, targetTolerance);
  return {
    ok: true,
    mode,
    target: { map: String(targetMap), x: Number(targetX), y: Number(targetY) },
    before,
    after,
    probe,
    settleMs: Date.now() - startedAt,
  };
}

async function waitForStableTargetPosition(client, targetMap, targetX, targetY, tolerance = 0, settleMs = 500, timeoutMs = 5_000) {
  const deadline = Date.now() + timeoutMs;
  let stableSince = null;
  let latest = null;
  while (Date.now() < deadline) {
    latest = await readPositionState(client);
    if (isTargetPosition(latest, targetMap, targetX, targetY, tolerance)) {
      stableSince ??= Date.now();
      if (Date.now() - stableSince >= settleMs) return latest;
    } else {
      stableSince = null;
    }
    await delay(100);
  }
  return latest;
}

function isTargetPosition(state, targetMap, targetX, targetY, tolerance = 0) {
  return (
    state?.mapFileName === String(targetMap) &&
    Math.abs(Number(state?.player?.x) - Number(targetX)) <= tolerance &&
    Math.abs(Number(state?.player?.y) - Number(targetY)) <= tolerance
  );
}

async function readPositionState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      return {
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        player: state.player ?? null,
        authoritativePlayer: state.world?.authoritativePlayer ?? state.authoritativePlayer ?? null,
        lastCommand: state.lastCommand ?? null,
        lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
      };
    })()
  `);
}

async function readQaAlignmentState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const world = state.world ?? {};
      const inventoryItems = world.inventoryItems ?? state.inventoryItems ?? [];
      const beltItems = world.beltItems ?? state.beltItems ?? [];
      const storageItems = world.storageItems ?? state.storageItems ?? [];
      const equipmentItems = world.equipmentItems ?? state.equipmentItems ?? [];
      return {
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        player: state.player ?? null,
        playerHp: world.playerHp ?? state.playerHp ?? null,
        playerMaxHp: world.playerMaxHp ?? state.playerMaxHp ?? null,
        playerMp: world.playerMp ?? state.playerMp ?? null,
        playerMaxMp: world.playerMaxMp ?? state.playerMaxMp ?? null,
        playerExperience: world.playerExperience ?? state.playerExperience ?? null,
        playerMaxExperience: world.playerMaxExperience ?? state.playerMaxExperience ?? null,
        gold: world.gold ?? state.gold ?? null,
        currentWeight: world.currentWeight ?? state.currentWeight ?? null,
        maxWeight: world.maxWeight ?? state.maxWeight ?? null,
        freeBagSlots: world.freeBagSlots ?? state.freeBagSlots ?? null,
        maxBagSlots: world.maxBagSlots ?? state.maxBagSlots ?? null,
        inventoryItemCount: inventoryItems.length,
        beltItemCount: beltItems.length,
        storageItemCount: storageItems.length,
        equipmentItemCount: equipmentItems.length,
      };
    })()
  `);
}

async function sendCommandProbe(client, command) {
  return client.evaluate(`
    (() => {
      const bridge = window.__mir2Stage5;
      const stateBefore = bridge?.state ?? {};
      const sent = bridge?.send?.(${JSON.stringify(command)}) === true;
      const stateAfter = bridge?.state ?? {};
      return {
        sent,
        commandType: ${JSON.stringify(command.type ?? null)},
        screenBefore: stateBefore.screen ?? null,
        wsStateBefore: stateBefore.wsState ?? null,
        screenAfter: stateAfter.screen ?? null,
        wsStateAfter: stateAfter.wsState ?? null,
        lastCommand: stateAfter.lastCommand ?? null,
        lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
        at: Date.now(),
      };
    })()
  `);
}

async function waitForGameVisualReadiness(client, timeoutMs) {
  await waitUntil(
    client,
    `
      (() => {
        const state = window.__mir2Stage5?.state;
        if (state?.screen !== "game") return false;
        const sceneReady =
          state.sceneInteractionReady === true ||
          state.sceneAssetReadiness?.visualReady === true ||
          state.sceneAssetReadiness?.ready === true;
        if (!sceneReady) return false;

        const mapDebug = window.__mir2BevyMapRendererDebug;
        const entityDebug = window.__mir2BevyEntityRendererDebug;
        const domMapSpriteCount = document.querySelectorAll(".scene-map-floor-sprite, .scene-map-object-sprite").length;
        const webGlMapCanvas = document.querySelector(".webgl2-map-atlas-canvas:not(.hidden)");
        const bevyCanvas = document.querySelector("#mir2-web3-canvas:not(.bevy-canvas-hidden)");
        const bevyMapReady =
          mapDebug?.enabled === true &&
          (mapDebug?.tileCount ?? 0) > 0 &&
          ((mapDebug?.atlasImageCount ?? 0) > 0 || (mapDebug?.decodedPageCount ?? 0) > 0);
        const entityReady =
          entityDebug?.ready === true ||
          (entityDebug?.enabled === true && (entityDebug?.layerCount ?? 0) > 0) ||
          document.querySelectorAll(".entity-nameplate").length > 0;
        const mapReady = bevyMapReady || domMapSpriteCount > 0 || Boolean(webGlMapCanvas) || Boolean(bevyCanvas);
        return mapReady && entityReady;
      })()
    `,
    "game visual readiness",
    timeoutMs,
  );
}

async function waitForAnimationFrames(client, frameCount) {
  await client.evaluate(`
    new Promise((resolve) => {
      let remaining = ${Math.max(1, Number(frameCount) || 1)};
      const tick = () => {
        remaining -= 1;
        if (remaining <= 0) {
          resolve(true);
          return;
        }
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    })
  `);
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

async function loginWithPassword(client, accountId, password) {
  const evidence = await client.evaluate(`
    (() => {
      const stage = window.__mir2Stage5;
      const before = {
        screen: stage?.state?.screen ?? null,
        wsState: stage?.state?.wsState ?? null,
        loginBusy: stage?.state?.loginBusy ?? null,
      };
      if (typeof stage?.loginPassword === "function") {
        return {
          mode: "stage.loginPassword",
          invoked: stage.loginPassword(${JSON.stringify(accountId)}, ${JSON.stringify(password)}),
          before,
        };
      }
      const form = document.querySelector(".login-dialog");
      if (form && typeof form.requestSubmit === "function") {
        form.requestSubmit();
        return { mode: "form.requestSubmit", invoked: true, before };
      }
      const button = document.querySelector(".login-button.ok button");
      if (button) {
        button.click();
        return { mode: "button.click", invoked: true, before };
      }
      return { mode: "none", invoked: false, before };
    })()
  `);
  if (!evidence?.invoked) {
    throw new Error(`Could not trigger password login: ${JSON.stringify(evidence)}`);
  }
  return evidence;
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

async function ensureCharacter(client, name) {
  const result = await client.evaluate(`
    (() => {
      const stage = window.__mir2Stage5;
      const characters = Array.isArray(stage?.state?.characters) ? stage.state.characters : [];
      if (characters.some((character) => character?.name === ${JSON.stringify(name)})) {
        return { ok: true, existed: true };
      }
      if (!stage?.send) return { ok: false, reason: "missing-stage-send" };
      return {
        ok: stage.send(${JSON.stringify({
          type: "newCharacter",
          name,
          gender: "male",
          class: "warrior",
        })}) === true,
        existed: false,
      };
    })()
  `);
  if (!result?.ok) {
    throw new Error(`Failed to create capture character ${name}: ${JSON.stringify(result)}`);
  }
  await waitUntil(
    client,
    `
      Array.isArray(window.__mir2Stage5?.state?.characters)
        && window.__mir2Stage5.state.characters.some((character) => character?.name === ${JSON.stringify(name)})
    `,
    `capture character ${name}`,
    15_000,
  );
}

async function startCharacterByName(client, name) {
  const started = await client.evaluate(`
    (() => {
      const stage = window.__mir2Stage5;
      const characters = Array.isArray(stage?.state?.characters) ? stage.state.characters : [];
      const character = characters.find((entry) => entry?.name === ${JSON.stringify(name)});
      if (!stage?.send || !character) return false;
      return stage.send({ type: "startGame", characterIndex: character.index ?? 0 }) === true;
    })()
  `);
  if (!started) throw new Error(`Failed to start capture character ${name}`);
}

async function startSelectedCharacter(client) {
  const started = await client.evaluate(`
    (() => {
      const stage = window.__mir2Stage5;
      const state = stage?.state;
      if (!stage?.send || !state || !Array.isArray(state.characters) || state.characters.length === 0) {
        return false;
      }
      const selectedIndex = Number.isInteger(state.selectedCharacterIndex) ? state.selectedCharacterIndex : 0;
      const selected = state.characters[selectedIndex] ?? state.characters[0];
      const characterIndex = Number.isInteger(selected?.index) ? selected.index : selectedIndex;
      return stage.send({ type: "startGame", characterIndex }) === true;
    })()
  `);
  if (started) return;
  await click(client, ".select-action.start button");
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
  lastValue = await client.evaluate(`Boolean(${expression})`).catch(() => lastValue);
  if (lastValue) return;
  const debug = await client
    .evaluate(`
      (() => ({
        url: location.href,
        readyState: document.readyState,
        title: document.title,
        stageScreen: window.__mir2Stage5?.state?.screen ?? null,
        wsState: window.__mir2Stage5?.state?.wsState ?? null,
        loginBusy: window.__mir2Stage5?.state?.loginBusy ?? null,
        stageKeys: window.__mir2Stage5?.state ? Object.keys(window.__mir2Stage5.state).slice(0, 20) : [],
        loginInputs: {
          account: Boolean(document.querySelector(".login-input.account")),
          password: Boolean(document.querySelector(".login-input.password")),
          button: Boolean(document.querySelector(".login-button.ok button")),
        },
        selectStartButton: Boolean(document.querySelector(".select-action.start button")),
        transitionOverlay: Boolean(document.querySelector(".login-transition-overlay")),
        lastCommand: window.__mir2LastCommand ?? null,
        commandHistory: (window.__mir2CommandHistory ?? []).slice(0, 8),
        visibleLogs: Array.from(document.querySelectorAll(".chat-feed-line"))
          .map((node) => node.textContent?.trim() ?? "")
          .filter(Boolean)
          .slice(-12),
        sceneInteractionReady: window.__mir2Stage5?.state?.sceneInteractionReady ?? null,
        sceneAssetReadiness: window.__mir2Stage5?.state?.sceneAssetReadiness ?? null,
        bodyText: document.body?.innerText?.slice(0, 500) ?? "",
        webSocketFramesSent: ${JSON.stringify(client.webSocketFramesSent.slice(-12))},
        webSocketFramesReceived: ${JSON.stringify(client.webSocketFramesReceived.slice(-12))},
      }))()
    `)
    .catch((error) => ({ debugError: String(error) }));
  throw new Error(
    `Timed out waiting for ${label}; last=${JSON.stringify(lastValue)}; debug=${JSON.stringify(redactCaptureSecrets(debug))}`,
  );
}

function redactCaptureSecrets(value) {
  if (Array.isArray(value)) {
    return value.map(redactCaptureSecrets);
  }
  if (!value || typeof value !== "object") {
    return value;
  }

  return Object.fromEntries(
    Object.entries(value).map(([key, nested]) => [
      key,
      /(?:password|passkey|secret|token)/i.test(key) ? "[redacted]" : redactCaptureSecrets(nested),
    ]),
  );
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
      const miniMapLight = document.querySelector(".mini-map-light");
      const chatLines = Array.from(document.querySelectorAll(".chat-feed-line")).map((node) => node.textContent ?? "");
      const state = window.__mir2Stage5?.state ?? {};
      const inventoryItems = state.world?.inventoryItems ?? state.inventoryItems ?? [];
      const beltItems = state.world?.beltItems ?? state.beltItems ?? [];
      const storageItems = state.world?.storageItems ?? state.storageItems ?? [];
      const equipmentItems = state.world?.equipmentItems ?? state.equipmentItems ?? [];
      const questLog = state.world?.questLog ?? state.questLog ?? [];
      const knownSkills = state.world?.knownSkills ?? state.knownSkills ?? [];
      const sceneGate = window.__mir2SceneGate ?? null;
      const bevyMapRenderer = window.__mir2BevyMapRendererDebug ?? null;
      const bevyEntityRenderer = window.__mir2BevyEntityRendererDebug ?? null;
      const bevyRuntime = window.__mir2BevyRuntimeDebug ?? null;
      const webGl2MapCanvas = document.querySelector(".webgl2-map-atlas-canvas");
      const webGl2EntityCanvas = document.querySelector(".webgl2-entity-atlas-canvas");
      const bevyCanvas = document.querySelector("#mir2-web3-canvas");
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
      const mapObjectSprites = Array.from(document.querySelectorAll(".scene-map-object-sprite")).map((node) => ({
        path: node.getAttribute("data-map-sprite-path") ?? node.getAttribute("src") ?? "",
        renderPath: node.getAttribute("data-map-render-path") ?? node.getAttribute("src") ?? "",
        cellX: Number(node.getAttribute("data-map-cell-x")),
        cellY: Number(node.getAttribute("data-map-cell-y")),
        rect: rect(node.getBoundingClientRect()),
        mixBlendMode: getComputedStyle(node).mixBlendMode,
        opacity: getComputedStyle(node).opacity,
        filter: getComputedStyle(node).filter,
        zIndex: getComputedStyle(node).zIndex,
      }));
      const entities = state.entities ?? [];
      const nameplateNodes = Array.from(document.querySelectorAll(".entity-nameplate"));
      const stageCursor = stage ? getComputedStyle(document.querySelector(".client-stage-frame")).cursor : null;
      const itemSummary = (item) => ({
        key: item?.key ?? null,
        name: item?.name ?? null,
        slot: item?.slot ?? null,
        container: item?.container ?? null,
        quantity: item?.quantity ?? null,
        uniqueId: item?.uniqueId ?? item?.unique_id ?? null,
        icon: item?.icon ?? null,
        durabilityCurrent: item?.durabilityCurrent ?? item?.durability_current ?? null,
        durabilityMax: item?.durabilityMax ?? item?.durability_max ?? null,
      });
      const equipmentSummary = (item) => ({
        ...itemSummary(item),
        equipmentSlot: item?.equipmentSlot ?? item?.equipSlot ?? item?.slot ?? null,
        attack: item?.attack ?? null,
        defence: item?.defence ?? null,
      });
      const textNode = (selector) => {
        const node = document.querySelector(selector);
        return node
          ? {
              text: node.textContent?.trim() ?? "",
              rect: rect(node.getBoundingClientRect()),
              classes: node.className,
            }
          : null;
      };
      const hudTexts = {
        healthOnly: textNode(".hud-health-only-label"),
        top: textNode(".hud-top-label"),
        bottom: textNode(".hud-bottom-label"),
        level: textNode(".hud-level-label"),
        name: textNode(".hud-name-label"),
        map: textNode(".hud-map-label"),
        exp: textNode(".hud-exp-label"),
        gold: textNode(".hud-gold-label"),
        weight: textNode(".hud-weight-label"),
        space: textNode(".hud-space-label"),
        buff: textNode(".hud-buff-label"),
      };
      const hudOrbNode = (selector) => {
        const node = document.querySelector(selector);
        if (!node) return null;
        const style = getComputedStyle(node);
        return {
          rect: rect(node.getBoundingClientRect()),
          classes: node.className,
          heightStyle: node.style.height || null,
          display: style.display,
          visibility: style.visibility,
          opacity: style.opacity,
        };
      };
      const hudWeightBarNode = () => {
        const node = document.querySelector(".hud-weight-bar");
        if (!node) return null;
        const clip = node.querySelector(".hud-weight-bar-clip");
        const image = node.querySelector(".hud-weight-bar-fill");
        return {
          rect: rect(node.getBoundingClientRect()),
          clipRect: rect(clip?.getBoundingClientRect()),
          imageRect: rect(image?.getBoundingClientRect()),
          weightRatio: node.getAttribute("data-weight-ratio") ?? null,
          fillWidth: node.getAttribute("data-fill-width") ?? null,
          fillWidthStyle: clip?.style.width || null,
          originalSrc: node.getAttribute("data-mir2-original-src") ?? image?.getAttribute("data-mir2-original-src") ?? null,
          imageSrc: image?.getAttribute("src") ?? null,
        };
      };
      const beltLabels = Array.from(document.querySelectorAll(".belt-dialog > .belt-slot-label"));
      const beltDom = Array.from(document.querySelectorAll(".belt-slot")).map((node, index) => {
        const item = node.querySelector(".belt-item");
        const label = beltLabels[index] ?? node.querySelector(".belt-slot-label");
        return {
          index,
          rect: rect(node.getBoundingClientRect()),
          label: label?.textContent?.trim() ?? null,
          labelRect: rect(label?.getBoundingClientRect()),
          itemName: item?.getAttribute("aria-label") ?? null,
          quantity: node.querySelector(".belt-item-count")?.textContent?.trim() ?? null,
          iconSrc: node.querySelector(".belt-item-icon")?.getAttribute("src") ?? null,
        };
      });
      return {
        screen: state.screen ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        player: state.player ?? null,
        playerHp: state.world?.playerHp ?? state.playerHp ?? null,
        playerMaxHp: state.world?.playerMaxHp ?? state.playerMaxHp ?? null,
        playerMp: state.world?.playerMp ?? state.playerMp ?? null,
        playerMaxMp: state.world?.playerMaxMp ?? state.playerMaxMp ?? null,
        playerExperience: state.world?.playerExperience ?? state.playerExperience ?? null,
        playerMaxExperience: state.world?.playerMaxExperience ?? state.playerMaxExperience ?? null,
        gold: state.world?.gold ?? state.gold ?? null,
        credit: state.world?.credit ?? state.credit ?? null,
        currentWeight: state.world?.currentWeight ?? state.currentWeight ?? null,
        maxWeight: state.world?.maxWeight ?? state.maxWeight ?? null,
        freeBagSlots: state.world?.freeBagSlots ?? state.freeBagSlots ?? null,
        maxBagSlots: state.world?.maxBagSlots ?? state.maxBagSlots ?? null,
        lightSetting: state.world?.lightSetting ?? state.lightSetting ?? null,
        inventoryItemCount: inventoryItems.length,
        beltItemCount: beltItems.length,
        storageItemCount: storageItems.length,
        equipmentItemCount: equipmentItems.length,
        inventoryItems: inventoryItems.map(itemSummary),
        beltItems: beltItems.map(itemSummary),
        storageItems: storageItems.map(itemSummary),
        equipmentItems: equipmentItems.map(equipmentSummary),
        hudTexts,
        hudDom: {
          hpOrb: hudOrbNode(".hud-orb-fill.hp"),
          mpOrb: hudOrbNode(".hud-orb-fill.mp"),
          weightBar: hudWeightBarNode(),
          belt: beltDom,
        },
        questCount: questLog.length,
        skillCount: knownSkills.length,
        sceneInteractionReady: state.sceneInteractionReady ?? null,
        sceneAssetReadiness: state.sceneAssetReadiness ?? null,
        sceneGate,
        bevyMapRenderer,
        bevyEntityRenderer,
        bevyRuntime,
        hudHealthOnlyLabel: hudHealthOnlyLabel?.textContent ?? null,
        logs: state.logs ?? [],
        transitionOverlayVisible: Boolean(document.querySelector(".login-transition-overlay")),
        tutorialOverlayVisible: Boolean(document.querySelector('[aria-label="Beginner tutorial"]')),
        objectiveTrackerVisible: Array.from(document.querySelectorAll('[role="status"]')).some((node) =>
          /New Quest|Current Objective|Ready to Turn In/i.test(node.textContent ?? ""),
        ),
        entityCount: entities.length,
        npcCount: entities.filter((entity) => entity.kind === "npc").length,
        monsterCount: entities.filter((entity) => entity.kind === "monster").length,
        entities: entities.map((entity) => ({
          objectId: entity.objectId ?? null,
          kind: entity.kind ?? null,
          name: entity.name ?? null,
          x: entity.x ?? null,
          y: entity.y ?? null,
          direction: entity.direction ?? null,
          image: entity.image ?? null,
          frame: entity.frame ?? null,
          visible: entity.visible ?? null,
        })),
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
        mapObjectSprites,
        torchLightSprites: mapObjectSprites.filter((sprite) =>
          /\\/original-map\\/WemadeMir2\\/Objects\\/27(2[3-9]|3[0-2])\\.png$/i.test(sprite.path),
        ),
        torchBodySprites: mapObjectSprites.filter((sprite) =>
          /\\/original-map\\/WemadeMir2\\/Objects\\/2733\\.png$/i.test(sprite.path),
        ),
        renderCanvases: {
          bevy: canvasState(rect, bevyCanvas),
          webGl2Map: canvasState(rect, webGl2MapCanvas),
          webGl2Entity: canvasState(rect, webGl2EntityCanvas),
        },
        visibleChatLines: chatLines,
        nextDevPortalCount: nextDevPortals.length,
        nextDevIndicatorVisible: visibleNextDevPortals.length > 0,
        stage: rect(stage),
        hud: rect(hud),
        miniMap: rect(miniMap),
        miniMapLight: miniMapLight
          ? {
              src: miniMapLight.getAttribute("src") ?? null,
              originalSrc: miniMapLight.getAttribute("data-mir2-original-src") ?? null,
              rect: rect(miniMapLight.getBoundingClientRect()),
              classes: miniMapLight.className,
            }
          : null,
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

      function canvasState(rect, canvas) {
        if (!canvas) return null;
        const style = getComputedStyle(canvas);
        return {
          rect: rect(canvas.getBoundingClientRect()),
          width: canvas.width ?? null,
          height: canvas.height ?? null,
          display: style.display,
          visibility: style.visibility,
          opacity: style.opacity,
          hidden: canvas.classList.contains("hidden") || canvas.classList.contains("bevy-canvas-hidden"),
        };
      }
    })()
  `);
}

function isCriticalConsoleError(error) {
  const text = String(error?.text ?? "");
  if (!text.trim()) return false;
  if (text.includes("net::ERR_FAILED")) return false;
  if (text.includes("favicon")) return false;
  return true;
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

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
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

async function readJsonFile(filePath) {
  return JSON.parse((await fs.readFile(filePath, "utf8")).replace(/^\uFEFF/, ""));
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

await main();
