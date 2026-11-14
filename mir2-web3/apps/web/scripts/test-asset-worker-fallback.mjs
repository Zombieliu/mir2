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

const fontTurn = () => new Promise(resolve => setImmediate(resolve));

test("six renderer font reads share two complete-body lanes while ordinary images stay independent", async t => {
  const f = fontWorkerFixture(t);
  const names = ["NotoSansTC", "NotoSans", "NotoSansArabic", "NotoSansThai", "NotoSansDevanagari", "NotoSansCJKsc"];
  const reads = names.map(name => f.read(undefined, new Request(`https://preview.example.test/original-ui/fonts/${name}.ttf`)));
  await fontTurn();
  assert.equal(f.calls.length, 2, "the renderer must not bypass the font download budget");
  const image = await vm.runInContext('fetchStaticAsset(new Request("https://preview.example.test/original-ui/Prguse/1.png"))', f.context);
  assert.equal(f.calls.length, 3, "font admission must not occupy the ordinary image lane");
  f.streams[2].enqueue(new Uint8Array([137, 80])); f.streams[2].close();
  assert.deepEqual([...new Uint8Array(await image.arrayBuffer())], [137, 80]);
  const fontStreams = () => f.calls.map((request, index) => ({ request, stream: f.streams[index] }))
    .filter(row => new URL(row.request.url).pathname.includes("/fonts/"));
  for (let index = 0; index < names.length; index++) {
    await fontTurn();
    const streams = fontStreams();
    assert(streams[index], "a completed body admits the next waiting font");
    assert(streams.length - index <= 2, "headers alone cannot release a font lane");
    streams[index].stream.enqueue(new Uint8Array([index + 1])); streams[index].stream.close();
  }
  const responses = await Promise.all(reads);
  assert.deepEqual(await Promise.all(responses.map(async response => [...new Uint8Array(await response.arrayBuffer())])), names.map((_, i) => [i + 1]));
  await Promise.all(f.completions);
});

test("a font waiting behind live bodies starts its own unchanged header budget only after admission", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = fontWorkerFixture(t);
  const reads = ["First", "Second", "Third"].map(name => f.read(undefined, new Request(`https://preview.example.test/original-ui/fonts/${name}.ttf`)));
  await fontTurn(); assert.equal(f.calls.length, 2);
  t.mock.timers.tick(8000); await fontTurn();
  assert.equal(f.calls.length, 2, "queue time is not network header time");
  f.streams[0].enqueue(new Uint8Array([1])); f.streams[0].close();
  await fontTurn();
  assert.equal(f.calls.length, 3); assert.equal(f.calls[2].signal.aborted, false);
  f.streams[1].enqueue(new Uint8Array([2])); f.streams[1].close();
  f.streams[2].enqueue(new Uint8Array([3])); f.streams[2].close();
  assert.deepEqual(await Promise.all(reads.map(async read => [...new Uint8Array(await (await read).arrayBuffer())])), [[1], [2], [3]]);
  await Promise.all(f.completions);
});

test("a cancelled queued direct font reader leaves no network request or occupied lane", async t => {
  const f = fontWorkerFixture(t), owner = new AbortController();
  const first = f.read("font-a"), second = f.read("font-b");
  await fontTurn();
  f.context.cancelledFont = new Request("https://preview.example.test/original-ui/fonts/Cancelled.ttf", { signal: owner.signal });
  const cancelled = vm.runInContext('fetchCompleteFont(cancelledFont, cancelledFont.signal, 7000, true)', f.context);
  await fontTurn(); assert.equal(f.calls.length, 2);
  owner.abort("queued direct reader left"); assert.equal(await cancelled, null);
  f.streams[0].enqueue(new Uint8Array([1])); f.streams[0].close();
  f.streams[1].enqueue(new Uint8Array([2])); f.streams[1].close();
  await Promise.all([first, second]); await Promise.all(f.completions);
  assert.equal(f.calls.length, 2);
  const next = f.read("font-c"); await fontTurn();
  assert.equal(f.calls.length, 3); f.streams[2].enqueue(new Uint8Array([3])); f.streams[2].close();
  assert.deepEqual([...new Uint8Array(await (await next).arrayBuffer())], [3]); await Promise.all(f.completions);
});

