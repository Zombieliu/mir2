import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const LOOPBACK_HOST = "127.0.0.1";
const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const CAPTURE_SCRIPT = path.join(SCRIPT_DIR, "capture-crystal-parity.mjs");
const DEFAULT_GATEWAY_EXE = path.join(REPO_ROOT, "target", "debug", "mir2-gateway.exe");
const DEFAULT_BASE_URL = "http://127.0.0.1:3002";
const DEFAULT_OUTPUT_DIR = path.join(
  REPO_ROOT,
  "docs",
  "generated",
  "player-qa",
  "bevy-map-standalone",
);
const DEFAULT_MAP = "0";
const DEFAULT_X = 324;
const DEFAULT_Y = 41;
const DEFAULT_BACKEND = "webgpu";
const DEFAULT_GATEWAY_READY_TIMEOUT_MS = 30_000;
const DEFAULT_CAPTURE_TIMEOUT_MS = 180_000;
const DEFAULT_VISUAL_READY_TIMEOUT_MS = 45_000;
const DEFAULT_SETTLE_MS = 5_000;
const MAX_CHILD_OUTPUT_CHARS = 512 * 1024;
const VALID_BACKENDS = new Set(["webgpu", "webgl2"]);
const KNOWN_ARGS = new Set([
  "allowFallback",
  "baseUrl",
  "bevyBackend",
  "captureTimeoutMs",
  "gatewayExe",
  "gatewayPath",
  "gatewayReadyTimeoutMs",
  "gatewayTcpPort",
  "gatewayWebPort",
  "help",
  "keepGatewayLogOnFailure",
  "map",
  "settleMs",
  "visualReadyTimeoutMs",
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
  await fs.mkdir(DEFAULT_OUTPUT_DIR, { recursive: true });

  const account = randomUUID();
  const identityKey = account.replaceAll("-", "");
  const qaControlToken = randomUUID();
  const password = `Mir2${identityKey.slice(0, 12)}A1`;
  const characterName = `M${identityKey.slice(16, 27)}`;
  runtimeSecrets.add(qaControlToken);
  runtimeSecrets.add(password);

  const startedAt = Date.now();
  const runId = `${timestamp()}-${identityKey.slice(-8)}`;
  const prefix = `bevy-map-standalone-${config.bevyBackend}-${runId}`;
  const screenshotPath = path.join(DEFAULT_OUTPUT_DIR, `${prefix}.png`);
  const statePath = path.join(DEFAULT_OUTPUT_DIR, `${prefix}-state.json`);
  const reportPath = path.join(DEFAULT_OUTPUT_DIR, `${prefix}-report.json`);
  const report = {
    schemaVersion: 1,
    ok: false,
    runId,
    startedAt: new Date(startedAt).toISOString(),
    run: {
      webBaseUrl: config.baseUrl.toString(),
      requestedBackend: config.bevyBackend,
      allowFallback: config.allowFallback,
      target: { map: config.map, x: config.x, y: config.y },
      gateway: {
        host: LOOPBACK_HOST,
        webPort: null,
        tcpPort: null,
        healthUrl: null,
      },
      security: {
        qaControlMode: "token-gated",
        playerCommandSafetyEnforced: true,
        accountStoreBackend: "file",
      },
    },
    artifacts: {
      screenshot: repoRelative(screenshotPath),
      state: repoRelative(statePath),
      report: repoRelative(reportPath),
      gatewayLog: null,
    },
    assertions: {
      gatewayHealth200: false,
      captureCompleted: false,
      captureArtifactsPresent: false,
    },
    observed: null,
    capture: null,
    cleanup: {
      gatewayStopped: null,
      temporaryAccountDirectoryRemoved: null,
      gatewayLogDisposition: null,
    },
    errors: [],
  };

  let accountTempDir = null;
  let gatewayLogDir = null;
  let gateway = null;
  const removeSignalHandlers = installSignalHandlers();

  try {
    await assertFile(CAPTURE_SCRIPT, "capture-crystal-parity.mjs");
    await assertFile(config.gatewayExe, "mir2 gateway executable");
    const ports = await selectIsolatedGatewayPorts(config.gatewayWebPort, config.gatewayTcpPort);
    const healthUrl = `http://${LOOPBACK_HOST}:${ports.web}/health`;
    const captureUrl = buildCaptureUrl(config.baseUrl, ports.web, config.bevyBackend);
    report.run.captureUrl = captureUrl.toString();
    report.run.gateway = {
      host: LOOPBACK_HOST,
      webPort: ports.web,
      tcpPort: ports.tcp,
      healthUrl,
    };

    accountTempDir = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-bevy-map-standalone-account-"));
    gatewayLogDir = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-bevy-map-standalone-log-"));
    const accountStorePath = path.join(accountTempDir, "accounts.json");
    const gatewayLogPath = path.join(gatewayLogDir, "gateway.log");

    gateway = await launchGateway({
      executable: config.gatewayExe,
      webPort: ports.web,
      tcpPort: ports.tcp,
      accountStorePath,
      qaControlToken,
      logPath: gatewayLogPath,
    });
    await waitForGatewayHealth(gateway, healthUrl, config.gatewayReadyTimeoutMs);
    report.assertions.gatewayHealth200 = true;

    const captureResult = await runCapture({
      captureUrl,
      prefix,
      account,
      password,
      characterName,
      qaControlToken,
      config,
    });
    report.capture = {
      exitCode: captureResult.exitCode,
      durationMs: captureResult.durationMs,
    };
    report.assertions.captureCompleted = true;

    const rawState = (await fs.readFile(statePath, "utf8")).replace(/^\uFEFF/, "");
    let state = JSON.parse(rawState);
    const credentialLeakFree = !containsRuntimeSecret(rawState);
    if (!credentialLeakFree) {
      state = redactSensitive(state);
      await fs.writeFile(statePath, `${JSON.stringify(state, null, 2)}\n`, "utf8");
    }

    const screenshotPresent = await isNonEmptyFile(screenshotPath);
    const statePresent = await isNonEmptyFile(statePath);
    report.assertions.captureArtifactsPresent = screenshotPresent && statePresent;
    Object.assign(
      report.assertions,
      buildStateAssertions(state, config, credentialLeakFree),
    );
    report.observed = buildObservedState(state);
  } catch (error) {
    report.errors.push(safeErrorMessage(error));
  } finally {
    try {
      report.cleanup.gatewayStopped = gateway ? await stopGateway(gateway) : true;
    } catch (error) {
      report.cleanup.gatewayStopped = false;
      report.errors.push(`Gateway cleanup failed: ${safeErrorMessage(error)}`);
    }
    report.assertions.gatewayStopped = report.cleanup.gatewayStopped === true;

    if (accountTempDir) {
      try {
        await removeOwnedTempDir(accountTempDir, "mir2-bevy-map-standalone-account-");
        report.cleanup.temporaryAccountDirectoryRemoved = true;
      } catch (error) {
        report.cleanup.temporaryAccountDirectoryRemoved = false;
        report.errors.push(`Account-store cleanup failed: ${safeErrorMessage(error)}`);
      }
    } else {
      report.cleanup.temporaryAccountDirectoryRemoved = true;
    }
    report.assertions.temporaryAccountDirectoryRemoved =
      report.cleanup.temporaryAccountDirectoryRemoved === true;

    const failedBeforeLogCleanup =
      report.errors.length > 0 || Object.values(report.assertions).some((value) => value !== true);
    if (gatewayLogDir) {
      const gatewayLogPath = path.join(gatewayLogDir, "gateway.log");
      const retainGatewayLog = failedBeforeLogCleanup && config.keepGatewayLogOnFailure;
      try {
        if (retainGatewayLog) {
          await sanitizeTextFile(gatewayLogPath);
          report.artifacts.gatewayLog = path.resolve(gatewayLogPath);
          report.cleanup.gatewayLogDisposition = "retained-on-failure";
        } else {
          await removeOwnedTempDir(gatewayLogDir, "mir2-bevy-map-standalone-log-");
          report.cleanup.gatewayLogDisposition = "removed";
        }
      } catch (error) {
        report.cleanup.gatewayLogDisposition = "cleanup-failed";
        report.errors.push(`Gateway-log cleanup failed: ${safeErrorMessage(error)}`);
      }
    } else {
      report.cleanup.gatewayLogDisposition = "not-created";
    }
    report.assertions.gatewayLogHandled = ["removed", "retained-on-failure", "not-created"].includes(
      report.cleanup.gatewayLogDisposition,
    );

  }

  report.completedAt = new Date().toISOString();
  report.durationMs = Date.now() - startedAt;
  report.ok = report.errors.length === 0 && Object.values(report.assertions).every((value) => value === true);
  const safeReport = redactSensitive(report);
  try {
    await fs.writeFile(reportPath, `${JSON.stringify(safeReport, null, 2)}\n`, "utf8");
  } finally {
    removeSignalHandlers();
  }

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
        gatewayLogPath: safeReport.artifacts.gatewayLog,
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
  const bevyBackend = String(args.bevyBackend ?? DEFAULT_BACKEND).trim().toLowerCase();
  if (!VALID_BACKENDS.has(bevyBackend)) {
    throw new Error(`--bevyBackend must be webgpu or webgl2; received ${JSON.stringify(bevyBackend)}.`);
  }

  const gatewayExe = path.resolve(
    args.gatewayExe ?? args.gatewayPath ?? process.env.MIR2_GATEWAY_EXE ?? DEFAULT_GATEWAY_EXE,
  );
  return {
    baseUrl,
    bevyBackend,
    allowFallback: booleanArg(args.allowFallback, false, "allowFallback"),
    gatewayExe,
    gatewayWebPort: optionalPortArg(args.gatewayWebPort, "gatewayWebPort"),
    gatewayTcpPort: optionalPortArg(args.gatewayTcpPort, "gatewayTcpPort"),
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
    visualReadyTimeoutMs: positiveIntegerArg(
      args.visualReadyTimeoutMs,
      DEFAULT_VISUAL_READY_TIMEOUT_MS,
      "visualReadyTimeoutMs",
    ),
    settleMs: nonNegativeIntegerArg(args.settleMs, DEFAULT_SETTLE_MS, "settleMs"),
    keepGatewayLogOnFailure: booleanArg(
      args.keepGatewayLogOnFailure,
      false,
      "keepGatewayLogOnFailure",
    ),
    map: nonEmptyArg(args.map, DEFAULT_MAP, "map"),
    x: nonNegativeIntegerArg(args.x, DEFAULT_X, "x"),
    y: nonNegativeIntegerArg(args.y, DEFAULT_Y, "y"),
  };
}

function buildCaptureUrl(baseUrl, gatewayWebPort, bevyBackend) {
  const url = new URL(baseUrl);
  url.searchParams.set("gatewayWs", `ws://${LOOPBACK_HOST}:${gatewayWebPort}/ws`);
  url.searchParams.set("bevyBackend", bevyBackend);
  url.searchParams.set("bevyEntities", "1");
  url.searchParams.set("bevyAtlas", "1");
  return url;
}

async function selectIsolatedGatewayPorts(requestedWebPort, requestedTcpPort) {
  if (requestedWebPort !== null && requestedWebPort === requestedTcpPort) {
    throw new Error(
      `Gateway web and TCP ports must differ; both were set to ${requestedWebPort} on ${LOOPBACK_HOST}.`,
    );
  }

  const reservations = [];
  try {
    const web = await reserveLoopbackPort(requestedWebPort, "Gateway web");
    reservations.push(web);
    const tcp = await reserveLoopbackPort(requestedTcpPort, "Gateway TCP");
    reservations.push(tcp);
    return { web: web.port, tcp: tcp.port };
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
        reject(
          new Error(
            `${label} port ${port} is already in use on ${LOOPBACK_HOST}; choose a different isolated port.`,
          ),
        );
        return;
      }
      reject(new Error(`Could not reserve ${label.toLowerCase()} port ${port} on ${LOOPBACK_HOST}: ${error.message}`));
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
  webPort,
  tcpPort,
  accountStorePath,
  qaControlToken,
  logPath,
}) {
  const logHandle = await fs.open(logPath, "a");
  const child = spawn(executable, [], {
    cwd: REPO_ROOT,
    env: {
      ...process.env,
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
      MIR2_GATEWAY_ENFORCE_PLAYER_COMMAND_SAFETY: "1",
      MIR2_GATEWAY_QA_CONTROL_TOKEN: qaControlToken,
    },
    shell: false,
    stdio: ["ignore", logHandle.fd, logHandle.fd],
    windowsHide: true,
  });
  const gateway = { child, logHandle, logPath, spawnError: null };
  activeChildren.add(child);
  child.on("error", (error) => {
    gateway.spawnError = error;
  });
  child.once("close", () => activeChildren.delete(child));

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
      throw new Error(`Gateway process error before /health became ready: ${gateway.spawnError.message}`);
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
      lastObservation = error?.name === "TimeoutError" ? "request timed out" : safeErrorMessage(error);
    }
    await delay(150);
  }

  const tail = await readGatewayLogTail(gateway.logPath);
  throw new Error(`Timed out after ${timeoutMs}ms waiting for ${healthUrl} to return 200; last=${lastObservation}.${tail}`);
}

