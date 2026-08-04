"use client";

import { useEffect, useState } from "react";

import {
  resolveAssetPrewarmPolicy,
  shouldPrewarmRawMapFrames,
  type AssetPrewarmPolicy,
  type BackgroundPrewarmMode,
} from "../../lib/asset-prewarm-policy";
import {
  AssetPrewarmOrchestrator,
  MIR2_ASSET_FIRST_PLAYABLE_EVENT,
  isAbortError,
  type AssetOrchestratorSnapshot,
  type AssetPrewarmLane,
} from "../../lib/asset-orchestrator";
import {
  selectAssetCachePacksForStage,
  type AssetCachePack,
  type AssetPrewarmStage,
} from "../../lib/asset-cache-packs";
import {
  setAssetReleaseCapabilities,
  type AssetReleaseCapabilities,
} from "../../lib/asset-release-capabilities";
import { normalizeDeviceMemoryGiB, resolveRenderTier } from "../../lib/render-tier";

type AssetManifest = {
  version?: string;
  cacheNamespace?: string;
  capabilities?: Omit<AssetReleaseCapabilities, "releaseId">;
  remoteAssets?: {
    enabled?: boolean;
    assetBaseUrl?: string | null;
    browserFallbackBaseUrls?: string[];
    objectPrefix?: string | null;
  };
  runtimeCaches?: {
    staticAssetMaxEntries?: number;
    staticCriticalMaxEntries?: number;
    staticBackgroundMaxEntries?: number;
    staticRuntimeMaxEntries?: number;
    sceneBlueprintMaxEntries?: number;
    apiMetadataMaxEntries?: number;
  };
  resourcePacks?: AssetCachePack[];
};

type CacheResourceKind =
  | "original-ui"
  | "original-map"
  | "generated-map-blend"
  | "bevy-entity-atlas"
  | "bevy-runtime"
  | "scene-api"
  | "asset-manifest"
  | "original-ui-meta"
  | "other";

type CacheResourceMetric = {
  url: string;
  path: string;
  kind: CacheResourceKind;
  initiatorType: string;
  durationMs: number;
  transferSize: number;
  encodedBodySize: number;
  decodedBodySize: number;
  cachedLike: boolean;
  deliveryType: string | null;
  startTime: number;
};

type CacheApiMetric = {
  url: string;
  path: string;
  kind: CacheResourceKind;
  ok: boolean;
  status: number;
  durationMs: number;
  sceneCache: string | null;
  sceneCacheKey?: string | null;
  originalMapSpriteCount?: number | null;
  originalMapCellCount?: number | null;
  cacheControl: string | null;
  error?: string;
};

type CachePrewarmMetric = {
  name: string;
  label: string;
  requested: number;
  ok: number;
  failed: number;
  failedUrls: string[];
  durationMs: number;
  sceneCacheHits: number;
  sceneCacheMisses: number;
  status: "pending" | "running" | "done" | "error" | "cancelled";
};

type CacheMilestone = {
  name: string;
  atMs: number;
  epochMs: number;
  detail?: Record<string, unknown>;
};

type CacheStorageMetric = {
  supported: boolean;
  updatedAtMs: number;
  cacheCount: number;
  entryCount: number;
  caches: Array<{ name: string; entries: number }>;
  usageBytes: number | null;
  quotaBytes: number | null;
  persisted: boolean | null;
  persistAttempted: boolean;
  persistGranted: boolean | null;
  error?: string;
};

type CacheMetricsSummary = {
  uptimeMs: number;
  firstPlayableMs: number | null;
  resourceCount: number;
  gameResourceCount: number;
  transferBytes: number;
  encodedBytes: number;
  cachedLikeCount: number;
  sceneRequests: number;
  sceneHits: number;
  sceneMisses: number;
  latestSceneCacheKey: string | null;
  latestOriginalMapSpriteCount: number | null;
  latestOriginalMapCellCount: number | null;
  loadedSpriteLibraryCount: number | null;
  domImageCount: number;
  bevyAtlasCount: number | null;
  bevyAtlasPixelBytes: number | null;
  alphaKeyedBlobCount: number | null;
  alphaKeyedBlobBytes: number | null;
  prewarmRequested: number;
  prewarmOk: number;
  prewarmFailed: number;
  cacheStorageCacheCount: number;
  cacheStorageEntryCount: number;
  storageUsageBytes: number | null;
  storageQuotaBytes: number | null;
  storagePersisted: boolean | null;
  storagePersistGranted: boolean | null;
  slowest: Array<Pick<CacheResourceMetric, "path" | "kind" | "durationMs" | "transferSize">>;
};

type CacheMetricsSnapshot = {
  startedAt: number;
  debug: boolean;
  summary: CacheMetricsSummary;
  resources: CacheResourceMetric[];
  apiRequests: CacheApiMetric[];
  prewarmRuns: CachePrewarmMetric[];
  milestones: CacheMilestone[];
  cacheStorage: CacheStorageMetric | null;
};

type CacheMetricsHandle = {
  startedAt: number;
  debug: boolean;
  consoleLog: boolean;
  lastProgressLogKey: string | null;
  resourceKeys: Set<string>;
  resources: CacheResourceMetric[];
  apiRequests: CacheApiMetric[];
  prewarmRuns: CachePrewarmMetric[];
  milestones: CacheMilestone[];
  cacheStorage: CacheStorageMetric | null;
  markMilestone: (name: string, detail?: Record<string, unknown>) => CacheMilestone;
  snapshot: () => CacheMetricsSnapshot;
  reset: () => void;
};

type AssetCacheResetOptions = {
  reload?: boolean;
};

type AssetCacheResetResult = {
  deletedCaches: string[];
  unregisteredScopes: string[];
  reloaded: boolean;
};

type AssetCachePrewarmStatus = "idle" | "running" | "done" | "error";

declare global {
  interface Window {
    __mir2AssetCache?: {
      enabled: boolean;
      status: string;
      version?: string;
      cacheNamespace?: string;
      message?: string;
      prewarm?: string;
      prewarmProgress?: {
        status: "idle" | "running" | "done" | "error";
        activePackName: string | null;
        activePackLabel: string | null;
        requested: number;
        completed: number;
        ok: number;
        failed: number;
        percent: number;
        cacheStorageEntryCount: number;
        transferBytes: number;
      };
      remoteAssetBaseUrl?: string | null;
      remoteAssetFallbackBaseUrls?: string[];
      staticAssetTiers?: Record<string, number> | null;
      renderTier?: "low" | "medium" | "high";
      prewarmPolicy?: AssetPrewarmPolicy;
      releaseCapabilities?: AssetReleaseCapabilities;
    };
    __mir2AssetCacheReset?: (options?: AssetCacheResetOptions) => Promise<AssetCacheResetResult>;
    __mir2AssetCachePrewarmStage?: (stage: AssetPrewarmStage) => Promise<void>;
    __mir2PendingAssetPrewarmStage?: AssetPrewarmStage;
    __mir2AssetOrchestrator?: {
      snapshot: () => AssetOrchestratorSnapshot;
    };
    __mir2CacheMetrics?: CacheMetricsHandle;
    __mir2ActiveCacheMetrics?: CacheMetricsHandle;
    __mir2PendingCacheMilestones?: Array<{ name: string; detail?: Record<string, unknown> }>;
    __mir2CacheFetchPatched?: boolean;
    __mir2OriginalFetch?: typeof fetch;
    __mir2CacheResourceObserver?: PerformanceObserver;
  }
}

const MAX_RESOURCE_METRICS = 2000;
const MAX_API_METRICS = 500;
const CACHE_NAMESPACE_STORAGE_KEY = "mir2.assetCacheNamespace";

