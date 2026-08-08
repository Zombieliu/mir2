import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const execFileAsync = promisify(execFile);
const loginBackgroundScript = path.join(
  scriptDir,
  "generate-login-background-assets.mjs",
);
const originalAssetManifestScript = path.join(
  scriptDir,
  "generate-original-asset-manifest.mjs",
);
const registrarSource = await fs.readFile(
  path.join(webRoot, "app", "components", "asset-cache-registrar.tsx"),
  "utf8",
);
const pageSource = await fs.readFile(path.join(webRoot, "app", "page.tsx"), "utf8");
const shellSource = await fs.readFile(path.join(webRoot, "app", "original-client-shell.tsx"), "utf8");
const vercelBuildSource = await fs.readFile(
  path.join(scriptDir, "vercel-build.sh"),
  "utf8",
);

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

test("the large Bevy runtime and speculative scene assets stay off the first-playable path", () => {
  assert.match(pageSource, /const shouldBootBevyRuntime = screen === "game" && assetFirstPlayable/);
  assert.match(pageSource, /if \(screen !== "game" \|\| !sceneBlueprintRequest/);
  assert.match(pageSource, /controller\.abort\("scene-request-superseded"\)/);
  assert.match(shellSource, /concurrency: 8/);
  assert.match(shellSource, /concurrency: 4/);
  assert.match(shellSource, /const visualReady = readiness\.visualReady/);
  assert.doesNotMatch(shellSource, /for \(const url of urls\)/);
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

test("remote production builds reuse validated bootstrap images without original UI source", async () => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-login-bootstrap-"));
  const outputRoot = path.join(tempRoot, "bootstrap", "login");
  await fs.mkdir(outputRoot, { recursive: true });

  try {
    for (const width of [768, 1024]) {
      await fs.copyFile(
        path.join(webRoot, "public", "bootstrap", "login", `chrsel-0-${width}.webp`),
        path.join(outputRoot, `chrsel-0-${width}.webp`),
      );
    }

    const { stdout } = await execFileAsync(process.execPath, [loginBackgroundScript], {
      env: {
        ...process.env,
        MIR2_ORIGINAL_ASSET_MANIFEST_MODE: "remote-release",
        MIR2_LOGIN_BACKGROUND_SOURCE: path.join(tempRoot, "missing", "ChrSel", "0.png"),
        MIR2_LOGIN_BACKGROUND_OUTPUT_ROOT: outputRoot,
      },
      maxBuffer: 1024 * 1024,
    });
    const report = JSON.parse(stdout);
    assert.equal(report.ok, true);
    assert.equal(report.mode, "prebuilt");
    assert.deepEqual(report.generated.map((entry) => entry.width), [768, 1024]);
  } finally {
    await fs.rm(tempRoot, { recursive: true, force: true });
  }
});

test("prebuilt Vercel releases skip Rust toolchain installation", () => {
  const prebuiltGuard = vercelBuildSource.indexOf(
    'if [ "${MIR2_USE_PREBUILT_BEVY_RUNTIME:-0}" = "1" ]; then',
  );
  const rustupInstall = vercelBuildSource.indexOf(
    'rustup toolchain install "$TOOLCHAIN"',
  );
  assert.ok(prebuiltGuard >= 0, "prebuilt runtime guard is present");
  assert.ok(rustupInstall > prebuiltGuard, "prebuilt runtime guard runs before rustup");
  assert.match(
    vercelBuildSource.slice(prebuiltGuard, rustupInstall),
    /skipping Rust toolchain installation/,
  );
});

test("Vercel reuses a validated immutable original-asset manifest", async () => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-original-manifest-"));
  const outputPath = path.join(tempRoot, "original-asset-manifest.generated.json");
  const remoteRelease = "https://127.0.0.1:1/mir2/v/fixture-v1/remote-asset-release.json";
  await fs.writeFile(
    outputPath,
    JSON.stringify({
      schemaVersion: 1,
      kind: "mir2-original-asset-manifest",
      generatedAt: "2026-08-05T00:00:00.000Z",
      collectionMode: "remote-release",
      assetHash: "a".repeat(64),
      stats: {
        assetCount: 1,
        originalMapPngCount: 1,
        originalUiPngCount: 0,
        totalBytes: 7,
      },
      remoteRelease: {
        source: remoteRelease,
        version: "fixture-v1",
        assetBaseUrl: "https://127.0.0.1:1/mir2/v/fixture-v1",
        objectPrefix: "mir2/v/fixture-v1",
        fileCount: 1,
        missingCount: 0,
      },
      assets: {
        "/original-map/fixture.png": {
          size: 7,
          source: "original-map",
          sha256: "b".repeat(64),
        },
      },
    }),
  );

  try {
    const { stdout } = await execFileAsync(process.execPath, [originalAssetManifestScript], {
      env: {
        ...process.env,
        MIR2_ASSET_VERSION: "fixture-v1",
        MIR2_ORIGINAL_ASSET_MANIFEST_MODE: "remote-release",
        MIR2_ORIGINAL_ASSET_REMOTE_RELEASE: remoteRelease,
        MIR2_ORIGINAL_ASSET_MANIFEST_PATH: outputPath,
        MIR2_REUSE_ORIGINAL_ASSET_MANIFEST: "1",
      },
      maxBuffer: 1024 * 1024,
    });
    const report = JSON.parse(stdout);
    assert.equal(report.ok, true);
    assert.equal(report.reused, true);
    assert.equal(report.assetCount, 1);
  } finally {
    await fs.rm(tempRoot, { recursive: true, force: true });
  }
});
