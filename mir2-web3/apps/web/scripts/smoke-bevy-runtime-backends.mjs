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

  try {
    await waitForChrome(debugPort);
    const scenarios = [
      { name: "default", query: "" },
      { name: "force-webgl2", query: "bevyBackend=webgl2&bevyEntities=1&bevyAtlas=1" },
      { name: "force-webgpu", query: "bevyBackend=webgpu&bevyEntities=1&bevyAtlas=1" },
      { name: "raw-webgl2-probe", path: "/qa/webgl2-entity-renderer", rawWebGl2Probe: true },
    ];
    const results = [];
    for (const scenario of scenarios) {
      results.push(await runScenarioInFreshTarget(debugPort, scenario));
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
      movementShadowApiAvailable: results
        .filter((result) => result.name !== "raw-webgl2-probe")
        .every((result) => result.assertions.movementShadowApiAvailable),
      presentationPoseSinkDeliveredMonotonicFrames: results
        .filter((result) => result.name !== "raw-webgl2-probe")
        .every((result) => result.assertions.presentationPoseSinkDeliveredMonotonicFrames),
      remoteMotionPresentationDrovePackedOffsets: results
        .filter((result) => result.name !== "raw-webgl2-probe")
        .every((result) => result.assertions.remoteMotionPresentationDrovePackedOffset),
      unifiedPresentationPoseDroveDomContract: results
        .filter((result) => result.name !== "raw-webgl2-probe")
        .every((result) => result.assertions.unifiedPresentationPoseDroveDomContract),
      localMotionShadowMatchesCurrentPose: results
        .filter((result) => result.name !== "raw-webgl2-probe")
        .every((result) => result.assertions.localMotionShadowMatchesCurrentPose),
      localMotionPresentationOwnsSelfPose: results
        .filter((result) => result.name !== "raw-webgl2-probe")
        .every((result) => result.assertions.localMotionPresentationOwnsSelfPose),
      localMotionPathMismatchFallsBack: results
        .filter((result) => result.name !== "raw-webgl2-probe")
        .every((result) => result.assertions.localMotionPathMismatchFallsBack),
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
    chrome.kill("SIGTERM");
    await fs.rm(userDataDir, { recursive: true, force: true }).catch(() => {});
  }
}

async function runScenarioInFreshTarget(debugPort, scenario) {
  const target = await createTarget(debugPort, "about:blank");
  const client = new CdpClient(target.webSocketDebuggerUrl);
  try {
    await client.connect();
    await client.send("Page.enable");
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await setViewport(client, DEFAULT_VIEWPORT);
    return await runScenario(client, scenario);
  } finally {
    client.close();
    await closeTarget(debugPort, target.id).catch(() => {});
  }
}

