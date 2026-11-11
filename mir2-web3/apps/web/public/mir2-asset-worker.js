const CACHE_PREFIX = "mir2-asset-cache";
const CACHE_SCHEMA_VERSION = "sw8";
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
  remoteAssetBaseUrls: [],
  fontMirrorFirst: false,
};
let staticAssetTiers = new Map();

// Short-lived negative cache for confirmed missing assets. Do not cache
// transient network/abort failures: gameplay prefetch can fan out hundreds of
// requests, and one momentary 504 must not blacklist a valid animation frame.
const NEGATIVE_CACHE_TTL_MS = 10_000;
const NEGATIVE_CACHE_MAX_ENTRIES = 2000;
const staticAssetNegativeCache = new Map();
const staticAssetInflightFetches = new Map();
// CSS and the shared renderer can request the same large font while its first
// body is still downloading. Retain that response until the cache fill ends,
// rather than coalescing response headers only. Bounds limit retained tees.
const fontAssetInflightFetches = new Map();
const FONT_INFLIGHT_MAX_ENTRIES = 16;
const FONT_INFLIGHT_MAX_AGE_MS = 180_000;
const FONT_MIRROR_HEADER_TIMEOUT_MS = 8_000;
const FONT_MIRROR_BODY_IDLE_TIMEOUT_MS = 15_000;
const FONT_MIRROR_MAX_BYTES = 32 * 1024 * 1024;
const sceneBlueprintInflightFetches = new Map();
// CacheStorage operations are serialized by browsers much more aggressively
// than ordinary HTTP/2 requests. Rewriting every cache hit and calling
// cache.keys() after every miss turns a few hundred scene images into a
// minutes-long storage queue. Hits are already immutable/versioned, so they do
// not need a recency write. Misses amortize the O(n) trim scan instead.
const CACHE_TRIM_WRITE_INTERVAL = 64;
const cacheWritesSinceTrim = new Map();
const cacheTrimInflight = new Map();
const TRANSIENT_FETCH_RETRY_DELAYS_MS = [120, 350];
const TRANSIENT_FETCH_STATUSES = new Set([408, 425, 429, 500, 502, 503, 504]);
// A new page attempt must not inherit a coalesced network request whose
// headers have stopped arriving. The page owns the JSON body idle deadline.
const SCENE_HEADERS_TIMEOUT_MS = 7_000;
// Chrome can retain the worker's cache tee after the page cancels its own
// branch. Keep the upstream hard limit aligned with the page's 24s body limit
// so a progressing foreground response can finish through the worker.
const SCENE_BLUEPRINT_RESPONSE_TIMEOUT_MS = SCENE_HEADERS_TIMEOUT_MS + 24_000;
const SCENE_METADATA_RESPONSE_TIMEOUT_MS = 10_000;

