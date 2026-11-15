import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash, webcrypto } from "node:crypto";
import vm from "node:vm";
import test from "node:test";

const source = readFileSync(process.env.MIR2_ASSET_WORKER_TEST_SOURCE ?? new URL("../public/mir2-asset-worker.js", import.meta.url), "utf8");
const path = "/original-ui/fonts/NotoSansCJKsc-Regular.otf";
const original = readFileSync(new URL(`../public${path}`, import.meta.url));
const digest = value => createHash("sha256").update(value).digest("hex");
const expected = "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b";
assert.equal(original.length, 16_437_364); assert.equal(digest(original), expected);
const origin = "https://range.game.test", primary = "https://cdn.game.test/mir2/v/current";
const mirror = "https://cdn.game.test/hotlink-ok/mir2/v/current";
const turn = () => new Promise(resolve => setImmediate(resolve));
const waitFor = async predicate => {
  for (let i = 0; i < 2000 && !predicate(); i++) await turn();
  assert(predicate(), 'expected request boundary not reached');
};
const sliceResponse = request => {
  const match = /^bytes=(\d+)-(\d+)$/.exec(request.headers.get("range") ?? "");
  if (!match) return new Response(original);
  const start = Number(match[1]), end = Number(match[2]);
  assert(start >= 0 && end < original.length && end - start < 1048576);
  return new Response(original.subarray(start, end + 1), { status: 206, headers: {
    "content-range": `bytes ${start}-${end}/${original.length}`, "content-length": String(end - start + 1),
  } });
};
const rangeIndex = request => Math.floor(Number(/^bytes=(\d+)/.exec(request.headers.get("range") ?? "")?.[1]) / 262144);
const requestedPieces = calls => calls.flatMap(request => {
  const [, start, end] = /^bytes=(\d+)-(\d+)$/.exec(request.headers.get('range'));
  return Array.from({length:Math.floor(Number(end)/262144)-Number(start)/262144+1},(_,n)=>Number(start)/262144+n);
});
const cachedPieces = f => new Set([...f.parts()[0][1].keys()].map(key => Number(new URL(key).pathname.split('/')[3])/262144));

function fixture(fetchImpl = sliceResponse, stores = new Map(), options = {}) {
  const calls = [], pending = [];
  const key = value => typeof value === "string" ? value : value.url;
  const caches = { async open(name) {
    if (options.denied) throw new Error("storage denied");
    if (!stores.has(name)) stores.set(name, new Map());
    const rows = stores.get(name);
    return {
      async match(request) { return rows.get(key(request))?.clone(); },
      async put(request, response) { rows.set(key(request), new Response(await response.arrayBuffer(), response)); },
      async delete(request) { return rows.delete(key(request)); },
      async keys() { return [...rows.keys()].map(url => new Request(url)); },
    };
  }, async keys() { return [...stores.keys()]; }, async delete(name) { return stores.delete(name); } };
  const context = vm.createContext({ AbortController, AbortSignal, Request, Response, Headers, URL,
    Map, Set, Uint8Array, crypto: webcrypto, setTimeout, clearTimeout, console, caches,
    fetch: async request => { calls.push(request); return fetchImpl(request); },
    self: { location: { origin }, addEventListener() {}, clients: { async claim() {} }, async skipWaiting() {} },
  });
  vm.runInContext(source, context);
  vm.runInContext(`runtimeConfig = { ...runtimeConfig, version:'font-range-test', assetVersion:'current',
    remoteAssetBaseUrl:'${primary}', remoteAssetBaseUrls:['${mirror}','${primary}'] }`, context);
  context.retain = promise => pending.push(promise);
  const read = (fontPath = path) => vm.runInContext(`cacheFirst(new Request('${origin}${fontPath}'),
    cacheName('static-critical'), 3000, {waitUntil:retain})`, context);
  return { context, calls, stores, read, flush: () => Promise.all(pending),
    parts: () => [...stores.entries()].filter(([name]) => name.includes('font-parts-')),
    full: () => stores.get('mir2-asset-cache-sw8-static-critical-font-range-test')?.get(origin + path),
  };
}
async function assertOriginal(response) {
  assert.equal(response.status, 200); assert.equal(response.headers.get("content-range"), null);
  const body = new Uint8Array(await response.arrayBuffer());
  assert.equal(body.length, original.length); assert.equal(digest(body), expected);
}

