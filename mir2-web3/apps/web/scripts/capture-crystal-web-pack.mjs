import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_OUTPUT_ROOT = path.join(REPO_ROOT, "docs", "generated", "player-qa", "visual-parity");
const DEFAULT_BASE_URL = "http://127.0.0.1:3002";
const DEFAULT_ACCOUNT = "QA0429A";
const DEFAULT_PASSWORD = "Mir2test1";
const DEFAULT_MAP = "0";
const DEFAULT_X = 149;
const DEFAULT_Y = 411;

const args = parseArgs(process.argv.slice(2));
const prefix = args.prefix ?? `crystal-web-pack-${timestamp()}`;
const outputRoot = path.resolve(args.output ?? args.outputRoot ?? DEFAULT_OUTPUT_ROOT);
const packDir = path.resolve(args.packDir ?? path.join(outputRoot, prefix));
const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL;
const gatewayWs = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS ?? "";
const crystalLineMessage = args.crystalLineMessage ?? process.env.MIR2_CRYSTAL_LINE_MESSAGE ?? "";
const crystalVisibleChatLines =
  args.crystalVisibleChatLines ?? args.crystalChatLines ?? process.env.MIR2_CRYSTAL_VISIBLE_CHAT_LINES ?? "";
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? DEFAULT_ACCOUNT;
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? DEFAULT_PASSWORD;
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, false);
const characterName = args.characterName ?? account;
const map = args.map ?? DEFAULT_MAP;
const x = numberArg(args.x, DEFAULT_X);
const y = numberArg(args.y, DEFAULT_Y);
const originalDurationMs = numberArg(args.originalDurationMs, 250);
const originalSampleMs = numberArg(args.originalSampleMs, 50);
const windowTitlePattern = args.windowTitlePattern ?? "*Legend of Mir 2*";
const samplePrefix = args.samplePrefix ?? `${prefix}-same-scene`;
const nativePreClickClient = args.nativePreClickClient ?? args.nativePreClick ?? "";
const nativePreKeys = args.nativePreKeys ?? "";
const nativeStatePath = args.nativeState ?? args.nativeAccountState ?? "";
const syncWebAccountState = booleanArg(
  args.syncWebAccountState ?? process.env.MIR2_SYNC_WEB_ACCOUNT_STATE,
  Boolean(nativeStatePath),
);
const accountStorePath = path.resolve(args.accountStore ?? process.env.MIR2_ACCOUNT_STORE_PATH ?? path.join(REPO_ROOT, ".mir2-data", "accounts.json"));

await main();

async function main() {
  await fs.mkdir(packDir, { recursive: true });

  const accountStateSync = await syncWebAccountStateFromNative();
  const nativeCapture = await captureNativeWindow();
  const webCapture = await captureWebScene(accountStateSync?.qaStatePath ?? null);

  const originalPath = path.join(packDir, `${samplePrefix}-original.png`);
  const webPath = path.join(packDir, `${samplePrefix}-web.png`);
  const webStatePath = path.join(packDir, `${samplePrefix}-web-state.json`);

  await fs.copyFile(nativeCapture.imagePath, originalPath);
  await fs.copyFile(webCapture.screenshotPath, webPath);
  await fs.copyFile(webCapture.statePath, webStatePath);

  const sideBySidePath = path.join(packDir, `${samplePrefix}-side-by-side.png`);
  await renderSideBySide({
    originalPath,
    webPath,
    outputPath: sideBySidePath,
    leftLabel: "Crystal native",
    rightLabel: `Web map ${map} @ ${x},${y}`,
  });
  const cropSet = await renderRegionCrops({
    originalPath,
    webPath,
    webStatePath,
    outputDir: packDir,
  });

  const visualReport = await runNodeScript(
    path.join(SCRIPT_DIR, "report-crystal-visual-parity.mjs"),
    [
      "--input",
      packDir,
      "--output",
      packDir,
      "--prefix",
      `${samplePrefix}-visual-score`,
      "--maxSamples",
      "1",
    ],
    { timeoutMs: 45_000 },
  );

  const summary = {
    ok: true,
    generatedAt: new Date().toISOString(),
    packDir,
    target: { map: String(map), x, y },
    native: {
      jsonPath: nativeCapture.jsonPath,
      imagePath: originalPath,
      sourceImagePath: nativeCapture.imagePath,
      sampleCount: nativeCapture.sampleCount,
      accountStatePath: accountStateSync?.nativeAccountStatePath ?? null,
    },
    web: {
      screenshotPath: webPath,
      statePath: webStatePath,
      sourceScreenshotPath: webCapture.screenshotPath,
      sourceStatePath: webCapture.statePath,
      accountStateSync,
    },
    sideBySidePath,
    cropSet,
    visualReport,
  };
  const summaryPath = path.join(packDir, `${samplePrefix}-summary.json`);
  await fs.writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  await fs.writeFile(path.join(packDir, "README.md"), renderReadme({ ...summary, summaryPath }), "utf8");

  console.log(
    JSON.stringify(
      {
        ok: true,
        packDir,
        summaryPath,
        sideBySidePath,
        visualReport: visualReport.markdownPath ?? visualReport.mdPath ?? null,
      },
      null,
      2,
    ),
  );
}

