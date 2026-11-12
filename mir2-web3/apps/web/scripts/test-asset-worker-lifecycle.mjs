import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const source = readFileSync(new URL("../public/mir2-asset-worker.js", import.meta.url), "utf8");
const origin = "https://game.example.test";
const path = "/original-ui/Prguse/1.png";
const bytes = [11, 22, 33];
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };
const flush = () => new Promise(resolve => setImmediate(resolve));

function storage() {
  const stores = new Map();
  const key = request => typeof request === "string" ? request : request.url;
  return {
    stores,
    async keys() { return [...stores.keys()]; },
    async delete(name) { return stores.delete(name); },
    async open(name) {
      if (!stores.has(name)) {
        const entries = new Map();
        stores.set(name, {
          entries,
          async match(request) { return entries.get(key(request))?.clone(); },
          async put(request, response) {
            const body = await response.arrayBuffer();
            entries.set(key(request), new Response(body, { status: response.status, headers: response.headers }));
          },
          async keys() { return [...entries.keys()].map(url => new Request(url)); },
          async delete(request) { return entries.delete(key(request)); },
        });
      }
      return stores.get(name);
    },
  };
}

function worker(cacheStorage, fetchImpl) {
  const listeners = new Map(), calls = [];
  const network = async request => {
    calls.push(request.url);
    return fetchImpl?.(request) ?? new Response(new Uint8Array(bytes), { status: request.url.startsWith(origin + "/") ? 504 : 200 });
  };
  const context = vm.createContext({ AbortController, AbortSignal, caches: cacheStorage, clearTimeout, console,
    fetch: network, Headers, Map, Request, Response, Set, setTimeout, URL,
    self: { location: { origin }, clients: { async claim() {} }, async skipWaiting() {},
      addEventListener(type, listener) { listeners.set(type, listener); } } });
  vm.runInContext(source, context);
  function message(data) {
    const waits = [], replies = [];
    listeners.get("message")({ data, source: { postMessage(value) { replies.push(value); } }, waitUntil(value) { waits.push(value); } });
    return { replies, done: Promise.all(waits) };
  }
  function fetch(pathname = path, options) {
    const request = new Request(new URL(pathname, origin), options), waits = [];
    let response, active = true;
    listeners.get("fetch")({ request,
      respondWith(value) { assert(active, "respondWith must be synchronous"); assert.equal(response, undefined); response = Promise.resolve(value); },
      waitUntil(value) { assert(active, "native waitUntil must be synchronous"); waits.push(value); },
    });
    active = false;
    const intercepted = Boolean(response);
    return { intercepted, response: response ?? network(request), async done() { await Promise.all(waits); } };
  }
  return { context, message, fetch, calls };
}

function configuration(version = "a") {
  return { type: "MIR2_ASSET_CACHE_CONFIG", manifestVersion: "release-" + version,
    cacheNamespace: "namespace-" + version, fontMirrorFirst: true,
    manifest: { version: "release-" + version,
      remoteAssets: { assetBaseUrl: "https://mirror.example.test/release-" + version, browserFallbackBaseUrls: [] },
      resourcePacks: [{ cacheTier: "critical", urls: [path] }] } };
}

async function configure(w, version) {
  const message = w.message(configuration(version)); await message.done;
  assert(message.replies.some(x => x.type === "MIR2_ASSET_CACHE_CONFIGURED"));
  return message.replies.find(x => x.type === "MIR2_ASSET_CACHE_CONFIGURED");
}

test("a restarted worker serves the complete already-versioned HUD cache without another network read", async () => {
  const cache = storage(), first = worker(cache);
  await configure(first, "a");
  const warm = first.fetch(); assert.equal((await warm.response).status, 200); await warm.done();
  const restarted = worker(cache, async () => new Response(null, { status: 504 }));
  const read = restarted.fetch();
  const response = await read.response;
  assert(read.intercepted, "a cold worker must restore configuration before deciding to bypass");
  assert.equal(response.status, 200); assert.equal(restarted.calls.length, 0);
  assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], bytes); await read.done();
  assert.equal(vm.runInContext("runtimeConfig.version", restarted.context), "namespace-a");
  assert.equal(vm.runInContext("staticAssetTiers.get('/original-ui/Prguse/1.png')", restarted.context), "critical");
});