async function runCapture({
  captureUrl,
  prefix,
  account,
  password,
  characterName,
  qaControlToken,
  config,
}) {
  const captureArgs = [
    CAPTURE_SCRIPT,
    "--baseUrl",
    captureUrl.toString(),
    "--output",
    DEFAULT_OUTPUT_DIR,
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
    "--bevyBackend",
    config.bevyBackend,
    "--bevyEntities",
    "1",
    "--bevyAtlas",
    "1",
    "--settleMs",
    String(config.settleMs),
    "--visualReadyTimeoutMs",
    String(config.visualReadyTimeoutMs),
  ];
  const startedAt = Date.now();
  const result = await runChild(process.execPath, captureArgs, config.captureTimeoutMs);
  return {
    exitCode: result.exitCode,
    durationMs: Date.now() - startedAt,
  };
}

function runChild(command, commandArgs, timeoutMs) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, commandArgs, {
      cwd: WEB_ROOT,
      env: process.env,
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
      const detail = sanitizeText((stderr.trim() || stdout.trim()).slice(-8_000));
      if (spawnError) {
        reject(new Error(`Could not launch capture-crystal-parity.mjs: ${spawnError.message}`));
        return;
      }
      if (timedOut) {
        reject(new Error(`capture-crystal-parity.mjs timed out after ${timeoutMs}ms.${detail ? ` ${detail}` : ""}`));
        return;
      }
      if (exitCode !== 0) {
        reject(
          new Error(
            `capture-crystal-parity.mjs failed (code=${exitCode}, signal=${signal ?? "none"}).${detail ? ` ${detail}` : ""}`,
          ),
        );
        return;
      }
      resolve({ exitCode, stdout, stderr });
    });
  });
}

