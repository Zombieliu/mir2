import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_BASE_URL = "http://127.0.0.1:13010";
const DEFAULT_OUTPUT_DIR = path.resolve(REPO_ROOT, "docs", "generated", "player-qa", "mir2-probe");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };
const MOVEMENT_SUITE_DIRECTIONS = ["Right", "Left", "Down", "Up"];
// Run-left first so the later run-right returns toward spawn instead of entering the starter east gate.
const MOVEMENT_SUITE_RUN_DIRECTIONS = ["Left", "Right", "Down", "Up"];
const MOVEMENT_SUITE_DEFAULT_HOLD_MS = 3_000;
const MOVEMENT_SUITE_DEFAULT_AFTER_MS = 2_000;
const MOVEMENT_ACCEPTANCE = {
  expectedCadenceMs: 600,
  maxStartupGapMs: 1_000,
  maxSteadyGapMs: 900,
  maxSteadyGapP95Ms: 780,
  maxAckLatencyMs: 500,
  maxAckLatencyP95Ms: 350,
  maxSceneAssetTotal: 8,
  maxLongtasksInLastSecond: 0,
  maxRafDeltaMaxMs: 120,
  maxTimerDriftMaxMs: 120,
  maxFinalGatewayTickAvgMs: 2,
  maxFinalGatewayTickMaxMs: 75,
};

const args = parseArgs(process.argv.slice(2));
const baseUrl = args.url ?? args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL;
const movementSuiteEnabled =
  booleanArg(args.movementSuite ?? args.movement_suite ?? process.env.MIR2_PROBE_MOVEMENT_SUITE, false) ||
  args.suite === "movement" ||
  args.suite === "movement-suite";
const label = sanitizeLabel(
  args.label ?? args.probe ?? process.env.MIR2_PROBE_LABEL ?? (movementSuiteEnabled ? "movement-suite" : "headless"),
);
const durationMs = numberArg(args.durationMs ?? args.ms ?? process.env.MIR2_PROBE_DURATION_MS, 10_000);
const waitTimeoutMs = numberArg(args.waitTimeoutMs ?? process.env.MIR2_PROBE_WAIT_MS, 30_000);
const outputDir = path.resolve(args.output ?? process.env.MIR2_PROBE_OUTPUT ?? DEFAULT_OUTPUT_DIR);
const runId = args.runId ?? new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
const prefix = args.prefix ?? `mir2-probe-${label}-${runId}`;
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 500));
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const autoLogin = booleanArg(args.login ?? args.autoLogin ?? process.env.MIR2_PROBE_LOGIN, false);
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_PROBE_CREATE_ACCOUNT, false);
const createCharacterArg = args.createCharacter ?? process.env.MIR2_PROBE_CREATE_CHARACTER;
const createCharacter = createCharacterArg === undefined ? createAccount : booleanArg(createCharacterArg, false);
const generatedAccount = `probe${Date.now().toString().slice(-8)}`;
const account = args.account ?? process.env.MIR2_PROBE_ACCOUNT ?? (createAccount ? generatedAccount : "demo");
const password = args.password ?? process.env.MIR2_PROBE_PASSWORD ?? "demo";
const characterName = args.characterName ?? args.character ?? process.env.MIR2_PROBE_CHARACTER ?? `P${Date.now().toString().slice(-8)}`;
const movementSequenceArg = args.movement ?? args.movementSequence ?? process.env.MIR2_PROBE_MOVEMENT ?? "";
const movementSuiteHoldMs = numberArg(args.movementSuiteHoldMs ?? args.holdMs, MOVEMENT_SUITE_DEFAULT_HOLD_MS);
const movementSuiteAfterMs = numberArg(args.movementSuiteAfterMs ?? args.afterMs, MOVEMENT_SUITE_DEFAULT_AFTER_MS);
const movementTimelineSampleMs = numberArg(
  args.timelineMs ?? args.sampleMs ?? process.env.MIR2_PROBE_TIMELINE_SAMPLE_MS,
  500,
);
const captureWsFrames = booleanArg(
  args.captureWsFrames ?? args.capture_ws_frames ?? process.env.MIR2_PROBE_CAPTURE_WS_FRAMES,
  false,
);
const audioEnabled =
  args.audio === undefined && args.audioEnabled === undefined && process.env.MIR2_PROBE_AUDIO === undefined
    ? null
    : booleanArg(args.audio ?? args.audioEnabled ?? process.env.MIR2_PROBE_AUDIO, true);
const showOverlay = booleanArg(
  args.overlay ?? args.showOverlay ?? process.env.MIR2_PROBE_OVERLAY,
  false,
);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

class CdpClient {
  constructor(wsUrl, options = {}) {
    this.wsUrl = wsUrl;
    this.captureWsFrames = options.captureWsFrames === true;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
    this.consoleWarnings = [];
    this.network404s = [];
    this.webSocketFrames = [];
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
      if (message.error) reject(new Error(`${message.error.message}: ${message.error.data ?? ""}`));
      else resolve(message.result ?? {});
      return;
    }

    if (message.method === "Runtime.consoleAPICalled") {
      const type = message.params?.type;
      const text = (message.params?.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" ");
      if (type === "error") this.consoleErrors.push({ source: "console", text });
      if (type === "warning") this.consoleWarnings.push({ source: "console", text });
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
      if (entry?.level === "warning") {
        this.consoleWarnings.push({ source: entry.source ?? "log", text: entry.text ?? "" });
      }
    }

    if (message.method === "Network.responseReceived") {
      const response = message.params?.response;
      if (response?.status === 404 && !String(response.url ?? "").includes("favicon")) {
        this.network404s.push(response.url);
      }
    }

