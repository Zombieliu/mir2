import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  const name = process.argv[i], value = process.argv[i + 1];
  assert(["--output", "--worker-source", "--chrome", "--playwright-package", "--expect"].includes(name) && value);
  assert(!args.has(name)); args.set(name, value);
}
assert(args.has("--output"));
const expected = args.get("--expect") ?? "complete";
assert(["complete", "delayed"].includes(expected));
const web = resolve(dirname(fileURLToPath(import.meta.url)), ".."), output = resolve(args.get("--output"));
await mkdir(output, { recursive: false });
const { chromium } = (args.has("--playwright-package") ? createRequire(resolve(args.get("--playwright-package"))) : createRequire(import.meta.url))("playwright");
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const worker = await readFile(resolve(args.get("--worker-source") ?? join(web, "public/mir2-asset-worker.js")));
const paths = [...(await readFile(join(web, "lib/bevy-runtime-font-prewarm.ts"), "utf8")).matchAll(/"(\/original-ui\/fonts\/[^"\n]+\.(?:ttf|otf))"/g)].map(row => row[1]);
assert.equal(new Set(paths).size, 6);
const fonts = new Map(await Promise.all(paths.map(async path => [path, await readFile(join(web, "public", path.slice(1)))])));
const image = await readFile(join(web, "public/original-ui/Prguse/985.png"));
const proof = { kind: "real-chrome-font-fallback-admission-chain", expected, passed: false, expectedOutcomeObserved: false,
  publicNetworkUsed: false, gameAccountUsed: false, workerSha256: hash(worker),
  scope: "Six original fonts and one duplicate; partial SC source fails while TC progresses, four renderer families queue, complete SC fallback must retain its admission. Local HTTP fault fixture, not public latency qualification.",
  fontPins: [...fonts].map(([path, bytes]) => ({ path, bytes: bytes.length, sha256: hash(bytes) })), requests: [] };
