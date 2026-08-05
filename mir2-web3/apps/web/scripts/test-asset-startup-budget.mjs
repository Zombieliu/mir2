import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const registrarSource = await fs.readFile(
  path.join(webRoot, "app", "components", "asset-cache-registrar.tsx"),
  "utf8",
);
const pageSource = await fs.readFile(path.join(webRoot, "app", "page.tsx"), "utf8");
const shellSource = await fs.readFile(path.join(webRoot, "app", "original-client-shell.tsx"), "utf8");

test("Service Worker lifecycle work cannot block prewarm or first play", () => {
  assert.match(registrarSource, /void configureServiceWorkerInBackground\(/);
  assert.match(registrarSource, /void registration\.update\(\)\.then\(/);
  assert.doesNotMatch(registrarSource, /await registration\.update\(\)/);
  assert.doesNotMatch(registrarSource, /await navigator\.serviceWorker\.ready/);
  assert.match(registrarSource, /SERVICE_WORKER_CONFIG_ACK_TIMEOUT_MS = 750/);
  assert.ok(
    registrarSource.indexOf("void configureServiceWorkerInBackground") <
      registrarSource.indexOf("new AssetPrewarmOrchestrator"),
    "prewarm setup must continue immediately after background worker setup is queued",
  );
});

test("mobile runtime failures stay on the playable compatibility path", () => {
  assert.match(pageSource, /resolveBevyRuntimeBootDecision/);
  assert.match(pageSource, /setRuntimePhase\("dom-only"\)/);
  assert.match(pageSource, /bevyRuntimeDegraded/);
  assert.doesNotMatch(pageSource, /appendLog\(t\("runtime\.bootFailed"/);
  assert.match(pageSource, /new Request\(runtimeWasmPath, \{ signal: controller\.signal \}\)/);
});

test("login bootstrap images stay below the critical-path byte budget", async () => {
  const variants = [
    { name: "chrsel-0-768.webp", width: 768, maxBytes: 240 * 1024 },
    { name: "chrsel-0-1024.webp", width: 1024, maxBytes: 380 * 1024 },
  ];
  for (const variant of variants) {
    const assetPath = path.join(webRoot, "public", "bootstrap", "login", variant.name);
    const stat = await fs.stat(assetPath);
    const metadata = await sharp(assetPath).metadata();
    assert.equal(metadata.width, variant.width);
    assert.equal(metadata.format, "webp");
    assert.ok(stat.size <= variant.maxBytes, `${variant.name} exceeds ${variant.maxBytes} bytes`);
  }
  assert.match(shellSource, /<picture className="client-scene-background-picture">/);
  assert.match(shellSource, /LOGIN_BACKGROUND_MOBILE_WEBP/);
  assert.match(shellSource, /fetchPriority="high"/);
  assert.doesNotMatch(shellSource, /optimizedLoginBackground/);
  assert.ok(
    shellSource.includes(
      'screen === "login" ? `url("${LOGIN_BACKGROUND_MOBILE_WEBP}")` : undefined',
    ),
    "the coarse-pointer letterbox must reuse the mobile bootstrap request during hydration",
  );
});