    if (
      this.captureWsFrames &&
      (
        message.method === "Network.webSocketFrameReceived" ||
        message.method === "Network.webSocketFrameSent"
      )
    ) {
      const response = message.params?.response;
      this.webSocketFrames.push({
        direction: message.method === "Network.webSocketFrameReceived" ? "in" : "out",
        timestamp: message.params?.timestamp ?? null,
        receivedAt: Date.now(),
        opcode: response?.opcode ?? null,
        payloadData: response?.payloadData ?? "",
      });
      this.webSocketFrames = this.webSocketFrames.slice(-200);
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
  const userDataDir = path.join(os.tmpdir(), `mir2-probe-${process.pid}-${Date.now()}`);
  const chrome = spawn(
    chromePath,
    [
      headed ? "" : "--headless=new",
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      "--no-first-run",
      "--no-default-browser-check",
      "--ignore-gpu-blocklist",
      "--enable-unsafe-webgpu",
      "about:blank",
    ].filter(Boolean),
    { stdio: "ignore" },
  );

  let client;
  try {
    await waitForChrome(debugPort);
    const target = await createTarget(debugPort, "about:blank");
    client = new CdpClient(target.webSocketDebuggerUrl, { captureWsFrames });
    await client.connect();
    await client.send("Page.enable");
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await setViewport(client, DEFAULT_VIEWPORT);
    if (audioEnabled === false) {
      await installAudioDisabledPreset(client);
    }

    const url = buildProbeUrl(baseUrl, label);
    const startedAt = Date.now();
    const navigation = await client.send("Page.navigate", { url });
    if (navigation.errorText) {
      throw new Error(`Page navigation failed for ${url}: ${navigation.errorText}`);
    }

    await waitForPageLoad(client);
    await waitForProbe(client, waitTimeoutMs);
    await client.evaluate(`window.__mir2Probe?.start?.()`);
    const initialSnapshot = await readProbeSnapshot(client, label);
    const actions = [];
    if (autoLogin) {
      actions.push(await loginAndStartGame(client, { account, password, createAccount, createCharacter, characterName }));
    }
    const movementSteps = movementSuiteEnabled
      ? buildMovementSuiteSteps(movementSuiteHoldMs, movementSuiteAfterMs)
      : parseMovementSequence(movementSequenceArg);
    if (movementSteps.length > 0) {
      if (!autoLogin) {
        throw new Error("--movement/--movementSuite requires --login true so the page is in game state");
      }
      actions.push(...(await runMovementSequence(client, movementSteps, label, movementTimelineSampleMs)));
    }
    await sleep(durationMs);
    const finalSnapshot = await readProbeSnapshot(client, label);
    await client.evaluate(`window.__mir2Probe?.stop?.()`);

    const report = {
      ok: false,
      runId,
      label,
      url,
      durationMs,
      movementTimelineSampleMs,
      elapsedMs: Date.now() - startedAt,
      actions,
      initialSnapshot,
      finalSnapshot,
      consoleErrors: client.consoleErrors,
      criticalConsoleErrors: client.consoleErrors.filter(isCriticalConsoleError),
      consoleWarnings: client.consoleWarnings,
      ...(captureWsFrames ? { webSocketFrames: compactWebSocketFrames(client.webSocketFrames) } : {}),
      nonFaviconNetwork404s: [...new Set(client.network404s)],
    };
    if (movementSuiteEnabled) {
      report.movementSuite = summarizeMovementSuite(report);
    }
    report.assertions = {
      probeReady: Boolean(finalSnapshot?.schema === "mir2-probe/1"),
      labelStamped: finalSnapshot?.label === label,
      frameLayerPresent: Boolean(finalSnapshot?.frame),
      stageLayerPresent: Boolean(finalSnapshot?.stage),
      noCriticalConsoleErrors: report.criticalConsoleErrors.length === 0,
      ...(movementSuiteEnabled ? { movementSuitePassed: report.movementSuite?.ok === true } : {}),
    };
    report.ok = Object.values(report.assertions).every(Boolean);

    const reportPath = path.join(outputDir, `${prefix}.json`);
    const latestPath = path.join(outputDir, "latest-mir2-probe.json");
    await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
    await fs.writeFile(latestPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
    let movementSuiteSummaryPath = null;
    let latestMovementSuiteSummaryPath = null;
    if (movementSuiteEnabled) {
      movementSuiteSummaryPath = path.join(outputDir, `${prefix}-movement-suite-summary.json`);
      latestMovementSuiteSummaryPath = path.join(outputDir, "latest-movement-suite-summary.json");
      await fs.writeFile(movementSuiteSummaryPath, `${JSON.stringify(report.movementSuite, null, 2)}\n`, "utf8");
      await fs.writeFile(latestMovementSuiteSummaryPath, `${JSON.stringify(report.movementSuite, null, 2)}\n`, "utf8");
    }

    console.log(
      JSON.stringify(
        {
          ok: report.ok,
          reportPath,
          latestPath,
          movementSuiteSummaryPath,
          latestMovementSuiteSummaryPath,
          assertions: report.assertions,
          movementSuite: report.movementSuite
            ? {
                ok: report.movementSuite.ok,
                passed: report.movementSuite.passed,
                failed: report.movementSuite.failed,
                warnings: report.movementSuite.warnings.length,
              }
            : undefined,
        },
        null,
        2,
      ),
    );
    process.exitCode = report.ok ? 0 : 1;
  } finally {
    client?.close();
    chrome.kill("SIGTERM");
    await fs.rm(userDataDir, { recursive: true, force: true }).catch(() => {});
  }
}

async function readProbeSnapshot(client, sampleLabel, profile = "full") {
  const raw = await client.evaluate(`
    (() => {
      const handle = window.__mir2Probe;
      if (!handle?.snapshot) return null;
      return JSON.stringify(handle.snapshot({
        label: ${JSON.stringify(sampleLabel)},
        profile: ${JSON.stringify(profile)},
      }));
    })()
  `);
  return raw ? JSON.parse(raw) : null;
}

async function loginAndStartGame(client, credentials) {
  await waitForExpression(
    client,
    `['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)`,
    "stage ready",
    20_000,
  );

  let screen = await client.evaluate(`window.__mir2Stage5?.state?.screen ?? null`);
  if (screen === "login") {
    await waitForExpression(client, `document.querySelector('.login-input.account')`, "login inputs", 15_000);
    await setInputValue(client, ".login-input.account", credentials.account);
    await setInputValue(client, ".login-input.password", credentials.password);
    if (credentials.createAccount) {
      await clickSelector(client, ".login-button.account button");
      await waitForExpression(client, `window.__mir2Stage5?.state?.wsState === 'open'`, "account creation socket", 15_000);
      await sleep(1_200);
    }
    await clickSelector(client, ".login-button.ok button");
    await waitForExpression(client, `window.__mir2Stage5?.state?.screen === 'select'`, "select screen", 30_000);
    screen = "select";
  }

  if (screen === "select") {
    if (credentials.createCharacter) {
      const created = await client.evaluate(`
        window.__mir2Stage5?.send?.(${JSON.stringify({
          type: "newCharacter",
          name: credentials.characterName,
          gender: "male",
          class: "warrior",
        })}) === true
      `);
      if (!created) throw new Error(`Failed to create probe character ${credentials.characterName}`);
      await waitForExpression(
        client,
        `(window.__mir2Stage5?.state?.characters ?? []).some((character) => character?.name === ${JSON.stringify(credentials.characterName)})`,
        "probe character creation",
        15_000,
      );
    }
    const started = await client.evaluate(`
      (() => {
        const state = window.__mir2Stage5?.state;
        const selectedIndex = state?.selectedCharacterIndex ?? 0;
        const requestedName = ${JSON.stringify(credentials.createCharacter ? credentials.characterName : null)};
        const character = requestedName
          ? state?.characters?.find((entry) => entry?.name === requestedName)
          : state?.characters?.[selectedIndex] ?? state?.characters?.[0] ?? null;
        if (!character) return false;
        return window.__mir2Stage5?.send?.({ type: "startGame", characterIndex: character.index ?? selectedIndex }) === true;
      })()
    `);
    if (!started) {
      await clickSelector(client, ".select-action.start button");
    }
  }

  await waitForExpression(
    client,
    `window.__mir2Stage5?.state?.screen === 'game' && Boolean(window.__mir2Stage5?.state?.player)`,
    "game screen",
    60_000,
  );
  await waitForExpression(client, `!document.querySelector('.login-transition-overlay')`, "login transition clear", 10_000);
  await waitForExpression(
    client,
    `window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)`,
    "scene interaction ready",
    45_000,
  );

  return {
    type: "login",
    stage: await readStageState(client),
  };
}

async function runMovementSequence(client, steps, sampleLabel, timelineSampleMs) {
  const actions = [];
  await client.send("Page.bringToFront");
  for (const [index, step] of steps.entries()) {
    await waitForMovementProbeStable(client, sampleLabel, `movement ${index} stable`);
    const before = await readStageState(client);
    const beforeProbe = await readProbeSnapshot(client, sampleLabel, "movement");
    const startedAt = Date.now();
    const dispatch = dispatchKeyboardStep(client, step);
    const timeline = await sampleMovementTimeline(client, sampleLabel, dispatch, timelineSampleMs);
    await dispatch;
    const keyUpAt = Date.now();
    await sleep(step.afterMs);
    const completedAt = Date.now();
    const after = await readStageState(client);
    const afterProbe = await readProbeSnapshot(client, sampleLabel, "movement");
    actions.push({
      type: "movement",
      index,
      step,
      startedAt,
      keyUpAt,
      completedAt,
      before,
      after,
      timeline,
      probe: {
        before: compactMovementProbe(beforeProbe),
        after: compactMovementProbe(afterProbe),
      },
    });
  }
  return actions;
}

async function waitForMovementProbeStable(client, sampleLabel, label, timeoutMs = 30_000) {
  const startedAt = Date.now();
  let stableSince = null;
  let lastDetail = null;
  while (Date.now() - startedAt < timeoutMs) {
    const sample = await readProbeSnapshot(client, sampleLabel, "movement").catch(() => null);
    const stage = sample?.stage ?? {};
    const frame = sample?.frame ?? {};
    const sceneReady =
      stage.screen === "game" &&
      Boolean(stage.player) &&
      (stage.sceneInteractionReady === true || stage.sceneAssetReadiness?.ready === true);
    const bevyReady =
      !sample?.bevy?.backend ||
      sample.bevy.lastPhase === "map-render-synced" ||
      sample.bevy.lastPhase === "runtime-ready" ||
      sample.bevy.lastPhase === null;
    const frameReady =
      (frame.longtasksInLastSecond ?? 0) <= MOVEMENT_ACCEPTANCE.maxLongtasksInLastSecond &&
      (frame.rafDeltaMax ?? 0) <= MOVEMENT_ACCEPTANCE.maxRafDeltaMaxMs &&
      (frame.timerDriftMax ?? 0) <= MOVEMENT_ACCEPTANCE.maxTimerDriftMaxMs;
    const queuesReady = (sample?.movement?.movementQueueDepth ?? 0) === 0;
    const ready = sceneReady && bevyReady && frameReady && queuesReady;
    lastDetail = {
      sceneReady,
      bevyReady,
      frameReady,
      queuesReady,
      screen: stage.screen ?? null,
      bevyPhase: sample?.bevy?.lastPhase ?? null,
      movementQueueDepth: sample?.movement?.movementQueueDepth ?? null,
      frame,
    };
    if (ready) {
      stableSince ??= Date.now();
      if (Date.now() - stableSince >= 1_000) return sample;
    } else {
      stableSince = null;
    }
    await sleep(100);
  }
  throw new Error(`Timed out waiting for ${label}: ${JSON.stringify(lastDetail)}`);
}

async function sampleMovementTimeline(client, sampleLabel, dispatchPromise, intervalMs) {
  const timeline = [];
  let done = false;
  dispatchPromise.finally(() => {
    done = true;
  });
  while (!done) {
    const sample = await readProbeSnapshot(client, sampleLabel, "movement").catch(() => null);
    timeline.push(compactMovementProbe(sample));
    await sleep(intervalMs);
  }
  const sample = await readProbeSnapshot(client, sampleLabel, "movement").catch(() => null);
  timeline.push(compactMovementProbe(sample));
  return timeline;
}

async function dispatchKeyboardStep(client, step) {
  const modifiers = step.shift ? 8 : 0;
  if (step.shift) {
    await client.send("Input.dispatchKeyEvent", {
      type: "keyDown",
      key: "Shift",
      code: "ShiftLeft",
      windowsVirtualKeyCode: 16,
      modifiers,
    });
  }
  await client.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: step.key,
    code: step.code,
    windowsVirtualKeyCode: step.windowsVirtualKeyCode,
    modifiers,
  });
  await sleep(step.holdMs);
  await client.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: step.key,
    code: step.code,
    windowsVirtualKeyCode: step.windowsVirtualKeyCode,
    modifiers,
  });
  if (step.shift) {
    await client.send("Input.dispatchKeyEvent", {
      type: "keyUp",
      key: "Shift",
      code: "ShiftLeft",
      windowsVirtualKeyCode: 16,
      modifiers: 0,
    });
  }
}