export function AssetCacheRegistrar() {
  const [mounted, setMounted] = useState(false);
  const [debugEnabled, setDebugEnabled] = useState(false);
  const [snapshot, setSnapshot] = useState<CacheMetricsSnapshot | null>(null);

  useEffect(() => {
    setMounted(true);
    const params = new URLSearchParams(window.location.search);
    const debug = params.get("cacheDebug") === "1";
    const consoleLog = debug || params.get("cacheLog") === "1";
    const metrics = ensureCacheMetrics(debug, consoleLog);
    // The asset Service Worker — and its R2 fallback for /original-ui, /original-map, … assets
    // that are NOT committed to the repo — always runs in production. In dev it stays opt-in so a
    // bytes-less local checkout is never surprised by a Service Worker, but it now turns on
    // automatically whenever NEXT_PUBLIC_MIR2_ASSET_BASE_URL is configured. That makes sprites AND
    // sounds (e.g. /original-ui/Sound/62.wav, whose bytes live only on R2) resolve from the release
    // origin like prod instead of 404ing locally. Explicit ?assetCache=1 still forces it on with no
    // base URL configured; ?assetCache=0 forces it off for raw-network debugging.
    const assetCacheParam = params.get("assetCache");
    const remoteAssetBaseConfigured = Boolean(process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL);
    // `?assetCache=0` is a universal kill-switch (now honoured in production too):
    // behind the same-origin local proxy the SW is redundant, and its asset cache
    // otherwise keeps serving a stale pre-rebuild build (blue/missing tiles that
    // survive a hard refresh). The disabled branch below also tears down any worker
    // + mir2 caches a prior load left behind.
    const shouldRegisterServiceWorker =
      assetCacheParam !== "0" &&
      (process.env.NODE_ENV === "production" ||
        assetCacheParam === "1" ||
        remoteAssetBaseConfigured);
    const shouldPrewarm =
      params.get("prewarm") !== "0" &&
      (process.env.NODE_ENV === "production" ||
        params.get("prewarm") === "1" ||
        params.get("assetCache") === "1");
    const deviceMemoryGiB = normalizeDeviceMemoryGiB(
      (navigator as Navigator & { deviceMemory?: number }).deviceMemory,
    );
    const renderTier = resolveRenderTier({
      forcedTier: params.get("renderTier"),
      deviceMemoryGiB,
      coarsePointer: window.matchMedia?.("(pointer: coarse)").matches ?? false,
    });
    const prewarmPolicy = resolveAssetPrewarmPolicy(renderTier, resolveBackgroundPrewarmModeOverride(params));

    setDebugEnabled(debug);
    installFetchProbe();
    installPerformanceProbe(metrics);
    const assetCacheReset = installAssetCacheReset(metrics);

    let disposed = false;
    let debugInterval = 0;
    let installedStagePrewarm: ((stage: AssetPrewarmStage) => Promise<void>) | null = null;
    let prewarmOrchestrator: AssetPrewarmOrchestrator | null = null;
    const handleFirstPlayable = () => {
      void prewarmOrchestrator?.markFirstPlayable();
    };
    window.addEventListener(MIR2_ASSET_FIRST_PLAYABLE_EVENT, handleFirstPlayable);

    async function bootCacheLayer() {
      try {
        metrics.markMilestone("assetManifestStart");
        const manifestResponse = await fetch("/api/asset-manifest", {
          cache: "no-cache",
        });
        if (!manifestResponse.ok) {
          throw new Error(`asset manifest returned ${manifestResponse.status}`);
        }
        const manifest = (await manifestResponse.json()) as AssetManifest;
        const cacheNamespace = manifest.cacheNamespace || manifest.version || "";
        const releaseCapabilities = setAssetReleaseCapabilities({
          releaseId: manifest.version,
          ...(manifest.capabilities ?? {}),
        });
        metrics.markMilestone("assetManifestReady", {
          version: manifest.version,
          cacheNamespace,
          packCount: manifest.resourcePacks?.length ?? 0,
          remoteAssetsEnabled: Boolean(manifest.remoteAssets?.enabled),
          remoteAssetBaseUrl: manifest.remoteAssets?.assetBaseUrl ?? null,
          remoteAssetFallbackBaseUrls: manifest.remoteAssets?.browserFallbackBaseUrls ?? [],
          crystalFullPackEnabled: releaseCapabilities.crystalFullPack.enabled,
        });
        logCacheEvent(metrics, "manifest", {
          version: manifest.version,
          cacheNamespace,
          packCount: manifest.resourcePacks?.length ?? 0,
          packs: manifest.resourcePacks?.map((pack) => ({
            name: pack.name,
            label: pack.label,
            urls: pack.urls.length,
            scenes: pack.scenes?.length ?? 0,
            cacheTier: pack.cacheTier ?? (pack.phase === "background" ? "background" : "critical"),
          })),
          runtimeCaches: manifest.runtimeCaches ?? null,
          remoteAssets: manifest.remoteAssets ?? null,
        });

        if (shouldRegisterServiceWorker && "serviceWorker" in navigator && isServiceWorkerSafeOrigin()) {
          metrics.markMilestone("serviceWorkerRegisterStart");
          const registration = await navigator.serviceWorker.register(
            "/mir2-asset-worker.js",
            { scope: "/", updateViaCache: "none" },
          );
          try {
            await registration.update();
          } catch {
            // The active worker can still serve from cache if the update check is transiently unavailable.
          }
          if (registration.waiting) {
            registration.waiting.postMessage({ type: "MIR2_ASSET_WORKER_ACTIVATE" });
            await waitForServiceWorkerActivation(registration.waiting, 3000);
          }
          const readyRegistration = await navigator.serviceWorker.ready;

          if (disposed) return;

          const payload = {
            type: "MIR2_ASSET_CACHE_CONFIG",
            manifest,
            manifestVersion: manifest.version,
            cacheNamespace,
          };
          const configuredMessage = waitForServiceWorkerMessage("MIR2_ASSET_CACHE_CONFIGURED", 3000);
          readyRegistration.active?.postMessage(payload);
          registration.waiting?.postMessage(payload);
          registration.installing?.postMessage(payload);
          const configured = await configuredMessage;
          const deletedCaches = Array.isArray(configured?.deletedCaches)
            ? configured.deletedCaches.map(String)
            : [];
          metrics.markMilestone("serviceWorkerConfigured", {
            version: manifest.version,
            cacheNamespace,
            remoteAssetsEnabled: Boolean(manifest.remoteAssets?.enabled),
            deletedCaches: deletedCaches.length,
            staticAssetTiers: configured?.staticAssetTiers ?? null,
          });

          window.__mir2AssetCache = {
            enabled: true,
            status: "registered",
            version: manifest.version,
            cacheNamespace,
            remoteAssetBaseUrl: manifest.remoteAssets?.assetBaseUrl ?? null,
            remoteAssetFallbackBaseUrls: manifest.remoteAssets?.browserFallbackBaseUrls ?? [],
            staticAssetTiers:
              typeof configured?.staticAssetTiers === "object" && configured.staticAssetTiers
                ? (configured.staticAssetTiers as Record<string, number>)
                : null,
            renderTier,
            prewarmPolicy,
            releaseCapabilities,
          };
          logCacheEvent(metrics, "service-worker", {
            status: window.__mir2AssetCache.status,
            version: manifest.version,
            cacheNamespace,
            remoteAssetBaseUrl: manifest.remoteAssets?.assetBaseUrl ?? null,
            remoteAssetFallbackBaseUrls: manifest.remoteAssets?.browserFallbackBaseUrls ?? [],
            deletedCaches: deletedCaches.length,
          });
          maybeReloadAfterCacheNamespaceChange(cacheNamespace, deletedCaches);
          void refreshCacheStorageMetrics(metrics).then(() => {
            if (!disposed && debug) setSnapshot(metrics.snapshot());
          });
          // Request persistent storage EARLY (before prewarm fills the cache) so the
          // browser is more likely to grant it and CacheStorage isn't evicted between
          // loads. Non-blocking + idempotent; the later await inside prewarmAssetPacks
          // then no-ops instead of stalling prewarm progress at requested:0.
          void requestPersistentStorage(metrics);
          metrics.markMilestone("serviceWorkerReady", {
            version: manifest.version,
            controlled: Boolean(navigator.serviceWorker.controller),
          });
        } else {
          // Explicit ?assetCache=0: actively tear down any worker + mir2 CacheStorage
          // a prior load left, so a stale (e.g. pre-proxy-fix) build stops being
          // served. Takes full effect on the next navigation.
          if (assetCacheParam === "0" && "serviceWorker" in navigator) {
            void unregisterMir2AssetWorkers();
            if ("caches" in window) {
              void caches
                .keys()
                .then((keys) => Promise.all(keys.filter((k) => /mir2/i.test(k)).map((k) => caches.delete(k))))
                .catch(() => {});
            }
          }
          window.__mir2AssetCache = {
            enabled: false,
            status: shouldRegisterServiceWorker ? "unsupported-origin" : "disabled-dev",
            version: manifest.version,
            cacheNamespace,
            remoteAssetBaseUrl: manifest.remoteAssets?.assetBaseUrl ?? null,
            remoteAssetFallbackBaseUrls: manifest.remoteAssets?.browserFallbackBaseUrls ?? [],
            renderTier,
            prewarmPolicy,
            releaseCapabilities,
          };
          metrics.markMilestone("serviceWorkerSkipped", {
            status: window.__mir2AssetCache.status,
          });
          logCacheEvent(metrics, "service-worker", {
            status: window.__mir2AssetCache.status,
            version: manifest.version,
          });
        }

        // Expose stage prewarming only after the Service Worker has received its
        // manifest/cache-tier configuration. The orchestrator is the single owner
        // of lifecycle gating and cancellation: critical work runs immediately,
        // while stale background work is replaced and afterPlayable really waits
        // for the renderer-backed first-playable signal.
        const manifestPacks = manifest.resourcePacks ?? [];
        prewarmOrchestrator = new AssetPrewarmOrchestrator({
          backgroundMode: prewarmPolicy.backgroundMode,
          hasWork: (stage, lane) =>
            selectAssetCachePacksForStage(stage, manifestPacks).some((pack) =>
              lane === "background" ? pack.phase === "background" : pack.phase !== "background",
            ),
          run: async (stage, lane, signal) => {
            await prewarmAssetPacks(manifest, metrics, prewarmPolicy, stage, lane, signal);
            if (!disposed) setSnapshot(metrics.snapshot());
          },
        });
        window.__mir2AssetOrchestrator = {
          snapshot: () => prewarmOrchestrator?.snapshot() ?? {
            backgroundMode: prewarmPolicy.backgroundMode,
            firstPlayable: Boolean(window.__mir2AssetFirstPlayable),
            requestedStage: null,
            pendingBackgroundStage: null,
            activeBackgroundStage: null,
            completed: [],
            disposed: true,
          },
        };
        if (window.__mir2AssetFirstPlayable) {
          void prewarmOrchestrator.markFirstPlayable();
        }
        installedStagePrewarm = (stage) => {
          if (!shouldPrewarm) return Promise.resolve();
          return prewarmOrchestrator?.requestStage(stage) ?? Promise.resolve();
        };
        window.__mir2AssetCachePrewarmStage = installedStagePrewarm;

        if (shouldPrewarm) {
          const pendingStage = window.__mir2PendingAssetPrewarmStage ?? "login";
          const finishInitialPrewarm = () => {
            if (disposed) return;
            window.__mir2AssetCache = {
              ...(window.__mir2AssetCache ?? { enabled: false, status: "unknown" }),
              prewarm: "done",
            };
            if (debug || process.env.NODE_ENV !== "production") {
              console.info("[mir2-cache] prewarm", metrics.snapshot().summary);
            }
            setSnapshot(metrics.snapshot());
          };
          if (pendingStage === "login") {
            scheduleIdle(() => {
              void installedStagePrewarm?.("login").then(finishInitialPrewarm);
            });
          } else {
            void installedStagePrewarm(pendingStage).then(() => {
              if (disposed) return;
              finishInitialPrewarm();
            });
          }
        }
      } catch (error) {
        if (disposed) return;
        window.__mir2AssetCache = {
          enabled: false,
          status: "error",
          message: error instanceof Error ? error.message : String(error),
        };
        console.warn("[mir2] asset cache registration failed", error);
      }
    }

    void bootCacheLayer();

    if (debug || shouldPrewarm) {
      setSnapshot(metrics.snapshot());
      debugInterval = window.setInterval(() => setSnapshot(metrics.snapshot()), debug ? 1000 : 500);
    } else if (process.env.NODE_ENV !== "production") {
      window.setTimeout(() => {
        console.info("[mir2-cache]", metrics.snapshot().summary);
      }, 4000);
    }

    return () => {
      disposed = true;
      if (window.__mir2AssetCacheReset === assetCacheReset) {
        delete window.__mir2AssetCacheReset;
      }
      if (window.__mir2AssetCachePrewarmStage === installedStagePrewarm) {
        delete window.__mir2AssetCachePrewarmStage;
      }
      prewarmOrchestrator?.dispose();
      delete window.__mir2AssetOrchestrator;
      window.removeEventListener(MIR2_ASSET_FIRST_PLAYABLE_EVENT, handleFirstPlayable);
      if (debugInterval) window.clearInterval(debugInterval);
    };
  }, []);

  if (!mounted || !snapshot) return null;

  if (debugEnabled) return <CacheDebugPanel snapshot={snapshot} />;

  if (!shouldShowCacheProgress(snapshot)) return null;

  return <CacheProgressPanel snapshot={snapshot} />;
}

