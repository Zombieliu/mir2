import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  const key = process.argv[i], value = process.argv[i + 1];
  assert(["--output", "--worker-source", "--chrome", "--playwright-package", "--failure-mode", "--font"].includes(key) && value && !args.has(key)); args.set(key, value);
}
assert(args.has("--output"));
const failureMode = args.get("--failure-mode") ?? "range-stall";
assert(["range-stall", "whole-prefix", "progressing", "chunk-progress", "header-failover"].includes(failureMode));
const family = args.get("--font") ?? "sc"; assert(["sc", "tc"].includes(family));
const web = resolve(dirname(fileURLToPath(import.meta.url)), ".."), output = resolve(args.get("--output"));
await mkdir(output, { recursive: false });
const require = createRequire(args.has("--playwright-package") ? resolve(args.get("--playwright-package")) : import.meta.url);
const { chromium } = require("playwright");
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const worker = await readFile(args.has("--worker-source") ? resolve(args.get("--worker-source")) : join(web, "public/mir2-asset-worker.js"));
const paths = [...(await readFile(join(web, "lib/bevy-runtime-font-prewarm.ts"), "utf8")).matchAll(/"(\/original-ui\/fonts\/[^"\n]+\.(?:ttf|otf))"/g)].map(row => row[1]);
assert.equal(new Set(paths).size, 6);
const fonts = new Map(await Promise.all(paths.map(async path => [path, await readFile(join(web, "public", path.slice(1)))])));
const sc = paths[0]; assert(sc.endsWith("NotoSansCJKsc-Regular.otf"));
const target = family === "sc" ? sc : paths.find(path => path.endsWith("NotoSansTC.ttf")); assert(target);
const targetParts = Math.ceil(fonts.get(target).length / 262144), prefixParts = targetParts - 3;
const proof = { kind: failureMode === "whole-prefix" ? "actual-browser-whole-font-prefix-restart" : "actual-edge-font-range-restart",
  failureMode, family, target, targetParts, workerSha256: sha(worker), noCachePrefill: true,
  fontPins: [...fonts].map(([path, bytes]) => ({ path, bytes: bytes.length, sha256: sha(bytes) })), requests: [], browserClosures: [], passed: false };
