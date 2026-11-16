import assert from "node:assert/strict";
import { createHash, webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const source = readFileSync(process.env.MIR2_ASSET_WORKER_TEST_SOURCE
  ?? new URL("../public/mir2-asset-worker.js", import.meta.url), "utf8");
const sha = value => createHash("sha256").update(value).digest("hex");
const families = [
  { file: "NotoSansCJKsc-Regular.otf", size: 16_437_364,
    digest: "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b", type: "font/otf", count: 63 },
  { file: "NotoSansTC.ttf", size: 11_941_968,
    digest: "864727d210d54f2537bbe23b3a839436c3992af72de9322af5270897246bd44f", type: "font/ttf", count: 46 },
].map(font => {
  const path = `/original-ui/fonts/${font.file}`;
  const bytes = readFileSync(new URL(`../public${path}`, import.meta.url));
  assert.equal(bytes.length, font.size); assert.equal(sha(bytes), font.digest);
  return { ...font, path, bytes };
});
const [sc, tc] = families;
const origin = "https://cjk.game.test", mirror = "https://cdn.game.test/mir2/v/current";
const turn = () => new Promise(resolve => setImmediate(resolve));
async function waitFor(predicate) {
  for (let i = 0; i < 4000 && !predicate(); i++) await turn();
  assert(predicate(), "expected font boundary not reached");
}
const fontOf = request => families.find(font => new URL(request.url).pathname.endsWith(font.path));
function rangeResponse(request) {
  const font = fontOf(request), match = /^bytes=(\d+)-(\d+)$/.exec(request.headers.get("range") ?? "");
  if (!font) return new Response(new Uint8Array([1, 2, 3]));
  if (!match) return new Response(font.bytes);
  const start = Number(match[1]), end = Number(match[2]);
  assert(start >= 0 && end < font.size && end - start < 1_048_576);
  return new Response(font.bytes.subarray(start, end + 1), { status: 206, headers: {
    "content-length": String(end - start + 1), "content-range": `bytes ${start}-${end}/${font.size}`,
  } });
}
function fixture(fetchImpl = rangeResponse, stores = new Map()) {
  const calls = [], writes = [];
  const key = value => typeof value === "string" ? value : value.url;
  const caches = {
    async open(name) {
      if (!stores.has(name)) stores.set(name, new Map());
      const rows = stores.get(name);
      return {
        async match(request) { return rows.get(key(request))?.clone(); },
        async put(request, response) { rows.set(key(request), new Response(await response.arrayBuffer(), response)); },
        async delete(request) { return rows.delete(key(request)); },
        async keys() { return [...rows.keys()].map(url => new Request(url)); },
      };
    },
    async keys() { return [...stores.keys()]; },
    async delete(name) { return stores.delete(name); },
  };
  const context = vm.createContext({ AbortController, AbortSignal, Request, Response, Headers, URL,
    Map, Set, Uint8Array, crypto: webcrypto, setTimeout, clearTimeout, console, caches,
    fetch: async request => { calls.push(request); return fetchImpl(request); },
    self: { location: { origin }, addEventListener() {}, clients: { async claim() {} }, async skipWaiting() {} },
  });
  vm.runInContext(source, context);
  vm.runInContext(`runtimeConfig = { ...runtimeConfig, version:'cjk-test', assetVersion:'current',
    remoteAssetBaseUrl:'${mirror}', remoteAssetBaseUrls:['${mirror}'], fontMirrorFirst:false }`, context);
  context.retain = promise => writes.push(promise);
  const full = font => stores.get("mir2-asset-cache-sw8-static-critical-cjk-test")?.get(origin + font.path);
  const parts = font => stores.get(`mir2-asset-cache-sw8-font-parts-${font.digest.slice(0, 16)}-cjk-test`);
  const read = font => vm.runInContext(`cacheFirst(new Request('${origin}${font.path}'),
    cacheName('static-critical'),3000,{waitUntil:retain})`, context);
  return { context, calls, stores, read, full, parts, flush: () => Promise.all(writes) };
}
async function assertFont(response, font) {
  assert.equal(response.status, 200); assert.equal(response.headers.get("content-range"), null);
  const bytes = new Uint8Array(await response.arrayBuffer());
  assert.equal(bytes.length, font.size); assert.equal(sha(bytes), font.digest);
}
function requestedParts(calls) {
  return calls.flatMap(request => {
    const match = /^bytes=(\d+)-(\d+)$/.exec(request.headers.get("range") ?? "");
    if (!match) return [];
    const a = Number(match[1]) / 262_144, b = Math.floor(Number(match[2]) / 262_144);
    return Array.from({ length: b - a + 1 }, (_, n) => a + n);
  }).toSorted((a, b) => a - b);
}
function heldRanges(t) {
  const held = [];
  const f = fixture(request => {
    const response = rangeResponse(request);
    if (!request.headers.has("range")) return response;
    const font = fontOf(request), [, a, b] = /^bytes=(\d+)-(\d+)$/.exec(request.headers.get("range"));
    let row;
    return new Response(new ReadableStream({ start(controller) {
      row = { cancelled: false, done: false, finish() {
        if (row.done || row.cancelled) return;
        row.done = true; controller.enqueue(font.bytes.subarray(Number(a), Number(b) + 1)); controller.close();
      } };
      held.push(row);
    }, cancel() { row.cancelled = true; } }), response);
  });
  // Cleanup is independent of whether an assertion fails against old code.
  let pending;
  t.after(async () => { held.forEach(row => row.finish()); await pending?.catch(() => {}); await f.flush(); });
  return { ...f, held, pending(value) { pending = value; return value; },
    waiting: () => held.filter(row => !row.done && !row.cancelled) };
}

test("TC cold delivery uses 46 original pinned restart units and a complete SHA-verified font", async () => {
  const f = fixture(), response = await f.read(tc);
  assert.equal(response.headers.get("content-type"), tc.type);
  await assertFont(response, tc); await f.flush();
  assert.equal(f.calls.length, 12); assert(f.calls.every(request => request.headers.has("range")));
  assert.deepEqual(requestedParts(f.calls), Array.from({ length: tc.count }, (_, n) => n));
  assert.equal(f.parts(tc).size, tc.count); assert(f.full(tc));
});

test("a broken TC whole body preserves verified pieces and a restarted Worker fetches only its missing tail", async () => {
  const length = 40 * 262_144 + 17_391;
  const f = fixture(() => {
    let emitted = false;
    return new Response(new ReadableStream({ pull(controller) {
      if (!emitted) { emitted = true; controller.enqueue(tc.bytes.subarray(0, length)); }
      else controller.error(new TypeError("interrupted TC body"));
    } }));
  });
  const failed = await vm.runInContext(`readCompleteFont(new Request('${origin}${tc.path}'),
    new AbortController().signal,8000,false)`, f.context);
  assert.equal(failed, null); assert.equal(f.full(tc), undefined);
  assert.equal(f.parts(tc)?.size, 40);
  const restarted = fixture(rangeResponse, f.stores);
  await assertFont(await restarted.read(tc), tc); await restarted.flush();
  assert.deepEqual(requestedParts(restarted.calls), [40, 41, 42, 43, 44, 45]);
});

for (const font of families) test(`${font.file}: verified progress can finish past 24s without discarding into a whole-body fallback`, async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = heldRanges(t), pending = f.pending(f.read(font));
  const batches = Math.ceil(Math.ceil(font.count / 4) / 2);
  const stepMs = font === sc ? 4000 : 6000;
  for (let n = 0; n < batches; n++) {
    await waitFor(() => f.waiting().length === 2);
    t.mock.timers.tick(stepMs); await turn();
    assert(f.calls.every(request => request.headers.has("range")), "continuous verified progress must not restart the full body at 24s");
    f.waiting().forEach(row => row.finish());
  }
  await assertFont(await pending, font); await f.flush();
  assert(f.calls.every(request => request.headers.has("range")));
  assert.equal(f.parts(font).size, font.count);
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
});

