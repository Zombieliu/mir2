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
  assert(["--output", "--worker-source", "--chrome", "--playwright-package"].includes(key) && value && !args.has(key)); args.set(key, value);
}
assert(args.has("--output"));
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
const proof = { kind: "actual-edge-font-range-restart", workerSha256: sha(worker), noCachePrefill: true,
  fontPins: [...fonts].map(([path, bytes]) => ({ path, bytes: bytes.length, sha256: sha(bytes) })), requests: [], browserClosures: [], passed: false };
const sockets = new Set(); let app, assets, context, phase = "partial", active = 0, peak = 0;
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
  response.writeHead(range ? 206 : 200, { "content-type": logical.endsWith(".otf") ? "font/otf" : "font/ttf",
    "content-length": end - offset + 1, ...(range ? { "content-range": `bytes ${offset}-${end}/${original.length}` } : {}) });
  if (phase === "partial" && logical === sc && offset === 4 * 262144) {
    row.injectedBodyStall = true; response.write(original.subarray(offset, offset + 8192)); return;
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
  proof.first = await page.evaluate(async sc => {
    const began = performance.now(), response = await fetch(sc); await response.arrayBuffer();
    const parts = []; for (const name of await caches.keys()) if (name.includes('font-parts-')) {
      const cache = await caches.open(name); for (const key of await cache.keys()) parts.push(key.url); }
    const whole = await caches.match(location.origin + sc);
    return { status: response.status, elapsedMs: performance.now() - began, parts, wholePresent: !!whole };
  }, sc);
  assert.equal(proof.first.status, 504); assert(!proof.first.wholePresent); assert(proof.first.parts.length > 0 && proof.first.parts.length < 63);
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
  const resumed = proof.requests.filter(r => r.phase === 'resume' && r.logical === sc);
  assert(resumed.every(r => r.range)); assert.equal(resumed.length, 63 - proof.first.parts.length);
  const savedOffsets = new Set(proof.first.parts.map(url => Number(url.split('/').at(-2))));
  assert(resumed.every(row => !savedOffsets.has(row.offset)));
  proof.resumedMissingOnly = true; proof.sixFontsComplete = proof.second.rows.every(r => r.status === 200 && r.decoded);
  proof.peakServerFontBodies = peak; assert(peak <= 2); assert(proof.sixFontsComplete);
  await page.screenshot({ path: join(output, 'six-original-fonts-after-restart.png') }); proof.passed = true;
} catch (error) { proof.failure = { name: error.name, message: error.message }; process.exitCode = 1; }
finally {
  await closeBrowser(); for (const socket of sockets) socket.destroy();
  for (const server of [app, assets]) if (server) await new Promise(resolve => server.close(resolve));
  proof.serversClosed = true; await writeFile(join(output, 'receipt.json'), JSON.stringify(proof,null,2)+'\n', { flag: 'wx' });
}
console.log(JSON.stringify({passed:proof.passed,firstSaved:proof.first?.parts.length,resumedMissingOnly:proof.resumedMissingOnly,
 sixFontsComplete:proof.sixFontsComplete,browserClosures:proof.browserClosures,serversClosed:proof.serversClosed,failure:proof.failure??null}));