let server, browser, origin, active = 0, peak = 0;
const timers = new Set(), sockets = new Set();
const later = (callback, delay) => { const timer = setTimeout(() => { timers.delete(timer); callback(); }, delay); timers.add(timer); };
try {
  server = createServer((request, response) => {
    const path = new URL(request.url, "http://127.0.0.1").pathname;
    const row = { path, started: Date.now() }; proof.requests.push(row);
    response.setHeader("cache-control", "no-store"); response.setHeader("access-control-allow-origin", "*");
    response.on("finish", () => { row.status = response.statusCode; row.finished = Date.now(); });
    if (path === "/mir2-asset-worker.js") {
      response.writeHead(200, { "content-type": "text/javascript", "service-worker-allowed": "/" }); response.end(worker); return;
    }
    if (path === "/") { response.writeHead(200, { "content-type": "text/html" }); response.end('<!doctype html><title>Font fallback admission</title><canvas width="960" height="360"></canvas>'); return; }
    if (path === "/original-ui/Prguse/985.png") { row.fontsActiveAtImage = active; response.writeHead(200, { "content-type": "image/png" }); response.end(image); return; }
    const prefix = path.startsWith("/hotlink-ok/mir2/v/current/") ? "/hotlink-ok/mir2/v/current" : path.startsWith("/mir2/v/current/") ? "/mir2/v/current" : "";
    const logical = path.slice(prefix.length), bytes = fonts.get(logical);
    if (!bytes) { response.writeHead(404); response.end(); return; }
    active++; peak = Math.max(peak, active); row.activeAtStart = active;
    response.once("close", () => { active--; row.closed = Date.now(); });
    response.writeHead(200, { "content-type": logical.endsWith(".otf") ? "font/otf" : "font/ttf", "content-length": bytes.length });
    response.write(bytes.subarray(0, 64));
    if (prefix === "/mir2/v/current" && logical === paths[0]) {
      row.injectedPartialFailure = true; later(() => { row.injectedFailureAt = Date.now(); response.destroy(); }, 150);
    } else {
      const delay = logical === paths[1] ? 3000 : logical === paths[0] ? 120 : 400;
      later(() => response.end(bytes.subarray(64)), delay);
    }
  });
  server.on("connection", socket => { sockets.add(socket); socket.once("close", () => sockets.delete(socket)); });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port; origin = "http://127.0.0.1:" + port;
  browser = await chromium.launch({ headless: true, ...(args.has("--chrome") ? { executablePath: resolve(args.get("--chrome")) } : {}) });
  const context = await browser.newContext({ serviceWorkers: "allow" }), page = await context.newPage();
  const unexpected = [];
  context.on("request", request => { const u = new URL(request.url()); if (!["127.0.0.1", "localhost"].includes(u.hostname)) unexpected.push(u.origin); });
  await page.goto(origin);
  proof.browser = await page.evaluate(async ({ paths, port }) => {
    await navigator.serviceWorker.register("/mir2-asset-worker.js"); await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener("controllerchange", resolve, { once: true }));
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error("worker configuration missing")), 5000);
      const listener = event => { if (event.data?.type !== "MIR2_ASSET_CACHE_CONFIGURED") return; clearTimeout(timeout); navigator.serviceWorker.removeEventListener("message", listener); resolve(); };
      navigator.serviceWorker.addEventListener("message", listener);
      navigator.serviceWorker.controller.postMessage({ type: "MIR2_ASSET_CACHE_CONFIG", manifestVersion: "current", cacheNamespace: "font-chain",
        manifest: { version: "current", remoteAssets: { enabled: true, assetBaseUrl: `http://localhost:${port}/mir2/v/current`, browserFallbackBaseUrls: [`http://localhost:${port}/hotlink-ok/mir2/v/current`] } } });
    });
    const started = performance.now();
    const digest = async bytes => [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map(x => x.toString(16).padStart(2, "0")).join("");
    const read = async path => {
      const response = await fetch(path), bytes = await response.arrayBuffer(), index = paths.indexOf(path);
      const face = new FontFace("Original" + index, bytes); await face.load(); document.fonts.add(face);
      return { path, status: response.status, bytes: bytes.byteLength, sha256: await digest(bytes), decoded: face.status === "loaded", completedMs: performance.now() - started };
    };
    const pending = paths.map(read), duplicate = read(paths[1]);
    const image = await fetch("/original-ui/Prguse/985.png"); await image.arrayBuffer();
    const imageMs = performance.now() - started, rows = await Promise.all(pending), duplicateRow = await duplicate;
    const cache = [];
    for (const path of paths) { const response = await caches.match(location.origin + path), bytes = response ? await response.arrayBuffer() : null;
      cache.push({ path, present: !!response, bytes: bytes?.byteLength ?? 0, sha256: bytes ? await digest(bytes) : null }); }
    const canvas = document.querySelector("canvas"), ctx = canvas.getContext("2d"); ctx.fillStyle = "#15110b"; ctx.fillRect(0, 0, canvas.width, canvas.height);
    const samples = ["比奇省 · 阿洛", "繁體中文 · 冒險日記", "Adventure with Alo", "देवनागरी", "ภาษาไทย", "العربية"];
    for (let i = 0; i < rows.length; i++) { ctx.fillStyle = "#e9c77b"; ctx.font = `25px Original${i}`; ctx.fillText(samples[i] + " — complete", 22, 48 + i * 51); }
    return { rows, duplicate: duplicateRow, cache, imageMs, elapsedMs: performance.now() - started };
  }, { paths, port });
  assert.equal(unexpected.length, 0); proof.unexpectedOrigins = unexpected;
  proof.peakServerFontBodies = peak; assert.equal(peak, 2);
  proof.sixFontsComplete = proof.browser.rows.every(row => {
    const pin = proof.fontPins.find(pin => pin.path === row.path), cache = proof.browser.cache.find(c => c.path === row.path);
    return row.status === 200 && row.decoded && row.bytes === pin.bytes && row.sha256 === pin.sha256 && cache?.present && cache.bytes === pin.bytes && cache.sha256 === pin.sha256;
  }); assert(proof.sixFontsComplete);
  assert.equal(proof.browser.duplicate.sha256, proof.fontPins[1].sha256);
  assert.equal(proof.requests.filter(r => r.path.endsWith(paths[1])).length, 1, "duplicate TC consumers must share one full body");
  assert(proof.requests.some(r => r.path === "/original-ui/Prguse/985.png" && r.fontsActiveAtImage === 2));
  const first = proof.requests.find(r => r.injectedPartialFailure), fallback = proof.requests.find(r => r.path === "/hotlink-ok/mir2/v/current" + paths[0]);
  assert(first && fallback && first.injectedFailureAt);
  const small = proof.requests.filter(r => paths.slice(2).includes(r.path)); assert.equal(small.length, 4);
  proof.fallbackAfterFailureMs = fallback.started - first.injectedFailureAt;
  proof.smallReadsBeforeFallback = small.filter(r => proof.requests.indexOf(r) < proof.requests.indexOf(fallback)).length;
  proof.passed = proof.smallReadsBeforeFallback === 0;
  proof.expectedOutcomeObserved = expected === "complete" ? proof.passed : !proof.passed && proof.smallReadsBeforeFallback === 4;
  await page.screenshot({ path: join(output, "six-fonts-after-fallback.png") });
  assert(proof.expectedOutcomeObserved, "complete font fallback admission differs from expected outcome");
} catch (error) {
  proof.failure = { code: error.code ?? null, message: error.message }; process.exitCode = 1;
} finally {
  for (const timer of timers) clearTimeout(timer);
  if (browser) { await browser.close(); proof.browserClosed = !browser.isConnected(); }
  if (server) { for (const socket of sockets) socket.destroy(); await new Promise(resolve => server.close(resolve)); proof.serverClosed = true; }
  await writeFile(join(output, "receipt.json"), JSON.stringify(proof, null, 2) + "\n", { flag: "wx" });
}
console.log(JSON.stringify({ passed: proof.passed, expectedOutcomeObserved: proof.expectedOutcomeObserved, fallbackAfterFailureMs: proof.fallbackAfterFailureMs, smallReadsBeforeFallback: proof.smallReadsBeforeFallback, browserClosed: proof.browserClosed, serverClosed: proof.serverClosed }));