async function captureNativeWindow() {
  const rawPrefix = `${samplePrefix}-native-raw`;
  const nativeArgs = [
      "-OutputDir",
      packDir,
      "-Prefix",
      rawPrefix,
      "-Label",
      "native-static",
      "-DurationMs",
      String(originalDurationMs),
      "-SampleMs",
      String(originalSampleMs),
      "-ImageFormat",
      "png",
      "-WindowTitlePattern",
      windowTitlePattern,
      "-ActivateWindow",
    ];
  if (nativePreClickClient) {
    nativeArgs.push("-PreClickClientPoints", nativePreClickClient);
  }
  if (nativePreKeys) {
    nativeArgs.push("-PreKeys", nativePreKeys);
  }

  const result = await runPowerShellScript(
    path.join(SCRIPT_DIR, "capture-original-window-frames.ps1"),
    nativeArgs,
    { timeoutMs: 30_000 },
  );
  const report = await readJson(result.jsonPath);
  const firstSample = Array.isArray(report.samples) ? report.samples[0] : null;
  const imagePath = firstSample?.capture?.path;
  if (!imagePath) {
    throw new Error(`Native window capture did not produce a frame: ${result.jsonPath}`);
  }
  return {
    ...result,
    imagePath: path.resolve(imagePath),
    sampleCount: report.sampleCount ?? report.samples?.length ?? 0,
  };
}

async function syncWebAccountStateFromNative() {
  if (!syncWebAccountState) {
    return {
      ok: true,
      mode: "skipped",
      note: "syncWebAccountState was not enabled.",
    };
  }
  if (!nativeStatePath) {
    throw new Error("--syncWebAccountState requires --nativeState/--nativeAccountState.");
  }

  const sourceNativeStatePath = path.resolve(nativeStatePath);
  const copiedNativeStatePath = path.join(packDir, "native-account-state.json");
  if (path.resolve(sourceNativeStatePath).toLowerCase() !== path.resolve(copiedNativeStatePath).toLowerCase()) {
    await fs.copyFile(sourceNativeStatePath, copiedNativeStatePath);
  }

  const syncSummaryPath = path.join(packDir, "web-account-sync.json");
  const qaStatePath = path.join(packDir, "qa-character-state.json");
  const result = await runNodeScript(
    path.join(SCRIPT_DIR, "upsert-web-account-from-crystal-state.mjs"),
    [
      "--nativeState",
      copiedNativeStatePath,
      "--accountStore",
      accountStorePath,
      "--account",
      account,
      "--password",
      password,
      "--characterName",
      characterName,
      "--map",
      String(map),
      "--x",
      String(x),
      "--y",
      String(y),
      "--output",
      syncSummaryPath,
      "--qaStateOutput",
      qaStatePath,
    ],
    { timeoutMs: 30_000 },
  );

  return {
    ...result,
    mode: "nativeStateToWebAccountStoreAndQaPayload",
    nativeAccountStatePath: copiedNativeStatePath,
    syncSummaryPath,
    qaStatePath,
  };
}

async function captureWebScene(qaCharacterStatePath) {
  const rawPrefix = `${samplePrefix}-web-raw`;
  const webBaseUrl = buildWebBaseUrl();
  const result = await runNodeScript(
    path.join(SCRIPT_DIR, "capture-crystal-parity.mjs"),
    [
      "--baseUrl",
      webBaseUrl,
      "--output",
      packDir,
      "--prefix",
      rawPrefix,
      "--account",
      account,
      "--password",
      password,
      ...(createAccount ? ["--createAccount", "true", "--characterName", characterName] : []),
      ...(qaCharacterStatePath ? ["--qaCharacterState", qaCharacterStatePath] : []),
      ...(args.qaControlToken ? ["--qaControlToken", args.qaControlToken] : []),
      ...(args.debugPort ? ["--debugPort", args.debugPort] : []),
      ...(args.visualReadyTimeoutMs ? ["--visualReadyTimeoutMs", args.visualReadyTimeoutMs] : []),
      ...(args.targetTolerance ? ["--targetTolerance", args.targetTolerance] : []),
      "--map",
      String(map),
      "--x",
      String(x),
      "--y",
      String(y),
      "--settleMs",
      String(numberArg(args.webSettleMs, 1500)),
    ],
    { timeoutMs: numberArg(args.webCaptureTimeoutMs, 150_000) },
  );
  if (!result.screenshotPath || !result.statePath) {
    throw new Error(`Web capture did not return screenshot/state paths: ${JSON.stringify(result)}`);
  }
  return result;
}

