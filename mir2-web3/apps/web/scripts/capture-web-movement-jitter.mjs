import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const DEFAULT_BASE_URL = "http://127.0.0.1:3002";
const DEFAULT_OUTPUT_DIR = path.resolve(process.cwd(), "docs", "generated", "player-qa", "movement-jitter");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };
const DEFAULT_ACCOUNT = "QA0429A";
const DEFAULT_PASSWORD = "Mir2test1";

const args = parseArgs(process.argv.slice(2));
const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL;
const outputDir = path.resolve(args.output ?? DEFAULT_OUTPUT_DIR);
const prefix = args.prefix ?? `movement-jitter-${Date.now()}`;
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? DEFAULT_ACCOUNT;
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? DEFAULT_PASSWORD;
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, false);
const characterName = args.characterName ?? defaultCharacterName();
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9500 + (process.pid % 1000));
const sampleMs = numberArg(args.sampleMs, 50);
const interaction = args.interaction ?? "click";
const holdButton = args.button ?? args.holdButton ?? "right";
const holdMs = numberArg(args.holdMs, 2200);
const startMap = args.map ?? "0";
const startX = numberArg(args.x, 330);
const startY = numberArg(args.y, 270);
const targetDx = numberArg(args.targetDx, 10);
const targetDy = numberArg(args.targetDy, 0);
const fixedSpriteX = numberArg(args.fixedSpriteX ?? args.x, startX);
const fixedSpriteY = numberArg(args.fixedSpriteY ?? args.y, startY);

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
    await login(client);
    await installSendProbe(client);
    await transferTo(client, startMap, startX, startY);
    await delay(800);

    const start = await readMovementState(client);
    const route = buildRoute(start.player);
    const samples = [];
    const actions = [];

    if (interaction === "packetRun" || interaction === "packetWalk") {
      const commandType = interaction === "packetRun" ? "run" : "walk";
      const step = {
        label: `${interaction}-${args.direction ?? "Right"}`,
        mode: interaction === "packetRun" ? "run" : "walk",
        x: start.player.x,
        y: start.player.y,
        durationMs: 2800,
      };
      for (let index = 0; index < 5; index += 1) {
        const before = await readMovementState(client);
        const ok = await client.evaluate(`
          window.__mir2Stage5?.send?.(${JSON.stringify({ type: commandType, direction: args.direction ?? "Right" })}) === true
        `);
        await delay(700);
        const afterDispatch = await readMovementState(client);
        actions.push({
          ...step,
          interaction,
          dispatch: { type: commandType, direction: args.direction ?? "Right", ok, index },
          before: compactState(before),
          afterDispatch: compactState(afterDispatch),
        });
      }
      samples.push(...(await sampleMovement(client, step.label, step.durationMs)));
    } else if (interaction === "hold") {
      const step = {
        label: `hold-${holdButton === "right" ? "run" : "walk"}-${targetDx},${targetDy}`,
        mode: holdButton === "right" ? "run" : "walk",
        x: start.player.x + targetDx,
        y: start.player.y + targetDy,
        durationMs: holdMs + 600,
      };
      const before = await readMovementState(client);
      const hold = await beginHoldTile(client, step.x, step.y, holdButton);
      await delay(Math.min(80, sampleMs));
      const afterDispatch = await readMovementState(client);
      actions.push({
        ...step,
        interaction,
        dispatch: hold.dispatch,
        before: compactState(before),
        afterDispatch: compactState(afterDispatch),
      });
      const releaseAfterHold = (async () => {
        await delay(holdMs);
        await hold.release();
      })();
      try {
        samples.push(...(await sampleMovement(client, step.label, step.durationMs)));
      } finally {
        await hold.release().catch(() => undefined);
        await releaseAfterHold.catch(() => undefined);
      }
    } else if (interaction === "clickTarget") {
      const step = {
        label: `click-target-${targetDx},${targetDy}`,
        mode: holdButton === "left" ? "walk" : "run",
        x: start.player.x + targetDx,
        y: start.player.y + targetDy,
        durationMs: Math.max(holdMs, 5200),
      };
      const before = await readMovementState(client);
      const dispatch = await clickTile(client, step.x, step.y, step.mode === "run" ? "right" : "left");
      await delay(Math.min(120, sampleMs));
      const afterDispatch = await readMovementState(client);
      actions.push({
        ...step,
        interaction,
        dispatch,
        before: compactState(before),
        afterDispatch: compactState(afterDispatch),
      });
      samples.push(...(await sampleMovement(client, step.label, step.durationMs)));
    } else {
      for (const step of route) {
        const before = await readMovementState(client);
        let dispatch;
        if (interaction === "direct") {
          dispatch = await sendMoveTo(client, step.x, step.y, step.mode);
        } else {
          dispatch = await clickTile(client, step.x, step.y, step.mode === "run" ? "right" : "left");
        }
        await delay(Math.min(80, sampleMs));
        const afterDispatch = await readMovementState(client);
        actions.push({
          ...step,
          interaction,
          dispatch,
          before: compactState(before),
          afterDispatch: compactState(afterDispatch),
        });
        samples.push(...(await sampleMovement(client, step.label, step.durationMs)));
      }
    }

    const finalState = await readMovementState(client);
    const screenshotPath = path.join(outputDir, `${prefix}.png`);
    const statePath = path.join(outputDir, `${prefix}.json`);
    const screenshot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    await fs.writeFile(screenshotPath, Buffer.from(screenshot.data, "base64"));

    const report = {
      ok: true,
      baseUrl,
      account,
      createAccount,
      characterName: createAccount ? characterName : undefined,
      interaction,
      startTarget: { map: startMap, x: startX, y: startY },
      startedAt: new Date().toISOString(),
      start,
      finalState,
      sampleMs,
      holdMs: interaction === "hold" ? holdMs : undefined,
      sampleCount: samples.length,
      actions,
      jumps: detectJumps(samples),
      summary: summarizeSamples(samples),
      samples,
      nonFaviconNetwork404s: [...new Set(client.network404s)],
      consoleErrors: client.consoleErrors,
      screenshotPath,
    };
    await fs.writeFile(statePath, `${JSON.stringify(report, null, 2)}\n`);
    console.log(JSON.stringify({ ok: true, statePath, screenshotPath, summary: report.summary, jumps: report.jumps }, null, 2));
  } finally {
    client?.close();
    await stopChrome(chrome);
    await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => undefined);
  }
}

