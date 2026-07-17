import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import sharp from "sharp";

const DEFAULT_WINDOW_PATTERN = "Legend of Mir 2";
const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));

export async function captureOriginalComputerUse(options = {}) {
  const sky = await resolveSky(options);
  const outputDir = path.resolve(
    options.outputDir ?? path.join("docs", "generated", "player-qa", "movement-jitter"),
  );
  const prefix = options.prefix ?? `original-computer-use-${Date.now()}`;
  const windowTitlePattern = new RegExp(options.windowTitlePattern ?? DEFAULT_WINDOW_PATTERN, "i");
  const clickX = numberOption(options.clickX, 620);
  const clickY = numberOption(options.clickY, 520);
  const button = options.button === "right" ? "right" : "left";
  const sampleMs = numberOption(options.sampleMs, 160);
  const clickIntervalMs = numberOption(options.clickIntervalMs, 900);
  const routePostMs = numberOption(options.routePostMs, 1800);
  const warmupMs = numberOption(options.warmupMs, 250);
  const settleAfterClickMs = numberOption(options.settleAfterClickMs, 0);
  const frameCaptureMode = options.frameCaptureMode ?? options.captureMode ?? "computerUse";
  const frameImageFormat = options.frameImageFormat ?? "jpeg";
  const frameImageQuality = numberOption(options.frameImageQuality, 82);
  const clickActions = normalizeClickActions(options, { clickX, clickY, button, clickIntervalMs });
  const captureMs = Math.max(
    numberOption(options.captureMs, clickActions.at(-1)?.atMs + routePostMs),
    clickActions.at(-1)?.atMs + Math.min(routePostMs, sampleMs),
  );
  const label = options.label ?? (clickActions.length > 1 ? "computer-use-route" : `computer-use-${button}-${clickX}-${clickY}`);

  await fs.mkdir(outputDir, { recursive: true });
  const target = await findTargetWindow(sky, windowTitlePattern);
  let targetWindow = await sky.get_window(target.window);
  await sky.activate_window({ window: targetWindow });
  await delay(warmupMs);
  const initialState = await sky.get_window_state({ window: targetWindow });
  targetWindow = initialState.window;

  if (frameCaptureMode === "powershell") {
    return await captureWithPowerShellFrames({
      sky,
      target,
      targetWindow,
      outputDir,
      prefix,
      label,
      clickActions,
      sampleMs,
      captureMs,
      clickIntervalMs,
      routePostMs,
      warmupMs,
      settleAfterClickMs,
      frameImageFormat,
      frameImageQuality,
      windowTitlePattern: options.powershellWindowTitlePattern ?? options.windowTitleWildcard ?? `*${DEFAULT_WINDOW_PATTERN}*`,
    });
  }

  const samples = [];
  let state = await sky.get_window_state({ window: targetWindow });
  targetWindow = state.window;
  samples.push(await saveStateSample(state, outputDir, prefix, label, 0, 0));

  const startedAt = Date.now();
  let nextActionIndex = 0;
  let index = 1;
  while (Date.now() - startedAt <= captureMs) {
    const elapsedBeforeActions = Date.now() - startedAt;
    while (nextActionIndex < clickActions.length && clickActions[nextActionIndex].atMs <= elapsedBeforeActions) {
      const action = clickActions[nextActionIndex];
      await sky.click({
        window: targetWindow,
        x: action.windowX,
        y: action.windowY,
        mouse_button: action.button,
        click_count: 1,
        screenshotId: state.screenshots?.[0]?.id,
      });
      action.performedAtMs = Date.now() - startedAt;
      nextActionIndex += 1;
      if (settleAfterClickMs > 0) {
        await delay(settleAfterClickMs);
      }
    }

    state = await sky.get_window_state({ window: targetWindow });
    targetWindow = state.window;
    samples.push(await saveStateSample(state, outputDir, prefix, label, index, Date.now() - startedAt));
    index += 1;
    await delay(sampleMs);
  }

  const report = {
    ok: true,
    interaction: "computerUseClickTarget",
    startedAt: new Date().toISOString(),
    windowTitlePattern: String(options.windowTitlePattern ?? DEFAULT_WINDOW_PATTERN),
    window: targetWindow,
    app: target.app,
    sampleMs,
    captureMs,
    clickIntervalMs,
    warmupMs,
    routePostMs,
    actionCount: clickActions.length,
    actions: clickActions.map((action, index) => ({
      label: action.label ?? `${label}-${index + 1}`,
      type: "computerUseClickTarget",
      button: action.button,
      windowX: action.windowX,
      windowY: action.windowY,
      atMs: action.atMs,
      performedAtMs: action.performedAtMs ?? null,
    })),
    sampleCount: samples.length,
    samples,
  };

  const jsonPath = path.join(outputDir, `${prefix}.json`);
  await fs.writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  return { ok: true, jsonPath, report };
}