self.addEventListener("install", (event) => {
  event.waitUntil(self.skipWaiting());
});

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener("message", (event) => {
  const data = event.data || {};
  if (data.type === "MIR2_ASSET_WORKER_ACTIVATE") {
    event.waitUntil(self.skipWaiting());
    return;
  }

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
  const remoteAssetBaseUrl = normalizeAssetBaseUrl(
    data.assetBaseUrl || remoteAssets.assetBaseUrl || manifest.assetBaseUrl || "",
  );
  const remoteAssetBaseUrls = normalizeAssetBaseUrls([
    ...(Array.isArray(data.browserFallbackBaseUrls) ? data.browserFallbackBaseUrls : []),
    ...(Array.isArray(remoteAssets.browserFallbackBaseUrls)
      ? remoteAssets.browserFallbackBaseUrls
      : []),
    remoteAssetBaseUrl,
  ]);
  runtimeConfig = {
    version: String(data.cacheNamespace || manifest.cacheNamespace || assetVersion || DEFAULT_VERSION),
    assetVersion,
    staticAssetMaxEntries,
    staticCriticalMaxEntries: positiveNumber(caches.staticCriticalMaxEntries, Math.min(staticAssetMaxEntries, 3000)),
    staticBackgroundMaxEntries: positiveNumber(caches.staticBackgroundMaxEntries, Math.min(staticAssetMaxEntries, 6000)),
    staticRuntimeMaxEntries: positiveNumber(caches.staticRuntimeMaxEntries, staticAssetMaxEntries),
    sceneBlueprintMaxEntries: positiveNumber(caches.sceneBlueprintMaxEntries, 512),
    apiMetadataMaxEntries: positiveNumber(caches.apiMetadataMaxEntries, 512),
    remoteAssetBaseUrl,
    remoteAssetBaseUrls,
    fontMirrorFirst: data.fontMirrorFirst === true,
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
        remoteAssetBaseUrls: runtimeConfig.remoteAssetBaseUrls,
        fontMirrorFirst: runtimeConfig.fontMirrorFirst,
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
    const stableRequest = stableStaticAssetRequest(request);
    const policy = staticAssetCachePolicy(stableRequest);
    event.respondWith(
      cacheFirst(stableRequest, cacheName(policy.cacheKind), policy.maxEntries, event),
    );
    return;
  }

  if (url.pathname === "/api/scene/crystal") {
    // The caller's recovery attempt needs its own network read of this URL.
    if (request.cache === "reload") return;
    respondWithSceneMetadata(
      event,
      cacheName("scene"),
      runtimeConfig.sceneBlueprintMaxEntries,
      coalescedSceneBlueprintFetch,
      true,
    );
    return;
  }

  if (url.pathname === "/api/asset-manifest") {
    respondWithSceneMetadata(event, cacheName("api"), runtimeConfig.apiMetadataMaxEntries, fetch, false);
    return;
  }

  if (url.pathname === "/api/original-ui-meta") {
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
    url.pathname.startsWith("/generated/map-atlas/") ||
    url.pathname.startsWith("/generated/crystal-packs/full/") ||
    url.pathname.startsWith("/bevy-entity-atlases/")
  );
}

function stableStaticAssetRequest(request) {
  const url = new URL(request.url);
  const hadLegacyRetryQuery =
    url.searchParams.has("mir2ImgRetry") || url.searchParams.has("mir2ImgRetryTs");
  if (!hadLegacyRetryQuery) return request;

  url.searchParams.delete("mir2ImgRetry");
  url.searchParams.delete("mir2ImgRetryTs");
  return new Request(url.href, {
    method: request.method,
    headers: request.headers,
    mode: request.mode,
    credentials: request.credentials,
    cache: request.cache,
    redirect: request.redirect,
    referrer: request.referrer,
    referrerPolicy: request.referrerPolicy,
    integrity: request.integrity,
    keepalive: request.keepalive,
  });
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
      return cached;
    }
  } catch {
    // CacheStorage can be unavailable (private mode, quota teardown). Fall
    // through to a direct fetch and never let it become an uncaught rejection.
    cache = null;
  }

  if (cache && isSharedUiFont(request) && !request.headers.has("range")) {
    const entry = sharedFontCacheFill(request, cache, name, maxEntries);
    if (entry) {
      event?.waitUntil(entry.cached.catch(() => null));
      return (await entry.response).clone();
    }
  }

  const response = await coalescedStaticAssetFetch(request);
  if (cache && response && response.ok) {
    // Never make first paint wait for CacheStorage to consume the complete
    // response body. Atlas pages can be several MiB: returning the network
    // stream immediately while event.waitUntil owns a cloned cache write keeps
    // cold-start latency equal to download latency instead of download + disk.
    const cacheWrite = putCacheEntry(cache, request, response.clone(), maxEntries, name);
    event?.waitUntil(cacheWrite.catch(() => null));
  }
  return response;
}

function isSharedUiFont(request) {
  return /^\/original-ui\/fonts\/[^/]+\.(?:ttf|otf|woff2?)$/i.test(new URL(request.url).pathname);
}