function CacheProgressPanel({ snapshot }: { snapshot: CacheMetricsSnapshot }) {
  const summary = snapshot.summary;
  const activeRun = getActivePrewarmRun(snapshot);
  const completed = summary.prewarmOk + summary.prewarmFailed;
  const isComplete = summary.prewarmRequested > 0 && completed >= summary.prewarmRequested;
  const statusLabel = summary.prewarmFailed > 0 ? "失败" : isComplete ? "完成" : "加载中";
  const percent =
    summary.prewarmRequested > 0
      ? Math.min(100, Math.round((completed / summary.prewarmRequested) * 100))
      : 0;
  const localCount = summary.cachedLikeCount + summary.cacheStorageEntryCount;

  return (
    <section
      className="mir-cache-progress-panel"
      aria-label="Mir2 resource loading status"
      style={{
        position: "fixed",
        left: 12,
        bottom: 12,
        zIndex: 9998,
        width: 286,
        maxWidth: "calc(100vw - 24px)",
        padding: "9px 10px",
        color: "#f5f1df",
        background: "rgba(7, 7, 6, 0.84)",
        border: "1px solid rgba(205, 174, 96, 0.55)",
        borderRadius: 6,
        font: "12px/1.35 Arial, Helvetica, sans-serif",
        boxShadow: "0 2px 10px rgba(0, 0, 0, 0.35)",
        pointerEvents: "none",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, fontWeight: 700 }}>
        <span>资源缓存 · {statusLabel}</span>
        <span>{percent}%</span>
      </div>
      <div
        style={{
          height: 4,
          margin: "7px 0 6px",
          background: "rgba(255, 255, 255, 0.16)",
          overflow: "hidden",
          borderRadius: 2,
        }}
      >
        <div
          style={{
            width: `${percent}%`,
            height: "100%",
            background: "linear-gradient(90deg, #cfa84f, #79c36a)",
          }}
        />
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
        <span>{activeRun?.label ?? "准备资源包"}</span>
        <span>
          {completed}/{summary.prewarmRequested}
        </span>
      </div>
      <div style={{ color: "rgba(245, 241, 223, 0.72)", marginTop: 3 }}>
        本地 {localCount} 项 · 远程 {formatBytes(summary.transferBytes)}
        {summary.prewarmFailed > 0 ? ` · 失败 ${summary.prewarmFailed}` : ""}
      </div>
    </section>
  );
}

