import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

const source = readFileSync(process.env.MIR2_ASSET_WORKER_TEST_SOURCE
  ?? new URL("../public/mir2-asset-worker.js", import.meta.url), "utf8");
const origin = "https://game.example.test", mirror = "https://assets.example.test/current";
const path = n => `/original-ui/Prguse/${n}.png`;
const turn = () => new Promise(resolve => setImmediate(resolve));

function loadTs(relativePath, dependencies = {}) {
  const module = { exports: {} };
  const file = new URL(relativePath, import.meta.url);
  const compiled = ts.transpileModule(readFileSync(file, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("module", "exports", "require", compiled)(module, module.exports, name => {
    assert(Object.hasOwn(dependencies, name)); return dependencies[name];
  });
  return module.exports;
}
const { ASSET_CACHE_PACKS } = loadTs("../lib/asset-cache-packs.ts", {
  "./original-ui": loadTs("../lib/original-ui.ts"),
});
const lateHudPaths = [1, 4, 6, 12, 13, 8, 76, 2034, 826, 827, 828, 1960, 1961]
  .map(path).concat(["/original-ui/UI_32bit/473.png", "/original-ui/UI_32bit/472.png"]);

function fixture(t, implementation) {
  const calls = [], gates = [], pending = [], writes = [], stored = new Map(), listeners = new Map();
  const hold = request => new Promise((resolve, reject) => {
    const release = response => { request.signal.removeEventListener("abort", abort); resolve(response ?? new Response(new Uint8Array([1, 2, 3]))); };
    const abort = () => reject(request.signal.reason);
    request.signal.addEventListener("abort", abort, { once: true });
    gates.push({ request, release });
  });
  const context = vm.createContext({
    AbortController, AbortSignal, Request, Response, Headers, URL, Map, Set,
    setTimeout, clearTimeout, console,
    fetch: async request => { calls.push(request); return implementation(request, hold); },
    caches: { async keys() { return []; }, async delete() { return true; }, async open() { return {
      async match(request) { return stored.get(request.url)?.clone(); },
      async put(request, response) { stored.set(request.url, new Response(await response.arrayBuffer(), { headers: response.headers })); },
      async keys() { return []; },
    }; } },
    self: { location: { origin }, addEventListener(type, fn) { listeners.set(type, fn); }, clients: { async claim() {} }, async skipWaiting() {} },
  });
  vm.runInContext(source, context);
  const read = (n, options = {}) => {
    context.request = new Request(origin + (typeof n === "number" ? path(n) : n), options.signal ? { signal: options.signal } : undefined);
    context.retainWrite = p => writes.push(p);
    const p = vm.runInContext(options.cache
      ? 'cacheFirst(request, "static-admission", 500, { waitUntil: retainWrite })'
      : 'fetchStaticAsset(request)', context);
    pending.push(p); return p;
  };
  t.after(async () => {
    // Cleanup must also release work admitted after earlier headers finish.
    for (let round = 0; round < 20; round++) { gates.forEach(g => g.release()); await turn(); }
    await Promise.allSettled(pending); await Promise.allSettled(writes);
  });
  const configure = (packs = ASSET_CACHE_PACKS) => {
    const waits = [];
    listeners.get("message")({ data: { type: "MIR2_ASSET_CACHE_CONFIG", cacheNamespace: "static-admission", manifest: { version: "release", resourcePacks: packs } },
      source: { postMessage() {} }, waitUntil(value) { waits.push(value); } });
    return Promise.all(waits);
  };
  return { context, calls, gates, writes, stored, read, configure };
}

test("a renderer burst admits six logical images and starts queued deadlines only on dispatch", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const f = fixture(t, async (request, hold) => Number(new URL(request.url).pathname.match(/(\d+)\.png/)[1]) < 6
    ? hold(request) : new Response(new Uint8Array([8])));
  const requests = Array.from({ length: 40 }, (_, n) => f.read(n));
  await turn(); assert.equal(f.calls.length, 6, "queued reads must not already consume network/header budgets");
  t.mock.timers.tick(7000); await turn(); await turn();
  const results = await Promise.all(requests);
  assert(results.slice(0, 6).every(r => r.status === 504));
  assert(results.slice(6).every(r => r.status === 200), "waiting in admission cannot expire a later source timer");
  assert.equal(vm.runInContext("activeStaticHeaderFetches", f.context), 0);
});

test("queued critical HUD images precede runtime and background work, with FIFO within each tier", async t => {
  const f = fixture(t, (request, hold) => hold(request));
  Array.from({ length: 6 }, (_, n) => f.read(n)); await turn();
  vm.runInContext(`staticAssetTiers.set('${path(10)}', 'background'); staticAssetTiers.set('${path(11)}', 'critical'); staticAssetTiers.set('${path(12)}', 'critical')`, f.context);
  f.read(10); f.read(20); f.read(11); f.read(12); await turn();
  assert.equal(f.calls.length, 6);
  f.gates.slice(0, 4).forEach(g => g.release()); await turn();
  assert.deepEqual(f.calls.slice(6).map(r => new URL(r.url).pathname), [path(11), path(12), path(20), path(10)]);
});

test("a source fallback keeps its logical admission instead of rejoining a burst", async t => {
  const f = fixture(t, (request, hold) => request.url.startsWith(origin)
    ? Promise.resolve(new Response(null, { status: 404 })) : hold(request));
  vm.runInContext(`runtimeConfig = { ...runtimeConfig, remoteAssetBaseUrls: ['${mirror}'] }`, f.context);
  Array.from({ length: 20 }, (_, n) => f.read(n)); await turn();
  assert.equal(f.calls.filter(r => r.url.startsWith(origin)).length, 6);
  assert.equal(f.calls.filter(r => r.url.startsWith(mirror)).length, 6);
  assert.equal(vm.runInContext("activeStaticHeaderFetches", f.context), 6);
  f.gates[0].release(); await turn();
  assert.equal(f.calls.filter(r => r.url.startsWith(origin)).length, 7);
});

test("cancelling a queued direct image starts no fetch and leaves admitted readers alive", async t => {
  const f = fixture(t, (request, hold) => hold(request));
  Array.from({ length: 6 }, (_, n) => f.read(n)); await turn();
  const owner = new AbortController(), next = f.read(100, { signal: owner.signal });
  await turn(); owner.abort();
  assert.equal((await next).status, 504); assert.equal(f.calls.length, 6);
  assert(f.calls.every(r => !r.signal.aborted));
  assert.equal(vm.runInContext("staticHeaderQueue.length", f.context), 0);
});

test("coalesced image ownership, complete cache bodies and fonts stay independent of image admission", async t => {
  const f = fixture(t, (request, hold) => request.url.includes('/fonts/')
    ? Promise.resolve(new Response(new Uint8Array([7, 8, 9]))) : hold(request));
  const firstOwner = new AbortController();
  const first = f.read(0, { cache: true, signal: firstOwner.signal });
  const duplicate = f.read(0, { cache: true });
  Array.from({ length: 5 }, (_, n) => f.read(n + 1)); await turn();
  assert.equal(f.calls.length, 6); firstOwner.abort();
  assert(f.calls.every(r => !r.signal.aborted), "retiring a DOM image cannot abort its shared cache fill");
  const font = await f.read('/original-ui/fonts/NotoSans-Regular.ttf');
  assert.equal(font.status, 200); assert.equal((await font.arrayBuffer()).byteLength, 3);
  assert.equal(f.calls.length, 7, "a font does not wait for image headers");
  f.gates.find(g => new URL(g.request.url).pathname === path(0)).release();
  for (const response of await Promise.all([first, duplicate])) assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [1, 2, 3]);
  await Promise.all(f.writes);
  assert.deepEqual([...new Uint8Array(await f.stored.get(origin + path(0)).clone().arrayBuffer())], [1, 2, 3]);
  const cached = await f.read(0, { cache: true }); assert.equal(cached.status, 200); assert.equal(f.calls.length, 7);
});

