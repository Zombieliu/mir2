import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { auditOutgoingBrowserCommand } from "./policy.mjs";

const OPTIONAL_ORIGINAL_ASSET_PATH = /\/original-(?:map|effects|ui)\//;
const RASTER_ASSET_EXTENSION = /\.(?:avif|gif|jpe?g|png|webp)(?:[?#]|$)/i;
const DYNAMIC_SCENE_REQUEST_PATH = /\/api\/scene\/crystal(?:[?#]|$)/;

export function classifyBrowserDiagnostics(consoleErrors = [], networkFailures = []) {
  const knownAssetFallbackConsoleErrors = consoleErrors.filter((entry) => {
    const text = String(entry?.text ?? "");
    return (text.includes("/original-map/") && text.includes("404")) ||
      text.startsWith("[mir2] scene asset missing");
  });
  const knownAssetFallbackNetworkFailures = networkFailures.filter((entry) => (
    Number(entry?.status) === 404 && String(entry?.url ?? "").includes("/original-map/")
  ));
  const abortedOptionalAssetRequests = networkFailures.filter((entry) => {
    const url = String(entry?.url ?? "");
    return Number(entry?.status) === 0
      && String(entry?.error ?? "").includes("ERR_ABORTED")
      && OPTIONAL_ORIGINAL_ASSET_PATH.test(url)
      && RASTER_ASSET_EXTENSION.test(url);
  });
  // A map/region change aborts the superseded scene fetch through the browser's
  // AbortController. That cancellation is expected transport evidence, not a
  // failed destination scene. Real scene failures still have an HTTP status or
  // a non-ERR_ABORTED network error and remain critical below.
  const abortedSupersededSceneRequests = networkFailures.filter((entry) => {
    const url = String(entry?.url ?? "");
    return Number(entry?.status) === 0
      && String(entry?.error ?? "").includes("ERR_ABORTED")
      && DYNAMIC_SCENE_REQUEST_PATH.test(url);
  });
  const criticalConsoleErrors = consoleErrors.filter(
    (entry) => !knownAssetFallbackConsoleErrors.includes(entry),
  );
  const criticalNetworkFailures = networkFailures.filter(
    (entry) => !knownAssetFallbackNetworkFailures.includes(entry)
      && !abortedOptionalAssetRequests.includes(entry)
      && !abortedSupersededSceneRequests.includes(entry),
  );
  return {
    knownAssetFallbackConsoleErrors,
    knownAssetFallbackNetworkFailures,
    abortedOptionalAssetRequests,
    abortedSupersededSceneRequests,
    criticalConsoleErrors,
    criticalNetworkFailures,
  };
}

export class CdpClient {
  constructor(wsUrl, onInput = () => {}) {
    this.wsUrl = wsUrl;
    this.inputs = [];
    this.onInput = (input) => {
      this.inputs.push(input);
      this.inputs = this.inputs.slice(-20_000);
      onInput(input);
    };
    this.nextId = 1;
    this.pending = new Map();
    this.console = [];
    this.consoleErrors = [];
    this.networkFailures = [];
    this.requestUrlById = new Map();
    this.webSocketUrlByRequestId = new Map();
    this.wsReceived = [];
    this.wsSent = [];
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (event) => this.#handleMessage(event.data));
    this.ws.addEventListener("close", () => this.#rejectPending("CDP socket closed"));
    this.ws.addEventListener("error", () => this.#rejectPending("CDP socket failed"));
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }

  #handleMessage(raw) {
    const message = JSON.parse(raw);
    if (message.id && this.pending.has(message.id)) {
      const pending = this.pending.get(message.id);
      this.pending.delete(message.id);
      clearTimeout(pending.timer);
      if (message.error) pending.reject(new Error(`${message.error.message}: ${message.error.data ?? ""}`));
      else pending.resolve(message.result ?? {});
      return;
    }

    const method = message.method;
    const params = message.params ?? {};
    if (method === "Runtime.consoleAPICalled") {
      const entry = {
        source: "console",
        level: params.type ?? "log",
        text: (params.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
        at: Date.now(),
      };
      this.console.push(entry);
      this.console = this.console.slice(-1000);
      if (["error", "warning"].includes(entry.level)) this.consoleErrors.push(entry);
    } else if (method === "Runtime.exceptionThrown") {
      this.consoleErrors.push({
        source: "exception",
        level: "error",
        text: params.exceptionDetails?.exception?.description ?? params.exceptionDetails?.text ?? "runtime exception",
        at: Date.now(),
      });
    } else if (method === "Log.entryAdded") {
      const entry = params.entry ?? {};
      if (["error", "warning"].includes(entry.level) && !String(entry.url ?? "").includes("favicon")) {
        this.consoleErrors.push({
          source: entry.source ?? "log",
          level: entry.level,
          text: `${entry.text ?? ""}${entry.url ? ` (${entry.url})` : ""}`,
          at: Date.now(),
        });
      }
    } else if (method === "Network.requestWillBeSent") {
      if (params.requestId && params.request?.url) this.requestUrlById.set(params.requestId, params.request.url);
    } else if (method === "Network.responseReceived") {
      const response = params.response;
      const url = String(response?.url ?? "");
      if (response?.status >= 400 && !url.includes("favicon")) {
        this.networkFailures.push({ url, status: response.status, at: Date.now() });
      }
    } else if (method === "Network.loadingFailed") {
      const url = this.requestUrlById.get(params.requestId) ?? "(unknown)";
      if (!String(url).includes("favicon")) {
        this.networkFailures.push({ url, status: 0, error: params.errorText ?? "", at: Date.now() });
      }
    } else if (method === "Network.webSocketCreated") {
      if (params.requestId && params.url) this.webSocketUrlByRequestId.set(params.requestId, params.url);
    } else if (method === "Network.webSocketFrameReceived") {
      this.wsReceived.push({
        payloadData: params.response?.payloadData ?? "",
        url: this.webSocketUrlByRequestId.get(params.requestId) ?? null,
        at: Date.now(),
      });
      this.wsReceived = this.wsReceived.slice(-12000);
    } else if (method === "Network.webSocketFrameSent") {
      this.wsSent.push({
        payloadData: params.response?.payloadData ?? "",
        url: this.webSocketUrlByRequestId.get(params.requestId) ?? null,
        at: Date.now(),
      });
      this.wsSent = this.wsSent.slice(-12000);
    }
  }

  #rejectPending(reason) {
    for (const [id, pending] of this.pending) {
      clearTimeout(pending.timer);
      pending.reject(
        reason instanceof Error
          ? reason
          : new Error(`${reason}; pending CDP ${pending.method} #${id}`),
      );
    }
    this.pending.clear();
  }

  cancelPending(reason = "CDP command cancelled") {
    this.#rejectPending(reason);
  }

  send(method, params = {}, timeoutMs = 30_000) {
    const id = this.nextId++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        if (!this.pending.delete(id)) return;
        reject(new Error(`CDP ${method} timed out after ${timeoutMs}ms`));
      }, timeoutMs);
      timer.unref?.();
      this.pending.set(id, { resolve, reject, timer, method });
    });
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: false,
    });
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text ?? "evaluation failed");
    }
    return result.result?.value;
  }

  async enable() {
    await this.send("Runtime.enable");
    await this.send("Log.enable");
    await this.send("Network.enable");
    await this.send("Page.enable");
    await this.send("Page.bringToFront");
  }

  async setViewport(width, height) {
    const metrics = { width, height, deviceScaleFactor: 1, mobile: false };
    await this.send("Emulation.setDeviceMetricsOverride", metrics);
    await this.send("Emulation.setVisibleSize", { width, height });
  }

  async navigate(url) {
    await this.send("Page.navigate", { url });
    await waitUntil(this, "document.readyState === 'complete' || document.readyState === 'interactive'", 30_000);
  }

  async clickSelector(selector, meta = {}) {
    const center = await elementCenter(this, selector, meta.text ?? null);
    if (!center) throw new Error(`visible element not found: ${selector}${meta.text ? ` text=${meta.text}` : ""}`);
    await this.#mouseClick(center.x, center.y, meta.button ?? "left", meta);
    return center;
  }

  async clickTile(x, y, button = "left", meta = {}) {
    const selector = `[aria-label="tile ${Number(x)}, ${Number(y)}"]`;
    // Prefer the legacy QA grid when explicitly present, but normal agent runs
    // derive the same physical point from the visible stage geometry. Avoiding
    // 1,155 transparent React buttons keeps long world-snapshot runs responsive.
    const center = await rawElementCenter(this, selector) ?? await tileScreenCenter(this, x, y);
    if (!center) return false;
    await this.#mouseClick(center.x, center.y, button, { ...meta, tile: { x, y } });
    return true;
  }

  async holdTileDirection(x, y, button = "right", durationMs = 700, meta = {}) {
    const selector = `[aria-label="tile ${Number(x)}, ${Number(y)}"]`;
    const center = await rawElementCenter(this, selector) ?? await tileScreenCenter(this, x, y);
    if (!center) return false;
    const buttons = button === "right" ? 2 : 1;
    const holdMs = Math.max(100, Math.min(2_000, Number(durationMs) || 700));
    this.onInput({
      kind: "mouse-hold", button, x: round(center.x), y: round(center.y),
      durationMs: holdMs, tile: { x, y }, ...meta, at: Date.now(),
    });
    await this.send("Input.dispatchMouseEvent", {
      type: "mouseMoved", x: center.x, y: center.y, button: "none",
    });
    await this.send("Input.dispatchMouseEvent", {
      type: "mousePressed", x: center.x, y: center.y, button, buttons, clickCount: 1,
    });
    await delay(holdMs);
    await this.send("Input.dispatchMouseEvent", {
      type: "mouseReleased", x: center.x, y: center.y, button, buttons: 0, clickCount: 1,
    });
    return true;
  }

  async wheelSelector(selector, deltaY, meta = {}) {
    const center = await rawElementCenter(this, selector);
    if (!center) return false;
    const wheelDeltaY = Math.max(-800, Math.min(800, Number(deltaY) || 0));
    this.onInput({
      kind: "mouse-wheel",
      x: round(center.x),
      y: round(center.y),
      deltaY: wheelDeltaY,
      selector,
      ...meta,
      at: Date.now(),
    });
    await this.send("Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: center.x,
      y: center.y,
      button: "none",
    });
    await this.send("Input.dispatchMouseEvent", {
      type: "mouseWheel",
      x: center.x,
      y: center.y,
      deltaX: 0,
      deltaY: wheelDeltaY,
    });
    return true;
  }

  async #mouseClick(x, y, button, meta) {
    const buttons = button === "right" ? 2 : 1;
    this.onInput({ kind: "mouse", button, x: round(x), y: round(y), ...meta, at: Date.now() });
    await this.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, button: "none" });
    await this.send("Input.dispatchMouseEvent", {
      type: "mousePressed", x, y, button, buttons, clickCount: 1,
    });
    await this.send("Input.dispatchMouseEvent", {
      type: "mouseReleased", x, y, button, buttons: 0, clickCount: 1,
    });
  }

  async fillSelector(selector, value, meta = {}) {
    await this.clickSelector(selector, { ...meta, action: "focus-input" });
    await this.#clearFocusedInput(selector, meta);
    this.onInput({ kind: "text", selector, length: String(value).length, secret: meta.secret === true, at: Date.now() });
    await this.send("Input.insertText", { text: String(value) });
  }

  async #clearFocusedInput(selector, meta) {
    const modifier = process.platform === "darwin" ? 4 : 2;
    const modifierKey = process.platform === "darwin"
      ? { key: "Meta", code: "MetaLeft", virtualKeyCode: 91 }
      : { key: "Control", code: "ControlLeft", virtualKeyCode: 17 };
    this.onInput({
      kind: "key", key: `${modifierKey.key}+A`, code: "KeyA", action: "select-all-input",
      selector, secret: meta.secret === true, at: Date.now(),
    });
    await this.send("Input.dispatchKeyEvent", {
      type: "keyDown", key: modifierKey.key, code: modifierKey.code, modifiers: modifier,
      windowsVirtualKeyCode: modifierKey.virtualKeyCode, nativeVirtualKeyCode: modifierKey.virtualKeyCode,
    });
    await this.send("Input.dispatchKeyEvent", {
      type: "keyDown", key: "a", code: "KeyA", modifiers: modifier,
      windowsVirtualKeyCode: 65, nativeVirtualKeyCode: 65,
    });
    await this.send("Input.dispatchKeyEvent", {
      type: "keyUp", key: "a", code: "KeyA", modifiers: modifier,
      windowsVirtualKeyCode: 65, nativeVirtualKeyCode: 65,
    });
    await this.send("Input.dispatchKeyEvent", {
      type: "keyUp", key: modifierKey.key, code: modifierKey.code, modifiers: 0,
      windowsVirtualKeyCode: modifierKey.virtualKeyCode, nativeVirtualKeyCode: modifierKey.virtualKeyCode,
    });
    await this.pressKey("Backspace", "Backspace", 8, { action: "clear-input" });
    await delay(25);

    const remaining = Number(await this.evaluate(`
      document.activeElement instanceof HTMLInputElement || document.activeElement instanceof HTMLTextAreaElement
        ? document.activeElement.value.length
        : 0
    `));
    if (remaining > 0) {
      this.onInput({
        kind: "key", key: "Backspace", code: "Backspace", repeat: Math.min(remaining + 2, 256),
        action: "clear-input-fallback", selector, secret: meta.secret === true, at: Date.now(),
      });
      await this.pressKey("End", "End", 35, { action: "move-input-caret-end", secret: meta.secret === true });
      for (let index = 0; index < Math.min(remaining + 2, 256); index += 1) {
        await this.send("Input.dispatchKeyEvent", {
          type: "keyDown", key: "Backspace", code: "Backspace",
          windowsVirtualKeyCode: 8, nativeVirtualKeyCode: 8,
        });
        await this.send("Input.dispatchKeyEvent", {
          type: "keyUp", key: "Backspace", code: "Backspace",
          windowsVirtualKeyCode: 8, nativeVirtualKeyCode: 8,
        });
      }
    }

    const cleared = await this.evaluate(`
      document.activeElement instanceof HTMLInputElement || document.activeElement instanceof HTMLTextAreaElement
        ? document.activeElement.value.length === 0
        : false
    `);
    if (!cleared) throw new Error(`real key input could not clear visible field: ${selector}`);
  }

  async pressKey(key, code = key, virtualKeyCode = 0, meta = {}) {
    this.onInput({ kind: "key", key, code, ...meta, at: Date.now() });
    await this.send("Input.dispatchKeyEvent", {
      type: "keyDown", key, code, windowsVirtualKeyCode: virtualKeyCode, nativeVirtualKeyCode: virtualKeyCode,
    });
    await this.send("Input.dispatchKeyEvent", {
      type: "keyUp", key, code, windowsVirtualKeyCode: virtualKeyCode, nativeVirtualKeyCode: virtualKeyCode,
    });
  }

  async pressKeyChord(keys, meta = {}) {
    this.onInput({
      kind: "keyChord",
      keys: keys.map((entry) => entry.key),
      codes: keys.map((entry) => entry.code),
      ...meta,
      at: Date.now(),
    });
    let modifiers = 0;
    for (const entry of keys) {
      modifiers |= modifierMask(entry.key);
      await this.send("Input.dispatchKeyEvent", {
        type: "keyDown",
        key: entry.key,
        code: entry.code,
        modifiers,
        windowsVirtualKeyCode: entry.vk,
        nativeVirtualKeyCode: entry.vk,
      });
    }
    await delay(80);
    for (const entry of [...keys].reverse()) {
      const entryModifier = modifierMask(entry.key);
      const keyUpModifiers = entryModifier ? modifiers & ~entryModifier : modifiers;
      await this.send("Input.dispatchKeyEvent", {
        type: "keyUp",
        key: entry.key,
        code: entry.code,
        modifiers: keyUpModifiers,
        windowsVirtualKeyCode: entry.vk,
        nativeVirtualKeyCode: entry.vk,
      });
      modifiers = keyUpModifiers;
    }
  }

  async holdKeyChord(keys, durationMs = 700, meta = {}) {
    const holdMs = Math.max(100, Math.min(2_500, Number(durationMs) || 700));
    this.onInput({
      kind: "keyHoldChord",
      keys: keys.map((entry) => entry.key),
      codes: keys.map((entry) => entry.code),
      durationMs: holdMs,
      ...meta,
      at: Date.now(),
    });
    let modifiers = 0;
    const pressed = [];
    try {
      for (const entry of keys) {
        modifiers |= modifierMask(entry.key);
        await this.send("Input.dispatchKeyEvent", {
          type: "keyDown",
          key: entry.key,
          code: entry.code,
          modifiers,
          windowsVirtualKeyCode: entry.vk,
          nativeVirtualKeyCode: entry.vk,
        });
        pressed.push(entry);
      }
      await delay(holdMs);
    } finally {
      for (const entry of [...pressed].reverse()) {
        const entryModifier = modifierMask(entry.key);
        const keyUpModifiers = entryModifier ? modifiers & ~entryModifier : modifiers;
        await this.send("Input.dispatchKeyEvent", {
          type: "keyUp",
          key: entry.key,
          code: entry.code,
          modifiers: keyUpModifiers,
          windowsVirtualKeyCode: entry.vk,
          nativeVirtualKeyCode: entry.vk,
        });
        modifiers = keyUpModifiers;
      }
    }
  }

  async capture(filePath) {
    const shot = await this.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
  }

  outgoingCommandAudit() {
    const commands = [];
    const violations = [];
    for (const frame of this.wsSent) {
      if (!isGameplayWebSocketUrl(frame.url)) continue;
      let command;
      try {
        command = JSON.parse(frame.payloadData);
      } catch {
        continue;
      }
      if (!command || typeof command !== "object") continue;
      commands.push({ at: frame.at, type: command.type ?? null });
      const recentInputs = this.inputs.filter(
        (input) => input.at <= frame.at + 250 && input.at >= frame.at - 60_000,
      );
      const result = auditOutgoingBrowserCommand(command, { recentInputs });
      if (!result.ok) violations.push({ at: frame.at, type: command.type ?? null, reason: result.reason });
    }
    return { commands, violations };
  }

  close() {
    this.#rejectPending("CDP client closed");
    this.ws?.close();
  }
}