function CacheDebugPanel({ snapshot }: { snapshot: CacheMetricsSnapshot }) {
  const summary = snapshot.summary;
  const assetCache = typeof window !== "undefined" ? window.__mir2AssetCache : null;

  return (
    <section
      aria-label="Mir2 cache debug"
      style={{
        position: "fixed",
        right: 12,
        bottom: 12,
        zIndex: 9999,
        width: 300,
        maxWidth: "calc(100vw - 24px)",
        padding: "10px 12px",
        color: "#d6f7d0",
        background: "rgba(0, 0, 0, 0.82)",
        border: "1px solid rgba(130, 210, 120, 0.55)",
        borderRadius: 6,
        font: "12px/1.35 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
        pointerEvents: "none",
      }}
    >
      <div style={{ color: "#ffffff", fontWeight: 700, marginBottom: 6 }}>Mir2 Cache Debug</div>
      <div>SW: {assetCache?.status ?? "pending"} {assetCache?.version ? `(${assetCache.version})` : ""}</div>
      <div>CDN: {assetCache?.remoteAssetBaseUrl ? shortPath(assetCache.remoteAssetBaseUrl) : "off"}</div>
      <div>Browser fallback: {assetCache?.remoteAssetFallbackBaseUrls?.length ?? 0}</div>
      <div>Tiers: {formatStaticAssetTiers(assetCache?.staticAssetTiers)}</div>
      <div>Resources: {summary.gameResourceCount}/{summary.resourceCount}</div>
      <div>Transfer: {formatBytes(summary.transferBytes)} / encoded {formatBytes(summary.encodedBytes)}</div>
      <div>Cache-like: {summary.cachedLikeCount}</div>
      <div>Scene: {summary.sceneHits} hit / {summary.sceneMisses} miss / {summary.sceneRequests} total</div>
      <div>Prewarm: {summary.prewarmOk}/{summary.prewarmRequested} ok, {summary.prewarmFailed} failed</div>
      <div>
        Storage: {summary.cacheStorageEntryCount} entries / {summary.cacheStorageCacheCount} caches
        {summary.storageUsageBytes != null ? ` / ${formatBytes(summary.storageUsageBytes)}` : ""}
        {summary.storageQuotaBytes != null ? ` of ${formatBytes(summary.storageQuotaBytes)}` : ""}
      </div>
      <div>
        Persist: {summary.storagePersisted == null ? "unknown" : summary.storagePersisted ? "yes" : "no"}
        {summary.storagePersistGranted != null ? ` / grant ${summary.storagePersistGranted ? "yes" : "no"}` : ""}
      </div>
      <div style={{ color: "#ffffff", marginTop: 6 }}>Slowest</div>
      {summary.slowest.slice(0, 4).map((entry, index) => (
        <div key={`${index}:${entry.kind}:${entry.path}:${entry.durationMs}`}>
          {entry.kind} {Math.round(entry.durationMs)}ms {shortPath(entry.path)}
        </div>
      ))}
    </section>
  );
}

function ensureCacheMetrics(debug: boolean, consoleLog = false): CacheMetricsHandle {
  const existing = window.__mir2CacheMetrics;
  if (existing) {
    existing.debug = existing.debug || debug;
    existing.consoleLog = existing.consoleLog || consoleLog;
    window.__mir2ActiveCacheMetrics = existing;
    return existing;
  }

  const handle: CacheMetricsHandle = {
    startedAt: performance.now(),
    debug,
    consoleLog,
    lastProgressLogKey: null,
    resourceKeys: new Set<string>(),
    resources: [],
    apiRequests: [],
    prewarmRuns: [],
    milestones: [],
    cacheStorage: null,
    markMilestone(name, detail) {
      const existing = this.milestones.find((milestone) => milestone.name === name);
      if (existing) return existing;
      const milestone = {
        name,
        atMs: performance.now() - this.startedAt,
        epochMs: Date.now(),
        detail,
      };
      this.milestones.push(milestone);
      return milestone;
    },
    snapshot() {
      return {
        startedAt: this.startedAt,
        debug: this.debug,
        summary: summarizeMetrics(this),
        resources: [...this.resources],
        apiRequests: [...this.apiRequests],
        prewarmRuns: [...this.prewarmRuns],
        milestones: [...this.milestones],
        cacheStorage: this.cacheStorage
          ? {
              ...this.cacheStorage,
              caches: [...this.cacheStorage.caches],
            }
          : null,
      };
    },
    reset() {
      this.startedAt = performance.now();
      this.resourceKeys.clear();
      this.resources.length = 0;
      this.apiRequests.length = 0;
      this.prewarmRuns.length = 0;
      this.milestones.length = 0;
      this.cacheStorage = null;
    },
  };

  window.__mir2CacheMetrics = handle;
  window.__mir2ActiveCacheMetrics = handle;
  flushPendingMilestones(handle);
  handle.markMilestone("cacheMetricsReady");
  return handle;
}

function flushPendingMilestones(metrics: CacheMetricsHandle) {
  const pending = window.__mir2PendingCacheMilestones ?? [];
  window.__mir2PendingCacheMilestones = [];
  for (const milestone of pending) {
    metrics.markMilestone(milestone.name, milestone.detail);
  }
}

function installFetchProbe() {
  if (window.__mir2CacheFetchPatched) return;

  const originalFetch = window.fetch.bind(window);
  window.__mir2OriginalFetch = originalFetch;
  window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const metrics = window.__mir2ActiveCacheMetrics;
    const requestUrl = resolveFetchUrl(input);
    const kind = requestUrl ? classifyUrl(requestUrl) : "other";
    const shouldRecord = kind === "scene-api" || kind === "asset-manifest" || kind === "original-ui-meta";
    const startedAt = performance.now();

    try {
      const response = await originalFetch(input, init);
      if (metrics && shouldRecord && requestUrl) {
        recordApiMetric(metrics, {
          url: requestUrl.href,
          path: `${requestUrl.pathname}${requestUrl.search}`,
          kind,
          ok: response.ok,
          status: response.status,
          durationMs: performance.now() - startedAt,
          sceneCache: response.headers.get("x-mir2-scene-cache"),
          sceneCacheKey: response.headers.get("x-mir2-scene-cache-key"),
          originalMapSpriteCount: numberHeader(response.headers.get("x-mir2-original-map-sprite-count")),
          originalMapCellCount: numberHeader(response.headers.get("x-mir2-original-map-cell-count")),
          cacheControl: response.headers.get("cache-control"),
        });
      }
      return response;
    } catch (error) {
      if (metrics && shouldRecord && requestUrl) {
        recordApiMetric(metrics, {
          url: requestUrl.href,
          path: `${requestUrl.pathname}${requestUrl.search}`,
          kind,
          ok: false,
          status: 0,
          durationMs: performance.now() - startedAt,
          sceneCache: null,
          sceneCacheKey: null,
          originalMapSpriteCount: null,
          originalMapCellCount: null,
          cacheControl: null,
          error: error instanceof Error ? error.message : String(error),
        });
      }
      throw error;
    }
  };
  window.__mir2CacheFetchPatched = true;
}

function installPerformanceProbe(metrics: CacheMetricsHandle) {
  performance.setResourceTimingBufferSize?.(3000);

  for (const entry of performance.getEntriesByType("resource")) {
    recordResourceEntry(metrics, entry as PerformanceResourceTiming);
  }

  if (window.__mir2CacheResourceObserver) return;

  const observer = new PerformanceObserver((list) => {
    const activeMetrics = window.__mir2ActiveCacheMetrics;
    if (!activeMetrics) return;
    for (const entry of list.getEntries()) {
      recordResourceEntry(activeMetrics, entry as PerformanceResourceTiming);
    }
  });
  observer.observe({ type: "resource", buffered: true });
  window.__mir2CacheResourceObserver = observer;
}

function waitForServiceWorkerMessage(type: string, timeoutMs: number) {
  if (!("serviceWorker" in navigator)) return Promise.resolve(null);

  return new Promise<Record<string, unknown> | null>((resolve) => {
    let timeout = 0;
    const cleanup = () => {
      navigator.serviceWorker.removeEventListener("message", onMessage);
      if (timeout) window.clearTimeout(timeout);
    };
    const onMessage = (event: MessageEvent) => {
      const data = event.data as Record<string, unknown> | undefined;
      if (data?.type !== type) return;
      cleanup();
      resolve(data);
    };

    timeout = window.setTimeout(() => {
      cleanup();
      resolve(null);
    }, timeoutMs);
    navigator.serviceWorker.addEventListener("message", onMessage);
  });
}

function waitForServiceWorkerActivation(worker: ServiceWorker, timeoutMs: number) {
  if (worker.state === "activated" || worker.state === "redundant") {
    return Promise.resolve();
  }

  return new Promise<void>((resolve) => {
    let timeout = 0;
    const cleanup = () => {
      worker.removeEventListener("statechange", onStateChange);
      if (timeout) window.clearTimeout(timeout);
    };
    const finish = () => {
      cleanup();
      resolve();
    };
    const onStateChange = () => {
      if (worker.state === "activated" || worker.state === "redundant") finish();
    };

    timeout = window.setTimeout(finish, timeoutMs);
    worker.addEventListener("statechange", onStateChange);
  });
}