test("an uncached frame after restart retains exact-release remote fallbacks and original cache tier", async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  const restarted = worker(cache), read = restarted.fetch();
  assert.equal((await read.response).status, 200); await read.done();
  assert.deepEqual(restarted.calls, [origin + path, origin + path, origin + path, "https://mirror.example.test/release-a" + path]);
  assert((await cache.keys()).some(name => name.includes("static-critical-namespace-a")));
});

test("an explicit newer configuration cannot be replaced by an older delayed restore", { timeout: 2000 }, async () => {
  const cache = storage(), initial = worker(cache); await configure(initial, "a");
  const savedOpen = cache.open.bind(cache), gate = deferred(); let restoreStarted = false;
  cache.open = async name => {
    const store = await savedOpen(name);
    if (!name.includes("configuration")) return store;
    return { ...store, async match(request) { const old = await store.match(request); restoreStarted = true; await gate.promise; return old; } };
  };
  const restarted = worker(cache), read = restarted.fetch();
  await flush(); assert(restoreStarted, "the restart must actually read persisted configuration");
  const newer = restarted.message(configuration("b"));
  await newer.done;
  assert.equal((await read.response).status, 200); await read.done();
  gate.resolve(); await flush();
  assert.equal(vm.runInContext("runtimeConfig.version", restarted.context), "namespace-b");
  assert(restarted.calls.includes("https://mirror.example.test/release-b" + path));
  assert(!restarted.calls.some(url => url.includes("release-a")));
});

test("overlapping configuration writes are serialized so a restart restores the latest release", { timeout: 2000 }, async () => {
  const cache = storage(), open = cache.open.bind(cache), gate = deferred(); let writes = 0;
  cache.open = async name => {
    const store = await open(name);
    if (!name.includes("configuration")) return store;
    return { ...store, async put(request, response) { writes += 1; if (writes === 1) await gate.promise; return store.put(request, response); } };
  };
  const first = worker(cache), a = first.message(configuration("a")); await flush();
  const b = first.message(configuration("b")); await flush();
  assert.equal(writes, 1); gate.resolve(); await Promise.all([a.done, b.done]);
  const restarted = worker(cache), read = restarted.fetch(); assert.equal((await read.response).status, 200); await read.done();
  assert.equal(vm.runInContext("runtimeConfig.version", restarted.context), "namespace-b");
});

test("CacheStorage denial preserves active in-memory fallbacks and reports non-durable configuration", async () => {
  const denied = { async open() { throw Error("storage denied"); }, async keys() { throw Error("storage denied"); } };
  const first = worker(denied), reply = await configure(first, "a");
  assert.equal(reply.configurationPersisted, false);
  const read = first.fetch(); assert.equal((await read.response).status, 200); await read.done();
  const restarted = worker(denied, async () => new Response("ordinary origin")), fallback = restarted.fetch();
  assert.equal(await (await fallback.response).text(), "ordinary origin"); await fallback.done();
});

test("corrupt persisted metadata is ignored without fetching a stored arbitrary remote URL", async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  const entry = [...cache.stores].find(([name]) => name.includes("configuration")); assert(entry);
  const store = entry[1]; for (const key of store.entries.keys()) store.entries.set(key, Response.json({ schema: -1, runtimeConfig: { version: "bad", remoteAssetBaseUrls: ["https://wrong.example.test"] } }));
  const restarted = worker(cache, async () => new Response("ordinary origin")), read = restarted.fetch();
  assert.equal(await (await read.response).text(), "ordinary origin"); await read.done();
  assert.deepEqual(restarted.calls, [origin + path]);
});

test("configuration cleanup keeps its single metadata record and the current versioned cache", async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  const warm = first.fetch(); await warm.response; await warm.done();
  await cache.open("mir2-asset-cache-sw8-static-runtime-obsolete"); await cache.open("foreign-cache");
  await configure(first, "a");
  const names = await cache.keys(); assert(names.some(name => name.includes("configuration")));
  assert(names.some(name => name.includes("static-critical-namespace-a")));
  assert(!names.some(name => name.endsWith("obsolete"))); assert(names.includes("foreign-cache"));
});

