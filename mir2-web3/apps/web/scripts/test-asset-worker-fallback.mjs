import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const workerSource = readFileSync(
  new URL("../public/mir2-asset-worker.js", import.meta.url),
  "utf8",
);

function createWorkerContext(fetchImpl, cacheStorage) {
  const listeners = new Map();
  const context = vm.createContext({
    AbortController,
    AbortSignal,
    caches: cacheStorage ?? {
      async delete() {
        return true;
      },
      async keys() {
        return [];
      },
      async open() {
        return {
          async keys() {
            return [];
          },
        };
      },
    },
    clearTimeout,
    console,
    fetch: fetchImpl,
    Headers,
    Map,
    Request,
    Response,
    Set,
    setTimeout,
    URL,
    self: {
      clients: { async claim() {} },
      location: { origin: "https://preview.example.test" },
      async skipWaiting() {},
      addEventListener(type, listener) {
        listeners.set(type, listener);
      },
    },
  });
  vm.runInContext(workerSource, context, { filename: "mir2-asset-worker.js" });
  return { context, listeners };
}

test("browser-safe R2 fallback resolves before a referer-sensitive CDN", async () => {
  const calls = [];
  const { context } = createWorkerContext(async (request) => {
    const url = typeof request === "string" ? request : request.url;
    calls.push(url);
    if (url.startsWith("https://preview.example.test/")) {
      return new Response(null, { status: 404 });
    }
    if (url.startsWith("https://public-r2.example.test/")) {
      return new Response(new Uint8Array([1, 2, 3]), {
        status: 200,
        headers: { "content-type": "image/png" },
      });
    }
    throw new Error(`referer-sensitive CDN must not run after fallback success: ${url}`);
  });

  vm.runInContext(
    `runtimeConfig = {
      ...runtimeConfig,
      version: "release-1",
      remoteAssetBaseUrl: "https://assets.example.test/release-1",
      remoteAssetBaseUrls: [
        "https://public-r2.example.test/release-1",
        "https://assets.example.test/release-1"
      ]
    }`,
    context,
  );

  const response = await vm.runInContext(
    `fetchStaticAsset(new Request(
      "https://preview.example.test/generated/map-atlas/WemadeMir2-Tiles/p0.png"
    ))`,
    context,
  );

  assert.equal(response.status, 200);
  assert.equal((await response.arrayBuffer()).byteLength, 3);
  assert.deepEqual(calls, [
    "https://preview.example.test/generated/map-atlas/WemadeMir2-Tiles/p0.png",
    "https://public-r2.example.test/release-1/generated/map-atlas/WemadeMir2-Tiles/p0.png",
  ]);
});

test("remote candidates are deduplicated and preserve fallback-first order", () => {
  const { context } = createWorkerContext(async () => new Response(null, { status: 404 }));
  vm.runInContext(
    `runtimeConfig = {
      ...runtimeConfig,
      remoteAssetBaseUrl: "https://assets.example.test/release-1",
      remoteAssetBaseUrls: normalizeAssetBaseUrls([
        "https://public-r2.example.test/release-1/",
        "https://public-r2.example.test/release-1",
        "https://assets.example.test/release-1"
      ])
    }`,
    context,
  );

  const urls = vm.runInContext(
    `createRemoteAssetRequests(new Request(
      "https://preview.example.test/original-ui/Title/30.png?retry=1"
    )).map((request) => request.url)`,
    context,
  );
  assert.deepEqual(Array.from(urls), [
    "https://public-r2.example.test/release-1/original-ui/Title/30.png?retry=1",
    "https://assets.example.test/release-1/original-ui/Title/30.png?retry=1",
  ]);
});

test("legacy image retry query parameters collapse to one stable cache key", () => {
  const { context } = createWorkerContext(async () => new Response(null, { status: 404 }));
  const stableUrls = vm.runInContext(
    `[
      stableStaticAssetRequest(new Request(
        "https://preview.example.test/original-ui/Prguse/2090.png?mir2ImgRetry=3&mir2ImgRetryTs=one"
      )).url,
      stableStaticAssetRequest(new Request(
        "https://preview.example.test/original-ui/Prguse/2090.png?mir2ImgRetry=13&mir2ImgRetryTs=two"
      )).url
    ]`,
    context,
  );

  assert.deepEqual(Array.from(stableUrls), [
    "https://preview.example.test/original-ui/Prguse/2090.png",
    "https://preview.example.test/original-ui/Prguse/2090.png",
  ]);
});