const sockets = new Set(); let app, assets, context, phase = "partial", active = 0, peak = 0, activeBodies = 0, peakBodies = 0;
const serve = (request, response) => {
  const pathname = new URL(request.url, "http://127.0.0.1").pathname;
  response.setHeader("cache-control", "no-store"); response.setHeader("access-control-allow-origin", "*");
  response.setHeader("access-control-expose-headers", "content-range,content-length,content-encoding");
  if (pathname === "/mir2-asset-worker.js") { response.writeHead(200, { "content-type": "text/javascript", "service-worker-allowed": "/" }); response.end(worker); return; }
  if (pathname === "/") { response.writeHead(200, { "content-type": "text/html" }); response.end('<!doctype html><title>Original font resume</title><canvas width="960" height="360"></canvas>'); return; }
  const logical = pathname.replace(/^\/(?:hotlink-ok\/)?mir2\/v\/current/, "");
  const original = fonts.get(logical);
  if (!original) { response.writeHead(404); response.end(); return; }
  const range = /^bytes=(\d+)-(\d+)$/.exec(request.headers.range ?? "");
  const offset = range ? Number(range[1]) : 0, end = range ? Number(range[2]) : original.length - 1;
  assert(offset >= 0 && end < original.length);
  const row = { phase, path: pathname, logical, range: request.headers.range ?? null, offset, bytes: end - offset + 1, at: Date.now() };
  proof.requests.push(row); active++; peak = Math.max(peak, active);
  response.once("close", () => { active--; row.closed = Date.now(); });
  if (failureMode === "whole-prefix" && phase === "partial" && logical === target
    && (range || request.socket.localPort !== app.address().port)) {
    response.writeHead(503); response.end(); return;
  }
  if (failureMode === "range-stall" && phase === "partial" && logical === target && !range) {
    // Isolate restart recovery: an available whole-font fallback would make
    // the first logical request succeed instead of leaving missing pieces.
    row.injectedWholeFallbackUnavailable = true; response.writeHead(504); response.end(); return;
  }
  if (failureMode === "header-failover" && phase === "partial" && logical === target
    && request.socket.localPort !== app.address().port) {
    row.injectedHeaderStall = true; return;
  }
  activeBodies++; peakBodies = Math.max(peakBodies, activeBodies);
  response.once("close", () => { activeBodies--; });
  response.writeHead(range ? 206 : 200, { "content-type": logical.endsWith(".otf") ? "font/otf" : "font/ttf",
    "content-length": end - offset + 1, ...(range ? { "content-range": `bytes ${offset}-${end}/${original.length}` } : {}) });
  if (failureMode === "whole-prefix" && phase === "partial" && logical === target) {
    row.injectedWholeBodyInterrupt = true; row.sentPrefixBytes = prefixParts * 262144 + 12345;
    response.write(original.subarray(0, row.sentPrefixBytes), () => setTimeout(() => response.destroy(), 150)); return;
  }
  if (failureMode === "range-stall" && phase === "partial" && logical === target && offset === 4 * 262144) {
    row.injectedBodyStall = true; response.write(original.subarray(offset, offset + 8192)); return;
  }
  if (failureMode === "progressing" && phase === "partial" && logical === target && range) {
    row.injectedProgressDelayMs = family === "sc" ? 3600 : 5000;
    response.flushHeaders();
    const timer = setTimeout(() => response.end(original.subarray(offset, end + 1)), row.injectedProgressDelayMs);
    response.once("close", () => clearTimeout(timer)); return;
  }
  if (failureMode === "chunk-progress" && phase === "partial" && logical === target && range && offset === 0) {
    row.injectedChunkIntervalMs = 4000; response.flushHeaders(); let sent = 0;
    const timer = setInterval(() => {
      const next = Math.min(sent + 262144, end + 1); response.write(original.subarray(sent, next)); sent = next;
      if (sent === end + 1) { clearInterval(timer); response.end(); }
    }, 4000);
    response.once("close", () => clearInterval(timer)); return;
  }
  response.end(original.subarray(offset, end + 1));
};
const listen = async () => {
  const server = createServer(serve); server.on("connection", socket => { sockets.add(socket); socket.once("close", () => sockets.delete(socket)); });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve)); return server;
};
const launch = async url => {
  context = await chromium.launchPersistentContext(join(output, "profile"), { headless: true, viewport: { width: 1000, height: 500 },
    ...(args.has("--chrome") ? { executablePath: resolve(args.get("--chrome")) } : {}) });
  const page = context.pages()[0] ?? await context.newPage(); await page.goto(url); return page;
};
const closeBrowser = async () => { if (context) { const owned = context; context = null; await owned.close(); proof.browserClosures.push(true); } };
try {
  app = await listen(); assets = await listen();
  const origin = `http://127.0.0.1:${app.address().port}`, remote = `http://127.0.0.1:${assets.address().port}`;
  let page = await launch(origin);
  await page.evaluate(async remote => {
    await navigator.serviceWorker.register('/mir2-asset-worker.js'); await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener('controllerchange', resolve, { once: true }));
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('configuration missing')), 5000);
      const listener = event => { if (event.data?.type === 'MIR2_ASSET_CACHE_CONFIGURED') {
        clearTimeout(timer); navigator.serviceWorker.removeEventListener('message', listener); resolve(); } };
      navigator.serviceWorker.addEventListener('message', listener);
      navigator.serviceWorker.controller.postMessage({ type: 'MIR2_ASSET_CACHE_CONFIG', manifestVersion: 'current', cacheNamespace: 'resume-test',
        manifest: { version: 'current', remoteAssets: { enabled: true, assetBaseUrl: remote + '/mir2/v/current', browserFallbackBaseUrls: [remote + '/hotlink-ok/mir2/v/current'] } } });
    });
  }, remote);
  proof.first = await page.evaluate(async target => {
    const began = performance.now(), response = await fetch(target), bytes = await response.arrayBuffer();
    const sha256 = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(x => x.toString(16).padStart(2,'0')).join('');
    const parts = []; for (const name of await caches.keys()) if (name.includes('font-parts-')) {
      const cache = await caches.open(name); for (const key of await cache.keys()) parts.push(key.url); }
    const whole = await caches.match(location.origin + target);
    return { status: response.status, bytes: bytes.byteLength, sha256, elapsedMs: performance.now() - began, parts, wholePresent: !!whole };
  }, target);
  if (failureMode === "whole-prefix") {
    assert([503,504].includes(proof.first.status));
    assert.equal(proof.first.parts.length, prefixParts, 'the interrupted whole body retains exactly its complete verified units');
    assert.equal(proof.requests.filter(r => r.injectedWholeBodyInterrupt).length, 1);
  } else if (failureMode === "range-stall") {
    assert.equal(proof.first.status, 504); assert(proof.first.parts.length > 0 && proof.first.parts.length < targetParts);
  } else {
    assert.equal(proof.first.status, 200);
    assert.equal(proof.first.bytes, fonts.get(target).length); assert.equal(proof.first.sha256, sha(fonts.get(target)));
    if (failureMode === "header-failover") {
      const attempts = proof.requests.filter(row => row.logical === target);
      assert.equal(attempts.filter(row => row.range).length, 4, 'each of two failed sources can own only its two initially admitted ranges');
      assert.equal(attempts.filter(row => !row.range).length, 1, 'one independent complete app read');
      assert(proof.first.elapsedMs >= 16000 && proof.first.elapsedMs < 22000);
    } else {
      assert.equal(proof.first.parts.length, targetParts);
      assert(proof.first.elapsedMs > (failureMode === "chunk-progress" ? 15000 : 24000)
        && proof.first.elapsedMs < (failureMode === "chunk-progress" ? 24000 : 48000));
      assert(proof.requests.filter(row => row.logical === target).every(row => row.range), 'progress must not be discarded for a whole-body fallback');
      if (failureMode === "chunk-progress") assert.equal(proof.requests.filter(row => row.logical === target && row.offset === 0).length, 1);
    }
    assert(proof.first.wholePresent);
  }
  if (["range-stall", "whole-prefix"].includes(failureMode)) assert(!proof.first.wholePresent);
  await closeBrowser(); phase = "resume";
  page = await launch(origin);
  // No configuration resend or cache population: restore the previous Worker configuration from its own persistent cache.
  proof.restoredWithoutConfigMessage = true;
  proof.second = await page.evaluate(async paths => {
    await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener('controllerchange', resolve, { once: true }));
    const digest = async bytes => [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(x => x.toString(16).padStart(2,'0')).join('');
    const rows = [];
    for (const [i, path] of paths.entries()) {
      const response = await fetch(path), bytes = await response.arrayBuffer(), face = new FontFace('Original' + i, bytes);
      await face.load(); document.fonts.add(face);
      rows.push({ path, status: response.status, bytes: bytes.byteLength, sha256: await digest(bytes), decoded: face.status === 'loaded' });
    }
    const cache = [];
    for (const path of paths) { const response = await caches.match(location.origin + path), bytes = response && await response.arrayBuffer();
      cache.push({ path, present: !!response, bytes: bytes?.byteLength ?? 0, sha256: bytes && await digest(bytes) }); }
    const canvas = document.querySelector('canvas'), ctx = canvas.getContext('2d');ctx.fillStyle='#15110b';ctx.fillRect(0,0,canvas.width,canvas.height);
    const samples=['比奇省 · 阿洛','繁體中文 · 冒險日記','Adventure with Alo','देवनागरी','ภาษาไทย','العربية'];
    for (let i=0;i<rows.length;i++) { ctx.fillStyle='#e9c77b';ctx.font=`25px Original${i}`;ctx.fillText(samples[i],22,48+i*51); }
    return { rows, cache };
  }, paths);
  for (const pin of proof.fontPins) {
    for (const row of [proof.second.rows.find(r => r.path === pin.path), proof.second.cache.find(r => r.path === pin.path)]) {
      assert.equal(row.bytes, pin.bytes); assert.equal(row.sha256, pin.sha256); }
  }
  const resumed = proof.requests.filter(r => r.phase === 'resume' && r.logical === target);
  assert(resumed.every(r => r.range));
  const savedOffsets = new Set(proof.first.parts.map(url => Number(url.split('/').at(-2))));
  const fetchedOffsets = resumed.flatMap(row => Array.from({length:Math.ceil(row.bytes/262144)},(_,n)=>row.offset+n*262144));
  assert.deepEqual(fetchedOffsets.toSorted((a,b)=>a-b),proof.first.wholePresent?[]:Array.from({length:targetParts},(_,n)=>n*262144).filter(offset=>!savedOffsets.has(offset)));
  proof.resumedRequests = resumed.length; proof.resumedPieces = fetchedOffsets.length;
  proof.resumedMissingOnly = true; proof.sixFontsComplete = proof.second.rows.every(r => r.status === 200 && r.decoded);
  // A cancelled header-only request can close on the server in the same event
  // turn as the fallback starts. Keep the raw lifecycle peak, and separately
  // enforce the two actual response-body readers in that failover fixture.
  proof.peakServerFontBodies = peak; proof.peakAdmittedFontBodies = peakBodies;
  proof.concurrencyScope = 'Raw server request lifetimes retained; header-failover body concurrency excludes cancelled requests that never sent headers or bytes.';
  assert(peakBodies <= 2); if (failureMode !== "header-failover") assert(peak <= 2);
  assert(proof.sixFontsComplete);
  await page.screenshot({ path: join(output, 'six-original-fonts-after-restart.png') }); proof.passed = true;
} catch (error) { proof.failure = { name: error.name, message: error.message }; process.exitCode = 1; }
finally {
  await closeBrowser(); for (const socket of sockets) socket.destroy();
  for (const server of [app, assets]) if (server) await new Promise(resolve => server.close(resolve));
  proof.serversClosed = true; await writeFile(join(output, 'receipt.json'), JSON.stringify(proof,null,2)+'\n', { flag: 'wx' });
}
console.log(JSON.stringify({passed:proof.passed,firstSaved:proof.first?.parts.length,resumedMissingOnly:proof.resumedMissingOnly,
 sixFontsComplete:proof.sixFontsComplete,browserClosures:proof.browserClosures,serversClosed:proof.serversClosed,failure:proof.failure??null}));
