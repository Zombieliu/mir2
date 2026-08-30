import { spawn } from "node:child_process";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { DEFAULT_LOCAL_COMMAND_POSE_LATENCY_BUDGET_MS } from "./local-command-pose-latency.mjs";

const LOOPBACK_HOST = "127.0.0.1";
const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const CAPTURE_SCRIPT = path.join(SCRIPT_DIR, "capture-web-movement-jitter.mjs");
const DEFAULT_GATEWAY_EXE = path.join(REPO_ROOT, "target", "debug", "mir2-gateway.exe");
const DEFAULT_BASE_URL = "http://127.0.0.1:3002";
const DEFAULT_OUTPUT_DIR = path.join(
  REPO_ROOT,
  "docs",
  "generated",
  "player-qa",
  "bevy-movement-shadow",
);
const DEFAULT_MAP = "0";
const DEFAULT_X = 330;
const DEFAULT_Y = 270;
const DEFAULT_KEYS = "d,d,a,a";
const DEFAULT_KEY_PRESS_COUNT = 4;
const DEFAULT_KEY_INTERVAL_MS = 700;
const DEFAULT_SAMPLE_MS = 50;
const DEFAULT_PRE_INTERACTION_DELAY_MS = 1_000;
const DEFAULT_SETTLE_MS = 3_000;
const DEFAULT_GATEWAY_READY_TIMEOUT_MS = 30_000;
const DEFAULT_CAPTURE_TIMEOUT_MS = 180_000;
const DEFAULT_FINAL_SCENE_READY_TIMEOUT_MS = 30_000;
const DEFAULT_FINAL_RENDERER_READY_TIMEOUT_MS = 15_000;
const MAX_CHILD_OUTPUT_CHARS = 512 * 1024;
const TEMP_DIR_PREFIX = "mir2-bevy-movement-shadow-";
const REDACTED = "[redacted]";
const KNOWN_ARGS = new Set([
  "baseUrl",
  "bevyLocalMotion",
  "bevyPoseCommit",
  "captureFrameImages",
  "captureTimeoutMs",
  "chromeDebugPort",
  "clickSequence",
  "clickSequencePostMs",
  "expectedFinalX",
  "expectedFinalY",
  "expectedMovementCommandCount",
  "finalRendererReadyTimeoutMs",
  "finalSceneReadyTimeoutMs",
  "frameCaptureMode",
  "frameImageFormat",
  "frameImageQuality",
  "gatewayExe",
  "gatewayPath",
  "gatewayReadyTimeoutMs",
  "gatewayTcpPort",
  "gatewayWebPort",
  "help",
  "headed",
  "interaction",
  "keyIntervalMs",
  "keyPressCount",
  "keys",
  "maxLocalCommandPoseLatencyMs",
  "map",
  "output",
  "preInteractionDelayMs",
  "sampleMs",
  "settleMs",
  "skipStartTransfer",
  "windowFrameActivate",
  "windowFrameCropMode",
  "windowFrameMinimizeTitlePatterns",
  "x",
  "y",
]);

const activeChildren = new Set();
const runtimeSecrets = new Set();
let interruptedSignal = null;

try {
  const args = parseArgs(process.argv.slice(2));
  validateKnownArgs(args);
  if (booleanArg(args.help, false, "help")) {
    printHelp();
  } else {
    await main(args);
  }
} catch (error) {
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: safeErrorMessage(error),
      },
      null,
      2,
    ),
  );
  process.exitCode = 1;
}