test("cold transfer coalesces pinned pieces into sixteen bounded requests while keeping 63 restart units", async () => {
  const f = fixture(); await assertOriginal(await f.read()); await f.flush();
  assert.equal(f.calls.length, 16);
  assert.equal(f.parts()[0][1].size, 63);
  assert(f.calls.every(request => {
    const [, start, end] = /^bytes=(\d+)-(\d+)$/.exec(request.headers.get('range'));
    return Number(end) - Number(start) + 1 <= 1048576;
  }));
  assert.equal(f.calls.map(request => Number(/^bytes=\d+-(\d+)$/.exec(request.headers.get('range'))[1]) -
    Number(/^bytes=(\d+)/.exec(request.headers.get('range'))[1]) + 1).reduce((a,b)=>a+b,0), original.length);
});

test("sparse restart holes fetch only missing original pieces and coalesce adjacent holes", async () => {
  const f = fixture(); await assertOriginal(await f.read()); await f.flush();
  const missing = new Set([3, 4, 6, 7, 8, 62]);
  const rows = f.parts()[0][1];
  for (const key of rows.keys()) if (missing.has(Number(new URL(key).pathname.split('/')[3]) / 262144)) rows.delete(key);
  assert.equal(rows.size, 57);
  f.stores.get('mir2-asset-cache-sw8-static-critical-font-range-test').clear();
  const restarted = fixture(sliceResponse, f.stores); await assertOriginal(await restarted.read()); await restarted.flush();
  const fetched=[];
  for (const request of restarted.calls) {
    const [, start, end] = /^bytes=(\d+)-(\d+)$/.exec(request.headers.get('range'));
    for(let n=Number(start)/262144;n<=Math.floor(Number(end)/262144);n++)fetched.push(n);
  }
  assert.deepEqual(fetched.toSorted((a,b)=>a-b), [...missing]);
  assert(restarted.calls.length < missing.size, 'adjacent missing pieces should share a request');
});

test("a source limited to single pieces downgrades coalescing within the original two global lanes", async () => {
  let peak=0,active=0;
  const f=fixture(async request=>{
    const match=/^bytes=(\d+)-(\d+)$/.exec(request.headers.get('range')??'');
    if(match && Number(match[2])-Number(match[1])+1>262144)return new Response(null,{status:416});
    active++;peak=Math.max(peak,active);await turn();active--;return sliceResponse(request);
  });
  await assertOriginal(await f.read()); await f.flush();
  assert(peak<=2);assert.equal(f.parts()[0][1].size,63);
  assert(f.calls.filter(request=>{
    const [,a,b]=/^bytes=(\d+)-(\d+)$/.exec(request.headers.get('range'));return Number(b)-Number(a)+1>262144;
  }).length<=4,'only the two initially admitted batches may try both sources');
});

test("a corrupted interior piece of a coalesced body is never cached under its original digest", async () => {
  const f=fixture(request=>{
    const match=/^bytes=(\d+)-(\d+)$/.exec(request.headers.get('range')??'');
    if(!match)return new Response(null,{status:503});
    const a=Number(match[1]),b=Number(match[2]),bytes=Buffer.from(original.subarray(a,b+1));
    if(a<=262144&&b>=262144)bytes[262144-a]^=1;
    return new Response(bytes,sliceResponse(request));
  });
  const response=await f.read();assert.notEqual(response.status,200);await response.arrayBuffer();await f.flush();
  assert.equal(f.full(),undefined);
  for(const [,rows]of f.parts())for(const [key,response]of rows){
    const offset=Number(new URL(key).pathname.split('/')[3]);assert.notEqual(offset,262144);
    assert.equal(digest(Buffer.from(await response.arrayBuffer())),digest(original.subarray(offset,Math.min(offset+262144,original.length))));
  }
});

