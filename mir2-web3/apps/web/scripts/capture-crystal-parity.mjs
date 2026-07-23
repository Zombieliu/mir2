import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import sharp from "sharp";

import {
  additiveEffectHasDirectWorldBackdrop,
  crystalCaptureLightState,
  isDayCaptureLight,
  parseCaptureEffectFrame,
  parseCaptureLightSetting,
} from "./crystal-capture-visual-state.mjs";
import { decodeCdpMessage, isCriticalConsoleError } from "./cdp-message.mjs";
import { redactCaptureSecrets } from "./capture-secret-redaction.mjs";

const require = createRequire(import.meta.url);
const CdpWebSocket = require("next/dist/compiled/ws");

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
const captureLightSetting = parseCaptureLightSetting(
  args.captureLightSetting ?? process.env.MIR2_CAPTURE_LIGHT_SETTING,
);
const cleanCaptureOverlays = booleanArg(
  args.cleanCaptureOverlays ?? process.env.MIR2_CLEAN_CAPTURE_OVERLAYS,
  true,
);
const captureTrapHexagonFrame = parseCaptureEffectFrame(
  args.captureTrapHexagonFrame ?? process.env.MIR2_CAPTURE_TRAP_HEXAGON_FRAME,
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
    this.ws = new CdpWebSocket(this.wsUrl, { perMessageDeflate: false });
    this.ws.binaryType = "arraybuffer";
    this.ws.addEventListener("message", (event) => {
      void this.handleMessage(event.data).catch((error) => this.rejectPending(error));
    });
    this.ws.addEventListener("close", (event) => {
      this.rejectPending(
        new Error(`CDP WebSocket closed (${event.code}): ${event.reason || "no reason"}`),
      );
    });
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }

  async handleMessage(raw) {
    const message = await decodeCdpMessage(raw);
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

  rejectPending(error) {
    const failure = error instanceof Error ? error : new Error(String(error));
    for (const { reject } of this.pending.values()) reject(failure);
    this.pending.clear();
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
  let captureVisualStateInstalled = false;

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
    let protocolReadyEvidence = null;
    if (createAccount) {
      await click(client, ".login-button.account button");
      protocolReadyEvidence = await waitForGatewayProtocolReady(client);
      await delay(2_000);
    }
    const loginEvidence = await loginWithPassword(client, account, password);
    protocolReadyEvidence ??= await waitForGatewayProtocolReady(client);
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
    const effectLocks = await loadCaptureEffectLocks({ trapHexagonFrame: captureTrapHexagonFrame });
    const visualNormalizationEvidence = await installCaptureVisualState(client, {
      lightSetting: captureLightSetting,
      cleanOverlays: cleanCaptureOverlays,
      effectLocks,
    });
    captureVisualStateInstalled = true;
    await waitForAnimationFrames(client, 2);
    await delay(settleMs);
    const settledCaptureVisualState = await assertCaptureVisualStateStable(
      client,
      visualNormalizationEvidence,
    );
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
    const finalCaptureVisualState = await assertCaptureVisualStateStable(
      client,
      visualNormalizationEvidence,
    );
    const screenshot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    const screenshotBuffer = Buffer.from(screenshot.data, "base64");
    const effectPixelContribution =
      effectLocks.length > 0
        ? await captureEffectPixelContribution(client, screenshotBuffer)
        : null;
    let effectHiddenScreenshot = null;
    if (effectPixelContribution) {
      if (effectPixelContribution.changedPixelCount < 100) {
        throw new Error(
          `Locked Crystal effects did not contribute visible pixels: ${JSON.stringify(effectPixelContribution)}`,
        );
      }
      const effectHiddenScreenshotPath = path.join(outputDir, `${prefix}-effects-hidden.png`);
      await fs.writeFile(effectHiddenScreenshotPath, effectPixelContribution.hiddenBuffer);
      effectHiddenScreenshot = path.relative(process.cwd(), effectHiddenScreenshotPath).replaceAll("\\", "/");
      delete effectPixelContribution.hiddenBuffer;
    }
    await fs.writeFile(screenshotPath, screenshotBuffer);
    const captureEvidence = redactCaptureSecrets({
      ...state,
      network404Count: client.network404s.length,
      consoleErrorCount: client.consoleErrors.length,
      criticalConsoleErrorCount: client.criticalConsoleErrors().length,
      nonFaviconNetwork404s: [...new Set(client.network404s)],
      consoleErrors: client.consoleErrors,
      criticalConsoleErrors: client.criticalConsoleErrors(),
      captureControl: {
        protocolReady: protocolReadyEvidence,
        login: loginEvidence,
        stateAlignment: stateAlignmentEvidence,
        transfer: transferEvidence,
        visualNormalization: {
          ...visualNormalizationEvidence,
          settled: settledCaptureVisualState,
          final: finalCaptureVisualState,
          effectPixelContribution,
        },
      },
      screenshot: path.relative(process.cwd(), screenshotPath).replaceAll("\\", "/"),
      effectHiddenScreenshot,
    });
    await fs.writeFile(
      statePath,
      `${JSON.stringify(captureEvidence, null, 2)}\n`,
    );

    console.log(JSON.stringify({ ok: true, screenshotPath, statePath }, null, 2));
  } finally {
    if (client && captureVisualStateInstalled) {
      await cleanupCaptureVisualState(client).catch(() => undefined);
    }
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
      "--remote-allow-origins=*",
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

async function loadCaptureEffectLocks({ trapHexagonFrame }) {
  if (trapHexagonFrame === null) return [];
  const meta = JSON.parse(
    await fs.readFile(new URL("../public/original-effects/Magic/meta.json", import.meta.url), "utf8"),
  );
  const base = 1390;
  const frames = {};
  for (let offset = 0; offset < 10; offset += 1) {
    const index = base + offset;
    const frame = meta.frames?.[String(index)];
    if (!frame) throw new Error(`Magic frame metadata is missing index ${index}`);
    frames[`/original-effects/Magic/${index}.png`] = {
      path: `/original-effects/Magic/${index}.png`,
      width: frame.width,
      height: frame.height,
      x: frame.x,
      y: frame.y,
    };
  }
  return [
    {
      effectName: "TrapHexagon",
      frameOffset: trapHexagonFrame,
      desired: frames[`/original-effects/Magic/${base + trapHexagonFrame}.png`],
      frames,
    },
  ];
}

async function installCaptureVisualState(client, { lightSetting, cleanOverlays, effectLocks }) {
  const requested = lightSetting === null ? null : crystalCaptureLightState(lightSetting);
  const serverLightSetting = await client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state;
      return state?.world?.lightSetting ?? state?.lightSetting ?? null;
    })()
  `);

  if (requested && !isDayCaptureLight(requested.setting) && serverLightSetting !== requested.setting) {
    throw new Error(
      `Cannot synthesize ${requested.label} lighting: server lightSetting is ${serverLightSetting}, requested ${requested.setting}`,
    );
  }

  const expected = requested
    ? {
        setting: requested.setting,
        label: requested.label,
        overlayClass: requested.overlayClass,
        miniMapIcon: requested.miniMapIcon,
      }
    : null;
  const installed = await client.evaluate(`
    (() => {
      const expected = ${JSON.stringify(expected)};
      const effectLocks = ${JSON.stringify(effectLocks)};
      const cleanOverlays = ${JSON.stringify(cleanOverlays)};
      window.__mir2CaptureVisualStateCleanup?.();
      const root = document.documentElement;
      const styleId = "mir2-capture-visual-state";
      let style = document.getElementById(styleId);
      const styleWasPresent = Boolean(style);
      const originalStyleText = style?.textContent ?? "";
      const originalRootAttributes = {
        clean: root.getAttribute("data-mir2-capture-clean"),
        light: root.getAttribute("data-mir2-capture-light-setting"),
      };
      const touchedNodes = new Map();
      const rememberNode = (node) => {
        if (touchedNodes.has(node)) return;
        touchedNodes.set(node, {
          src: node.getAttribute("src"),
          originalSrc: node.getAttribute("data-mir2-original-src"),
          captureFrame: node.getAttribute("data-mir2-capture-effect-frame"),
          captureBaseLeft: node.getAttribute("data-mir2-capture-effect-base-left"),
          captureBaseTop: node.getAttribute("data-mir2-capture-effect-base-top"),
          style: {
            left: node.style.left,
            top: node.style.top,
            width: node.style.width,
            height: node.style.height,
          },
        });
      };
      const restoreAttribute = (node, name, value) => {
        if (value === null) node.removeAttribute(name);
        else node.setAttribute(name, value);
      };
      if (!style) {
        style = document.createElement("style");
        style.id = styleId;
        document.head.appendChild(style);
      }
      style.textContent = [
        cleanOverlays
          ? '[aria-label="Mir2 resource loading status"], [aria-label="Mir2 cache debug status"] { display: none !important; }'
          : '',
        expected?.label === "day"
          ? '.viewport-crystal-light-overlay { display: none !important; }'
          : '',
      ].filter(Boolean).join("\\n");
      root.dataset.mir2CaptureClean = cleanOverlays ? "1" : "0";
      if (expected) root.dataset.mir2CaptureLightSetting = String(expected.setting);
      else delete root.dataset.mir2CaptureLightSetting;

      window.__mir2CaptureVisualStateObserver?.disconnect?.();
      const apply = () => {
        if (expected) {
          const icon = document.querySelector(".mini-map-light");
          if (icon && new URL(icon.src, location.href).pathname !== expected.miniMapIcon) {
            rememberNode(icon);
            icon.src = expected.miniMapIcon;
            icon.setAttribute("data-mir2-original-src", expected.miniMapIcon);
          }
        }
        for (const lock of effectLocks) {
          const nodes = document.querySelectorAll(
            '.scene-crystal-effect-frame[data-effect-name="' + lock.effectName + '"]:not(.mask)',
          );
          for (const node of nodes) {
            rememberNode(node);
            const currentPath = new URL(
              node.getAttribute("data-mir2-original-src") || node.src,
              location.href,
            ).pathname;
            const currentFrame = lock.frames[currentPath] || lock.desired;
            const style = node.style;
            if (!node.dataset.mir2CaptureEffectBaseLeft) {
              node.dataset.mir2CaptureEffectBaseLeft = String((Number.parseFloat(style.left) || 0) - currentFrame.x);
              node.dataset.mir2CaptureEffectBaseTop = String((Number.parseFloat(style.top) || 0) - currentFrame.y);
            }
            const desiredLeft = Number(node.dataset.mir2CaptureEffectBaseLeft) + lock.desired.x;
            const desiredTop = Number(node.dataset.mir2CaptureEffectBaseTop) + lock.desired.y;
            const desiredStyles = {
              left: String(desiredLeft) + "px",
              top: String(desiredTop) + "px",
              width: String(lock.desired.width) + "px",
              height: String(lock.desired.height) + "px",
            };
            for (const [property, value] of Object.entries(desiredStyles)) {
              if (style[property] !== value) style[property] = value;
            }
            if (new URL(node.src, location.href).pathname !== lock.desired.path) node.src = lock.desired.path;
            if (node.getAttribute("data-mir2-original-src") !== lock.desired.path) {
              node.setAttribute("data-mir2-original-src", lock.desired.path);
            }
            node.dataset.mir2CaptureEffectFrame = String(lock.frameOffset);
          }
        }
      };
      apply();
      let observer = null;
      if (expected || effectLocks.length > 0) {
        observer = new MutationObserver(apply);
        observer.observe(document.documentElement, {
          subtree: true,
          childList: true,
          attributes: true,
          attributeFilter: ["src", "style", "data-mir2-original-src"],
        });
        window.__mir2CaptureVisualStateObserver = observer;
      }
      window.__mir2CaptureVisualStateCleanup = () => {
        observer?.disconnect();
        for (const [node, snapshot] of touchedNodes) {
          if (!node?.isConnected) continue;
          restoreAttribute(node, "src", snapshot.src);
          restoreAttribute(node, "data-mir2-original-src", snapshot.originalSrc);
          restoreAttribute(node, "data-mir2-capture-effect-frame", snapshot.captureFrame);
          restoreAttribute(node, "data-mir2-capture-effect-base-left", snapshot.captureBaseLeft);
          restoreAttribute(node, "data-mir2-capture-effect-base-top", snapshot.captureBaseTop);
          Object.assign(node.style, snapshot.style);
        }
        if (styleWasPresent) style.textContent = originalStyleText;
        else style.remove();
        restoreAttribute(root, "data-mir2-capture-clean", originalRootAttributes.clean);
        restoreAttribute(root, "data-mir2-capture-light-setting", originalRootAttributes.light);
        delete window.__mir2CaptureVisualStateObserver;
        delete window.__mir2CaptureVisualStateCleanup;
      };
      return { styleInstalled: style.isConnected, observerInstalled: Boolean(expected || effectLocks.length > 0) };
    })()
  `);

  const stabilitySamples = [];
  await waitForAnimationFrames(client, 2);
  stabilitySamples.push(await readCaptureVisualState(client));
  assertCaptureVisualState(stabilitySamples.at(-1), expected, cleanOverlays, effectLocks);
  const stabilityDelaysMs = effectLocks.length > 0 ? [137, 173, 211] : [34];
  for (const stabilityDelayMs of stabilityDelaysMs) {
    await delay(stabilityDelayMs);
    stabilitySamples.push(await readCaptureVisualState(client));
    assertCaptureVisualState(stabilitySamples.at(-1), expected, cleanOverlays, effectLocks);
  }

  return {
    mode: requested && isDayCaptureLight(requested.setting) ? "presentation-only-day-normalization" : "assert-only",
    requested: expected,
    serverLightSetting,
    cleanOverlays,
    effectLocks: effectLocks.map(({ frames: _frames, ...lock }) => lock),
    installed,
    stableAcrossAnimationFrames: true,
    stabilityDelaysMs,
    stabilitySamples,
    first: stabilitySamples[0],
    second: stabilitySamples[1],
  };
}

async function cleanupCaptureVisualState(client) {
  await client.evaluate(`
    (() => {
      window.__mir2CaptureVisualStateCleanup?.();
      return {
        observerPresent: Boolean(window.__mir2CaptureVisualStateObserver),
        cleanupPresent: Boolean(window.__mir2CaptureVisualStateCleanup),
        stylePresent: Boolean(document.getElementById("mir2-capture-visual-state")),
      };
    })()
  `);
}

async function assertCaptureVisualStateStable(client, evidence) {
  const current = await readCaptureVisualState(client);
  assertCaptureVisualState(
    current,
    evidence.requested,
    evidence.cleanOverlays,
    evidence.effectLocks ?? [],
  );
  return current;
}

async function captureEffectPixelContribution(client, normalBuffer) {
  const prepared = await client.evaluate(`
    (() => {
      const nodes = Array.from(document.querySelectorAll(".scene-crystal-effect-frame"))
        .filter((node) => {
          const style = getComputedStyle(node);
          const rect = node.getBoundingClientRect();
          return style.display !== "none" && style.visibility !== "hidden" && Number(style.opacity) > 0 &&
            rect.right > 0 && rect.bottom > 0 && rect.left < innerWidth && rect.top < innerHeight;
        });
      if (nodes.length === 0) return null;
      const rects = nodes.map((node) => node.getBoundingClientRect());
      const left = Math.max(0, Math.floor(Math.min(...rects.map((rect) => rect.left))));
      const top = Math.max(0, Math.floor(Math.min(...rects.map((rect) => rect.top))));
      const right = Math.min(innerWidth, Math.ceil(Math.max(...rects.map((rect) => rect.right))));
      const bottom = Math.min(innerHeight, Math.ceil(Math.max(...rects.map((rect) => rect.bottom))));
      window.__mir2EffectPixelOpacity = nodes.map((node) => ({
        node,
        value: node.style.getPropertyValue("opacity"),
        priority: node.style.getPropertyPriority("opacity"),
      }));
      for (const node of nodes) node.style.setProperty("opacity", "0", "important");
      return { nodeCount: nodes.length, rect: { left, top, width: right - left, height: bottom - top } };
    })()
  `);
  if (!prepared?.rect?.width || !prepared?.rect?.height) {
    throw new Error(`No visible Crystal effect pixels were available to measure: ${JSON.stringify(prepared)}`);
  }

  let hiddenBuffer;
  try {
    await waitForAnimationFrames(client, 2);
    await delay(100);
    const hidden = await client.send("Page.captureScreenshot", {
      format: "png",
      captureBeyondViewport: false,
    });
    hiddenBuffer = Buffer.from(hidden.data, "base64");
  } finally {
    await client.evaluate(`
      (() => {
        for (const entry of window.__mir2EffectPixelOpacity ?? []) {
          if (!entry.node?.isConnected) continue;
          if (entry.value) entry.node.style.setProperty("opacity", entry.value, entry.priority);
          else entry.node.style.removeProperty("opacity");
        }
        delete window.__mir2EffectPixelOpacity;
      })()
    `).catch(() => undefined);
  }

  const rect = prepared.rect;
  const [normal, hidden] = await Promise.all([
    sharp(normalBuffer).extract(rect).removeAlpha().raw().toBuffer({ resolveWithObject: true }),
    sharp(hiddenBuffer).extract(rect).removeAlpha().raw().toBuffer({ resolveWithObject: true }),
  ]);
  const channels = Math.min(normal.info.channels, hidden.info.channels, 3);
  const pixelCount = rect.width * rect.height;
  const pixelDeltaThreshold = 4;
  let changedPixelCount = 0;
  let absoluteDeltaTotal = 0;
  for (let pixel = 0; pixel < pixelCount; pixel += 1) {
    const normalOffset = pixel * normal.info.channels;
    const hiddenOffset = pixel * hidden.info.channels;
    let maximumDelta = 0;
    let pixelDelta = 0;
    for (let channel = 0; channel < channels; channel += 1) {
      const delta = Math.abs(normal.data[normalOffset + channel] - hidden.data[hiddenOffset + channel]);
      maximumDelta = Math.max(maximumDelta, delta);
      pixelDelta += delta;
    }
    if (maximumDelta >= pixelDeltaThreshold) changedPixelCount += 1;
    absoluteDeltaTotal += pixelDelta / channels;
  }

  return {
    nodeCount: prepared.nodeCount,
    rect,
    pixelCount,
    pixelDeltaThreshold,
    changedPixelCount,
    changedPixelRatio: changedPixelCount / pixelCount,
    meanAbsDelta: absoluteDeltaTotal / pixelCount,
    hiddenBuffer,
  };
}

async function readCaptureVisualState(client) {
  return client.evaluate(`
    (() => {
      const overlay = document.querySelector(".viewport-crystal-light-overlay");
      const icon = document.querySelector(".mini-map-light");
      const resourceOverlays = Array.from(document.querySelectorAll(
        '[aria-label="Mir2 resource loading status"], [aria-label="Mir2 cache debug status"]'
      ));
      const visible = (node) => {
        if (!node) return false;
        const style = getComputedStyle(node);
        const rect = node.getBoundingClientRect();
        return style.display !== "none" && style.visibility !== "hidden" && Number(style.opacity) > 0 && rect.width > 0 && rect.height > 0;
      };
      return {
        rootLightSetting: document.documentElement.dataset.mir2CaptureLightSetting ?? null,
        overlay: overlay
          ? {
              className: overlay.className,
              dataLightSetting: overlay.getAttribute("data-light-setting"),
              display: getComputedStyle(overlay).display,
              backgroundColor: getComputedStyle(overlay).backgroundColor,
              visible: visible(overlay),
            }
          : null,
        miniMapIcon: icon
          ? {
              src: icon.getAttribute("src"),
              resolvedSrc: icon.src,
              originalSrc: icon.getAttribute("data-mir2-original-src"),
            }
          : null,
        resourceOverlayCount: resourceOverlays.length,
        visibleResourceOverlayCount: resourceOverlays.filter(visible).length,
        sceneEffects: Array.from(document.querySelectorAll(".scene-crystal-effect-frame:not(.mask)")).map((node) => {
          const effectOverlay = node.closest(".viewport-effect-overlay");
          const worldComposite = node.closest(".game-world-composite");
          const worldStyle = worldComposite ? getComputedStyle(worldComposite) : null;
          const effectOverlayStyle = effectOverlay ? getComputedStyle(effectOverlay) : null;
          const effectStyle = getComputedStyle(node);
          const worldRenderer =
            worldComposite?.querySelector("#mir2-web3-canvas:not(.bevy-canvas-hidden), .webgl2-entity-atlas-canvas:not(.hidden)") ??
            worldComposite?.querySelector("#mir2-web3-canvas, .webgl2-entity-atlas-canvas");
          return {
            effectName: node.getAttribute("data-effect-name"),
            effectKey: node.getAttribute("data-effect-key"),
            blend: node.getAttribute("data-effect-blend"),
            src: node.getAttribute("src"),
            resolvedSrc: node.src,
            originalSrc: node.getAttribute("data-mir2-original-src"),
            captureFrame: node.getAttribute("data-mir2-capture-effect-frame"),
            captureBaseLeft: node.getAttribute("data-mir2-capture-effect-base-left"),
            captureBaseTop: node.getAttribute("data-mir2-capture-effect-base-top"),
            left: node.style.left,
            top: node.style.top,
            width: node.style.width,
            height: node.style.height,
            effectOverlayZIndex: effectOverlayStyle?.zIndex ?? null,
            effectOverlayTranslate: effectOverlayStyle?.translate ?? null,
            effectOverlayTransform: effectOverlayStyle?.transform ?? null,
            effectNodeZIndex: effectStyle.zIndex,
            worldRendererZIndex: worldRenderer ? getComputedStyle(worldRenderer).zIndex : null,
            worldCompositeIsolation: worldStyle?.isolation ?? null,
            worldCompositeVisible:
              Boolean(worldComposite) &&
              worldStyle?.display !== "none" &&
              worldStyle?.visibility !== "hidden",
          };
        }),
      };
    })()
  `);
}

function assertCaptureVisualState(actual, expected, cleanOverlays, effectLocks = []) {
  if (cleanOverlays && actual.visibleResourceOverlayCount !== 0) {
    throw new Error(`Capture resource overlay remained visible: ${JSON.stringify(actual)}`);
  }
  if (expected) {
    if (actual.rootLightSetting !== String(expected.setting)) {
      throw new Error(`Capture light marker drifted: ${JSON.stringify(actual)}`);
    }
    const iconPath = actual.miniMapIcon?.resolvedSrc
      ? new URL(actual.miniMapIcon.resolvedSrc).pathname
      : null;
    if (iconPath !== expected.miniMapIcon) {
      throw new Error(`Capture minimap light icon mismatch: expected ${expected.miniMapIcon}, observed ${iconPath}`);
    }
    if (expected.label === "day") {
      if (actual.overlay?.visible === true) {
        throw new Error(`Day capture still has a visible Crystal light overlay: ${JSON.stringify(actual.overlay)}`);
      }
    } else if (
      !actual.overlay?.visible ||
      !String(actual.overlay.className).split(/\s+/).includes(expected.overlayClass)
    ) {
      throw new Error(`Capture light overlay mismatch: ${JSON.stringify(actual.overlay)}`);
    }
  }
  for (const effect of actual.sceneEffects) {
    if (!additiveEffectHasDirectWorldBackdrop(effect)) {
      throw new Error(`Capture additive effect lacks a direct world backdrop: ${JSON.stringify(effect)}`);
    }
  }
  for (const lock of effectLocks) {
    const matching = actual.sceneEffects.filter((effect) => effect.effectName === lock.effectName);
    if (matching.length === 0) {
      throw new Error(`Capture effect ${lock.effectName} was not present`);
    }
    for (const effect of matching) {
      const pathName = effect.resolvedSrc ? new URL(effect.resolvedSrc).pathname : null;
      const baseLeft = Number(effect.captureBaseLeft);
      const baseTop = Number(effect.captureBaseTop);
      const actualLeft = Number.parseFloat(effect.left);
      const actualTop = Number.parseFloat(effect.top);
      const expectedLeft = baseLeft + lock.desired.x;
      const expectedTop = baseTop + lock.desired.y;
      if (
        pathName !== lock.desired.path ||
        effect.width !== `${lock.desired.width}px` ||
        effect.height !== `${lock.desired.height}px` ||
        effect.captureFrame !== String(lock.frameOffset) ||
        !Number.isFinite(baseLeft) ||
        !Number.isFinite(baseTop) ||
        !Number.isFinite(actualLeft) ||
        !Number.isFinite(actualTop) ||
        Math.abs(actualLeft - expectedLeft) > 0.01 ||
        Math.abs(actualTop - expectedTop) > 0.01
      ) {
        throw new Error(`Capture effect lock drifted: ${JSON.stringify({ lock, effect })}`);
      }
    }
  }
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

async function waitForGatewayProtocolReady(client) {
  const startedAt = Date.now();
  await waitUntil(
    client,
    "window.__mir2Stage5?.state?.wsState === 'open'",
    "login socket open",
    30_000,
  );
  await waitUntil(
    client,
    `window.__mir2Stage5?.state?.gatewayProtocolReady === true &&
      (window.__mir2GatewayEventHistory ?? []).some(
        (event) => event?.type === "packet" && event?.packet === "Connected"
      )`,
    "Crystal Connected handshake",
    30_000,
  );
  const evidence = await client.evaluate(`
    (() => {
      const connected = (window.__mir2GatewayEventHistory ?? []).find(
        (event) => event?.type === "packet" && event?.packet === "Connected"
      );
      return {
        wsState: window.__mir2Stage5?.state?.wsState ?? null,
        gatewayProtocolReady: window.__mir2Stage5?.state?.gatewayProtocolReady ?? false,
        connectedAt: connected?.at ?? null,
      };
    })()
  `);
  return { ...evidence, waitMs: Date.now() - startedAt };
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
        gatewayProtocolReady: window.__mir2Stage5?.state?.gatewayProtocolReady ?? null,
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
        gatewayEventHistory: (window.__mir2GatewayEventHistory ?? []).slice(0, 8),
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
    `Timed out waiting for ${label}; last=${JSON.stringify(redactCaptureSecrets(lastValue))}; debug=${JSON.stringify(redactCaptureSecrets(debug))}`,
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
      const gdiTextNodes = Array.from(document.querySelectorAll("[data-crystal-gdi-text]"));
      const animationPoseNodes = Array.from(
        document.querySelectorAll("[data-object-id][data-animation-action]"),
      );
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
        crystalGdiText: {
          count: gdiTextNodes.length,
          assets: gdiTextNodes.slice(0, 64).map((node) => ({
            key: node.getAttribute("data-crystal-gdi-text"),
            text: node.textContent?.trim() ?? "",
            imageSrc: node.querySelector("img")?.getAttribute("src") ?? null,
            rect: rect(node.getBoundingClientRect()),
          })),
        },
        entityAnimationRuntime: {
          resolverAvailable:
            typeof window.__mir2BevyRuntime?.resolveMir2EntityAnimationPoses === "function",
          poseCount: animationPoseNodes.length,
          poses: animationPoseNodes.slice(0, 64).map((node) => ({
            objectId: node.getAttribute("data-object-id"),
            action: node.getAttribute("data-animation-action"),
            frame: node.getAttribute("data-animation-frame"),
            incarnation: node.getAttribute("data-animation-incarnation"),
          })),
        },
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
          ai: entity.ai ?? null,
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
