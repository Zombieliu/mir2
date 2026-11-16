import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  const key = process.argv[i], value = process.argv[i + 1];
  assert(["--output", "--worker-source", "--chrome", "--playwright-package", "--expect"].includes(key) && value && !args.has(key));
  args.set(key, value);
}
assert(args.has("--output"));
const expected = args.get("--expect") ?? "complete";
assert(["complete", "overloaded"].includes(expected));
const web = resolve(dirname(fileURLToPath(import.meta.url)), ".."), output = resolve(args.get("--output"));
await mkdir(output, { recursive: false });
const { chromium } = createRequire(resolve(args.get("--playwright-package") ?? join(web, "package.json")))("playwright");
const hash = b => createHash("sha256").update(b).digest("hex");
const worker = await readFile(resolve(args.get("--worker-source") ?? join(web, "public/mir2-asset-worker.js")));
const image = await readFile(join(web, "public/original-ui/Prguse/985.png"));
const fontPaths = ["/original-ui/fonts/NotoSansCJKsc-Regular.otf", "/original-ui/fonts/NotoSansTC.ttf"];
const fonts = new Map(await Promise.all(fontPaths.map(async path => [path, await readFile(join(web, "public", path.slice(1)))])));
const paths = Array.from({ length: 72 }, (_, n) => `/original-ui/Prguse/${5000 + n}.png`);
const proof = { kind: "real-chrome-static-header-admission", expected, passed: false, expectedOutcomeObserved: false,
  scope: "Local HTTP/1.1, 72 simultaneous image consumers plus one duplicate and two complete original fonts. Each fixture image deliberately uses the same pinned PNG; this verifies scheduling/body/cache behavior, not game pixel parity or public speed.",
  publicNetworkUsed: false, gameAccountUsed: false, workerSha256: hash(worker), imageSha256: hash(image),
  headerDelayMs: 1100, headerBudgetMs: 7000, requests: [] };
