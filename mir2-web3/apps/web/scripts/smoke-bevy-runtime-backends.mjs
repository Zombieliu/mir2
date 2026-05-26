import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_BASE_URL = "http://127.0.0.1:13010";
const DEFAULT_OUTPUT_DIR = path.resolve(REPO_ROOT, "docs", "generated", "player-qa", "bevy-runtime-backends");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

const args = parseArgs(process.argv.slice(2));
const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL;
const outputDir = path.resolve(args.output ?? process.env.MIR2_BEVY_BACKEND_OUTPUT ?? DEFAULT_OUTPUT_DIR);
const runId = args.runId ?? new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
const prefix = args.prefix ?? `bevy-runtime-backends-${runId}`;
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9900 + (process.pid % 500));
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const waitTimeoutMs = numberArg(args.waitTimeoutMs ?? process.env.MIR2_BEVY_BACKEND_WAIT_MS, 45_000);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
    this.consoleWarnings = [];
    this.responses = [];
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
      const url = String(response?.url ?? "");
      if (url.includes("/bevy-runtime/")) {
        this.responses.push({
          url,
          status: response.status,
          fromDiskCache: Boolean(response.fromDiskCache),
          fromServiceWorker: Boolean(response.fromServiceWorker),
          encodedDataLength: response.encodedDataLength ?? 0,
        });
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
  const userDataDir = path.join(os.tmpdir(), `mir2-bevy-backends-${process.pid}-${Date.now()}`);
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
    client = new CdpClient(target.webSocketDebuggerUrl);
    await client.connect();
    await client.send("Page.enable");
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await setViewport(client, DEFAULT_VIEWPORT);

    const scenarios = [
      { name: "default", query: "" },
      { name: "force-webgl2", query: "bevyBackend=webgl2&bevyEntities=1&bevyAtlas=1" },
      { name: "force-webgpu", query: "bevyBackend=webgpu&bevyEntities=1&bevyAtlas=1" },
      { name: "raw-webgl2-probe", path: "/qa/webgl2-entity-renderer", rawWebGl2Probe: true },
    ];
    const results = [];
    for (const scenario of scenarios) {
      results.push(await runScenario(client, scenario));
    }

    const allConsoleErrors = results.flatMap((result) => result.consoleErrors);
    const allConsoleWarnings = results.flatMap((result) => result.consoleWarnings);
    const allRuntimeResponses = results.flatMap((result) => result.runtimeResponses);
    const report = {
      ok: false,
      runId,
      baseUrl,
      outputDir,
      scenarios: results,
      consoleErrors: allConsoleErrors,
      criticalConsoleErrors: allConsoleErrors.filter(isCriticalConsoleError),
      consoleWarnings: allConsoleWarnings,
      sampledRuntimeResponses: allRuntimeResponses.slice(-80),
    };
    report.assertions = {
      scenariosLoaded: results
        .filter((result) => result.name !== "raw-webgl2-probe")
        .every((result) => result.assertions.runtimeDebugPresent),
      runtimeBackendsStayedHealthy: results.every((result) => result.assertions.noCriticalConsoleErrors),
      packageFetchesSucceeded: results.every((result) => result.assertions.packageFetchSucceeded),
      defaultPrefersWebGpuOrFallsBack: Boolean(results.find((result) => result.name === "default")?.assertions.prefersWebGpuOrFallsBack),
      forcedWebGl2UsesWebGl2Package: Boolean(results.find((result) => result.name === "force-webgl2")?.assertions.usesRequestedWebGl2),
      forcedWebGpuUsesWebGpuOrFallsBack: Boolean(results.find((result) => result.name === "force-webgpu")?.assertions.usesRequestedWebGpuOrFallsBack),
      rawWebGl2ProbeRendered: Boolean(results.find((result) => result.name === "raw-webgl2-probe")?.assertions.rawWebGl2ProbeRendered),
      noCriticalConsoleErrors: report.criticalConsoleErrors.length === 0,
    };
    report.ok = Object.values(report.assertions).every(Boolean);

    const reportPath = path.join(outputDir, `${prefix}.json`);
    const latestPath = path.join(outputDir, "latest-bevy-runtime-backends.json");
    await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
    await fs.writeFile(latestPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");

    console.log(JSON.stringify({ ok: report.ok, reportPath, latestPath, assertions: report.assertions }, null, 2));
    process.exitCode = report.ok ? 0 : 1;
  } finally {
    client?.close();
    chrome.kill("SIGTERM");
    await fs.rm(userDataDir, { recursive: true, force: true }).catch(() => {});
  }
}