test("all generated 256KiB parts and complete font are pinned to original bytes", async () => {
  const f = fixture(); await assertOriginal(await f.read()); await f.flush();
  assert.equal(f.calls.length, 16); assert(f.calls.every(r => r.headers.has("range")));
  assert.equal(f.parts()[0][1].size, 63); assert(f.full());
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
  const before = f.calls.length; await assertOriginal(await f.read()); assert.equal(f.calls.length, before);
});

test("simultaneous font consumers wait for the final part and share one complete cache fill", async () => {
  let release, reached;
  const arrived = new Promise(resolve => { reached = resolve; });
  const f = fixture(async request => {
    if (request.headers.get('range')?.endsWith(`-${original.length-1}`)) { reached(); await new Promise(resolve => { release = resolve; }); }
    return sliceResponse(request);
  });
  let done = false;
  const a = f.read().then(r => { done = true; return r; }), b = f.read();
  await arrived; assert.equal(done, false); assert.equal(f.full(), undefined);
  release(); await assertOriginal(await a); await assertOriginal(await b); await f.flush();
  assert.equal(f.calls.length, 16);
});

test("a failed body retains verified parts across a new Worker instance and downloads only missing parts", async () => {
  const stores = new Map();
  const f = fixture(request => {
    const response = sliceResponse(request);
    return rangeIndex(request) === 8 ? new Response(new Uint8Array([1, 2]), response) : response;
  }, stores);
  const failed = await f.read(); assert.equal(failed.status, 504); await failed.arrayBuffer(); await f.flush();
  const saved = f.parts()[0][1].size; assert(saved >= 3 && saved < 63); assert.equal(f.full(), undefined);
  const kept = cachedPieces(f), restarted = fixture(sliceResponse, stores);
  await assertOriginal(await restarted.read()); await restarted.flush();
  assert.deepEqual(requestedPieces(restarted.calls).toSorted((a,b)=>a-b),Array.from({length:63},(_,n)=>n).filter(n=>!kept.has(n)));
  assert(restarted.full());
});

test("a corrupt stored part is revalidated, evicted and fetched without redownloading valid parts", async () => {
  const f = fixture(); await assertOriginal(await f.read()); await f.flush();
  const rows = f.parts()[0][1], [key, response] = [...rows][7];
  const bytes = new Uint8Array(await response.arrayBuffer()); bytes[0] ^= 1;
  rows.set(key, new Response(bytes));
  f.stores.get('mir2-asset-cache-sw8-static-critical-font-range-test').clear();
  const restarted = fixture(sliceResponse, f.stores);
  await assertOriginal(await restarted.read()); await restarted.flush();
  assert.equal(restarted.calls.length, 1); assert.equal(rangeIndex(restarted.calls[0]), 7);
});

test("a poisoned whole-font cache cannot bypass the full digest", async () => {
  const f = fixture(); await assertOriginal(await f.read()); await f.flush();
  const corrupt = Buffer.from(original); corrupt[12345] ^= 1;
  f.stores.get('mir2-asset-cache-sw8-static-critical-font-range-test').set(origin + path, new Response(corrupt));
  const restarted = fixture(sliceResponse, f.stores);
  await assertOriginal(await restarted.read()); await restarted.flush(); assert.equal(restarted.calls.length, 0);
});

test("denied CacheStorage retains complete in-memory validation and two-reader admission", async () => {
  const f = fixture(sliceResponse, new Map(), { denied: true });
  await assertOriginal(await f.read()); assert.equal(f.calls.length, 16);
  assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
});