async function main(args) {
  const config = buildConfig(args);
  await fs.mkdir(config.outputDir, { recursive: true });

  const account = randomUUID();
  const identityKey = account.replaceAll("-", "");
  const password = `Mir2${identityKey.slice(0, 12)}A1`;
  const qaControlToken = randomUUID();
  const characterName = `M${identityKey.slice(16, 27)}`;
  runtimeSecrets.add(account);
  runtimeSecrets.add(password);
  runtimeSecrets.add(qaControlToken);

  const startedAt = Date.now();
  const runId = `${timestamp()}-${identityKey.slice(-8)}`;
  const prefix = `bevy-movement-shadow-webgpu-${runId}`;
  const screenshotPath = path.join(config.outputDir, `${prefix}.png`);
  const statePath = path.join(config.outputDir, `${prefix}.json`);
  const reportPath = path.join(config.outputDir, `${prefix}-report.json`);
  const gatewayLogArtifactPath = path.join(config.outputDir, `${prefix}-gateway.log`);
  const gatewayExecutablePath = repoRelative(config.gatewayExe);
  const report = {
    schemaVersion: 1,
    ok: false,
    runId,
    startedAt: new Date(startedAt).toISOString(),
    run: {
      webBaseUrl: config.baseUrl.toString(),
      captureUrl: null,
      requestedBackend: "webgpu",
      bevyLocalMotionRequested: config.bevyLocalMotion,
      bevyPoseCommitRequested: config.bevyPoseCommit,
      interaction: config.interaction,
      route: {
        map: config.map,
        startX: config.x,
        startY: config.y,
        keys: config.interaction === "keyboardSequence" ? config.keys : null,
        keyPressCount: config.interaction === "keyboardSequence" ? config.keyPressCount : null,
        keyIntervalMs: config.interaction === "keyboardSequence" ? config.keyIntervalMs : null,
        clickSequence: config.interaction === "clickSequence" ? config.clickSequence : null,
        clickSequencePostMs:
          config.interaction === "clickSequence" ? config.clickSequencePostMs : null,
        expectedMovementCommandCount: config.expectedMovementCommandCount,
        expectedFinalX: config.expectedFinalX,
        expectedFinalY: config.expectedFinalY,
        maxLocalCommandPoseLatencyMs: config.maxLocalCommandPoseLatencyMs,
        skipStartTransfer: config.skipStartTransfer,
        captureFrameImages: config.captureFrameImages,
        frameCaptureMode: config.captureFrameImages ? config.frameCaptureMode : null,
      },
      gateway: {
        host: LOOPBACK_HOST,
        webPort: null,
        tcpPort: null,
        healthUrl: null,
        executablePath: gatewayExecutablePath,
        executableSha256: null,
        pid: null,
        closedAt: null,
        closedAtMs: null,
        exitCode: null,
        signal: null,
        exitCodeUnsignedHex: null,
        logArtifact: null,
      },
      security: {
        randomIdentityGenerated: true,
        qaControlMode: "token-gated",
        playerCommandSafetyEnforced: true,
        accountStoreBackend: "file",
      },
    },
    gateway: {
      executablePath: gatewayExecutablePath,
      executableSha256: null,
      pid: null,
      closedAt: null,
      closedAtMs: null,
      exitCode: null,
      signal: null,
      exitCodeUnsignedHex: null,
      aliveThroughCapture: null,
      aliveBeforeCleanup: null,
      logArtifact: null,
    },
    artifacts: {
      report: repoRelative(reportPath),
      screenshot: repoRelative(screenshotPath),
      state: repoRelative(statePath),
      gatewayLog: null,
    },
    assertions: {
      captureScriptPresent: false,
      gatewayExecutablePresent: false,
      gatewayHealth200: false,
      captureCompleted: false,
      captureExitedZero: false,
      captureArtifactsPresent: false,
      captureStateParsed: false,
      captureReportOk: false,
      interactionMatchesRequest: false,
      frameCaptureMatchesRequest: false,
      transferUsesQaControlWhenRequested: false,
      gameScreenReached: false,
      safeStartPositionReached: false,
      expectedMovementCommandCountMatches: false,
      expectedFinalPlayerPositionReached: false,
      movementShadowPresent: false,
      presentationPoseBridgePresent: false,
      presentationPoseSamplesPositive: false,
      presentationPoseEntityHitsPositive: false,
      presentationPoseFallbackIsMinority: false,
      presentationPoseSnapshotPresent: false,
      presentationPoseSelfEntryPresent: false,
      presentationPoseOverflowZero: false,
      presentationPoseCommitFlagMatchesRequest: false,
      presentationPoseCommitOwnsFramesWhenRequested: false,
      presentationPoseCommitProvenanceClean: false,
      presentationPoseCommitNodeFramesAtomic: false,
      presentationPoseCommitRequiredSurfacesRegistered: false,
      presentationPoseSelfCameraInvariant: false,
      localMotionShadowPresent: false,
      localMotionCommandObserved: false,
      localMotionComparisonSampled: false,
      localMotionComparisonExact: false,
      localMotionQueuesAndDecodeClean: false,
      localMotionPresentationFlagMatchesRequest: false,
      localMotionPresentationOwnsSelfWhenRequested: false,
      localCommandPoseLatencyCoverage: false,
      localCommandPoseLatencyWithinBudget: false,
      bridgeErrorsZero: false,
      bridgeDroppedZero: false,
      runtimeFixedIntervalMs100: false,
      runtimeProcessedEventCountPositive: false,
      runtimePendingEventDropCountZero: false,
      runtimeCommandMatchCountPositive: false,
      runtimeCommandMismatchCountZero: false,
      runtimePendingCommandCountZero: false,
      runtimePendingCommandDropCountZero: false,
      runtimeAckMatchOrDegradedCountPositive: false,
      runtimeAckMismatchCountZero: false,
      runtimeDecodeErrorCountZero: false,
      backendWebgpu: false,
      backendNoFallback: false,
      criticalConsoleEvidencePresent: false,
      noCriticalConsoleErrors: false,
      network404EvidencePresent: false,
      noNonFaviconNetwork404s: false,
      stateSecretsRedacted: false,
      gatewayAliveThroughCapture: false,
      gatewayPidRecorded: false,
      gatewayExecutableSha256Recorded: false,
      gatewayCloseObserved: false,
      gatewayStopped: false,
      temporaryDirectoryRemoved: false,
      allChildProcessesStopped: false,
      reportSecretsRedacted: false,
    },
    observed: null,
    capture: null,
    cleanup: {
      gatewayStopped: null,
      gatewayAliveBeforeCleanup: null,
      gatewayPid: null,
      gatewayExecutableSha256: null,
      gatewayClosedAt: null,
      gatewayClosedAtMs: null,
      gatewayExitCode: null,
      gatewaySignal: null,
      gatewayExitCodeUnsignedHex: null,
      gatewayLogArtifact: null,
      gatewayLogArtifactSanitized: null,
      gatewayLogPreservationReasons: [],
      temporaryDirectoryRemoved: null,
      activeChildCount: null,
    },
    errors: [],
  };

  let tempDir = null;
  let gateway = null;
  let stateArtifact = null;
  let captureAttempted = false;
  const removeSignalHandlers = installSignalHandlers();

  try {
    await assertFile(CAPTURE_SCRIPT, "capture-web-movement-jitter.mjs");
    report.assertions.captureScriptPresent = true;
    await assertFile(config.gatewayExe, "mir2 gateway executable");
    report.assertions.gatewayExecutablePresent = true;
    const gatewayExecutableSha256 = await sha256File(config.gatewayExe);
    report.run.gateway.executableSha256 = gatewayExecutableSha256;
    report.gateway.executableSha256 = gatewayExecutableSha256;
    report.cleanup.gatewayExecutableSha256 = gatewayExecutableSha256;
    report.assertions.gatewayExecutableSha256Recorded = isSha256(gatewayExecutableSha256);

    const ports = await selectIsolatedPorts({
      gatewayWebPort: config.gatewayWebPort,
      gatewayTcpPort: config.gatewayTcpPort,
      chromeDebugPort: config.chromeDebugPort,
    });
    const healthUrl = `http://${LOOPBACK_HOST}:${ports.gatewayWeb}/health`;
    const captureUrl = buildCaptureUrl(config.baseUrl, ports.gatewayWeb, config);
    report.run.captureUrl = captureUrl.toString();
    Object.assign(report.run.gateway, {
      host: LOOPBACK_HOST,
      webPort: ports.gatewayWeb,
      tcpPort: ports.gatewayTcp,
      healthUrl,
    });

    tempDir = await fs.mkdtemp(path.join(os.tmpdir(), TEMP_DIR_PREFIX));
    const accountStorePath = path.join(tempDir, "accounts.json");
    const saveRecoveryDir = path.join(tempDir, "save-recovery");
    const saveRecoveryMacKey = randomBytes(32).toString("hex");
    runtimeSecrets.add(saveRecoveryMacKey);
    const gatewayLogPath = path.join(tempDir, "gateway.log");

    gateway = await launchGateway({
      executable: config.gatewayExe,
      webPort: ports.gatewayWeb,
      tcpPort: ports.gatewayTcp,
      accountStorePath,
      saveRecoveryDir,
      saveRecoveryMacKey,
      qaControlToken,
      logPath: gatewayLogPath,
      executableSha256: gatewayExecutableSha256,
    });
    updateGatewayLifecycleReport(report, gateway);
    await waitForGatewayHealth(gateway, healthUrl, config.gatewayReadyTimeoutMs);
    report.assertions.gatewayHealth200 = true;

    captureAttempted = true;
    const captureResult = await runCapture({
      captureUrl,
      prefix,
      account,
      password,
      characterName,
      qaControlToken,
      chromeDebugPort: ports.chromeDebug,
      config,
    });
    report.capture = {
      exitCode: captureResult.exitCode,
      signal: captureResult.signal,
      durationMs: captureResult.durationMs,
      timedOut: captureResult.timedOut,
      spawnError: captureResult.spawnError ? safeErrorMessage(captureResult.spawnError) : null,
    };
    report.assertions.captureCompleted =
      captureResult.closed && !captureResult.timedOut && captureResult.spawnError === null;
    report.assertions.captureExitedZero =
      report.assertions.captureCompleted && captureResult.exitCode === 0;
    report.assertions.gatewayAliveThroughCapture = !hasExited(gateway.child);
    report.gateway.aliveThroughCapture = report.assertions.gatewayAliveThroughCapture;

    const screenshotPresent = await isNonEmptyFile(screenshotPath);
    const statePresent = await isNonEmptyFile(statePath);
    report.assertions.captureArtifactsPresent = screenshotPresent && statePresent;

    if (statePresent) {
      stateArtifact = await redactJsonArtifact(statePath);
      report.assertions.captureStateParsed = true;
      report.assertions.stateSecretsRedacted = stateArtifact.secretsRedacted;
      Object.assign(report.assertions, buildStateAssertions(stateArtifact.value, config));
      report.observed = buildObservedState(
        stateArtifact.value,
        stateArtifact.redactionApplied,
        config,
      );
    }

    if (!report.assertions.captureExitedZero) {
      report.errors.push(captureFailureMessage(captureResult));
    }
  } catch (error) {
    report.errors.push(safeErrorMessage(error));
  } finally {
    if (!stateArtifact && (await isNonEmptyFile(statePath))) {
      try {
        stateArtifact = await redactJsonArtifact(statePath);
        report.assertions.captureStateParsed = true;
        report.assertions.stateSecretsRedacted = stateArtifact.secretsRedacted;
        Object.assign(report.assertions, buildStateAssertions(stateArtifact.value, config));
        report.observed = buildObservedState(
          stateArtifact.value,
          stateArtifact.redactionApplied,
          config,
        );
      } catch (error) {
        report.errors.push(`State redaction failed: ${safeErrorMessage(error)}`);
      }
    }

    const captureFailed =
      captureAttempted &&
      (report.capture === null ||
        report.assertions.captureCompleted !== true ||
        report.assertions.captureExitedZero !== true ||
        report.assertions.captureArtifactsPresent !== true ||
        report.assertions.captureStateParsed !== true ||
        report.assertions.captureReportOk !== true);
    const gatewayAliveBeforeCleanup = gateway ? !hasExited(gateway.child) : null;
    report.gateway.aliveBeforeCleanup = gatewayAliveBeforeCleanup;
    report.cleanup.gatewayAliveBeforeCleanup = gatewayAliveBeforeCleanup;
    if (gateway) updateGatewayLifecycleReport(report, gateway);

    if (captureFailed) report.cleanup.gatewayLogPreservationReasons.push("captureFailure");
    if (
      gateway &&
      (gatewayAliveBeforeCleanup === false || report.gateway.aliveThroughCapture === false)
    ) {
      report.cleanup.gatewayLogPreservationReasons.push("gatewayNotAlive");
    }

    try {
      report.cleanup.gatewayStopped = gateway ? await stopGateway(gateway) : true;
    } catch (error) {
      report.cleanup.gatewayStopped = false;
      report.errors.push(`Gateway cleanup failed: ${safeErrorMessage(error)}`);
    } finally {
      if (gateway) updateGatewayLifecycleReport(report, gateway);
    }
    report.assertions.gatewayStopped = report.cleanup.gatewayStopped === true;

    if (gateway && report.cleanup.gatewayLogPreservationReasons.length > 0) {
      try {
        const gatewayLogArtifact = await writeSanitizedGatewayLogArtifact(
          gateway.logPath,
          gatewayLogArtifactPath,
        );
        report.artifacts.gatewayLog = gatewayLogArtifact;
        report.run.gateway.logArtifact = gatewayLogArtifact;
        report.gateway.logArtifact = gatewayLogArtifact;
        report.cleanup.gatewayLogArtifact = gatewayLogArtifact;
        report.cleanup.gatewayLogArtifactSanitized = true;
      } catch (error) {
        report.cleanup.gatewayLogArtifactSanitized = false;
        report.errors.push(`Gateway log artifact failed: ${safeErrorMessage(error)}`);
      }
    }

    if (tempDir) {
      try {
        await removeOwnedTempDir(tempDir, TEMP_DIR_PREFIX);
        report.cleanup.temporaryDirectoryRemoved = !(await pathExists(tempDir));
      } catch (error) {
        report.cleanup.temporaryDirectoryRemoved = false;
        await sanitizeTemporaryDirectory(tempDir).catch(() => undefined);
        report.errors.push(`Temporary-directory cleanup failed: ${safeErrorMessage(error)}`);
      }
    } else {
      report.cleanup.temporaryDirectoryRemoved = true;
    }
    report.assertions.temporaryDirectoryRemoved =
      report.cleanup.temporaryDirectoryRemoved === true;

    report.cleanup.activeChildCount = activeChildren.size;
    report.assertions.allChildProcessesStopped = activeChildren.size === 0;
    removeSignalHandlers();
  }

  report.completedAt = new Date().toISOString();
  report.durationMs = Date.now() - startedAt;

  let safeReport = redactSensitive(report);
  safeReport.assertions.reportSecretsRedacted = reportValueIsRedacted(safeReport);
  safeReport.ok =
    safeReport.errors.length === 0 &&
    Object.values(safeReport.assertions).every((value) => value === true);
  safeReport = redactSensitive(safeReport);
  const serializedReport = `${JSON.stringify(safeReport, null, 2)}\n`;
  if (containsRuntimeSecret(serializedReport)) {
    throw new Error("Refusing to write a smoke report containing runtime credentials.");
  }
  await fs.writeFile(reportPath, serializedReport, "utf8");

  const failedAssertions = Object.entries(safeReport.assertions)
    .filter(([, passed]) => passed !== true)
    .map(([name]) => name);
  console.log(
    JSON.stringify(
      {
        ok: safeReport.ok,
        reportPath,
        screenshotPath: (await isNonEmptyFile(screenshotPath)) ? screenshotPath : null,
        statePath: (await isNonEmptyFile(statePath)) ? statePath : null,
        failedAssertions,
        errors: safeReport.errors,
      },
      null,
      2,
    ),
  );
  process.exitCode = safeReport.ok ? 0 : interruptedSignal ? 130 : 1;
}