export async function readAgentState(client) {
  const expression = `
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const entities = Array.isArray(state.entities) ? state.entities : [];
      const self = entities.find((entry) => String(entry?.objectId) === String(state.playerObjectId)) ?? null;
      const validPosition = (entry) => (
        entry != null &&
        Number.isFinite(Number(entry.x)) && Number(entry.x) > 0 &&
        Number.isFinite(Number(entry.y)) && Number(entry.y) > 0
      );
      const visiblePlayer = validPosition(self)
        ? self
        : validPosition(state.authoritativePlayer)
          ? state.authoritativePlayer
          : validPosition(state.player)
            ? state.player
            : null;
      const authoritativePlayer = validPosition(state.authoritativePlayer)
        ? state.authoritativePlayer
        : visiblePlayer;
      const compactItem = (item) => ({
        key: item?.key ?? null,
        name: item?.name ?? null,
        uniqueId: item?.uniqueId ?? null,
        slot: item?.slot ?? null,
        container: item?.container ?? null,
        quantity: item?.quantity ?? 1,
        sellValue: item?.sellValue ?? 0,
        equipSlot: item?.equipSlot ?? null,
      });
      return {
        capturedAt: Date.now(),
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        reconnectStatus: state.reconnectStatus ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        sceneInteractionReady: state.sceneInteractionReady === true,
        playerObjectId: state.playerObjectId ?? null,
        playerClass: self?.classKey ?? self?.class ?? null,
        // Policy and evidence must follow the server-acknowledged transform.
        // The rendered entity may briefly contain a local prediction for a
        // rejected step; treating that frame as movement creates false A<->B
        // navigation loops. Keep the rendered transform separately for visual
        // diagnostics and use it only as a bootstrap fallback before the first
        // authoritative movement acknowledgement exists.
        player: authoritativePlayer
          ? { x: authoritativePlayer.x, y: authoritativePlayer.y, direction: authoritativePlayer.direction }
          : null,
        renderedPlayer: visiblePlayer
          ? { x: visiblePlayer.x, y: visiblePlayer.y, direction: visiblePlayer.direction }
          : null,
        playerHp: state.playerHp ?? self?.hp ?? null,
        playerMaxHp: state.playerMaxHp ?? self?.maxHp ?? null,
        playerMp: state.playerMp ?? self?.mp ?? null,
        playerMaxMp: state.playerMaxMp ?? self?.maxMp ?? null,
        playerLevel: self?.level ?? null,
        playerDead: self?.dead === true || state.playerHp === 0,
        playerExperience: state.playerExperience ?? null,
        playerMaxExperience: state.playerMaxExperience ?? null,
        gold: state.gold ?? null,
        credit: state.credit ?? null,
        selectedObjectId: state.selectedObjectId ?? null,
        movementPlan: state.movementPlan ?? null,
        mapTransfers: (Array.isArray(state.mapTransfers) ? state.mapTransfers : []).map((transfer) => ({
          key: transfer?.key ?? null,
          mapFileName: transfer?.mapFileName ?? null,
          minX: transfer?.minX ?? null,
          maxX: transfer?.maxX ?? null,
          minY: transfer?.minY ?? null,
          maxY: transfer?.maxY ?? null,
          toMapFileName: transfer?.toMapFileName ?? null,
          toMapTitle: transfer?.toMapTitle ?? null,
        })),
        // Physical hit testing is intentionally collected on demand for the
        // handful of candidate object ids about to be clicked. Performing nine
        // elementFromPoint samples for every rendered entity on every movement
        // poll can monopolize the browser renderer during long autonomous runs.
        entityHitTargets: [],
        entities: entities.slice(0, 300).map((entry) => ({
          objectId: entry?.objectId ?? null,
          kind: entry?.kind ?? null,
          name: entry?.name ?? null,
          x: entry?.x ?? null,
          y: entry?.y ?? null,
          hp: entry?.hp ?? null,
          maxHp: entry?.maxHp ?? null,
          dead: entry?.dead === true,
          disposition: entry?.disposition ?? null,
          // These rendered timestamps are the only evidence that an
          // incidental monster is actively attacking during travel. Preserve
          // them in the read-only policy snapshot; otherwise every attacker
          // is indistinguishable from an idle occupancy obstacle.
          attackStartedAt: entry?.attackStartedAt ?? null,
          attackUntil: entry?.attackUntil ?? null,
        })),
        groundDrops: (Array.isArray(state.groundDrops) ? state.groundDrops : []).slice(0, 100),
        questLog: (Array.isArray(state.questLog) ? state.questLog : []).map((quest) => ({
          questId: quest?.questId ?? null,
          title: quest?.title ?? null,
          stage: quest?.stage ?? null,
          current: quest?.current ?? null,
          required: quest?.required ?? null,
          objective: quest?.objective ?? null,
          progressLabel: quest?.progressLabel ?? null,
          objectives: Array.isArray(quest?.objectives) ? quest.objectives : [],
          rewards: quest?.rewards ?? null,
          rewardPreview: quest?.rewardPreview ?? null,
        })),
        activeNpcDialog: state.activeNpcDialog ? {
          npcObjectId: state.activeNpcDialog.npcObjectId ?? null,
          npcName: state.activeNpcDialog.npcName ?? null,
          title: state.activeNpcDialog.title ?? null,
          body: Array.isArray(state.activeNpcDialog.body) ? state.activeNpcDialog.body : [],
          links: Array.isArray(state.activeNpcDialog.links)
            ? state.activeNpcDialog.links.map((link) => ({ text: link?.text ?? null, target: link?.target ?? null }))
            : [],
        } : null,
        inventoryItems: (Array.isArray(state.inventoryItems) ? state.inventoryItems : []).map(compactItem),
        beltItems: (Array.isArray(state.beltItems) ? state.beltItems : []).map(compactItem),
        equipmentItems: (Array.isArray(state.equipmentItems) ? state.equipmentItems : []).map((item) => ({
          slot: item?.slot ?? null,
          name: item?.name ?? null,
          durabilityCurrent: item?.durabilityCurrent ?? null,
          durabilityMax: item?.durabilityMax ?? null,
        })),
        knownSkills: (Array.isArray(state.knownSkills) ? state.knownSkills : []).map((skill) => ({
          key: skill?.key ?? null,
          name: skill?.name ?? null,
          spell: skill?.spell ?? null,
          castKind: skill?.castKind ?? null,
          offensive: skill?.offensive === true,
          hotkey: skill?.hotkey ?? null,
          delayMs: skill?.delayMs ?? null,
          castTimeMs: skill?.castTimeMs ?? null,
          cooldownRemainingTicks: skill?.cooldownRemainingTicks ?? 0,
        })),
        logs: (Array.isArray(state.logs) ? state.logs : []).slice(-20).map((line) => ({
          text: line?.text ?? String(line ?? ""), type: line?.type ?? null,
        })),
        questWindowOpen: document.querySelector('[data-quest-stage-filter]') != null,
        deathOverlayVisible: document.querySelector('[data-testid="town-revive-button"]') != null,
        loginFeedback: document.querySelector('.login-feedback')?.textContent?.trim() ?? null,
      };
    })()
  `;
  let snapshot = null;
  // Town revive/map bootstrap can briefly materialize the self entity at the
  // protocol sentinel 0,0 before its authoritative transform arrives. Never
  // feed that non-playable coordinate into collision routing: it can poison
  // every source-field cooldown even though the next rendered frame is valid.
  for (let attempt = 0; attempt < 8; attempt += 1) {
    snapshot = await client.evaluate(expression);
    const needsSettledPlayer = snapshot?.screen === "game" &&
      snapshot?.sceneInteractionReady === true &&
      snapshot?.wsState === "open" &&
      snapshot?.player == null &&
      snapshot?.playerDead !== true;
    if (!needsSettledPlayer) return snapshot;
    await delay(125);
  }
  throw new Error("rendered authoritative player position did not settle after sentinel 0,0");
}