test("reset waits for an older pending configuration write and cannot resurrect it", { timeout: 2000 }, async () => {
  const cache = storage(), open = cache.open.bind(cache), gate = deferred(); let writeStarted = false;
  cache.open = async name => {
    const store = await open(name);
    return !name.includes("configuration") ? store : { ...store, async put(request, response) { writeStarted = true; await gate.promise; return store.put(request, response); } };
  };
  const first = worker(cache), config = first.message(configuration("a")); await flush(); assert(writeStarted);
  const reset = first.message({ type: "MIR2_ASSET_CACHE_RESET" }); gate.resolve(); await Promise.all([config.done, reset.done]);
  assert.deepEqual(await cache.keys(), []);
  const restarted = worker(cache, async () => new Response("ordinary origin")), read = restarted.fetch();
  assert.equal(await (await read.response).text(), "ordinary origin"); await read.done();
  assert.equal(vm.runInContext("runtimeConfig.version", restarted.context), "bootstrap");
});

test("restart restoration leaves auth, other origins, POST and explicit scene reload outside the worker", () => {
  const noStorage = { open() { assert.fail("unrelated traffic must not restore configuration"); } };
  const w = worker(noStorage, async () => new Response("ordinary origin"));
  for (const [requestPath, options] of [["/health"], ["/api/auth/session"], ["https://other.example.test/original-ui/1.png"], [path, { method: "POST" }], ["/api/scene/crystal?map=0", { cache: "reload" }]]) {
    const read = w.fetch(requestPath, options); assert.equal(read.intercepted, false);
  }
});

test("simultaneous cold requests share one configuration read", async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  const warm = first.fetch(); await warm.response; await warm.done();
  const open = cache.open.bind(cache); let configReads = 0;
  cache.open = async name => { const store = await open(name); return !name.includes("configuration") ? store : {
    ...store, async match(request) { configReads += 1; return store.match(request); },
  }; };
  const restarted = worker(cache, () => assert.fail("all three requests are already cached"));
  const reads = [restarted.fetch(), restarted.fetch(), restarted.fetch()];
  assert.deepEqual(await Promise.all(reads.map(async r => (await r.response).status)), [200, 200, 200]);
  await Promise.all(reads.map(r => r.done())); assert.equal(configReads, 1);
});

test("a stalled metadata read has a bounded ordinary-origin fallback", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const cache = { open() { return new Promise(() => {}); } };
  const restarted = worker(cache, async () => new Response("ordinary origin")), read = restarted.fetch();
  await flush(); assert.equal(restarted.calls.length, 0);
  t.mock.timers.tick(749); await flush(); assert.equal(restarted.calls.length, 0);
  t.mock.timers.tick(1);
  assert.equal(await (await read.response).text(), "ordinary origin"); await read.done();
  assert.deepEqual(restarted.calls, [origin + path]);
});

test("reset interrupts an old restore and its later result cannot revive configuration", { timeout: 2000 }, async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  const open = cache.open.bind(cache), gate = deferred(); let restoring = false;
  cache.open = async name => { const store = await open(name); return !name.includes("configuration") ? store : {
    ...store, async match(request) { const old = await store.match(request); restoring = true; await gate.promise; return old; },
  }; };
  const restarted = worker(cache, async () => new Response("ordinary origin")), read = restarted.fetch();
  await flush(); assert(restoring);
  const reset = restarted.message({ type: "MIR2_ASSET_CACHE_RESET" }); await reset.done;
  assert.equal(await (await read.response).text(), "ordinary origin"); await read.done();
  gate.resolve(); await flush();
  assert.equal(vm.runInContext("runtimeConfig.version", restarted.context), "bootstrap");
  assert(![...cache.stores.values()].some(store => store.entries.size));
});

test("STATUS wakes and restores a worker before reporting its namespace and canonical tiers", async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  const restarted = worker(cache), status = restarted.message({ type: "MIR2_ASSET_CACHE_STATUS" }); await status.done;
  const reply = status.replies.find(x => x.type === "MIR2_ASSET_CACHE_STATUS");
  assert.equal(reply.version, "namespace-a"); assert.equal(reply.staticAssetTiers.critical, 1);
});

