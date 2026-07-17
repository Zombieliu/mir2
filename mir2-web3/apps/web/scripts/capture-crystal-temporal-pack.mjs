import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const DEFAULT_SCENARIO_PATH = path.join(SCRIPT_DIR, "scenarios", "bichon-332275-left4.json");
const PHASE_NAMES = ["native", "web", "report"];
const REDACTED = "[redacted]";
const MAX_CHILD_OUTPUT_CHARS = 1024 * 1024;
const MAX_TIMEOUT_MS = 30 * 60 * 1000;

const PHASE_SCRIPTS = {
  native: path.join(SCRIPT_DIR, "capture-original-computer-use.mjs"),
  web: path.join(SCRIPT_DIR, "capture-web-movement-jitter.mjs"),
  report: path.join(SCRIPT_DIR, "report-movement-temporal-parity.mjs"),
};

const CLI_ARGS = new Set(["dryRun", "help", "native", "output", "phases", "report", "scenario", "web"]);
const ROOT_KEYS = new Set(["description", "id", "outputDir", "phases", "schemaVersion"]);
const PHASE_KEYS = new Set(["args", "enabled", "timeoutMs"]);
const RESERVED_ARGS = {
  native: new Set(["output", "outputDir", "prefix"]),
  web: new Set(["output", "prefix"]),
  report: new Set(["output", "prefix"]),
};

const NATIVE_ARGS = new Set([
  "button",
  "captureMode",
  "captureMs",
  "clickIntervalMs",
  "clickRoute",
  "clickX",
  "clickY",
  "clicks",
  "computerUseClientModule",
  "frameCaptureMode",
  "frameImageFormat",
  "frameImageQuality",
  "label",
  "powershellWindowTitlePattern",
  "route",
  "routePostMs",
  "sampleMs",
  "settleAfterClickMs",
  "warmupMs",
  "window",
  "windowTitlePattern",
  "windowTitleWildcard",
  "x",
  "y",
]);

const WEB_ARGS = new Set([
  "account",
  "allowBlockedResidual",
  "avoidEntityHits",
  "backend",
  "baseUrl",
  "bevyBackend",
  "button",
  "canvasOnlyScreenshot",
  "captureFrameImages",
  "captureMs",
  "cdpCommandTimeoutMs",
  "characterName",
  "chromeHostResolverRules",
  "clickCount",
  "clickHoldMs",
  "clickIntervalMs",
  "clickRoute",
  "clickSequence",
  "clickSequenceDurationMs",
  "clickSequenceLabel",
  "clickSequencePostMs",
  "clickTargetDurationMs",
  "createAccount",
  "debugPort",
  "deviceScaleFactor",
  "direction",
  "directionLagMs",
  "disableGpu",
  "disableQuic",
  "expectBevyWebGl2Renderer",
  "expectBevyWebGpuRenderer",
  "expectCorrectionCount",
  "expectDegradedRunCount",
  "expectFinalDelta",
  "expectMounted",
  "expectRawWebGl2Renderer",
  "failOnInteractionPollution",
  "finalRendererReadyTimeoutMs",
  "finalSceneReadyTimeoutMs",
  "fixedSpriteX",
  "fixedSpriteY",
  "frameCaptureMode",
  "frameImageFormat",
  "frameImageQuality",
  "gameScreenTimeoutMs",
  "headed",
  "height",
  "holdButton",
  "holdMs",
  "initialRendererReadyTimeoutMs",
  "initialSceneReadyTimeoutMs",
  "interaction",
  "key",
  "keyInterval",
  "keyIntervalMs",
  "keyPressMs",
  "keys",
  "localCommandPoseLatencyMs",
  "map",
  "maxCameraOffsetHoldMs",
  "maxDirectionQueueLength",
  "mobile",
  "mobileDirection",
  "mobileMode",
  "mode",
  "mountItem",
  "mountRequiredLevel",
  "mouseHoldMs",
  "movementAckLatencyMs",
  "packetSequence",
  "password",
  "preHoldMs",
  "preInputDelayMs",
  "preInteractionDelayMs",
  "qaControlToken",
  "route",
  "routePattern",
  "routePostMs",
  "routeStepMs",
  "run",
  "sampleMs",
  "secondTargetDx",
  "secondTargetDy",
  "sequence",
  "settleMs",
  "shift",
  "skipStartTransfer",
  "skipTransfer",
  "slowCommandQueueMs",
  "stalePredictedMs",
  "stepWaitMs",
  "strictMovementChecks",
  "suppressTutorial",
  "target2Dx",
  "target2Dy",
  "targetDurationMs",
  "targetDx",
  "targetDy",
  "viewportHeight",
  "viewportWidth",
  "width",
  "windowFrameActivate",
  "windowFrameCropHeight",
  "windowFrameCropLeft",
  "windowFrameCropMode",
  "windowFrameCropTop",
  "windowFrameCropWidth",
  "windowFrameMinimizeTitlePatterns",
  "windowFrameRestoreMinimized",
  "windowFrameTitlePattern",
  "x",
  "y",
]);

