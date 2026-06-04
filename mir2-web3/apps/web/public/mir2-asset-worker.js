const CACHE_PREFIX = "mir2-asset-cache";
const CACHE_SCHEMA_VERSION = "sw4";
const DEFAULT_VERSION = "bootstrap";

let runtimeConfig = {
  version: DEFAULT_VERSION,
  assetVersion: DEFAULT_VERSION,
  staticAssetMaxEntries: 20000,
  staticCriticalMaxEntries: 3000,
  staticBackgroundMaxEntries: 6000,
  staticRuntimeMaxEntries: 16000,
  sceneBlueprintMaxEntries: 512,
  apiMetadataMaxEntries: 512,
  remoteAssetBaseUrl: "",
};
let staticAssetTiers = new Map();

// Short-lived negative cache for confirmed missing assets. Do not cache
// transient network/abort failures: gameplay prefetch can fan out hundreds of
// requests, and one momentary 504 must not blacklist a valid animation frame.
const NEGATIVE_CACHE_TTL_MS = 10_000;
const NEGATIVE_CACHE_MAX_ENTRIES = 2000;
const staticAssetNegativeCache = new Map();
const staticAssetInflightFetches = new Map();
const sceneBlueprintInflightFetches = new Map();
const TRANSIENT_FETCH_RETRY_DELAYS_MS = [120, 350];
const TRANSIENT_FETCH_STATUSES = new Set([408, 425, 429, 500, 502, 503, 504]);