function buildWebBaseUrl() {
  const url = new URL(baseUrl);
  if (gatewayWs && !url.searchParams.has("gatewayWs")) {
    url.searchParams.set("gatewayWs", gatewayWs);
  }
  if (crystalLineMessage && !url.searchParams.has("crystalLineMessage")) {
    url.searchParams.set("crystalLineMessage", crystalLineMessage);
  }
  if (crystalVisibleChatLines && !url.searchParams.has("crystalVisibleChatLines")) {
    url.searchParams.set("crystalVisibleChatLines", crystalVisibleChatLines);
  }
  return url.toString();
}

async function runNodeScript(scriptPath, scriptArgs, options = {}) {
  return runJsonCommand(process.execPath, [scriptPath, ...scriptArgs], { cwd: REPO_ROOT, ...options });
}

async function runPowerShellScript(scriptPath, scriptArgs, options = {}) {
  return runJsonCommand(
    "powershell.exe",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      scriptPath,
      ...scriptArgs,
    ],
    options,
  );
}

async function runJsonCommand(command, commandArgs, options = {}) {
  const child = spawn(command, commandArgs, {
    cwd: options.cwd ?? REPO_ROOT,
    windowsHide: true,
    env: process.env,
  });
  let stdout = "";
  let stderr = "";
  let timedOut = false;
  const timeoutMs = numberArg(options.timeoutMs, 0);
  const timer =
    timeoutMs > 0
      ? setTimeout(() => {
          timedOut = true;
          killProcessTree(child.pid);
        }, timeoutMs)
      : null;
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const code = await new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", resolve);
  });
  if (timer) clearTimeout(timer);
  if (timedOut) {
    throw new Error(`${command} ${commandArgs.join(" ")} timed out after ${timeoutMs}ms\n${stderr || stdout}`);
  }
  if (code !== 0) {
    throw new Error(`${command} ${commandArgs.join(" ")} failed with code ${code}\n${stderr || stdout}`);
  }
  const trimmed = stdout.trim();
  const start = trimmed.lastIndexOf("\n{") >= 0 ? trimmed.lastIndexOf("\n{") + 1 : trimmed.indexOf("{");
  const end = trimmed.lastIndexOf("}");
  if (start < 0 || end < start) {
    throw new Error(`Command did not print a JSON object:\n${stdout}\n${stderr}`);
  }
  return JSON.parse(trimmed.slice(start, end + 1));
}

function killProcessTree(pid) {
  if (!pid) return;
  if (process.platform === "win32") {
    spawn("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
      windowsHide: true,
      stdio: "ignore",
    });
    return;
  }
  try {
    process.kill(pid, "SIGKILL");
  } catch {
    // The process may already have exited.
  }
}

async function renderSideBySide({ originalPath, webPath, outputPath, leftLabel, rightLabel }) {
  const [originalMeta, webMeta] = await Promise.all([sharp(originalPath).metadata(), sharp(webPath).metadata()]);
  const labelHeight = 34;
  const gap = 8;
  const leftWidth = originalMeta.width ?? 1024;
  const leftHeight = originalMeta.height ?? 768;
  const rightWidth = webMeta.width ?? 1024;
  const rightHeight = webMeta.height ?? 768;
  const width = leftWidth + rightWidth + gap;
  const height = Math.max(leftHeight, rightHeight) + labelHeight;
  const labelSvg = Buffer.from(`
    <svg width="${width}" height="${labelHeight}" xmlns="http://www.w3.org/2000/svg">
      <rect width="100%" height="100%" fill="#111"/>
      <text x="14" y="23" fill="#f4f4f0" font-family="Arial, sans-serif" font-size="16">${escapeXml(leftLabel)}</text>
      <text x="${leftWidth + gap + 14}" y="23" fill="#f4f4f0" font-family="Arial, sans-serif" font-size="16">${escapeXml(rightLabel)}</text>
    </svg>
  `);

  await sharp({
    create: {
      width,
      height,
      channels: 4,
      background: "#000000",
    },
  })
    .composite([
      { input: labelSvg, left: 0, top: 0 },
      { input: originalPath, left: 0, top: labelHeight },
      { input: webPath, left: leftWidth + gap, top: labelHeight },
    ])
    .png()
    .toFile(outputPath);
}

