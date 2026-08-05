import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

import { normalizeWorkerUploadPath } from "./asset-pipeline/upload-safety.mjs";

function loadTypeScriptModule(url) {
  const sourcePath = fileURLToPath(url);
  const compiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: sourcePath,
    reportDiagnostics: true,
  });
  const errors = (compiled.diagnostics ?? []).filter(
    (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
  );
  assert.deepEqual(errors, []);
  const loadedModule = { exports: {} };
  new Function("exports", "module", compiled.outputText)(loadedModule.exports, loadedModule);
  return loadedModule.exports;
}

const { normalizeAssetReleaseCapabilities } = loadTypeScriptModule(
  new URL("../lib/asset-release-capabilities.ts", import.meta.url),
);
const { createBevyRuntimeUrls } = loadTypeScriptModule(
  new URL("../lib/bevy-runtime-url.ts", import.meta.url),
);
const bulkUploadWorker = loadTypeScriptModule(
  new URL("../../../infra/cloudflare/mir2-r2-bulk-upload/src/index.ts", import.meta.url),
).default;

test("a full pack is enabled only by a verified content-addressed release capability", () => {
  const valid = normalizeAssetReleaseCapabilities({
    releaseId: "release-1",
    crystalFullPack: {
      enabled: true,
      verified: true,
      indexPath: "/generated/crystal-packs/full/index.json",
      contentHash: "f".repeat(64),
      libraryCount: 1440,
      pageCount: 4446,
    },
  });
  assert.equal(valid.crystalFullPack.enabled, true);
  assert.equal(valid.crystalFullPack.contentHash, "f".repeat(64));

  assert.equal(
    normalizeAssetReleaseCapabilities({
      crystalFullPack: {
        enabled: true,
        verified: false,
        indexPath: "/generated/crystal-packs/full/index.json",
        contentHash: "f".repeat(64),
      },
    }).crystalFullPack.enabled,
    false,
  );
  assert.equal(
    normalizeAssetReleaseCapabilities({
      crystalFullPack: {
        enabled: true,
        verified: true,
        indexPath: "https://attacker.test/index.json",
        contentHash: "f".repeat(64),
      },
    }).crystalFullPack.enabled,
    false,
  );
});

test("a map atlas is enabled only when its verified manifest path matches its content hash", () => {
  const hash = "7".repeat(64);
  const valid = normalizeAssetReleaseCapabilities({
    mapAtlas: {
      enabled: true,
      verified: true,
      manifestPath: `/generated/map-atlas/manifest.${hash}.json`,
      contentHash: hash,
      pageCount: 57,
      maxPageBytes: 468963,
    },
  });
  assert.equal(valid.mapAtlas.enabled, true);
  assert.equal(valid.mapAtlas.manifestPath, `/generated/map-atlas/manifest.${hash}.json`);

  for (const manifestPath of [
    "/generated/map-atlas/manifest.json",
    `/generated/map-atlas/manifest.${"8".repeat(64)}.json`,
    `https://attacker.test/manifest.${hash}.json`,
  ]) {
    assert.equal(
      normalizeAssetReleaseCapabilities({
        mapAtlas: { enabled: true, verified: true, manifestPath, contentHash: hash },
      }).mapAtlas.enabled,
      false,
    );
  }
});

test("Bevy runtime URLs use an immutable version path instead of a cache-busting query", () => {
  assert.deepEqual(createBevyRuntimeUrls("bevy-abc/123", "webgpu"), {
    moduleUrl: "/bevy-runtime/v/bevy-abc%2F123/pkg-webgpu/mir2_bevy_runtime.js",
    wasmUrl: "/bevy-runtime/v/bevy-abc%2F123/pkg-webgpu/mir2_bevy_runtime_bg.wasm",
  });
  assert.equal(createBevyRuntimeUrls("", "webgl2").moduleUrl.includes("?"), false);
});

test("R2 upload Worker paths cannot escape the configured origin", () => {
  assert.equal(normalizeWorkerUploadPath(undefined), "/upload");
  assert.equal(normalizeWorkerUploadPath("/temporary/map-atlas-upload"), "/temporary/map-atlas-upload");
  for (const unsafePath of [
    "//attacker.test/upload",
    "/safe//attacker.test",
    "/upload?redirect=https://attacker.test",
    "/upload#fragment",
    "/../upload",
    "https://attacker.test/upload",
  ]) {
    assert.throws(() => normalizeWorkerUploadPath(unsafePath), /Unsafe R2 upload Worker path/);
  }
});