test("a batched dynamic tier HINT hydrates first and survives the next restart", async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  const dynamic = "/original-map/forest/2.png", restarted = worker(cache);
  const hint = restarted.message({ type: "MIR2_ASSET_CACHE_HINT", urls: [dynamic, dynamic], cacheTier: "background" }); await hint.done;
  const reply = hint.replies.find(x => x.type === "MIR2_ASSET_CACHE_HINTED");
  assert.equal(reply.added, 1); assert.equal(reply.configurationPersisted, true);
  assert.equal(reply.staticAssetTiers.critical, 1); assert.equal(reply.staticAssetTiers.background, 1);
  const again = worker(cache), read = again.fetch(dynamic); assert.equal((await read.response).status, 200); await read.done();
  assert.equal(vm.runInContext("staticAssetTiers.get('/original-map/forest/2.png')", again.context), "background");
  assert((await cache.keys()).some(name => name.includes("static-background-namespace-a")));
});

test("an old HINT waiting for disk cannot persist or alter tiers after reset", { timeout: 2000 }, async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  const open = cache.open.bind(cache), gate = deferred(); let restoring = false;
  cache.open = async name => { const store = await open(name); return !name.includes("configuration") ? store : {
    ...store, async match(request) { const old = await store.match(request); restoring = true; await gate.promise; return old; },
  }; };
  const restarted = worker(cache), hint = restarted.message({ type: "MIR2_ASSET_CACHE_HINT", urls: ["/original-map/forest/2.png"], cacheTier: "background" });
  await flush(); assert(restoring);
  await restarted.message({ type: "MIR2_ASSET_CACHE_RESET" }).done;
  await hint.done; gate.resolve(); await flush();
  assert.equal(vm.runInContext("staticAssetTiers.size", restarted.context), 0);
  assert(![...cache.stores.values()].some(store => store.entries.size));
});

test("a restarted cold image responds before its background cache write while retaining the write lifetime", { timeout: 2000 }, async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  const open = cache.open.bind(cache), gate = deferred(); let writing = false;
  cache.open = async name => {
    const store = await open(name); return !name.includes("static-") ? store : {
      ...store, async put(request, response) { writing = true; await gate.promise; return store.put(request, response); },
    };
  };
  const restarted = worker(cache), read = restarted.fetch();
  try {
    const response = await read.response; assert.equal(response.status, 200);
    assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], bytes); assert(writing);
    let cached = false; const completion = read.done().then(() => { cached = true; });
    await flush(); assert.equal(cached, false, "the event lifetime must retain the unresolved write");
    gate.resolve(); await completion;
    assert.equal(cached, true);
  } finally { gate.resolve(); }
});

test("a new HINT after live reset cannot recreate durable configuration until a fresh CONFIG", async () => {
  const cache = storage(), first = worker(cache); await configure(first, "a");
  await first.message({ type: "MIR2_ASSET_CACHE_RESET" }).done;
  assert.deepEqual(await cache.keys(), []);
  const dynamic = "/original-map/forest/3.png";
  const hint = first.message({ type: "MIR2_ASSET_CACHE_HINT", urls: [dynamic], cacheTier: "background" }); await hint.done;
  assert.equal(vm.runInContext("runtimeConfig.version", first.context), "namespace-a", "live reset retains the existing network route");
  assert.equal(hint.replies[0].added, 1); assert.notEqual(hint.replies[0].configurationPersisted, true);
  assert(![...cache.stores.values()].some(store => store.entries.size));
  const restarted = worker(cache, async () => new Response("ordinary origin")), read = restarted.fetch();
  assert.equal(await (await read.response).text(), "ordinary origin"); await read.done();
  assert.equal(vm.runInContext("runtimeConfig.version", restarted.context), "bootstrap");
  await configure(first, "b");
  const newHint = first.message({ type: "MIR2_ASSET_CACHE_HINT", urls: [dynamic], cacheTier: "background" }); await newHint.done;
  assert.equal(newHint.replies[0].configurationPersisted, true);
  const again = worker(cache), againRead = again.fetch(dynamic); assert.equal((await againRead.response).status, 200); await againRead.done();
  assert.equal(vm.runInContext("runtimeConfig.version", again.context), "namespace-b");
  assert.equal(vm.runInContext("staticAssetTiers.get('/original-map/forest/3.png')", again.context), "background");
});