test("a cold asset streams before its background CacheStorage write completes", async () => {
  let releaseCacheWrite;
  const blockedCacheWrite = new Promise((resolve) => {
    releaseCacheWrite = resolve;
  });
  const waitUntilPromises = [];
  const cache = {
    async match() {
      return undefined;
    },
    async put() {
      await blockedCacheWrite;
    },
    async keys() {
      return [];
    },
  };
  const { context } = createWorkerContext(
    async () =>
      new Response(new Uint8Array([4, 5, 6]), {
        status: 200,
        headers: { "content-type": "image/png" },
      }),
    {
      async delete() {
        return true;
      },
      async keys() {
        return [];
      },
      async open() {
        return cache;
      },
    },
  );
  context.captureWaitUntil = (promise) => waitUntilPromises.push(promise);

  const response = await Promise.race([
    vm.runInContext(
      `cacheFirst(
        new Request("https://preview.example.test/original-ui/Title/30.png"),
        "cache-test",
        10,
        { waitUntil: captureWaitUntil }
      )`,
      context,
    ),
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error("cacheFirst waited for CacheStorage")), 100),
    ),
  ]);

  assert.equal(response.status, 200);
  assert.equal((await response.arrayBuffer()).byteLength, 3);
  assert.equal(waitUntilPromises.length, 1);
  releaseCacheWrite();
  await Promise.all(waitUntilPromises);
});

test("a warm asset cache hit does not rewrite or scan CacheStorage", async () => {
  let putCount = 0;
  let keyScanCount = 0;
  const waitUntilPromises = [];
  const cache = {
    async match() {
      return new Response(new Uint8Array([7, 8, 9]), {
        status: 200,
        headers: { "content-type": "image/png" },
      });
    },
    async put() {
      putCount += 1;
    },
    async keys() {
      keyScanCount += 1;
      return [];
    },
  };
  const { context } = createWorkerContext(
    async () => {
      throw new Error("network must not run on a cache hit");
    },
    {
      async delete() {
        return true;
      },
      async keys() {
        return [];
      },
      async open() {
        return cache;
      },
    },
  );
  context.captureWaitUntil = (promise) => waitUntilPromises.push(promise);

  const response = await vm.runInContext(
    `cacheFirst(
      new Request("https://preview.example.test/original-map/Tiles/1.png"),
      "warm-cache-test",
      100,
      { waitUntil: captureWaitUntil }
    )`,
    context,
  );

  assert.equal(response.status, 200);
  assert.equal((await response.arrayBuffer()).byteLength, 3);
  assert.equal(putCount, 0);
  assert.equal(keyScanCount, 0);
  assert.equal(waitUntilPromises.length, 0);
});

test("cache trimming is amortized across a burst of writes", async () => {
  let putCount = 0;
  let keyScanCount = 0;
  const cache = {
    async put() {
      putCount += 1;
    },
    async keys() {
      keyScanCount += 1;
      return [];
    },
  };
  const { context } = createWorkerContext(async () => new Response(null, { status: 200 }));
  context.testCache = cache;

  for (let index = 0; index < 63; index += 1) {
    context.testIndex = index;
    await vm.runInContext(
      `putCacheEntry(
        testCache,
        new Request("https://preview.example.test/original-ui/Test/" + testIndex + ".png"),
        new Response(new Uint8Array([1]), { status: 200 }),
        1000,
        "burst-cache-test"
      )`,
      context,
    );
  }
  assert.equal(putCount, 63);
  assert.equal(keyScanCount, 0);

  context.testIndex = 63;
  await vm.runInContext(
    `putCacheEntry(
      testCache,
      new Request("https://preview.example.test/original-ui/Test/" + testIndex + ".png"),
      new Response(new Uint8Array([1]), { status: 200 }),
      1000,
      "burst-cache-test"
    )`,
    context,
  );
  assert.equal(putCount, 64);
  assert.equal(keyScanCount, 1);
});

function dispatchSceneMetadataRequest(path, fetchImpl, store) {
  const { context, listeners } = createWorkerContext(fetchImpl, { async open() { return store; } });
  vm.runInContext('runtimeConfig.version = "first-scene-test"', context);
  const completions = [];
  let response;
  let dispatching = true;
  listeners.get("fetch")({
    request: new Request(`https://preview.example.test${path}`),
    waitUntil(promise) {
      assert.equal(dispatching, true, "waitUntil must be registered during the FetchEvent");
      completions.push(promise);
    },
    respondWith(promise) {
      assert.equal(dispatching, true);
      response = promise;
    },
  });
  dispatching = false;
  assert(response);
  return { response, completions };
}

async function whileCacheIsBlocked(promise) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error("CacheStorage delayed the foreground response")), 500);
    })]);
  } finally {
    clearTimeout(timer);
  }
}