test("retiring the first HUD consumer cannot cancel a coalesced prewarm body", async t => {
  const owner = new AbortController(), calls = [];
  let headers, body;
  const headerGate = new Promise(resolve => { headers = resolve; });
  const { context } = createWorkerContext(async request => {
    calls.push(request);
    await headerGate;
    return new Response(new ReadableStream({ start(controller) {
      body = controller;
      request.signal.addEventListener("abort", () => controller.error(request.signal.reason), { once: true });
    } }));
  });
  t.after(() => { headers(); try { body?.close(); } catch {} });
  context.firstHud = new Request("https://preview.example.test/original-ui/Prguse/1.png", { signal: owner.signal });
  context.nextHud = new Request(context.firstHud.url);
  const first = vm.runInContext("coalescedStaticAssetFetch(firstHud)", context);
  const next = vm.runInContext("coalescedStaticAssetFetch(nextHud)", context);
  assert.equal(calls.length, 1, "consumers share one pending header request");
  headers();
  const [image, prewarm] = await Promise.all([first, next]);
  owner.abort("legacy HUD unmounted during Native handoff");
  assert.equal(calls[0].signal.aborted, false, "one image retiring must not abort the shared network body");
  body.enqueue(new Uint8Array([137, 80, 78, 71])); body.close();
  assert.deepEqual([...new Uint8Array(await prewarm.arrayBuffer())], [137, 80, 78, 71]);
  assert.deepEqual([...new Uint8Array(await image.arrayBuffer())], [137, 80, 78, 71]);
});

test("CSS and renderer consumers share one font through its unfinished body and cache fill", async t => {
  const f = fontWorkerFixture(t);
  let exposed = false;
  const first = f.read().then(response => { exposed = true; return response; });
  await fontTurn();
  f.streams[0].enqueue(new Uint8Array([1, 2]));
  const second = f.read();
  await fontTurn();
  assert.equal(f.calls.length, 1, "the second consumer must not download the same font again");
  assert.equal(exposed, false, "neither CSS nor the renderer may see an unfinished font prefix");
  assert.equal(f.writes(), 0, "an unfinished font must not enter CacheStorage");
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 1);
  f.streams[0].enqueue(new Uint8Array([3, 4])); f.streams[0].close();
  assert.deepEqual([...new Uint8Array(await (await first).arrayBuffer())], [1,2,3,4]);
  assert.deepEqual([...new Uint8Array(await (await second).arrayBuffer())], [1,2,3,4]);
  await Promise.all(f.completions);
  assert.equal(f.writes(), 1, "one complete shared font has only one cache writer");
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 0);
  const cached = await f.read();
  assert.deepEqual([...new Uint8Array(await cached.arrayBuffer())], [1,2,3,4]);
  assert.equal(f.calls.length, 1);
});

test("one font consumer retiring does not cancel another consumer's shared download", async t => {
  const f = fontWorkerFixture(t), owner = new AbortController();
  const first = f.read(undefined, new Request("https://preview.example.test/original-ui/fonts/NotoSansTC.ttf", { signal: owner.signal }));
  await fontTurn();
  owner.abort("first document left");
  assert.equal(f.calls[0].signal.aborted, false);
  const second = f.read();
  await fontTurn();
  assert.equal(f.calls.length, 1);
  f.streams[0].enqueue(new Uint8Array([8,9])); f.streams[0].close();
  assert.deepEqual([...new Uint8Array(await (await second).arrayBuffer())], [8,9]);
  assert.deepEqual([...new Uint8Array(await (await first).arrayBuffer())], [8,9]);
  await Promise.all(f.completions);
});

test("font fills retain separate cache namespaces across a release change", async t => {
  const f = fontWorkerFixture(t);
  const old = f.read("version-a"), next = f.read("version-b");
  await fontTurn();
  assert.equal(f.calls.length, 2);
  f.streams[0].enqueue(new Uint8Array([1])); f.streams[0].close();
  f.streams[1].enqueue(new Uint8Array([2])); f.streams[1].close();
  assert.deepEqual([...new Uint8Array(await (await old).arrayBuffer())], [1]);
  assert.deepEqual([...new Uint8Array(await (await next).arrayBuffer())], [2]);
  await Promise.all(f.completions);
});