async function captureWithPowerShellFrames({
  sky,
  target,
  targetWindow,
  outputDir,
  prefix,
  label,
  clickActions,
  sampleMs,
  captureMs,
  clickIntervalMs,
  routePostMs,
  warmupMs,
  settleAfterClickMs,
  frameImageFormat,
  frameImageQuality,
  windowTitlePattern,
}) {
  const framePrefix = `${prefix}-frames`;
  const frameCapture = await startPowerShellFrameCapture({
    outputDir,
    prefix: framePrefix,
    label,
    durationMs: captureMs,
    sampleMs,
    imageFormat: frameImageFormat,
    jpegQuality: frameImageQuality,
    windowTitlePattern,
  });

  const captureStart = await frameCapture.start();
  const startedAt = Number(captureStart.startedAtMs);
  if (!Number.isFinite(startedAt)) {
    throw new Error("PowerShell frame capture did not report a valid startedAtMs timestamp.");
  }
  let nextActionIndex = 0;
  let interactionError = null;
  try {
    while (Date.now() - startedAt <= captureMs) {
      const elapsedBeforeActions = Date.now() - startedAt;
      while (nextActionIndex < clickActions.length && clickActions[nextActionIndex].atMs <= elapsedBeforeActions) {
        const action = clickActions[nextActionIndex];
        await sky.click({
          window: targetWindow,
          x: action.windowX,
          y: action.windowY,
          mouse_button: action.button,
          click_count: 1,
        });
        action.performedAtMs = Date.now() - startedAt;
        action.performedAtCaptureMs = action.performedAtMs;
        nextActionIndex += 1;
        if (settleAfterClickMs > 0) {
          await delay(settleAfterClickMs);
        }
      }

      const nextActionAtMs = clickActions[nextActionIndex]?.atMs ?? captureMs;
      const waitMs = Math.max(10, Math.min(35, nextActionAtMs - (Date.now() - startedAt)));
      await delay(waitMs);
    }
  } catch (error) {
    interactionError = error;
  }

  const frameReport = await frameCapture.wait();
  const frameContent = await validateCapturedFrameContent(frameReport.samples);
  const actionCoverageComplete = nextActionIndex === clickActions.length;
  const report = {
    ok: frameContent.valid && actionCoverageComplete && interactionError === null,
    interaction: "computerUseClickTarget",
    startedAt: new Date(startedAt).toISOString(),
    startedAtMs: startedAt,
    captureHandshake: "start-signal-v1",
    windowTitlePattern: String(windowTitlePattern),
    window: frameReport.window ?? targetWindow,
    app: target.app,
    sampleMs,
    captureMs,
    clickIntervalMs,
    warmupMs,
    routePostMs,
    frameCaptureMode: "powershell",
    frameCaptureReportPath: frameCapture.jsonPath,
    frameImageFormat,
    frameImageQuality: frameImageFormat === "jpeg" ? frameImageQuality : null,
    frameContent,
    actionCoverageComplete,
    interactionError: interactionError ? String(interactionError?.stack ?? interactionError) : null,
    actionCount: clickActions.length,
    actions: clickActions.map((action, index) => ({
      label: action.label ?? `${label}-${index + 1}`,
      type: "computerUseClickTarget",
      button: action.button,
      windowX: action.windowX,
      windowY: action.windowY,
      atMs: action.atMs,
      performedAtMs: action.performedAtMs ?? null,
      performedAtCaptureMs: action.performedAtCaptureMs ?? null,
    })),
    sampleCount: frameReport.samples.length,
    samples: frameReport.samples,
  };

  const jsonPath = path.join(outputDir, `${prefix}.json`);
  await fs.writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  return { ok: report.ok, jsonPath, report };
}

async function validateCapturedFrameContent(samples) {
  const framePaths = [samples?.[0]?.capture?.path, samples?.at(-1)?.capture?.path].filter(Boolean);
  const frames = await Promise.all(framePaths.map((framePath) => inspectFrameContent(framePath)));
  return {
    valid:
      frames.length === 2 &&
      frames.every((frame) => frame.meanLuma >= 8 && frame.nonBlackRatio >= 0.1),
    frames,
  };
}

async function inspectFrameContent(framePath) {
  const { data, info } = await sharp(framePath)
    .resize(64, 48, { fit: "fill" })
    .removeAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });
  let lumaSum = 0;
  let nonBlackPixels = 0;
  const pixelCount = data.length / info.channels;
  for (let offset = 0; offset < data.length; offset += info.channels) {
    const luma = (data[offset] + data[offset + 1] + data[offset + 2]) / 3;
    lumaSum += luma;
    if (luma >= 12) nonBlackPixels += 1;
  }
  return {
    path: framePath,
    meanLuma: Number((lumaSum / pixelCount).toFixed(4)),
    nonBlackRatio: Number((nonBlackPixels / pixelCount).toFixed(6)),
  };
}