function sharedFontCacheFill(request, cache, name, maxEntries) {
  const key = `${name}:${request.url}`;
  const existing = fontAssetInflightFetches.get(key);
  if (existing) return existing;
  if (fontAssetInflightFetches.size >= FONT_INFLIGHT_MAX_ENTRIES) return null;
  // One document leaving must not cancel another document's immutable font.
  const sharedRequest = new Request(request, { signal: new AbortController().signal });
  const fallbacks = createRemoteAssetRequests(sharedRequest);
  const entry = {
    response: fetchStaticAsset(freshStaticAssetRequest(sharedRequest), fallbacks)
      .catch(() => syntheticAssetResponse(504, "font fetch failed")),
    cached: null,
  };
  fontAssetInflightFetches.set(key, entry);
  const retire = () => {
    if (fontAssetInflightFetches.get(key) === entry) fontAssetInflightFetches.delete(key);
  };
  // Expiry releases only the shared lookup; it never truncates a progressing
  // foreground body or a cache write owned by waitUntil.
  const expiry = setTimeout(retire, FONT_INFLIGHT_MAX_AGE_MS);
  entry.cached = entry.response.then((response) =>
    response.ok && putCacheEntry(cache, request, response.clone(), maxEntries, name),
  ).finally(() => { clearTimeout(expiry); retire(); });
  return entry;
}