function parseMovementSequence(value) {
  const raw = String(value ?? "").trim();
  if (!raw) return [];
  if (raw === "shift-arrows") {
    return [
      movementStep("ArrowRight", true, 220, 850),
      movementStep("ArrowRight", true, 220, 850),
      movementStep("ArrowLeft", true, 220, 850),
      movementStep("ArrowUp", true, 220, 850),
    ];
  }
  return raw
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const [combo, holdRaw] = part.split(":", 2);
      const keys = combo.split("+").map((key) => key.trim()).filter(Boolean);
      const shift = keys.some((key) => key.toLowerCase() === "shift");
      const key = keys.find((entry) => entry.toLowerCase() !== "shift") ?? combo;
      return movementStep(key, shift, numberArg(holdRaw, 220), 850);
    });
}

function buildMovementSuiteSteps(holdMs, afterMs) {
  return [
    ...MOVEMENT_SUITE_DIRECTIONS.map((direction) => movementSuiteStep(direction, false, holdMs, afterMs)),
    ...MOVEMENT_SUITE_RUN_DIRECTIONS.map((direction) => movementSuiteStep(direction, true, holdMs, afterMs)),
  ];
}

function movementSuiteStep(direction, shift, holdMs, afterMs) {
  return {
    ...movementStep(direction, shift, holdMs, afterMs),
    suiteCase: `${shift ? "run" : "walk"}-${direction.toLowerCase()}`,
    expectedMode: shift ? "run" : "walk",
    direction,
  };
}