for (const broken of ["wrong-range", "wrong-length", "encoded", "short", "oversized", "corrupt"]) {
  test(`${broken} range bytes never become a whole font or a verified part`, async () => {
    const f = fixture(request => {
      if (!request.headers.has("range")) return new Response(null, { status: 503 });
      const response = sliceResponse(request), headers = new Headers(response.headers);
      const [, begin, end] = /^bytes=(\d+)-(\d+)$/.exec(request.headers.get('range'));
      let body = Buffer.from(original.subarray(Number(begin), Number(end)+1));
      if (broken === "wrong-range") headers.set("content-range", `bytes 0-NaN/${original.length}`);
      if (broken === "wrong-length") headers.set("content-length", "2");
      if (broken === "encoded") headers.set("content-encoding", "gzip");
      if (broken === "short") body = body.subarray(0, 4);
      if (broken === "oversized") body = Buffer.concat([body, Buffer.from([1])]);
      if (broken === "corrupt") body[4] ^= 1;
      return new Response(body, { status: 206, headers });
    });
    const response = await f.read(); assert.equal(response.status, 503, 'preserve the complete-source HTTP failure'); await response.arrayBuffer(); await f.flush();
    assert.equal(f.full(), undefined); assert(f.parts().every(([, rows]) => rows.size === 0));
    assert.equal(vm.runInContext("activeFontDownloads", f.context), 0);
  });
}

test("a source ignoring Range falls back to a full, SHA-verified original", async () => {
  const f = fixture(() => new Response(original));
  await assertOriginal(await f.read()); await f.flush();
  assert(f.calls.some(r => r.headers.has("range"))); assert(f.calls.some(r => !r.headers.has("range")));
  assert(f.parts().every(([, rows]) => rows.size === 0));
});

test("current Range outage can still use the healthy original app body", async () => {
  const f = fixture(request => request.headers.has("range") || !request.url.startsWith(origin)
    ? new Response(null, { status: 404 }) : new Response(original));
  await assertOriginal(await f.read()); await f.flush(); assert(f.calls.some(r => r.url === origin + path));
});

test("new configuration cannot redirect queued old parts or persist them into the new namespace", async () => {
  let unblock; const held = new Promise(resolve => { unblock = resolve; }); let first = true;
  const f = fixture(async request => { if (first) { first = false; await held; } return sliceResponse(request); });
  const pending = f.read(); await turn();
  vm.runInContext("noteConfigurationChange();runtimeConfig={...runtimeConfig,version:'new-release',assetVersion:'new',remoteAssetBaseUrl:'https://other.test/mir2/v/new',remoteAssetBaseUrls:[]}", f.context);
  unblock(); await assertOriginal(await pending); await f.flush();
  assert(f.calls.every(r => r.url.startsWith(primary) || r.url.startsWith(mirror)));
  assert(![...f.stores.keys()].some(name => name.endsWith('-new-release')));
});

test("two range readers share the original global lanes with another font; images remain independent", async () => {
  const held = []; let requested = 0;
  const f = fixture(request => {
    if (request.headers.has("range") && requested++ < 2 || request.url.endsWith('NotoSansTC.ttf')) {
      const response = request.headers.has("range") ? sliceResponse(request) : new Response(new Uint8Array([1, 2]));
      return new Response(new ReadableStream({ start(c) {
        response.arrayBuffer().then(body => { c.enqueue(new Uint8Array(body)); held.push(() => c.close()); });
      } }), response);
    }
    return request.headers.has('range') ? sliceResponse(request) : new Response(new Uint8Array([7]));
  });
  const sc = f.read(); while (held.length < 2) await turn();
  const tc = f.read('/original-ui/fonts/NotoSansTC.ttf'); await turn();
  assert.equal(f.calls.length, 2); held[0]();
  while (held.length < 3) await turn();
  assert(f.calls.some(r => r.url.endsWith('NotoSansTC.ttf')));
  assert.equal(vm.runInContext('activeFontDownloads', f.context), 2);
  const image = await vm.runInContext(`fetchStaticAsset(new Request('${origin}/original-ui/Prguse/1.png'))`, f.context);
  assert.equal(image.status, 200); held[1](); held[2]();
  await assertOriginal(await sc); assert.equal((await tc).status, 200); await f.flush();
});