function maybeReloadAfterCacheNamespaceChange(cacheNamespace: string, deletedCaches: string[]) {
  if (!cacheNamespace || deletedCaches.length === 0) return;

  const previous = readStorageValue(window.localStorage, CACHE_NAMESPACE_STORAGE_KEY);
  writeStorageValue(window.localStorage, CACHE_NAMESPACE_STORAGE_KEY, cacheNamespace);
  if (previous === cacheNamespace) return;

  const reloadKey = `${CACHE_NAMESPACE_STORAGE_KEY}.reloaded.${cacheNamespace}`;
  if (readStorageValue(window.sessionStorage, reloadKey) === "1") return;
  writeStorageValue(window.sessionStorage, reloadKey, "1");

  window.setTimeout(() => {
    window.location.reload();
  }, 250);
}

function readStorageValue(storage: Storage, key: string) {
  try {
    return storage.getItem(key);
  } catch {
    return null;
  }
}

function writeStorageValue(storage: Storage, key: string, value: string) {
  try {
    storage.setItem(key, value);
  } catch {
    // Storage can be unavailable in privacy modes; cache correctness should not depend on it.
  }
}

async function prewarmAssetPacks(
  manifest: AssetManifest,
  metrics: CacheMetricsHandle,
  options: AssetPrewarmPolicy,
  stage: AssetPrewarmStage,
  lane: AssetPrewarmLane,
  signal: AbortSignal,
) {
  const stagePacks = selectAssetCachePacksForStage(stage, manifest.resourcePacks ?? []);
  const packs = stagePacks.filter((pack) =>
    lane === "background" ? pack.phase === "background" : pack.phase !== "background",
  );
  const includeSceneFrames = lane === "background";
  metrics.markMilestone(`prewarmStart:${stage}:${lane}`, {
    stage,
    lane,
    packCount: packs.length,
    skippedPackCount: stagePacks.length - packs.length,
    includeSceneFrames,
    renderTier: options.tier,
    criticalConcurrency: options.criticalConcurrency,
    backgroundConcurrency: options.backgroundConcurrency,
    maxSceneFrames: options.maxSceneFrames,
    backgroundMode: options.backgroundMode,
  });
  publishPrewarmProgress(metrics, "running");
  await requestPersistentStorage(metrics);
  const concurrency = lane === "critical" ? options.criticalConcurrency : options.backgroundConcurrency;
  try {
    for (const pack of packs) {
      if (signal.aborted) throw createAbortError();
      await prewarmPack(
        pack,
        metrics,
        concurrency,
        options.maxSceneFrames,
        options.tier,
        undefined,
        includeSceneFrames,
        signal,
      );
    }
  } catch (error) {
    if (signal.aborted || isAbortError(error)) {
      metrics.markMilestone(`prewarmCancelled:${stage}:${lane}`, {
        stage,
        lane,
        reason: signal.reason == null ? null : String(signal.reason),
      });
      throw error;
    }
    throw error;
  } finally {
    await refreshCacheStorageMetrics(metrics);
  }
  metrics.markMilestone(`prewarmDone:${stage}:${lane}`, summarizeMetrics(metrics));
  publishPrewarmProgress(metrics, "done");
}

function createPrewarmMetric(
  pack: AssetCachePack,
  status: CachePrewarmMetric["status"],
  includeScenes = true,
): CachePrewarmMetric {
  return {
    name: pack.name,
    label: pack.label,
    requested: pack.urls.length + (includeScenes ? (pack.scenes?.length ?? 0) : 0),
    ok: 0,
    failed: 0,
    failedUrls: [],
    durationMs: 0,
    sceneCacheHits: 0,
    sceneCacheMisses: 0,
    status,
  };
}

async function prewarmPack(
  pack: AssetCachePack,
  metrics: CacheMetricsHandle,
  concurrency: number,
  maxSceneFrames: number | null,
  renderTier: AssetPrewarmPolicy["tier"],
  existingMetric?: CachePrewarmMetric,
  includeScenes = true,
  signal?: AbortSignal,
) {
  const startedAt = performance.now();
  const packMetric = existingMetric ?? createPrewarmMetric(pack, "running", includeScenes);
  packMetric.status = "running";
  if (!existingMetric) metrics.prewarmRuns.push(packMetric);
  publishPrewarmProgress(metrics, "running");

  try {
    if (signal?.aborted) throw createAbortError();
    await hintAssetCacheTier(pack.urls, cacheTierForPack(pack));
    await prewarmUrls(pack.urls, packMetric, metrics, concurrency, signal);

    for (const scene of includeScenes ? (pack.scenes ?? []) : []) {
      if (signal?.aborted) throw createAbortError();
      let response: Response;
      try {
        response = await fetchWithRetry(scene.url, { cache: "force-cache", signal }, signal);
      } catch (error) {
        if (signal?.aborted || isAbortError(error)) throw error;
        packMetric.failed += 1;
        packMetric.failedUrls.push(scene.url);
        continue;
      }

      if (response.ok) {
        packMetric.ok += 1;
      } else {
        packMetric.failed += 1;
        packMetric.failedUrls.push(scene.url);
        continue;
      }

      const sceneCache = response.headers.get("x-mir2-scene-cache");
      if (sceneCache === "hit") packMetric.sceneCacheHits += 1;
      if (sceneCache === "miss") packMetric.sceneCacheMisses += 1;

      const blueprint = await response.json();
      const sceneFrameLimit = maxSceneFrames === null
        ? scene.spriteFrameLimit
        : Math.min(scene.spriteFrameLimit, maxSceneFrames);
      const frameUrls = extractSceneFrameUrls(blueprint, sceneFrameLimit, {
        includeRawMapFrames: shouldPrewarmRawMapFrames(renderTier),
      });
      packMetric.requested += frameUrls.length;
      publishPrewarmProgress(metrics, "running");
      await hintAssetCacheTier(frameUrls, cacheTierForPack(pack));
      await prewarmUrls(frameUrls, packMetric, metrics, concurrency, signal);
    }

    packMetric.status = "done";
  } catch (error) {
    if (signal?.aborted || isAbortError(error)) {
      packMetric.status = "cancelled";
      throw error;
    }
    packMetric.status = "error";
    packMetric.failed += 1;
    console.warn("[mir2] asset prewarm failed", pack.name, error);
  } finally {
    packMetric.durationMs = performance.now() - startedAt;
    publishPrewarmProgress(metrics, packMetric.status === "error" ? "error" : "running");
  }
}

function cacheTierForPack(pack: AssetCachePack) {
  return pack.cacheTier ?? (pack.phase === "background" ? "background" : "critical");
}

async function hintAssetCacheTier(urls: string[], tier: "critical" | "background") {
  if (!urls.length || !("serviceWorker" in navigator)) return;
  if (window.__mir2AssetCache?.enabled !== true) return;
  try {
    const registration = await navigator.serviceWorker.ready;
    const worker = navigator.serviceWorker.controller ?? registration.active;
    worker?.postMessage({
      type: "MIR2_ASSET_CACHE_HINT",
      cacheTier: tier,
      urls,
    });
  } catch {
    // Cache hints are best-effort; fetch/prewarm should continue without them.
  }
}

async function prewarmUrls(
  urls: string[],
  metric: CachePrewarmMetric,
  metrics: CacheMetricsHandle,
  concurrency: number,
  signal?: AbortSignal,
) {
  const queue = [...new Set(urls)].filter(Boolean);
  let index = 0;
  let completedSincePublish = 0;
  let lastPublishAt = performance.now();

  async function worker() {
    while (index < queue.length) {
      if (signal?.aborted) throw createAbortError();
      const url = queue[index];
      index += 1;
      try {
        const response = await fetchWithRetry(url, { cache: "force-cache", signal }, signal);
        if (response.ok) metric.ok += 1;
        else {
          metric.failed += 1;
          metric.failedUrls.push(url);
        }
      } catch (error) {
        if (signal?.aborted || isAbortError(error)) throw error;
        metric.failed += 1;
        metric.failedUrls.push(url);
      }
      completedSincePublish += 1;
      const now = performance.now();
      if (completedSincePublish >= 12 || now - lastPublishAt >= 300) {
        completedSincePublish = 0;
        lastPublishAt = now;
        publishPrewarmProgress(metrics, "running");
      }
    }
  }

  await Promise.all(Array.from({ length: Math.min(concurrency, queue.length) }, worker));
  publishPrewarmProgress(metrics, "running");
}