function movementStep(key, shift, holdMs, afterMs) {
  const normalized = normalizeMovementKey(key);
  return {
    key: normalized.key,
    code: normalized.code,
    windowsVirtualKeyCode: normalized.windowsVirtualKeyCode,
    shift,
    holdMs,
    afterMs,
    direction: normalized.direction,
    expectedMode: shift ? "run" : "walk",
  };
}

function normalizeMovementKey(key) {
  const value = String(key);
  const lower = value.toLowerCase();
  if (lower === "right" || lower === "arrowright") {
    return { key: "ArrowRight", code: "ArrowRight", windowsVirtualKeyCode: 39, direction: "Right" };
  }
  if (lower === "left" || lower === "arrowleft") {
    return { key: "ArrowLeft", code: "ArrowLeft", windowsVirtualKeyCode: 37, direction: "Left" };
  }
  if (lower === "up" || lower === "arrowup") {
    return { key: "ArrowUp", code: "ArrowUp", windowsVirtualKeyCode: 38, direction: "Up" };
  }
  if (lower === "down" || lower === "arrowdown") {
    return { key: "ArrowDown", code: "ArrowDown", windowsVirtualKeyCode: 40, direction: "Down" };
  }
  throw new Error(`Unsupported movement key: ${key}`);
}

async function readStageState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const player = state.player ?? null;
      return {
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        sceneInteractionReady: state.sceneInteractionReady ?? null,
        sceneAssetReadiness: state.sceneAssetReadiness ?? null,
        worldSnapshotVersion: state.worldSnapshotVersion ?? null,
        player: player
          ? {
              objectId: player.objectId ?? null,
              x: player.x,
              y: player.y,
              serverX: player.serverX ?? null,
              serverY: player.serverY ?? null,
              renderX: player.renderX ?? null,
              renderY: player.renderY ?? null,
              direction: player.direction ?? null,
              dead: player.dead ?? null,
            }
          : null,
      };
    })()
  `);
}

function compactMovementProbe(sample) {
  const movement = sample?.movement ?? null;
  const gateway = sample?.gateway?.metrics ?? null;
  const stage = sample?.stage ?? null;
  const frame = sample?.frame ?? null;
  const lastSample = movement?.lastSample ?? null;
  return {
    at: sample?.epoch ?? null,
    t: sample?.t ?? null,
    ack: movement?.lastAckOutcome ?? null,
    movementQueueDepth: movement?.movementQueueDepth ?? null,
    diagnosticEventCount: movement?.diagnosticEventCount ?? null,
    nextMoveSendAtDelta: movement?.nextMoveSendAtDelta ?? null,
    inputBlockedUntilDelta: movement?.inputBlockedUntilDelta ?? null,
    queues: compactMovementQueues(lastSample?.queues ?? null),
    sample: compactMovementSample(lastSample),
    sent: movement?.sentCommands?.slice?.(-8) ?? [],
    received: movement?.receivedPackets?.slice?.(-8) ?? [],
    keyboard: movement?.keyboardEvents?.slice?.(0, 12) ?? [],
    shellRenderPerf: movement?.shellRenderPerf?.slice?.(-8) ?? [],
    stage: stage
      ? {
          screen: stage.screen ?? null,
          playerObjectId: stage.playerObjectId ?? null,
          player: stage.player ?? null,
          predictedPlayer: stage.predictedPlayer ?? null,
          sceneInteractionReady: stage.sceneInteractionReady ?? null,
          sceneAssetReadiness: stage.sceneAssetReadiness ?? null,
          worldSnapshotVersion: stage.worldSnapshotVersion ?? null,
        }
      : null,
    tick: gateway?.tick ?? null,
    sharedTick: gateway?.sharedTick ?? null,
    frame,
  };
}

async function installAudioDisabledPreset(client) {
  await client.send("Page.addScriptToEvaluateOnNewDocument", {
    source: `
      (() => {
        try {
          localStorage.setItem("mir2.originalAudioSettings", JSON.stringify({
            musicEnabled: false,
            effectsEnabled: false,
            musicVolume: 0,
            effectsVolume: 0
          }));
        } catch {}
      })();
    `,
  });
}

function compactWebSocketFrames(frames) {
  return (frames ?? []).map((frame) => {
    const payload = String(frame.payloadData ?? "");
    let parsed = null;
    try {
      parsed = JSON.parse(payload);
    } catch {
      parsed = null;
    }
    return {
      direction: frame.direction,
      timestamp: frame.timestamp,
      receivedAt: frame.receivedAt,
      opcode: frame.opcode,
      packet: parsed?.packet ?? parsed?.type ?? null,
      payload: parsed?.payload ?? null,
      payloadPreview: payload.length > 500 ? `${payload.slice(0, 500)}...` : payload,
    };
  });
}

function compactMovementQueues(queues) {
  if (!queues || typeof queues !== "object") return null;
  const movementPlan = queues.movementPlan;
  const pendingSelfMove = queues.pendingSelfMove;
  const queuedMoveIntent = queues.queuedMoveIntent;
  return {
    movementPlan: movementPlan
      ? {
          mode: movementPlan.mode ?? null,
          targetX: movementPlan.targetX ?? null,
          targetY: movementPlan.targetY ?? null,
          pendingX: movementPlan.pendingX ?? null,
          pendingY: movementPlan.pendingY ?? null,
          waitMs: movementPlan.waitMs ?? null,
          pendingAgeMs: movementPlan.pendingAgeMs ?? null,
        }
      : null,
    pendingSelfMove: pendingSelfMove
      ? {
          mode: pendingSelfMove.mode ?? null,
          from: pendingSelfMove.from ?? null,
          to: pendingSelfMove.to ?? null,
          targetX: pendingSelfMove.targetX ?? null,
          targetY: pendingSelfMove.targetY ?? null,
          direction: pendingSelfMove.direction ?? null,
          sentAgeMs: pendingSelfMove.sentAt ? Date.now() - pendingSelfMove.sentAt : null,
        }
      : null,
    queuedMoveIntent: queuedMoveIntent
      ? {
          kind: queuedMoveIntent.kind ?? null,
          mode: queuedMoveIntent.mode ?? queuedMoveIntent.requestedMode ?? null,
          direction: queuedMoveIntent.direction ?? null,
          consumeAfterSend: queuedMoveIntent.consumeAfterSend ?? null,
          requestedAgeMs: queuedMoveIntent.requestedAt ? Date.now() - queuedMoveIntent.requestedAt : null,
        }
      : null,
    nextMoveWaitMs: queues.nextMoveWaitMs ?? null,
    queuedDirectionStep: queues.queuedDirectionStep
      ? {
          mode: queues.queuedDirectionStep.mode ?? null,
          direction: queues.queuedDirectionStep.direction ?? null,
          repeatCount: queues.queuedDirectionStep.repeatCount ?? null,
        }
      : null,
    directionStepPending: queues.directionStepPending
      ? {
          mode: queues.directionStepPending.mode ?? null,
          direction: queues.directionStepPending.direction ?? null,
          sentAgeMs: queues.directionStepPending.sentAgeMs ?? null,
        }
      : null,
    directionStepPendingQueueLength: queues.directionStepPendingQueueLength ?? null,
    crystalSelfActionFeedLength: queues.crystalSelfActionFeedLength ?? null,
    outstandingSelfMovementActionsLength: queues.outstandingSelfMovementActionsLength ?? null,
    movementInputBlockedForMs: queues.movementInputBlockedForMs ?? null,
  };
}

function compactMovementSample(sample) {
  if (!sample || typeof sample !== "object") return null;
  return {
    self: sample.self ?? null,
    render: sample.render ?? null,
    predicted: sample.predicted ?? null,
    transport: {
      lastMovementCommand: sample.transport?.lastMovementCommand ?? null,
      lastSelfMovementAck: sample.transport?.lastSelfMovementAck ?? null,
      lastSelfNoProgressAck: sample.transport?.lastSelfNoProgressAck ?? null,
    },
  };
}

function summarizeMovementSuite(report) {
  const movementActions = (report.actions ?? []).filter((action) => action.type === "movement");
  const cases = movementActions.map((action) => summarizeMovementAction(action));
  const finalSnapshot = report.finalSnapshot ?? {};
  const finalStage = finalSnapshot.stage ?? {};
  const finalMovement = finalSnapshot.movement ?? {};
  const finalPlayer = finalStage.player ?? null;
  const finalPending = finalMovement.lastSample?.queues?.pendingSelfMove ?? null;
  const finalPredicted = finalStage.predictedPlayer ?? finalMovement.lastSample?.predicted ?? null;
  const finalRenderMatchesServer =
    !finalPlayer ||
    finalPlayer.serverX === null ||
    finalPlayer.serverY === null ||
    finalPlayer.renderX === null ||
    finalPlayer.renderY === null ||
    (finalPlayer.serverX === finalPlayer.renderX && finalPlayer.serverY === finalPlayer.renderY);
  const finalChecks = [
    {
      name: "finalQueueEmpty",
      ok: (finalMovement.movementQueueDepth ?? 0) === 0 && !finalPending,
      detail: { movementQueueDepth: finalMovement.movementQueueDepth ?? null, finalPending },
    },
    {
      name: "finalPredictionCleared",
      ok: !finalPredicted,
      detail: { predictedPlayer: finalPredicted },
    },
    {
      name: "finalRenderMatchesServer",
      ok: finalRenderMatchesServer,
      detail: finalPlayer
        ? {
            serverX: finalPlayer.serverX ?? null,
            serverY: finalPlayer.serverY ?? null,
            renderX: finalPlayer.renderX ?? null,
            renderY: finalPlayer.renderY ?? null,
          }
        : null,
    },
    {
      name: "noLongTasks",
      ok: (finalSnapshot.frame?.longtasksInLastSecond ?? 0) <= MOVEMENT_ACCEPTANCE.maxLongtasksInLastSecond,
      detail: finalSnapshot.frame ?? null,
    },
    {
      name: "gatewayTickHealthy",
      ok:
        (finalSnapshot.gateway?.metrics?.tick?.msAvg ?? 0) <= MOVEMENT_ACCEPTANCE.maxFinalGatewayTickAvgMs &&
        (finalSnapshot.gateway?.metrics?.tick?.msMax ?? 0) <= MOVEMENT_ACCEPTANCE.maxFinalGatewayTickMaxMs,
      detail: finalSnapshot.gateway?.metrics?.tick ?? null,
    },
  ];
  const failures = [
    ...cases.flatMap((entry) => entry.failures.map((failure) => ({ case: entry.case, ...failure }))),
    ...finalChecks.filter((check) => !check.ok).map((check) => ({ case: "final", ...check })),
  ];
  const warnings = cases.flatMap((entry) => entry.warnings.map((warning) => ({ case: entry.case, ...warning })));
  return {
    ok: failures.length === 0,
    label: report.label,
    runId: report.runId,
    generatedAt: new Date().toISOString(),
    thresholds: MOVEMENT_ACCEPTANCE,
    passed: cases.filter((entry) => entry.ok).length,
    failed: cases.filter((entry) => !entry.ok).length,
    failures,
    warnings,
    finalChecks,
    cases,
  };
}

function summarizeMovementAction(action) {
  const expectedDirection = action.step?.direction ?? null;
  const sent = collectMovementItems(action, "sent").filter(
    (entry) => !expectedDirection || entry.direction === expectedDirection,
  );
  const received = collectMovementItems(action, "received").filter(
    (entry) => !expectedDirection || entry.payload?.direction === expectedDirection,
  );
  const sentGaps = gapsBetween(sent.map((entry) => entry.at));
  const expectedMode = action.step?.expectedMode ?? (action.step?.shift ? "run" : "walk");
  const runCommands = sent.filter((entry) => entry.type === "run");
  const walkCommands = sent.filter((entry) => entry.type === "walk");
  const steadyGaps = expectedMode === "run" ? gapsBetween(runCommands.map((entry) => entry.at)) : sentGaps;
  const startupGap = expectedMode === "run" && sent.length > 1 ? sent[1].at - sent[0].at : null;
  const ackLatencies = received
    .map((packet, index) => (sent[index]?.at ? packet.at - sent[index].at : null))
    .filter((value) => typeof value === "number" && Number.isFinite(value) && value >= 0);
  const missingAckCount = Math.max(0, sent.length - received.length);
  const maxSceneAssetTotal = maxTimelineNumber(action, (entry) => entry.stage?.sceneAssetReadiness?.total);
  const maxSceneAssetPending = maxTimelineNumber(action, (entry) => entry.stage?.sceneAssetReadiness?.pending);
  const longtaskDeltaInAction = frameCounterDelta(action, "longtaskCountTotal");
  const rafSpikeDeltaInAction = frameCounterDelta(action, "rafDeltaSpikeCountTotal");
  const timerSpikeDeltaInAction = frameCounterDelta(action, "timerDriftSpikeCountTotal");
  const maxLongtasksInAction = maxTimelineNumber(action, (entry) => entry.frame?.longtasksInLastSecond);
  const maxRafDeltaMax = maxTimelineNumber(action, (entry) => entry.frame?.rafDeltaMax);
  const maxTimerDriftMax = maxTimelineNumber(action, (entry) => entry.frame?.timerDriftMax);
  const afterProbe = action.probe?.after ?? {};
  const afterPlayer = action.after?.player ?? afterProbe.stage?.player ?? null;
  const afterPending = afterProbe.queues?.pendingSelfMove ?? null;
  const afterPredicted = afterProbe.sample?.predicted ?? afterProbe.stage?.predictedPlayer ?? null;
  const afterRenderMatchesServer =
    !afterPlayer ||
    afterPlayer.serverX === null ||
    afterPlayer.serverY === null ||
    afterPlayer.renderX === null ||
    afterPlayer.renderY === null ||
    (afterPlayer.serverX === afterPlayer.renderX && afterPlayer.serverY === afterPlayer.renderY);
  const expectedCommandCount = Math.max(3, Math.floor((action.step?.holdMs ?? 0) / MOVEMENT_ACCEPTANCE.expectedCadenceMs));
  const expectedRunCount = expectedMode === "run" ? Math.max(2, expectedCommandCount - 1) : 0;
  const failures = [];
  const warnings = [];

  addCheck(failures, "enoughCommands", sent.length >= expectedCommandCount, {
    expectedCommandCount,
    sentCount: sent.length,
  });
  if (expectedMode === "walk") {
    addCheck(failures, "walkOnly", sent.length === 0 || sent.every((entry) => entry.type === "walk"), {
      commandTypes: sent.map((entry) => entry.type),
    });
  } else {
    addCheck(failures, "runCommandsPresent", runCommands.length >= expectedRunCount, {
      expectedRunCount,
      runCount: runCommands.length,
      walkCount: walkCommands.length,
    });
    addCheck(failures, "startupGap", startupGap === null || startupGap <= MOVEMENT_ACCEPTANCE.maxStartupGapMs, {
      startupGap,
      maxStartupGapMs: MOVEMENT_ACCEPTANCE.maxStartupGapMs,
    });
  }
  addCheck(failures, "steadyCadenceMax", maxNumber(steadyGaps) <= MOVEMENT_ACCEPTANCE.maxSteadyGapMs, {
    steadyGaps,
    maxSteadyGapMs: MOVEMENT_ACCEPTANCE.maxSteadyGapMs,
  });
  addCheck(failures, "steadyCadenceP95", percentile(steadyGaps, 0.95) <= MOVEMENT_ACCEPTANCE.maxSteadyGapP95Ms, {
    steadyGaps,
    p95: percentile(steadyGaps, 0.95),
    maxSteadyGapP95Ms: MOVEMENT_ACCEPTANCE.maxSteadyGapP95Ms,
  });
  addCheck(failures, "ackLatencyMax", maxNumber(ackLatencies) <= MOVEMENT_ACCEPTANCE.maxAckLatencyMs, {
    ackLatencies,
    maxAckLatencyMs: MOVEMENT_ACCEPTANCE.maxAckLatencyMs,
  });
  addCheck(failures, "ackLatencyP95", percentile(ackLatencies, 0.95) <= MOVEMENT_ACCEPTANCE.maxAckLatencyP95Ms, {
    ackLatencies,
    p95: percentile(ackLatencies, 0.95),
    maxAckLatencyP95Ms: MOVEMENT_ACCEPTANCE.maxAckLatencyP95Ms,
  });
  addCheck(failures, "noMissingAck", missingAckCount === 0, {
    sentCount: sent.length,
    receivedCount: received.length,
    missingAckCount,
  });
  addCheck(failures, "queueSettled", (afterProbe.movementQueueDepth ?? 0) === 0 && !afterPending, {
    movementQueueDepth: afterProbe.movementQueueDepth ?? null,
    pendingSelfMove: afterPending,
  });
  addCheck(failures, "predictionCleared", !afterPredicted, { predicted: afterPredicted });
  addCheck(failures, "renderMatchesServer", afterRenderMatchesServer, {
    serverX: afterPlayer?.serverX ?? null,
    serverY: afterPlayer?.serverY ?? null,
    renderX: afterPlayer?.renderX ?? null,
    renderY: afterPlayer?.renderY ?? null,
  });
  addCheck(failures, "sceneAssetBudget", maxSceneAssetTotal <= MOVEMENT_ACCEPTANCE.maxSceneAssetTotal, {
    maxSceneAssetTotal,
    maxSceneAssetPending,
    maxSceneAssetTotalAllowed: MOVEMENT_ACCEPTANCE.maxSceneAssetTotal,
  });
  addCheck(failures, "noActionLongtasks", longtaskDeltaInAction <= MOVEMENT_ACCEPTANCE.maxLongtasksInLastSecond, {
    longtaskDeltaInAction,
    maxLongtasksInAction,
    maxLongtasksInLastSecond: MOVEMENT_ACCEPTANCE.maxLongtasksInLastSecond,
  });
  addCheck(failures, "noActionRafSpikes", rafSpikeDeltaInAction === 0, {
    rafSpikeDeltaInAction,
    maxRafDeltaMax,
    maxRafDeltaMaxMs: MOVEMENT_ACCEPTANCE.maxRafDeltaMaxMs,
  });
  addCheck(failures, "noActionTimerSpikes", timerSpikeDeltaInAction === 0, {
    timerSpikeDeltaInAction,
    maxTimerDriftMax,
    maxTimerDriftMaxMs: MOVEMENT_ACCEPTANCE.maxTimerDriftMaxMs,
  });
  if (missingAckCount > 0) {
    warnings.push({
      name: "missingAck",
      detail: { sentCount: sent.length, receivedCount: received.length, missingAckCount },
    });
  }

  return {
    ok: failures.length === 0,
    case: action.step?.suiteCase ?? `${expectedMode}-${action.step?.direction ?? action.step?.key ?? action.index}`,
    index: action.index,
    expectedMode,
    direction: action.step?.direction ?? null,
    holdMs: action.step?.holdMs ?? null,
    sentCount: sent.length,
    receivedCount: received.length,
    missingAckCount,
    sentGaps,
    steadyGaps,
    startupGap,
    ackLatencies,
    steadyGapP95: percentile(steadyGaps, 0.95),
    ackLatencyP95: percentile(ackLatencies, 0.95),
    maxSceneAssetTotal,
    maxSceneAssetPending,
    longtaskDeltaInAction,
    rafSpikeDeltaInAction,
    timerSpikeDeltaInAction,
    maxLongtasksInAction,
    maxRafDeltaMax,
    maxTimerDriftMax,
    before: action.before,
    after: action.after,
    sent,
    received,
    failures,
    warnings,
  };
}

function collectMovementItems(action, key) {
  const startedAt = (action.startedAt ?? 0) - 100;
  const completedAt = (action.completedAt ?? Number.MAX_SAFE_INTEGER) + 100;
  const probes = [
    action.probe?.before,
    ...(action.timeline ?? []),
    action.probe?.after,
  ].filter(Boolean);
  const items = [];
  for (const probe of probes) {
    for (const item of probe?.[key] ?? []) {
      if (!item || typeof item !== "object") continue;
      if (typeof item.at !== "number") continue;
      if (item.at < startedAt || item.at > completedAt) continue;
      items.push(item);
    }
  }
  const byKey = new Map();
  for (const item of items) {
    byKey.set(movementItemKey(item), item);
  }
  return [...byKey.values()].sort((left, right) => left.at - right.at);
}

function movementItemKey(item) {
  if (typeof item.movementSeq === "number" && Number.isFinite(item.movementSeq)) {
    return [
      "movementSeq",
      item.movementSeq,
      item.type ?? "",
      item.packet ?? "",
      item.direction ?? item.payload?.direction ?? "",
      item.payload?.x ?? "",
      item.payload?.y ?? "",
    ].join(":");
  }
  return [
    item.at ?? "",
    item.movementSeq ?? "",
    item.type ?? "",
    item.packet ?? "",
    item.payload?.x ?? "",
    item.payload?.y ?? "",
  ].join(":");
}

function maxTimelineNumber(action, read) {
  const values = (action.timeline ?? [])
    .map(read)
    .filter((value) => typeof value === "number" && Number.isFinite(value));
  return maxNumber(values);
}

function frameCounterDelta(action, key) {
  const beforeValue = action.probe?.before?.frame?.[key];
  const baseline = typeof beforeValue === "number" && Number.isFinite(beforeValue) ? beforeValue : 0;
  const values = [
    ...(action.timeline ?? []).map((entry) => entry.frame?.[key]),
    action.probe?.after?.frame?.[key],
  ].filter((value) => typeof value === "number" && Number.isFinite(value));
  if (values.length === 0) return 0;
  return Math.max(0, maxNumber(values) - baseline);
}

function gapsBetween(values) {
  const gaps = [];
  for (let index = 1; index < values.length; index += 1) {
    const gap = values[index] - values[index - 1];
    if (Number.isFinite(gap)) gaps.push(gap);
  }
  return gaps;
}

function maxNumber(values) {
  return values.length ? Math.max(...values) : 0;
}

function percentile(values, fraction) {
  if (!values.length) return 0;
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * fraction) - 1));
  return sorted[index];
}

function addCheck(failures, name, ok, detail) {
  if (ok) return;
  failures.push({ name, detail });
}

async function setInputValue(client, selector, value) {
  const updated = await client.evaluate(`
    (() => {
      const input = document.querySelector(${JSON.stringify(selector)});
      if (!input) return false;
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      setter.call(input, ${JSON.stringify(value)});
      input.dispatchEvent(new Event("input", { bubbles: true }));
      return true;
    })()
  `);
  if (!updated) throw new Error(`Could not set input ${selector}`);
}

async function clickSelector(client, selector) {
  const clicked = await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click ${selector}`);
}