test("font lookup expiry releases the retained entry without truncating either body", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = fontWorkerFixture(t), first = f.read(), progress = [];
  await fontTurn();
  for (let index = 0; index < 13; index++) {
    t.mock.timers.tick(14_000); await fontTurn();
    if (index < 12) {
      f.streams[0].enqueue(new Uint8Array([2])); progress.push(2); await fontTurn();
    }
  }
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 0);
  assert.equal(f.calls[0].signal.aborted, false);
  const second = f.read();
  await fontTurn();
  assert.equal(f.calls.length, 2);
  f.streams[0].enqueue(new Uint8Array([3])); f.streams[0].close();
  assert.deepEqual([...new Uint8Array(await (await first).arrayBuffer())], [...progress, 3]);
  await f.completions[0];
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 1, "retiring an expired fill cannot delete its successor");
  f.streams[1].enqueue(new Uint8Array([4])); f.streams[1].close();
  assert.deepEqual([...new Uint8Array(await (await second).arrayBuffer())], [4]);
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
  const whole = f.read();
  await fontTurn();
  assert.equal(f.calls.length, 2);
  assert.equal(vm.runInContext("fontAssetInflightFetches.size", f.context), 1);
  f.streams[0].enqueue(new Uint8Array([1,2])); f.streams[0].close();
  f.streams[1].enqueue(new Uint8Array([1,2,3])); f.streams[1].close();
  assert.equal((await partial.arrayBuffer()).byteLength, 2);
  assert.equal((await (await whole).arrayBuffer()).byteLength, 3);
  await Promise.all(f.completions);
});

test("font response retention stays bounded when many distinct fonts are requested", async t => {
  const f = fontWorkerFixture(t), responses = [];
  for (let index=0; index<17; index++) {
    responses.push(f.read(undefined, new Request(`https://preview.example.test/original-ui/fonts/Font-${index}.ttf`)));
    await fontTurn();
  }
  assert.equal(f.calls.length,2);
  assert.equal(vm.runInContext("fontAssetInflightFetches.size",f.context),16);
  for (let index=0; index<17; index++) {
    await fontTurn();
    assert(f.streams[index]);
    f.streams[index].enqueue(new Uint8Array([1])); f.streams[index].close();
  }
  await Promise.all(responses.map(async response=>(await response).arrayBuffer()));
  await Promise.all(f.completions);
  assert.equal(vm.runInContext("fontAssetInflightFetches.size",f.context),0);
});

function configureCurrentFontMirror(context, version = "font-release-a", includeSecondary = false) {
  const origin = "https://preview.example.test/original-ui/fonts/NotoSansTC.ttf";
  const mirror = `https://public-r2.example.test/hotlink-ok/mir2/v/${version}/original-ui/fonts/NotoSansTC.ttf`;
  context.fontRouteConfig = {
    version, assetVersion: version, fontMirrorFirst: true,
    remoteAssetBaseUrl: `https://cdn.example.test/mir2/v/${version}`,
    remoteAssetBaseUrls: [
      "https://public-r2.example.test/hotlink-ok/mir2/v/older-release",
      `https://public-r2.example.test/hotlink-ok/mir2/v/${version}`,
      ...(includeSecondary ? [`https://cdn.example.test/mir2/v/${version}`] : []),
    ],
  };
  vm.runInContext("runtimeConfig = { ...runtimeConfig, ...fontRouteConfig }", context);
  return { origin, mirror, secondary: `https://cdn.example.test/mir2/v/${version}/original-ui/fonts/NotoSansTC.ttf` };
}

function configureAppFirstFont(context) {
  const routes = configureCurrentFontMirror(context);
  vm.runInContext(`runtimeConfig.fontMirrorFirst = false;
    runtimeConfig.remoteAssetBaseUrls = [fontRouteConfig.remoteAssetBaseUrls[1], fontRouteConfig.remoteAssetBaseUrls[0]]`, context);
  return routes;
}

test("a complete shipped app font is exposed and cached without any CDN read", async t => {
  const f = fontWorkerFixture(t), { origin } = configureAppFirstFont(f.context);
  let exposed = false;
  const pending = f.read().then(response => { exposed = true; return response; });
  await fontTurn();
  f.streams[0].enqueue(new Uint8Array([1, 2])); await fontTurn();
  assert.equal(exposed, false); assert.equal(f.writes(), 0);
  assert.deepEqual(f.calls.map(request => request.url), [origin]);
  f.streams[0].enqueue(new Uint8Array([3, 4])); f.streams[0].close();
  assert.deepEqual([...new Uint8Array(await (await pending).arrayBuffer())], [1, 2, 3, 4]);
  await Promise.all(f.completions);
  assert.equal(f.writes(), 1);
  assert.deepEqual([...new Uint8Array(await (await f.read()).arrayBuffer())], [1, 2, 3, 4]);
  assert.deepEqual(f.calls.map(request => request.url), [origin]);
});