function buildConfig(args) {
  const baseUrl = parseLocalWebUrl(args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL);
  const expectedFinalX = optionalNonNegativeIntegerArg(args.expectedFinalX, "expectedFinalX");
  const expectedFinalY = optionalNonNegativeIntegerArg(args.expectedFinalY, "expectedFinalY");
  if ((expectedFinalX === null) !== (expectedFinalY === null)) {
    throw new Error("--expectedFinalX and --expectedFinalY must be provided together.");
  }

  return {
    baseUrl,
    bevyLocalMotion: booleanArg(args.bevyLocalMotion, false, "bevyLocalMotion"),
    bevyPoseCommit: booleanArg(
      args.bevyPoseCommit,
      baseUrl.searchParams.get("bevyPoseCommit") === "1",
      "bevyPoseCommit",
    ),
    interaction: interactionArg(args.interaction),
    skipStartTransfer: booleanArg(args.skipStartTransfer, true, "skipStartTransfer"),
    clickSequence: nonEmptyArg(
      args.clickSequence,
      "1,1,left,0,step1;2,2,left,900,step2;3,3,left,1800,step3;4,4,left,2700,step4",
      "clickSequence",
    ),
    clickSequencePostMs: nonNegativeIntegerArg(
      args.clickSequencePostMs,
      1_800,
      "clickSequencePostMs",
    ),
    expectedMovementCommandCount: optionalNonNegativeIntegerArg(
      args.expectedMovementCommandCount,
      "expectedMovementCommandCount",
    ),
    expectedFinalX,
    expectedFinalY,
    captureFrameImages: booleanArg(args.captureFrameImages, false, "captureFrameImages"),
    frameCaptureMode: nonEmptyArg(args.frameCaptureMode, "window", "frameCaptureMode"),
    frameImageFormat: nonEmptyArg(args.frameImageFormat, "jpeg", "frameImageFormat"),
    frameImageQuality: positiveIntegerArg(args.frameImageQuality, 82, "frameImageQuality"),
    headed: booleanArg(args.headed, false, "headed"),
    windowFrameActivate: booleanArg(args.windowFrameActivate, true, "windowFrameActivate"),
    windowFrameCropMode: nonEmptyArg(args.windowFrameCropMode, "content", "windowFrameCropMode"),
    windowFrameMinimizeTitlePatterns: String(args.windowFrameMinimizeTitlePatterns ?? "").trim(),
    outputDir: path.resolve(args.output ?? DEFAULT_OUTPUT_DIR),
    gatewayExe: path.resolve(
      args.gatewayExe ?? args.gatewayPath ?? process.env.MIR2_GATEWAY_EXE ?? DEFAULT_GATEWAY_EXE,
    ),
    gatewayWebPort: optionalPortArg(args.gatewayWebPort, "gatewayWebPort"),
    gatewayTcpPort: optionalPortArg(args.gatewayTcpPort, "gatewayTcpPort"),
    chromeDebugPort: optionalPortArg(args.chromeDebugPort, "chromeDebugPort"),
    gatewayReadyTimeoutMs: positiveIntegerArg(
      args.gatewayReadyTimeoutMs,
      DEFAULT_GATEWAY_READY_TIMEOUT_MS,
      "gatewayReadyTimeoutMs",
    ),
    captureTimeoutMs: positiveIntegerArg(
      args.captureTimeoutMs,
      DEFAULT_CAPTURE_TIMEOUT_MS,
      "captureTimeoutMs",
    ),
    finalSceneReadyTimeoutMs: positiveIntegerArg(
      args.finalSceneReadyTimeoutMs,
      DEFAULT_FINAL_SCENE_READY_TIMEOUT_MS,
      "finalSceneReadyTimeoutMs",
    ),
    finalRendererReadyTimeoutMs: positiveIntegerArg(
      args.finalRendererReadyTimeoutMs,
      DEFAULT_FINAL_RENDERER_READY_TIMEOUT_MS,
      "finalRendererReadyTimeoutMs",
    ),
    sampleMs: positiveIntegerArg(args.sampleMs, DEFAULT_SAMPLE_MS, "sampleMs"),
    preInteractionDelayMs: nonNegativeIntegerArg(
      args.preInteractionDelayMs,
      DEFAULT_PRE_INTERACTION_DELAY_MS,
      "preInteractionDelayMs",
    ),
    settleMs: nonNegativeIntegerArg(args.settleMs, DEFAULT_SETTLE_MS, "settleMs"),
    map: nonEmptyArg(args.map, DEFAULT_MAP, "map"),
    x: nonNegativeIntegerArg(args.x, DEFAULT_X, "x"),
    y: nonNegativeIntegerArg(args.y, DEFAULT_Y, "y"),
    keys: keyboardSequenceArg(args.keys ?? DEFAULT_KEYS),
    keyPressCount: positiveIntegerArg(
      args.keyPressCount,
      DEFAULT_KEY_PRESS_COUNT,
      "keyPressCount",
    ),
    keyIntervalMs: positiveIntegerArg(
      args.keyIntervalMs,
      DEFAULT_KEY_INTERVAL_MS,
      "keyIntervalMs",
    ),
    maxLocalCommandPoseLatencyMs: positiveIntegerArg(
      args.maxLocalCommandPoseLatencyMs,
      DEFAULT_LOCAL_COMMAND_POSE_LATENCY_BUDGET_MS,
      "maxLocalCommandPoseLatencyMs",
    ),
  };
}