async function fetchStaticAsset(request, fallbackRequests) {
  // Never throws. Resolves to the best available response and, on total
  // failure, to a synthetic 504 so callers (cacheFirst -> respondWith) always
  // receive a settled promise instead of an uncaught network rejection.
  const negativeKey = request.url;
  const negativeStatus = getNegativeCacheStatus(negativeKey);
  if (negativeStatus) {
    return syntheticAssetResponse(negativeStatus, "asset recently unavailable");
  }

  const remoteRequests = fallbackRequests || createRemoteAssetRequests(request);
  const fontMirror = currentReleaseFontMirror(request, remoteRequests);
  if (fontMirror) {
    const response = await fetchFontMirror(fontMirror, request.signal);
    if (response?.status === 200) return response;
    void response?.body?.cancel().catch(() => {});
  }

  let originResponse = null;
  originResponse = await fetchWithTransientRetries(freshStaticAssetRequest(request));
  if (originResponse?.ok) return originResponse;

  let lastRemoteResponse = null;
  for (const remoteRequest of remoteRequests) {
    const remoteResponse = await fetchWithTransientRetries(freshStaticAssetRequest(remoteRequest));
    if (!remoteResponse) continue;
    lastRemoteResponse = remoteResponse;
    if (remoteResponse.ok) return remoteResponse;
  }

  if (lastRemoteResponse && (lastRemoteResponse.status === 404 || !originResponse)) {
    if (lastRemoteResponse.status === 404 || lastRemoteResponse.status === 410) {
      rememberNegativeResult(negativeKey, lastRemoteResponse.status);
    }
    return lastRemoteResponse;
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

function currentReleaseFontMirror(request, remoteRequests) {
  const version = runtimeConfig.assetVersion;
  if (!runtimeConfig.fontMirrorFirst || !isSharedUiFont(request) || request.headers.has("range") || version === DEFAULT_VERSION ||
      typeof version !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(version)) return null;
  const source = new URL(request.url);
  const suffix = `/mir2/v/${version}${source.pathname}`;
  // Only the configured current release may precede the app. Inherited releases
  // remain fallbacks so an older font cannot shadow a valid current local file.
  return remoteRequests.find(remote => {
    const url = new URL(remote.url);
    return url.origin !== source.origin && url.pathname.endsWith(suffix);
  }) || null;
}

async function fetchFontMirror(request, owner) {
  const deadline = new AbortController();
  const headerTimer = setTimeout(() => deadline.abort(), FONT_MIRROR_HEADER_TIMEOUT_MS);
  let bodyTimer, reader;
  try {
    const response = await fetch(freshStaticAssetRequest(new Request(request, {
      signal: AbortSignal.any([owner, request.signal, deadline.signal]),
    })));
    clearTimeout(headerTimer);
    if (response.status !== 200) return response;
    if (!response.body) return null;
    reader = response.body.getReader();
    let rejectIdle;
    const idle = new Promise((_, reject) => { rejectIdle = reject; });
    const expectProgress = () => {
      clearTimeout(bodyTimer);
      bodyTimer = setTimeout(() => {
        deadline.abort();
        rejectIdle(new Error("font body stopped progressing"));
      }, FONT_MIRROR_BODY_IDLE_TIMEOUT_MS);
    };
    expectProgress();
    const chunks = [];
    let length = 0;
    for (;;) {
      const { value, done } = await Promise.race([reader.read(), idle]);
      if (done) break;
      if (!value.byteLength) continue;
      length += value.byteLength;
      if (length > FONT_MIRROR_MAX_BYTES) throw new Error("font mirror body too large");
      chunks.push(value);
      expectProgress();
    }
    if (!length) return null;
    // Fonts become usable only after their full body arrives. Do not expose a
    // partial prefix: a stalled mirror must be replaceable by the complete app
    // response, without mixing two sources or poisoning the shared font cache.
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
    const headers = new Headers(response.headers);
    // fetch has decoded a compressed body; its wire length/encoding no longer
    // describe the reconstructed response delivered to CSS and the runtime.
    headers.delete("content-encoding");
    headers.set("content-length", String(length));
    return new Response(bytes, { status: response.status, statusText: response.statusText, headers });
  } catch {
    deadline.abort();
    void reader?.cancel().catch(() => {});
    return null;
  } finally {
    // Neither budget is a total body deadline: each nonempty chunk renews the
    // idle budget, so a large progressing original font can finish normally.
    clearTimeout(headerTimer);
    clearTimeout(bodyTimer);
    reader?.releaseLock();
  }
}

function freshStaticAssetRequest(request) {
  const source = request instanceof Request ? request : new Request(request);
  // A versioned CacheStorage must never be repopulated from stale HTTP-cache
  // bytes. Atlas manifests and their PNG pages have to advance atomically.
  return new Request(source, { cache: "reload" });
}

async function coalescedStaticAssetFetch(request) {
  // DOM images and prewarming may share the same immutable response. Retiring
  // the first image during the Native HUD handoff must not abort the other
  // consumer or its CacheStorage body write.
  const sharedRequest = new Request(request, { signal: new AbortController().signal });
  return coalescedResponse(staticAssetInflightFetches, request.url, () => fetchStaticAsset(sharedRequest));
}

async function coalescedSceneBlueprintFetch(request) {
  return coalescedResponse(sceneBlueprintInflightFetches, request.url, async () => {
    const deadline = new AbortController();
    const signal = AbortSignal.any([request.signal, deadline.signal]);
    const timer = setTimeout(() => deadline.abort(), SCENE_HEADERS_TIMEOUT_MS);
    try {
      return await fetchWithTransientRetries(new Request(request, { signal }), signal);
    } finally {
      clearTimeout(timer);
    }
  });
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

async function fetchWithTransientRetries(request, sceneSignal) {
  let lastResponse = null;
  for (let attempt = 0; attempt <= TRANSIENT_FETCH_RETRY_DELAYS_MS.length; attempt += 1) {
    try {
      const response = await fetch(cloneRequestForRetry(request));
      if (!isTransientFetchStatus(response.status) || (sceneSignal && response.headers.has("retry-after"))) {
        return response;
      }
      lastResponse = response;
    } catch {
      lastResponse = null;
    }

    if (sceneSignal?.aborted) return null;
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

function createRemoteAssetRequests(request) {
  if (!runtimeConfig.remoteAssetBaseUrls.length) return [];

  const localUrl = new URL(request.url);
  if (!isRemoteBackedStaticGameAsset(localUrl)) return [];

  const relativePath = localUrl.pathname.replace(/^\/+/, "");
  return runtimeConfig.remoteAssetBaseUrls.flatMap((baseUrl) => {
    const remoteUrl = new URL(relativePath, `${baseUrl}/`);
    remoteUrl.search = localUrl.search;

    if (remoteUrl.origin === localUrl.origin && remoteUrl.pathname === localUrl.pathname) {
      return [];
    }

    return [
      new Request(remoteUrl.href, {
        method: "GET",
        mode: "cors",
        credentials: "omit",
        redirect: "follow",
      }),
    ];
  });
}

function isRemoteBackedStaticGameAsset(url) {
  return (
    url.pathname.startsWith("/original-ui/") ||
    url.pathname.startsWith("/original-map/") ||
    url.pathname.startsWith("/generated/original-map-blend/") ||
    url.pathname.startsWith("/generated/map-atlas/") ||
    url.pathname.startsWith("/generated/crystal-packs/full/") ||
    url.pathname.startsWith("/bevy-entity-atlases/")
  );
}

function respondWithSceneMetadata(event, name, maxEntries, fetchNetwork, preferCached) {
  const request = event.request;
  const cache = caches.open(name).catch(() => null);
  const readCache = () => cache.then((store) => store?.match(request)).catch(() => null);
  const responseTimeout = preferCached ? SCENE_BLUEPRINT_RESPONSE_TIMEOUT_MS : SCENE_METADATA_RESPONSE_TIMEOUT_MS;
  const networkRequest = new Request(request, {
    signal: AbortSignal.any([request.signal, AbortSignal.timeout(responseTimeout)]),
  });
  const network = fetchNetwork(networkRequest);

  // Register synchronously while the FetchEvent is active, and tee the body
  // before the foreground response can be consumed by the browser.
  event.waitUntil(network.then((response) => {
    if (!response?.ok) return;
    const copy = response.clone();
    return cache.then((store) =>
      store && putCacheEntry(store, request, copy, maxEntries, name, networkRequest.signal));
  }).catch(() => null));

  if (!preferCached) {
    event.respondWith(network.catch(async (error) => {
      const cached = await readCache();
      if (cached) return cached;
      throw error;
    }));
    return;
  }

  const refresh = network.catch(() => null);
  event.respondWith(readCache().then(async (cached) => cached || await refresh || unavailableSceneBlueprint()));
}

function unavailableSceneBlueprint() {

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
    await putCacheEntry(cache, request, response, maxEntries, name);
    return response;
  } catch (error) {
    const cached = await cache.match(request);
    if (cached) return cached;
    throw error;
  }
}

async function putCacheEntry(cache, request, response, maxEntries, cacheIdentity = "", responseSignal) {
  if (!response || !response.ok || responseSignal?.aborted) return;
  try {
    await cache.put(request, response.clone());
    await maybeTrimCache(cache, maxEntries, cacheIdentity);
  } catch (error) {
    if (responseSignal && (responseSignal.aborted || error?.name !== "QuotaExceededError")) return;
    await trimCache(cache, Math.max(32, Math.floor(maxEntries / 2)));
    try {
      await cache.put(request, response.clone());
    } catch {
      // Quota pressure should never break gameplay.
    }
  }
}

async function maybeTrimCache(cache, maxEntries, cacheIdentity) {
  const identity = cacheIdentity || `anonymous:${maxEntries}`;
  const writes = (cacheWritesSinceTrim.get(identity) || 0) + 1;
  if (writes < CACHE_TRIM_WRITE_INTERVAL) {
    cacheWritesSinceTrim.set(identity, writes);
    return;
  }
  cacheWritesSinceTrim.set(identity, 0);

  const existing = cacheTrimInflight.get(identity);
  if (existing) return existing;

  const pending = trimCache(cache, maxEntries).finally(() => {
    if (cacheTrimInflight.get(identity) === pending) {
      cacheTrimInflight.delete(identity);
    }
  });
  cacheTrimInflight.set(identity, pending);
  return pending;
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
    remoteAssetBaseUrls: runtimeConfig.remoteAssetBaseUrls,
    fontMirrorFirst: runtimeConfig.fontMirrorFirst,
    staticAssetTiers: summarizeStaticAssetTiers(),
    cacheCount: entries.length,
    entryCount: entries.reduce((sum, entry) => sum + entry.entries, 0),
    caches: entries,
  };
}

function staticAssetCachePolicy(request) {
  const url = new URL(request.url);
  // Changing from CSS to renderer ownership must not change the cache holding
  // an unfinished font and start a second complete download.
  const tier = isSharedUiFont(request) ? "critical" : staticAssetTiers.get(staticAssetCacheKey(url)) || "runtime";
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

function normalizeAssetBaseUrls(values) {
  const normalized = [];
  for (const value of values) {
    const baseUrl = normalizeAssetBaseUrl(value);
    if (baseUrl && !normalized.includes(baseUrl)) normalized.push(baseUrl);
  }
  return normalized;
}

function postClientMessage(event, type, payload) {
  if (!event.source || typeof event.source.postMessage !== "function") return;
  event.source.postMessage({
    type,
    ...(payload || {}),
  });
}