async function runScenario(client, scenario) {
  client.consoleErrors.length = 0;
  client.consoleWarnings.length = 0;
  client.responses.length = 0;
  const scenarioUrl = new URL(scenario.path ?? "/", baseUrl).toString();
  const url = withQuery(scenarioUrl, scenario.query);
  await client.send("Page.navigate", { url });
  if (scenario.rawWebGl2Probe) {
    await waitForRawWebGl2Probe(client, waitTimeoutMs);
  } else {
    await waitForRuntime(client, waitTimeoutMs);
  }
  await sleep(1_500);
  const snapshot = await client.evaluate(`(() => {
    const canvas = document.querySelector("#mir2-web3-canvas");
    const canvasStyle = canvas ? window.getComputedStyle(canvas) : null;
    const runtime = window.__mir2BevyRuntimeDebug ?? null;
    const renderer = window.__mir2BevyEntityRendererDebug ?? null;
    const webgl2Renderer = window.__mir2WebGl2EntityRendererDebug ?? null;
    return {
      href: window.location.href,
      runtime,
      renderer,
      webgl2Renderer,
      canvas: canvas ? {
        exists: true,
        className: canvas.className || "",
        width: canvas.clientWidth,
        height: canvas.clientHeight,
        opacity: canvasStyle?.opacity ?? null,
        display: canvasStyle?.display ?? null,
        pointerEvents: canvasStyle?.pointerEvents ?? null,
      } : { exists: false },
      runtimeResources: performance
        .getEntriesByType("resource")
        .map((entry) => entry.name)
        .filter((name) => name.includes("/bevy-runtime/")),
    };
  })()`);
  return {
    name: scenario.name,
    url,
    ...snapshot,
    consoleErrors: client.consoleErrors,
    consoleWarnings: client.consoleWarnings,
    runtimeResponses: client.responses,
    assertions: scenarioAssertions(scenario.name, snapshot, client),
  };
}

function scenarioAssertions(name, snapshot, client) {
  const runtime = snapshot.runtime;
  const selected = runtime?.selectedBackend ?? null;
  const compiled = runtime?.compiledBackend ?? null;
  const fallbackFrom = runtime?.fallbackFrom ?? null;
  const resources = snapshot.runtimeResources ?? [];
  const fetchedWebGpu = resources.some((entry) => entry.includes("/pkg-webgpu/"));
  const fetchedWebGl2 = resources.some((entry) => entry.includes("/pkg-webgl2/"));
  const runtimeDebugPresent = Boolean(runtime);
  const rawWebGl2Renderer = snapshot.webgl2Renderer ?? null;
  const compiledMatchesSelected = selected === null || compiled === null || selected === compiled;
  const packageFetchSucceeded = client.responses.every((response) => response.status >= 200 && response.status < 400);
  const noCriticalConsoleErrors = client.consoleErrors.filter(isCriticalConsoleError).length === 0;

  return {
    runtimeDebugPresent,
    compiledMatchesSelected,
    packageFetchSucceeded,
    noCriticalConsoleErrors,
    prefersWebGpuOrFallsBack:
      name !== "default" ||
      !runtimeDebugPresent ||
      (runtime.webgpuSupported
        ? selected === "webgpu" || (selected === "webgl2" && fallbackFrom === "webgpu")
        : selected === "webgl2" || !runtime.webgl2Supported),
    usesRequestedWebGl2:
      name !== "force-webgl2" ||
      (runtimeDebugPresent &&
        selected === "webgl2" &&
        compiled === "webgl2" &&
        fetchedWebGl2 &&
        !fetchedWebGpu &&
        noCriticalConsoleErrors),
    usesRequestedWebGpuOrFallsBack:
      name !== "force-webgpu" ||
      (runtimeDebugPresent &&
        (selected === "webgpu"
          ? compiled === "webgpu" && fetchedWebGpu && noCriticalConsoleErrors
          : selected === "webgl2" &&
            compiled === "webgl2" &&
            fetchedWebGl2 &&
            noCriticalConsoleErrors)),
    rawWebGl2ProbeRendered:
      name !== "raw-webgl2-probe" ||
      (Boolean(rawWebGl2Renderer) &&
        rawWebGl2Renderer.supported === true &&
        rawWebGl2Renderer.reason === "rendered" &&
        rawWebGl2Renderer.renderedLayers > 0 &&
        noCriticalConsoleErrors),
  };
}

async function waitForRuntime(client, timeoutMs) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const state = await client.evaluate(`(() => ({
      runtime: window.__mir2BevyRuntimeDebug ?? null,
      phaseText: document.body?.innerText ?? "",
      readyState: document.readyState
    }))()`);
    if (state.runtime) {
      return;
    }
    if (String(state.phaseText).includes("Bevy runtime skipped") || String(state.phaseText).includes("boot-error")) {
      return;
    }
    await sleep(250);
  }
  throw new Error(`Timed out waiting ${timeoutMs}ms for Bevy runtime debug state`);
}

async function waitForRawWebGl2Probe(client, timeoutMs) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const state = await client.evaluate(`(() => window.__mir2WebGl2EntityRendererDebug ?? null)()`);
    if (state?.reason === "rendered" && state.renderedLayers > 0) {
      return;
    }
    if (state?.reason === "error" || state?.reason === "no-webgl2") {
      throw new Error(`Raw WebGL2 renderer probe failed: ${JSON.stringify(state)}`);
    }
    await sleep(250);
  }
  throw new Error(`Timed out waiting ${timeoutMs}ms for raw WebGL2 renderer probe`);
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

function withQuery(url, query) {
  const parsed = new URL(url);
  if (query) {
    const params = new URLSearchParams(query);
    for (const [key, value] of params) {
      parsed.searchParams.set(key, value);
    }
  }
  return parsed.toString();
}

function isCriticalConsoleError(entry) {
  const text = entry.text ?? "";
  return !/favicon|ResizeObserver loop|WebGPU is not supported/i.test(text);
}

function findChromePath() {
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
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

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
