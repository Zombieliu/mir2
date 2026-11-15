import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  const key = process.argv[i], value = process.argv[i + 1];
  assert(["--output", "--worker-source", "--chrome", "--playwright-package", "--expect"].includes(key) && value && !args.has(key));
  args.set(key, value);
}
assert(args.has("--output"));
const expected = args.get("--expect") ?? "complete";
assert(["complete", "starved"].includes(expected));
const web = resolve(dirname(fileURLToPath(import.meta.url)), ".."), output = resolve(args.get("--output"));
await mkdir(output, { recursive: false });
const { chromium } = createRequire(resolve(args.get("--playwright-package") ?? join(web, "package.json")))("playwright");
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
async function loadTs(path, dependencies = {}) {
  const module = { exports: {} };
  const compiled = ts.transpileModule(await readFile(join(web, path), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("module", "exports", "require", compiled)(module, module.exports, name => {
    assert(Object.hasOwn(dependencies, name)); return dependencies[name];
  });
  return module.exports;
}
const { ASSET_CACHE_PACKS } = await loadTs("lib/asset-cache-packs.ts", {
  "./original-ui": await loadTs("lib/original-ui.ts"),
});
const hud = ASSET_CACHE_PACKS.find(pack => pack.name === "hud-core");
assert.equal(hud.urls.length, 113); assert.equal(hud.cacheTier, "background"); assert.equal(hud.phase, "background");
const worker = await readFile(resolve(args.get("--worker-source") ?? join(web, "public/mir2-asset-worker.js")));
const images = new Map(await Promise.all(hud.urls.map(async path => [path, await readFile(join(web, "public", path.slice(1)))])));
const imagePins = Object.fromEntries([...images].map(([path, bytes]) => {
  assert.equal(bytes.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
  return [path, { sha256: hash(bytes), width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) }];
}));
assert.deepEqual(Object.entries(imagePins).filter(([, pin]) => !pin.width || !pin.height).map(([path]) => path), ["/original-ui/MMap/0.png"]);
const bulk = Array.from({ length: 48 }, (_, n) => `/original-ui/Prguse/${6000 + n}.png`);
const proof = { kind: "real-chrome-required-hud-priority", expected, passed: false, expectedOutcomeObserved: false,
  scope: "Local controlled HTTP/1.1: six admitted scene reads have missing response headers, 42 more scene reads queue before the real 113-image HUD manifest. Origin scene attempts expire at the unchanged 7-second deadline; a configured mirror serves complete original PNG bytes. This tests admission, coalescing and cache bodies, not public speed, full gameplay or pixel parity.",
  publicNetworkUsed: false, gameAccountUsed: false, workerSha256: hash(worker),
  inventorySha256: hash(JSON.stringify(ASSET_CACHE_PACKS)), hudPhase: hud.phase, hudCacheTier: hud.cacheTier,
  originalEmptyImage: "/original-ui/MMap/0.png", imagePins,
  localObservationBudgetMs: 18000, requests: [] };
let context, server, origin, initialCount = 0, resolveInitial;
const initialReady = new Promise(resolve => { resolveInitial = resolve; }), sockets = new Set(), timers = new Set();
const later = (fn, ms) => { const timer = setTimeout(() => { timers.delete(timer); fn(); }, ms); timers.add(timer); return timer; };
try {
  server = createServer((request, response) => {
    const path = new URL(request.url, "http://127.0.0.1").pathname;
    response.setHeader("cache-control", "no-store");
    if (path === "/mir2-asset-worker.js") { response.writeHead(200, { "content-type": "text/javascript", "service-worker-allowed": "/" }); response.end(worker); return; }
    if (path === "/") { response.writeHead(200, { "content-type": "text/html" }); response.end('<!doctype html><title>Required HUD priority fixture</title><body style="background:#171610;color:#eee;font:18px sans-serif"><h1>Required HUD and queued scene reads</h1><p id="status">Waiting</p><canvas width="1024" height="640"></canvas>'); return; }
    const mirrored = path.startsWith("/mirror/current/"), assetPath = mirrored ? path.slice("/mirror/current".length) : path;
    const row = { path, assetPath, mirrored, requestAt: Date.now() }; proof.requests.push(row);
    response.once("close", () => { row.closedAt = Date.now(); });
    if (!mirrored && bulk.includes(path)) {
      // Never send headers. The real Worker timeout, not a fixture retry, releases
      // each logical admission to the configured source fallback.
      initialCount++; if (initialCount === 6) resolveInitial(); return;
    }
    const bytes = images.get(assetPath) ?? (bulk.includes(assetPath) ? images.values().next().value : null);
    if (!bytes) { response.writeHead(404); response.end(); return; }
    row.headersAt = Date.now();
    response.writeHead(200, { "content-type": "image/png", "content-length": bytes.length });
    response.end(bytes);
  });
  server.on("connection", socket => { sockets.add(socket); socket.once("close", () => sockets.delete(socket)); });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve)); origin = `http://127.0.0.1:${server.address().port}`;
  context = await chromium.launchPersistentContext(join(output, "profile"), { headless: true,
    ...(args.has("--chrome") ? { executablePath: resolve(args.get("--chrome")) } : {}),
    viewport: { width: 1060, height: 800 }, serviceWorkers: "allow" });
  const page = context.pages()[0], errors = [], unexpected = [];
  page.on("pageerror", error => errors.push(error.name));
  context.on("request", request => { if (new URL(request.url()).origin !== origin) unexpected.push(new URL(request.url()).origin); });
  await page.goto(origin);
  await page.evaluate(async ({ packs, bulk }) => {
    await navigator.serviceWorker.register("/mir2-asset-worker.js"); await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener("controllerchange", resolve, { once: true }));
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error("Worker configuration timeout")), 5000);
      const listener = event => { if (event.data?.type !== "MIR2_ASSET_CACHE_CONFIGURED") return;
        clearTimeout(timeout); navigator.serviceWorker.removeEventListener("message", listener); resolve(); };
      navigator.serviceWorker.addEventListener("message", listener);
      navigator.serviceWorker.controller.postMessage({ type: "MIR2_ASSET_CACHE_CONFIG", cacheNamespace: "hud-priority-fixture",
        manifest: { version: "fixture", remoteAssets: { assetBaseUrl: location.origin + "/mirror/current" }, resourcePacks: packs } });
    });
    window.fixtureBulk = bulk.slice(0, 6).map(path => fetch(path).then(response => response.arrayBuffer()).catch(() => null));
  }, { packs: ASSET_CACHE_PACKS, bulk });
  await Promise.race([initialReady, new Promise((_, reject) => later(() => reject(Error("six initial requests not observed")), 5000))]);
  proof.browser = await page.evaluate(async ({ bulk, paths, pins, budget }) => {
    const started = performance.now(), rows = [], sha = async bytes => [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map(n => n.toString(16).padStart(2, "0")).join("");
    window.fixtureBulk.push(...bulk.slice(6).map(path => fetch(path).then(response => response.arrayBuffer()).catch(() => null)));
    const reads = paths.map(async (path, index) => {
      try {
        const response = await fetch(path), bytes = await response.arrayBuffer(), digest = await sha(bytes);
        // Crystal's MMap/0.png is the original zero-dimension placeholder. Its
        // complete bytes still count, but it cannot be decoded as a bitmap.
        const expectedEmpty = !pins[path].width || !pins[path].height;
        if (!expectedEmpty) {
          const bitmap = await createImageBitmap(new Blob([bytes]));
          document.querySelector("canvas").getContext("2d").drawImage(bitmap, (index % 17) * 60, Math.floor(index / 17) * 90, 56, 80); bitmap.close();
        }
        rows.push({ path, status: response.status, bytes: bytes.byteLength, sha256: digest,
          complete: response.status === 200 && digest === pins[path].sha256, decoded: !expectedEmpty, expectedEmpty, atMs: Math.round(performance.now() - started) });
      } catch (error) { rows.push({ path, complete: false, error: error.name }); }
    });
    // A retiring duplicate owner must not abort the HUD's shared cache fill.
    const owner = new AbortController();
    const duplicate = fetch(paths[0], { signal: owner.signal }).then(r => r.arrayBuffer()).then(() => "complete").catch(e => e.name);
    setTimeout(() => owner.abort(), 30);
    const img = new Image(); img.src = paths[0]; document.body.append(img);
    let timer;
    await Promise.race([Promise.all(reads), new Promise(resolve => { timer = setTimeout(resolve, budget); })]); clearTimeout(timer);
    const cacheRows = [];
    if (rows.length === paths.length && rows.every(row => row.complete)) {
      for (let round = 0; round < 40; round++) {
        cacheRows.length = 0;
        const stores = await Promise.all((await caches.keys()).filter(name => name.includes("static-background-hud-priority-fixture")).map(name => caches.open(name)));
        for (const path of paths) {
          for (const store of stores) {
            const response = await store.match(path);
            if (response) { const bytes = await response.arrayBuffer(); cacheRows.push({ path, sha256: await sha(bytes), bytes: bytes.byteLength }); break; }
          }
        }
        if (cacheRows.length === paths.length) break;
        await new Promise(resolve => setTimeout(resolve, 50));
      }
    }
    document.querySelector("#status").textContent = `${rows.filter(row => row.complete).length}/${paths.length} original HUD bodies; ${rows.filter(row => row.decoded).length} decoded images; ${cacheRows.length} cached`;
    return { rows, cacheRows, duplicateOwner: await duplicate, elapsedMs: Math.round(performance.now() - started),
      cacheBodiesCorrect: cacheRows.length === paths.length && cacheRows.every(row => row.sha256 === pins[row.path].sha256),
      imageElementComplete: img.complete && img.naturalWidth > 0 };
  }, { bulk, paths: hud.urls, pins: imagePins, budget: proof.localObservationBudgetMs });
  await page.screenshot({ path: join(output, "hud-priority.png") });
  const originReads = proof.requests.filter(row => !row.mirrored && (bulk.includes(row.assetPath) || images.has(row.assetPath)));
  const queuedSceneIndex = originReads.findIndex((row, i) => i >= 6 && bulk.includes(row.assetPath));
  proof.requiredBeforeQueuedScene = originReads.slice(6, queuedSceneIndex < 0 ? undefined : queuedSceneIndex).filter(row => images.has(row.assetPath)).length;
  proof.pageErrors = errors; proof.unexpectedOrigins = [...new Set(unexpected)];
  proof.passed = proof.browser.rows.length === 113 && proof.browser.rows.every(row => row.complete && (row.decoded || row.expectedEmpty))
    && proof.browser.cacheBodiesCorrect && proof.browser.imageElementComplete
    && proof.requiredBeforeQueuedScene === 113 && errors.length === 0 && unexpected.length === 0;
  proof.expectedOutcomeObserved = expected === "complete" ? proof.passed
    : proof.browser.rows.length === 0 && proof.requiredBeforeQueuedScene === 0 && errors.length === 0 && unexpected.length === 0;
} finally {
  if (context) { await context.close(); proof.browserClosed = true; }
  for (const timer of timers) clearTimeout(timer);
  if (server) { const closed = new Promise(resolve => server.close(resolve)); for (const socket of sockets) socket.destroy(); await closed; proof.serverClosed = true; }
  await writeFile(join(output, "receipt.json"), JSON.stringify(proof, null, 2) + "\n");
}
assert(proof.expectedOutcomeObserved);
console.log(JSON.stringify({ expected, passed: proof.passed, expectedOutcomeObserved: proof.expectedOutcomeObserved,
  completeHud: proof.browser.rows.filter(row => row.complete).length, requiredBeforeQueuedScene: proof.requiredBeforeQueuedScene,
  elapsedMs: proof.browser.elapsedMs, browserClosed: proof.browserClosed, serverClosed: proof.serverClosed }));