function buildCaptureUrl(baseUrl, gatewayWebPort, config) {
  const url = new URL(baseUrl);
  url.searchParams.set("gatewayWs", `ws://${LOOPBACK_HOST}:${gatewayWebPort}/ws`);
  url.searchParams.set("bevyBackend", "webgpu");
  url.searchParams.set("bevyEntities", "1");
  url.searchParams.set("bevyAtlas", "1");
  if (config.bevyLocalMotion) {
    url.searchParams.set("bevyLocalMotion", "1");
  }
  url.searchParams.set("bevyPoseCommit", config.bevyPoseCommit ? "1" : "0");
  return url;
}

async function selectIsolatedPorts({ gatewayWebPort, gatewayTcpPort, chromeDebugPort }) {
  const requested = [gatewayWebPort, gatewayTcpPort, chromeDebugPort].filter(
    (port) => port !== null,
  );
  if (new Set(requested).size !== requested.length) {
    throw new Error("Gateway web, gateway TCP, and Chrome debug ports must be distinct.");
  }

  const reservations = [];
  try {
    const gatewayWeb = await reserveLoopbackPort(gatewayWebPort, "Gateway web");
    reservations.push(gatewayWeb);
    const gatewayTcp = await reserveLoopbackPort(gatewayTcpPort, "Gateway TCP");
    reservations.push(gatewayTcp);
    const chromeDebug = await reserveLoopbackPort(chromeDebugPort, "Chrome debug");
    reservations.push(chromeDebug);
    return {
      gatewayWeb: gatewayWeb.port,
      gatewayTcp: gatewayTcp.port,
      chromeDebug: chromeDebug.port,
    };
  } finally {
    await Promise.all(reservations.map((reservation) => closeServer(reservation.server)));
  }
}

function reserveLoopbackPort(requestedPort, label) {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    const port = requestedPort ?? 0;
    server.once("error", (error) => {
      if (error?.code === "EADDRINUSE") {
        reject(new Error(`${label} port ${port} is already in use on ${LOOPBACK_HOST}.`));
        return;
      }
      reject(
        new Error(
          `Could not reserve ${label.toLowerCase()} port ${port} on ${LOOPBACK_HOST}: ${error.message}`,
        ),
      );
    });
    server.listen({ host: LOOPBACK_HOST, port, exclusive: true }, () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        server.close();
        reject(new Error(`Could not determine the reserved ${label.toLowerCase()} port.`));
        return;
      }
      resolve({ server, port: address.port });
    });
  });
}

function closeServer(server) {
  return new Promise((resolve) => {
    if (!server.listening) {
      resolve();
      return;
    }
    server.close(() => resolve());
  });
}

async function launchGateway({
  executable,
  executableSha256,
  webPort,
  tcpPort,
  accountStorePath,
  saveRecoveryDir,
  saveRecoveryMacKey,
  qaControlToken,
  logPath,
}) {
  const logHandle = await fs.open(logPath, "a");
  const child = spawn(executable, [], {
    cwd: REPO_ROOT,
    env: {
      ...isolatedChildEnvironment(),
      MIR2_RUNTIME_ENV: "development",
      MIR2_DEPLOYMENT_ENV: "development",
      MIR2_ENV: "development",
      MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES: "0",
      MIR2_GATEWAY_REQUIRE_REDIS_CACHE: "0",
      MIR2_GATEWAY_REDIS_CACHE_URL: "",
      MIR2_GATEWAY_WEB_ADDR: `${LOOPBACK_HOST}:${webPort}`,
      MIR2_GATEWAY_TCP_ADDR: `${LOOPBACK_HOST}:${tcpPort}`,
      MIR2_ACCOUNT_STORE_PATH: accountStorePath,
      MIR2_ACCOUNT_STORE_BACKEND: "file",
      MIR2_SAVE_RECOVERY_MAC_KEY: saveRecoveryMacKey,
      MIR2_SAVE_RECOVERY_DIR: saveRecoveryDir,
      MIR2_GATEWAY_ENFORCE_PLAYER_COMMAND_SAFETY: "1",
      MIR2_GATEWAY_QA_CONTROL_TOKEN: qaControlToken,
    },
    shell: false,
    stdio: ["ignore", logHandle.fd, logHandle.fd],
    windowsHide: true,
  });
  const gateway = {
    child,
    logHandle,
    logPath,
    executablePath: repoRelative(executable),
    executableSha256,
    pid: child.pid ?? null,
    closedAt: null,
    closedAtMs: null,
    exitCode: null,
    signal: null,
    exitCodeUnsignedHex: null,
    spawnError: null,
  };
  activeChildren.add(child);
  child.on("error", (error) => {
    gateway.spawnError = error;
  });
  gateway.closePromise = new Promise((resolve) => {
    child.once("close", (exitCode, signal) => {
      gateway.closedAtMs = Date.now();
      gateway.closedAt = new Date(gateway.closedAtMs).toISOString();
      gateway.exitCode = exitCode;
      gateway.signal = signal ?? null;
      gateway.exitCodeUnsignedHex = unsignedExitCodeHex(exitCode);
      activeChildren.delete(child);
      resolve();
    });
  });

  try {
    await new Promise((resolve, reject) => {
      child.once("spawn", resolve);
      child.once("error", reject);
    });
    return gateway;
  } catch (error) {
    activeChildren.delete(child);
    await logHandle.close().catch(() => undefined);
    throw new Error(`Could not start gateway executable ${executable}: ${error.message}`);
  }
}

async function waitForGatewayHealth(gateway, healthUrl, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastObservation = "no response";
  while (Date.now() < deadline) {
    throwIfInterrupted();
    if (gateway.spawnError) {
      throw new Error(
        `Gateway process error before /health became ready: ${gateway.spawnError.message}`,
      );
    }
    if (hasExited(gateway.child)) {
      const tail = await readGatewayLogTail(gateway.logPath);
      throw new Error(
        `Gateway exited before ${healthUrl} returned 200 (code=${gateway.child.exitCode}, signal=${gateway.child.signalCode ?? "none"}).${tail}`,
      );
    }

    try {
      const response = await fetch(healthUrl, { signal: AbortSignal.timeout(1_000) });
      lastObservation = `HTTP ${response.status}`;
      await response.body?.cancel().catch(() => undefined);
      if (response.status === 200) return;
    } catch (error) {
      lastObservation =
        error?.name === "TimeoutError" ? "request timed out" : safeErrorMessage(error);
    }
    await delay(150);
  }

  const tail = await readGatewayLogTail(gateway.logPath);
  throw new Error(
    `Timed out after ${timeoutMs}ms waiting for ${healthUrl} to return 200; last=${lastObservation}.${tail}`,
  );
}

async function runCapture({
  captureUrl,
  prefix,
  account,
  password,
  characterName,
  qaControlToken,
  chromeDebugPort,
  config,
}) {
  const captureArgs = [
    CAPTURE_SCRIPT,
    "--baseUrl",
    captureUrl.toString(),
    "--output",
    config.outputDir,
    "--prefix",
    prefix,
    "--createAccount",
    "true",
    "--account",
    account,
    "--password",
    password,
    "--characterName",
    characterName,
    "--qaControlToken",
    qaControlToken,
    "--map",
    config.map,
    "--x",
    String(config.x),
    "--y",
    String(config.y),
    "--skipStartTransfer",
    String(config.skipStartTransfer),
    "--interaction",
    config.interaction,
    "--bevyBackend",
    "webgpu",
    "--expectBevyWebGpuRenderer",
    "true",
    "--strictMovementChecks",
    "true",
    "--allowBlockedResidual",
    "false",
    "--sampleMs",
    String(config.sampleMs),
    "--localCommandPoseLatencyMs",
    String(config.maxLocalCommandPoseLatencyMs),
    "--preInteractionDelayMs",
    String(config.preInteractionDelayMs),
    "--settleMs",
    String(config.settleMs),
    "--finalSceneReadyTimeoutMs",
    String(config.finalSceneReadyTimeoutMs),
    "--finalRendererReadyTimeoutMs",
    String(config.finalRendererReadyTimeoutMs),
    "--debugPort",
    String(chromeDebugPort),
  ];
  if (config.interaction === "keyboardSequence") {
    captureArgs.push(
      "--keys",
      config.keys,
      "--clickCount",
      String(config.keyPressCount),
      "--keyIntervalMs",
      String(config.keyIntervalMs),
      "--run",
      "false",
    );
  } else {
    captureArgs.push(
      "--clickSequence",
      config.clickSequence,
      "--clickSequencePostMs",
      String(config.clickSequencePostMs),
    );
  }
  if (config.captureFrameImages) {
    captureArgs.push(
      "--captureFrameImages",
      "true",
      "--frameCaptureMode",
      config.frameCaptureMode,
      "--frameImageFormat",
      config.frameImageFormat,
      "--frameImageQuality",
      String(config.frameImageQuality),
      "--headed",
      String(config.headed),
      "--windowFrameActivate",
      String(config.windowFrameActivate),
      "--windowFrameCropMode",
      config.windowFrameCropMode,
    );
    if (config.windowFrameMinimizeTitlePatterns) {
      captureArgs.push(
        "--windowFrameMinimizeTitlePatterns",
        config.windowFrameMinimizeTitlePatterns,
      );
    }
  }
  const startedAt = Date.now();
  const result = await runChild(process.execPath, captureArgs, config.captureTimeoutMs, {
    ...isolatedChildEnvironment(),
    MIR2_BEVY_BACKEND: "webgpu",
    MIR2_QA_CONTROL_TOKEN: qaControlToken,
  });
  return {
    ...result,
    durationMs: Date.now() - startedAt,
  };
}