async function startPowerShellFrameCapture({
  outputDir,
  prefix,
  label,
  durationMs,
  sampleMs,
  imageFormat,
  jpegQuality,
  windowTitlePattern,
}) {
  const scriptPath = path.join(SCRIPT_DIR, "capture-original-window-frames.ps1");
  const jsonPath = path.join(outputDir, `${prefix}.json`);
  const readyPath = path.join(outputDir, `${prefix}-ready.json`);
  const startSignalPath = path.join(outputDir, `${prefix}-start.signal`);
  await Promise.all([
    fs.rm(readyPath, { force: true }),
    fs.rm(startSignalPath, { force: true }),
  ]);
  const args = [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    scriptPath,
    "-OutputDir",
    outputDir,
    "-Prefix",
    prefix,
    "-ReadyFile",
    readyPath,
    "-StartSignalFile",
    startSignalPath,
    "-StartSignalTimeoutMs",
    "30000",
    "-Label",
    label,
    "-DurationMs",
    String(durationMs),
    "-SampleMs",
    String(sampleMs),
    "-ImageFormat",
    imageFormat,
    "-JpegQuality",
    String(jpegQuality),
    "-WindowTitlePattern",
    windowTitlePattern,
  ];
  const child = spawn("powershell.exe", args, { windowsHide: true });
  let stdout = "";
  let stderr = "";
  const exitPromise = new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", resolve);
  });
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  const waitForReadyStage = async (expectedStage, timeoutMs) => {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      if (child.exitCode !== null || child.signalCode !== null) {
        throw new Error(
          `PowerShell frame capture exited before ${expectedStage}: ${stderr || stdout}`,
        );
      }
      try {
        const raw = await fs.readFile(readyPath, "utf8");
        const ready = JSON.parse(raw.replace(/^\uFEFF/, ""));
        if (ready?.stage === expectedStage) return ready;
      } catch (error) {
        if (error?.code !== "ENOENT" && !(error instanceof SyntaxError)) throw error;
      }
      await delay(10);
    }
    throw new Error(`Timed out after ${timeoutMs}ms waiting for frame capture stage ${expectedStage}.`);
  };

  return {
    jsonPath,
    start: async () => {
      try {
        await waitForReadyStage("waiting", 15_000);
        await fs.writeFile(startSignalPath, `${JSON.stringify({ signalAtMs: Date.now() })}\n`, "utf8");
        return await waitForReadyStage("capturing", 5_000);
      } catch (error) {
        child.kill();
        await Promise.all([
          fs.rm(readyPath, { force: true }),
          fs.rm(startSignalPath, { force: true }),
        ]);
        throw error;
      }
    },
    wait: async () => {
      try {
        const code = await exitPromise;
        if (code !== 0) {
          throw new Error(`PowerShell frame capture failed with code ${code}: ${stderr || stdout}`);
        }
        const raw = await fs.readFile(jsonPath, "utf8");
        return JSON.parse(raw.replace(/^\uFEFF/, ""));
      } finally {
        await Promise.all([
          fs.rm(readyPath, { force: true }),
          fs.rm(startSignalPath, { force: true }),
        ]);
      }
    },
  };
}

function normalizeClickActions(options, defaults) {
  const explicit = Array.isArray(options.clicks)
    ? options.clicks
    : Array.isArray(options.routeClicks)
      ? options.routeClicks
      : parseRouteString(options.route ?? options.clickRoute);
  const rawActions =
    explicit && explicit.length
      ? explicit
      : [{ x: defaults.clickX, y: defaults.clickY, button: defaults.button, atMs: 0 }];
  return rawActions.map((action, index) => {
    const parsedButton = action.button === "right" ? "right" : "left";
    return {
      label: action.label ?? null,
      button: parsedButton,
      windowX: numberOption(action.windowX ?? action.x, defaults.clickX),
      windowY: numberOption(action.windowY ?? action.y, defaults.clickY),
      atMs: numberOption(action.atMs ?? action.at ?? action.delayMs, index * defaults.clickIntervalMs),
    };
  });
}

function parseRouteString(route) {
  if (!route) {
    return null;
  }
  return String(route)
    .split(";")
    .map((segment) => segment.trim())
    .filter(Boolean)
    .map((segment) => {
      const [x, y, button, atMs, label] = segment.split(",").map((part) => part.trim());
      return {
        x,
        y,
        ...(button ? { button } : null),
        ...(atMs ? { atMs } : null),
        ...(label ? { label } : null),
      };
    });
}