for (const path of ["/api/asset-manifest", "/api/scene/crystal?map=0&x=296&y=620&assetRelease=test"]) {
  test(`${path} returns headers and JSON while the independent cache write remains blocked`, async () => {
    let body;
    const stream = new ReadableStream({ start(controller) { body = controller; } });
    let releaseWrite;
    const writeBlocked = new Promise((resolve) => { releaseWrite = resolve; });
    let cachedBody;
    const request = dispatchSceneMetadataRequest(path,
      async () => new Response(stream, { headers: { "Content-Type": "application/json", "X-Test-Origin": "true" } }),
      {
        async match() { return undefined; },
        async put(_, cached) {
          cachedBody = await cached.json();
          await writeBlocked;
        },
      });
    assert.equal(request.completions.length, 1);
    try {
      const response = await whileCacheIsBlocked(request.response);
      assert.equal(response.headers.get("x-test-origin"), "true");
      const json = response.json();
      body.enqueue(new TextEncoder().encode('{"tile":296}'));
      body.close();
      assert.deepEqual(await whileCacheIsBlocked(json), { tile: 296 });
    } finally {
      releaseWrite();
    }
    await Promise.all(request.completions);
    assert.deepEqual(cachedBody, { tile: 296 });
  });
}

test("a warm versioned Crystal request returns cached data and refreshes it in the background", async () => {
  let finishRefresh;
  const network = new Promise((resolve) => { finishRefresh = resolve; });
  let refreshed;
  const request = dispatchSceneMetadataRequest("/api/scene/crystal?map=0&assetRelease=test",
    () => network, {
      async match() { return Response.json({ region: "cached" }); },
      async put(_, response) { refreshed = await response.json(); },
    });
  try {
    assert.deepEqual(await (await whileCacheIsBlocked(request.response)).json(), { region: "cached" });
    assert.equal(refreshed, undefined);
  } finally {
    finishRefresh(Response.json({ region: "network" }));
  }
  await Promise.all(request.completions);
  assert.deepEqual(refreshed, { region: "network" });
});

test("a Crystal reload leaves the retry to the browser without joining the worker cache path", () => {
  const { context, listeners } = createWorkerContext(
    () => assert.fail("the worker must not start a network request"),
    { open: () => assert.fail("the worker must not open its cache") },
  );
  vm.runInContext('runtimeConfig.version = "first-scene-test"', context);
  listeners.get("fetch")({
    request: new Request("https://preview.example.test/api/scene/crystal?map=0&assetRelease=test", { cache: "reload" }),
    waitUntil: () => assert.fail("the worker must not schedule a cache write"),
    respondWith: () => assert.fail("the browser must own the reload response"),
  });
});

test("asset-manifest stays network-first and uses the same key for offline fallback", async () => {
  let lookups = 0;
  const store = {
    async match(request) {
      assert.equal(new URL(request.url).pathname, "/api/asset-manifest");
      lookups += 1;
      return Response.json({ version: "cached" });
    },
    async put() {},
  };
  const fresh = dispatchSceneMetadataRequest("/api/asset-manifest",
    async () => Response.json({ version: "fresh" }), store);
  assert.deepEqual(await (await fresh.response).json(), { version: "fresh" });
  await Promise.all(fresh.completions);
  assert.equal(lookups, 0);
  const denied = dispatchSceneMetadataRequest("/api/asset-manifest",
    async () => new Response(null, { status: 403 }), store);
  assert.equal((await denied.response).status, 403);
  await Promise.all(denied.completions);
  assert.equal(lookups, 0);
  const offline = dispatchSceneMetadataRequest("/api/asset-manifest",
    async () => { throw new TypeError("offline"); }, store);
  assert.deepEqual(await (await offline.response).json(), { version: "cached" });
  await Promise.all(offline.completions);
  assert.equal(lookups, 1);
});

test("Crystal preserves Retry-After and the typed resource-missing 424 without cache writes", async () => {
  let fetchCount = 0;
  let cacheWrites = 0;
  const store = {
    async match() { return undefined; },
    async put() { cacheWrites += 1; },
  };
  for (const status of [424, 503]) {
    const request = dispatchSceneMetadataRequest("/api/scene/crystal?map=0&assetRelease=test", async () => {
      fetchCount += 1;
      return Response.json({ status }, { status, headers: status === 503 ? { "Retry-After": "15" } : {} });
    }, store);
    const response = await request.response;
    assert.equal(response.status, status);
    assert.deepEqual(await response.json(), { status });
    await Promise.all(request.completions);
  }
  assert.equal(fetchCount, 2);
  assert.equal(cacheWrites, 0);
});
