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
assert(["complete", "incomplete"].includes(expected));
const web = resolve(dirname(fileURLToPath(import.meta.url)), ".."), output = resolve(args.get("--output"));
await mkdir(output, { recursive: false });
const load = args.has("--playwright-package") ? createRequire(resolve(args.get("--playwright-package"))) : createRequire(import.meta.url);
const { chromium } = load("playwright"), hash = bytes => createHash("sha256").update(bytes).digest("hex");
const worker = await readFile(resolve(args.get("--worker-source") ?? join(web, "public/mir2-asset-worker.js")));
const fontSource = await readFile(join(web, "lib/bevy-runtime-font-prewarm.ts"), "utf8");
const paths = [...fontSource.matchAll(/"(\/original-ui\/fonts\/[^"\n]+\.(?:ttf|otf))"/g)].map(row => row[1]);
assert.equal(new Set(paths).size, 6);
const fonts = new Map(await Promise.all(paths.map(async path => [path, await readFile(join(web, "public", path.slice(1)))])));
const image = await readFile(join(web, "public/original-ui/Prguse/985.png"));
const proof = { kind: "real-chrome-six-font-body-admission", passed: false, expectedOutcomeObserved: false,
  expected, publicNetworkUsed: false, gameAccountUsed: false, workerSha256: hash(worker),
  story: "Renderer and CSS request six complete original font families together while the origin can serve two live bodies; images remain independent.",
  fontPins: [...fonts].map(([path, bytes]) => ({ path, bytes: bytes.length, sha256: hash(bytes) })), requests: [] };