async function transferTo(client, map, x, y) {
  const alreadyThere = await client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state;
      return state?.mapFileName === ${JSON.stringify(map)}
        && state?.player?.x === ${Number(x)}
        && state?.player?.y === ${Number(y)};
    })()
  `);
  if (alreadyThere) return;

  await client.evaluate(`
    window.__mir2Stage5?.send?.(${JSON.stringify({ type: "transferMap", key: `crystal:${map}:${x}:${y}` })}) === true
  `);
  await waitUntil(
    client,
    `
      (() => {
        const state = window.__mir2Stage5?.state;
        return state?.mapFileName === ${JSON.stringify(map)}
          && state?.player?.x === ${Number(x)}
          && state?.player?.y === ${Number(y)};
      })()
    `,
    "movement test start transfer",
    20_000,
  );
}

async function installSendProbe(client) {
  await client.evaluate(`
    (() => {
      const sentKey = "__mir2MovementSentCommands";
      const receivedKey = "__mir2MovementReceivedPackets";
      window[sentKey] = [];
      window[receivedKey] = [];

      if (!window.__mir2MovementWebSocketProbeWrapped) {
        const NativeWebSocket = window.WebSocket;
        window.WebSocket = class Mir2MovementProbeWebSocket extends NativeWebSocket {
          constructor(...args) {
            super(...args);
            this.addEventListener("message", (event) => {
              try {
                const message = JSON.parse(String(event.data ?? ""));
                if (
                  message &&
                  message.type === "packet" &&
                  ["UserLocation", "ObjectWalk", "ObjectRun", "ObjectBackStep", "ObjectSitDown"].includes(message.packet)
                ) {
                  window[receivedKey].push({
                    t: Date.now(),
                    packet: message.packet,
                    payload: message.payload,
                  });
                }
              } catch {
                // Best-effort QA probe only.
              }
            });
          }
        };
        Object.defineProperty(window.WebSocket, "__mir2MovementWebSocketProbeWrapped", {
          value: true,
          configurable: true,
        });
        Object.defineProperty(window, "__mir2MovementWebSocketProbeWrapped", {
          value: true,
          configurable: true,
        });
      }

      if (!WebSocket.prototype.__mir2MovementSendProbeWrapped) {
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
            if (command && ["moveTo", "walk", "run", "turn"].includes(command.type)) {
              window[sentKey].push({
                t: Date.now(),
                command,
              });
            }
          } catch {
            // Best-effort QA probe only.
          }
          return originalWebSocketSend.apply(this, arguments);
        };
        Object.defineProperty(WebSocket.prototype, "__mir2MovementSendProbeWrapped", {
          value: true,
          configurable: true,
        });
      }

      const wrapStage = (stage) => {
        if (!stage || stage.__mir2MovementSendProbeWrapped || typeof stage.send !== "function") {
          return stage;
        }

        const originalSend = stage.send;
        stage.send = function(command) {
          try {
            if (command && ["moveTo", "walk", "run", "turn"].includes(command.type)) {
              window[sentKey].push({
                t: Date.now(),
                command: JSON.parse(JSON.stringify(command)),
              });
            }
          } catch {
            // Best-effort QA probe only.
          }
          return originalSend.apply(this, arguments);
        };
        Object.defineProperty(stage, "__mir2MovementSendProbeWrapped", {
          value: true,
          configurable: true,
        });
        return stage;
      };

      const descriptor = Object.getOwnPropertyDescriptor(window, "__mir2Stage5");
      if (descriptor && descriptor.get?.__mir2MovementSendProbeAccessor) {
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
      getter.__mir2MovementSendProbeAccessor = true;
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

async function login(client) {
  await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'login'", "login screen", 15_000);
  await fillInput(client, ".login-input.account", account);
  await fillInput(client, ".login-input.password", password);

  if (createAccount) {
    await click(client, ".login-button.account button");
    await waitUntil(client, "window.__mir2Stage5?.state?.wsState === 'open'", "account creation socket", 15_000);
    await delay(800);
  }

  await click(client, ".login-button.ok button");
  await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'select'", "select screen", 15_000);

  if (createAccount) {
    const created = await client.evaluate(`
      window.__mir2Stage5?.send?.(${JSON.stringify({
        type: "newCharacter",
        name: characterName,
        gender: "male",
        class: "warrior",
      })}) === true
    `);
    if (!created) throw new Error(`Failed to create movement QA character ${characterName}`);

    await waitUntil(
      client,
      `
        Array.isArray(window.__mir2Stage5?.state?.characters)
          && window.__mir2Stage5.state.characters.some((character) => character?.name === ${JSON.stringify(characterName)})
      `,
      "movement QA character creation",
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
    if (!started) throw new Error(`Failed to start movement QA character ${characterName}`);
  } else {
    await click(client, ".select-action.start button");
  }

  await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'game'", "game screen", 20_000);
  await waitUntil(client, "!document.querySelector('.login-transition-overlay')", "login transition cleared", 5_000);
}

function buildRoute(player) {
  if (!player) throw new Error("No player state available.");
  return [
    { label: "run-right-1", mode: "run", x: player.x + 2, y: player.y, durationMs: 900 },
    { label: "run-right-2", mode: "run", x: player.x + 4, y: player.y, durationMs: 900 },
    { label: "walk-down-1", mode: "walk", x: player.x + 4, y: player.y + 1, durationMs: 900 },
    { label: "walk-left-1", mode: "walk", x: player.x + 3, y: player.y + 1, durationMs: 900 },
  ];
}

async function sendMoveTo(client, x, y, mode) {
  const ok = await client.evaluate(`
    window.__mir2Stage5?.send?.(${JSON.stringify({ type: "moveTo", x, y, mode })}) === true
  `);
  if (!ok) throw new Error(`Failed to send moveTo ${x},${y},${mode}`);
  return { type: "direct", ok, x, y, mode };
}

async function clickTile(client, x, y, button) {
  const point = await tilePoint(client, x, y);
  if (!point) throw new Error(`Could not find tile ${x},${y}`);

  await client.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: point.x,
    y: point.y,
    button: "none",
  });
  await client.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: point.x,
    y: point.y,
    button,
    buttons: button === "right" ? 2 : 1,
    clickCount: 1,
  });
  await client.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: point.x,
    y: point.y,
    button,
    buttons: 0,
    clickCount: 1,
  });
  return { type: "tile", x, y, button, clientX: point.x, clientY: point.y };
}