function buildStateAssertions(state, config, credentialLeakFree) {
  const mapRenderer = state?.bevyMapRenderer;
  const runtime = state?.bevyRuntime;
  const originalMap404s = collectOriginalMap404s(state);
  const selectedBackend = runtime?.selectedBackend ?? null;
  const compiledBackend = runtime?.compiledBackend ?? null;
  const selectedBackendAccepted =
    VALID_BACKENDS.has(selectedBackend) &&
    (config.allowFallback || selectedBackend === config.bevyBackend);
  const compiledBackendAccepted =
    VALID_BACKENDS.has(compiledBackend) &&
    (config.allowFallback || compiledBackend === config.bevyBackend);

  return {
    captureCredentialLeakFree: credentialLeakFree,
    gameScreenReached: state?.screen === "game",
    targetMapReached:
      state?.mapFileName === config.map &&
      state?.player?.x === config.x &&
      state?.player?.y === config.y,
    transferUsedQaControl: state?.captureControl?.transfer?.mode === "qaControl.transferMap",
    bevyMapRendererPresent: Boolean(mapRenderer),
    standaloneTileCountPositive: isPositiveNumber(mapRenderer?.standaloneTileCount),
    additiveStandaloneTileCountPositive: isPositiveNumber(
      mapRenderer?.standaloneAdditiveTileCount,
    ),
    standaloneImageSourceCountPositive: isPositiveNumber(mapRenderer?.standaloneImageSourceCount),
    standaloneDecodedImageCountPositive: isPositiveNumber(mapRenderer?.standaloneDecodedImageCount),
    standaloneFailedImageCountZero: mapRenderer?.standaloneFailedImageCount === 0,
    atlasIncludesStandaloneImages:
      isFiniteNumber(mapRenderer?.atlasImageCount) &&
      isFiniteNumber(mapRenderer?.atlasPageCount) &&
      mapRenderer.atlasImageCount > mapRenderer.atlasPageCount,
    domOnlyContainsAdditiveFallback:
      isFiniteNumber(mapRenderer?.domSpriteCount) &&
      isFiniteNumber(mapRenderer?.domBlendSpriteCount) &&
      mapRenderer.domSpriteCount === mapRenderer.domBlendSpriteCount,
    additiveDomFallbackCleared:
      mapRenderer?.domSpriteCount === 0 && mapRenderer?.domBlendSpriteCount === 0,
    originalMap404EvidencePresent: Array.isArray(state?.nonFaviconNetwork404s),
    noOriginalMap404s: originalMap404s.length === 0,
    requestedBackendRecorded: runtime?.requestedBackend === config.bevyBackend,
    selectedBackendMatchesRequest: selectedBackendAccepted,
    compiledBackendMatchesRequest: compiledBackendAccepted,
    selectedBackendMatchesCompiled: selectedBackend === compiledBackend,
  };
}