async function runScenario(client, scenario) {
  client.consoleErrors.length = 0;
  client.consoleWarnings.length = 0;
  client.responses.length = 0;
  const scenarioUrl = scenarioUrlWithBaseQuery(scenario.path ?? "/", baseUrl);
  const url = withQuery(scenarioUrl, scenario.query);
  await client.send("Page.navigate", { url });
  if (scenario.rawWebGl2Probe) {
    await waitForRawWebGl2Probe(client, waitTimeoutMs);
  } else {
    await waitForRuntime(client, waitTimeoutMs);
  }
  await sleep(1_500);
  const remoteMotionProbe = scenario.rawWebGl2Probe
    ? null
    : await runRemoteMotionPresentationProbe(client);
  const snapshot = await client.evaluate(`(() => {
    const canvas = document.querySelector("#mir2-web3-canvas");
    const canvasStyle = canvas ? window.getComputedStyle(canvas) : null;
    const runtime = window.__mir2BevyRuntimeDebug ?? null;
    const runtimeModule = window.__mir2BevyRuntime ?? null;
    const renderer = window.__mir2BevyEntityRendererDebug ?? null;
    const mapRenderer = window.__mir2BevyMapRendererDebug ?? null;
    const webgl2Renderer = window.__mir2WebGl2EntityRendererDebug ?? null;
    return {
      href: window.location.href,
      runtime,
      runtimeApi: {
        movementShadowPush: typeof runtimeModule?.pushMir2MovementShadowEvent === "function",
        movementShadowDiagnostics:
          typeof runtimeModule?.getMir2MovementShadowDiagnostics === "function",
        remoteMotionPresentationEnable:
          typeof runtimeModule?.setMir2RemoteMotionPresentationEnabled === "function",
        remoteMotionPresentationDiagnostics:
          typeof runtimeModule?.getMir2RemoteMotionPresentationDiagnostics === "function",
        presentationPoseEnable:
          typeof runtimeModule?.setMir2PresentationPoseEnabled === "function",
        presentationPoses:
          typeof runtimeModule?.getMir2PresentationPoses === "function",
        presentationPoseSink:
          typeof runtimeModule?.setMir2PresentationPoseSink === "function",
        presentationPoseSinkClear:
          typeof runtimeModule?.clearMir2PresentationPoseSink === "function",
        localMotionDiagnostics:
          typeof runtimeModule?.getMir2LocalMotionDiagnostics === "function",
        localMotionPresentationEnable:
          typeof runtimeModule?.setMir2LocalMotionPresentationEnabled === "function",
      },
      renderer,
      mapRenderer,
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
    remoteMotionProbe,
    consoleErrors: client.consoleErrors,
    consoleWarnings: client.consoleWarnings,
    runtimeResponses: client.responses,
    assertions: scenarioAssertions(
      scenario.name,
      { ...snapshot, remoteMotionProbe },
      client,
    ),
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
    movementShadowApiAvailable:
      name === "raw-webgl2-probe" ||
      (snapshot.runtimeApi?.movementShadowPush === true &&
        snapshot.runtimeApi?.movementShadowDiagnostics === true &&
        snapshot.runtimeApi?.remoteMotionPresentationEnable === true &&
        snapshot.runtimeApi?.remoteMotionPresentationDiagnostics === true &&
        snapshot.runtimeApi?.presentationPoseEnable === true &&
        snapshot.runtimeApi?.presentationPoses === true &&
        snapshot.runtimeApi?.presentationPoseSink === true &&
        snapshot.runtimeApi?.presentationPoseSinkClear === true &&
        snapshot.runtimeApi?.localMotionDiagnostics === true &&
        snapshot.runtimeApi?.localMotionPresentationEnable === true),
    presentationPoseSinkDeliveredMonotonicFrames:
      name === "raw-webgl2-probe" ||
      (snapshot.remoteMotionProbe?.poseSink?.count > 0 &&
        snapshot.remoteMotionProbe?.poseSink?.strictlyIncreasing === true &&
        snapshot.remoteMotionProbe?.poseSink?.parseErrorCount === 0),
    remoteMotionPresentationDrovePackedOffset:
      name === "raw-webgl2-probe" ||
      (snapshot.remoteMotionProbe?.mismatch?.targetMismatchCount > 0 &&
        snapshot.remoteMotionProbe?.matched?.offsetMatchCount > 0 &&
        snapshot.remoteMotionProbe?.matched?.decodeErrorCount === 0 &&
        snapshot.remoteMotionProbe?.matched?.pendingEventDropCount === 0 &&
        snapshot.remoteMotionProbe?.disabled?.enabled === false &&
        snapshot.remoteMotionProbe?.disabled?.entryCount === 0),
    unifiedPresentationPoseDroveDomContract:
      name === "raw-webgl2-probe" ||
      (snapshot.remoteMotionProbe?.matchedPoses?.bridgeEnabled === true &&
        snapshot.remoteMotionProbe?.matchedPoses?.rendererEnabled === true &&
        snapshot.remoteMotionProbe?.matchedPoses?.camera?.source === "localCommand" &&
        Math.abs(snapshot.remoteMotionProbe?.matchedPoses?.camera?.x ?? 0) > 0 &&
        snapshot.remoteMotionProbe?.matchedPoses?.entities?.some(
          (entry) => entry.objectId === "remote-motion-probe" && entry.source === "remotePacket",
        ) &&
        snapshot.remoteMotionProbe?.disabledPoses?.bridgeEnabled === false &&
        snapshot.remoteMotionProbe?.disabledPoses?.rendererEnabled === true &&
        snapshot.remoteMotionProbe?.disabledPoses?.camera?.source === "static" &&
        snapshot.remoteMotionProbe?.disabledPoses?.entities?.length === 0),
    localMotionShadowMatchesCurrentPose:
      name === "raw-webgl2-probe" ||
      (snapshot.remoteMotionProbe?.localMotion?.commandEventCount === 1 &&
        snapshot.remoteMotionProbe?.localMotion?.candidateMatchCount > 0 &&
        snapshot.remoteMotionProbe?.localMotion?.comparisonSampleCount > 0 &&
        snapshot.remoteMotionProbe?.localMotion?.comparisonMismatchCount === 0 &&
        snapshot.remoteMotionProbe?.localMotion?.maxAbsDeltaX === 0 &&
        snapshot.remoteMotionProbe?.localMotion?.maxAbsDeltaY === 0 &&
        snapshot.remoteMotionProbe?.localMotion?.pendingEventDropCount === 0 &&
        snapshot.remoteMotionProbe?.localMotion?.pendingCommandDropCount === 0 &&
        snapshot.remoteMotionProbe?.localMotion?.decodeErrorCount === 0),
    localMotionPresentationOwnsSelfPose:
      name === "raw-webgl2-probe" ||
      (snapshot.remoteMotionProbe?.localMotion?.presentationEnabled === true &&
        snapshot.remoteMotionProbe?.matchedPoses?.camera?.source === "localCommand" &&
        snapshot.remoteMotionProbe?.matchedPoses?.entities?.some(
          (entry) => entry.objectId === "local-motion-probe" && entry.source === "localCommand",
        )),
    localMotionPathMismatchFallsBack:
      name === "raw-webgl2-probe" ||
      (snapshot.remoteMotionProbe?.pathMismatchLocalMotion?.tsWindowPathMismatchCount > 0 &&
        snapshot.remoteMotionProbe?.pathMismatchPoses?.camera?.source === "selfWindow" &&
        snapshot.remoteMotionProbe?.pathMismatchPoses?.entities?.some(
          (entry) =>
            entry.objectId === "local-motion-probe" && entry.source === "snapshotWindow",
        )),
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

async function runRemoteMotionPresentationProbe(client) {
  return client.evaluate(`(async () => {
    const runtime = window.__mir2BevyRuntime;
    if (!runtime ||
        typeof runtime.setMir2EntityRenderState !== "function" ||
        typeof runtime.setMir2MapRenderState !== "function" ||
        typeof runtime.pushMir2MovementShadowEvent !== "function" ||
        typeof runtime.setMir2RemoteMotionPresentationEnabled !== "function" ||
        typeof runtime.getMir2RemoteMotionPresentationDiagnostics !== "function" ||
        typeof runtime.setMir2PresentationPoseEnabled !== "function" ||
        typeof runtime.getMir2PresentationPoses !== "function" ||
        typeof runtime.setMir2PresentationPoseSink !== "function" ||
        typeof runtime.clearMir2PresentationPoseSink !== "function" ||
        typeof runtime.getMir2LocalMotionDiagnostics !== "function" ||
        typeof runtime.setMir2LocalMotionPresentationEnabled !== "function" ||
        typeof runtime.setMir2SelfCameraMotion !== "function") {
      return { unsupported: true };
    }

    const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
    const read = () => JSON.parse(runtime.getMir2RemoteMotionPresentationDiagnostics());
    const readPoses = () => JSON.parse(runtime.getMir2PresentationPoses());
    const readLocalMotion = () => JSON.parse(runtime.getMir2LocalMotionDiagnostics());
    const sinkFrameIds = [];
    let sinkParseErrorCount = 0;
    runtime.setMir2PresentationPoseSink((json) => {
      try {
        const frameId = JSON.parse(json)?.frameId;
        if (Number.isSafeInteger(frameId)) sinkFrameIds.push(frameId);
        else sinkParseErrorCount += 1;
      } catch {
        sinkParseErrorCount += 1;
      }
    });
    const state = (gridX) => JSON.stringify({
      enabled: true,
      stageWidth: 1024,
      stageHeight: 768,
      centerX: 11,
      centerY: 20,
      atlases: [],
      entities: [
        {
          objectId: "remote-motion-probe",
          dead: false,
          gridX,
          gridY: 10,
          layers: [],
        },
        {
          objectId: "local-motion-probe",
          dead: false,
          isSelf: true,
          gridX: 11,
          gridY: 20,
          layers: [],
        },
      ],
    });
    const mapState = JSON.stringify({
      enabled: true,
      stageWidth: 1024,
      stageHeight: 768,
      revision: 1,
      centerX: 11,
      centerY: 20,
      atlases: [],
      tiles: [],
      standaloneTiles: [],
    });

    runtime.setMir2RemoteMotionPresentationEnabled(true);
    runtime.setMir2PresentationPoseEnabled(true);
    runtime.setMir2LocalMotionPresentationEnabled(true);
    runtime.setMir2MapRenderState(mapState);
    const startedAt = Date.now();
    runtime.setMir2SelfCameraMotion(9, 20, 11, 20, startedAt, startedAt + 600);
    runtime.pushMir2MovementShadowEvent(JSON.stringify({
      type: "reset",
      atMs: startedAt,
      objectId: "local-motion-probe",
      x: 10,
      y: 20,
      direction: "Right",
    }));
    runtime.pushMir2MovementShadowEvent(JSON.stringify({
      type: "commandSent",
      atMs: startedAt,
      direction: "Right",
      mode: "walk",
      fromX: 10,
      fromY: 20,
      toX: 11,
      toY: 20,
    }));
    runtime.setMir2EntityRenderState(state(10));
    runtime.pushMir2MovementShadowEvent(JSON.stringify({
      type: "remoteMotion",
      atMs: Date.now(),
      packet: "ObjectWalk",
      objectId: "remote-motion-probe",
      fromX: 10,
      fromY: 10,
      toX: 11,
      toY: 10,
      direction: "Right",
      mode: "walk",
    }));
    await wait(120);
    const mismatch = read();
    runtime.setMir2EntityRenderState(state(11));
    await wait(35);
    const pathMismatchPoses = readPoses();
    const pathMismatchLocalMotion = readLocalMotion();

    runtime.setMir2SelfCameraMotion(10, 20, 11, 20, startedAt, startedAt + 600);

    for (let index = 0; index < 6; index += 1) {
      runtime.setMir2EntityRenderState(state(11));
      await wait(35);
    }
    const matched = read();
    const matchedPoses = readPoses();
    const localMotion = readLocalMotion();
    runtime.setMir2RemoteMotionPresentationEnabled(false);
    runtime.setMir2LocalMotionPresentationEnabled(false);
    runtime.setMir2PresentationPoseEnabled(false);
    runtime.setMir2SelfCameraMotion(0, 0, 0, 0, 0, 0);
    await wait(70);
    const disabled = read();
    const disabledPoses = readPoses();
    runtime.clearMir2PresentationPoseSink();
    const poseSink = {
      count: sinkFrameIds.length,
      strictlyIncreasing: sinkFrameIds.every(
        (frameId, index) => index === 0 || frameId > sinkFrameIds[index - 1],
      ),
      parseErrorCount: sinkParseErrorCount,
      firstFrameId: sinkFrameIds[0] ?? null,
      lastFrameId: sinkFrameIds.at(-1) ?? null,
    };
    return {
      unsupported: false,
      mismatch,
      matched,
      disabled,
      matchedPoses,
      disabledPoses,
      localMotion,
      pathMismatchPoses,
      pathMismatchLocalMotion,
      poseSink,
    };
  })()`);
}

async function waitForRuntime(client, timeoutMs) {
  const startedAt = Date.now();
  let lastState = null;
  while (Date.now() - startedAt < timeoutMs) {
    const state = await client.evaluate(`(() => ({
      runtime: window.__mir2BevyRuntimeDebug ?? null,
      phaseText: document.body?.innerText ?? "",
      readyState: document.readyState,
      hasCanvas: Boolean(document.querySelector("#mir2-web3-canvas")),
      overlayText: (document.querySelector("[data-nextjs-dialog], .vite-error-overlay, #webpack-dev-server-client-overlay")?.textContent ?? "").slice(0, 1200),
      href: location.href
    }))()`);
    lastState = state;
    if (state.runtime) {
      return;
    }
    if (String(state.phaseText).includes("Bevy runtime skipped") || String(state.phaseText).includes("boot-error")) {
      return;
    }
    await sleep(250);
  }
  throw new Error(
    `Timed out waiting ${timeoutMs}ms for Bevy runtime debug state: ${JSON.stringify({
      href: lastState?.href ?? null,
      readyState: lastState?.readyState ?? null,
      hasCanvas: lastState?.hasCanvas ?? null,
      overlayText: lastState?.overlayText ?? "",
      phaseText: String(lastState?.phaseText ?? "").slice(0, 1200),
    })}`,
  );
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

async function closeTarget(port, targetId) {
  if (!targetId) return;
  await fetch(`http://127.0.0.1:${port}/json/close/${encodeURIComponent(targetId)}`);
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

function scenarioUrlWithBaseQuery(scenarioPath, base) {
  const baseParsed = new URL(base);
  const parsed = new URL(scenarioPath, baseParsed);
  for (const [key, value] of baseParsed.searchParams) {
    if (!parsed.searchParams.has(key)) {
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
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
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