test("R2 upload Worker persists the private gzip representation header", async () => {
  let stored = null;
  const request = new Request("https://assets.example.test/upload?key=mir2%2Fv%2Frelease%2Fruntime.wasm", {
    method: "PUT",
    headers: {
      authorization: "Bearer fixture-secret",
      "content-type": "application/wasm",
      "content-length": "4",
      "x-mir2-content-encoding": "gzip",
    },
    body: new Uint8Array([1, 2, 3, 4]),
    duplex: "half",
  });
  const response = await bulkUploadWorker.fetch(request, {
    MIR2_R2_UPLOAD_SECRET: "fixture-secret",
    MIR2_ASSETS: {
      async put(key, body, options) {
        stored = { key, body, options };
      },
    },
  });

  assert.equal(response.status, 200);
  assert.equal(stored.key, "mir2/v/release/runtime.wasm");
  assert.equal(stored.options.httpMetadata.contentEncoding, "gzip");
});

test("production release and Next routing expose the pinned capability and immutable runtime", () => {
  const productionConfig = JSON.parse(
    readFileSync(new URL("../../../config/production-web-assets.json", import.meta.url), "utf8"),
  );
  assert.equal(productionConfig.fullCrystalPack.enabled, true);
  assert.equal(productionConfig.fullCrystalPack.verified, true);
  assert.match(productionConfig.fullCrystalPack.contentHash, /^[a-f0-9]{64}$/);
  assert.ok(productionConfig.browserFallbackBaseUrls.length > 0);
  assert.match(productionConfig.browserFallbackBaseUrls[0], /\/hotlink-ok\/mir2\/v\//);
  assert.ok(
    productionConfig.browserFallbackBaseUrls.every((baseUrl) =>
      baseUrl.startsWith("https://"),
    ),
  );
  assert.equal(productionConfig.mapAtlas.enabled, true);
  assert.equal(productionConfig.mapAtlas.verified, true);
  assert.match(
    productionConfig.mapAtlas.manifestPath,
    new RegExp(`manifest\\.${productionConfig.mapAtlas.contentHash}\\.json$`),
  );
  assert.ok(productionConfig.mapAtlas.maxPageBytes < 512 * 1024);

  const routeSource = readFileSync(
    new URL("../app/api/asset-manifest/route.ts", import.meta.url),
    "utf8",
  );
  const nextConfigSource = readFileSync(new URL("../next.config.ts", import.meta.url), "utf8");
  const pageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
  const shellSource = readFileSync(
    new URL("../app/original-client-shell.tsx", import.meta.url),
    "utf8",
  );
  const assetWorkerSource = readFileSync(
    new URL("../../../infra/cloudflare/mir2-r2-asset-cache/src/index.ts", import.meta.url),
    "utf8",
  );
  const domainProxySource = readFileSync(
    new URL("../../../infra/cloudflare/mir2-domain-proxy/src/index.ts", import.meta.url),
    "utf8",
  );
  const uploadScriptSource = readFileSync(new URL("./upload-r2-assets.mjs", import.meta.url), "utf8");
  const releaseWorkflowSource = readFileSync(
    new URL("../../../../.github/workflows/web-assets-r2-release.yml", import.meta.url),
    "utf8",
  );
  assert.match(routeSource, /capabilities,/);
  assert.match(routeSource, /MIR2_PINNED_CRYSTAL_FULL_PACK_CONTENT_HASH/);
  assert.match(routeSource, /MIR2_PINNED_MAP_ATLAS_CONTENT_HASH/);
  assert.match(routeSource, /browserFallbackBaseUrls/);
  assert.match(nextConfigSource, /\/bevy-runtime\/v\/:version/);
  assert.match(nextConfigSource, /max-age=31536000, immutable/);
  assert.match(pageSource, /const BEVY_RUNTIME_VERSION = bevyRuntimeVersion\.version \|\| "local"/);
  assert.doesNotMatch(pageSource, /bevyRuntimeVersion\.version, process\.env\.NEXT_PUBLIC_VERCEL_GIT_COMMIT_SHA/);
  assert.match(shellSource, /return manifestPath \? loadMapAtlasIndex\(manifestPath\) : null/);
  assert.match(assetWorkerSource, /const HOTLINK_SAFE_PREFIX = "hotlink-ok\/"/);
  assert.match(assetWorkerSource, /key = key\.slice\(HOTLINK_SAFE_PREFIX\.length\)/);
  assert.match(assetWorkerSource, /cacheUrl\(url, key\)/);
  assert.match(uploadScriptSource, /"X-Mir2-Content-Encoding": upload\.contentEncoding/);
  assert.match(domainProxySource, /representation", "stored-gzip-v1"/);
  assert.match(domainProxySource, /runtime_storage_encoding_missing/);
  assert.match(domainProxySource, /encodeBody: "manual"/);
  assert.match(releaseWorkflowSource, /Deploy authenticated R2 upload Worker/);
  assert.match(releaseWorkflowSource, /mir2-r2-bulk-upload\/wrangler\.jsonc/);
  assert.match(releaseWorkflowSource, /npm run runtime:r2:build/);
  assert.match(releaseWorkflowSource, /MIR2_BEVY_RUNTIME_VERSION: runtime\.version/);
  assert.doesNotMatch(releaseWorkflowSource, /delete config\.routes/);
});
