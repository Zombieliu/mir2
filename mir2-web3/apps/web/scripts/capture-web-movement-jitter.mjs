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
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const sampleMs = numberArg(args.sampleMs, 50);
const interaction = args.interaction ?? "click";
const holdButton = args.button ?? args.holdButton ?? "right";
const holdMs = numberArg(args.holdMs, 2200);
const preHoldMs = numberArg(args.preHoldMs, 900);
const clickCount = numberArg(args.clickCount, 8);
const clickIntervalMs = numberArg(args.clickIntervalMs, 180);
const directionLagMs = numberArg(args.directionLagMs ?? process.env.MIR2_MOVEMENT_DIRECTION_LAG_MS, 260);
const stalePredictedMs = numberArg(args.stalePredictedMs ?? process.env.MIR2_MOVEMENT_STALE_PREDICTED_MS, 1200);
const slowCommandQueueMs = numberArg(
  args.slowCommandQueueMs ?? process.env.MIR2_MOVEMENT_SLOW_COMMAND_QUEUE_MS,
  1200,
);
const maxDirectionQueueLength = numberArg(
  args.maxDirectionQueueLength ?? process.env.MIR2_MOVEMENT_MAX_DIRECTION_QUEUE_LENGTH,
  1,
);
const strictMovementChecks = booleanArg(
  args.strictMovementChecks ?? process.env.MIR2_MOVEMENT_STRICT_CHECKS,
  true,
);
const allowBlockedResidual = booleanArg(
  args.allowBlockedResidual ?? process.env.MIR2_MOVEMENT_ALLOW_BLOCKED_RESIDUAL,
  false,
);
const settleMs = numberArg(
  args.settleMs ?? process.env.MIR2_MOVEMENT_SETTLE_MS,
  isStrictMovementInteraction(interaction) ? 5200 : 1200,
);
const startMap = args.map ?? "0";
const startX = numberArg(args.x, 330);
const startY = numberArg(args.y, 270);
const targetDx = numberArg(args.targetDx, 10);
const targetDy = numberArg(args.targetDy, 0);
const target2Dx = numberArg(args.target2Dx ?? args.secondTargetDx, targetDx);
const target2Dy = numberArg(args.target2Dy ?? args.secondTargetDy, targetDy - 4);
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
      const details = message.params?.exceptionDetails;
      this.consoleErrors.push({
        source: "exception",
        text: details?.exception?.description ?? details?.text ?? "runtime exception",
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
    } else if (interaction === "spamClickTarget") {
      const step = {
        label: `spam-click-target-${targetDx},${targetDy}`,
        mode: holdButton === "left" ? "walk" : "run",
        x: start.player.x + targetDx,
        y: start.player.y + targetDy,
        durationMs: Math.max(holdMs, clickCount * clickIntervalMs + 2400),
      };
      const before = await readMovementState(client);
      const samplePromise = sampleMovement(client, step.label, step.durationMs);
      const clickDispatches = [];
      for (let index = 0; index < clickCount; index += 1) {
        clickDispatches.push(await clickTile(client, step.x, step.y, step.mode === "run" ? "right" : "left"));
        await delay(clickIntervalMs);
      }
      await delay(Math.min(120, sampleMs));
      const afterDispatch = await readMovementState(client);
      actions.push({
        ...step,
        interaction,
        dispatch: { type: "spam-click-target", clickCount, clickIntervalMs, clicks: clickDispatches },
        before: compactState(before),
        afterDispatch: compactState(afterDispatch),
      });
      samples.push(...(await samplePromise));
    } else if (interaction === "holdThenSpamClickTarget") {
      const step = {
        label: `hold-then-spam-click-target-${targetDx},${targetDy}`,
        mode: holdButton === "left" ? "walk" : "run",
        x: start.player.x + targetDx,
        y: start.player.y + targetDy,
        durationMs: Math.max(holdMs, preHoldMs + clickCount * clickIntervalMs + 2800),
      };
      const before = await readMovementState(client);
      const samplePromise = sampleMovement(client, step.label, step.durationMs);
      const hold = await beginHoldTile(client, step.x, step.y, holdButton);
      await delay(preHoldMs);
      await hold.release();
      const clickDispatches = [];
      for (let index = 0; index < clickCount; index += 1) {
        clickDispatches.push(await clickTile(client, step.x, step.y, step.mode === "run" ? "right" : "left"));
        await delay(clickIntervalMs);
      }
      await delay(Math.min(120, sampleMs));
      const afterDispatch = await readMovementState(client);
      actions.push({
        ...step,
        interaction,
        dispatch: {
          type: "hold-then-spam-click-target",
          hold: hold.dispatch,
          preHoldMs,
          clickCount,
          clickIntervalMs,
          clicks: clickDispatches,
        },
        before: compactState(before),
        afterDispatch: compactState(afterDispatch),
      });
      samples.push(...(await samplePromise));
    } else if (interaction === "routeSpamObstacle") {
      const step = {
        label: `route-spam-obstacle-${targetDx},${targetDy}-then-${target2Dx},${target2Dy}`,
        mode: holdButton === "left" ? "walk" : "run",
        x: start.player.x + targetDx,
        y: start.player.y + targetDy,
        x2: start.player.x + target2Dx,
        y2: start.player.y + target2Dy,
        durationMs: Math.max(holdMs, clickCount * clickIntervalMs + 3600),
      };
      const before = await readMovementState(client);
      const samplePromise = sampleMovement(client, step.label, step.durationMs);
      const clickDispatches = [];
      for (let index = 0; index < clickCount; index += 1) {
        const useSecondTarget = index >= Math.ceil(clickCount / 2);
        const x = useSecondTarget ? step.x2 : step.x;
        const y = useSecondTarget ? step.y2 : step.y;
        clickDispatches.push({
          target: useSecondTarget ? "reroute" : "primary",
          ...(await clickTile(client, x, y, step.mode === "run" ? "right" : "left")),
        });
        await delay(clickIntervalMs);
      }
      await delay(Math.min(160, sampleMs * 2));
      const afterDispatch = await readMovementState(client);
      actions.push({
        ...step,
        interaction,
        dispatch: {
          type: "route-spam-obstacle",
          clickCount,
          clickIntervalMs,
          primary: { x: step.x, y: step.y },
          reroute: { x: step.x2, y: step.y2 },
          clicks: clickDispatches,
        },
        before: compactState(before),
        afterDispatch: compactState(afterDispatch),
      });
      samples.push(...(await samplePromise));
    } else if (interaction === "blockedTarget") {
      const step = {
        label: `blocked-target-${targetDx},${targetDy}`,
        mode: holdButton === "left" ? "walk" : "run",
        x: start.player.x + targetDx,
        y: start.player.y + targetDy,
        durationMs: Math.max(holdMs, clickCount * clickIntervalMs + 4200),
      };
      const before = await readMovementState(client);
      const samplePromise = sampleMovement(client, step.label, step.durationMs);
      const clickDispatches = [];
      for (let index = 0; index < clickCount; index += 1) {
        clickDispatches.push(await clickTile(client, step.x, step.y, step.mode === "run" ? "right" : "left"));
        await delay(clickIntervalMs);
      }
      await delay(Math.min(160, sampleMs * 2));
      const afterDispatch = await readMovementState(client);
      actions.push({
        ...step,
        interaction,
        blockedTarget: true,
        dispatch: {
          type: "blocked-target",
          clickCount,
          clickIntervalMs,
          target: { x: step.x, y: step.y },
          clicks: clickDispatches,
        },
        before: compactState(before),
        afterDispatch: compactState(afterDispatch),
      });
      samples.push(...(await samplePromise));
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

    const settle = await waitForMovementSettle(client, settleMs, { allowBlockedResidual });
    const finalState = settle.finalState;
    const screenshotPath = path.join(outputDir, `${prefix}.png`);
    const statePath = path.join(outputDir, `${prefix}.json`);
    const screenshot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    await fs.writeFile(screenshotPath, Buffer.from(screenshot.data, "base64"));
    const jumps = detectJumps(samples);
    const routeSpamWarnings = detectRouteSpam(samples);
    const logicalRollbackWarnings = detectLogicalRollbacks(samples);
    const directionLagWarnings = detectDirectionAnimationLag(samples, directionLagMs);
    const stalePredictionWarnings = detectStalePredictedPlayer(samples, stalePredictedMs);
    const commandQueueWarnings = detectSlowCommandQueue(samples, {
      maxPendingMs: slowCommandQueueMs,
      maxDirectionQueueLength,
    });
    const pendingPlanAtEnd = analyzePendingPlanAtEnd(finalState, settle.capturedAt);
    const assertions = buildAssertions({
      interaction,
      strictMovementChecks,
      allowBlockedResidual,
      jumps,
      routeSpamWarnings,
      logicalRollbackWarnings,
      directionLagWarnings,
      stalePredictionWarnings,
      commandQueueWarnings,
      pendingPlanAtEnd,
      consoleErrors: client.consoleErrors,
      network404s: client.network404s,
    });

    const report = {
      ok: assertions.every((assertion) => assertion.pass),
      baseUrl,
      account,
      createAccount,
      characterName: createAccount ? characterName : undefined,
      interaction,
      blockedTarget: interaction === "blockedTarget" ? true : undefined,
      startTarget: { map: startMap, x: startX, y: startY },
      startedAt: new Date().toISOString(),
      start,
      finalState,
      sampleMs,
      directionLagMs,
      stalePredictedMs,
      slowCommandQueueMs,
      maxDirectionQueueLength,
      strictMovementChecks,
      allowBlockedResidual,
      holdMs: interaction === "hold" ? holdMs : undefined,
      preHoldMs: interaction === "holdThenSpamClickTarget" ? preHoldMs : undefined,
      settleMs,
      settle,
      pendingPlanAtEnd,
      assertions,
      sampleCount: samples.length,
      actions,
      jumps,
      routeSpamWarnings,
      logicalRollbackWarnings,
      directionLagWarnings,
      stalePredictionWarnings,
      commandQueueWarnings,
      feelMetrics: {
        logicalRollbackCount: logicalRollbackWarnings.length,
        directionLagWindowMs: directionLagMs,
        directionLagCheckCount: movementDirectionCommandsFromSamples(samples).length,
        directionLagWarningCount: directionLagWarnings.length,
        stalePredictedWindowMs: stalePredictedMs,
        stalePredictionWarningCount: stalePredictionWarnings.length,
        slowCommandQueueWindowMs: slowCommandQueueMs,
        slowCommandQueueWarningCount: commandQueueWarnings.length,
      },
      summary: summarizeSamples(samples),
      samples,
      nonFaviconNetwork404s: [...new Set(client.network404s)],
      consoleErrors: client.consoleErrors,
      screenshotPath,
    };
    await fs.writeFile(statePath, `${JSON.stringify(report, null, 2)}\n`);
    console.log(
      JSON.stringify(
        {
          ok: report.ok,
          statePath,
          screenshotPath,
          settle: report.settle,
          pendingPlanAtEnd: report.pendingPlanAtEnd,
          assertions: report.assertions,
          summary: report.summary,
          jumps: report.jumps,
          routeSpamWarnings: report.routeSpamWarnings,
          logicalRollbackWarnings: report.logicalRollbackWarnings,
          directionLagWarnings: report.directionLagWarnings,
          stalePredictionWarnings: report.stalePredictionWarnings,
          commandQueueWarnings: report.commandQueueWarnings,
          feelMetrics: report.feelMetrics,
          nonFaviconNetwork404s: report.nonFaviconNetwork404s,
          consoleErrors: report.consoleErrors,
        },
        null,
        2,
      ),
    );
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
      window.__mir2MovementSentCommands = [];
      window.__mir2MovementReceivedPackets = [];
      return true;
    })()
  `);
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
  } else if (screen === "select") {
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

async function waitForMovementSettle(client, timeoutMs, options = {}) {
  const allowBlockedAsSettled = options.allowBlockedResidual === true;
  const startedAt = Date.now();
  let finalState = await readMovementState(client);
  let pendingPlanAtEnd = analyzePendingPlanAtEnd(finalState, Date.now());
  let observedBlockedTarget = pendingPlanAtEnd?.targetBlocked ? pendingPlanAtEnd : null;
  while (
    pendingPlanAtEnd &&
    !(allowBlockedAsSettled && pendingPlanAtEnd.nonFailure) &&
    Date.now() - startedAt < timeoutMs
  ) {
    await delay(Math.min(Math.max(sampleMs, 50), 160));
    finalState = await readMovementState(client);
    pendingPlanAtEnd = analyzePendingPlanAtEnd(finalState, Date.now());
    if (pendingPlanAtEnd?.targetBlocked) {
      observedBlockedTarget = pendingPlanAtEnd;
    }
  }
  const capturedAt = Date.now();
  const blockedResidual = pendingPlanAtEnd?.targetBlocked ? true : undefined;
  return {
    status: pendingPlanAtEnd?.targetBlocked ? "blocked" : pendingPlanAtEnd ? "pending" : "settled",
    strictStatus: pendingPlanAtEnd
      ? pendingPlanAtEnd.targetBlocked
        ? "blockedResidual"
        : "pendingResidual"
      : "settled",
    clean: !pendingPlanAtEnd,
    blockedTarget: pendingPlanAtEnd?.targetBlocked || observedBlockedTarget ? true : undefined,
    blockedResidual,
    pendingPlanAtEnd,
    observedBlockedTarget,
    waitedMs: capturedAt - startedAt,
    capturedAt,
    finalState,
  };
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
        capturedAt: Date.now(),
        screen: state.screen ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        player,
        predictedPlayer: state.predictedPlayer ?? null,
        movementPlan: state.movementPlan ?? null,
        directionStepPending: state.directionStepPending ?? null,
        directionStepPendingQueue: state.directionStepPendingQueue ?? [],
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
          .filter((entry) => [
            "UserLocation",
            "Pushed",
            "UserDash",
            "UserDashFail",
            "UserDashAttack",
            "UserAttackMove",
            "ObjectWalk",
            "ObjectRun",
            "ObjectPushed",
            "ObjectDash",
            "ObjectDashFail",
            "ObjectDashAttack",
            "ObjectBackStep",
            "ObjectSitDown",
          ].includes(entry?.packet))
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
      const centerBacktrack = isUnexpectedCenterSpriteDelta(centerDx, centerDy, sample, previous);
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

function detectRouteSpam(samples) {
  const commandsByKey = new Map();
  for (const sample of samples) {
    for (const command of sample.commandTail ?? []) {
      if (typeof command?.at !== "number") continue;
      const key = `${command.at}:${JSON.stringify(command)}`;
      const distance =
        typeof sample.capturedAt === "number" ? Math.abs(sample.capturedAt - command.at) : Number.POSITIVE_INFINITY;
      const existing = commandsByKey.get(key);
      if (!existing || distance < existing.distance) {
        commandsByKey.set(key, {
          command,
          player: routeSpamCommandSource(sample, command),
          distance,
        });
      }
    }
  }

  const buckets = new Map();
  for (const entry of commandsByKey.values()) {
    const command = entry.command;
    const player = entry.player;
    if (!player) continue;
    const commandKey =
      command.type === "moveTo"
        ? `moveTo:${command.x},${command.y}:${command.mode ?? ""}`
        : `${command.type}:${command.direction ?? ""}`;
    const key = `${player.x},${player.y}:${commandKey}`;
    const bucket = buckets.get(key) ?? {
      player,
      commandKey,
      count: 0,
      firstAt: command.at,
      lastAt: command.at,
    };
    bucket.count += 1;
    bucket.firstAt = Math.min(bucket.firstAt, command.at);
    bucket.lastAt = Math.max(bucket.lastAt, command.at);
    buckets.set(key, bucket);
  }

  return Array.from(buckets.values())
    .filter((bucket) => bucket.count >= 4)
    .map((bucket) => ({
      player: bucket.player,
      commandKey: bucket.commandKey,
      count: bucket.count,
      durationMs: bucket.lastAt - bucket.firstAt,
    }));
}

function routeSpamCommandSource(sample, command) {
  const plan = sample?.movementPlan;
  if (
    plan &&
    typeof plan.pendingSentAt === "number" &&
    Math.abs(plan.pendingSentAt - command.at) <= 120 &&
    Number.isFinite(plan.sentFromX) &&
    Number.isFinite(plan.sentFromY)
  ) {
    return { x: plan.sentFromX, y: plan.sentFromY };
  }

  return sample?.player ?? null;
}

function buildAssertions({
  interaction,
  strictMovementChecks,
  allowBlockedResidual,
  jumps,
  routeSpamWarnings,
  logicalRollbackWarnings,
  directionLagWarnings,
  stalePredictionWarnings,
  commandQueueWarnings,
  pendingPlanAtEnd,
  consoleErrors,
  network404s,
}) {
  const assertions = [
    {
      name: "noVisualJumps",
      pass: jumps.length === 0,
      count: jumps.length,
    },
    {
      name: "noRouteSpamWarnings",
      pass: routeSpamWarnings.length === 0,
      count: routeSpamWarnings.length,
    },
    {
      name: "noLogicalTileRollback",
      pass: logicalRollbackWarnings.length === 0,
      count: logicalRollbackWarnings.length,
    },
    {
      name: "directionAnimationWithinCrystalWindow",
      pass: directionLagWarnings.length === 0,
      count: directionLagWarnings.length,
    },
    {
      name: "stalePredictedPlayerCleared",
      pass: !strictMovementChecks || stalePredictionWarnings.length === 0,
      count: stalePredictionWarnings.length,
      maxAgeMs: stalePredictedMs,
      strict: strictMovementChecks,
      warnings: stalePredictionWarnings,
    },
    {
      name: "movementCommandQueueResponsive",
      pass: !strictMovementChecks || commandQueueWarnings.length === 0,
      count: commandQueueWarnings.length,
      maxPendingMs: slowCommandQueueMs,
      maxDirectionQueueLength,
      strict: strictMovementChecks,
      warnings: commandQueueWarnings,
    },
    {
      name: "movementSettledWithoutResidualPlan",
      pass: !strictMovementChecks || !pendingPlanAtEnd || (allowBlockedResidual && pendingPlanAtEnd.nonFailure === true),
      status: pendingPlanAtEnd?.status ?? "settled",
      strictStatus: pendingPlanAtEnd
        ? pendingPlanAtEnd.targetBlocked
          ? "blockedResidual"
          : "pendingResidual"
        : "settled",
      strict: strictMovementChecks,
      allowBlockedResidual,
      pendingPlanAtEnd,
    },
    {
      name: "noConsoleErrors",
      pass: consoleErrors.length === 0,
      count: consoleErrors.length,
    },
    {
      name: "noNonFaviconNetwork404s",
      pass: network404s.length === 0,
      count: network404s.length,
    },
  ];

  if (isBlockedTargetInteraction(interaction)) {
    assertions.push({
      name: `${interaction}BlockedResidualCleared`,
      pass:
        !strictMovementChecks ||
        !pendingPlanAtEnd ||
        (allowBlockedResidual && pendingPlanAtEnd.nonFailure === true),
      status: pendingPlanAtEnd?.status ?? "settled",
      strictStatus: pendingPlanAtEnd
        ? pendingPlanAtEnd.targetBlocked
          ? "blockedResidual"
          : "pendingResidual"
        : "settled",
      targetBlocked: pendingPlanAtEnd?.targetBlocked ?? false,
      hasMovementPlan: pendingPlanAtEnd?.hasMovementPlan ?? false,
      hasPredictedPlayer: pendingPlanAtEnd?.hasPredictedPlayer ?? false,
      hasDirectionStepPending: pendingPlanAtEnd?.hasDirectionStepPending ?? false,
      directionQueueLength: pendingPlanAtEnd?.directionQueueLength ?? 0,
      strict: strictMovementChecks,
      allowBlockedResidual,
      pendingPlanAtEnd,
    });
  }

  if (interaction === "holdThenSpamClickTarget") {
    assertions.push({
      name: "holdThenSpamClickTargetQueueStrict",
      pass:
        !strictMovementChecks ||
        (logicalRollbackWarnings.length === 0 &&
          commandQueueWarnings.length === 0 &&
          !pendingPlanAtEnd &&
          stalePredictionWarnings.length === 0),
      logicalRollbackCount: logicalRollbackWarnings.length,
      commandQueueWarningCount: commandQueueWarnings.length,
      stalePredictionWarningCount: stalePredictionWarnings.length,
      status: pendingPlanAtEnd?.status ?? "settled",
      strict: strictMovementChecks,
    });
  }

  return assertions;
}

function isBlockedTargetInteraction(value) {
  return value === "routeSpamObstacle" || value === "blockedTarget";
}

function isStrictMovementInteraction(value) {
  return isBlockedTargetInteraction(value) || value === "holdThenSpamClickTarget";
}

function detectStalePredictedPlayer(samples, maxAgeMs) {
  const warnings = [];
  let active = null;

  const closeActive = () => {
    if (!active) return;
    const durationMs = active.lastAt - active.firstAt;
    if (durationMs >= maxAgeMs) {
      warnings.push({
        label: active.label,
        durationMs,
        maxAgeMs,
        player: active.player,
        predicted: active.predicted,
        movementPlan: active.lastSample?.movementPlan ?? null,
        first: compactSample(active.firstSample),
        last: compactSample(active.lastSample),
      });
    }
    active = null;
  };

  for (const sample of samples) {
    const player = normalizedPoint(sample?.player);
    const predicted = normalizedPoint(sample?.predictedPlayer);
    const stale = Boolean(player && predicted && !samePoint(player, predicted));
    const key = stale ? `${sample.label}:${tileKey(player)}:${tileKey(predicted)}` : null;
    const capturedAt = numericTimestamp(sample?.capturedAt, sample?.t);

    if (!stale || !Number.isFinite(capturedAt)) {
      closeActive();
      continue;
    }

    if (!active || active.key !== key) {
      closeActive();
      active = {
        key,
        label: sample.label,
        player,
        predicted: {
          ...predicted,
          direction: sample.predictedPlayer?.direction ?? null,
        },
        firstAt: capturedAt,
        lastAt: capturedAt,
        firstSample: sample,
        lastSample: sample,
      };
      continue;
    }

    active.lastAt = capturedAt;
    active.lastSample = sample;
    active.predicted = {
      ...predicted,
      direction: sample.predictedPlayer?.direction ?? active.predicted.direction ?? null,
    };
  }

  closeActive();
  return warnings;
}

function detectSlowCommandQueue(samples, { maxPendingMs, maxDirectionQueueLength }) {
  const warnings = [];
  const pendingSpans = new Map();
  const queueSpans = new Map();

  for (const sample of samples) {
    const capturedAt = numericTimestamp(sample?.capturedAt, sample?.t);
    if (!Number.isFinite(capturedAt)) continue;

    const plan = sample?.movementPlan ?? null;
    if (plan && typeof plan.pendingSentAt === "number") {
      const pending = normalizedPoint({ x: plan.pendingX, y: plan.pendingY });
      const player = normalizedPoint(sample?.player);
      const pendingAgeMs = capturedAt - plan.pendingSentAt;
      if (pending && player && !samePoint(player, pending) && pendingAgeMs > maxPendingMs) {
        const key = `${sample.label}:${plan.pendingSentAt}:${tileKey(player)}:${tileKey(pending)}:${plan.sentDirection ?? ""}`;
        updateSpan(pendingSpans, key, {
          label: sample.label,
          type: "pendingMovementPlan",
          maxPendingMs,
          player,
          pending,
          target: normalizedPoint({ x: plan.targetX, y: plan.targetY }),
          sentDirection: plan.sentDirection ?? null,
          sentMode: plan.sentMode ?? plan.mode ?? null,
          pendingSentAt: plan.pendingSentAt,
          ageMs: pendingAgeMs,
          sample,
        });
      }
    }

    const queue = Array.isArray(sample?.directionStepPendingQueue) ? sample.directionStepPendingQueue : [];
    if (queue.length > maxDirectionQueueLength) {
      const key = `${sample.label}:direction-queue`;
      updateSpan(queueSpans, key, {
        label: sample.label,
        type: "directionStepPendingQueue",
        maxDirectionQueueLength,
        queueLength: queue.length,
        sample,
      });
    }

    const pendingStep = sample?.directionStepPending ?? null;
    const pendingStepAt = directionStepTimestamp(pendingStep);
    if (pendingStep && Number.isFinite(pendingStepAt) && capturedAt - pendingStepAt > maxPendingMs) {
      const key = `${sample.label}:direction-step:${pendingStepAt}`;
      updateSpan(queueSpans, key, {
        label: sample.label,
        type: "directionStepPending",
        maxPendingMs,
        ageMs: capturedAt - pendingStepAt,
        pendingStep,
        sample,
      });
    }
  }

  for (const span of [...pendingSpans.values(), ...queueSpans.values()]) {
    warnings.push({
      label: span.label,
      type: span.type,
      durationMs: span.lastAt - span.firstAt,
      maxAgeMs: span.maxPendingMs,
      maxPendingMs: span.maxPendingMs,
      maxDirectionQueueLength: span.maxDirectionQueueLength,
      maxObservedAgeMs: span.maxObservedAgeMs ?? null,
      maxObservedQueueLength: span.maxObservedQueueLength ?? null,
      player: span.player,
      pending: span.pending,
      target: span.target,
      sentDirection: span.sentDirection,
      sentMode: span.sentMode,
      pendingSentAt: span.pendingSentAt,
      first: compactSample(span.firstSample),
      last: compactSample(span.lastSample),
    });
  }

  return warnings;
}

function updateSpan(spans, key, detail) {
  const capturedAt = numericTimestamp(detail.sample?.capturedAt, detail.sample?.t);
  const existing = spans.get(key);
  if (!existing) {
    spans.set(key, {
      ...detail,
      firstAt: capturedAt,
      lastAt: capturedAt,
      firstSample: detail.sample,
      lastSample: detail.sample,
      maxObservedAgeMs: Number.isFinite(detail.ageMs) ? detail.ageMs : null,
      maxObservedQueueLength: Number.isFinite(detail.queueLength) ? detail.queueLength : null,
    });
    return;
  }

  existing.lastAt = capturedAt;
  existing.lastSample = detail.sample;
  if (Number.isFinite(detail.ageMs)) {
    existing.maxObservedAgeMs = Math.max(existing.maxObservedAgeMs ?? 0, detail.ageMs);
  }
  if (Number.isFinite(detail.queueLength)) {
    existing.maxObservedQueueLength = Math.max(existing.maxObservedQueueLength ?? 0, detail.queueLength);
  }
}

function detectLogicalRollbacks(samples) {
  const warnings = [];
  let previous = null;
  for (const sample of samples) {
    if (previous && sample.label === previous.label) {
      const direction = activeMovementDirection(sample) ?? activeMovementDirection(previous);
      const previousDirection = activeMovementDirection(previous);
      const vector = direction && direction === previousDirection ? directionVector(direction) : null;
      const from = visualPlayerPoint(previous);
      const to = visualPlayerPoint(sample);
      if (vector && from && to) {
        const dx = to.x - from.x;
        const dy = to.y - from.y;
        if (logicalAxisBacktracked(dx, vector.x) || logicalAxisBacktracked(dy, vector.y)) {
          warnings.push({
            label: sample.label,
            t: sample.t,
            direction,
            dx,
            dy,
            from: compactSample(previous),
            to: compactSample(sample),
          });
        }
      }
    }
    previous = sample;
  }
  return warnings;
}

function detectDirectionAnimationLag(samples, maxLagMs) {
  const warnings = [];
  for (const command of movementDirectionCommandsFromSamples(samples)) {
    const observed = samples.find(
      (sample) =>
        typeof sample.capturedAt === "number" &&
        sample.capturedAt >= command.at &&
        sample.capturedAt - command.at <= maxLagMs &&
        activeMovementDirections(sample).includes(command.direction),
    );
    if (observed) {
      continue;
    }

    const nearest = samples.find(
      (sample) =>
        typeof sample.capturedAt === "number" &&
        sample.capturedAt >= command.at &&
        sample.capturedAt - command.at <= maxLagMs,
    );
    warnings.push({
      command,
      maxLagMs,
      observedDirections: nearest ? activeMovementDirections(nearest) : [],
      observedAtDeltaMs: nearest ? nearest.capturedAt - command.at : null,
      sample: nearest ? compactSample(nearest) : null,
    });
  }
  return warnings;
}

function movementDirectionCommandsFromSamples(samples) {
  return movementCommandsFromSamples(samples).filter(
    (command) =>
      (command.type === "walk" || command.type === "run" || command.type === "turn") &&
      typeof command.direction === "string",
  );
}

function movementCommandsFromSamples(samples) {
  const commands = new Map();
  for (const sample of samples) {
    for (const command of sample.commandTail ?? []) {
      if (!command || typeof command.at !== "number" || typeof command.type !== "string") {
        continue;
      }
      if (!["moveTo", "walk", "run", "turn"].includes(command.type)) {
        continue;
      }
      const key = `${command.at}:${command.type}:${command.direction ?? ""}:${command.x ?? ""}:${command.y ?? ""}`;
      commands.set(key, command);
    }
  }
  return Array.from(commands.values()).sort((left, right) => left.at - right.at);
}

function analyzePendingPlanAtEnd(state, capturedAt = Date.now()) {
  const plan = state?.movementPlan ?? null;
  const predicted = state?.predictedPlayer ?? null;
  const directionStepPending = state?.directionStepPending ?? null;
  const directionStepPendingQueue = Array.isArray(state?.directionStepPendingQueue)
    ? state.directionStepPendingQueue
    : [];
  if (!plan && !predicted && !directionStepPending && directionStepPendingQueue.length === 0) {
    return null;
  }

  const player = state?.player ?? null;
  const target =
    plan && Number.isFinite(plan.targetX) && Number.isFinite(plan.targetY)
      ? { x: plan.targetX, y: plan.targetY }
      : null;
  const pending =
    plan && Number.isFinite(plan.pendingX) && Number.isFinite(plan.pendingY)
      ? { x: plan.pendingX, y: plan.pendingY }
      : null;
  const blockedSteps = Array.isArray(plan?.blockedSteps) ? plan.blockedSteps : [];
  const lastMovementCommand = lastTimedEntry(state?.commandTail ?? []);
  const lastSelfPacket = lastTimedEntry(
    (state?.gatewayMoveTail ?? []).filter((entry) => isSelfMovementPacket(entry?.packet)),
  );
  const targetReached = Boolean(player && target && player.x === target.x && player.y === target.y);
  const pendingReached = Boolean(player && pending && player.x === pending.x && player.y === pending.y);
  const predictedReached = Boolean(player && predicted && player.x === predicted.x && player.y === predicted.y);
  const serverCorrectedPending = Boolean(
    pending &&
      lastSelfPacket &&
      typeof plan?.pendingSentAt === "number" &&
      lastSelfPacket.at >= plan.pendingSentAt &&
      (packetX(lastSelfPacket) !== pending.x || packetY(lastSelfPacket) !== pending.y),
  );
  const recentBlockedStep = blockedSteps
    .filter((step) => typeof step?.at === "number")
    .sort((left, right) => right.at - left.at)[0];
  const blockedAtCurrentTile = Boolean(
    player && blockedSteps.some((step) => step?.fromX === player.x && step?.fromY === player.y),
  );
  const hasRecentBlockedCorrection = Boolean(recentBlockedStep && capturedAt - recentBlockedStep.at <= 5000);
  const targetBlocked = !targetReached && (serverCorrectedPending || blockedAtCurrentTile || hasRecentBlockedCorrection);
  const inFlight = Boolean(
    pending &&
      typeof plan?.pendingSentAt === "number" &&
      capturedAt - plan.pendingSentAt < 900 &&
      !serverCorrectedPending,
  );
  const status = targetReached
    ? "staleAfterTargetReached"
    : targetBlocked
      ? "targetBlocked"
      : inFlight
        ? "pendingInFlight"
        : "pendingUnresolved";

  return {
    status,
    nonFailure: status === "targetBlocked",
    hasMovementPlan: Boolean(plan),
    hasPredictedPlayer: Boolean(predicted),
    hasDirectionStepPending: Boolean(directionStepPending),
    directionQueueLength: directionStepPendingQueue.length,
    movementPlan: compactMovementPlan(plan),
    targetBlocked,
    targetReached,
    pendingReached,
    predictedReached,
    player,
    target,
    pending,
    predicted,
    directionStepPending,
    directionStepPendingQueue,
    blockedSteps,
    lastMovementCommand,
    lastSelfMovementPacket: lastSelfPacket,
    capturedAt,
    pendingAgeMs: typeof plan?.pendingSentAt === "number" ? capturedAt - plan.pendingSentAt : null,
  };
}

function compactMovementPlan(plan) {
  if (!plan) return null;
  return {
    targetX: plan.targetX,
    targetY: plan.targetY,
    mode: plan.mode,
    packetMode: plan.packetMode,
    actionX: plan.actionX,
    actionY: plan.actionY,
    pendingX: plan.pendingX,
    pendingY: plan.pendingY,
    pendingSentAt: plan.pendingSentAt,
    visualUntil: plan.visualUntil,
    sentFromX: plan.sentFromX,
    sentFromY: plan.sentFromY,
    sentDirection: plan.sentDirection,
    sentMode: plan.sentMode,
    blockedSteps: Array.isArray(plan.blockedSteps) ? plan.blockedSteps : [],
  };
}

function isSelfMovementPacket(packet) {
  return [
    "UserLocation",
    "Pushed",
    "UserDash",
    "UserDashFail",
    "UserDashAttack",
    "UserAttackMove",
  ].includes(packet);
}

function lastTimedEntry(entries) {
  let latest = null;
  for (const entry of entries) {
    if (typeof entry?.at !== "number") continue;
    if (!latest || entry.at > latest.at) {
      latest = entry;
    }
  }
  return latest;
}

function packetX(entry) {
  const payload = entry?.payload ?? {};
  return payload.x ?? payload.location?.x ?? null;
}

function packetY(entry) {
  const payload = entry?.payload ?? {};
  return payload.y ?? payload.location?.y ?? null;
}

function normalizedPoint(value) {
  if (!value || !Number.isFinite(value.x) || !Number.isFinite(value.y)) return null;
  return { x: value.x, y: value.y };
}

function samePoint(left, right) {
  return Boolean(left && right && left.x === right.x && left.y === right.y);
}

function tileKey(point) {
  return point ? `${point.x},${point.y}` : "null";
}

function numericTimestamp(...values) {
  for (const value of values) {
    if (Number.isFinite(value)) return value;
  }
  return null;
}

function directionStepTimestamp(step) {
  if (!step || typeof step !== "object") return null;
  return numericTimestamp(step.at, step.sentAt, step.pendingSentAt, step.createdAt, step.startedAt);
}

function isUnexpectedCenterSpriteDelta(centerDx, centerDy, sample, previous) {
  const direction = activeMovementDirection(sample) ?? activeMovementDirection(previous);
  const vector = directionVector(direction);
  if (!vector) {
    return centerDx > 12 || Math.abs(centerDy) > 12;
  }

  const expectedMapX = -vector.x;
  const expectedMapY = -vector.y;
  return (
    axisBacktracked(centerDx, expectedMapX) ||
    axisBacktracked(centerDy, expectedMapY) ||
    axisMovedOffDirection(centerDx, expectedMapX) ||
    axisMovedOffDirection(centerDy, expectedMapY)
  );
}

function axisBacktracked(deltaValue, expectedSign) {
  return expectedSign !== 0 && Math.sign(deltaValue) === -expectedSign && Math.abs(deltaValue) > 12;
}

function axisMovedOffDirection(deltaValue, expectedSign) {
  return expectedSign === 0 && Math.abs(deltaValue) > 12;
}

function activeMovementDirection(sample) {
  if (!sample) return null;
  return activeMovementDirections(sample)[0] ?? null;
}

function activeMovementDirections(sample) {
  if (!sample) return [];
  return uniqueCompact([
    sample.predictedPlayer?.direction,
    lastQueuedDirection(sample.directionStepPendingQueue),
    sample.directionStepPending?.direction,
    sample.movementPlan?.sentDirection,
    lastMovementCommandDirection(sample.commandTail),
    sample.selfEntity?.direction,
  ]);
}

function lastQueuedDirection(queue) {
  if (!Array.isArray(queue) || queue.length === 0) return null;
  return queue[queue.length - 1]?.direction ?? null;
}

function lastMovementCommandDirection(commands) {
  if (!Array.isArray(commands) || commands.length === 0) return null;
  return commands.find((command) => ["walk", "run", "turn"].includes(command?.type))?.direction ?? null;
}

function visualPlayerPoint(sample) {
  if (!sample) return null;
  if (sample.predictedPlayer && Number.isFinite(sample.predictedPlayer.x) && Number.isFinite(sample.predictedPlayer.y)) {
    return { x: sample.predictedPlayer.x, y: sample.predictedPlayer.y };
  }
  if (sample.player && Number.isFinite(sample.player.x) && Number.isFinite(sample.player.y)) {
    return { x: sample.player.x, y: sample.player.y };
  }
  return null;
}

function logicalAxisBacktracked(deltaValue, expectedSign) {
  return expectedSign !== 0 && Math.sign(deltaValue) === -expectedSign && Math.abs(deltaValue) >= 1;
}

function uniqueCompact(values) {
  return [...new Set(values.filter((value) => typeof value === "string" && value.length > 0))];
}

function directionVector(direction) {
  switch (direction) {
    case "Up":
      return { x: 0, y: -1 };
    case "UpRight":
      return { x: 1, y: -1 };
    case "Right":
      return { x: 1, y: 0 };
    case "DownRight":
      return { x: 1, y: 1 };
    case "Down":
      return { x: 0, y: 1 };
    case "DownLeft":
      return { x: -1, y: 1 };
    case "Left":
      return { x: -1, y: 0 };
    case "UpLeft":
      return { x: -1, y: -1 };
    default:
      return null;
  }
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
    directionStepPendingQueue: sample.directionStepPendingQueue,
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
    directionStepPendingQueue: state.directionStepPendingQueue,
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
  const debug = await client
    .evaluate(`
      (() => ({
        url: location.href,
        readyState: document.readyState,
        title: document.title,
        stageScreen: window.__mir2Stage5?.state?.screen ?? null,
        stageKeys: window.__mir2Stage5?.state ? Object.keys(window.__mir2Stage5.state).slice(0, 20) : [],
        bodyText: document.body?.innerText?.slice(0, 500) ?? "",
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