test("continuous progress still has a 48s absolute range bound and preserves completed pieces before the pinned fallback", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = heldRanges(t), pending = f.pending(f.read(sc));
  for (let n = 0; n < 6; n++) {
    await waitFor(() => f.waiting().length === 2);
    t.mock.timers.tick(7000); await turn();
    assert(f.calls.every(request => request.headers.has("range")));
    f.waiting().forEach(row => row.finish());
  }
  await waitFor(() => f.waiting().length === 2);
  t.mock.timers.tick(5999); await turn(); assert(f.calls.every(request => request.headers.has("range")));
  t.mock.timers.tick(1); await waitFor(() => f.calls.some(request => !request.headers.has("range")));
  await assertFont(await pending, sc); await f.flush();
  assert.equal(f.calls.find(request => !request.headers.has("range")).url, origin + sc.path);
  assert.equal(f.parts(sc).size, 48);
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
  assert.equal(vm.runInContext("fontDownloadQueue.length", f.context), 0);
  const calls = f.calls.length; t.mock.timers.tick(180_000); await turn(); assert.equal(f.calls.length, calls);
});

test("both complete CJK families share the original two lanes, retain separate pins, and do not block images", async () => {
  let active = 0, peak = 0;
  const f = fixture(async request => {
    active++; peak = Math.max(peak, active); await turn(); active--; return rangeResponse(request);
  });
  const a = f.read(sc), b = f.read(tc);
  await waitFor(() => f.calls.length >= 2);
  const image = await vm.runInContext(`fetchStaticAsset(new Request('${origin}/original-ui/Prguse/1.png'))`, f.context);
  assert.equal(image.status, 200); await image.arrayBuffer();
  await assertFont(await a, sc); await assertFont(await b, tc); await f.flush();
  assert(peak <= 3, "at most two fonts plus the independent image");
  assert.equal(f.parts(sc).size, sc.count); assert.equal(f.parts(tc).size, tc.count);
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
});

test("a corrupt TC complete cache is discarded and valid pinned pieces rebuild the original without redownloading", async () => {
  const f = fixture(); await assertFont(await f.read(tc), tc); await f.flush();
  const corrupt = Buffer.from(tc.bytes); corrupt[456_789] ^= 1;
  f.stores.get("mir2-asset-cache-sw8-static-critical-cjk-test").set(origin + tc.path, new Response(corrupt));
  const restarted = fixture(rangeResponse, f.stores);
  await assertFont(await restarted.read(tc), tc); await restarted.flush(); assert.equal(restarted.calls.length, 0);
});

test("TC Range-ignorant sources still require the original complete digest and reject corruption", async () => {
  for (const corrupt of [false, true]) {
    const bytes = Buffer.from(tc.bytes); if (corrupt) bytes[123_456] ^= 1;
    const f = fixture(() => new Response(bytes)), response = await f.read(tc);
    if (corrupt) {
      assert.notEqual(response.status, 200); await response.arrayBuffer(); assert.equal(f.full(tc), undefined);
    } else await assertFont(response, tc);
    await f.flush();
  }
});