async function fetchWithRetry(url: string, init: RequestInit, signal?: AbortSignal) {
  let lastError: unknown;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    if (signal?.aborted) throw createAbortError();
    try {
      const response = await fetch(url, init);
      if (response.ok || attempt === 2) return response;
    } catch (error) {
      if (signal?.aborted || isAbortError(error)) throw error;
      lastError = error;
    }
    await sleep(150 * (attempt + 1), signal);
  }
  throw lastError;
}

function createAbortError() {
  return new DOMException("Asset prewarm aborted", "AbortError");
}

function extractSceneFrameUrls(
  blueprint: unknown,
  limit: number,
  options: { includeRawMapFrames: boolean },
) {
  const typedBlueprint = blueprint as {
    sceneView?: { center?: { x?: number; y?: number } };
    originalMapRegion?: {
      playBounds?: { minX: number; maxX: number; minY: number; maxY: number };
      sprites?: Record<string, { kind?: string; drawMode?: string; frames?: Array<{ path?: string }> }>;
      cells?: Array<{ x?: number; y?: number; back?: string; middle?: string; front?: string; tileAnimation?: string }>;
    } | null;
  };
  const region = typedBlueprint.originalMapRegion;
  const sprites = region?.sprites;
  if (!sprites) return [];

  const urls: string[] = [];
  let sourceFrameCount = 0;
  for (const spriteKey of rankSceneSpriteKeys(typedBlueprint)) {
    const sprite = sprites[spriteKey];
    if (!sprite) continue;
    for (const frame of sprite.frames ?? []) {
      if (frame.path?.startsWith("/original-map/")) {
        sourceFrameCount += 1;
        const renderPath = mapSpriteRenderPrewarmPath(frame.path);
        if (options.includeRawMapFrames) urls.push(frame.path);
        if (renderPath !== frame.path) urls.push(renderPath);
      }
      if (sourceFrameCount >= limit) return Array.from(new Set(urls));
    }
  }

  return Array.from(new Set(urls));
}

function rankSceneSpriteKeys(blueprint: {
  sceneView?: { center?: { x?: number; y?: number } };
  originalMapRegion?: {
    playBounds?: { minX: number; maxX: number; minY: number; maxY: number };
    sprites?: Record<string, { kind?: string; drawMode?: string }>;
    cells?: Array<{ x?: number; y?: number; back?: string; middle?: string; front?: string; tileAnimation?: string }>;
  } | null;
}) {
  const region = blueprint.originalMapRegion;
  const sprites = region?.sprites ?? {};
  const center = scenePrewarmCenter(blueprint);
  const seen = new Map<string, { distance: number; priority: number; layer: number; order: number }>();
  let order = 0;

  for (const cell of region?.cells ?? []) {
    const x = typeof cell.x === "number" ? cell.x : center.x;
    const y = typeof cell.y === "number" ? cell.y : center.y;
    const distance = Math.max(Math.abs(x - center.x), Math.abs(y - center.y));
    for (const [layer, key] of [cell.front, cell.middle, cell.tileAnimation, cell.back].entries()) {
      if (!key || !(key in sprites)) continue;
      const priority = sceneSpritePrewarmPriority(sprites[key], layer);
      const previous = seen.get(key);
      if (
        !previous ||
        priority < previous.priority ||
        (priority === previous.priority && distance < previous.distance) ||
        (priority === previous.priority && distance === previous.distance && layer < previous.layer)
      ) {
        seen.set(key, { distance, priority, layer, order });
      }
    }
    order += 1;
  }

  const ranked = Array.from(seen.entries())
    .sort((a, b) => a[1].priority - b[1].priority || a[1].distance - b[1].distance || a[1].layer - b[1].layer || a[1].order - b[1].order)
    .map(([key]) => key);
  for (const key of Object.keys(sprites)) {
    if (!seen.has(key)) ranked.push(key);
  }
  return ranked;
}

function sceneSpritePrewarmPriority(sprite: { kind?: string; drawMode?: string } | undefined, layer: number) {
  if (sprite?.drawMode === "object") return 0;
  if (sprite?.kind === "front") return 1;
  if (sprite?.kind === "middle" || sprite?.kind === "tileAnimation") return 2;
  if (sprite?.kind === "back") return 4;
  return 3 + layer;
}

function scenePrewarmCenter(blueprint: {
  sceneView?: { center?: { x?: number; y?: number } };
  originalMapRegion?: { playBounds?: { minX: number; maxX: number; minY: number; maxY: number } } | null;
}) {
  const center = blueprint.sceneView?.center;
  if (typeof center?.x === "number" && typeof center.y === "number") {
    return center as { x: number; y: number };
  }
  const bounds = blueprint.originalMapRegion?.playBounds;
  if (bounds) {
    return {
      x: Math.round((bounds.minX + bounds.maxX) / 2),
      y: Math.round((bounds.minY + bounds.maxY) / 2),
    };
  }
  return { x: 0, y: 0 };
}

function recordResourceEntry(metrics: CacheMetricsHandle, entry: PerformanceResourceTiming) {
  const url = safeUrl(entry.name);
  if (!url || url.origin !== window.location.origin) return;

  const kind = classifyUrl(url);
  if (kind === "other") return;

  const deliveryType = (entry as PerformanceResourceTiming & { deliveryType?: string }).deliveryType ?? null;
  const resourceKey = `${entry.name}:${entry.startTime}:${entry.duration}:${entry.transferSize}`;
  if (metrics.resourceKeys.has(resourceKey)) return;
  metrics.resourceKeys.add(resourceKey);

  const metric: CacheResourceMetric = {
    url: url.href,
    path: `${url.pathname}${url.search}`,
    kind,
    initiatorType: entry.initiatorType,
    durationMs: entry.duration,
    transferSize: entry.transferSize,
    encodedBodySize: entry.encodedBodySize,
    decodedBodySize: entry.decodedBodySize,
    cachedLike: deliveryType === "cache" || (entry.transferSize === 0 && entry.encodedBodySize > 0),
    deliveryType,
    startTime: entry.startTime,
  };

  metrics.resources.push(metric);
  trimArray(metrics.resources, MAX_RESOURCE_METRICS);
}

function recordApiMetric(metrics: CacheMetricsHandle, metric: CacheApiMetric) {
  metrics.apiRequests.push(metric);
  trimArray(metrics.apiRequests, MAX_API_METRICS);
}

async function refreshCacheStorageMetrics(metrics: CacheMetricsHandle) {
  const unsupported: CacheStorageMetric = {
    supported: false,
    updatedAtMs: performance.now() - metrics.startedAt,
    cacheCount: 0,
    entryCount: 0,
    caches: [],
    usageBytes: null,
    quotaBytes: null,
    persisted: null,
    persistAttempted: false,
    persistGranted: null,
  };

  if (!("caches" in window)) {
    metrics.cacheStorage = unsupported;
    return unsupported;
  }

  try {
    const names = (await caches.keys()).filter((name) => name.startsWith("mir2-asset-cache"));
    const cacheEntries = await Promise.all(
      names.sort().map(async (name) => {
        const cache = await caches.open(name);
        const entries = await cache.keys();
        return { name, entries: entries.length };
      }),
    );
    const storageManager = navigator.storage;
    const estimate = await storageManager?.estimate?.();
    const persisted = await storageManager?.persisted?.();
    const metric: CacheStorageMetric = {
      supported: true,
      updatedAtMs: performance.now() - metrics.startedAt,
      cacheCount: cacheEntries.length,
      entryCount: cacheEntries.reduce((sum, entry) => sum + entry.entries, 0),
      caches: cacheEntries,
      usageBytes: typeof estimate?.usage === "number" ? estimate.usage : null,
      quotaBytes: typeof estimate?.quota === "number" ? estimate.quota : null,
      persisted: typeof persisted === "boolean" ? persisted : null,
      persistAttempted: metrics.cacheStorage?.persistAttempted ?? false,
      persistGranted: metrics.cacheStorage?.persistGranted ?? null,
    };
    metrics.cacheStorage = metric;
    return metric;
  } catch (error) {
    const metric: CacheStorageMetric = {
      ...unsupported,
      supported: true,
      error: error instanceof Error ? error.message : String(error),
    };
    metrics.cacheStorage = metric;
    return metric;
  }
}