async function resolveSky(options) {
  if (globalThis.sky) {
    return globalThis.sky;
  }
  const modulePath =
    options.computerUseClientModule ??
    (typeof process !== "undefined" ? process.env.MIR2_COMPUTER_USE_CLIENT_MODULE : null);
  if (!modulePath) {
    throw new Error("Computer Use is not initialized; run through node_repl with globalThis.sky or pass computerUseClientModule.");
  }
  const { setupComputerUseRuntime } = await import(pathToFileURL(modulePath).href);
  await setupComputerUseRuntime({ globals: globalThis });
  return globalThis.sky;
}

async function findTargetWindow(sky, titlePattern) {
  const liveWindows = await sky.list_windows();
  const liveCandidates = liveWindows.filter((window) => titlePattern.test(window.title ?? ""));
  if (liveCandidates.length === 1) {
    const window = liveCandidates[0];
    return {
      app: {
        id: window.app,
        displayName: window.title ?? null,
        isRunning: true,
      },
      window,
    };
  }
  if (liveCandidates.length > 1) {
    throw new Error(`Expected one live target window matching ${titlePattern}, found ${liveCandidates.length}`);
  }

  const apps = await sky.list_apps();
  const candidates = [];
  for (const app of apps) {
    for (const window of app.windows ?? []) {
      if (titlePattern.test(window.title ?? "")) {
        candidates.push({ app, window });
      }
    }
  }
  if (candidates.length !== 1) {
    throw new Error(`Expected one target window matching ${titlePattern}, found ${candidates.length}`);
  }
  return candidates[0];
}

async function saveStateSample(state, outputDir, prefix, label, index, elapsedMs) {
  const screenshot = state.screenshots?.[0];
  if (!screenshot?.url) {
    throw new Error("Computer Use did not return a screenshot for the target window");
  }
  const decoded = decodeDataUrl(screenshot.url);
  const safeLabel = String(label).replace(/[^a-z0-9_-]+/gi, "-").replace(/^-|-$/g, "") || "sample";
  const fileName = `${prefix}-${String(index).padStart(3, "0")}-${safeLabel}.${decoded.extension}`;
  const imagePath = path.join(outputDir, fileName);
  await fs.writeFile(imagePath, decoded.bytes);
  return {
    label,
    index,
    elapsedMs,
    capture: {
      path: imagePath,
      width: screenshot.width ?? null,
      height: screenshot.height ?? null,
      mimeType: decoded.mimeType,
    },
  };
}

function decodeDataUrl(dataUrl) {
  const match = /^data:([^;,]+);base64,(.*)$/s.exec(dataUrl);
  if (!match) {
    throw new Error("Unsupported screenshot data URL");
  }
  const mimeType = match[1];
  const extension = mimeType.includes("jpeg") ? "jpg" : mimeType.includes("webp") ? "webp" : "png";
  return {
    mimeType,
    extension,
    bytes: Buffer.from(match[2], "base64"),
  };
}

function numberOption(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
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

function cliOptions(args) {
  return {
    outputDir: args.output ?? args.outputDir,
    prefix: args.prefix,
    windowTitlePattern: args.windowTitlePattern ?? args.window,
    clickX: args.clickX ?? args.x,
    clickY: args.clickY ?? args.y,
    button: args.button,
    sampleMs: args.sampleMs,
    captureMs: args.captureMs,
    clickIntervalMs: args.clickIntervalMs,
    routePostMs: args.routePostMs,
    warmupMs: args.warmupMs,
    settleAfterClickMs: args.settleAfterClickMs,
    label: args.label,
    route: args.route ?? args.clickRoute ?? args.clicks,
    frameCaptureMode: args.frameCaptureMode ?? args.captureMode,
    frameImageFormat: args.frameImageFormat,
    frameImageQuality: args.frameImageQuality,
    powershellWindowTitlePattern: args.powershellWindowTitlePattern ?? args.windowTitleWildcard,
    computerUseClientModule: args.computerUseClientModule,
  };
}

function isDirectRun() {
  if (typeof process === "undefined") {
    return false;
  }
  const entry = process.argv[1];
  return entry ? path.resolve(fileURLToPath(import.meta.url)) === path.resolve(entry) : false;
}

if (isDirectRun()) {
  captureOriginalComputerUse(cliOptions(parseArgs(process.argv.slice(2))))
    .then((result) => {
      console.log(
        JSON.stringify(
          {
            ok: result.ok,
            jsonPath: result.jsonPath,
            sampleCount: result.report.sampleCount,
            actionCount: result.report.actionCount,
          },
          null,
          2,
        ),
      );
      if (!result.ok) process.exitCode = 1;
    })
    .catch((error) => {
      console.error(error?.stack ?? String(error));
      process.exitCode = 1;
    });
}
