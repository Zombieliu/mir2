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