function runChild(command, commandArgs, timeoutMs, env) {
  return new Promise((resolve) => {
    const child = spawn(command, commandArgs, {
      cwd: WEB_ROOT,
      env,
      shell: false,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    activeChildren.add(child);
    let stdout = "";
    let stderr = "";
    let spawnError = null;
    let timedOut = false;
    let forceKillTimer = null;

    child.stdout?.on("data", (chunk) => {
      stdout = appendBounded(stdout, chunk.toString());
    });
    child.stderr?.on("data", (chunk) => {
      stderr = appendBounded(stderr, chunk.toString());
    });
    child.once("error", (error) => {
      spawnError = error;
    });

    const timeout = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
      forceKillTimer = setTimeout(() => {
        if (!hasExited(child)) child.kill("SIGKILL");
      }, 3_000);
      forceKillTimer.unref();
    }, timeoutMs);
    timeout.unref();

    child.once("close", (exitCode, signal) => {
      activeChildren.delete(child);
      clearTimeout(timeout);
      if (forceKillTimer) clearTimeout(forceKillTimer);
      resolve({
        closed: true,
        exitCode,
        signal: signal ?? null,
        timedOut,
        spawnError,
        stdout,
        stderr,
      });
    });
  });
}

function isolatedChildEnvironment() {
  const env = { ...process.env };
  delete env.MIR2_SAVE_RECOVERY_MAC_KEY;
  delete env.MIR2_SAVE_RECOVERY_DIR;
  return env;
}

function captureFailureMessage(result) {
  const detail = sanitizeText((result.stderr.trim() || result.stdout.trim()).slice(-8_000));
  if (result.spawnError) {
    return `Could not launch capture-web-movement-jitter.mjs: ${result.spawnError.message}`;
  }
  if (result.timedOut) {
    return `capture-web-movement-jitter.mjs timed out.${detail ? ` ${detail}` : ""}`;
  }
  return `capture-web-movement-jitter.mjs failed (code=${result.exitCode}, signal=${result.signal ?? "none"}).${detail ? ` ${detail}` : ""}`;
}

