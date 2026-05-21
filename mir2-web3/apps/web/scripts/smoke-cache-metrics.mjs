import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_BASE_URL = "http://127.0.0.1:13010";
const DEFAULT_OUTPUT_DIR = path.resolve(REPO_ROOT, "docs", "generated", "player-qa", "cache-metrics");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

const args = parseArgs(process.argv.slice(2));
const mode = args.mode ?? process.env.MIR2_CACHE_METRICS_MODE ?? "resource";
const playableMode = mode === "playable";
const baseUrl = buildCacheSmokeUrl(args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL, mode);
const outputDir = path.resolve(args.output ?? process.env.MIR2_CACHE_METRICS_OUTPUT ?? DEFAULT_OUTPUT_DIR);
const runId = args.runId ?? new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
const prefix = args.prefix ?? `cache-metrics-${runId}`;
const account = args.account ?? process.env.MIR2_CACHE_METRICS_ACCOUNT ?? "demo";
const password = args.password ?? process.env.MIR2_CACHE_METRICS_PASSWORD ?? "demo";
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CACHE_METRICS_CREATE_ACCOUNT, false);
const characterName = args.characterName ?? defaultCharacterName();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9800 + (process.pid % 500));
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const waitTimeoutMs = numberArg(args.waitTimeoutMs ?? process.env.MIR2_CACHE_METRICS_WAIT_MS, 90_000);
const coldFirstPlayableBudgetMs = numberArg(args.coldFirstPlayableBudgetMs ?? process.env.MIR2_COLD_FIRST_PLAYABLE_BUDGET_MS, 60_000);
const warmFirstPlayableBudgetMs = numberArg(args.warmFirstPlayableBudgetMs ?? process.env.MIR2_WARM_FIRST_PLAYABLE_BUDGET_MS, 20_000);
const maxPrewarmRequested = numberArg(args.maxPrewarmRequested ?? process.env.MIR2_CACHE_MAX_PREWARM_REQUESTS, 1_000);
const maxWarmCacheEntries = numberArg(args.maxWarmCacheEntries ?? process.env.MIR2_CACHE_MAX_WARM_ENTRIES, 2_500);
const maxWarmStorageUsageBytes = numberArg(
  args.maxWarmStorageUsageBytes ?? process.env.MIR2_CACHE_MAX_WARM_STORAGE_USAGE_BYTES,
  256 * 1024 * 1024,
);
const exerciseMaintenance = booleanArg(
  args.exerciseMaintenance ??
    args.exerciseReset ??
    process.env.MIR2_CACHE_METRICS_EXERCISE_MAINTENANCE ??
    process.env.MIR2_CACHE_METRICS_EXERCISE_RESET,
  false,
);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();