let server, context, origin, activeImages = 0, peakImages = 0, activeFonts = 0;
const sockets = new Set(), timers = new Set();
const later = (fn, delay) => { const timer = setTimeout(() => { timers.delete(timer); fn(); }, delay); timers.add(timer); };
try {
  server = createServer((request, response) => {
    const path = new URL(request.url, "http://127.0.0.1").pathname;
    response.setHeader("cache-control", "no-store");
    if (path === "/mir2-asset-worker.js") { response.writeHead(200, { "content-type": "text/javascript", "service-worker-allowed": "/" }); response.end(worker); return; }
    if (path === "/") { response.writeHead(200, { "content-type": "text/html" }); response.end('<!doctype html><title>Static image admission fixture</title><body style="background:#151510;color:#eee;font:18px sans-serif"><h1>72 image consumers and two full fonts</h1><p id="status">Loading</p><canvas width="960" height="600"></canvas>'); return; }
    const row = { path, requestAt: Date.now() }; proof.requests.push(row);
    if (fonts.has(path)) {
      activeFonts++; row.activeFonts = activeFonts;
      response.once("close", () => { activeFonts--; row.closedAt = Date.now(); });
      const body = fonts.get(path);
      response.writeHead(200, { "content-type": path.endsWith(".otf") ? "font/otf" : "font/ttf", "content-length": body.length });
      response.write(body.subarray(0, 64)); later(() => response.end(body.subarray(64)), 3500); return;
    }
    if (!paths.includes(path)) { response.writeHead(404); response.end(); return; }
    activeImages++; peakImages = Math.max(peakImages, activeImages); row.activeImages = activeImages; row.activeFonts = activeFonts;
    response.once("close", () => { activeImages--; row.closedAt = Date.now(); });
    later(() => { if (response.destroyed) return; row.headersAt = Date.now(); response.writeHead(200, { "content-type": "image/png", "content-length": image.length }); response.end(image); }, proof.headerDelayMs);
  });
  server.on("connection", socket => { sockets.add(socket); socket.once("close", () => sockets.delete(socket)); });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  origin = "http://127.0.0.1:" + server.address().port;
  context = await chromium.launchPersistentContext(join(output, "profile"), { headless: true,
    ...(args.has("--chrome") ? { executablePath: resolve(args.get("--chrome")) } : {}), viewport: { width: 1000, height: 760 }, serviceWorkers: "allow" });
  const unexpected = [], errors = [];
  context.on("request", r => { if (new URL(r.url()).hostname !== "127.0.0.1") unexpected.push(new URL(r.url()).origin); });
  const page = context.pages()[0]; page.on("pageerror", e => errors.push(e.name));
  await page.goto(origin);
  proof.browser = await page.evaluate(async ({ paths, fontPaths, imageHash, fontPins }) => {
    await navigator.serviceWorker.register("/mir2-asset-worker.js"); await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener("controllerchange", resolve, { once: true }));
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error("worker configuration missing")), 5000);
      const listener = event => { if (event.data?.type !== "MIR2_ASSET_CACHE_CONFIGURED") return; clearTimeout(timeout); navigator.serviceWorker.removeEventListener("message", listener); resolve(); };
      navigator.serviceWorker.addEventListener("message", listener);
      navigator.serviceWorker.controller.postMessage({ type: "MIR2_ASSET_CACHE_CONFIG", cacheNamespace: "static-admission-fixture", manifest: { version: "fixture", remoteAssets: { enabled: false }, resourcePacks: [] } });
    });
    const sha = async b => [...new Uint8Array(await crypto.subtle.digest("SHA-256", b))].map(n => n.toString(16).padStart(2, "0")).join("");
    const started = performance.now();
    const read = async (path, decode) => {
      try { const response = await fetch(path, { cache: "reload" }); const bytes = await response.arrayBuffer();
        if (!response.ok) return { path, status: response.status, complete: false, atMs: Math.round(performance.now() - started) };
        const digest = await sha(bytes); let decoded = false;
        if (decode) { const bitmap = await createImageBitmap(new Blob([bytes])); const index = paths.indexOf(path), c = document.querySelector("canvas").getContext("2d"); c.drawImage(bitmap, (index % 12) * 80, Math.floor(index / 12) * 90, 72, 72); bitmap.close(); decoded = true; }
        return { path, status: response.status, bytes: bytes.byteLength, sha256: digest, complete: digest === (decode ? imageHash : fontPins[path]), decoded, atMs: Math.round(performance.now() - started) };
      } catch (error) { return { path, complete: false, errorName: error.name }; }
    };
    const fontReads = fontPaths.map(path => read(path, false));
    const imageReads = [...paths, paths[0]].map(path => read(path, true));
    const images = await Promise.all(imageReads), fonts = await Promise.all(fontReads);
    const cached = [];
    for (let round = 0; round < 50; round++) {
      cached.length = 0; const stores = await Promise.all((await caches.keys()).map(n => caches.open(n)));
      for (const path of paths) { let response; for (const store of stores) { response = await store.match(path); if (response) break; } if (response) cached.push({ path, sha256: await sha(await response.arrayBuffer()) }); }
      if (cached.length === images.filter((r, i) => i < paths.length && r.complete).length) break;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    const imageSuccess = images.slice(0, paths.length).filter(r => r.complete && r.decoded).length;
    document.querySelector("#status").textContent = `${imageSuccess}/${paths.length} complete, ${fonts.filter(r => r.complete).length}/2 fonts; duplicate ${images.at(-1).complete ? "complete" : "failed"}`;
    return { images, fonts, cached, imageSuccess, imageFailure: paths.length - imageSuccess,
      cacheBodiesCorrect: cached.every(r => r.sha256 === imageHash), elapsedMs: Math.round(performance.now() - started) };
  }, { paths, fontPaths, imageHash: hash(image), fontPins: Object.fromEntries([...fonts].map(([p, b]) => [p, hash(b)])) });
  await page.screenshot({ path: join(output, "images.png") });
  proof.peakImagesAtServer = peakImages; proof.pageErrors = errors; proof.unexpectedOrigins = [...new Set(unexpected)];
  proof.imagesWhileFontsActive = proof.requests.some(r => paths.includes(r.path) && r.activeFonts > 0);
  proof.passed = proof.browser.imageSuccess === 72 && proof.browser.images.at(-1).complete && proof.browser.fonts.every(r => r.complete) && proof.browser.cached.length === 72 && proof.browser.cacheBodiesCorrect && errors.length === 0 && unexpected.length === 0;
  proof.expectedOutcomeObserved = expected === "complete" ? proof.passed : proof.browser.imageFailure > 0 && proof.browser.imageSuccess > 0 && errors.length === 0;
} finally {
  if (context) { await context.close(); proof.browserClosed = true; }
  for (const timer of timers) clearTimeout(timer);
  if (server) { const closed = new Promise(resolve => server.close(resolve)); for (const socket of sockets) socket.destroy(); await closed; proof.serverClosed = true; }
  await writeFile(join(output, "receipt.json"), JSON.stringify(proof, null, 2) + "\n");
}
assert(proof.expectedOutcomeObserved);
console.log(JSON.stringify({ expected, passed: proof.passed, expectedOutcomeObserved: proof.expectedOutcomeObserved, success: proof.browser.imageSuccess, failure: proof.browser.imageFailure, elapsedMs: proof.browser.elapsedMs, peakImagesAtServer: proof.peakImagesAtServer, browserClosed: proof.browserClosed, serverClosed: proof.serverClosed }));
