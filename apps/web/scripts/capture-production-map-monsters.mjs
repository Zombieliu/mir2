import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const DEFAULT_BASE_URL = "https://mir2.obelisk.build";
const DEFAULT_OUTPUT_DIR = path.join(REPO_ROOT, "docs", "generated", "player-qa", "production-map-monsters");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

const args = parseArgs(process.argv.slice(2));
const baseUrl = normalizeBaseUrl(args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL);
const outputDir = path.resolve(args.output ?? process.env.MIR2_MAP_MONSTER_CAPTURE_OUTPUT ?? DEFAULT_OUTPUT_DIR);
const runId = args.prefix ?? `prod-map-monsters-${new Date().toISOString().replace(/[:.]/g, "-")}`;
const limitPerMap = numberArg(args.limitPerMap ?? process.env.MIR2_MAP_MONSTER_LIMIT_PER_MAP, 3);
const limit = optionalNumberArg(args.limit ?? process.env.MIR2_MAP_MONSTER_CAPTURE_LIMIT);
const offset = numberArg(args.offset ?? process.env.MIR2_MAP_MONSTER_CAPTURE_OFFSET, 0);
const sceneIndexes = parseSceneIndexes(args.sceneIndexes ?? process.env.MIR2_MAP_MONSTER_CAPTURE_SCENE_INDEXES);
const concurrency = Math.max(1, Math.min(numberArg(args.concurrency ?? process.env.MIR2_MAP_MONSTER_CAPTURE_CONCURRENCY, 3), 12));
const settleMs = numberArg(args.settleMs ?? process.env.MIR2_MAP_MONSTER_CAPTURE_SETTLE_MS, 200);
const waitTimeoutMs = numberArg(args.waitTimeoutMs ?? process.env.MIR2_MAP_MONSTER_CAPTURE_WAIT_MS, 45_000);
const imageWaitTimeoutMs = numberArg(
  args.imageWaitTimeoutMs ?? process.env.MIR2_MAP_MONSTER_CAPTURE_IMAGE_WAIT_MS,
  waitTimeoutMs,
);
const allowPendingImages = booleanArg(
  args.allowPendingImages ?? process.env.MIR2_MAP_MONSTER_CAPTURE_ALLOW_PENDING_IMAGES,
  false,
);
const fulfillOriginalMapFromPublic = booleanArg(
  args.fulfillOriginalMapFromPublic ?? process.env.MIR2_MAP_MONSTER_CAPTURE_FULFILL_ORIGINAL_MAP_FROM_PUBLIC,
  false,
);
const sceneAttempts = Math.max(1, Math.min(numberArg(args.sceneAttempts ?? process.env.MIR2_MAP_MONSTER_CAPTURE_ATTEMPTS, 2), 5));
const strict = booleanArg(args.strict ?? process.env.MIR2_MAP_MONSTER_CAPTURE_STRICT, true);
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9600 + (process.pid % 700));

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
    this.networkFailures = [];
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (event) => this.handleMessage(event.data));
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }

  resetPageObservations() {
    this.consoleErrors = [];
    this.network404s = [];
    this.networkFailures = [];
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

    if (message.method === "Network.loadingFailed") {
      const params = message.params ?? {};
      const errorText = String(params.errorText ?? "");
      if (!errorText.includes("net::ERR_ABORTED")) {
        this.networkFailures.push({
          errorText,
          blockedReason: params.blockedReason ?? null,
          canceled: Boolean(params.canceled),
        });
      }
    }

    if (message.method === "Fetch.requestPaused") {
      void this.handleFetchRequest(message.params).catch((error) => {
        this.consoleErrors.push({
          source: "fetch",
          text: error instanceof Error ? error.message : String(error),
        });
      });
    }
  }

  async handleFetchRequest(params) {
    const requestId = params?.requestId;
    if (!requestId) return;

    const localPath = localPublicAssetPath(params?.request?.url);
    if (!localPath) {
      await this.send("Fetch.continueRequest", { requestId });
      return;
    }

    try {
      const body = await fs.readFile(localPath);
      await this.send("Fetch.fulfillRequest", {
        requestId,
        responseCode: 200,
        responseHeaders: [
          { name: "Content-Type", value: contentTypeForPath(localPath) },
          { name: "Cache-Control", value: "public, max-age=31536000, immutable" },
          { name: "Access-Control-Allow-Origin", value: "*" },
        ],
        body: body.toString("base64"),
      });
    } catch {
      await this.send("Fetch.continueRequest", { requestId });
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
  const sceneList = await fetchSceneList();
  const selectedScenes = sceneIndexes === null
    ? sceneList.scenes.slice(offset, limit === null ? undefined : offset + limit)
    : sceneIndexes.map((sceneIndex) => {
        const scene = sceneList.scenes[sceneIndex];
        if (!scene) throw new Error(`Unknown QA scene index ${sceneIndex}; sceneCount=${sceneList.sceneCount}`);
        return scene;
      });
  const runDir = path.join(outputDir, runId);
  await fs.mkdir(runDir, { recursive: true });

  const chrome = await launchChrome();
  const results = [];
  const failures = [];
  const workerCount = Math.min(concurrency, selectedScenes.length);
  const chunkSize = Math.max(1, Math.ceil(selectedScenes.length / workerCount));

  try {
    await Promise.all(
      Array.from({ length: workerCount }, async (_, workerIndex) => {
        const workerScenes = selectedScenes.slice(workerIndex * chunkSize, (workerIndex + 1) * chunkSize);
        const client = await createClient(workerIndex);
        try {
          for (const scene of workerScenes) {
            const absoluteIndex = scene.sceneIndex;
            const startedAt = Date.now();
            try {
              const result = await captureSceneWithRetries(client, scene, runDir);
              results.push(result);
              const elapsedMs = Date.now() - startedAt;
              console.log(
                JSON.stringify({
                  ok: true,
                  worker: workerIndex,
                  scene: scene.id,
                  map: scene.mapFileName,
                  monster: scene.selectedRespawn.monsterName,
                  elapsedMs,
                  screenshot: path.relative(REPO_ROOT, result.screenshotPath),
                }),
              );
            } catch (error) {
              const failure = {
                ok: false,
                worker: workerIndex,
                scene: scene.id,
                sceneIndex: absoluteIndex,
                map: scene.mapFileName,
                monster: scene.selectedRespawn.monsterName,
                error: error instanceof Error ? error.message : String(error),
              };
              failures.push(failure);
              console.error(JSON.stringify(failure));
            }
          }
        } finally {
          client.close();
        }
      }),
    );
  } finally {
    await stopChrome(chrome);
    await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => undefined);
  }

  results.sort((a, b) => a.sceneIndex - b.sceneIndex);
  failures.sort((a, b) => a.sceneIndex - b.sceneIndex);
  const summary = {
    ok: failures.length === 0,
    baseUrl,
    runId,
    limitPerMap: sceneList.limitPerMap,
    sourceMapCount: sceneList.sourceMapCount,
    respawnMapCount: sceneList.respawnMapCount,
    positiveRespawnRowCount: sceneList.positiveRespawnRowCount,
    totalSceneCount: sceneList.sceneCount,
    requestedSceneCount: selectedScenes.length,
    capturedSceneCount: results.length,
    failureCount: failures.length,
    outputDir: runDir,
    failures,
    results: results.map((result) => ({
      sceneId: result.sceneId,
      sceneIndex: result.sceneIndex,
      mapFileName: result.mapFileName,
      mapTitle: result.mapTitle,
      selectedMonster: result.selectedMonster,
      center: result.center,
      visibleRespawnCount: result.visibleRespawnCount,
      mapSpriteCount: result.mapSpriteCount,
      imageCount: result.imageCount,
      pendingImageCount: result.pendingImages.length,
      imagesComplete: result.imagesComplete,
      brokenImageCount: result.brokenImages.length,
      network404Count: result.network404s.length,
      networkFailureCount: result.networkFailures.length,
      consoleErrorCount: result.consoleErrors.length,
      screenshot: path.relative(REPO_ROOT, result.screenshotPath).replaceAll("\\", "/"),
      state: path.relative(REPO_ROOT, result.statePath).replaceAll("\\", "/"),
    })),
  };
  const summaryPath = path.join(runDir, "summary.json");
  await fs.writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
  console.log(JSON.stringify({ ...summary, summaryPath }, null, 2));

  if (strict && failures.length > 0) {
    throw new Error(`Map monster capture failed for ${failures.length} scene(s).`);
  }
}