const REPORT_ARGS = new Set([
  "alignActions",
  "analyzeFrames",
  "emitPairedFrameDiffs",
  "frameDiffChangedRatioThreshold",
  "frameDiffMeanThreshold",
  "frameDiffPixelThreshold",
  "frameDiffWidth",
  "original",
  "pairedFrameMaxDeltaMs",
  "pairedFrameMaxOutputBytes",
  "pairedFrameMaxPairs",
  "pairedFrameWidth",
  "postActionMs",
  "preActionMs",
  "web",
  "webBichon",
]);

const BOOLEAN_ARGS = {
  native: new Set(),
  web: new Set([
    "allowBlockedResidual",
    "avoidEntityHits",
    "canvasOnlyScreenshot",
    "captureFrameImages",
    "createAccount",
    "disableGpu",
    "disableQuic",
    "expectBevyWebGl2Renderer",
    "expectBevyWebGpuRenderer",
    "expectMounted",
    "expectRawWebGl2Renderer",
    "failOnInteractionPollution",
    "headed",
    "mobile",
    "run",
    "shift",
    "skipStartTransfer",
    "skipTransfer",
    "strictMovementChecks",
    "suppressTutorial",
    "windowFrameActivate",
    "windowFrameRestoreMinimized",
  ]),
  report: new Set(["alignActions", "analyzeFrames", "emitPairedFrameDiffs"]),
};

const NUMBER_ARGS = {
  native: new Set([
    "captureMs",
    "clickIntervalMs",
    "clickX",
    "clickY",
    "frameImageQuality",
    "routePostMs",
    "sampleMs",
    "settleAfterClickMs",
    "warmupMs",
    "x",
    "y",
  ]),
  web: new Set([
    "captureMs",
    "cdpCommandTimeoutMs",
    "clickCount",
    "clickHoldMs",
    "clickIntervalMs",
    "clickSequenceDurationMs",
    "clickSequencePostMs",
    "clickTargetDurationMs",
    "debugPort",
    "deviceScaleFactor",
    "directionLagMs",
    "expectCorrectionCount",
    "expectDegradedRunCount",
    "finalRendererReadyTimeoutMs",
    "finalSceneReadyTimeoutMs",
    "fixedSpriteX",
    "fixedSpriteY",
    "frameImageQuality",
    "gameScreenTimeoutMs",
    "height",
    "holdMs",
    "initialRendererReadyTimeoutMs",
    "initialSceneReadyTimeoutMs",
    "keyInterval",
    "keyIntervalMs",
    "keyPressMs",
    "localCommandPoseLatencyMs",
    "maxCameraOffsetHoldMs",
    "maxDirectionQueueLength",
    "mountRequiredLevel",
    "mouseHoldMs",
    "movementAckLatencyMs",
    "preHoldMs",
    "preInputDelayMs",
    "preInteractionDelayMs",
    "routePostMs",
    "routeStepMs",
    "sampleMs",
    "secondTargetDx",
    "secondTargetDy",
    "settleMs",
    "slowCommandQueueMs",
    "stalePredictedMs",
    "stepWaitMs",
    "target2Dx",
    "target2Dy",
    "targetDurationMs",
    "targetDx",
    "targetDy",
    "viewportHeight",
    "viewportWidth",
    "width",
    "windowFrameCropHeight",
    "windowFrameCropLeft",
    "windowFrameCropTop",
    "windowFrameCropWidth",
    "x",
    "y",
  ]),
  report: new Set([
    "frameDiffChangedRatioThreshold",
    "frameDiffMeanThreshold",
    "frameDiffPixelThreshold",
    "frameDiffWidth",
    "pairedFrameMaxDeltaMs",
    "pairedFrameMaxOutputBytes",
    "pairedFrameMaxPairs",
    "pairedFrameWidth",
    "postActionMs",
    "preActionMs",
  ]),
};

const ALLOWED_ARGS = { native: NATIVE_ARGS, web: WEB_ARGS, report: REPORT_ARGS };
const SIGNED_NUMBER_ARGS = new Set([
  "secondTargetDx",
  "secondTargetDy",
  "target2Dx",
  "target2Dy",
  "targetDx",
  "targetDy",
]);

export async function loadTemporalPackScenario(scenarioPath = DEFAULT_SCENARIO_PATH) {
  const absolutePath = path.resolve(scenarioPath);
  let raw;
  try {
    raw = await fs.readFile(absolutePath, "utf8");
  } catch (error) {
    throw new Error(`Could not read scenario ${absolutePath}: ${error.message}`);
  }
  let scenario;
  try {
    scenario = JSON.parse(raw.replace(/^\uFEFF/, ""));
  } catch (error) {
    throw new Error(`Scenario is not valid JSON: ${error.message}`);
  }
  validateTemporalPackScenario(scenario);
  return { path: absolutePath, scenario };
}