function buildStateAssertions(state, config) {
  const finalState = state?.finalState;
  const shadow = finalState?.bevyMovementShadow;
  const bridge = shadow?.bridge;
  const runtime = shadow?.runtime;
  const poses = shadow?.poses;
  const localMotion = shadow?.localPresentation;
  const localCommandPoseLatency = state?.localCommandPoseLatency;
  const poseBridge = finalState?.bevyEntityRenderer?.presentationPoseBridge;
  const entityRenderer = finalState?.bevyEntityRenderer;
  const poseBridgeBaseline =
    state?.start?.bevyEntityRenderer?.presentationPoseBridge ??
    state?.samples?.[0]?.bevyEntityRenderer?.presentationPoseBridge ??
    null;
  const bevyRuntime = finalState?.bevyRuntime;
  const criticalConsoleErrors = state?.criticalConsoleErrors;
  const network404s = state?.nonFaviconNetwork404s;
  const requestedBackend = bevyRuntime?.requestedBackend ?? null;
  const selectedBackend = bevyRuntime?.selectedBackend ?? null;
  const compiledBackend = bevyRuntime?.compiledBackend ?? null;
  const poseNodes = Array.isArray(finalState?.bevyPoseNodes) ? finalState.bevyPoseNodes : [];
  const stampedPoseNodeFrames = poseNodes
    .map((node) => node?.frameId)
    .filter((frameId) => typeof frameId === "string" && frameId.length > 0);
  const cameraSurfaceKeys = new Set(
    poseNodes
      .filter((node) => node?.role === "camera")
      .map((node) => node?.key)
      .filter((key) => typeof key === "string"),
  );
  const selfPose =
    typeof runtime?.selfObjectId === "string" && Array.isArray(poses?.entities)
      ? poses.entities.find((entry) => entry?.objectId === runtime.selfObjectId)
      : null;

  return {
    captureReportOk: state?.ok === true,
    interactionMatchesRequest: state?.interaction === config.interaction,
    frameCaptureMatchesRequest:
      !config.captureFrameImages ||
      (state?.captureFrameImages === true &&
        state?.frameCaptureMode === config.frameCaptureMode &&
        isPositiveNumber(state?.frameImageCount) &&
        (config.frameCaptureMode !== "window" ||
          (state?.frameImageCaptureArea?.width === state?.viewport?.width &&
            state?.frameImageCaptureArea?.height <= state?.viewport?.height &&
            state?.frameImageCaptureArea?.height >= state?.viewport?.height - 8)) &&
        Array.isArray(state?.frameImageCaptureErrors) &&
        state.frameImageCaptureErrors.length === 0),
    transferUsesQaControlWhenRequested:
      config.skipStartTransfer ||
      state?.captureControl?.transfer?.mode === "qaControl.transferMap",
    gameScreenReached: finalState?.screen === "game",
    safeStartPositionReached:
      state?.start?.mapFileName === config.map &&
      state?.start?.player?.x === config.x &&
      state?.start?.player?.y === config.y,
    expectedMovementCommandCountMatches:
      config.expectedMovementCommandCount === null ||
      (Array.isArray(state?.movementWebSocketFramesSent) &&
        state.movementWebSocketFramesSent.length === config.expectedMovementCommandCount &&
        runtime?.commandMatchCount === config.expectedMovementCommandCount),
    expectedFinalPlayerPositionReached:
      config.expectedFinalX === null ||
      (finalState?.player?.x === config.expectedFinalX &&
        finalState?.player?.y === config.expectedFinalY),
    movementShadowPresent: Boolean(bridge && runtime),
    presentationPoseBridgePresent:
      poseBridge?.enabled === true && poseBridge?.bevyPoseRequested === true,
    presentationPoseSamplesPositive: isPositiveNumber(poseBridge?.bevyPoseSamples),
    presentationPoseEntityHitsPositive: isPositiveNumber(poseBridge?.entityPoseHits),
    presentationPoseFallbackIsMinority:
      isPositiveNumber(poseBridge?.bevyPoseSamples) &&
      Number(poseBridge?.fallbackSamples ?? 0) < Number(poseBridge?.bevyPoseSamples ?? 0),
    presentationPoseSnapshotPresent:
      poses?.ready === true &&
      poses?.version === 1 &&
      poses?.bridgeEnabled === true &&
      poses?.rendererEnabled === true &&
      Array.isArray(poses?.entities),
    presentationPoseSelfEntryPresent:
      typeof runtime?.selfObjectId === "string" &&
      Array.isArray(poses?.entities) &&
      poses.entities.some((entry) => entry?.objectId === runtime.selfObjectId),
    presentationPoseOverflowZero:
      poses?.frameOverflowCount === 0 && poses?.totalOverflowCount === 0,
    presentationPoseCommitFlagMatchesRequest:
      entityRenderer?.poseCommitRequested === config.bevyPoseCommit &&
      entityRenderer?.poseCommitActive === config.bevyPoseCommit &&
      poseBridge?.poseCommitRequested === config.bevyPoseCommit &&
      poseBridge?.poseCommitActive === config.bevyPoseCommit,
    presentationPoseCommitOwnsFramesWhenRequested:
      !config.bevyPoseCommit ||
      (isPositiveNumber(poseBridge?.poseCommitFrames) &&
        poseBridge?.poseCommitSinkAvailable === true &&
        poseBridge?.poseCommitRegistrationError == null),
    presentationPoseCommitProvenanceClean:
      !config.bevyPoseCommit ||
      (poseBridge?.poseCommitReady === true &&
        poseBridge?.lastProvenanceComparison === "match" &&
        poseBridgeBaseline?.poseCommitReady === true &&
        poseBridge?.provenanceMismatchCount === poseBridgeBaseline?.provenanceMismatchCount &&
        poseBridge?.provenanceUnavailableCount === poseBridgeBaseline?.provenanceUnavailableCount &&
        poseBridge?.stalePoseFrames === poseBridgeBaseline?.stalePoseFrames &&
        poseBridge?.duplicatePoseFrames === poseBridgeBaseline?.duplicatePoseFrames),
    presentationPoseCommitNodeFramesAtomic:
      !config.bevyPoseCommit ||
      (poseNodes.length > 0 &&
        stampedPoseNodeFrames.length === poseNodes.length &&
        new Set(stampedPoseNodeFrames).size === 1 &&
        poseNodes.some((node) => node?.role === "camera") &&
        poseNodes.some((node) => node?.role === "entity")),
    presentationPoseCommitRequiredSurfacesRegistered:
      !config.bevyPoseCommit ||
      ["drops", "sprites", "names", "overlays"].every((key) => cameraSurfaceKeys.has(key)),
    presentationPoseSelfCameraInvariant:
      !config.bevyPoseCommit ||
      (Number.isFinite(poses?.camera?.x) &&
        Number.isFinite(poses?.camera?.y) &&
        Number.isFinite(selfPose?.x) &&
        Number.isFinite(selfPose?.y) &&
        Math.abs(poses.camera.x + selfPose.x) <= 0.001 &&
        Math.abs(poses.camera.y + selfPose.y) <= 0.001),
    localMotionShadowPresent: Boolean(localMotion),
    localMotionCommandObserved: isPositiveNumber(localMotion?.commandEventCount),
    localMotionComparisonSampled: isPositiveNumber(localMotion?.comparisonSampleCount),
    localMotionComparisonExact:
      localMotion?.comparisonMismatchCount === 0 &&
      localMotion?.maxAbsDeltaX === 0 &&
      localMotion?.maxAbsDeltaY === 0,
    localMotionQueuesAndDecodeClean:
      localMotion?.pendingEventDropCount === 0 &&
      localMotion?.pendingCommandDropCount === 0 &&
      localMotion?.decodeErrorCount === 0,
    localMotionPresentationFlagMatchesRequest:
      localMotion?.presentationEnabled === config.bevyLocalMotion &&
      entityRenderer?.localMotionRequested === config.bevyLocalMotion &&
      entityRenderer?.localMotionActive === config.bevyLocalMotion,
    localMotionPresentationOwnsSelfWhenRequested:
      !config.bevyLocalMotion ||
      (poses?.camera?.source === "localCommand" &&
        typeof runtime?.selfObjectId === "string" &&
        poses?.entities?.some(
          (entry) => entry?.objectId === runtime.selfObjectId && entry?.source === "localCommand",
        )),
    localCommandPoseLatencyCoverage:
      !config.bevyLocalMotion ||
      !config.bevyPoseCommit ||
      (localCommandPoseLatency?.eligibleCommandCount > 0 &&
        localCommandPoseLatency?.coverageComplete === true &&
        localCommandPoseLatency?.droppedSinkEventCount === 0),
    localCommandPoseLatencyWithinBudget:
      !config.bevyLocalMotion ||
      !config.bevyPoseCommit ||
      (localCommandPoseLatency?.responsive === true &&
        localCommandPoseLatency?.budgetMs === config.maxLocalCommandPoseLatencyMs &&
        Number(localCommandPoseLatency?.maxCommandToSinkMs) <=
          config.maxLocalCommandPoseLatencyMs),
    bridgeErrorsZero: bridge?.errors === 0,
    bridgeDroppedZero: bridge?.dropped === 0,
    runtimeFixedIntervalMs100: runtime?.fixedIntervalMs === 100,
    runtimeProcessedEventCountPositive: isPositiveNumber(runtime?.processedEventCount),
    runtimePendingEventDropCountZero: runtime?.pendingEventDropCount === 0,
    runtimeCommandMatchCountPositive: isPositiveNumber(runtime?.commandMatchCount),
    runtimeCommandMismatchCountZero: runtime?.commandMismatchCount === 0,
    runtimePendingCommandCountZero: runtime?.pendingCommandCount === 0,
    runtimePendingCommandDropCountZero: runtime?.pendingCommandDropCount === 0,
    runtimeAckMatchOrDegradedCountPositive:
      Number(runtime?.ackMatchCount ?? 0) + Number(runtime?.ackDegradedCount ?? 0) > 0,
    runtimeAckMismatchCountZero: runtime?.ackMismatchCount === 0,
    runtimeDecodeErrorCountZero: runtime?.decodeErrorCount === 0,
    backendWebgpu:
      requestedBackend === "webgpu" &&
      selectedBackend === "webgpu" &&
      compiledBackend === "webgpu",
    backendNoFallback: bevyRuntime?.fallbackFrom == null,
    criticalConsoleEvidencePresent: Array.isArray(criticalConsoleErrors),
    noCriticalConsoleErrors:
      Array.isArray(criticalConsoleErrors) && criticalConsoleErrors.length === 0,
    network404EvidencePresent: Array.isArray(network404s),
    noNonFaviconNetwork404s: Array.isArray(network404s) && network404s.length === 0,
  };
}

function buildObservedState(state, redactionApplied, config) {
  const finalState = state?.finalState ?? null;
  const shadow = finalState?.bevyMovementShadow ?? null;
  const bevyRuntime = finalState?.bevyRuntime ?? null;
  return {
    captureOk: state?.ok ?? null,
    interaction: state?.interaction ?? null,
    frameCapture: {
      requested: config.captureFrameImages,
      mode: state?.frameCaptureMode ?? null,
      imageFormat: state?.frameImageFormat ?? null,
      imageCount: state?.frameImageCount ?? null,
      imageDirectory: state?.frameImageDir ?? null,
      captureArea: state?.frameImageCaptureArea ?? null,
      errorCount: Array.isArray(state?.frameImageCaptureErrors)
        ? state.frameImageCaptureErrors.length
        : null,
    },
    captureControl: state?.captureControl ?? null,
    screen: finalState?.screen ?? null,
    start: {
      map: state?.start?.mapFileName ?? null,
      x: state?.start?.player?.x ?? null,
      y: state?.start?.player?.y ?? null,
    },
    finalPlayer: {
      map: finalState?.mapFileName ?? null,
      x: finalState?.player?.x ?? null,
      y: finalState?.player?.y ?? null,
    },
    expectedMovement: {
      commandCount: config.expectedMovementCommandCount,
      finalX: config.expectedFinalX,
      finalY: config.expectedFinalY,
      observedCommandCount: Array.isArray(state?.movementWebSocketFramesSent)
        ? state.movementWebSocketFramesSent.length
        : null,
    },
    backend: {
      requested: bevyRuntime?.requestedBackend ?? null,
      selected: bevyRuntime?.selectedBackend ?? null,
      compiled: bevyRuntime?.compiledBackend ?? null,
      fallbackFrom: bevyRuntime?.fallbackFrom ?? null,
    },
    movementShadow: shadow,
    localCommandPoseLatency: state?.localCommandPoseLatency ?? null,
    criticalConsoleErrorCount: Array.isArray(state?.criticalConsoleErrors)
      ? state.criticalConsoleErrors.length
      : null,
    nonFaviconNetwork404Count: Array.isArray(state?.nonFaviconNetwork404s)
      ? state.nonFaviconNetwork404s.length
      : null,
    captureStateRedactionApplied: redactionApplied,
  };
}

