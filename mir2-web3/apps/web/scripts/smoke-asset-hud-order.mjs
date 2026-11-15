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
const expected = args.get("--expect") ?? "foundation-first";
assert(["foundation-first", "controls-first"].includes(expected));
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
const { ASSET_CACHE_PACKS } = await loadTs("lib/asset-cache-packs.ts", { "./original-ui": await loadTs("lib/original-ui.ts") });
const hud = ASSET_CACHE_PACKS.find(pack => pack.name === "hud-core");
assert.equal(hud.urls.length, 113); assert.equal(hud.phase, "background"); assert.equal(hud.cacheTier, "background");
const foundation = hud.urls.slice(0, 9), controls = hud.urls.slice(9).reverse();
assert.deepEqual(foundation, [1, 4, 6, 12, 13, 8, 76].map(n => `/original-ui/Prguse/${n}.png`)
  .concat(["/original-ui/UI_32bit/473.png", "/original-ui/UI_32bit/472.png"]));
const workerBytes = await readFile(resolve(args.get("--worker-source") ?? join(web, "public/mir2-asset-worker.js")));
const images = new Map(await Promise.all(hud.urls.map(async path => [path, await readFile(join(web, "public", path.slice(1)))])));
const pins = Object.fromEntries([...images].map(([path, bytes]) => [path, {
  sha256: hash(bytes), bytes: bytes.length, width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20),
}]));
assert.deepEqual(Object.entries(pins).filter(([, pin]) => !pin.width || !pin.height).map(([path]) => path), ["/original-ui/MMap/0.png"]);
const bulk = Array.from({ length: 6 }, (_, n) => `/original-ui/Prguse/${6000 + n}.png`);
const proof = { kind: "real-chrome-hud-foundation-order", expected, passed: false, expectedOutcomeObserved: false,
  scope: "Six real upstream reads are held. The other 104 HUD images queue before the nine foundation images. Release the six reads, then verify foundation admission, all 113 original bodies and background-cache SHA. Local scheduling evidence only; not public speed or full gameplay.",
  publicNetworkUsed: false, gameAccountUsed: false, workerSha256: hash(workerBytes), inventorySha256: hash(JSON.stringify(ASSET_CACHE_PACKS)),
  hudPhase: hud.phase, hudCacheTier: hud.cacheTier, foundation, imagePins: pins, requests: [] };