async function captureSceneWithRetries(client, scene, runDir) {
  let lastError;
  for (let attempt = 1; attempt <= sceneAttempts; attempt += 1) {
    try {
      return await captureScene(client, scene, runDir);
    } catch (error) {
      lastError = error;
      if (attempt >= sceneAttempts) break;
      console.warn(
        JSON.stringify({
          retry: true,
          scene: scene.id,
          map: scene.mapFileName,
          monster: scene.selectedRespawn.monsterName,
          attempt,
          maxAttempts: sceneAttempts,
          error: error instanceof Error ? error.message : String(error),
        }),
      );
      await delay(1_000 * attempt);
    }
  }
  throw lastError;
}

async function fetchSceneList() {
  const url = new URL("/api/qa/map-monster-scenes", baseUrl);
  url.searchParams.set("limitPerMap", String(limitPerMap));
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Scene list request failed: HTTP ${response.status} ${await response.text()}`);
  }
  return response.json();
}

async function createClient(workerIndex) {
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/new?about:blank`, { method: "PUT" });
  if (!response.ok) throw new Error(`Chrome target creation failed: ${response.status}`);
  const target = await response.json();
  const client = new CdpClient(target.webSocketDebuggerUrl);
  await client.connect();
  await client.send("Runtime.enable");
  await client.send("Log.enable");
  await client.send("Network.enable");
  await client.send("Page.enable");
  await client.send("Network.setCacheDisabled", { cacheDisabled: false });
  if (fulfillOriginalMapFromPublic) {
    await client.send("Fetch.enable", {
      patterns: [
        { urlPattern: "*://*/original-map/*", requestStage: "Request" },
        { urlPattern: "*://*/generated/original-map-blend/*", requestStage: "Request" },
      ],
    });
  }
  await setViewport(client, DEFAULT_VIEWPORT);
  await client.send("Runtime.evaluate", {
    expression: `document.title = ${JSON.stringify(`map-monster-worker-${workerIndex}`)}`,
    returnByValue: true,
  });
  return client;
}