async function requestPersistentStorage(metrics: CacheMetricsHandle) {
  if (!navigator.storage?.persist || !navigator.storage.persisted) return;
  const current = await refreshCacheStorageMetrics(metrics);
  if (current.persisted === true || current.persistAttempted) return;

  try {
    const granted = await navigator.storage.persist();
    const next = await refreshCacheStorageMetrics(metrics);
    next.persistAttempted = true;
    next.persistGranted = granted;
    next.persisted = granted || next.persisted;
    metrics.cacheStorage = next;
    metrics.markMilestone("storagePersistRequest", {
      granted,
      persisted: next.persisted,
    });
  } catch (error) {
    const next = await refreshCacheStorageMetrics(metrics);
    next.persistAttempted = true;
    next.persistGranted = false;
    next.error = error instanceof Error ? error.message : String(error);
    metrics.cacheStorage = next;
    metrics.markMilestone("storagePersistRequest", {
      granted: false,
      error: next.error,
    });
  }
}

function installAssetCacheReset(metrics: CacheMetricsHandle) {
  const resetAssetCache = async (options: AssetCacheResetOptions = {}): Promise<AssetCacheResetResult> => {
    const shouldReload = options.reload !== false;
    const deletedCaches = await deleteMir2CacheStorage();
    const unregisteredScopes = await unregisterMir2AssetWorkers();

    window.__mir2AssetCache = {
      enabled: false,
      status: "reset",
      version: window.__mir2AssetCache?.version,
      prewarm: "cleared",
    };

    await refreshCacheStorageMetrics(metrics);
    metrics.markMilestone("assetCacheReset", {
      deletedCaches: deletedCaches.length,
      unregisteredScopes: unregisteredScopes.length,
      reload: shouldReload,
    });

    if (shouldReload) {
      window.setTimeout(() => window.location.reload(), 50);
    }

    return {
      deletedCaches,
      unregisteredScopes,
      reloaded: shouldReload,
    };
  };

  window.__mir2AssetCacheReset = resetAssetCache;
  return resetAssetCache;
}

async function deleteMir2CacheStorage() {
  if (!("caches" in window)) return [];
  const names = (await caches.keys()).filter((name) => name.startsWith("mir2-asset-cache"));
  const results = await Promise.all(
    names.map(async (name) => ({
      name,
      deleted: await caches.delete(name),
    })),
  );
  return results.filter((result) => result.deleted).map((result) => result.name);
}

async function unregisterMir2AssetWorkers() {
  if (!("serviceWorker" in navigator)) return [];
  const registrations = await navigator.serviceWorker.getRegistrations();
  const scopes: string[] = [];

  for (const registration of registrations) {
    if (!isMir2AssetWorkerRegistration(registration)) continue;
    const unregistered = await registration.unregister();
    if (unregistered) scopes.push(registration.scope);
  }

  return scopes;
}

function isMir2AssetWorkerRegistration(registration: ServiceWorkerRegistration) {
  const workers = [registration.active, registration.waiting, registration.installing].filter(Boolean);
  if (workers.some((worker) => worker?.scriptURL.includes("/mir2-asset-worker.js"))) return true;

  const rootScope = new URL("/", window.location.href).href;
  return registration.scope === rootScope && window.__mir2AssetCache?.enabled === true;
}

function summarizeMetrics(metrics: CacheMetricsHandle): CacheMetricsSummary {
  const gameResources = metrics.resources.filter((entry) => entry.kind !== "asset-manifest");
  const sceneRequests = metrics.apiRequests.filter((entry) => entry.kind === "scene-api");
  const sceneHits = sceneRequests.filter((entry) => entry.sceneCache === "hit").length;
  const sceneMisses = sceneRequests.filter((entry) => entry.sceneCache === "miss").length;
  const latestSceneRequest = sceneRequests.at(-1) ?? null;
  const resourceRuntime = currentResourceRuntimeMetrics();
  // Superseded background runs are intentionally aborted. Excluding them keeps
  // current-screen progress truthful instead of permanently pinning it below
  // 100% after an obsolete pack was cancelled halfway through.
  const activePrewarmRuns = metrics.prewarmRuns.filter((run) => run.status !== "cancelled");
  const prewarmRequested = activePrewarmRuns.reduce((sum, run) => sum + run.requested, 0);
  const prewarmOk = activePrewarmRuns.reduce((sum, run) => sum + run.ok, 0);
  const prewarmFailed = activePrewarmRuns.reduce((sum, run) => sum + run.failed, 0);
  const slowest = [...gameResources]
    .sort((a, b) => b.durationMs - a.durationMs)
    .slice(0, 5)
    .map((entry) => ({
      path: entry.path,
      kind: entry.kind,
      durationMs: entry.durationMs,
      transferSize: entry.transferSize,
    }));

  return {
    uptimeMs: performance.now() - metrics.startedAt,
    firstPlayableMs: metrics.milestones.find((milestone) => milestone.name === "firstPlayableFrame")?.atMs ?? null,
    resourceCount: metrics.resources.length,
    gameResourceCount: gameResources.length,
    transferBytes: gameResources.reduce((sum, entry) => sum + entry.transferSize, 0),
    encodedBytes: gameResources.reduce((sum, entry) => sum + entry.encodedBodySize, 0),
    cachedLikeCount: gameResources.filter((entry) => entry.cachedLike).length,
    sceneRequests: sceneRequests.length,
    sceneHits,
    sceneMisses,
    latestSceneCacheKey: latestSceneRequest?.sceneCacheKey ?? null,
    latestOriginalMapSpriteCount: latestSceneRequest?.originalMapSpriteCount ?? null,
    latestOriginalMapCellCount: latestSceneRequest?.originalMapCellCount ?? null,
    loadedSpriteLibraryCount: resourceRuntime.loadedSpriteLibraryCount,
    domImageCount: resourceRuntime.domImageCount,
    bevyAtlasCount: resourceRuntime.bevyAtlasCount,
    bevyAtlasPixelBytes: resourceRuntime.bevyAtlasPixelBytes,
    alphaKeyedBlobCount: resourceRuntime.alphaKeyedBlobCount,
    alphaKeyedBlobBytes: resourceRuntime.alphaKeyedBlobBytes,
    prewarmRequested,
    prewarmOk,
    prewarmFailed,
    cacheStorageCacheCount: metrics.cacheStorage?.cacheCount ?? 0,
    cacheStorageEntryCount: metrics.cacheStorage?.entryCount ?? 0,
    storageUsageBytes: metrics.cacheStorage?.usageBytes ?? null,
    storageQuotaBytes: metrics.cacheStorage?.quotaBytes ?? null,
    storagePersisted: metrics.cacheStorage?.persisted ?? null,
    storagePersistGranted: metrics.cacheStorage?.persistGranted ?? null,
    slowest,
  };
}

function publishPrewarmProgress(
  metrics: CacheMetricsHandle,
  status: AssetCachePrewarmStatus,
) {
  const summary = summarizeMetrics(metrics);
  const activeRun = metrics.prewarmRuns.find((run) => run.status === "running") ?? null;
  const completed = summary.prewarmOk + summary.prewarmFailed;
  const percent =
    summary.prewarmRequested > 0
      ? Math.min(100, Math.round((completed / summary.prewarmRequested) * 100))
      : 0;

  window.__mir2AssetCache = {
    ...(window.__mir2AssetCache ?? { enabled: false, status: "unknown" }),
    prewarm: status,
    prewarmProgress: {
      status,
      activePackName: activeRun?.name ?? null,
      activePackLabel: activeRun?.label ?? null,
      requested: summary.prewarmRequested,
      completed,
      ok: summary.prewarmOk,
      failed: summary.prewarmFailed,
      percent,
      cacheStorageEntryCount: summary.cacheStorageEntryCount,
      transferBytes: summary.transferBytes,
    },
  };

  logCacheProgress(metrics, status, summary, activeRun);
}