export function validateTemporalPackScenario(scenario) {
  assertRecord(scenario, "scenario");
  assertKnownKeys(scenario, ROOT_KEYS, "scenario");
  if (scenario.schemaVersion !== 1) {
    throw new Error(`scenario.schemaVersion must be 1; received ${JSON.stringify(scenario.schemaVersion)}.`);
  }
  if (typeof scenario.id !== "string" || !/^[a-z0-9](?:[a-z0-9-]{0,78}[a-z0-9])?$/.test(scenario.id)) {
    throw new Error("scenario.id must be 1-80 lowercase alphanumeric/hyphen characters.");
  }
  if (scenario.description !== undefined && (typeof scenario.description !== "string" || !scenario.description.trim())) {
    throw new Error("scenario.description must be a non-empty string when present.");
  }
  if (typeof scenario.outputDir !== "string" || !scenario.outputDir.trim() || scenario.outputDir.includes("\0")) {
    throw new Error("scenario.outputDir must be a non-empty path string.");
  }
  assertRecord(scenario.phases, "scenario.phases");
  assertKnownKeys(scenario.phases, new Set(PHASE_NAMES), "scenario.phases");
  for (const phaseName of PHASE_NAMES) {
    if (!(phaseName in scenario.phases)) {
      throw new Error(`scenario.phases.${phaseName} is required.`);
    }
    validatePhase(phaseName, scenario.phases[phaseName]);
  }
  return scenario;
}

export function buildTemporalPackPlan({
  scenario,
  scenarioPath = DEFAULT_SCENARIO_PATH,
  outputDir,
  dryRun = false,
  phaseOverrides = {},
} = {}) {
  validateTemporalPackScenario(scenario);
  validatePhaseOverrides(phaseOverrides);
  const resolvedOutputDir = resolveRepoPath(outputDir ?? scenario.outputDir);
  const manifestPath = path.join(resolvedOutputDir, "manifest.json");
  const enabled = Object.fromEntries(
    PHASE_NAMES.map((name) => [name, phaseOverrides[name] ?? scenario.phases[name].enabled]),
  );
  const prefixes = Object.fromEntries(PHASE_NAMES.map((name) => [name, `${scenario.id}-${name}`]));
  const artifacts = {
    native: { jsonPath: path.join(resolvedOutputDir, `${prefixes.native}.json`) },
    web: {
      jsonPath: path.join(resolvedOutputDir, `${prefixes.web}.json`),
      screenshotPath: path.join(resolvedOutputDir, `${prefixes.web}.png`),
    },
    report: {
      jsonPath: path.join(resolvedOutputDir, `${prefixes.report}.json`),
      markdownPath: path.join(resolvedOutputDir, `${prefixes.report}.md`),
    },
  };

  const phasePlans = {};
  for (const name of PHASE_NAMES) {
    const config = scenario.phases[name];
    const args = { ...config.args };
    if (name === "report" && enabled.report) {
      args.original = resolveReportInput("original", args.original, enabled.native, artifacts.native.jsonPath);
      args.web = resolveReportInput("web", args.web, enabled.web, artifacts.web.jsonPath);
      if (args.webBichon) args.webBichon = resolveRepoPath(args.webBichon);
    }
    if (name === "native" && args.computerUseClientModule) {
      args.computerUseClientModule = resolveRepoPath(args.computerUseClientModule);
    }
    args.output = resolvedOutputDir;
    args.prefix = prefixes[name];
    const argv = [PHASE_SCRIPTS[name], ...argsToArgv(args)];
    phasePlans[name] = {
      name,
      enabled: enabled[name],
      timeoutMs: config.timeoutMs ?? 180_000,
      scriptPath: PHASE_SCRIPTS[name],
      args,
      argv,
      artifacts: artifacts[name],
    };
  }

  const secretValues = collectSecrets(scenario);
  collectEnvironmentSecrets(secretValues);
  const manifest = {
    schemaVersion: 1,
    kind: "mir2-crystal-temporal-pack",
    ok: dryRun,
    status: dryRun ? "dry-run" : "planned",
    dryRun,
    generatedAt: dryRun ? null : new Date().toISOString(),
    completedAt: null,
    scenario: {
      path: displayPath(path.resolve(scenarioPath)),
      config: redactValue(scenario, secretValues),
    },
    outputDir: displayPath(resolvedOutputDir),
    manifestPath: displayPath(manifestPath),
    phaseOrder: [...PHASE_NAMES],
    redaction: {
      placeholder: REDACTED,
      argv: true,
      scenario: true,
    },
    phases: Object.fromEntries(
      PHASE_NAMES.map((name) => {
        const phase = phasePlans[name];
        return [
          name,
          {
            enabled: phase.enabled,
            status: phase.enabled ? (dryRun ? "planned" : "pending") : "skipped",
            timeoutMs: phase.timeoutMs,
            script: displayPath(phase.scriptPath),
            command: {
              executable: nodeExecutable(),
              argv: redactArgv(phase.argv, secretValues),
            },
            artifacts: redactValue(mapDisplayPaths(phase.artifacts), secretValues),
            startedAt: null,
            completedAt: null,
            durationMs: null,
            result: null,
            error: null,
          },
        ];
      }),
    ),
  };
  assertRedacted(manifest, secretValues);
  return { dryRun, outputDir: resolvedOutputDir, manifestPath, phasePlans, secretValues, manifest };
}