let server, browser, activeFonts = 0, peakFonts = 0;
const timers = new Set();
try {
  server = createServer((request, response) => {
    const path = new URL(request.url, "http://127.0.0.1").pathname;
    const row = { path, started: Date.now() }; proof.requests.push(row);
    response.setHeader("cache-control", "no-store"); response.setHeader("access-control-allow-origin", "*");
    response.on("finish", () => { row.status = response.statusCode; row.finished = Date.now(); });
    if (path === "/mir2-asset-worker.js") {
      response.writeHead(200, { "content-type": "text/javascript", "service-worker-allowed": "/" }); response.end(worker);
    } else if (path === "/") {
      response.writeHead(200, { "content-type": "text/html" });
      response.end('<!doctype html><title>Original font admission</title><canvas width="920" height="360"></canvas>');
    } else if (path === "/original-ui/Prguse/985.png") {
      row.fontsActiveAtImageRequest = activeFonts; response.writeHead(200, { "content-type": "image/png" }); response.end(image);
    } else {
      const logical = path.startsWith("/mir2/v/current/") ? path.slice("/mir2/v/current".length) : path;
      const bytes = fonts.get(logical);
      if (!bytes) { response.writeHead(404); response.end(); return; }
      if (activeFonts >= 2) { response.writeHead(503, { "retry-after": "1" }); response.end("two font bodies already active"); return; }
      activeFonts++; peakFonts = Math.max(peakFonts, activeFonts);
      let released = false;
      response.once("close", () => { if (!released) { released = true; activeFonts--; } });
      response.writeHead(200, { "content-type": logical.endsWith(".otf") ? "font/otf" : "font/ttf", "content-length": bytes.length });
      response.write(bytes.subarray(0, 32));
      const timer = setTimeout(() => { timers.delete(timer); response.end(bytes.subarray(32)); }, 2000);
      timers.add(timer);
    }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port, origin = "http://127.0.0.1:" + port;
  browser = await chromium.launch({ headless: true, ...(args.has("--chrome") ? { executablePath: resolve(args.get("--chrome")) } : {}) });
  const context = await browser.newContext({ serviceWorkers: "allow" }), page = await context.newPage();
  const unexpected = [];
  context.on("request", request => { const url = new URL(request.url()); if (!["127.0.0.1", "localhost"].includes(url.hostname)) unexpected.push(url.origin); });
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  proof.browser = await page.evaluate(async ({ paths, port }) => {
    await navigator.serviceWorker.register("/mir2-asset-worker.js"); await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener("controllerchange", resolve, { once: true }));
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error("worker configuration missing")), 5000);
      const listener = event => { if (event.data?.type !== "MIR2_ASSET_CACHE_CONFIGURED") return;
        clearTimeout(timeout); navigator.serviceWorker.removeEventListener("message", listener); resolve(); };
      navigator.serviceWorker.addEventListener("message", listener);
      navigator.serviceWorker.controller.postMessage({ type: "MIR2_ASSET_CACHE_CONFIG", manifestVersion: "current", cacheNamespace: "font-admission",
        manifest: { version: "current", remoteAssets: { enabled: true, assetBaseUrl: "http://localhost:" + port + "/mir2/v/current" } } });
    });
    const started = performance.now();
    const read = async path => { const response = await fetch(path), body = await response.arrayBuffer();
      let decoded = false;
      if (response.ok) { const family = "Original" + paths.indexOf(path), face = new FontFace(family, body); await face.load(); document.fonts.add(face); decoded = face.status === "loaded"; }
      return { path, status: response.status, bytes: body.byteLength, decoded, sha256: [...new Uint8Array(await crypto.subtle.digest("SHA-256", body))].map(x => x.toString(16).padStart(2, "0")).join("") }; };
    const reads = paths.map(read), duplicate = read(paths[1]);
    const image = await fetch("/original-ui/Prguse/985.png"); await image.arrayBuffer();
    const imageMs = performance.now() - started;
    const rows = await Promise.all(reads), duplicateRow = await duplicate;
    const cached = [];
    for (const path of paths) { const response = await caches.match(location.origin + path); const body = response ? await response.arrayBuffer() : null;
      cached.push({ path, present: !!response, bytes: body?.byteLength ?? 0, sha256: body ? [...new Uint8Array(await crypto.subtle.digest("SHA-256", body))].map(x => x.toString(16).padStart(2, "0")).join("") : null }); }
    const canvas = document.querySelector("canvas"), ctx = canvas.getContext("2d"); ctx.fillStyle = "#15110b"; ctx.fillRect(0, 0, canvas.width, canvas.height);
    const samples = ["比奇省 · 阿洛", "繁體中文 · 冒險日記", "Adventure with Alo", "देवनागरी", "ภาษาไทย", "العربية"];
    for (let i = 0; i < rows.length; i++) { ctx.fillStyle = rows[i].decoded ? "#e9c77b" : "#f06c60"; ctx.font = `25px Original${i}`; ctx.fillText(samples[i] + " — " + rows[i].status, 22, 48 + i * 51); }
    return { rows, duplicate: duplicateRow, cache: cached, elapsedMs: performance.now() - started, imageMs };
  }, { paths, port });
  assert.equal(unexpected.length, 0); proof.unexpectedNetworkOrigins = unexpected;
  proof.peakServerFontBodies = peakFonts;
  proof.passed = proof.browser.rows.every(row => { const pin = proof.fontPins.find(pin => pin.path === row.path), cache = proof.browser.cache.find(c => c.path === row.path);
    return row.status === 200 && row.decoded && row.bytes === pin.bytes && row.sha256 === pin.sha256 && cache?.present && cache.bytes === pin.bytes && cache.sha256 === pin.sha256; });
  assert(proof.requests.some(row => row.path === "/original-ui/Prguse/985.png" && row.fontsActiveAtImageRequest === 2), "image must progress while both font bodies are live");
  assert.equal(proof.requests.filter(row => row.path === paths[1]).length, 1, "CSS and renderer share one full TC body");
  assert.equal(proof.browser.duplicate.sha256, proof.browser.rows[1].sha256);
  assert.equal(proof.passed, expected === "complete");
  if (proof.passed) assert.equal(proof.requests.filter(row => row.status === 503).length, 0);
  else assert(proof.browser.rows.some(row => row.status === 503));
  proof.expectedOutcomeObserved = true;
  await page.screenshot({ path: join(output, "original-fonts.png") });
} catch (error) { proof.failure = { name: error.name, message: error.message }; }
finally {
  await browser?.close(); proof.browserClosed = true;
  for (const timer of timers) clearTimeout(timer);
  if (server) { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
  proof.serverClosed = true;
  await writeFile(join(output, "receipt.json"), JSON.stringify(proof, null, 2) + "\n", { flag: "wx" });
}
console.log(JSON.stringify({ passed: proof.passed, expectedOutcomeObserved: proof.expectedOutcomeObserved, failure: proof.failure,
  completeFonts: proof.browser?.rows.filter(r => r.decoded).length, imageMs: proof.browser?.imageMs, output }));
if (!proof.expectedOutcomeObserved) process.exitCode = 1;
