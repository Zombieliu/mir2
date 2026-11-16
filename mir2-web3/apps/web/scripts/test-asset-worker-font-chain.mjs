import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import test from "node:test";

const source = readFileSync(process.env.MIR2_ASSET_WORKER_TEST_SOURCE
  ?? new URL("../public/mir2-asset-worker.js", import.meta.url), "utf8");
const turn = () => new Promise(resolve => setImmediate(resolve));
const font = name => `/original-ui/fonts/${name}`;
const origin = "https://game.example.test";
const primary = "https://assets.example.test/mir2/v/current";
const mirror = "https://assets.example.test/hotlink-ok/mir2/v/current";
const sc = font("NotoSansCJKsc-Regular.otf"), tc = font("NotoSansTC.ttf");

function fixture(t, fetchImpl) {
  const calls = [], held = [], pending = [], writes = [], stores = new Map();
  const body = bytes => new ReadableStream({ start(controller) {
    if (bytes) controller.enqueue(new Uint8Array(bytes));
    held.push(controller);
  } });
  const context = vm.createContext({
    AbortController, AbortSignal, Request, Response, Headers, URL, Map, Set,
    setTimeout, clearTimeout, console,
    fetch: async request => { calls.push(request); return fetchImpl(request, body); },
    caches: { async open() { return {
      async match(request) { return stores.get(request.url)?.clone(); },
      async put(request, response) { stores.set(request.url, new Response(await response.arrayBuffer(), { headers: response.headers })); },
      async keys() { return []; },
    }; } },
    self: { location: { origin }, addEventListener() {}, clients: { async claim() {} }, async skipWaiting() {} },
  });
  vm.runInContext(source, context, { filename: "mir2-asset-worker.js" });
  context.config = { version: "current", assetVersion: "current", fontMirrorFirst: false,
    remoteAssetBaseUrl: primary, remoteAssetBaseUrls: [mirror, "https://assets.example.test/mir2/v/older", primary] };
  context.retainWrite = promise => writes.push(promise);
  vm.runInContext("runtimeConfig = { ...runtimeConfig, ...config }", context);
  const read = (path, signal) => {
    context.request = new Request(origin + path, signal ? { signal } : undefined);
    const result = vm.runInContext('cacheFirst(request, "font-chain-test", 20, { waitUntil: retainWrite })', context);
    pending.push(result.then(async response => ({ status: response.status, bytes: [...new Uint8Array(await response.arrayBuffer())] })));
    return pending.at(-1);
  };
  t.after(async () => {
    for (const controller of held) { try { controller.close(); } catch {} }
    await Promise.all(pending); await Promise.all(writes);
  });
  return { context, calls, held, pending, stores, writes, read };
}

for (const failure of ["headers", "body"]) test(`a ${failure} failure continues the admitted SC font before four queued renderer families`, async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = fixture(t, async (request, body) => {
    if (request.url === primary + sc) {
      if (failure === "headers") return new Promise(() => {});
      return new Response(body([1, 2]));
    }
    if (request.url === origin + tc) return new Response(body([9]));
    return new Response(new Uint8Array([7, 8, 9]));
  });
  const scRead = f.read(sc), tcRead = f.read(tc);
  const small = ["NotoSans-Regular.ttf", "NotoSansArabic-Regular.ttf", "NotoSansThai-Regular.ttf", "NotoSansDevanagari-Regular.ttf"];
  const queued = small.map(name => f.read(font(name)));
  await turn();
  assert.deepEqual(f.calls.map(r => r.url), [primary + sc, origin + tc]);
  if (failure === "body") {
    t.mock.timers.tick(14_000); await turn();
    f.held[1].enqueue(new Uint8Array([10])); await turn();
    t.mock.timers.tick(1_000);
  } else t.mock.timers.tick(8_000);
  await turn();
  assert.equal(f.calls[2]?.url, mirror + sc, "a failed source must not send the admitted font to the tail of the renderer queue");
  assert.deepEqual(await scRead, { status: 200, bytes: [7, 8, 9] }, "a failed prefix cannot mix with the complete fallback");
  const scSources = f.calls.filter(r => r.url.endsWith(sc));
  assert.deepEqual(scSources.map(r => r.url), [primary + sc, mirror + sc]);
  assert(scSources[0].signal.aborted);
  f.held.at(-1).close();
  assert.equal((await tcRead).status, 200);
  assert((await Promise.all(queued)).every(r => r.status === 200));
  await Promise.all(f.writes);
  assert.deepEqual([...new Uint8Array(await f.stores.get(origin + sc).clone().arrayBuffer())], [7, 8, 9]);
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
});

test("successive mirrors retain two lanes through full bodies and still admit images", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = fixture(t, async (request, body) => {
    if (request.url === primary + sc) return new Promise(() => {});
    if (request.url.endsWith(sc) || request.url.endsWith(tc)) return new Response(body([1]));
    return new Response(new Uint8Array([4, 5]));
  });
  const a = f.read(sc), b = f.read(tc), next = f.read(font("NotoSansArabic-Regular.ttf"));
  await turn(); t.mock.timers.tick(8_000); await turn();
  assert.equal(f.calls[2]?.url, mirror + sc);
  assert.equal(f.calls.length, 3, "fallback headers do not admit a third logical font");
  f.context.image = new Request(origin + "/original-ui/Prguse/1.png");
  const image = await vm.runInContext("fetchStaticAsset(image)", f.context);
  assert.deepEqual([...new Uint8Array(await image.arrayBuffer())], [4, 5]);
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 2);
  f.held[1].enqueue(new Uint8Array([2])); await turn();
  assert.equal(f.calls.length, 4, "a partial body must retain its lane");
  f.held[1].close(); assert.deepEqual(await a, { status: 200, bytes: [1, 2] });
  await turn(); assert.equal(f.calls[4]?.url, origin + font("NotoSansArabic-Regular.ttf"));
  f.held[0].close(); await Promise.all([b, next]); await Promise.all(f.writes);
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
});

test("a queued direct whole-font abort releases no network work and cannot cancel live bodies", async t => {
  const f = fixture(t, async (_request, body) => new Response(body([8])));
  const a = f.read(sc), b = f.read(tc); await turn();
  const owner = new AbortController();
  f.context.cancelled = new Request(origin + font("Cancelled.ttf"), { signal: owner.signal });
  const queued = vm.runInContext("fetchStaticAsset(cancelled)", f.context);
  await turn(); assert.equal(f.calls.length, 2);
  owner.abort(); const response = await queued;
  assert.equal(response.status, 504); await response.arrayBuffer();
  assert.equal(f.calls.length, 2); assert(f.calls.every(request => !request.signal.aborted));
  f.held.forEach(c => c.close()); await Promise.all([a, b]); await Promise.all(f.writes);
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
  assert.equal(vm.runInContext("fontDownloadQueue.length", f.context), 0);
});

test("one exhausted logical font releases its slot and only confirmed absence is negatively cached", async t => {
  const f = fixture(t, async request => request.url.endsWith(sc)
    ? new Response(null, { status: 404 }) : new Response(new Uint8Array([3])));
  const rows = await Promise.all([f.read(sc), f.read(tc), f.read(font("NotoSansArabic-Regular.ttf"))]);
  assert.deepEqual(rows.map(r => r.status), [404, 200, 200]); await Promise.all(f.writes);
  const before = f.calls.length; assert.equal((await f.read(sc)).status, 404); assert.equal(f.calls.length, before);
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
  assert.equal(vm.runInContext("fontDownloadQueue.length", f.context), 0);
});