export async function runTemporalPack(options = {}) {
  const loaded = options.scenario
    ? { path: path.resolve(options.scenarioPath ?? DEFAULT_SCENARIO_PATH), scenario: options.scenario }
    : await loadTemporalPackScenario(options.scenarioPath ?? DEFAULT_SCENARIO_PATH);
  const plan = buildTemporalPackPlan({
    scenario: loaded.scenario,
    scenarioPath: loaded.path,
    outputDir: options.outputDir,
    dryRun: options.dryRun === true,
    phaseOverrides: options.phaseOverrides,
  });
  await validateExternalInputs(plan);
  await fs.mkdir(plan.outputDir, { recursive: true });
  await writeManifest(plan);
  if (plan.dryRun) return summarizeRun(plan);

  plan.manifest.status = "running";
  plan.manifest.ok = false;
  await writeManifest(plan);
  for (const phaseName of PHASE_NAMES) {
    const phasePlan = plan.phasePlans[phaseName];
    if (!phasePlan.enabled) continue;
    const phaseManifest = plan.manifest.phases[phaseName];
    const startedAtMs = Date.now();
    phaseManifest.status = "running";
    phaseManifest.startedAt = new Date(startedAtMs).toISOString();
    await writeManifest(plan);
    try {
      const result =
        phaseName === "native" && typeof options.nativeCapture === "function"
          ? await runNativeCapture(phasePlan, options.nativeCapture)
          : await runJsonCommand(phasePlan, plan.secretValues);
      await verifyPhaseArtifacts(phaseName, phasePlan, result, startedAtMs);
      phaseManifest.status = "passed";
      phaseManifest.result = redactValue(result, plan.secretValues);
    } catch (error) {
      phaseManifest.status = "failed";
      phaseManifest.error = safeErrorMessage(error, plan.secretValues);
      phaseManifest.completedAt = new Date().toISOString();
      phaseManifest.durationMs = Date.now() - startedAtMs;
      plan.manifest.status = "failed";
      plan.manifest.completedAt = phaseManifest.completedAt;
      await writeManifest(plan);
      throw new Error(
        `Temporal pack phase ${phaseName} failed: ${phaseManifest.error}. Manifest: ${plan.manifestPath}`,
      );
    }
    phaseManifest.completedAt = new Date().toISOString();
    phaseManifest.durationMs = Date.now() - startedAtMs;
    await writeManifest(plan);
  }

  plan.manifest.ok = true;
  plan.manifest.status = "passed";
  plan.manifest.completedAt = new Date().toISOString();
  await writeManifest(plan);
  return summarizeRun(plan);
}

async function runNativeCapture(phase, nativeCapture) {
  const result = await withTimeout(
    Promise.resolve(
      nativeCapture({
        ...phase.args,
        outputDir: path.dirname(phase.artifacts.jsonPath),
        prefix: path.basename(phase.artifacts.jsonPath, path.extname(phase.artifacts.jsonPath)),
      }),
    ),
    phase.timeoutMs,
    `${path.basename(phase.scriptPath)} timed out after ${phase.timeoutMs}ms`,
  );
  if (!result || typeof result !== "object" || Array.isArray(result) || result.ok !== true) {
    throw new Error(`${path.basename(phase.scriptPath)} in-process runner reported ok=${JSON.stringify(result?.ok)}.`);
  }
  return { ok: true, jsonPath: result.jsonPath };
}

function withTimeout(promise, timeoutMs, message) {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(message)), timeoutMs);
    }),
  ]).finally(() => clearTimeout(timer));
}

export function redactArgv(argv, secretValues = new Set()) {
  const redacted = [];
  for (let index = 0; index < argv.length; index += 1) {
    const value = String(argv[index]);
    if (!value.startsWith("--")) {
      redacted.push(sanitizeText(value, secretValues));
      continue;
    }
    const separator = value.indexOf("=");
    const key = value.slice(2, separator > 2 ? separator : undefined);
    if (separator > 2) {
      redacted.push(isSensitiveKey(key) ? `--${key}=${REDACTED}` : sanitizeText(value, secretValues));
      continue;
    }
    redacted.push(value);
    if (index + 1 < argv.length) {
      const next = String(argv[++index]);
      redacted.push(isSensitiveKey(key) ? REDACTED : sanitizeText(next, secretValues));
    }
  }
  return redacted;
}

function validatePhase(name, phase) {
  assertRecord(phase, `scenario.phases.${name}`);
  assertKnownKeys(phase, PHASE_KEYS, `scenario.phases.${name}`);
  if (typeof phase.enabled !== "boolean") {
    throw new Error(`scenario.phases.${name}.enabled must be boolean.`);
  }
  if (phase.timeoutMs !== undefined && !isPositiveInteger(phase.timeoutMs, MAX_TIMEOUT_MS)) {
    throw new Error(`scenario.phases.${name}.timeoutMs must be an integer from 1 to ${MAX_TIMEOUT_MS}.`);
  }
  assertRecord(phase.args, `scenario.phases.${name}.args`);
  for (const [key, value] of Object.entries(phase.args)) {
    if (!ALLOWED_ARGS[name].has(key)) {
      throw new Error(`Unknown ${name} phase argument --${key}.`);
    }
    if (RESERVED_ARGS[name].has(key)) {
      throw new Error(`${name} phase argument --${key} is managed by the orchestrator.`);
    }
    validateArgValue(name, key, value);
  }
  validatePhaseSemantics(name, phase.args);
}