let createdPlayableAccount = false;

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
    this.consoleMessages = [];
    this.network404s = [];
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
      if ((type === "log" || type === "info" || type === "debug") && text.includes("[mir2-cache")) {
        this.consoleMessages.push({ source: "console", type, text });
        this.consoleMessages = this.consoleMessages.slice(-200);
      }
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
      if (!response?.url) return;
      if (response.status === 404 && !String(response.url).includes("favicon")) {
        this.network404s.push(response.url);
      }
      if (isCacheRelevantUrl(response.url)) {
        this.responses.push({
          url: response.url,
          status: response.status,
          fromDiskCache: Boolean(response.fromDiskCache),
          fromServiceWorker: Boolean(response.fromServiceWorker),
          encodedDataLength: response.encodedDataLength ?? 0,
          cacheControl: response.headers?.["cache-control"] ?? response.headers?.["Cache-Control"] ?? null,
          sceneCache: response.headers?.["x-mir2-scene-cache"] ?? response.headers?.["X-Mir2-Scene-Cache"] ?? null,
        });
        this.responses = this.responses.slice(-2500);
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
  const userDataDir = path.join(os.tmpdir(), `mir2-cache-metrics-${process.pid}-${Date.now()}`);
  const chrome = spawn(
    chromePath,
    [
      headed ? "" : "--headless=new",
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
      "--enable-features=NetworkService,NetworkServiceInProcess",
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
    await client.send("ServiceWorker.enable").catch(() => {});
    await setViewport(client, DEFAULT_VIEWPORT);

    const cold = await runMetricsPass(client, "cold", baseUrl);
    const warm = await runMetricsPass(client, "warm", baseUrl);
    const maintenance = exerciseMaintenance ? await exerciseCacheMaintenance(client, baseUrl) : null;

    const report = {
      ok: false,
      runId,
      mode,
      baseUrl,
      outputDir,
      cold,
      warm,
      maintenance,
      budgets: pickBudgets(),
      comparison: compareRuns(cold, warm),
      consoleErrors: client.consoleErrors,
      criticalConsoleErrors: client.consoleErrors.filter(isCriticalConsoleError),
      consoleWarnings: client.consoleWarnings,
      consoleMessages: client.consoleMessages,
      nonFaviconNetwork404s: client.network404s,
      sampledResponses: client.responses.slice(-80),
    };

    report.assertions = {
      coldMetricsPresent: Boolean(cold.metrics),
      warmMetricsPresent: Boolean(warm.metrics),
      warmCompletedPrewarm: warm.summary.prewarmRequested > 0 && warm.summary.prewarmOk === warm.summary.prewarmRequested,
      noPrewarmFailures: warm.summary.prewarmFailed === 0,
      warmCacheStoragePresent: warm.summary.cacheStorageEntryCount > 0,
      warmCacheStorageCachesPresent: warm.summary.cacheStorageCacheCount > 0,
      prewarmWithinBudget: warm.summary.prewarmRequested <= maxPrewarmRequested,
      warmCacheStorageEntriesWithinBudget: warm.summary.cacheStorageEntryCount <= maxWarmCacheEntries,
      warmStorageUsageWithinBudget:
        typeof warm.summary.storageUsageBytes !== "number" ||
        warm.summary.storageUsageBytes <= maxWarmStorageUsageBytes,
      noCriticalConsoleErrors: report.criticalConsoleErrors.length === 0,
      noNonFavicon404s: report.nonFaviconNetwork404s.length === 0,
    };
    if (hasCacheConsoleLogging(baseUrl)) {
      report.assertions.cacheConsoleLogsPresent = report.consoleMessages.some((entry) =>
        entry.text.includes("[mir2-cache-progress]"),
      );
    }
    if (exerciseMaintenance) {
      report.assertions.maintenanceLegacyCacheSeeded = Boolean(maintenance?.seeded?.hasLegacy);
      report.assertions.maintenanceLegacyCacheCleanedByVersion = Boolean(
        maintenance?.afterCleanup?.supported &&
          !maintenance.afterCleanup.cacheNames.some((name) => name.includes("legacy-smoke")),
      );
      report.assertions.maintenanceManualResetAvailable = Boolean(maintenance?.reset?.available);
      report.assertions.maintenanceManualResetClearedCaches = Boolean(
        maintenance?.afterReset?.supported && maintenance.afterReset.cacheNames.length === 0,
      );
    }
    if (playableMode) {
      report.assertions.coldFirstPlayablePresent = typeof cold.summary.firstPlayableMs === "number";
      report.assertions.warmFirstPlayablePresent = typeof warm.summary.firstPlayableMs === "number";
      report.assertions.coldFirstPlayableWithinBudget =
        typeof cold.summary.firstPlayableMs === "number" && cold.summary.firstPlayableMs <= coldFirstPlayableBudgetMs;
      report.assertions.warmFirstPlayableWithinBudget =
        typeof warm.summary.firstPlayableMs === "number" && warm.summary.firstPlayableMs <= warmFirstPlayableBudgetMs;
    }
    report.ok = Object.values(report.assertions).every(Boolean);

    const reportPath = path.join(outputDir, `${prefix}.json`);
    const latestPath = path.join(outputDir, "latest-cache-metrics.json");
    await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
    await fs.writeFile(latestPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");

    console.log(JSON.stringify({
      ok: report.ok,
      reportPath,
      mode,
      cold: pickSummary(cold.summary),
      warm: pickSummary(warm.summary),
      maintenance: maintenance ? pickMaintenance(maintenance) : null,
      comparison: report.comparison,
      assertions: report.assertions,
      budgets: pickBudgets(),
    }, null, 2));

    if (!report.ok) {
      process.exitCode = 1;
    }
  } finally {
    client?.close();
    chrome.kill();
    await fs.rm(userDataDir, { recursive: true, force: true }).catch(() => {});
  }
}

async function runMetricsPass(client, label, url) {
  const responseStart = client.responses.length;
  const startedAt = Date.now();
  const navigation = await client.send("Page.navigate", { url });
  if (navigation.errorText) {
    throw new Error(`Page navigation failed for ${url}: ${navigation.errorText}`);
  }
  await waitForPageLoad(client);
  if (playableMode) {
    await drivePlayableFlow(client);
    await waitForFirstPlayable(client, waitTimeoutMs);
  }
  await waitForMetricsReady(client, waitTimeoutMs);
  const metrics = await readMetrics(client);
  return {
    label,
    url,
    elapsedMs: Date.now() - startedAt,
    assetCache: await readAssetCache(client),
    metrics,
    summary: metrics?.summary ?? emptySummary(),
    responses: summarizeResponses(client.responses.slice(responseStart)),
  };
}

async function exerciseCacheMaintenance(client, url) {
  const seeded = await seedLegacyCache(client);
  const cleanupPass = await runMetricsPass(client, "maintenance-cleanup", url);
  const afterCleanup = await readMir2CacheNames(client);
  const reset = await resetAssetCache(client);
  const afterReset = await readMir2CacheNames(client);

  return {
    seeded,
    cleanupPass: {
      elapsedMs: cleanupPass.elapsedMs,
      assetCache: cleanupPass.assetCache,
      summary: cleanupPass.summary,
      responses: cleanupPass.responses,
    },
    afterCleanup,
    reset,
    afterReset,
  };
}

async function seedLegacyCache(client) {
  return client.evaluate(`
    (async () => {
      if (!("caches" in window)) {
        return { supported: false, cacheNames: [], hasLegacy: false };
      }
      const legacyName = "mir2-asset-cache-static-legacy-smoke";
      const cache = await caches.open(legacyName);
      await cache.put(
        new Request("/original-ui/__legacy-cache-smoke.txt"),
        new Response("legacy", {
          headers: {
            "content-type": "text/plain",
            "cache-control": "max-age=31536000"
          }
        })
      );
      const cacheNames = (await caches.keys())
        .filter((name) => name.startsWith("mir2-asset-cache"))
        .sort();
      return {
        supported: true,
        legacyName,
        cacheNames,
        hasLegacy: cacheNames.includes(legacyName)
      };
    })()
  `);
}

async function resetAssetCache(client) {
  return client.evaluate(`
    (async () => {
      if (typeof window.__mir2AssetCacheReset !== "function") {
        return { available: false, result: null, assetCache: window.__mir2AssetCache ?? null };
      }
      const result = await window.__mir2AssetCacheReset({ reload: false });
      return {
        available: true,
        result,
        assetCache: window.__mir2AssetCache ?? null,
        metrics: window.__mir2CacheMetrics?.snapshot?.() ?? null
      };
    })()
  `);
}

async function readMir2CacheNames(client) {
  return client.evaluate(`
    (async () => {
      if (!("caches" in window)) return { supported: false, cacheNames: [] };
      const cacheNames = (await caches.keys())
        .filter((name) => name.startsWith("mir2-asset-cache"))
        .sort();
      const cachesWithEntries = await Promise.all(cacheNames.map(async (name) => {
        const cache = await caches.open(name);
        return { name, entries: (await cache.keys()).length };
      }));
      return {
        supported: true,
        cacheNames,
        caches: cachesWithEntries,
        entryCount: cachesWithEntries.reduce((sum, entry) => sum + entry.entries, 0)
      };
    })()
  `);
}

async function drivePlayableFlow(client) {
  await waitUntil(
    async () => Boolean(await client.evaluate("Boolean(window.__mir2Stage5?.state)")),
    30_000,
    "stage5 bridge",
  );

  const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  const shouldCreateAccount = createAccount && !createdPlayableAccount;
  if (screen === "login") {
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    if (shouldCreateAccount) {
      await clickSelector(client, ".login-button.account button");
      await waitUntil(
        async () => (await client.evaluate("window.__mir2Stage5?.state?.wsState ?? null")) === "open",
        15_000,
        "account creation socket",
      );
      await delay(2000);
    }
    await clickSelector(client, ".login-button.ok button");
    await waitUntil(
      async () => (await client.evaluate("window.__mir2Stage5?.state?.screen ?? null")) === "select",
      30_000,
      `${account} login select screen`,
    );
  }

  const afterLoginScreen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  if (afterLoginScreen === "select" && shouldCreateAccount) {
    const created = await client.evaluate(`
      window.__mir2Stage5?.send?.(${JSON.stringify({
        type: "newCharacter",
        name: characterName,
        gender: "male",
        class: "warrior",
      })}) === true
    `);
    if (!created) throw new Error(`Failed to create playable smoke character ${characterName}`);
    await waitUntil(
      async () =>
        Boolean(
          await client.evaluate(`
            Array.isArray(window.__mir2Stage5?.state?.characters)
              && window.__mir2Stage5.state.characters.some((character) => character?.name === ${JSON.stringify(characterName)})
          `),
        ),
      15_000,
      `playable smoke character ${characterName}`,
    );
    createdPlayableAccount = true;
  }

  if (afterLoginScreen === "select") {
    const sent = await client.evaluate(`
      (() => {
        const state = window.__mir2Stage5?.state;
        const requestedName = ${JSON.stringify(characterName)};
        const character = requestedName
          ? state?.characters?.find((entry) => entry?.name === requestedName)
          : state?.characters?.[state?.selectedCharacterIndex ?? 0] ?? state?.characters?.[0];
        const fallback = state?.characters?.[state?.selectedCharacterIndex ?? 0] ?? state?.characters?.[0];
        return window.__mir2Stage5?.send?.({ type: "startGame", characterIndex: (character ?? fallback)?.index ?? 0 }) === true;
      })()
    `);
    if (!sent) {
      await clickSelector(client, ".select-action.start button");
    }
  }

  await waitUntil(
    async () =>
      Boolean(
        await client.evaluate(`
          (() => {
            const state = window.__mir2Stage5?.state;
            return state?.screen === "game" && Boolean(state?.player) && Boolean(state?.originalMapRegionSummary);
          })()
        `),
      ),
    60_000,
    "playable game stage",
  );
}

async function waitForFirstPlayable(client, timeoutMs) {
  try {
    await waitUntil(
      async () =>
        Boolean(
          await client.evaluate(`
            (() => (window.__mir2CacheMetrics?.snapshot?.().milestones ?? [])
              .some((milestone) => milestone.name === "firstPlayableFrame"))()
          `),
        ),
      timeoutMs,
      "first playable frame milestone",
    );
  } catch (error) {
    const diagnostics = await readFirstPlayableDiagnostics(client).catch((diagnosticError) => ({
      diagnosticError: diagnosticError instanceof Error ? diagnosticError.message : String(diagnosticError),
    }));
    throw new Error(`${error.message}\n${JSON.stringify(diagnostics, null, 2)}`);
  }
}

async function waitForMetricsReady(client, timeoutMs) {
  try {
    await waitUntil(
      async () => {
        const status = await client.evaluate(`
          (() => {
            const snapshot = window.__mir2CacheMetrics?.snapshot?.();
            if (!snapshot) return { ready: false };
            const summary = snapshot.summary ?? {};
            const running = (snapshot.prewarmRuns ?? []).some((run) => run.status === "running" || run.status === "pending");
            const requested = summary.prewarmRequested ?? 0;
            const completed = (summary.prewarmOk ?? 0) + (summary.prewarmFailed ?? 0);
            return {
              ready: requested > 0 && completed >= requested && !running,
              requested,
              completed,
            };
          })()
        `);
        return Boolean(status?.ready);
      },
      timeoutMs,
      "cache metrics prewarm completion",
    );
  } catch (error) {
    const diagnostics = await readPrewarmDiagnostics(client).catch((diagnosticError) => ({
      diagnosticError: diagnosticError instanceof Error ? diagnosticError.message : String(diagnosticError),
    }));
    throw new Error(`${error.message}\n${JSON.stringify(diagnostics, null, 2)}`);
  }
}

async function readMetrics(client) {
  return client.evaluate(`
    (() => {
      const snapshot = window.__mir2CacheMetrics?.snapshot?.();
      return snapshot ? JSON.parse(JSON.stringify(snapshot)) : null;
    })()
  `);
}

async function readAssetCache(client) {
  return client.evaluate(`
    (() => window.__mir2AssetCache ? JSON.parse(JSON.stringify(window.__mir2AssetCache)) : null)()
  `);
}

async function readFirstPlayableDiagnostics(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state;
      const snapshot = window.__mir2CacheMetrics?.snapshot?.();
      return {
        stage: state
          ? {
              screen: state.screen,
              wsState: state.wsState,
              player: state.player,
              playerObjectId: state.playerObjectId,
              entityCount: state.entities?.length ?? 0,
              hasOriginalMapRegion: Boolean(state.originalMapRegionSummary),
              originalMapRegionSummary: state.originalMapRegionSummary,
              sceneInteractionReady: state.sceneInteractionReady ?? null,
              sceneAssetReadiness: state.sceneAssetReadiness ?? null,
              mapFileName: state.mapFileName,
              worldTick: state.worldTick,
              logs: (state.logs ?? []).slice(0, 8).map((line) => line?.text ?? line),
            }
          : null,
        metricsSummary: snapshot?.summary ?? null,
        milestones: snapshot?.milestones ?? [],
        assetCache: window.__mir2AssetCache ?? null,
      };
    })()
  `);
}

async function readPrewarmDiagnostics(client) {
  return client.evaluate(`
    (() => {
      const snapshot = window.__mir2CacheMetrics?.snapshot?.();
      return {
        summary: snapshot?.summary ?? null,
        prewarmRuns: snapshot?.prewarmRuns ?? [],
        cacheStorage: snapshot?.cacheStorage ?? null,
        milestones: snapshot?.milestones ?? [],
        assetCache: window.__mir2AssetCache ?? null,
      };
    })()
  `);
}

function compareRuns(cold, warm) {
  const coldSummary = cold.summary;
  const warmSummary = warm.summary;
  return {
    elapsedMsDelta: warm.elapsedMs - cold.elapsedMs,
    transferBytesDelta: warmSummary.transferBytes - coldSummary.transferBytes,
    encodedBytesDelta: warmSummary.encodedBytes - coldSummary.encodedBytes,
    cachedLikeDelta: warmSummary.cachedLikeCount - coldSummary.cachedLikeCount,
    sceneHitDelta: warmSummary.sceneHits - coldSummary.sceneHits,
    coldSceneHitRate: rate(coldSummary.sceneHits, coldSummary.sceneRequests),
    warmSceneHitRate: rate(warmSummary.sceneHits, warmSummary.sceneRequests),
  };
}

function summarizeResponses(responses) {
  const sceneResponses = responses.filter((response) => new URL(response.url).pathname === "/api/scene/crystal");
  return {
    count: responses.length,
    fromServiceWorker: responses.filter((response) => response.fromServiceWorker).length,
    fromDiskCache: responses.filter((response) => response.fromDiskCache).length,
    sceneHits: sceneResponses.filter((response) => response.sceneCache === "hit").length,
    sceneMisses: sceneResponses.filter((response) => response.sceneCache === "miss").length,
    statusCounts: responses.reduce((counts, response) => {
      counts[response.status] = (counts[response.status] ?? 0) + 1;
      return counts;
    }, {}),
  };
}

function pickSummary(summary) {
  return {
    firstPlayableMs: summary.firstPlayableMs,
    resourceCount: summary.resourceCount,
    gameResourceCount: summary.gameResourceCount,
    transferBytes: summary.transferBytes,
    encodedBytes: summary.encodedBytes,
    cachedLikeCount: summary.cachedLikeCount,
    sceneRequests: summary.sceneRequests,
    sceneHits: summary.sceneHits,
    sceneMisses: summary.sceneMisses,
    prewarmRequested: summary.prewarmRequested,
    prewarmOk: summary.prewarmOk,
    prewarmFailed: summary.prewarmFailed,
    cacheStorageCacheCount: summary.cacheStorageCacheCount,
    cacheStorageEntryCount: summary.cacheStorageEntryCount,
    storageUsageBytes: summary.storageUsageBytes,
    storageQuotaBytes: summary.storageQuotaBytes,
    storagePersisted: summary.storagePersisted,
    storagePersistGranted: summary.storagePersistGranted,
  };
}

function pickBudgets() {
  return {
    maxPrewarmRequested,
    maxWarmCacheEntries,
    maxWarmStorageUsageBytes,
  };
}

function pickMaintenance(maintenance) {
  return {
    seededLegacy: maintenance.seeded?.hasLegacy ?? false,
    afterCleanupCacheCount: maintenance.afterCleanup?.cacheNames?.length ?? 0,
    afterCleanupEntryCount: maintenance.afterCleanup?.entryCount ?? 0,
    resetAvailable: maintenance.reset?.available ?? false,
    resetDeletedCaches: maintenance.reset?.result?.deletedCaches?.length ?? 0,
    resetUnregisteredScopes: maintenance.reset?.result?.unregisteredScopes?.length ?? 0,
    afterResetCacheCount: maintenance.afterReset?.cacheNames?.length ?? 0,
    afterResetEntryCount: maintenance.afterReset?.entryCount ?? 0,
  };
}

function emptySummary() {
  return {
    uptimeMs: 0,
    firstPlayableMs: null,
    resourceCount: 0,
    gameResourceCount: 0,
    transferBytes: 0,
    encodedBytes: 0,
    cachedLikeCount: 0,
    sceneRequests: 0,
    sceneHits: 0,
    sceneMisses: 0,
    prewarmRequested: 0,
    prewarmOk: 0,
    prewarmFailed: 0,
    cacheStorageCacheCount: 0,
    cacheStorageEntryCount: 0,
    storageUsageBytes: null,
    storageQuotaBytes: null,
    storagePersisted: null,
    storagePersistGranted: null,
    slowest: [],
  };
}

async function fillInput(client, selector, value) {
  const ok = await client.evaluate(`
    (() => {
      const element = document.querySelector(${JSON.stringify(selector)});
      if (!element) return false;
      element.focus();
      const descriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), "value");
      if (descriptor?.set) descriptor.set.call(element, ${JSON.stringify(value)});
      else element.value = ${JSON.stringify(value)};
      element.dispatchEvent(new Event("input", { bubbles: true }));
      element.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    })()
  `);
  if (!ok) throw new Error(`Could not fill ${selector}`);
}

async function clickSelector(client, selector) {
  const ok = await client.evaluate(`
    (() => {
      const element = document.querySelector(${JSON.stringify(selector)});
      if (!element) return false;
      element.click();
      return true;
    })()
  `);
  if (!ok) throw new Error(`Could not click ${selector}`);
}

async function waitForPageLoad(client) {
  await waitUntil(
    async () => {
      const state = await client.evaluate("document.readyState");
      return state === "interactive" || state === "complete";
    },
    30_000,
    "page load",
  );
}

async function waitForChrome(port) {
  await waitUntil(
    async () => {
      try {
        const response = await fetch(`http://127.0.0.1:${port}/json/version`);
        return response.ok;
      } catch {
        return false;
      }
    },
    15_000,
    `Chrome CDP port ${port}`,
  );
}