async function holdTile(client, x, y, button, durationMs) {
  const hold = await beginHoldTile(client, x, y, button);
  await delay(durationMs);
  await hold.release();
  return { ...hold.dispatch, durationMs };
}

async function beginHoldTile(client, x, y, button) {
  const point = await tilePoint(client, x, y);
  if (!point) throw new Error(`Could not find tile ${x},${y}`);

  await client.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: point.x,
    y: point.y,
    button: "none",
  });
  await client.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: point.x,
    y: point.y,
    button,
    buttons: button === "right" ? 2 : 1,
    clickCount: 1,
  });

  let released = false;
  return {
    dispatch: { type: "hold-tile", x, y, button, clientX: point.x, clientY: point.y },
    release: async () => {
      if (released) return;
      released = true;
      await client.send("Input.dispatchMouseEvent", {
        type: "mouseReleased",
        x: point.x,
        y: point.y,
        button,
        buttons: 0,
        clickCount: 1,
      });
    },
  };
}

async function tilePoint(client, x, y) {
  return client.evaluate(`
    (() => {
      const tile = document.querySelector(${JSON.stringify(`[aria-label="tile ${x}, ${y}"]`)});
      if (!tile) return null;
      const box = tile.getBoundingClientRect();
      return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
    })()
  `);
}