function validateArgValue(phase, key, value) {
  if (BOOLEAN_ARGS[phase].has(key)) {
    if (typeof value !== "boolean") throw new Error(`${phase} phase argument --${key} must be boolean.`);
    return;
  }
  if (NUMBER_ARGS[phase].has(key)) {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      throw new Error(`${phase} phase argument --${key} must be a finite number.`);
    }
    if (value < 0 && !SIGNED_NUMBER_ARGS.has(key)) {
      throw new Error(`${phase} phase argument --${key} must not be negative.`);
    }
    return;
  }
  if (typeof value !== "string" || !value.trim() || value.includes("\0") || value.startsWith("--")) {
    throw new Error(`${phase} phase argument --${key} must be a non-empty safe string.`);
  }
}

function validatePhaseSemantics(name, args) {
  if (name === "native") {
    if (args.button !== undefined && !["left", "right"].includes(args.button)) {
      throw new Error("native phase --button must be left or right.");
    }
    if (args.frameCaptureMode !== undefined && !["computerUse", "powershell"].includes(args.frameCaptureMode)) {
      throw new Error("native phase --frameCaptureMode must be computerUse or powershell.");
    }
    if (args.route !== undefined) validateNativeRoute(args.route);
  }
  if (name === "web") {
    if (args.baseUrl !== undefined) validateUrl(args.baseUrl, "web phase --baseUrl");
    if (args.bevyBackend !== undefined && !["auto", "default", "webgl2", "webgpu"].includes(args.bevyBackend)) {
      throw new Error("web phase --bevyBackend must be auto, default, webgl2, or webgpu.");
    }
    const interactions = new Set([
      "blockedTarget",
      "clickSequence",
      "clickTarget",
      "direct",
      "hold",
      "holdThenSpamClickTarget",
      "keyboard",
      "keyboardSequence",
      "mobileJoystick",
      "packetRun",
      "packetSequence",
      "packetWalk",
      "routeSpamObstacle",
      "spamClickTarget",
    ]);
    if (args.interaction !== undefined && !interactions.has(args.interaction)) {
      throw new Error(`Unsupported web phase interaction ${JSON.stringify(args.interaction)}.`);
    }
    if (args.clickSequence !== undefined) validateWebClickSequence(args.clickSequence);
  }
  const quality = args.frameImageQuality;
  if (quality !== undefined && (!Number.isInteger(quality) || quality < 1 || quality > 100)) {
    throw new Error(`${name} phase --frameImageQuality must be an integer from 1 to 100.`);
  }
  if (args.frameImageFormat !== undefined && !["jpeg", "jpg", "png"].includes(args.frameImageFormat)) {
    throw new Error(`${name} phase --frameImageFormat must be jpeg, jpg, or png.`);
  }
}

function validateNativeRoute(route) {
  const entries = splitRoute(route, "native phase --route");
  let previousAt = -1;
  for (const [index, entry] of entries.entries()) {
    const parts = entry.split(",").map((part) => part.trim());
    if (parts.length < 2 || parts.length > 5 || !isFiniteText(parts[0]) || !isFiniteText(parts[1])) {
      throw new Error(`native phase --route entry ${index + 1} must be x,y[,button,atMs,label].`);
    }
    if (parts[2] && !["left", "right"].includes(parts[2])) {
      throw new Error(`native phase --route entry ${index + 1} has an invalid button.`);
    }
    const atMs = parts[3] ? Number(parts[3]) : index * 900;
    if (!Number.isFinite(atMs) || atMs < 0 || atMs < previousAt) {
      throw new Error("native phase --route action times must be finite, non-negative, and monotonic.");
    }
    previousAt = atMs;
  }
}

function validateWebClickSequence(route) {
  const entries = splitRoute(route, "web phase --clickSequence");
  let previousAt = -1;
  for (const [index, entry] of entries.entries()) {
    const parts = entry.split(",").map((part) => part.trim());
    if (parts.length < 2 || parts.length > 5 || !isFiniteText(parts[0]) || !isFiniteText(parts[1])) {
      throw new Error(`web phase --clickSequence entry ${index + 1} must be dx,dy[,button,atMs,label].`);
    }
    const thirdIsButton = /^(left|right)$/.test(parts[2] ?? "");
    const atText = thirdIsButton ? parts[3] : parts[2];
    const atMs = atText ? Number(atText) : index * 900;
    if (!Number.isFinite(atMs) || atMs < 0 || atMs < previousAt) {
      throw new Error("web phase --clickSequence action times must be finite, non-negative, and monotonic.");
    }
    previousAt = atMs;
  }
}

function splitRoute(route, label) {
  const entries = route.split(";").map((entry) => entry.trim()).filter(Boolean);
  if (entries.length === 0) throw new Error(`${label} must contain at least one action.`);
  return entries;
}

function resolveReportInput(key, configured, upstreamEnabled, generatedPath) {
  if (upstreamEnabled) {
    if (configured !== undefined) {
      throw new Error(`report phase --${key} is managed while the upstream ${key === "original" ? "native" : "web"} phase is enabled.`);
    }
    return generatedPath;
  }
  return configured === undefined ? generatedPath : resolveRepoPath(configured);
}