async function waitForExpression(client, expression, label, timeoutMs) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const ok = await client.evaluate(`Boolean(${expression})`);
    if (ok) return;
    await sleep(100);
  }
  const state = await client.evaluate(`
    (() => ({
      stage: window.__mir2Stage5?.state ?? null,
      bodyText: document.body?.innerText?.slice(0, 600) ?? "",
      lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
      gatewayEventHistory: (window.__mir2GatewayEventHistory ?? []).slice(0, 20),
      commandHistory: window.__mir2CommandHistory ?? [],
      movementSentCommands: window.__mir2MovementSentCommands ?? [],
      movementReceivedPackets: window.__mir2MovementReceivedPackets ?? [],
      logsTail: (window.__mir2Stage5?.state?.logs ?? []).slice(0, 10).map((line) => line?.text ?? String(line)),
    }))()
  `);
  throw new Error(`Timed out waiting for ${label}; state=${JSON.stringify(state)}`);
}

async function waitForProbe(client, timeoutMs) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const ready = await client.evaluate(`Boolean(window.__mir2Probe?.snapshot)`);
    if (ready) return;
    await sleep(100);
  }
  throw new Error(`Timed out waiting ${timeoutMs}ms for window.__mir2Probe`);
}

async function waitForPageLoad(client) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < 15_000) {
    const ready = await client.evaluate(`document.readyState === "complete" || document.readyState === "interactive"`);
    if (ready) return;
    await sleep(100);
  }
  throw new Error("Timed out waiting for page load");
}