export async function renderGameToText(client) {
  return client.evaluate(`typeof window.render_game_to_text === "function" ? window.render_game_to_text() : null`);
}

export async function waitUntil(client, expression, timeoutMs, pollMs = 120) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      if (await client.evaluate(`Boolean(${expression})`)) return true;
    } catch (error) {
      lastError = error;
    }
    await delay(pollMs);
  }
  if (lastError) throw lastError;
  return false;
}

export async function elementCenter(client, selector, text = null) {
  return client.evaluate(`
    (() => {
      const nodes = Array.from(document.querySelectorAll(${JSON.stringify(selector)}));
      const wanted = ${JSON.stringify(text)};
      const candidates = nodes.filter((entry) => {
        if (!(entry instanceof HTMLElement)) return false;
        const box = entry.getBoundingClientRect();
        if (box.width <= 0 || box.height <= 0) return false;
        if (wanted === null) return true;
        return (entry.innerText || entry.textContent || "").trim().includes(wanted);
      });
      for (const node of candidates) {
        if (!(node instanceof HTMLElement)) continue;
        const box = node.getBoundingClientRect();
        const samples = [
          [0.5, 0.5], [0.25, 0.25], [0.75, 0.25], [0.25, 0.75], [0.75, 0.75],
          [0.5, 0.2], [0.5, 0.8], [0.2, 0.5], [0.8, 0.5],
        ];
        for (const [fx, fy] of samples) {
          const x = box.left + box.width * fx;
          const y = box.top + box.height * fy;
          const top = document.elementFromPoint(x, y);
          if (top === node || node.contains(top)) {
            return { x, y, width: box.width, height: box.height };
          }
        }
      }
      return null;
    })()
  `);
}