async function validateExternalInputs(plan) {
  for (const name of PHASE_NAMES) {
    if (plan.phasePlans[name].enabled && !(await isFile(plan.phasePlans[name].scriptPath))) {
      throw new Error(`${name} phase script does not exist: ${plan.phasePlans[name].scriptPath}`);
    }
  }
  if (!plan.phasePlans.report.enabled) return;
  for (const [upstream, key] of [["native", "original"], ["web", "web"]]) {
    if (plan.phasePlans[upstream].enabled) continue;
    const argv = plan.phasePlans.report.argv;
    const index = argv.indexOf(`--${key}`);
    const inputPath = argv[index + 1];
    if (!(await isFile(inputPath))) {
      throw new Error(`report phase --${key} input does not exist: ${inputPath}`);
    }
  }
  const webBichonIndex = plan.phasePlans.report.argv.indexOf("--webBichon");
  if (webBichonIndex >= 0 && !(await isFile(plan.phasePlans.report.argv[webBichonIndex + 1]))) {
    throw new Error(`report phase --webBichon input does not exist: ${plan.phasePlans.report.argv[webBichonIndex + 1]}`);
  }
}

async function runJsonCommand(phase, secretValues) {
  const result = await runChild(nodeExecutable(), phase.argv, phase.timeoutMs);
  if (result.spawnError) throw new Error(`Could not launch ${path.basename(phase.scriptPath)}: ${result.spawnError.message}`);
  if (result.timedOut) throw new Error(`${path.basename(phase.scriptPath)} timed out after ${phase.timeoutMs}ms.`);
  if (result.code !== 0) {
    const detail = sanitizeText((result.stderr.trim() || result.stdout.trim()).slice(-8_000), secretValues);
    throw new Error(`${path.basename(phase.scriptPath)} exited with code ${result.code}${detail ? `: ${detail}` : ""}`);
  }
  const parsed = extractLastJsonObject(result.stdout);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error(`${path.basename(phase.scriptPath)} did not print a JSON result object.`);
  }
  if (parsed.ok !== true) throw new Error(`${path.basename(phase.scriptPath)} reported ok=${JSON.stringify(parsed.ok)}.`);
  return parsed;
}

function runChild(command, argv, timeoutMs) {
  return new Promise((resolve) => {
    const environment = processObject()?.env;
    const child = spawn(command, argv, {
      cwd: REPO_ROOT,
      ...(environment ? { env: environment } : null),
      shell: false,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let spawnError = null;
    child.stdout?.on("data", (chunk) => { stdout = appendBounded(stdout, chunk.toString()); });
    child.stderr?.on("data", (chunk) => { stderr = appendBounded(stderr, chunk.toString()); });
    child.on("error", (error) => { spawnError = error; });
    const timer = setTimeout(() => {
      timedOut = true;
      killProcessTree(child.pid);
    }, timeoutMs);
    child.on("close", (code, signal) => {
      clearTimeout(timer);
      resolve({ code, signal, stdout, stderr, timedOut, spawnError });
    });
  });
}

async function verifyPhaseArtifacts(name, phase, result, startedAtMs) {
  const primaryResultKey = name === "web" ? "statePath" : "jsonPath";
  const primaryPath = phase.artifacts.jsonPath;
  if (!samePath(result[primaryResultKey], primaryPath)) {
    throw new Error(`${name} phase returned unexpected ${primaryResultKey}: ${JSON.stringify(result[primaryResultKey])}.`);
  }
  await assertFreshNonEmptyFile(primaryPath, startedAtMs, `${name} JSON artifact`);
  const report = await readJson(primaryPath);
  if (report?.ok !== true) throw new Error(`${name} JSON artifact reported ok=${JSON.stringify(report?.ok)}.`);
  if (name === "web") {
    if (!samePath(result.screenshotPath, phase.artifacts.screenshotPath)) {
      throw new Error(`web phase returned unexpected screenshotPath: ${JSON.stringify(result.screenshotPath)}.`);
    }
    await assertFreshNonEmptyFile(phase.artifacts.screenshotPath, startedAtMs, "web screenshot artifact");
  }
  if (name === "report") {
    if (!samePath(result.mdPath, phase.artifacts.markdownPath)) {
      throw new Error(`report phase returned unexpected mdPath: ${JSON.stringify(result.mdPath)}.`);
    }
    await assertFreshNonEmptyFile(phase.artifacts.markdownPath, startedAtMs, "report Markdown artifact");
  }
}

async function assertFreshNonEmptyFile(filePath, startedAtMs, label) {
  let stats;
  try {
    stats = await fs.stat(filePath);
  } catch (error) {
    throw new Error(`${label} was not created at ${filePath}: ${error.message}`);
  }
  if (!stats.isFile() || stats.size === 0) throw new Error(`${label} is empty or not a file: ${filePath}`);
  if (stats.mtimeMs < startedAtMs - 2_000) throw new Error(`${label} was not refreshed by this run: ${filePath}`);
}

async function writeManifest(plan) {
  const safeManifest = redactValue(plan.manifest, plan.secretValues);
  assertRedacted(safeManifest, plan.secretValues);
  const serialized = `${JSON.stringify(safeManifest, null, 2)}\n`;
  const temporaryPath = `${plan.manifestPath}.${processObject()?.pid ?? `desktop-${Date.now()}`}.tmp`;
  await fs.writeFile(temporaryPath, serialized, "utf8");
  await fs.rename(temporaryPath, plan.manifestPath);
}

function summarizeRun(plan) {
  return {
    ok: plan.manifest.ok,
    status: plan.manifest.status,
    dryRun: plan.dryRun,
    scenarioId: plan.manifest.scenario.config.id,
    outputDir: plan.outputDir,
    manifestPath: plan.manifestPath,
    phases: Object.fromEntries(PHASE_NAMES.map((name) => [name, plan.manifest.phases[name].status])),
  };
}

function argsToArgv(args) {
  return Object.keys(args).sort().flatMap((key) => [`--${key}`, String(args[key])]);
}

function mapDisplayPaths(paths) {
  return Object.fromEntries(Object.entries(paths).map(([key, value]) => [key, displayPath(value)]));
}

function displayPath(filePath) {
  const relative = path.relative(REPO_ROOT, filePath);
  return relative && !relative.startsWith("..") && !path.isAbsolute(relative)
    ? relative.replaceAll("\\", "/")
    : path.resolve(filePath);
}

function resolveRepoPath(filePath) {
  return path.isAbsolute(filePath) ? path.resolve(filePath) : path.resolve(REPO_ROOT, filePath);
}

function collectSecrets(value, secrets = new Set()) {
  if (Array.isArray(value)) {
    for (const entry of value) collectSecrets(entry, secrets);
    return secrets;
  }
  if (!value || typeof value !== "object") return secrets;
  for (const [key, nested] of Object.entries(value)) {
    if (isSensitiveKey(key) && ["string", "number"].includes(typeof nested) && String(nested).length >= 4) {
      secrets.add(String(nested));
    } else {
      collectSecrets(nested, secrets);
    }
  }
  return secrets;
}

function collectEnvironmentSecrets(secrets) {
  const environment = processObject()?.env;
  if (!environment) return;
  for (const key of ["MIR2_QA_ACCOUNT", "MIR2_QA_PASSWORD", "MIR2_QA_CONTROL_TOKEN"]) {
    const value = environment[key];
    if (value && value.length >= 4) secrets.add(value);
  }
}

function redactValue(value, secretValues = new Set()) {
  if (typeof value === "string") return sanitizeText(value, secretValues);
  if (Array.isArray(value)) return value.map((entry) => redactValue(entry, secretValues));
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, nested]) => [
      key,
      isSensitiveKey(key) ? REDACTED : redactValue(nested, secretValues),
    ]),
  );
}