function updateGatewayLifecycleReport(report, gateway) {
  const exitCode = gateway.exitCode ?? gateway.child.exitCode ?? null;
  const signal = gateway.signal ?? gateway.child.signalCode ?? null;
  const lifecycle = {
    executablePath: gateway.executablePath,
    executableSha256: gateway.executableSha256,
    pid: gateway.pid,
    closedAt: gateway.closedAt,
    closedAtMs: gateway.closedAtMs,
    exitCode,
    signal,
    exitCodeUnsignedHex: gateway.exitCodeUnsignedHex ?? unsignedExitCodeHex(exitCode),
    logArtifact: report.artifacts.gatewayLog,
  };
  Object.assign(report.run.gateway, lifecycle);
  Object.assign(report.gateway, lifecycle);
  Object.assign(report.cleanup, {
    gatewayPid: lifecycle.pid,
    gatewayExecutableSha256: lifecycle.executableSha256,
    gatewayClosedAt: lifecycle.closedAt,
    gatewayClosedAtMs: lifecycle.closedAtMs,
    gatewayExitCode: lifecycle.exitCode,
    gatewaySignal: lifecycle.signal,
    gatewayExitCodeUnsignedHex: lifecycle.exitCodeUnsignedHex,
    gatewayLogArtifact: lifecycle.logArtifact,
  });
  report.assertions.gatewayPidRecorded = Number.isInteger(lifecycle.pid) && lifecycle.pid > 0;
  report.assertions.gatewayExecutableSha256Recorded = isSha256(lifecycle.executableSha256);
  report.assertions.gatewayCloseObserved =
    typeof lifecycle.closedAt === "string" && Number.isInteger(lifecycle.closedAtMs);
  return lifecycle;
}

async function stopGateway(gateway) {
  try {
    await terminateChild(gateway.child, 5_000);
    if (!(await settledWithin(gateway.closePromise, 5_000))) {
      throw new Error(`Gateway process ${gateway.pid ?? "unknown"} did not emit close.`);
    }
  } finally {
    activeChildren.delete(gateway.child);
    await gateway.logHandle.close();
  }
  return hasExited(gateway.child) && gateway.closedAt !== null;
}

async function terminateChild(child, timeoutMs) {
  if (hasExited(child)) return;
  const exited = new Promise((resolve) => child.once("exit", resolve));
  child.kill("SIGTERM");
  if (await settledWithin(exited, timeoutMs)) return;
  child.kill("SIGKILL");
  if (!(await settledWithin(exited, timeoutMs))) {
    throw new Error(`Process ${child.pid ?? "unknown"} did not exit after termination.`);
  }
}

function installSignalHandlers() {
  const handlers = new Map();
  for (const signal of ["SIGINT", "SIGTERM"]) {
    const handler = () => {
      interruptedSignal ??= signal;
      for (const child of activeChildren) {
        if (!hasExited(child)) child.kill("SIGTERM");
      }
    };
    handlers.set(signal, handler);
    process.on(signal, handler);
  }
  return () => {
    for (const [signal, handler] of handlers) process.off(signal, handler);
  };
}

function throwIfInterrupted() {
  if (interruptedSignal) throw new Error(`Smoke interrupted by ${interruptedSignal}.`);
}

async function readGatewayLogTail(logPath) {
  try {
    const content = sanitizeText(await fs.readFile(logPath, "utf8"));
    const lines = content.trim().split(/\r?\n/).slice(-20).join("\n");
    return lines ? ` Gateway log tail:\n${lines}` : "";
  } catch {
    return "";
  }
}

async function writeSanitizedGatewayLogArtifact(logPath, artifactPath) {
  if (path.basename(logPath).toLowerCase() !== "gateway.log") {
    throw new Error(`Refusing to preserve an unexpected gateway log path: ${logPath}`);
  }
  const sanitized = sanitizeText(await fs.readFile(logPath, "utf8"));
  if (containsRuntimeSecret(sanitized)) {
    throw new Error("Refusing to write a gateway log artifact containing runtime credentials.");
  }
  await fs.writeFile(artifactPath, sanitized, "utf8");
  return repoRelative(artifactPath);
}

async function redactJsonArtifact(filePath) {
  const raw = (await fs.readFile(filePath, "utf8")).replace(/^\uFEFF/, "");
  let value;
  try {
    value = JSON.parse(raw);
  } catch (error) {
    const sanitized = sanitizeText(raw);
    if (sanitized !== raw) await fs.writeFile(filePath, sanitized, "utf8");
    throw new Error(`Could not parse capture state ${filePath}: ${error.message}`);
  }

  const safeValue = redactSensitive(value);
  const safeText = `${JSON.stringify(safeValue, null, 2)}\n`;
  await fs.writeFile(filePath, safeText, "utf8");
  return {
    value: safeValue,
    redactionApplied: safeText.trim() !== raw.trim(),
    secretsRedacted:
      !containsRuntimeSecret(safeText) && sensitiveFieldsAreRedacted(safeValue),
  };
}

async function sanitizeTemporaryDirectory(directory) {
  let entries;
  try {
    entries = await fs.readdir(directory, { withFileTypes: true });
  } catch {
    return;
  }
  await Promise.all(
    entries.map(async (entry) => {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        await sanitizeTemporaryDirectory(entryPath);
        return;
      }
      if (!entry.isFile()) return;
      try {
        const content = await fs.readFile(entryPath, "utf8");
        const sanitized = sanitizeText(content);
        if (sanitized !== content) await fs.writeFile(entryPath, sanitized, "utf8");
      } catch {
        // Best-effort hygiene after a failed removal; cleanup remains failed.
      }
    }),
  );
}

async function removeOwnedTempDir(directory, expectedPrefix) {
  const resolved = path.resolve(directory);
  const tempRoot = path.resolve(os.tmpdir());
  const relative = path.relative(tempRoot, resolved);
  const isInsideTemp = relative && !relative.startsWith("..") && !path.isAbsolute(relative);
  if (!isInsideTemp || !path.basename(resolved).startsWith(expectedPrefix)) {
    throw new Error(`Refusing to remove unowned temporary directory: ${resolved}`);
  }
  await fs.rm(resolved, { recursive: true, force: true });
}

function redactSensitive(value) {
  if (typeof value === "string") return sanitizeText(value);
  if (Array.isArray(value)) return value.map(redactSensitive);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, nested]) => [
      key,
      isSensitiveKey(key) ? REDACTED : redactSensitive(nested),
    ]),
  );
}

function isSensitiveKey(key) {
  const normalized = String(key).replace(/[^a-z0-9]/gi, "").toLowerCase();
  return (
    [
      "account",
      "accountid",
      "username",
      "password",
      "passkey",
      "token",
      "qacontroltoken",
      "accesstoken",
      "refreshtoken",
      "secret",
      "clientsecret",
    ].includes(normalized) ||
    normalized.endsWith("accountid") ||
    normalized.endsWith("password") ||
    normalized.endsWith("passkey") ||
    normalized.endsWith("token") ||
    normalized.endsWith("secret")
  );
}

function sensitiveFieldsAreRedacted(value) {
  if (Array.isArray(value)) return value.every(sensitiveFieldsAreRedacted);
  if (!value || typeof value !== "object") return true;
  return Object.entries(value).every(([key, nested]) =>
    isSensitiveKey(key) ? nested === REDACTED : sensitiveFieldsAreRedacted(nested),
  );
}

function reportValueIsRedacted(value) {
  const serialized = JSON.stringify(value);
  return !containsRuntimeSecret(serialized) && sensitiveFieldsAreRedacted(value);
}