async function renderRegionCrops({ originalPath, webPath, webStatePath, outputDir }) {
  const webState = await readJson(webStatePath);
  const [originalMeta, webMeta] = await Promise.all([sharp(originalPath).metadata(), sharp(webPath).metadata()]);
  const dimensions = {
    width: Math.min(originalMeta.width ?? 1024, webMeta.width ?? 1024),
    height: Math.min(originalMeta.height ?? 768, webMeta.height ?? 768),
  };
  const regions = buildCropRegions(webState, dimensions);
  const crops = {};

  for (const [name, rect] of Object.entries(regions)) {
    if (!rect) continue;
    const nativePath = path.join(outputDir, `${name}-native.png`);
    const webCropPath = path.join(outputDir, `${name}-web.png`);
    await sharp(originalPath).extract(rect).png().toFile(nativePath);
    await sharp(webPath).extract(rect).png().toFile(webCropPath);
    crops[name] = {
      rect,
      nativePath,
      webPath: webCropPath,
    };
  }

  return crops;
}

function buildCropRegions(webState, dimensions) {
  const hudTop = clampRectNumber(webState.hud?.top, 0, dimensions.height) ?? 616;
  const hudHeight = Math.max(1, dimensions.height - hudTop);
  return {
    "world": makeCropRect(0, 0, dimensions.width, hudTop, dimensions),
    "hud-full": rectFromState(webState.hud, dimensions),
    "hud-left": makeCropRect(0, hudTop, 230, hudHeight, dimensions),
    "hud-belt": makeCropRect(230, hudTop, 240, 40, dimensions),
    "hud-right-controls": makeCropRect(900, hudTop + 34, 124, 68, dimensions),
    "hud-right-status": makeCropRect(900, hudTop + 96, 124, 56, dimensions),
    "hud-bottom-center": makeCropRect(230, hudTop + 108, 670, 44, dimensions),
    "minimap": rectFromState(webState.miniMap, dimensions),
    "chat": rectFromState(webState.chat, dimensions),
  };
}

function rectFromState(rect, dimensions) {
  if (!rect) return null;
  return makeCropRect(rect.left, rect.top, rect.width, rect.height, dimensions);
}

function makeCropRect(left, top, width, height, dimensions) {
  const x = Math.max(0, Math.floor(Number(left) || 0));
  const y = Math.max(0, Math.floor(Number(top) || 0));
  const w = Math.max(1, Math.floor(Number(width) || 1));
  const h = Math.max(1, Math.floor(Number(height) || 1));
  if (x >= dimensions.width || y >= dimensions.height) return null;
  return {
    left: x,
    top: y,
    width: Math.max(1, Math.min(w, dimensions.width - x)),
    height: Math.max(1, Math.min(h, dimensions.height - y)),
  };
}

function clampRectNumber(value, min, max) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return null;
  return Math.max(min, Math.min(max, parsed));
}

async function readJson(filePath) {
  return JSON.parse((await fs.readFile(filePath, "utf8")).replace(/^\uFEFF/, ""));
}

function renderReadme(summary) {
  const visualMarkdown =
    summary.visualReport.markdownPath ?? summary.visualReport.mdPath ?? `${samplePrefix}-visual-score.md`;
  return `# Crystal/Web Same-Scene Evidence Pack

Generated: ${summary.generatedAt}

Target: map ${summary.target.map} @ ${summary.target.x},${summary.target.y}

## Files

- Native Crystal screenshot: ${path.basename(summary.native.imagePath)}
- Native account state: ${summary.native.accountStatePath ? path.basename(summary.native.accountStatePath) : "not captured"}
- Web screenshot: ${path.basename(summary.web.screenshotPath)}
- Web state: ${path.basename(summary.web.statePath)}
- Web account sync: ${summary.web.accountStateSync?.syncSummaryPath ? path.basename(summary.web.accountStateSync.syncSummaryPath) : "not enabled"}
- Web QA state payload: ${summary.web.accountStateSync?.qaStatePath ? path.basename(summary.web.accountStateSync.qaStatePath) : "not enabled"}
- Side-by-side: ${path.basename(summary.sideBySidePath)}
- Region crops: ${Object.keys(summary.cropSet ?? {}).length ? "native/web crop pairs generated in this folder" : "not generated"}
- Visual score: ${path.basename(visualMarkdown)}
- Summary JSON: ${path.basename(summary.summaryPath)}

## Notes

The native Crystal window is captured from the currently running \`Legend of Mir 2\` client. The Web scene is positioned through the token-gated QA control path when \`MIR2_QA_CONTROL_TOKEN\` is configured, then verified by waiting until the Web state reports the target map and coordinate.
`;
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
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function timestamp() {
  const date = new Date();
  const pad = (value) => String(value).padStart(2, "0");
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
}

function escapeXml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