function isSensitiveKey(key) {
  return /(?:account|authorization|credential|passkey|password|qaControlToken|secret|token|username)/i.test(key);
}

function sanitizeText(value, secretValues = new Set()) {
  let sanitized = String(value ?? "");
  for (const secret of secretValues) {
    sanitized = sanitized.split(secret).join(REDACTED);
    const encoded = encodeURIComponent(secret);
    if (encoded !== secret) sanitized = sanitized.split(encoded).join(REDACTED);
  }
  return sanitized
    .replace(/(--(?:account|authorization|passkey|password|qaControlToken|secret|token|username)(?:=|\s+))\S+/gi, `$1${REDACTED}`)
    .replace(/([?&](?:account|accountId|account_id|authorization|passkey|password|qaControlToken|secret|token|username)=)[^&#\s]+/gi, `$1${REDACTED}`)
    .replace(/("(?:account|accountId|account_id|authorization|passkey|password|qaControlToken|secret|token|username)"\s*:\s*")[^"]*/gi, `$1${REDACTED}`);
}

function assertRedacted(value, secretValues) {
  const serialized = JSON.stringify(value);
  for (const secret of secretValues) {
    if (secret && serialized.includes(secret)) {
      throw new Error("Refusing to write a temporal-pack manifest containing a known secret.");
    }
  }
}

function safeErrorMessage(error, secretValues = new Set()) {
  return sanitizeText(error instanceof Error ? error.message : String(error), secretValues);
}

function extractLastJsonObject(stdout) {
  const trimmed = stdout.trim();
  try {
    return JSON.parse(trimmed);
  } catch {
    const starts = [];
    for (let index = 0; index < trimmed.length; index += 1) {
      if (trimmed[index] === "{" && (index === 0 || trimmed[index - 1] === "\n")) starts.push(index);
    }
    for (let index = starts.length - 1; index >= 0; index -= 1) {
      try {
        return JSON.parse(trimmed.slice(starts[index]));
      } catch {
        // Try the previous top-level-looking JSON object.
      }
    }
    return null;
  }
}

function appendBounded(current, next) {
  const combined = current + next;
  return combined.length > MAX_CHILD_OUTPUT_CHARS ? combined.slice(-MAX_CHILD_OUTPUT_CHARS) : combined;
}

function killProcessTree(pid) {
  if (!pid) return;
  if ((processObject()?.platform ?? (path.sep === "\\" ? "win32" : "unknown")) === "win32") {
    spawn("taskkill.exe", ["/PID", String(pid), "/T", "/F"], { windowsHide: true, stdio: "ignore" });
    return;
  }
  try {
    processObject()?.kill?.(pid, "SIGKILL");
  } catch {
    // The process may already have exited.
  }
}

function assertRecord(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label} must be an object.`);
}

function assertKnownKeys(value, known, label) {
  const unknown = Object.keys(value).filter((key) => !known.has(key));
  if (unknown.length) throw new Error(`Unknown ${label} field(s): ${unknown.join(", ")}.`);
}

function validatePhaseOverrides(overrides) {
  assertRecord(overrides ?? {}, "phaseOverrides");
  for (const [key, value] of Object.entries(overrides ?? {})) {
    if (!PHASE_NAMES.includes(key) || typeof value !== "boolean") {
      throw new Error(`Invalid phase override ${key}=${JSON.stringify(value)}.`);
    }
  }
}

function validateUrl(value, label) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`${label} must be an absolute URL.`);
  }
  if (!["http:", "https:"].includes(url.protocol) || url.username || url.password) {
    throw new Error(`${label} must be an http(s) URL without embedded credentials.`);
  }
}

function isFiniteText(value) {
  return value !== "" && Number.isFinite(Number(value));
}

function isPositiveInteger(value, maximum = Number.MAX_SAFE_INTEGER) {
  return Number.isInteger(value) && value > 0 && value <= maximum;
}

async function readJson(filePath) {
  return JSON.parse((await fs.readFile(filePath, "utf8")).replace(/^\uFEFF/, ""));
}

async function isFile(filePath) {
  try {
    return (await fs.stat(filePath)).isFile();
  } catch {
    return false;
  }
}

function samePath(left, right) {
  return typeof left === "string" && path.resolve(left).toLowerCase() === path.resolve(right).toLowerCase();
}

export function parseTemporalPackCliArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--") continue;
    if (!arg.startsWith("--")) throw new Error(`Unexpected positional argument ${JSON.stringify(arg)}.`);
    const separator = arg.indexOf("=");
    const key = separator > 2 ? arg.slice(2, separator) : arg.slice(2);
    if (!CLI_ARGS.has(key)) throw new Error(`Unknown argument --${key}.`);
    if (Object.hasOwn(parsed, key)) throw new Error(`Argument --${key} was provided more than once.`);
    if (separator > 2) {
      parsed[key] = arg.slice(separator + 1);
      continue;
    }
    const next = argv[index + 1];
    parsed[key] = next === undefined || next.startsWith("--") ? "true" : argv[++index];
  }
  return parsed;
}

function buildPhaseOverrides(args) {
  if (args.phases !== undefined && ["native", "web", "report"].some((name) => args[name] !== undefined)) {
    throw new Error("--phases cannot be combined with --native, --web, or --report.");
  }
  if (args.phases !== undefined) {
    const requested = args.phases.split(",").map((value) => value.trim()).filter(Boolean);
    if (!requested.length || new Set(requested).size !== requested.length || requested.some((name) => !PHASE_NAMES.includes(name))) {
      throw new Error("--phases must be a unique comma-separated subset of native,web,report.");
    }
    return Object.fromEntries(PHASE_NAMES.map((name) => [name, requested.includes(name)]));
  }
  return Object.fromEntries(
    PHASE_NAMES.filter((name) => args[name] !== undefined).map((name) => [name, strictBoolean(args[name], name)]),
  );
}

function strictBoolean(value, label) {
  if (value === undefined) return false;
  const normalized = String(value).trim().toLowerCase();
  if (["1", "true", "yes", "on"].includes(normalized)) return true;
  if (["0", "false", "no", "off"].includes(normalized)) return false;
  throw new Error(`--${label} must be true or false; received ${JSON.stringify(value)}.`);
}

function printHelp() {
  console.log(`Usage: node apps/web/scripts/capture-crystal-temporal-pack.mjs [options]

Runs native capture, Web capture, and temporal reporting as one fail-closed pack.

Options:
  --scenario <json>       Scenario file (default: scripts/scenarios/bichon-332275-left4.json)
  --dryRun <bool>         Validate and write the redacted plan without launching apps
  --output <dir>          Override the scenario output directory
  --phases <list>         Run a comma-separated subset of native,web,report
  --native <bool>         Override the native phase
  --web <bool>            Override the Web phase
  --report <bool>         Override the report phase
  --help                  Show this help`);
}

async function cliMain() {
  const args = parseTemporalPackCliArgs(processObject().argv.slice(2));
  if (strictBoolean(args.help, "help")) {
    printHelp();
    return;
  }
  const result = await runTemporalPack({
    scenarioPath: args.scenario ? path.resolve(args.scenario) : DEFAULT_SCENARIO_PATH,
    outputDir: args.output,
    dryRun: strictBoolean(args.dryRun, "dryRun"),
    phaseOverrides: buildPhaseOverrides(args),
  });
  console.log(JSON.stringify(result, null, 2));
}

function isDirectRun() {
  const runtime = processObject();
  return Boolean(runtime?.argv?.[1]) && pathToFileURL(path.resolve(runtime.argv[1])).href === import.meta.url;
}

function processObject() {
  return typeof process === "undefined" ? null : process;
}

function nodeExecutable() {
  return processObject()?.execPath ?? "node";
}

if (isDirectRun()) {
  cliMain().catch((error) => {
    console.error(JSON.stringify({ ok: false, error: safeErrorMessage(error) }, null, 2));
    processObject().exitCode = 1;
  });
}