test("a stalled app font prefix waits for a complete CDN fallback and never poisons the shared cache", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = fontWorkerFixture(t), { origin, mirror } = configureAppFirstFont(f.context);
  let exposed = false;
  const pending = f.read().then(response => { exposed = true; return response; });
  await fontTurn();
  f.streams[0].enqueue(new Uint8Array([1, 2])); await fontTurn();
  t.mock.timers.tick(14_999); await fontTurn();
  assert.equal(f.calls.length, 1); assert.equal(exposed, false); assert.equal(f.writes(), 0);
  t.mock.timers.tick(1); await fontTurn();
  assert(f.calls[0].signal.aborted);
  assert.deepEqual(f.calls.map(request => request.url), [origin, mirror]);
  f.streams[1].enqueue(new Uint8Array([7])); await fontTurn();
  assert.equal(exposed, false); assert.equal(f.writes(), 0);
  f.streams[1].enqueue(new Uint8Array([8, 9])); f.streams[1].close();
  assert.deepEqual([...new Uint8Array(await (await pending).arrayBuffer())], [7, 8, 9]);
  await Promise.all(f.completions);
  assert.equal(f.writes(), 1);
  assert.deepEqual([...new Uint8Array(await (await f.read()).arrayBuffer())], [7, 8, 9]);
  assert.equal(f.calls.length, 2, "the cache must contain only the completed fallback bytes");
});

test("app font response headers retain their seven-second bound before ordinary mirror fallback", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [];
  let routes;
  const { context } = createWorkerContext(request => {
    calls.push(request);
    if (request.url === routes.origin) return new Promise((_, reject) =>
      request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true }));
    assert.equal(request.url, routes.mirror);
    return Promise.resolve(new Response(new Uint8Array([7, 8])));
  });
  routes = configureAppFirstFont(context);
  const pending = vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(routes.origin)}))`, context);
  await fontTurn(); t.mock.timers.tick(6_999); await fontTurn();
  assert.equal(calls.length, 1);
  t.mock.timers.tick(1);
  assert.deepEqual([...new Uint8Array(await (await pending).arrayBuffer())], [7, 8]);
  assert.deepEqual(calls.map(request => request.url), [routes.origin, routes.mirror]);
  assert(calls[0].signal.aborted);
});

test("app-first fonts preserve an explicitly older-first ordinary fallback order", async () => {
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request.url);
    return request.url.includes("older-release") ? new Response(new Uint8Array([7])) : new Response(null, { status: 404 });
  });
  const { origin } = configureCurrentFontMirror(context);
  vm.runInContext("runtimeConfig.fontMirrorFirst = false", context);
  const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [7]);
  assert.deepEqual(calls, [origin, "https://public-r2.example.test/hotlink-ok/mir2/v/older-release/original-ui/fonts/NotoSansTC.ttf"]);
});

test("app-first complete font reads work without CacheStorage", async t => {
  let body, exposed = false;
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request.url);
    return new Response(new ReadableStream({ start(controller) { body = controller; } }));
  }, { async open() { throw new Error("storage denied"); } });
  const { origin } = configureAppFirstFont(context);
  t.after(() => { try { body?.close(); } catch {} });
  const pending = vm.runInContext(`cacheFirst(new Request(${JSON.stringify(origin)}), "disabled", 20)`, context)
    .then(response => { exposed = true; return response; });
  await fontTurn(); body.enqueue(new Uint8Array([1])); await fontTurn();
  assert.equal(exposed, false);
  body.enqueue(new Uint8Array([2])); body.close();
  assert.deepEqual([...new Uint8Array(await (await pending).arrayBuffer())], [1, 2]);
  assert.deepEqual(calls, [origin]);
});

test("partial, empty and oversized app fonts fall back without exposing their bytes", async () => {
  for (const invalid of ["empty", "partial", "range-header", "oversized"]) {
    const calls = [];
    let routes;
    const { context } = createWorkerContext(async request => {
      calls.push(request.url);
      if (request.url !== routes.origin) return new Response(new Uint8Array([7, 8]));
      if (invalid === "empty") return new Response("");
      if (invalid === "partial") return new Response(new Uint8Array([1]), { status: 206 });
      if (invalid === "range-header") return new Response(new Uint8Array([1]), { headers: { "content-range": "bytes 0-0/4" } });
      return new Response(new Uint8Array(32 * 1024 * 1024 + 1));
    });
    routes = configureAppFirstFont(context);
    const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(routes.origin)}))`, context);
    assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [7, 8], invalid);
    assert.deepEqual(calls, [routes.origin, routes.mirror], invalid);
  }
});