async function rawElementCenter(client, selector) {
  return client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!(node instanceof HTMLElement)) return null;
      const box = node.getBoundingClientRect();
      if (box.width <= 0 || box.height <= 0) return null;
      return { x: box.left + box.width / 2, y: box.top + box.height / 2, width: box.width, height: box.height };
    })()
  `);
}

async function tileScreenCenter(client, x, y) {
  return client.evaluate(`
    (() => {
      const stage = document.querySelector('.client-stage-frame');
      if (!(stage instanceof HTMLElement)) return null;
      const box = stage.getBoundingClientRect();
      const playerX = Number(stage.dataset.viewportPlayerX);
      const playerY = Number(stage.dataset.viewportPlayerY);
      const centerX = Number(stage.dataset.viewportTileCenterX);
      const centerY = Number(stage.dataset.viewportTileCenterY);
      const cellWidth = Number(stage.dataset.viewportCellWidth);
      const cellHeight = Number(stage.dataset.viewportCellHeight);
      if (![playerX, playerY, centerX, centerY, cellWidth, cellHeight].every(Number.isFinite)) return null;
      const screenX = box.left + centerX + (${Number(x)} - playerX) * cellWidth;
      const screenY = box.top + centerY + (${Number(y)} - playerY) * cellHeight;
      if (screenX < box.left || screenX > box.right || screenY < box.top || screenY > box.bottom) return null;
      return { x: screenX, y: screenY, width: cellWidth, height: cellHeight };
    })()
  `);
}

export async function launchBrowser({ url, headed = false, width = 1024, height = 768, onInput = () => {} }) {
  const chromePath = findChromePath();
  if (!chromePath) throw new Error("Chrome/Edge/Brave was not found; set MIR2_CHROME_PATH");
  const userDataDir = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-quest-agent-"));
  const chrome = spawn(chromePath, [
    "--remote-debugging-port=0",
    `--user-data-dir=${userDataDir}`,
    ...(headed ? [] : ["--headless=new"]),
    "--ignore-gpu-blocklist",
    "--enable-webgl",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
    "--no-first-run",
    "--no-default-browser-check",
    `--window-size=${width},${height}`,
    "about:blank",
  ], { stdio: "ignore" });

  const debugPort = await waitForDebugPort(userDataDir, chrome);
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/new?${encodeURIComponent(url)}`, { method: "PUT" });
  if (!response.ok) throw new Error(`Chrome target creation failed: ${response.status}`);
  const target = await response.json();
  const client = new CdpClient(target.webSocketDebuggerUrl, onInput);
  await client.connect();
  await client.enable();
  await client.setViewport(width, height);
  await waitUntil(client, "document.readyState === 'complete' || document.readyState === 'interactive'", 30_000);
  return { chrome, client, userDataDir, debugPort };
}