function sanitizeText(value) {
  let sanitized = String(value ?? "");
  for (const secret of runtimeSecrets) {
    if (!secret) continue;
    sanitized = sanitized.split(secret).join(REDACTED);
    const encoded = encodeURIComponent(secret);
    if (encoded !== secret) sanitized = sanitized.split(encoded).join(REDACTED);
  }
  return sanitized
    .replace(/(--(?:account|password|qaControlToken)(?:=|\s+))\S+/gi, `$1${REDACTED}`)
    .replace(/(MIR2_(?:GATEWAY_)?QA_CONTROL_TOKEN\s*[=:]\s*)\S+/gi, `$1${REDACTED}`)
    .replace(
      /(\b(?:authorization|proxy-authorization)\s*[=:]\s*(?:bearer|basic)\s+)\S+/gi,
      `$1${REDACTED}`,
    )
    .replace(
      /([?&](?:account|accountId|account_id|username|password|passkey|qaControlToken|token|access_token|refresh_token|secret)=)[^&#\s]+/gi,
      `$1${REDACTED}`,
    )
    .replace(
      /("(?:account|accountId|account_id|username|password|passkey|qaControlToken|token|secret)"\s*:\s*")[^"]*/gi,
      `$1${REDACTED}`,
    );
}

function containsRuntimeSecret(value) {
  return [...runtimeSecrets].some((secret) => secret && String(value).includes(secret));
}

function safeErrorMessage(error) {
  return sanitizeText(error instanceof Error ? error.message : String(error));
}

function appendBounded(current, next) {
  const combined = current + next;
  return combined.length > MAX_CHILD_OUTPUT_CHARS
    ? combined.slice(-MAX_CHILD_OUTPUT_CHARS)
    : combined;
}

function hasExited(child) {
  return child.exitCode !== null || child.signalCode !== null;
}

function unsignedExitCodeHex(exitCode) {
  if (!Number.isInteger(exitCode)) return null;
  const unsigned = BigInt.asUintN(32, BigInt(exitCode));
  return `0x${unsigned.toString(16).toUpperCase().padStart(8, "0")}`;
}

function settledWithin(promise, timeoutMs) {
  return Promise.race([
    promise.then(() => true),
    delay(timeoutMs).then(() => false),
  ]);
}

async function assertFile(filePath, label) {
  try {
    const stats = await fs.stat(filePath);
    if (!stats.isFile()) throw new Error("not a file");
  } catch (error) {
    throw new Error(`${label} was not found at ${filePath}: ${error.message}`);
  }
}

async function isNonEmptyFile(filePath) {
  try {
    return (await fs.stat(filePath)).size > 0;
  } catch {
    return false;
  }
}

async function pathExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function sha256File(filePath) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(filePath)) hash.update(chunk);
  return hash.digest("hex");
}

function isSha256(value) {
  return typeof value === "string" && /^[a-f0-9]{64}$/i.test(value);
}

function parseLocalWebUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`--baseUrl must be an absolute URL; received ${JSON.stringify(value)}.`);
  }
  const localHosts = new Set(["127.0.0.1", "localhost", "[::1]"]);
  if (!['http:', 'https:'].includes(url.protocol) || !localHosts.has(url.hostname.toLowerCase())) {
    throw new Error("--baseUrl must use http(s) on localhost, 127.0.0.1, or ::1.");
  }
  if (url.username || url.password) {
    throw new Error("--baseUrl must not contain credentials.");
  }
  return url;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--") continue;
    if (!arg.startsWith("--")) {
      throw new Error(`Unexpected positional argument ${JSON.stringify(arg)}.`);
    }
    const separator = arg.indexOf("=");
    if (separator > 2) {
      parsed[arg.slice(2, separator)] = arg.slice(separator + 1);
      continue;
    }
    const key = arg.slice(2);
    const next = argv[index + 1];
    parsed[key] = next === undefined || next.startsWith("--") ? "true" : argv[++index];
  }
  return parsed;
}

function validateKnownArgs(args) {
  const unknown = Object.keys(args).filter((key) => !KNOWN_ARGS.has(key));
  if (unknown.length > 0) {
    throw new Error(`Unknown argument(s): ${unknown.map((key) => `--${key}`).join(", ")}.`);
  }
}

function booleanArg(value, fallback, label) {
  if (value === undefined || value === null || value === "") return fallback;
  const normalized = String(value).trim().toLowerCase();
  if (["1", "true", "yes", "on"].includes(normalized)) return true;
  if (["0", "false", "no", "off"].includes(normalized)) return false;
  throw new Error(`--${label} must be true or false; received ${JSON.stringify(value)}.`);
}

function optionalPortArg(value, label) {
  if (value === undefined || value === null || value === "") return null;
  const port = Number(value);
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    throw new Error(
      `--${label} must be an integer from 1 to 65535; received ${JSON.stringify(value)}.`,
    );
  }
  return port;
}

function positiveIntegerArg(value, fallback, label) {
  const number = value === undefined || value === null || value === "" ? fallback : Number(value);
  if (!Number.isInteger(number) || number <= 0) {
    throw new Error(`--${label} must be a positive integer; received ${JSON.stringify(value)}.`);
  }
  return number;
}

function nonNegativeIntegerArg(value, fallback, label) {
  const number = value === undefined || value === null || value === "" ? fallback : Number(value);
  if (!Number.isInteger(number) || number < 0) {
    throw new Error(
      `--${label} must be a non-negative integer; received ${JSON.stringify(value)}.`,
    );
  }
  return number;
}

function optionalNonNegativeIntegerArg(value, label) {
  if (value === undefined || value === null || value === "") return null;
  const number = Number(value);
  if (!Number.isInteger(number) || number < 0) {
    throw new Error(
      `--${label} must be a non-negative integer; received ${JSON.stringify(value)}.`,
    );
  }
  return number;
}

function nonEmptyArg(value, fallback, label) {
  const normalized = String(value ?? fallback).trim();
  if (!normalized) throw new Error(`--${label} must not be empty.`);
  return normalized;
}

function interactionArg(value) {
  const normalized = String(value ?? "keyboardSequence").trim();
  if (normalized === "keyboardSequence" || normalized === "clickSequence") {
    return normalized;
  }
  throw new Error(
    `--interaction must be keyboardSequence or clickSequence; received ${JSON.stringify(value)}.`,
  );
}

function keyboardSequenceArg(value) {
  const normalized = String(value)
    .split(",")
    .map((key) => key.trim().toLowerCase())
    .filter(Boolean);
  const supported = new Set([
    "w",
    "a",
    "s",
    "d",
    "arrowup",
    "arrowdown",
    "arrowleft",
    "arrowright",
  ]);
  if (normalized.length === 0 || normalized.some((key) => !supported.has(key))) {
    throw new Error(
      "--keys must be a comma-separated keyboard movement sequence using WASD or arrow keys.",
    );
  }
  return normalized.join(",");
}

function isPositiveNumber(value) {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

function repoRelative(filePath) {
  return path.relative(REPO_ROOT, filePath).replaceAll("\\", "/");
}

function timestamp() {
  return new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function printHelp() {
  console.log(`Usage: node apps/web/scripts/smoke-bevy-movement-shadow.mjs [options]

Runs a short WebGPU movement capture against a separately served Player Web.

Options:
  --baseUrl <url>                    Local Player Web URL (default: ${DEFAULT_BASE_URL})
  --bevyLocalMotion <bool>           Enable local Bevy self/camera takeover (default: false)
  --bevyPoseCommit <bool>            Require synchronous provenance-gated pose sink (default: false)
  --interaction <mode>               keyboardSequence or clickSequence (default: keyboardSequence)
  --clickSequence <route>            dx,dy,button,atMs,label entries separated by semicolons
  --expectedMovementCommandCount <n> Require an exact movement command count
  --expectedFinalX <tile>             Require an exact final player X coordinate
  --expectedFinalY <tile>             Require an exact final player Y coordinate
  --skipStartTransfer <bool>         Keep natural spawn instead of QA transfer (default: true)
  --captureFrameImages <bool>        Save temporal frame images (default: false)
  --frameCaptureMode <mode>          screenshot, canvas, or headed window (default: window)
  --output <dir>                     Artifact directory (default: ${DEFAULT_OUTPUT_DIR})
  --gatewayExe <path>                Gateway executable (default: ${DEFAULT_GATEWAY_EXE})
  --gatewayWebPort <port>            Isolated loopback web port (default: auto-select)
  --gatewayTcpPort <port>            Isolated loopback TCP port (default: auto-select)
  --chromeDebugPort <port>           Isolated Chrome CDP port (default: auto-select)
  --map <name> --x <tile> --y <tile> Expected fresh-character start (default: ${DEFAULT_MAP} @ ${DEFAULT_X},${DEFAULT_Y})
  --keys <sequence>                  Safe comma-separated route (default: ${DEFAULT_KEYS})
  --keyPressCount <count>            Number of keyboard presses (default: ${DEFAULT_KEY_PRESS_COUNT})
  --keyIntervalMs <ms>               Keyboard press cadence (default: ${DEFAULT_KEY_INTERVAL_MS})
  --settleMs <ms>                    Post-route settle window (default: ${DEFAULT_SETTLE_MS})
  --maxLocalCommandPoseLatencyMs <n> Maximum command-to-accepted-sink delay (default: ${DEFAULT_LOCAL_COMMAND_POSE_LATENCY_BUDGET_MS})
  --gatewayReadyTimeoutMs <ms>       Gateway /health timeout (default: ${DEFAULT_GATEWAY_READY_TIMEOUT_MS})
  --captureTimeoutMs <ms>            Capture process timeout (default: ${DEFAULT_CAPTURE_TIMEOUT_MS})`);
}