async function sampleMovement(client, label, durationMs) {
  const samples = [];
  const startedAt = Date.now();
  while (Date.now() - startedAt <= durationMs) {
    samples.push({ label, t: Date.now() - startedAt, ...(await readMovementState(client)) });
    await delay(sampleMs);
  }
  return samples;
}

async function readMovementState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const player = state.player ?? null;
      const self = player
        ? (state.entities ?? []).find((entity) => entity.x === player.x && entity.y === player.y && (entity.kind === "selfPlayer" || entity.objectId === state.playerObjectId))
        : null;
      const stage = document.querySelector(".client-stage-frame")?.getBoundingClientRect();
      const sprite = document.querySelector(".entity-sprite-stack.self")?.getBoundingClientRect();
      const selfNameplate = document.querySelector(".entity-nameplate.self");
      const nameplate = selfNameplate?.getBoundingClientRect();
      const floorNode = document.querySelector(".game-scene-floor, .scene-map-floor-sprite, .game-scene-backdrop img");
      const floor = floorNode?.getBoundingClientRect();
      const fixedSpriteNode =
        document.querySelector(${JSON.stringify(`[data-map-sprite-key*=":${fixedSpriteX}:${fixedSpriteY}:"]`)}) ??
        document.querySelector(${JSON.stringify(`[data-map-sprite-key*=":${fixedSpriteX + 2}:${fixedSpriteY}:"]`)}) ??
        null;
      const fixedSprite = fixedSpriteNode?.getBoundingClientRect();
      const sentMoves = Array.isArray(window.__mir2MovementSentCommands)
        ? window.__mir2MovementSentCommands
        : [];
      const receivedMoves = Array.isArray(window.__mir2MovementReceivedPackets)
        ? window.__mir2MovementReceivedPackets
        : [];
      const commandHistory = Array.isArray(window.__mir2CommandHistory)
        ? window.__mir2CommandHistory
        : [];
      const gatewayEvents = Array.isArray(window.__mir2GatewayEventHistory)
        ? window.__mir2GatewayEventHistory
        : [];
      const rect = (value) => value ? ({
        left: Math.round(value.left * 100) / 100,
        top: Math.round(value.top * 100) / 100,
        width: Math.round(value.width * 100) / 100,
        height: Math.round(value.height * 100) / 100,
      }) : null;
      return {
        screen: state.screen ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        player,
        predictedPlayer: state.predictedPlayer ?? null,
        movementPlan: state.movementPlan ?? null,
        directionStepPending: state.directionStepPending ?? null,
        selfEntity: self ? { x: self.x, y: self.y, direction: self.direction, kind: self.kind, objectId: self.objectId } : null,
        sprite: rect(sprite),
        nameplate: rect(nameplate),
        floor: rect(floor),
        floorKey: floorNode?.getAttribute?.("data-map-sprite-key") ?? null,
        centerSprite: rect(fixedSprite),
        centerSpriteKey: fixedSpriteNode?.getAttribute?.("data-map-sprite-key") ?? null,
        stage: rect(stage),
        logsTail: (state.logs ?? []).slice(-5).map((line) => line.text ?? String(line)),
        sentMoveTail: sentMoves.slice(-8),
        commandTail: commandHistory
          .filter((entry) => ["moveTo", "walk", "run", "turn"].includes(entry?.type))
          .slice(0, 8),
        receivedMoveTail: receivedMoves.slice(-8),
        gatewayMoveTail: gatewayEvents
          .filter((entry) => ["UserLocation", "ObjectWalk", "ObjectRun", "ObjectBackStep", "ObjectSitDown"].includes(entry?.packet))
          .slice(0, 8),
      };
    })()
  `);
}

function detectJumps(samples) {
  const jumps = [];
  let previous = null;
  for (const sample of samples) {
    if (previous && sample.label === previous.label) {
      const spriteDx = delta(sample.sprite?.left, previous.sprite?.left);
      const spriteDy = delta(sample.sprite?.top, previous.sprite?.top);
      const nameDx = delta(sample.nameplate?.left, previous.nameplate?.left);
      const nameDy = delta(sample.nameplate?.top, previous.nameplate?.top);
      const centerDx =
        sample.centerSpriteKey && previous.centerSpriteKey && sample.centerSpriteKey === previous.centerSpriteKey
          ? delta(sample.centerSprite?.left, previous.centerSprite?.left)
          : 0;
      const centerDy =
        sample.centerSpriteKey && previous.centerSpriteKey && sample.centerSpriteKey === previous.centerSpriteKey
          ? delta(sample.centerSprite?.top, previous.centerSprite?.top)
          : 0;
      const centerBacktrack = centerDx > 12 || Math.abs(centerDy) > 12;
      if (Math.max(Math.abs(spriteDx), Math.abs(spriteDy), Math.abs(nameDx), Math.abs(nameDy)) > 12 || centerBacktrack) {
        jumps.push({
          label: sample.label,
          t: sample.t,
          spriteDx,
          spriteDy,
          nameDx,
          nameDy,
          centerDx,
          centerDy,
          from: compactSample(previous),
          to: compactSample(sample),
        });
      }
    }
    previous = sample;
  }
  return jumps;
}

function summarizeSamples(samples) {
  const byLabel = new Map();
  for (const sample of samples) {
    const entry = byLabel.get(sample.label) ?? {
      count: 0,
      firstPlayer: sample.player,
      lastPlayer: null,
      minSpriteLeft: Number.POSITIVE_INFINITY,
      maxSpriteLeft: Number.NEGATIVE_INFINITY,
      minSpriteTop: Number.POSITIVE_INFINITY,
      maxSpriteTop: Number.NEGATIVE_INFINITY,
    };
    entry.count += 1;
    entry.lastPlayer = sample.player;
    if (sample.sprite) {
      entry.minSpriteLeft = Math.min(entry.minSpriteLeft, sample.sprite.left);
      entry.maxSpriteLeft = Math.max(entry.maxSpriteLeft, sample.sprite.left);
      entry.minSpriteTop = Math.min(entry.minSpriteTop, sample.sprite.top);
      entry.maxSpriteTop = Math.max(entry.maxSpriteTop, sample.sprite.top);
    }
    byLabel.set(sample.label, entry);
  }

  return Array.from(byLabel, ([label, entry]) => ({
    label,
    ...entry,
    spriteLeftRange: finiteRange(entry.minSpriteLeft, entry.maxSpriteLeft),
    spriteTopRange: finiteRange(entry.minSpriteTop, entry.maxSpriteTop),
  }));
}

function compactSample(sample) {
  return {
    t: sample.t,
    player: sample.player,
    predictedPlayer: sample.predictedPlayer,
    movementPlan: sample.movementPlan,
    directionStepPending: sample.directionStepPending,
    sprite: sample.sprite,
    centerSprite: sample.centerSprite,
    centerSpriteKey: sample.centerSpriteKey,
    nameplate: sample.nameplate,
  };
}

function compactState(state) {
  return {
    player: state.player,
    predictedPlayer: state.predictedPlayer,
    movementPlan: state.movementPlan,
    directionStepPending: state.directionStepPending,
    selfEntity: state.selfEntity,
    sprite: state.sprite,
    centerSprite: state.centerSprite,
    centerSpriteKey: state.centerSpriteKey,
    nameplate: state.nameplate,
    floor: state.floor,
    floorKey: state.floorKey,
    commandTail: state.commandTail,
    gatewayMoveTail: state.gatewayMoveTail,
  };
}

function finiteRange(min, max) {
  return Number.isFinite(min) && Number.isFinite(max) ? Math.round((max - min) * 100) / 100 : null;
}

function delta(next, previous) {
  if (typeof next !== "number" || typeof previous !== "number") return 0;
  return Math.round((next - previous) * 100) / 100;
}

async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-movement-jitter-${process.pid}-${Date.now()}`);
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

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) return;
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve));
}

function findChromePath() {
  const candidates = [
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
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
  if (value === undefined || value === null || value === "") return fallback;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function defaultCharacterName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `MV${suffix}`.slice(0, 10).toUpperCase();
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