test("a timed-out preferred font mirror tries the other current-release origin before the app", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [];
  let routes;
  const { context } = createWorkerContext(request => {
    calls.push(request);
    if (request.url === routes.mirror) return new Promise((_, reject) =>
      request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true }));
    assert([routes.secondary, routes.origin].includes(request.url));
    return Promise.resolve(new Response(new Uint8Array(request.url === routes.secondary ? [7, 8, 9] : [0])));
  });
  routes = configureCurrentFontMirror(context, "font-release-a", true);
  const pending = vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(routes.origin)}))`, context);
  await new Promise(resolve => setImmediate(resolve));
  t.mock.timers.tick(7999); await new Promise(resolve => setImmediate(resolve)); assert.equal(calls.length, 1);
  t.mock.timers.tick(1);
  assert.deepEqual([...new Uint8Array(await (await pending).arrayBuffer())], [7, 8, 9]);
  assert.deepEqual(calls.map(r => r.url), [routes.mirror, routes.secondary]);
  assert(calls[0].signal.aborted);
});

test("a stopped preferred font body is replaced by one complete current-release mirror", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [];
  let routes, cancelled = false;
  const { context } = createWorkerContext(async request => {
    calls.push(request.url);
    if (request.url === routes.mirror) return new Response(new ReadableStream({
      start(stream) { stream.enqueue(new Uint8Array([1])); }, cancel() { cancelled = true; },
    }));
    assert([routes.secondary, routes.origin].includes(request.url));
    return new Response(new Uint8Array(request.url === routes.secondary ? [7, 8, 9] : [0]));
  });
  routes = configureCurrentFontMirror(context, "font-release-a", true);
  const pending = vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(routes.origin)}))`, context);
  await new Promise(resolve => setImmediate(resolve));
  t.mock.timers.tick(15000);
  assert.deepEqual([...new Uint8Array(await (await pending).arrayBuffer())], [7, 8, 9]);
  assert(cancelled); assert.deepEqual(calls, [routes.mirror, routes.secondary]);
});