test("a queued image retains its release sources across a later Worker configuration", async t => {
  const f = fixture(t, (request, hold) => request.url.startsWith(origin) && !request.url.endsWith(path(7))
    ? hold(request) : Promise.resolve(new Response(request.url.startsWith(origin) ? null : new Uint8Array([8]), { status: request.url.startsWith(origin) ? 404 : 200 })));
  vm.runInContext(`runtimeConfig = { ...runtimeConfig, remoteAssetBaseUrls: ['${mirror}'] }`, f.context);
  Array.from({ length: 6 }, (_, n) => f.read(n)); await turn();
  const queued = f.read(7); await turn();
  assert.equal(f.calls.length, 6);
  vm.runInContext(`runtimeConfig = { ...runtimeConfig, remoteAssetBaseUrls: ['https://assets.example.test/newer'] }`, f.context);
  f.gates[0].release(); assert.equal((await queued).status, 200);
  assert(f.calls.some(r => r.url === mirror + path(7)));
  assert(!f.calls.some(r => r.url.includes('/newer/')));
});

test("the real deferred HUD manifest admits all fifteen late required images ahead of ordinary scene work", async t => {
  const f = fixture(t, (request, hold) => hold(request));
  await f.configure();
  const hud = ASSET_CACHE_PACKS.find(p => p.name === "hud-core");
  assert.equal(hud.urls.length, 113); assert.equal(hud.phase, "background"); assert.equal(hud.cacheTier, "background");
  assert(lateHudPaths.every(p => hud.urls.includes(p)));
  Array.from({ length: 6 }, (_, n) => f.read(6000 + n)); await turn();
  Array.from({ length: 24 }, (_, n) => f.read(7000 + n));
  const hudReads = lateHudPaths.map(p => f.read(p, { cache: true })); await turn();
  for (let batch = 0; batch < 3; batch++) { f.gates.slice().forEach(g => g.release()); await turn(); }
  assert.deepEqual(f.calls.slice(6, 21).map(r => new URL(r.url).pathname), lateHudPaths,
    "a background cache partition cannot leave required HUD images behind the scene backlog");
  f.gates.slice().forEach(g => g.release()); await Promise.all(hudReads); await Promise.all(f.writes);
  assert(lateHudPaths.every(p => f.stored.has(origin + p)));
  f.context.request = new Request(origin + lateHudPaths[0]);
  assert.equal(vm.runInContext("staticAssetCachePolicy(request).cacheKind", f.context), "static-background",
    "download priority does not migrate or invalidate the published cache partitions");
});

test("a later HUD configuration promotes already queued reads without admitting more than six", async t => {
  const f = fixture(t, (request, hold) => hold(request));
  Array.from({ length: 6 }, (_, n) => f.read(6000 + n)); await turn();
  f.read(7000); f.read(lateHudPaths[0]); await turn();
  await f.configure(); assert.equal(f.calls.length, 6);
  f.gates[0].release(); await turn();
  assert.equal(new URL(f.calls[6].url).pathname, lateHudPaths[0]);
  assert.equal(vm.runInContext("activeStaticHeaderFetches", f.context), 6);
});