function buildObservedState(state) {
  const mapRenderer = state?.bevyMapRenderer ?? null;
  const runtime = state?.bevyRuntime ?? null;
  return {
    screen: state?.screen ?? null,
    target: {
      map: state?.mapFileName ?? null,
      x: state?.player?.x ?? null,
      y: state?.player?.y ?? null,
    },
    transferMode: state?.captureControl?.transfer?.mode ?? null,
    standaloneTileCount: mapRenderer?.standaloneTileCount ?? null,
    standaloneAdditiveTileCount: mapRenderer?.standaloneAdditiveTileCount ?? null,
    standaloneImageSourceCount: mapRenderer?.standaloneImageSourceCount ?? null,
    standaloneDecodedImageCount: mapRenderer?.standaloneDecodedImageCount ?? null,
    standaloneFailedImageCount: mapRenderer?.standaloneFailedImageCount ?? null,
    atlasPageCount: mapRenderer?.atlasPageCount ?? null,
    atlasImageCount: mapRenderer?.atlasImageCount ?? null,
    domSpriteCount: mapRenderer?.domSpriteCount ?? null,
    domBlendSpriteCount: mapRenderer?.domBlendSpriteCount ?? null,
    originalMap404s: collectOriginalMap404s(state),
    requestedBackend: runtime?.requestedBackend ?? null,
    selectedBackend: runtime?.selectedBackend ?? null,
    compiledBackend: runtime?.compiledBackend ?? null,
    fallbackFrom: runtime?.fallbackFrom ?? null,
  };
}