async function createTarget(port, url) {
  const response = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(url)}`, {
    method: "PUT",
  });
  if (!response.ok) throw new Error(`Chrome target creation failed: ${response.status}`);
  return response.json();
}

async function setViewport(client, viewport) {
  await client.send("Emulation.setDeviceMetricsOverride", viewport);
}

async function waitUntil(predicate, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      if (await predicate()) return;
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw new Error(`Timed out waiting for ${label}${lastError ? `: ${lastError.message}` : ""}`);
}

function buildCacheSmokeUrl(rawUrl, currentMode) {
  const url = new URL(rawUrl);
  if (!url.searchParams.has("assetCache")) url.searchParams.set("assetCache", "1");
  if (!url.searchParams.has("cacheDebug")) url.searchParams.set("cacheDebug", "1");
  if (!url.searchParams.has("prewarm")) url.searchParams.set("prewarm", "1");
  if (!url.searchParams.has("skipRuntime")) {
    const defaultSkipRuntime = currentMode === "playable" ? "0" : "1";
    url.searchParams.set("skipRuntime", process.env.MIR2_CACHE_METRICS_SKIP_RUNTIME ?? defaultSkipRuntime);
  }
  const gatewayWs = process.env.MIR2_GATEWAY_WS_URL;
  if (gatewayWs && !url.searchParams.has("gatewayWs")) {
    url.searchParams.set("gatewayWs", gatewayWs);
  }
  return url.toString();
}

function isCacheRelevantUrl(rawUrl) {
  try {
    const url = new URL(rawUrl);
    return (
      url.pathname.startsWith("/original-ui/") ||
      url.pathname.startsWith("/original-map/") ||
      url.pathname.startsWith("/bevy-runtime/") ||
      url.pathname === "/api/asset-manifest" ||
      url.pathname === "/api/original-ui-meta" ||
      url.pathname === "/api/scene/crystal"
    );
  } catch {
    return false;
  }
}

function isCriticalConsoleError(error) {
  if (error?.source === "network" && String(error.text ?? "").includes("net::ERR_FAILED")) {
    return false;
  }
  return true;
}

function hasCacheConsoleLogging(rawUrl) {
  try {
    const url = new URL(rawUrl);
    return url.searchParams.get("cacheLog") === "1" || url.searchParams.get("cacheDebug") === "1";
  } catch {
    return false;
  }
}

function rate(numerator, denominator) {
  return denominator > 0 ? Number((numerator / denominator).toFixed(4)) : null;
}

function defaultCharacterName() {
  return `CM${Date.now().toString(36).slice(-7).toUpperCase()}`.slice(0, 10);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function findChromePath() {
  const candidates =
    process.platform === "win32"
      ? [
          path.join(process.env.ProgramFiles ?? "", "Google", "Chrome", "Application", "chrome.exe"),
          path.join(process.env["ProgramFiles(x86)"] ?? "", "Google", "Chrome", "Application", "chrome.exe"),
          path.join(process.env.ProgramFiles ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
          path.join(process.env["ProgramFiles(x86)"] ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
        ]
      : process.platform === "darwin"
        ? [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
          ]
        : ["/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser"];
  return candidates.find((candidate) => candidate && fsSync.existsSync(candidate));
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

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