async function captureScene(client, scene, runDir) {
  client.resetPageObservations();
  const url = new URL("/qa/map-monsters", baseUrl);
  url.searchParams.set("limitPerMap", String(limitPerMap));
  url.searchParams.set("scene", scene.id);
  await navigate(client, url.toString());
  await waitUntil(
    client,
    `document.querySelector('[data-qa-map-monster-ready="true"][data-scene-id="${escapeJsString(scene.id)}"]')`,
    `QA scene ${scene.id}`,
    waitTimeoutMs,
  );
  await waitUntil(
    client,
    `
      (() => {
        const root = document.querySelector('[data-qa-map-monster-ready="true"][data-scene-id="${escapeJsString(scene.id)}"]');
        const expected = Number(root?.getAttribute("data-visible-respawn-count") ?? -1);
        return expected > 0 && document.querySelectorAll("[data-respawn-index]").length >= expected;
      })()
    `,
    `respawn markers for ${scene.id}`,
    waitTimeoutMs,
  );
  let imagesComplete = true;
  try {
    await waitUntil(
      client,
      `
        (() => {
          const images = Array.from(document.images);
          return images.length > 0 && images.every((image) => image.complete);
        })()
      `,
      `images for ${scene.id}`,
      imageWaitTimeoutMs,
    );
  } catch (error) {
    imagesComplete = false;
    if (!allowPendingImages) throw error;
    console.warn(
      JSON.stringify({
        pendingImages: true,
        scene: scene.id,
        map: scene.mapFileName,
        monster: scene.selectedRespawn.monsterName,
        timeoutMs: imageWaitTimeoutMs,
        error: error instanceof Error ? error.message : String(error),
      }),
    );
  }
  await delay(settleMs);

  const state = await readPageState(client);
  const safeName = safeFileName(`${String(scene.sceneIndex + 1).padStart(4, "0")}-${scene.id}`);
  const screenshotPath = path.join(runDir, `${safeName}.png`);
  const statePath = path.join(runDir, `${safeName}.json`);
  const screenshot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(screenshotPath, Buffer.from(screenshot.data, "base64"));

  const result = {
    ...state,
    sceneId: scene.id,
    sceneIndex: scene.sceneIndex,
    mapFileName: scene.mapFileName,
    mapTitle: scene.mapTitle,
    selectedMonster: scene.selectedRespawn.monsterName,
    center: scene.center,
    requestedUrl: url.toString(),
    imagesComplete,
    network404s: [...new Set(client.network404s)],
    networkFailures: client.networkFailures,
    consoleErrors: client.consoleErrors,
    screenshotPath,
    statePath,
  };
  await fs.writeFile(statePath, `${JSON.stringify(result, null, 2)}\n`);

  if (result.network404s.length > 0) {
    throw new Error(`Network 404s: ${result.network404s.slice(0, 5).join(", ")}`);
  }
  if (result.brokenImages.length > 0) {
    throw new Error(`Broken images: ${result.brokenImages.slice(0, 5).join(", ")}`);
  }
  if (result.mapSpriteCount === 0) {
    throw new Error("No map sprites rendered.");
  }
  if (result.visibleRespawnCount === 0) {
    throw new Error("No respawn markers rendered.");
  }
  return result;
}

async function navigate(client, url) {
  await client.send("Page.navigate", { url });
  await delay(50);
}