self.addEventListener("install", (event) => {
  event.waitUntil(self.skipWaiting());
});

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener("message", (event) => {
  const data = event.data || {};
  if (data.type === "MIR2_ASSET_CACHE_RESET") {
    event.waitUntil(
      deleteAllMir2Caches()
        .then((deletedCaches) => {
          postClientMessage(event, "MIR2_ASSET_CACHE_RESET_DONE", {
            deletedCaches,
            version: runtimeConfig.version,
          });
        })
        .catch((error) => {
          postClientMessage(event, "MIR2_ASSET_CACHE_RESET_ERROR", {
            message: error instanceof Error ? error.message : String(error),
          });
        }),
    );
    return;
  }

  if (data.type === "MIR2_ASSET_CACHE_STATUS") {
    event.waitUntil(
      readCacheStatus().then((status) => {
        postClientMessage(event, "MIR2_ASSET_CACHE_STATUS", status);
      }),
    );
    return;
  }

  if (data.type === "MIR2_ASSET_CACHE_HINT") {
    const result = addStaticAssetTierHints(data.urls, data.cacheTier);
    postClientMessage(event, "MIR2_ASSET_CACHE_HINTED", result);
    return;
  }

  if (data.type !== "MIR2_ASSET_CACHE_CONFIG") return;

  const manifest = data.manifest || {};
  const caches = manifest.runtimeCaches || {};
  const remoteAssets = manifest.remoteAssets || {};
  const staticAssetMaxEntries = positiveNumber(caches.staticAssetMaxEntries, 20000);
  const assetVersion = String(data.manifestVersion || manifest.version || DEFAULT_VERSION);
  runtimeConfig = {
    version: String(data.cacheNamespace || manifest.cacheNamespace || assetVersion || DEFAULT_VERSION),
    assetVersion,
    staticAssetMaxEntries,
    staticCriticalMaxEntries: positiveNumber(caches.staticCriticalMaxEntries, Math.min(staticAssetMaxEntries, 3000)),
    staticBackgroundMaxEntries: positiveNumber(caches.staticBackgroundMaxEntries, Math.min(staticAssetMaxEntries, 6000)),
    staticRuntimeMaxEntries: positiveNumber(caches.staticRuntimeMaxEntries, staticAssetMaxEntries),
    sceneBlueprintMaxEntries: positiveNumber(caches.sceneBlueprintMaxEntries, 512),
    apiMetadataMaxEntries: positiveNumber(caches.apiMetadataMaxEntries, 512),
    remoteAssetBaseUrl: normalizeAssetBaseUrl(
      data.assetBaseUrl || remoteAssets.assetBaseUrl || manifest.assetBaseUrl || "",
    ),
  };
  staticAssetTiers = buildStaticAssetTierIndex(manifest.resourcePacks || []);

  event.waitUntil(
    cleanupOldCaches(runtimeConfig.version).then((deletedCaches) => {
      postClientMessage(event, "MIR2_ASSET_CACHE_CONFIGURED", {
        deletedCaches,
        version: runtimeConfig.version,
        assetVersion: runtimeConfig.assetVersion,
        cacheNamespace: runtimeConfig.version,
        remoteAssetBaseUrl: runtimeConfig.remoteAssetBaseUrl || null,
        staticAssetTiers: summarizeStaticAssetTiers(),
      });
    }),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  if (url.pathname === "/cdn-cgi/rum") {
    event.respondWith(new Response(null, { status: 204, headers: { "cache-control": "no-store" } }));
    return;
  }

  if (request.method !== "GET") return;
  if (runtimeConfig.version === DEFAULT_VERSION) return;

  if (isStaticGameAsset(url)) {
    const policy = staticAssetCachePolicy(request);
    event.respondWith(
      cacheFirst(request, cacheName(policy.cacheKind), policy.maxEntries, event),
    );
    return;
  }

  if (url.pathname === "/api/scene/crystal") {
    event.respondWith(
      staleWhileRevalidate(
        request,
        cacheName("scene"),
        runtimeConfig.sceneBlueprintMaxEntries,
        event,
      ),
    );
    return;
  }

  if (url.pathname === "/api/original-ui-meta" || url.pathname === "/api/asset-manifest") {
    event.respondWith(
      networkFirst(request, cacheName("api"), runtimeConfig.apiMetadataMaxEntries),
    );
  }
});

function isStaticGameAsset(url) {
  return (
    url.pathname.startsWith("/original-ui/") ||
    url.pathname.startsWith("/original-map/") ||
    url.pathname.startsWith("/generated/original-map-blend/") ||
    url.pathname.startsWith("/bevy-entity-atlases/")
  );
}

function cacheName(kind) {
  return `${CACHE_PREFIX}-${CACHE_SCHEMA_VERSION}-${kind}-${runtimeConfig.version || DEFAULT_VERSION}`;
}

async function cacheFirst(request, name, maxEntries, event) {
  // This is fed directly to event.respondWith(): it must ALWAYS resolve.
  // A rejected promise here surfaces as an uncaught "Failed to fetch" and can
  // hard-fail dependent subsystems (e.g. chunk loading), so every failure path
  // resolves to a cached entry or a graceful synthetic Response instead.
  let cache;
  try {
    cache = await caches.open(name);
    const cached = await cache.match(request);
    if (cached) {
      event?.waitUntil(touchCacheEntry(cache, request, cached, maxEntries));
      return cached;
    }
  } catch {
    // CacheStorage can be unavailable (private mode, quota teardown). Fall
    // through to a direct fetch and never let it become an uncaught rejection.
    cache = null;
  }

  const response = await coalescedStaticAssetFetch(request);
  if (cache && response && response.ok) {
    await putCacheEntry(cache, request, response, maxEntries);
  }
  return response;
}

async function touchCacheEntry(cache, request, response, maxEntries) {
  try {
    await cache.put(request, response.clone());
    await trimCache(cache, maxEntries);
  } catch {
    // Cache recency updates are opportunistic.
  }
}

async function fetchStaticAsset(request) {
  // Never throws. Resolves to the best available response and, on total
  // failure, to a synthetic 504 so callers (cacheFirst -> respondWith) always
  // receive a settled promise instead of an uncaught network rejection.
  const negativeKey = request.url;
  const negativeStatus = getNegativeCacheStatus(negativeKey);
  if (negativeStatus) {
    return syntheticAssetResponse(negativeStatus, "asset recently unavailable");
  }

  let originResponse = null;
  originResponse = await fetchWithTransientRetries(request);
  if (originResponse?.ok) return originResponse;

  const remoteRequest = createRemoteAssetRequest(request);
  if (remoteRequest) {
    const remoteResponse = await fetchWithTransientRetries(remoteRequest);
    if (remoteResponse) {
      if (remoteResponse.ok) return remoteResponse;
      if (remoteResponse.status === 404 || !originResponse) {
        if (remoteResponse.status === 404 || remoteResponse.status === 410) {
          rememberNegativeResult(negativeKey, remoteResponse.status);
        }
        return remoteResponse;
      }
    }
  }

  if (originResponse) {
    if (!originResponse.ok && (originResponse.status === 404 || originResponse.status === 410)) {
      rememberNegativeResult(negativeKey, originResponse.status);
    }
    return originResponse;
  }

  // Both the app origin and the remote asset origin rejected (offline / DNS /
  // aborted). Degrade gracefully for this request, but do not negative-cache
  // transient 504s because the next frame fetch may succeed immediately.
  return syntheticAssetResponse(504, "asset fetch failed");
}

async function coalescedStaticAssetFetch(request) {
  return coalescedResponse(staticAssetInflightFetches, request.url, () => fetchStaticAsset(request));
}

async function coalescedSceneBlueprintFetch(request) {
  return coalescedResponse(sceneBlueprintInflightFetches, request.url, () =>
    fetchWithTransientRetries(request),
  );
}

async function coalescedResponse(inflightMap, key, fetcher) {
  let inflight = inflightMap.get(key);
  if (!inflight) {
    inflight = fetcher().finally(() => {
      inflightMap.delete(key);
    });
    inflightMap.set(key, inflight);
  }

  const response = await inflight;
  if (!response) return null;
  try {
    return response.clone();
  } catch {
    return response;
  }
}

async function fetchWithTransientRetries(request) {
  let lastResponse = null;
  for (let attempt = 0; attempt <= TRANSIENT_FETCH_RETRY_DELAYS_MS.length; attempt += 1) {
    try {
      const response = await fetch(cloneRequestForRetry(request));
      if (!isTransientFetchStatus(response.status)) return response;
      lastResponse = response;
    } catch {
      lastResponse = null;
    }

    if (attempt < TRANSIENT_FETCH_RETRY_DELAYS_MS.length) {
      await delay(TRANSIENT_FETCH_RETRY_DELAYS_MS[attempt]);
    }
  }
  return lastResponse;
}

function cloneRequestForRetry(request) {
  return request instanceof Request ? request.clone() : request;
}

function isTransientFetchStatus(status) {
  return TRANSIENT_FETCH_STATUSES.has(status);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function syntheticAssetResponse(status, reason) {
  return new Response(null, {
    status,
    statusText: reason,
    headers: {
      "cache-control": "no-store",
      "x-mir2-asset-fallback": String(status),
    },
  });
}

function getNegativeCacheStatus(key) {
  const entry = staticAssetNegativeCache.get(key);
  if (!entry) return 0;
  if (entry.expiresAt <= Date.now()) {
    staticAssetNegativeCache.delete(key);
    return 0;
  }
  return entry.status;
}

function rememberNegativeResult(key, status) {
  if (status !== 404 && status !== 410) return;
  staticAssetNegativeCache.set(key, { status, expiresAt: Date.now() + NEGATIVE_CACHE_TTL_MS });
  if (staticAssetNegativeCache.size > NEGATIVE_CACHE_MAX_ENTRIES) {
    const oldestKey = staticAssetNegativeCache.keys().next().value;
    if (oldestKey !== undefined) staticAssetNegativeCache.delete(oldestKey);
  }
}

function createRemoteAssetRequest(request) {
  if (!runtimeConfig.remoteAssetBaseUrl) return null;

  const localUrl = new URL(request.url);
  if (!isRemoteBackedStaticGameAsset(localUrl)) return null;

  const relativePath = localUrl.pathname.replace(/^\/+/, "");
  const remoteUrl = new URL(relativePath, `${runtimeConfig.remoteAssetBaseUrl}/`);
  remoteUrl.search = localUrl.search;

  if (remoteUrl.origin === localUrl.origin && remoteUrl.pathname === localUrl.pathname) {
    return null;
  }

  return new Request(remoteUrl.href, {
    method: "GET",
    mode: "cors",
    credentials: "omit",
    redirect: "follow",
  });
}

function isRemoteBackedStaticGameAsset(url) {
  return (
    url.pathname.startsWith("/original-ui/") ||
    url.pathname.startsWith("/original-map/") ||
    url.pathname.startsWith("/generated/original-map-blend/") ||
    url.pathname.startsWith("/bevy-entity-atlases/")
  );
}

async function staleWhileRevalidate(request, name, maxEntries, event) {
  let cache = null;
  let cached = null;
  try {
    cache = await caches.open(name);
    cached = await cache.match(request);
  } catch {
    // Scene cache is an optimization. Network retry is still the source of truth.
  }

  const refresh = coalescedSceneBlueprintFetch(request)
    .then(async (response) => {
      if (cache) {
        await putCacheEntry(cache, request, response, maxEntries);
      }
      return response;
    })
    .catch(() => null);

  if (cached) {
    event.waitUntil(refresh.catch(() => null));
    return cached;
  }

  const response = await refresh;
  if (response) return response;

  return new Response(
    JSON.stringify({
      error: "scene blueprint unavailable",
      retryable: true,
    }),
    {
      status: 503,
      headers: {
        "cache-control": "no-store",
        "content-type": "application/json",
      },
    },
  );
}

async function networkFirst(request, name, maxEntries) {
  const cache = await caches.open(name);
  try {
    const response = await fetch(request);
    await putCacheEntry(cache, request, response, maxEntries);
    return response;
  } catch (error) {
    const cached = await cache.match(request);
    if (cached) return cached;
    throw error;
  }
}

async function putCacheEntry(cache, request, response, maxEntries) {
  if (!response || !response.ok) return;
  try {
    await cache.put(request, response.clone());
    await trimCache(cache, maxEntries);
  } catch (error) {
    await trimCache(cache, Math.max(32, Math.floor(maxEntries / 2)));
    try {
      await cache.put(request, response.clone());
    } catch {
      // Quota pressure should never break gameplay.
    }
  }
}

async function trimCache(cache, maxEntries) {
  const keys = await cache.keys();
  if (keys.length <= maxEntries) return;

  const deleteCount = keys.length - maxEntries;
  await Promise.all(keys.slice(0, deleteCount).map((key) => cache.delete(key)));
}

async function cleanupOldCaches(activeVersion) {
  const names = await caches.keys();
  const staleNames = names
    .filter((name) => name.startsWith(CACHE_PREFIX))
    .filter((name) => !name.endsWith(`-${activeVersion}`));
  const results = await Promise.all(
    staleNames.map(async (name) => ({
      name,
      deleted: await caches.delete(name),
    })),
  );
  return results.filter((result) => result.deleted).map((result) => result.name);
}

async function deleteAllMir2Caches() {
  const names = await caches.keys();
  const mir2Names = names.filter((name) => name.startsWith(CACHE_PREFIX));
  const results = await Promise.all(
    mir2Names.map(async (name) => ({
      name,
      deleted: await caches.delete(name),
    })),
  );
  return results.filter((result) => result.deleted).map((result) => result.name);
}

async function readCacheStatus() {
  const names = (await caches.keys()).filter((name) => name.startsWith(CACHE_PREFIX)).sort();
  const entries = await Promise.all(
    names.map(async (name) => {
      const cache = await caches.open(name);
      const keys = await cache.keys();
      return { name, entries: keys.length };
    }),
  );
  return {
    version: runtimeConfig.version,
    remoteAssetBaseUrl: runtimeConfig.remoteAssetBaseUrl || null,
    staticAssetTiers: summarizeStaticAssetTiers(),
    cacheCount: entries.length,
    entryCount: entries.reduce((sum, entry) => sum + entry.entries, 0),
    caches: entries,
  };
}

function staticAssetCachePolicy(request) {
  const url = new URL(request.url);
  const tier = staticAssetTiers.get(staticAssetCacheKey(url)) || "runtime";
  if (tier === "critical") {
    return {
      cacheKind: "static-critical",
      maxEntries: runtimeConfig.staticCriticalMaxEntries,
    };
  }
  if (tier === "background") {
    return {
      cacheKind: "static-background",
      maxEntries: runtimeConfig.staticBackgroundMaxEntries,
    };
  }
  return {
    cacheKind: "static-runtime",
    maxEntries: runtimeConfig.staticRuntimeMaxEntries,
  };
}

function buildStaticAssetTierIndex(resourcePacks) {
  const tiers = new Map();
  if (!Array.isArray(resourcePacks)) return tiers;

  for (const pack of resourcePacks) {
    const tier = normalizeStaticAssetTier(pack?.cacheTier || (pack?.phase === "background" ? "background" : "critical"));
    const urls = Array.isArray(pack?.urls) ? pack.urls : [];
    for (const value of urls) {
      const key = normalizeStaticAssetCacheKey(value);
      if (!key) continue;
      putStaticAssetTier(tiers, key, tier);
    }
  }

  return tiers;
}

function normalizeStaticAssetCacheKey(value) {
  if (typeof value !== "string" || !value) return "";
  try {
    const url = new URL(value, self.location.origin);
    if (url.origin !== self.location.origin || !isStaticGameAsset(url)) return "";
    return staticAssetCacheKey(url);
  } catch {
    return "";
  }
}

function addStaticAssetTierHints(urls, cacheTier) {
  if (!Array.isArray(urls)) return { added: 0, ignored: 0, staticAssetTiers: summarizeStaticAssetTiers() };
  const tier = normalizeStaticAssetTier(cacheTier);
  let added = 0;
  let ignored = 0;

  for (const value of urls) {
    const key = normalizeStaticAssetCacheKey(value);
    if (!key) {
      ignored += 1;
      continue;
    }
    const changed = putStaticAssetTier(staticAssetTiers, key, tier);
    if (changed) added += 1;
    else ignored += 1;
  }

  return { added, ignored, staticAssetTiers: summarizeStaticAssetTiers() };
}

function putStaticAssetTier(tiers, key, tier) {
  const existing = tiers.get(key);
  if (existing && tierPriority(existing) <= tierPriority(tier)) return false;
  tiers.set(key, tier);
  return true;
}

function staticAssetCacheKey(url) {
  return `${url.pathname}${url.search}`;
}

function normalizeStaticAssetTier(value) {
  return value === "background" ? "background" : "critical";
}

function tierPriority(tier) {
  return tier === "critical" ? 0 : 1;
}

function summarizeStaticAssetTiers() {
  const summary = { critical: 0, background: 0, runtime: 0 };
  for (const tier of staticAssetTiers.values()) {
    if (tier === "background") summary.background += 1;
    else summary.critical += 1;
  }
  return summary;
}

function positiveNumber(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function normalizeAssetBaseUrl(value) {
  if (typeof value !== "string") return "";
  return value.trim().replace(/\/+$/, "");
}

function postClientMessage(event, type, payload) {
  if (!event.source || typeof event.source.postMessage !== "function") return;
  event.source.postMessage({
    type,
    ...(payload || {}),
  });
}
