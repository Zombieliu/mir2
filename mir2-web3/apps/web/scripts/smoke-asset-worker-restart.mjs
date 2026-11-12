import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  const name = process.argv[i], value = process.argv[i + 1];
  assert(["--output", "--chrome", "--playwright-package", "--asset-file", "--worker-source"].includes(name) && value, "expected named smoke arguments");
  assert(!args.has(name), "duplicate smoke argument"); args.set(name, value);
}
assert(args.has("--output"), "--output must name a fresh evidence directory");
const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const workerSource = await readFile(resolve(args.get("--worker-source") ?? join(webRoot, "public/mir2-asset-worker.js")));
const image = await readFile(resolve(args.get("--asset-file") ?? join(webRoot, "public/original-ui/Prguse/1.png")));
const output = resolve(args.get("--output"));
const load = args.has("--playwright-package") ? createRequire(resolve(args.get("--playwright-package"))) : createRequire(import.meta.url);
const { chromium } = load("playwright");
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
await mkdir(output, { recursive: true });
const requests = [], versions = [];
const proof = { kind: "real-chrome-worker-cache-restart", passed: false, workerSha256: sha(workerSource), imageSha256: sha(image),
  publicNetworkUsed: false, gameAccountUsed: false };
let server, browser, origin;
try {
  server = createServer((request, response) => {
    const path = new URL(request.url, origin ?? "http://127.0.0.1").pathname;
    requests.push(path); response.setHeader("cache-control", "no-store");
    if (path === "/mir2-asset-worker.js") {
      response.writeHead(200, { "content-type": "application/javascript", "service-worker-allowed": "/" }); response.end(workerSource);
    } else if (path.startsWith("/mirror/release-a/original-ui/")) {
      response.writeHead(200, { "content-type": "image/png", "access-control-allow-origin": "*" }); response.end(image);
    } else if (path.startsWith("/original-ui/")) {
      response.writeHead(504); response.end("fixture original origin unavailable");
    } else {
      response.writeHead(200, { "content-type": "text/html" }); response.end("<!doctype html><title>Worker cache restart</title>");
    }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  origin = "http://127.0.0.1:" + server.address().port;
  browser = await chromium.launch({ headless: true, ...(args.has("--chrome") ? { executablePath: resolve(args.get("--chrome")) } : {}) });
  const context = await browser.newContext({ serviceWorkers: "allow" }), page = await context.newPage();
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await page.evaluate(async () => {
    await navigator.serviceWorker.register("/mir2-asset-worker.js", { scope: "/" }); await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener("controllerchange", resolve, { once: true }));
  });
  const cdp = await context.newCDPSession(page);
  cdp.on("ServiceWorker.workerVersionUpdated", event => versions.push(...event.versions.map(row => ({ versionId: row.versionId, runningStatus: row.runningStatus, status: row.status }))));
  await cdp.send("ServiceWorker.enable");
  const configured = await page.evaluate(async origin => {
    const ack = new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error("configuration acknowledgement missing")), 5000);
      const listener = event => { if (event.data?.type !== "MIR2_ASSET_CACHE_CONFIGURED") return;
        navigator.serviceWorker.removeEventListener("message", listener); clearTimeout(timeout); resolve(event.data); };
      navigator.serviceWorker.addEventListener("message", listener);
    });
    navigator.serviceWorker.controller.postMessage({ type: "MIR2_ASSET_CACHE_CONFIG", manifestVersion: "release-a", cacheNamespace: "restart-release-a", fontMirrorFirst: true,
      manifest: { version: "release-a", remoteAssets: { enabled: true, assetBaseUrl: origin + "/mirror/release-a", browserFallbackBaseUrls: [] },
        resourcePacks: [{ cacheTier: "critical", urls: ["/original-ui/Prguse/1.png"] }] } });
    return ack;
  }, origin);
  assert.equal(configured.version, "restart-release-a");
  const read = () => page.evaluate(async () => {
    const response = await fetch("/original-ui/Prguse/1.png"), body = await response.arrayBuffer();
    return { status: response.status, bytes: body.byteLength, sha256: [...new Uint8Array(await crypto.subtle.digest("SHA-256", body))].map(x => x.toString(16).padStart(2, "0")).join("") };
  });
  proof.before = await read(); assert.equal(proof.before.status, 200); assert.equal(proof.before.sha256, sha(image));
  const checkpoint = requests.length;
  // This stops only the browser instance launched above, never a user's tabs.
  await cdp.send("ServiceWorker.stopAllWorkers");
  for (let n = 0; n < 50 && !versions.some(row => row.runningStatus === "stopped"); n++) await delay(100);
  const stopped = versions.findIndex(row => row.runningStatus === "stopped"); assert(stopped >= 0, "Chrome must confirm worker stop");
  proof.after = await read();
  for (let n = 0; n < 50 && !versions.slice(stopped + 1).some(row => row.runningStatus === "running"); n++) await delay(100);
  proof.restartObserved = versions.slice(stopped + 1).some(row => row.runningStatus === "running");
  proof.requestsAfterRestart = requests.slice(checkpoint); proof.workerLifecycle = versions;
  assert(proof.restartObserved); assert.equal(proof.after.status, 200); assert.equal(proof.after.bytes, image.length); assert.equal(proof.after.sha256, sha(image));
  assert(!proof.requestsAfterRestart.some(path => path.includes("/original-ui/")), "restart must reuse the existing cache without an original-origin or mirror request");
  proof.passed = true;
} catch (error) {
  proof.failure = { name: error.name, message: error.message };
} finally {
  await browser?.close(); if (server) await new Promise(resolve => server.close(resolve));
  proof.browserClosed = true; proof.serverClosed = true;
  await writeFile(join(output, "receipt.json"), JSON.stringify(proof, null, 2) + "\n", { flag: "wx" });
}
console.log(JSON.stringify(proof)); if (!proof.passed) process.exitCode = 1;