function logCacheEvent(metrics: CacheMetricsHandle, event: string, detail: Record<string, unknown>) {
  if (!metrics.consoleLog) return;
  console.info("[mir2-cache]", {
    event,
    atMs: Math.round(performance.now() - metrics.startedAt),
    ...detail,
  });
}

function logCacheProgress(
  metrics: CacheMetricsHandle,
  status: AssetCachePrewarmStatus,
  summary: CacheMetricsSummary,
  activeRun: CachePrewarmMetric | null,
) {
  if (!metrics.consoleLog) return;

  const completed = summary.prewarmOk + summary.prewarmFailed;
  const percent =
    summary.prewarmRequested > 0
      ? Math.min(100, Math.round((completed / summary.prewarmRequested) * 100))
      : 0;
  const percentBucket = status === "done" || status === "error" ? status : Math.floor(percent / 10) * 10;
  const key = `${status}:${activeRun?.name ?? "all"}:${percentBucket}:${summary.prewarmFailed}`;
  if (metrics.lastProgressLogKey === key) return;
  metrics.lastProgressLogKey = key;

  const payload = {
    event: "prewarm-progress",
    status,
    activePack: activeRun
      ? {
          name: activeRun.name,
          label: activeRun.label,
          status: activeRun.status,
          requested: activeRun.requested,
          ok: activeRun.ok,
          failed: activeRun.failed,
        }
      : null,
    progress: {
      percent,
      completed,
      requested: summary.prewarmRequested,
      ok: summary.prewarmOk,
      failed: summary.prewarmFailed,
    },
    resources: {
      total: summary.resourceCount,
      game: summary.gameResourceCount,
      cachedLike: summary.cachedLikeCount,
      transferBytes: summary.transferBytes,
      transfer: formatBytes(summary.transferBytes),
      encodedBytes: summary.encodedBytes,
      encoded: formatBytes(summary.encodedBytes),
    },
    cacheStorage: {
      caches: summary.cacheStorageCacheCount,
      entries: summary.cacheStorageEntryCount,
      usageBytes: summary.storageUsageBytes,
      usage: summary.storageUsageBytes == null ? null : formatBytes(summary.storageUsageBytes),
      quotaBytes: summary.storageQuotaBytes,
      persisted: summary.storagePersisted,
      persistGranted: summary.storagePersistGranted,
    },
    remoteAssetBaseUrl: window.__mir2AssetCache?.remoteAssetBaseUrl ?? null,
    remoteAssetFallbackBaseUrls:
      window.__mir2AssetCache?.remoteAssetFallbackBaseUrls ?? [],
    failedUrls: activeRun?.failedUrls.slice(0, 5) ?? [],
    atMs: Math.round(performance.now() - metrics.startedAt),
  };

  if (summary.prewarmFailed > 0 || status === "error") {
    console.warn("[mir2-cache-progress]", payload);
    return;
  }

  console.log("[mir2-cache-progress]", payload);
}

function shouldShowCacheProgress(snapshot: CacheMetricsSnapshot) {
  const summary = snapshot.summary;
  if (summary.prewarmRequested <= 0) return false;
  // The panel is a loading INDICATOR pinned over the HP orb (bottom-left), not a
  // permanent HUD element — once the prewarm has caught up it must hide, or it sits
  // on top of the HUD for the whole session reading as "完成 100%" forever (the
  // gating returned true whenever any prewarm was ever requested). It re-appears if a
  // later wave requests more, then hides again. Keep it visible while anything failed
  // so the user still sees the failure count.
  const completed = summary.prewarmOk + summary.prewarmFailed;
  if (completed >= summary.prewarmRequested && summary.prewarmFailed === 0) {
    return false;
  }
  return true;
}

function getActivePrewarmRun(snapshot: CacheMetricsSnapshot) {
  return (
    snapshot.prewarmRuns.find((run) => run.status === "running") ??
    snapshot.prewarmRuns[snapshot.prewarmRuns.length - 1] ??
    null
  );
}

function resolveFetchUrl(input: RequestInfo | URL) {
  if (typeof input === "string" || input instanceof URL) return safeUrl(String(input));
  if (input instanceof Request) return safeUrl(input.url);
  return null;
}

function classifyUrl(url: URL): CacheResourceKind {
  if (url.pathname.startsWith("/original-ui/")) return "original-ui";
  if (url.pathname.startsWith("/original-map/")) return "original-map";
  if (url.pathname.startsWith("/generated/original-map-blend/")) return "generated-map-blend";
  if (url.pathname.startsWith("/bevy-entity-atlases/")) return "bevy-entity-atlas";
  if (url.pathname.startsWith("/bevy-runtime/")) return "bevy-runtime";
  if (url.pathname === "/api/scene/crystal") return "scene-api";
  if (url.pathname === "/api/asset-manifest") return "asset-manifest";
  if (url.pathname === "/api/original-ui-meta") return "original-ui-meta";
  return "other";
}

function safeUrl(value: string) {
  try {
    return new URL(value, window.location.origin);
  } catch {
    return null;
  }
}

function numberHeader(value: string | null) {
  if (value == null) return null;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

function currentResourceRuntimeMetrics() {
  const debug = (window as typeof window & {
    __mir2BevyEntityRendererDebug?: {
      atlasCount?: number;
      atlasPixelBytes?: number;
      spriteLibraryCache?: { loadedLibraryCount?: number };
      sceneAssetRuntime?: {
        alphaKeyedBlobCount?: number;
        alphaKeyedBlobBytes?: number;
      };
    };
  }).__mir2BevyEntityRendererDebug;
  return {
    loadedSpriteLibraryCount: debug?.spriteLibraryCache?.loadedLibraryCount ?? null,
    domImageCount: document.images.length,
    bevyAtlasCount: typeof debug?.atlasCount === "number" ? debug.atlasCount : null,
    bevyAtlasPixelBytes: typeof debug?.atlasPixelBytes === "number" ? debug.atlasPixelBytes : null,
    alphaKeyedBlobCount:
      typeof debug?.sceneAssetRuntime?.alphaKeyedBlobCount === "number"
        ? debug.sceneAssetRuntime.alphaKeyedBlobCount
        : null,
    alphaKeyedBlobBytes:
      typeof debug?.sceneAssetRuntime?.alphaKeyedBlobBytes === "number"
        ? debug.sceneAssetRuntime.alphaKeyedBlobBytes
        : null,
  };
}

function scheduleIdle(callback: () => void) {
  const idleCallback = window.requestIdleCallback as
    | ((callback: () => void, options?: { timeout: number }) => number)
    | undefined;
  if (idleCallback) {
    idleCallback(callback, { timeout: 2500 });
    return;
  }
  window.setTimeout(callback, 250);
}

function resolveBackgroundPrewarmModeOverride(params: URLSearchParams): BackgroundPrewarmMode | null {
  const configured = params.get("prewarmBackground");
  if (configured === "off" || configured === "immediate" || configured === "afterPlayable") return configured;
  if (params.get("skipRuntime") === "1") return "immediate";
  return null;
}

function sleep(ms: number, signal?: AbortSignal) {
  if (!signal) return new Promise<void>((resolve) => window.setTimeout(resolve, ms));
  if (signal.aborted) return Promise.reject(createAbortError());
  return new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    const onAbort = () => {
      window.clearTimeout(timer);
      signal.removeEventListener("abort", onAbort);
      reject(createAbortError());
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

function isServiceWorkerSafeOrigin() {
  if (window.isSecureContext) return true;
  return window.location.hostname === "localhost" || window.location.hostname === "127.0.0.1";
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatStaticAssetTiers(tiers: Record<string, number> | null | undefined) {
  if (!tiers) return "pending";
  return `C${tiers.critical ?? 0} / B${tiers.background ?? 0} / R${tiers.runtime ?? 0}`;
}

function shortPath(path: string) {
  return path.length > 42 ? `...${path.slice(-39)}` : path;
}

function mapSpriteRenderPrewarmPath(path: string) {
  const frame = path.match(/\/original-map\/WemadeMir2\/Objects\/(27(?:2[3-9]|3[0-2]))\.png$/i)?.[1];
  return frame ? `/generated/original-map-blend/WemadeMir2/Objects/${frame}.png` : path;
}

function trimArray<T>(items: T[], maxLength: number) {
  if (items.length > maxLength) items.splice(0, items.length - maxLength);
}