function collectOriginalMap404s(state) {
  if (!Array.isArray(state?.nonFaviconNetwork404s)) return [];
  return state.nonFaviconNetwork404s
    .map((url) => String(url))
    .filter((url) => /\/original-map(?:\/|$)/i.test(url));
}

async function stopGateway(gateway) {
  try {
    await terminateChild(gateway.child, 5_000);
  } finally {
    activeChildren.delete(gateway.child);
    await gateway.logHandle.close();
  }
  return true;
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

async function sanitizeTextFile(filePath) {
  const content = await fs.readFile(filePath, "utf8");
  const sanitized = sanitizeText(content);
  if (sanitized !== content) await fs.writeFile(filePath, sanitized, "utf8");
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
      /(?:password|passkey|secret|token)/i.test(key) ? "[redacted]" : redactSensitive(nested),
    ]),
  );
}

function sanitizeText(value) {
  let sanitized = String(value ?? "");
  for (const secret of runtimeSecrets) {
    if (secret) sanitized = sanitized.split(secret).join("[redacted]");
  }
  return sanitized
    .replace(/(--qaControlToken(?:=|\s+))\S+/gi, "$1[redacted]")
    .replace(/(--password(?:=|\s+))\S+/gi, "$1[redacted]")
    .replace(/(MIR2_GATEWAY_QA_CONTROL_TOKEN\s*[=:]\s*)\S+/gi, "$1[redacted]")
    .replace(/("(?:qaControlToken|token|password)"\s*:\s*")[^"]*/gi, "$1[redacted]");
}