export async function stopBrowser(browser) {
  browser?.client?.close();
  const chrome = browser?.chrome;
  if (chrome && chrome.exitCode === null && !chrome.killed) {
    chrome.kill("SIGTERM");
    await Promise.race([
      new Promise((resolve) => chrome.once("exit", resolve)),
      delay(5_000),
    ]);
  }
  if (browser?.userDataDir) await fs.rm(browser.userDataDir, { recursive: true, force: true });
}

export function wsPacketsSince(client, since, packetName) {
  return wsEventFramesSince(client, since, "packet")
    .filter(({ event }) => event?.packet === packetName)
    .map(({ event }) => event.payload ?? event);
}

export function wsEventFramesSince(client, since, eventType) {
  const events = [];
  for (const frame of client.wsReceived) {
    if (frame.at < since || !isGameplayWebSocketUrl(frame.url)) continue;
    try {
      const event = JSON.parse(frame.payloadData);
      if (event?.type === eventType) events.push({ at: frame.at, event });
    } catch {
      // Binary/keepalive frames are outside this evidence view.
    }
  }
  return events;
}

export function targetCombatEvidenceSince(client, since, targetObjectId, ownerObjectId = null) {
  const targetId = String(targetObjectId);
  const ownerId = ownerObjectId == null ? null : String(ownerObjectId);
  const matching = (packetName, objectIdKey = "objectId") => wsPacketsSince(client, since, packetName)
    .filter((payload) => String(payload?.[objectIdKey] ?? payload?.info?.[objectIdKey]) === targetId);
  const ownerAttacks = ownerId == null
    ? []
    : wsPacketsSince(client, since, "ObjectAttack")
      .filter((payload) => String(payload?.objectId ?? payload?.info?.objectId) === ownerId);
  const struck = matching("ObjectStruck");
  const health = matching("ObjectHealth");
  const damage = matching("DamageIndicator");
  const died = matching("ObjectDied");
  return {
    ownerAttackCount: ownerAttacks.length,
    struckCount: struck.length,
    healthCount: health.length,
    damageCount: damage.length,
    diedCount: died.length,
    targetResponded: struck.length + health.length + damage.length + died.length > 0,
    targetDied: died.length > 0,
  };
}

export function isGameplayWebSocketUrl(value) {
  if (!value) return false;
  try {
    return new URL(value).pathname === "/ws";
  } catch {
    return false;
  }
}

function modifierMask(key) {
  if (key === "Alt") return 1;
  if (key === "Control") return 2;
  if (key === "Meta") return 4;
  if (key === "Shift") return 8;
  return 0;
}

async function waitForDebugPort(userDataDir, chrome) {
  const file = path.join(userDataDir, "DevToolsActivePort");
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (chrome.exitCode !== null) throw new Error(`Chrome exited before CDP became ready (${chrome.exitCode})`);
    try {
      const content = await fs.readFile(file, "utf8");
      const port = Number(content.split(/\r?\n/, 1)[0]);
      if (Number.isFinite(port) && port > 0) return port;
    } catch {
      // The file appears only after Chrome has bound the debug socket.
    }
    await delay(100);
  }
  throw new Error("Timed out waiting for Chrome DevToolsActivePort");
}

function findChromePath() {
  const candidates = [
    process.env.MIR2_CHROME_PATH,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  ].filter(Boolean);
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function round(value) {
  return Math.round(value * 10) / 10;
}

export function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