test("all current font mirrors precede the app while inherited releases remain later fallbacks", async () => {
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request.url);
    return request.url.startsWith("https://preview.example.test/")
      ? new Response(new Uint8Array([6])) : new Response(null, { status: 404 });
  });
  const routes = configureCurrentFontMirror(context, "font-release-a", true);
  const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(routes.origin)}))`, context);
  assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [6]);
  assert.deepEqual(calls, [routes.mirror, routes.secondary, routes.origin]);
});

test("a current-release font mirror precedes a successful but slow app origin", async () => {
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request); return new Response(new Uint8Array([1, 2, 3]));
  });
  const { origin, mirror } = configureCurrentFontMirror(context);
  const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [1, 2, 3]);
  assert.deepEqual(calls.map(request => request.url), [mirror]);
  assert.equal(calls[0].credentials, "omit"); assert.equal(calls[0].mode, "cors");
});

test("a missing current font mirror returns to the app before inherited releases", async () => {
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request.url);
    return request.url.startsWith("https://preview.example.test/")
      ? new Response(new Uint8Array([5, 6])) : new Response(null, { status: 404 });
  });
  const { origin, mirror } = configureCurrentFontMirror(context);
  const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [5, 6]);
  assert.deepEqual(calls, [mirror, origin]);
});

test("a font mirror connection failure still allows the original complete font", async () => {
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request.url);
    if (!request.url.startsWith("https://preview.example.test/")) throw new TypeError("mirror offline");
    return new Response(new Uint8Array([7, 8]));
  });
  const { origin, mirror } = configureCurrentFontMirror(context);
  const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [7, 8]);
  assert.deepEqual(calls, [mirror, origin]);
});

test("font mirror headers have an eight-second bound before returning to the app", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [];
  const { context } = createWorkerContext(request => {
    calls.push(request);
    if (request.url.startsWith("https://preview.example.test/")) return Promise.resolve(new Response("font"));
    return new Promise((_, reject) => request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true }));
  });
  const { origin, mirror } = configureCurrentFontMirror(context);
  const pending = vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  await new Promise(resolve => setImmediate(resolve));
  t.mock.timers.tick(7_999); await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(calls.map(request => request.url), [mirror]);
  t.mock.timers.tick(1);
  assert.equal(await (await pending).text(), "font");
  assert.deepEqual(calls.map(request => request.url), [mirror, origin]);
  assert.equal(calls[0].signal.aborted, true);
});

test("font mirror header timeout does not truncate a progressing body", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = fontWorkerFixture(t);
  const { mirror } = configureCurrentFontMirror(f.context);
  const first = f.read();
  await new Promise(resolve => setImmediate(resolve));
  f.streams[0].enqueue(new Uint8Array([1]));
  await new Promise(resolve => setImmediate(resolve));
  t.mock.timers.tick(12_000);
  const second = f.read();
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(f.calls.map(request => request.url), [mirror]);
  assert.equal(f.calls[0].signal.aborted, false);
  f.streams[0].enqueue(new Uint8Array([2, 3])); f.streams[0].close();
  assert.deepEqual([...new Uint8Array(await (await first).arrayBuffer())], [1, 2, 3]);
  assert.deepEqual([...new Uint8Array(await (await second).arrayBuffer())], [1, 2, 3]);
  await Promise.all(f.completions);
  assert.equal(f.writes(), 1);
  await (await f.read()).arrayBuffer(); assert.equal(f.calls.length, 1);
});

test("a font mirror body that stops after a prefix returns the complete app font", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [];
  let cancelled = false;
  const { context } = createWorkerContext(async request => {
    calls.push(request);
    return request.url.startsWith("https://preview.example.test/") ? new Response(new Uint8Array([7, 8]))
      : new Response(new ReadableStream({
        start(controller) { controller.enqueue(new Uint8Array([1])); },
        cancel() { cancelled = true; },
      }));
  });
  const { origin, mirror } = configureCurrentFontMirror(context);
  const pending = vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  await new Promise(resolve => setImmediate(resolve));
  t.mock.timers.tick(14_999); await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(calls.map(request => request.url), [mirror]);
  t.mock.timers.tick(1); await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(calls.map(request => request.url), [mirror, origin]);
  assert.deepEqual([...new Uint8Array(await (await pending).arrayBuffer())], [7, 8]);
  assert.equal(calls[0].signal.aborted, true);
  assert.equal(cancelled, true);
});

test("font body idle time resets on progress beyond the header and idle budgets", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = fontWorkerFixture(t);
  const { mirror } = configureCurrentFontMirror(f.context);
  const first = f.read(); await new Promise(resolve => setImmediate(resolve));
  for (const value of [1, 2, 3]) {
    t.mock.timers.tick(14_000);
    f.streams[0].enqueue(new Uint8Array([value]));
    await new Promise(resolve => setImmediate(resolve));
  }
  const second = f.read(); await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(f.calls.map(request => request.url), [mirror]);
  assert.equal(f.calls[0].signal.aborted, false);
  f.streams[0].close();
  assert.deepEqual([...new Uint8Array(await (await first).arrayBuffer())], [1, 2, 3]);
  assert.deepEqual([...new Uint8Array(await (await second).arrayBuffer())], [1, 2, 3]);
  await Promise.all(f.completions);
});

test("an errored font body returns the complete app font instead of its prefix", async () => {
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request.url);
    if (request.url.startsWith("https://preview.example.test/")) return new Response(new Uint8Array([9, 8]));
    return new Response(new ReadableStream({
      start(controller) { controller.enqueue(new Uint8Array([1])); },
      pull(controller) { controller.error(new TypeError("font transport failed")); },
    }));
  });
  const { origin, mirror } = configureCurrentFontMirror(context);
  const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [9, 8]);
  assert.deepEqual(calls, [mirror, origin]);
});

test("a completed decoded font retains its type and correct body metadata", async () => {
  const { context } = createWorkerContext(async () => new Response(new Uint8Array([1, 2, 3]), {
    headers: { "content-type": "font/ttf", "content-encoding": "gzip", "content-length": "99", "cache-control": "public, max-age=31536000, immutable" },
  }));
  const { origin } = configureCurrentFontMirror(context);
  const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  assert.equal(response.headers.get("content-encoding"), null);
  assert.equal(response.headers.get("content-length"), "3");
  assert.equal(response.headers.get("content-type"), "font/ttf");
  assert.equal(response.headers.get("cache-control"), "public, max-age=31536000, immutable");
  assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [1, 2, 3]);
});

test("empty mirror bodies return to the app", async () => {
  for (const body of [null, ""]) {
    const calls = [];
    const { context } = createWorkerContext(async request => {
      calls.push(request.url);
      return new Response(request.url.startsWith("https://preview.example.test/") ? "complete" : body);
    });
    const { origin, mirror } = configureCurrentFontMirror(context);
    const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
    assert.equal(await response.text(), "complete");
    assert.deepEqual(calls, [mirror, origin]);
  }
});

test("font mirror buffering is bounded before returning to the app", async () => {
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request);
    return new Response(request.url.startsWith("https://preview.example.test/")
      ? "complete" : new Uint8Array(32 * 1024 * 1024 + 1));
  });
  const { origin, mirror } = configureCurrentFontMirror(context);
  const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  assert.equal(await response.text(), "complete");
  assert.deepEqual(calls.map(request => request.url), [mirror, origin]);
  assert.equal(calls[0].signal.aborted, true);
});

test("empty chunks do not renew a stalled font body", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let stream;
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request.url);
    return request.url.startsWith("https://preview.example.test/") ? new Response("complete")
      : new Response(new ReadableStream({ start(controller) { stream = controller; } }));
  });
  const { origin, mirror } = configureCurrentFontMirror(context);
  const pending = vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
  await new Promise(resolve => setImmediate(resolve));
  t.mock.timers.tick(14_000); stream.enqueue(new Uint8Array());
  await new Promise(resolve => setImmediate(resolve));
  t.mock.timers.tick(1_000); await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(calls, [mirror, origin]);
  assert.equal(await (await pending).text(), "complete");
});

test("font mirror selection works even when CacheStorage is unavailable", async () => {
  const calls = [];
  const { context } = createWorkerContext(async request => {
    calls.push(request.url); return new Response("font");
  }, { async open() { throw new Error("storage denied"); } });
  const { origin, mirror } = configureCurrentFontMirror(context);
  const response = await vm.runInContext(`cacheFirst(new Request(${JSON.stringify(origin)}), "disabled", 20)`, context);
  assert.equal(await response.text(), "font"); assert.deepEqual(calls, [mirror]);
});

test("unversioned fonts, byte ranges and images keep their app-first route", async () => {
  for (const kind of ["unversioned", "range", "image"]) {
    const calls = [];
    const { context } = createWorkerContext(async request => { calls.push(request); return new Response("original"); });
    let { origin } = configureCurrentFontMirror(context);
    if (kind === "unversioned") vm.runInContext('runtimeConfig.assetVersion = "bootstrap"', context);
    if (kind === "image") origin = "https://preview.example.test/original-ui/Prguse/985.png";
    context.unpreferredFontRequest = new Request(origin, kind === "range" ? { headers: { Range: "bytes=0-1" } } : {});
    const response = await vm.runInContext('fetchStaticAsset(unpreferredFontRequest)', context);
    assert.equal(await response.text(), "original"); assert.equal(calls.length, 1);
    assert.equal(calls[0].url, origin);
  }
});

test("partial or empty mirror responses cannot replace an entire font", async () => {
  for (const status of [204, 206]) {
    const calls = [];
    const { context } = createWorkerContext(async request => {
      calls.push(request.url);
      return request.url.startsWith("https://preview.example.test/")
        ? new Response("complete") : new Response(status === 204 ? null : "partial", { status });
    });
    const { origin, mirror } = configureCurrentFontMirror(context);
    const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
    assert.equal(await response.text(), "complete"); assert.deepEqual(calls, [mirror, origin]);
  }
});

test("only explicit hosted configuration opts into the current font mirror", async () => {
  for (const value of [undefined, false, "true", true]) {
    const calls = [], completions = [];
    const { context, listeners } = createWorkerContext(async request => { calls.push(request.url); return new Response("font"); });
    const { origin, mirror } = configureCurrentFontMirror(context);
    listeners.get("message")({
      data: { type: "MIR2_ASSET_CACHE_CONFIG", manifestVersion: "font-release-a", fontMirrorFirst: value,
        browserFallbackBaseUrls: ["https://public-r2.example.test/hotlink-ok/mir2/v/font-release-a"] },
      waitUntil(promise) { completions.push(promise); },
    });
    await Promise.all(completions);
    const response = await vm.runInContext(`fetchStaticAsset(new Request(${JSON.stringify(origin)}))`, context);
    assert.equal(await response.text(), "font");
    assert.deepEqual(calls, [value === true ? mirror : origin]);
    assert.equal(vm.runInContext("runtimeConfig.fontMirrorFirst", context), value === true);
  }
});

test("a stalled shared image header retires before page retries keep joining it", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [], owner = new AbortController();
  let releaseOrigin;
  const { context } = createWorkerContext(request => {
    calls.push(request);
    if (!request.url.startsWith("https://preview.example.test/")) {
      return Promise.resolve(new Response(new Uint8Array([137, 80, 78, 71])));
    }
    return new Promise((resolve, reject) => {
      releaseOrigin = () => resolve(new Response(new Uint8Array([1])));
      request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true });
    });
  });
  vm.runInContext(`runtimeConfig = { ...runtimeConfig,
    remoteAssetBaseUrls: ["https://public-r2.example.test/release-a"] }`, context);
  context.image = new Request("https://preview.example.test/original-ui/Prguse/1.png", { signal: owner.signal });
  const first = vm.runInContext("coalescedStaticAssetFetch(image)", context);
  const retry = vm.runInContext("coalescedStaticAssetFetch(new Request(image.url))", context);
  t.after(async () => { releaseOrigin?.(); await Promise.allSettled([first, retry]); });
  owner.abort("first image unmounted");
  assert.equal(calls[0].signal.aborted, false, "consumer cancellation must still leave the shared request alive");
  t.mock.timers.tick(6_999); await new Promise(resolve => setImmediate(resolve));
  assert.equal(calls.length, 1);
  t.mock.timers.tick(1); await new Promise(resolve => setImmediate(resolve));
  assert.equal(calls.length, 2, "the worker must leave a stalled origin before the page's 10s header deadline");
  assert.equal(calls[0].signal.aborted, true);
  assert.equal(calls[1].url, "https://public-r2.example.test/release-a/original-ui/Prguse/1.png");
  const responses = await Promise.all([first, retry]);
  for (const response of responses) assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [137, 80, 78, 71]);
  assert.equal(vm.runInContext("staticAssetInflightFetches.size", context), 0);
  assert.equal(vm.runInContext("staticAssetNegativeCache.size", context), 0);
});

test("each static fallback gets its own bounded header attempt", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [], releases = [];
  const { context } = createWorkerContext(request => {
    calls.push(request);
    if (request.url.startsWith("https://second-r2.example.test/")) return Promise.resolve(new Response("complete"));
    return new Promise((resolve, reject) => {
      releases.push(() => resolve(new Response(null, { status: 404 })));
      request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true });
    });
  });
  vm.runInContext(`runtimeConfig = { ...runtimeConfig,
    remoteAssetBaseUrls: ["https://first-r2.example.test/v1", "https://second-r2.example.test/v1"] }`, context);
  const pending = vm.runInContext(`coalescedStaticAssetFetch(new Request("https://preview.example.test/original-ui/Prguse/4.png"))`, context);
  t.after(async () => { releases.forEach(release => release()); await pending; });
  for (let n = 0; n < 2; n++) {
    t.mock.timers.tick(7_000); await new Promise(resolve => setImmediate(resolve));
    assert.equal(calls.length, n + 2);
    assert.equal(calls[n].signal.aborted, true);
  }
  assert.deepEqual(calls.map(request => new URL(request.url).hostname),
    ["preview.example.test", "first-r2.example.test", "second-r2.example.test"]);
  assert.equal(await (await pending).text(), "complete");
  assert.equal(vm.runInContext("staticAssetInflightFetches.size", context), 0);
});

test("an exhausted static header deadline is transient and the next read is fresh", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let calls = 0, release;
  const { context } = createWorkerContext(request => {
    if (++calls > 1) return Promise.resolve(new Response("recovered"));
    return new Promise((resolve, reject) => {
      release = () => resolve(new Response(null, { status: 404 }));
      request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true });
    });
  });
  const read = () => vm.runInContext(`coalescedStaticAssetFetch(new Request("https://preview.example.test/original-ui/Prguse/6.png"))`, context);
  const pending = read();
  t.after(async () => { release?.(); await pending; });
  t.mock.timers.tick(7_000); await new Promise(resolve => setImmediate(resolve));
  assert.equal((await pending).status, 504);
  assert.equal(vm.runInContext("staticAssetNegativeCache.size", context), 0);
  assert.equal(vm.runInContext("staticAssetInflightFetches.size", context), 0);
  assert.equal(await (await read()).text(), "recovered");
  assert.equal(calls, 2);
});

test("static header expiry never truncates an already delivered image body", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let body, signal;
  const { context } = createWorkerContext(request => {
    signal = request.signal;
    return Promise.resolve(new Response(new ReadableStream({ start(controller) { body = controller; } })));
  });
  const response = await vm.runInContext(`coalescedStaticAssetFetch(new Request("https://preview.example.test/original-ui/Prguse/8.png"))`, context);
  const pending = response.arrayBuffer();
  t.after(() => { try { body.close(); } catch {} });
  body.enqueue(new Uint8Array([1, 2]));
  t.mock.timers.tick(21_000); await new Promise(resolve => setImmediate(resolve));
  assert.equal(signal.aborted, false, "header timing must be cleared before streaming the body");
  body.enqueue(new Uint8Array([3, 4])); body.close();
  assert.deepEqual([...new Uint8Array(await pending)], [1, 2, 3, 4]);
});