let context, server;
const sockets = new Set(), held = [], errors = [], unexpected = [];
try {
  server = createServer((request, response) => {
    const path = new URL(request.url, "http://127.0.0.1").pathname;
    response.setHeader("cache-control", "no-store");
    if (path === "/mir2-asset-worker.js") { response.writeHead(200, { "content-type": "text/javascript", "service-worker-allowed": "/" }); response.end(workerBytes); return; }
    if (path === "/") { response.writeHead(200, { "content-type": "text/html" }); response.end('<!doctype html><title>HUD foundation order</title><body style="background:#171610;color:#eee"><h1>Original HUD foundation order</h1><p id="status">Waiting</p><canvas width="1024" height="640"></canvas>'); return; }
    const bytes = images.get(path) ?? (bulk.includes(path) ? images.values().next().value : null);
    if (!bytes) { response.writeHead(404); response.end(); return; }
    const row = { path, requestAt: Date.now() }; proof.requests.push(row);
    const send = () => { row.headersAt = Date.now(); response.writeHead(200, { "content-type": "image/png", "content-length": bytes.length }); response.end(bytes); };
    if (bulk.includes(path)) held.push(send); else send();
  });
  server.on("connection", socket => { sockets.add(socket); socket.once("close", () => sockets.delete(socket)); });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  context = await chromium.launchPersistentContext(join(output, "profile"), { headless: true,
    ...(args.has("--chrome") ? { executablePath: resolve(args.get("--chrome")) } : {}),
    viewport: { width: 1060, height: 800 }, serviceWorkers: "allow" });
  const page = context.pages()[0];
  page.on("pageerror", error => errors.push(error.name));
  context.on("request", request => { if (new URL(request.url()).origin !== origin) unexpected.push(new URL(request.url()).origin); });
  await page.goto(origin);
  await page.evaluate(async packs => {
    await navigator.serviceWorker.register("/mir2-asset-worker.js"); await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener("controllerchange", resolve, { once: true }));
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error("Worker configuration timeout")), 5000);
      const listener = event => { if (event.data?.type !== "MIR2_ASSET_CACHE_CONFIGURED") return;
        clearTimeout(timeout); navigator.serviceWorker.removeEventListener("message", listener); resolve(); };
      navigator.serviceWorker.addEventListener("message", listener);
      navigator.serviceWorker.controller.postMessage({ type: "MIR2_ASSET_CACHE_CONFIG", cacheNamespace: "hud-order-fixture",
        manifest: { version: "fixture", resourcePacks: packs } });
    });
    window.fixtureRows = [];
    window.readImages = paths => paths.map(path => (async () => {
      const response = await fetch(path), bytes = await response.arrayBuffer();
      const sha256 = [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map(n => n.toString(16).padStart(2, "0")).join("");
      window.fixtureRows.push({ path, status: response.status, sha256, bytes: bytes.byteLength, completedAt: performance.now() });
      return { path, bytes };
    })());
  }, ASSET_CACHE_PACKS);
  const sw = context.serviceWorkers()[0]; assert(sw);
  const waitForQueue = async count => {
    for (let i = 0; i < 120; i++) {
      const state = await sw.evaluate(() => ({ active: activeStaticHeaderFetches, queued: staticHeaderQueue.length }));
      if (state.active === 6 && state.queued === count && held.length === 6) return state;
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    assert.fail(`Worker queue did not reach ${count}`);
  };
  await page.evaluate(paths => { window.fixtureBulk = window.readImages(paths); }, bulk);
  proof.initialState = await waitForQueue(0);
  await page.evaluate(paths => { window.fixtureControls = window.readImages(paths); }, controls);
  proof.controlsQueued = await waitForQueue(104);
  await page.evaluate(paths => { window.fixtureFoundation = window.readImages(paths); }, foundation.slice().reverse());
  proof.allQueued = await waitForQueue(113);
  assert.equal(proof.requests.length, 6);
  const releasedAt = Date.now(); held.forEach(release => release());
  proof.browser = await page.evaluate(async ({ paths, pins }) => {
    let timer;
    const reads = await Promise.race([
      Promise.all([...window.fixtureControls, ...window.fixtureFoundation]),
      new Promise((_, reject) => { timer = setTimeout(() => reject(Error("HUD completion timeout")), 18000); }),
    ]).finally(() => clearTimeout(timer));
    let decoded = 0;
    for (const { path, bytes } of reads) {
      if (!pins[path].width || !pins[path].height) continue;
      const bitmap = await createImageBitmap(new Blob([bytes])), index = paths.indexOf(path);
      document.querySelector("canvas").getContext("2d").drawImage(bitmap, (index % 17) * 60, Math.floor(index / 17) * 90, 56, 80);
      bitmap.close(); decoded++;
    }
    const stores = await Promise.all((await caches.keys()).filter(name => name.includes("static-background-hud-order-fixture")).map(name => caches.open(name)));
    const cacheRows = [];
    for (let round = 0; round < 60; round++) {
      cacheRows.length = 0;
      for (const path of paths) for (const store of stores) {
        const response = await store.match(path); if (!response) continue;
        const bytes = await response.arrayBuffer();
        const sha256 = [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map(n => n.toString(16).padStart(2, "0")).join("");
        cacheRows.push({ path, sha256, bytes: bytes.byteLength }); break;
      }
      if (cacheRows.length === paths.length) break;
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    const rows = window.fixtureRows.filter(row => paths.includes(row.path));
    document.querySelector("#status").textContent = `${rows.length}/113 original bodies, ${decoded} decoded, ${cacheRows.length} cached`;
    return { rows, decoded, cacheRows };
  }, { paths: hud.urls, pins });
  proof.elapsedAfterReleaseMs = Date.now() - releasedAt;
  proof.hudUpstreamOrder = proof.requests.filter(row => images.has(row.path)).map(row => row.path);
  proof.foundationFirstWave = proof.hudUpstreamOrder.slice(0, 6).every(path => foundation.includes(path));
  proof.lastFoundationAdmission = Math.max(...foundation.map(path => proof.hudUpstreamOrder.indexOf(path)));
  proof.allBodiesCorrect = proof.browser.rows.length === 113 && proof.browser.rows.every(row => row.status === 200 && row.sha256 === pins[row.path].sha256 && row.bytes === pins[row.path].bytes);
  proof.cacheBodiesCorrect = proof.browser.cacheRows.length === 113 && proof.browser.cacheRows.every(row => row.sha256 === pins[row.path].sha256 && row.bytes === pins[row.path].bytes);
  proof.passed = proof.foundationFirstWave && proof.lastFoundationAdmission < 15 && proof.allBodiesCorrect && proof.cacheBodiesCorrect && proof.browser.decoded === 112 && !errors.length && !unexpected.length;
  proof.expectedOutcomeObserved = expected === "foundation-first" ? proof.passed
    : !proof.foundationFirstWave && proof.lastFoundationAdmission >= 104 && proof.allBodiesCorrect && proof.cacheBodiesCorrect && proof.browser.decoded === 112 && !errors.length && !unexpected.length;
  await page.screenshot({ path: join(output, "hud-order.png") });
} finally {
  proof.pageErrors = errors; proof.unexpectedOrigins = [...new Set(unexpected)];
  if (context) { await context.close(); proof.browserClosed = true; }
  if (server) { const closed = new Promise(resolve => server.close(resolve)); for (const socket of sockets) socket.destroy(); await closed; proof.serverClosed = true; }
  await writeFile(join(output, "receipt.json"), JSON.stringify(proof, null, 2) + "\n");
}
assert(proof.expectedOutcomeObserved);
console.log(JSON.stringify({ expected, passed: proof.passed, expectedOutcomeObserved: proof.expectedOutcomeObserved,
  foundationFirstWave: proof.foundationFirstWave, lastFoundationAdmission: proof.lastFoundationAdmission,
  completeHud: proof.browser.rows.length, cacheBodiesCorrect: proof.cacheBodiesCorrect,
  elapsedAfterReleaseMs: proof.elapsedAfterReleaseMs, browserClosed: proof.browserClosed, serverClosed: proof.serverClosed }));