async function setViewport(client, viewport) {
  await client.send("Emulation.setDeviceMetricsOverride", viewport);
}

async function waitForChrome(port) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < 15_000) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) return;
    } catch {}
    await sleep(100);
  }
  throw new Error(`Chrome did not open remote debugging port ${port}`);
}

async function createTarget(port, url) {
  const response = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(url)}`, {
    method: "PUT",
  });
  if (!response.ok) {
    throw new Error(`Unable to create Chrome target: ${response.status} ${await response.text()}`);
  }
  return response.json();
}

function buildProbeUrl(rawUrl, sampleLabel) {
  const parsed = new URL(rawUrl);
  if (showOverlay) parsed.searchParams.set("probe", sampleLabel || "headless");
  else parsed.searchParams.set("probeLabel", sampleLabel || "headless");
  if (args.gatewayWs) parsed.searchParams.set("gatewayWs", args.gatewayWs);
  if (args.query) {
    const extra = new URLSearchParams(args.query);
    for (const [key, value] of extra) parsed.searchParams.set(key, value);
  }
  return parsed.toString();
}

function isCriticalConsoleError(entry) {
  const text = entry.text ?? "";
  return !/favicon|ResizeObserver loop|WebGPU is not supported|Failed to load resource|404|net::ERR_/i.test(text);
}

function findChromePath() {
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    path.join(process.env.LOCALAPPDATA ?? "", "Google\\Chrome\\Application\\chrome.exe"),
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ];
  return candidates.find((candidate) => fileExistsSync(candidate)) ?? null;
}

function fileExistsSync(filePath) {
  try {
    return fsSync.existsSync(filePath);
  } catch {
    return false;
  }
}

function sanitizeLabel(value) {
  return String(value || "headless")
    .replace(/[^a-zA-Z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64) || "headless";
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function numberArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (!value.startsWith("--")) continue;
    const [rawKey, inlineValue] = value.slice(2).split("=", 2);
    const key = rawKey.trim();
    if (!key) continue;
    if (inlineValue !== undefined) {
      parsed[key] = inlineValue;
    } else {
      parsed[key] = argv[index + 1] && !argv[index + 1].startsWith("--") ? argv[++index] : "true";
    }
  }
  return parsed;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

await main();