test("a stalled partial body is cancelled, keeps completed parts and resumes after Worker restart", async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const stopped = [], signals = [];
  const f = fixture(request => {
    const response = sliceResponse(request);
    if (rangeIndex(request) !== 4) return response;
    signals.push(request.signal);
    return new Response(new ReadableStream({ start(c) { c.enqueue(original.subarray(4 * 262144, 4 * 262144 + 1024)); },
      cancel() { stopped.push(true); } }), response);
  });
  const pending = f.read(); await waitFor(() => signals.length === 1); await turn();
  t.mock.timers.tick(15000); await waitFor(() => signals.length === 2); await turn();
  t.mock.timers.tick(15000);
  await waitFor(() => signals.length === 3); await turn();
  t.mock.timers.tick(15000); await waitFor(() => signals.length === 4); await turn();
  t.mock.timers.tick(15000);
  const failed = await pending; assert.equal(failed.status, 504); await failed.arrayBuffer(); await f.flush();
  assert.equal(stopped.length, 4); assert(signals.every(signal => signal.aborted));
  const saved = f.parts()[0][1].size; assert(saved > 0 && saved < 63); assert.equal(f.full(), undefined);
  const kept = cachedPieces(f), restarted = fixture(sliceResponse, f.stores);
  await assertOriginal(await restarted.read()); await restarted.flush();
  assert.deepEqual(requestedPieces(restarted.calls).toSorted((a,b)=>a-b),Array.from({length:63},(_,n)=>n).filter(n=>!kept.has(n)));
});

test("a cancelled queued range font starts no transfer and does not cancel other font bodies", async () => {
  const close = [];
  const f = fixture(() => new Response(new ReadableStream({ start(c) { c.enqueue(new Uint8Array([1])); close.push(() => c.close()); } })));
  const a = f.read('/original-ui/fonts/NotoSansTC.ttf'), b = f.read('/original-ui/fonts/NotoSans-Regular.ttf');
  await waitFor(() => close.length === 2);
  const owner = new AbortController(); f.context.owned = new Request(origin + path, { signal: owner.signal });
  const pending = vm.runInContext('fetchStaticAsset(owned)', f.context);
  await turn(); owner.abort(); const response = await pending;
  assert.equal(response.status, 504); await response.arrayBuffer(); assert.equal(f.calls.length, 2);
  assert(f.calls.every(r => !r.signal.aborted)); close.forEach(finish => finish());
  await a; await b; await f.flush(); assert.equal(vm.runInContext('activeFontDownloads', f.context), 0);
});

test("an old partial 200 cache is discarded before rebuilding the original complete font", async () => {
  const stores = new Map([['mir2-asset-cache-sw8-static-critical-font-range-test',
    new Map([[origin + path, new Response(original.subarray(0, 100))]])]]);
  const f = fixture(sliceResponse, stores); await assertOriginal(await f.read()); await f.flush();
  assert.equal(f.calls.length, 16); assert.equal(Number(f.full().headers.get('content-length')), original.length);
});

test("ordinary incoming Range remains an ordinary 206 and never enters the resumable whole-font cache", async () => {
  const f = fixture(sliceResponse); f.context.ranged = new Request(origin + path, { headers: { Range: 'bytes=0-9' } });
  const response = await vm.runInContext('fetchStaticAsset(ranged)', f.context);
  assert.equal(response.status, 206); assert.equal((await response.arrayBuffer()).byteLength, 10);
  assert.equal(f.calls.length, 1); assert.equal(f.parts().length, 0); assert.equal(f.full(), undefined);
});