function containsRuntimeSecret(value) {
  return [...runtimeSecrets].some((secret) => secret && value.includes(secret));
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

function parseLocalWebUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`--baseUrl must be an absolute URL; received ${JSON.stringify(value)}.`);
  }
  const localHosts = new Set(["127.0.0.1", "localhost", "[::1]"]);
  if (!["http:", "https:"].includes(url.protocol) || !localHosts.has(url.hostname.toLowerCase())) {
    throw new Error("--baseUrl must use http(s) on localhost, 127.0.0.1, or ::1 for the isolated gateway override.");
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
    if (!arg.startsWith("--")) throw new Error(`Unexpected positional argument ${JSON.stringify(arg)}.`);
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
  if (unknown.length > 0) throw new Error(`Unknown argument(s): ${unknown.map((key) => `--${key}`).join(", ")}.`);
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
    throw new Error(`--${label} must be an integer from 1 to 65535; received ${JSON.stringify(value)}.`);
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
    throw new Error(`--${label} must be a non-negative integer; received ${JSON.stringify(value)}.`);
  }
  return number;
}

function nonEmptyArg(value, fallback, label) {
  const normalized = String(value ?? fallback).trim();
  if (!normalized) throw new Error(`--${label} must not be empty.`);
  return normalized;
}

function isFiniteNumber(value) {
  return typeof value === "number" && Number.isFinite(value);
}

function isPositiveNumber(value) {
  return isFiniteNumber(value) && value > 0;
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
  console.log(`Usage: npm run smoke:bevy-map-standalone -- [options]

Options:
  --baseUrl <url>                    Local Web URL (default: ${DEFAULT_BASE_URL})
  --gatewayExe <path>                Gateway executable (default: ${DEFAULT_GATEWAY_EXE})
  --gatewayWebPort <port>            Isolated loopback web port (default: auto-select)
  --gatewayTcpPort <port>            Isolated loopback TCP port (default: auto-select)
  --map <name> --x <tile> --y <tile> Target scene (default: ${DEFAULT_MAP} @ ${DEFAULT_X},${DEFAULT_Y})
  --bevyBackend <backend>            webgpu or webgl2 (default: ${DEFAULT_BACKEND})
  --allowFallback <bool>             Permit selected/compiled backend fallback (default: false)
  --keepGatewayLogOnFailure <bool>   Retain a sanitized gateway log on failure (default: false)
  --gatewayReadyTimeoutMs <ms>       Gateway /health timeout (default: ${DEFAULT_GATEWAY_READY_TIMEOUT_MS})
  --captureTimeoutMs <ms>            Capture process timeout (default: ${DEFAULT_CAPTURE_TIMEOUT_MS})
  --visualReadyTimeoutMs <ms>        Browser visual-ready timeout (default: ${DEFAULT_VISUAL_READY_TIMEOUT_MS})
  --settleMs <ms>                    Post-ready capture settle time (default: ${DEFAULT_SETTLE_MS})`);
}
