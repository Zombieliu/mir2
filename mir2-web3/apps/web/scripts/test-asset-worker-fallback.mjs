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

function fontWorkerFixture(t) {
  const calls = [], streams = [], completions = [], stores = new Map();
  let writes = 0;
  const cacheStorage = { async open(name) {
    if (!stores.has(name)) stores.set(name, new Map());
    const entries = stores.get(name);
    return {
      async match(request) { return entries.get(request.url)?.clone(); },
      async put(request, response) {
        writes += 1;
        const bytes = await response.arrayBuffer();
        entries.set(request.url, new Response(bytes, { headers: response.headers }));
      },
      async keys() { return [...entries.keys()].map(url => new Request(url)); },
      async delete(request) { return entries.delete(request.url); },
    };
  } };
  const { context } = createWorkerContext(async request => {
    calls.push(request);
    let body;
    const stream = new ReadableStream({ start(controller) { body = controller; } });
    request.signal.addEventListener("abort", () => { try { body.error(request.signal.reason); } catch {} }, { once: true });
    streams.push(body);
    return new Response(stream, { headers: { "content-type": "font/ttf" } });
  }, cacheStorage);
  t.after(() => { for (const stream of streams) { try { stream.close(); } catch {} } });
  context.captureFontWrite = value => completions.push(value);
  const read = (name = "font-cache-a", request) => {
    context.fontRequest = request ?? new Request("https://preview.example.test/original-ui/fonts/NotoSansTC.ttf");
    context.fontCacheName = name;
    return vm.runInContext('cacheFirst(fontRequest, fontCacheName, 20, { waitUntil: captureFontWrite })', context);
  };
  return { context, read, calls, streams, completions, stores, writes: () => writes };
}

test("CSS and renderer consumers share one font through its unfinished body and cache fill", async t => {
  const f = fontWorkerFixture(t);
  const first = await f.read();
  const firstBody = first.arrayBuffer();
  f.streams[0].enqueue(new Uint8Array([1, 2]));
  const second = await f.read();
  const secondBody = second.arrayBuffer();
  assert.equal(f.calls.length, 1, "the second consumer must not download the same font again");
  assert.equal(f.writes(), 1, "one in-flight body has only one cache writer");
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 1);
  f.streams[0].enqueue(new Uint8Array([3, 4])); f.streams[0].close();
  assert.deepEqual([...new Uint8Array(await firstBody)], [1,2,3,4]);
  assert.deepEqual([...new Uint8Array(await secondBody)], [1,2,3,4]);
  await Promise.all(f.completions);
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 0);
  const cached = await f.read();
  assert.deepEqual([...new Uint8Array(await cached.arrayBuffer())], [1,2,3,4]);
  assert.equal(f.calls.length, 1);
});

test("one font consumer retiring does not cancel another consumer's shared download", async t => {
  const f = fontWorkerFixture(t), owner = new AbortController();
  const first = await f.read(undefined, new Request("https://preview.example.test/original-ui/fonts/NotoSansTC.ttf", { signal: owner.signal }));
  owner.abort("first document left");
  assert.equal(f.calls[0].signal.aborted, false);
  const second = await f.read();
  assert.equal(f.calls.length, 1);
  f.streams[0].enqueue(new Uint8Array([8,9])); f.streams[0].close();
  assert.deepEqual([...new Uint8Array(await second.arrayBuffer())], [8,9]);
  assert.deepEqual([...new Uint8Array(await first.arrayBuffer())], [8,9]);
  await Promise.all(f.completions);
});

test("font fills retain separate cache namespaces across a release change", async t => {
  const f = fontWorkerFixture(t);
  const old = await f.read("version-a"), next = await f.read("version-b");
  assert.equal(f.calls.length, 2);
  f.streams[0].enqueue(new Uint8Array([1])); f.streams[0].close();
  f.streams[1].enqueue(new Uint8Array([2])); f.streams[1].close();
  assert.deepEqual([...new Uint8Array(await old.arrayBuffer())], [1]);
  assert.deepEqual([...new Uint8Array(await next.arrayBuffer())], [2]);
  await Promise.all(f.completions);
});

test("font lookup expiry releases the retained entry without truncating either body", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = fontWorkerFixture(t), first = await f.read();
  t.mock.timers.tick(180_000);
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 0);
  assert.equal(f.calls[0].signal.aborted, false);
  const second = await f.read();
  assert.equal(f.calls.length, 2);
  f.streams[0].enqueue(new Uint8Array([3])); f.streams[0].close();
  assert.deepEqual([...new Uint8Array(await first.arrayBuffer())], [3]);
  await f.completions[0];
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 1, "retiring an expired fill cannot delete its successor");
  f.streams[1].enqueue(new Uint8Array([4])); f.streams[1].close();
  assert.deepEqual([...new Uint8Array(await second.arrayBuffer())], [4]);
  await Promise.all(f.completions);
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 0);
});

test("fonts stay in the critical cache before and after background or runtime hints", () => {
  const { context } = createWorkerContext(() => assert.fail("policy lookup cannot fetch"));
  const policies = vm.runInContext(`(() => {
    const request = new Request("https://preview.example.test/original-ui/fonts/NotoSansTC.ttf");
    const first = staticAssetCachePolicy(request);
    addStaticAssetTierHints([request.url], "background");
    return [first, staticAssetCachePolicy(request), staticAssetCachePolicy(new Request("https://preview.example.test/original-ui/Title/30.png"))];
  })()`, context);
  assert.equal(policies[0].cacheKind, "static-critical");
  assert.equal(policies[1].cacheKind, "static-critical");
  assert.equal(policies[2].cacheKind, "static-runtime");
});

test("a byte-range font read cannot join or replace a whole-font fill", async t => {
  const f = fontWorkerFixture(t);
  const partial = await f.read(undefined, new Request("https://preview.example.test/original-ui/fonts/NotoSansTC.ttf", { headers: { range: "bytes=0-1" } }));
  const whole = await f.read();
  assert.equal(f.calls.length, 2);
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 1);
  f.streams[0].enqueue(new Uint8Array([1,2])); f.streams[0].close();
  f.streams[1].enqueue(new Uint8Array([1,2,3])); f.streams[1].close();
  assert.equal((await partial.arrayBuffer()).byteLength, 2);
  assert.equal((await whole.arrayBuffer()).byteLength, 3);
  await Promise.all(f.completions);
});

test("font response retention stays bounded when many distinct fonts are requested", async t => {
  const f = fontWorkerFixture(t), responses = [];
  for (let index=0; index<17; index++) responses.push(await f.read(undefined,
    new Request(`https://preview.example.test/original-ui/fonts/Font-${index}.ttf`)));
  assert.equal(f.calls.length,17);
  assert.equal(vm.runInContext("fontAssetInflightFetches.size",f.context),16);
  for (const stream of f.streams) { stream.enqueue(new Uint8Array([1])); stream.close(); }
  await Promise.all(responses.map(response=>response.arrayBuffer()));
  await Promise.all(f.completions);
  assert.equal(vm.runInContext("fontAssetInflightFetches.size",f.context),0);
});