async function readPageState(client) {
  return client.evaluate(`
    (() => {
      const root = document.querySelector('[data-qa-map-monster-ready="true"]');
      const images = Array.from(document.images);
      const pendingImages = images
        .filter((image) => !image.complete)
        .map((image) => image.currentSrc || image.src);
      const brokenImages = images
        .filter((image) => image.complete && image.naturalWidth === 0)
        .map((image) => image.currentSrc || image.src);
      return {
        sceneId: root?.getAttribute("data-scene-id") ?? null,
        sceneIndex: Number(root?.getAttribute("data-scene-index") ?? -1),
        sceneCount: Number(root?.getAttribute("data-scene-count") ?? -1),
        mapFileName: root?.getAttribute("data-map-file-name") ?? null,
        mapTitle: root?.getAttribute("data-map-title") ?? null,
        center: {
          x: Number(root?.getAttribute("data-center-x") ?? 0),
          y: Number(root?.getAttribute("data-center-y") ?? 0),
        },
        visibleRespawnCount: document.querySelectorAll("[data-respawn-index]").length,
        mapSpriteCount: document.querySelectorAll("[data-map-sprite-kind]").length,
        imageCount: images.length,
        pendingImages,
        brokenImages,
        markerLabels: Array.from(document.querySelectorAll("[data-monster-name]")).slice(0, 24).map((node) => ({
          monsterName: node.getAttribute("data-monster-name"),
          respawnIndex: Number(node.getAttribute("data-respawn-index")),
          count: Number(node.getAttribute("data-respawn-count")),
          spread: Number(node.getAttribute("data-respawn-spread")),
        })),
      };
    })()
  `);
}

async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-map-monsters-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      headed ? "" : "--headless=new",
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-background-networking",
      `--window-size=${DEFAULT_VIEWPORT.width},${DEFAULT_VIEWPORT.height}`,
      "about:blank",
    ].filter(Boolean),
    { stdio: "ignore" },
  );
  chrome.userDataDir = userDataDir;
  await waitForChrome();
  return chrome;
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
  chrome.kill("SIGTERM");
  await new Promise((resolve) => {
    const timer = setTimeout(resolve, 2_000);
    chrome.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
  });
  if (!chrome.killed) chrome.kill("SIGKILL");
}

function findChromePath() {
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  ];
  return candidates.find((candidate) => {
    try {
      return Boolean(candidate) && requireFsExists(candidate);
    } catch {
      return false;
    }
  });
}

function requireFsExists(candidate) {
  return Boolean(candidate) && existsSync(candidate);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (next && !next.startsWith("--")) {
      parsed[key] = next;
      index += 1;
    } else {
      parsed[key] = "true";
    }
  }
  return parsed;
}

function normalizeBaseUrl(value) {
  const parsed = new URL(value);
  parsed.pathname = parsed.pathname.replace(/\/+$/, "");
  parsed.search = "";
  parsed.hash = "";
  return parsed.toString().replace(/\/$/, "");
}

function localPublicAssetPath(value) {
  if (!fulfillOriginalMapFromPublic) return null;
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    return null;
  }

  const relativePath = decodeURIComponent(parsed.pathname).replace(/^\/+/, "");
  if (
    !relativePath.startsWith("original-map/") &&
    !relativePath.startsWith("generated/original-map-blend/")
  ) {
    return null;
  }

  const publicRoot = path.join(WEB_ROOT, "public");
  const localPath = path.resolve(publicRoot, relativePath);
  return isPathInside(localPath, publicRoot) && existsSync(localPath) ? localPath : null;
}

function contentTypeForPath(filePath) {
  const ext = path.extname(filePath).toLowerCase();
  if (ext === ".png") return "image/png";
  if (ext === ".jpg" || ext === ".jpeg") return "image/jpeg";
  if (ext === ".gif") return "image/gif";
  if (ext === ".webp") return "image/webp";
  return "application/octet-stream";
}

function isPathInside(candidate, root) {
  const relative = path.relative(root, candidate);
  return relative === "" || (!!relative && !relative.startsWith("..") && !path.isAbsolute(relative));
}

function numberArg(value, fallback) {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function optionalNumberArg(value) {
  if (value === null || value === undefined || value === "") return null;
  const parsed = Number.parseInt(String(value), 10);
  return Number.isFinite(parsed) ? parsed : null;
}

function parseSceneIndexes(value) {
  if (value === null || value === undefined || value === "") return null;
  const indexes = String(value)
    .split(",")
    .map((part) => Number.parseInt(part.trim(), 10))
    .filter((index) => Number.isFinite(index));
  return [...new Set(indexes)];
}

function booleanArg(value, fallback) {
  if (value === null || value === undefined || value === "") return fallback;
  const normalized = String(value).toLowerCase();
  return normalized === "1" || normalized === "true" || normalized === "yes";
}

function safeFileName(value) {
  return value.replace(/[^a-z0-9._-]+/gi, "-").replace(/^-+|-+$/g, "").slice(0, 180);
}

function escapeJsString(value) {
  return String(value).replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

await main();
