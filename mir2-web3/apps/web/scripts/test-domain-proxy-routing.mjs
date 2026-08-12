import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { gunzipSync, gzipSync } from "node:zlib";

import ts from "typescript";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const sourcePath = path.resolve(
  scriptDir,
  "..",
  "..",
  "..",
  "infra",
  "cloudflare",
  "mir2-domain-proxy",
  "src",
  "index.ts",
);
const source = await fs.readFile(sourcePath, "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ESNext,
    target: ts.ScriptTarget.ES2022,
  },
  fileName: sourcePath,
  reportDiagnostics: true,
});
const errors = (transpiled.diagnostics ?? []).filter((diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error);
assert.deepEqual(errors, [], "domain proxy TypeScript transpiles without syntax errors");

const temporaryModule = path.join(scriptDir, `.domain-proxy-routing-${process.pid}-${Date.now()}.mjs`);
try {
  await fs.writeFile(temporaryModule, transpiled.outputText, "utf8");
  const worker = await import(`${pathToFileURL(temporaryModule).href}?v=${Date.now()}`);
  for (const assetPath of [
    "/original-ui/Title/30.png",
    "/original-map/WemadeMir2/Tiles/1000.png",
    "/generated/original-map-blend/WemadeMir2/Objects/1.png",
    "/generated/map-atlas/manifest.json",
    "/generated/crystal-packs/full/index.json",
    "/generated/crystal-packs/full/libraries/entities/example.json",
    "/generated/crystal-packs/full/pages/aa/example.png",
    "/bevy-runtime/v/bevy-9a5cbecc8f85ff75/pkg-webgpu/mir2_bevy_runtime.js",
    "/bevy-runtime/v/bevy-9a5cbecc8f85ff75/pkg-webgl2/mir2_bevy_runtime_bg.wasm",
  ]) {
    assert.equal(worker.isStaticAssetRequest(new URL(`https://mir2.example${assetPath}`)), true, assetPath);
  }
  assert.equal(
    worker.bevyRuntimeObjectKeyForPath(
      "/bevy-runtime/v/bevy-9a5cbecc8f85ff75/pkg-webgl2/mir2_bevy_runtime_bg.wasm",
      "mir2/v/release",
      "bevy-9a5cbecc8f85ff75",
    ),
    "mir2/v/release/bevy-runtime/v/bevy-9a5cbecc8f85ff75/pkg-webgl2/mir2_bevy_runtime_bg.wasm",
  );
  assert.equal(
    worker.bevyRuntimeObjectKeyForPath(
      "/bevy-runtime/v/stale/pkg-webgl2/mir2_bevy_runtime_bg.wasm",
      "mir2/v/release",
      "bevy-9a5cbecc8f85ff75",
    ),
    "",
    "a stale immutable runtime URL must never receive bytes from the current release",
  );
  assert.equal(
    worker.overlayFallbackObjectKey(
      "mir2/v/overlay/original-map/WemadeMir2/Tiles/1000.png",
      "mir2/v/overlay",
      "mir2/v/full-pack",
    ),
    "mir2/v/full-pack/original-map/WemadeMir2/Tiles/1000.png",
  );
  assert.equal(
    worker.overlayFallbackObjectKey(
      "mir2/v/overlay/bevy-runtime/v/bevy-9a5cbecc8f85ff75/pkg-webgpu/mir2_bevy_runtime.js",
      "mir2/v/overlay",
      "mir2/v/full-pack",
    ),
    "",
    "the overlay must not mix Bevy runtime versions",
  );
  for (const applicationPath of ["/", "/api/asset-manifest", "/ws", "/generated/not-an-asset/file.json"]) {
    assert.equal(worker.isStaticAssetRequest(new URL(`https://mir2.example${applicationPath}`)), false, applicationPath);
  }

  const wasmBytes = Uint8Array.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
  const storedWasmBytes = gzipSync(wasmBytes, { mtime: 0 });
  const cachedResponses = new Map();
  const cacheWrites = [];
  let r2Reads = 0;
  const previousCaches = globalThis.caches;
  Object.defineProperty(globalThis, "caches", {
    configurable: true,
    value: {
      default: {
        async match(request) {
          return cachedResponses.get(request.url)?.clone() ?? null;
        },
        async put(request, response) {
          cacheWrites.push(request.url);
          const bytes = await response.arrayBuffer();
          cachedResponses.set(
            request.url,
            new Response(bytes, {
              headers: response.headers,
              status: response.status,
              statusText: response.statusText,
            }),
          );
        },
      },
    },
  });

  try {
    const waitUntilPromises = [];
    const runtimeUrl =
      "https://mir2.example/bevy-runtime/v/bevy-9a5cbecc8f85ff75/pkg-webgpu/mir2_bevy_runtime_bg.wasm";
    const env = {
      ASSET_ORIGIN_URL: "https://assets.example/mir2/v/{version}",
      MIR2_ASSET_OBJECT_PREFIX: "mir2/v/release",
      MIR2_ASSET_VERSION: "release",
      MIR2_BEVY_RUNTIME_VERSION: "bevy-9a5cbecc8f85ff75",
      MIR2_ASSETS: {
        async get() {
          r2Reads += 1;
          return {
            body: new Blob([storedWasmBytes]).stream(),
            customMetadata: { sha256: "a".repeat(64) },
            httpEtag: '"runtime-etag"',
            size: storedWasmBytes.byteLength,
            httpMetadata: {
              cacheControl: "public, max-age=31536000, immutable",
              contentEncoding: "gzip",
              contentType: "application/wasm",
            },
          };
        },
      },
    };
    const ctx = {
      waitUntil(promise) {
        waitUntilPromises.push(promise);
      },
    };

    const firstRuntimeResponse = await worker.default.fetch(new Request(runtimeUrl), env, ctx);
    assert.equal(firstRuntimeResponse.status, 200);
    assert.equal(firstRuntimeResponse.headers.get("content-encoding"), "gzip");
    assert.equal(firstRuntimeResponse.headers.get("content-length"), String(storedWasmBytes.byteLength));
    assert.match(firstRuntimeResponse.headers.get("cache-control"), /(?:^|,\s*)no-transform(?:,|$)/);
    assert.equal(firstRuntimeResponse.headers.get("x-mir2-storage-content-encoding"), "gzip");
    assert.equal(firstRuntimeResponse.headers.get("x-mir2-runtime-transport"), "stored-gzip-no-transform");
    assert.equal(WebAssembly.validate(gunzipSync(await firstRuntimeResponse.arrayBuffer())), true);
    await Promise.all(waitUntilPromises);
    assert.equal(cacheWrites.length, 0, "encoded WASM must bypass caches.default");
    assert.equal(r2Reads, 1);

    const secondRuntimeResponse = await worker.default.fetch(new Request(runtimeUrl), env, {
      waitUntil() {},
    });
    assert.equal(secondRuntimeResponse.headers.get("x-mir2-edge-cache"), "MISS");
    assert.equal(secondRuntimeResponse.headers.get("content-encoding"), "gzip");
    assert.equal(secondRuntimeResponse.headers.get("x-mir2-runtime-transport"), "stored-gzip-no-transform");
    assert.equal(WebAssembly.validate(gunzipSync(await secondRuntimeResponse.arrayBuffer())), true);
    assert.equal(r2Reads, 2, "each WASM request should read the immutable compressed R2 object");

    const overlayReads = [];
    const fallbackBody = Uint8Array.from([9, 8, 7]);
    const overlayWaitUntil = [];
    const overlayResponse = await worker.default.fetch(
      new Request("https://mir2.example/original-ui/Monster/012/223.png"),
      {
        ...env,
        MIR2_ASSET_OBJECT_PREFIX: "mir2/v/overlay-v2",
        MIR2_ASSET_VERSION: "overlay-v2",
        MIR2_FALLBACK_OBJECT_PREFIX: "mir2/v/full-v1",
        MIR2_ASSETS: {
          async get(key) {
            overlayReads.push(key);
            if (key !== "mir2/v/full-v1/original-ui/Monster/012/223.png") return null;
            return {
              body: new Blob([fallbackBody]).stream(),
              customMetadata: {},
              httpEtag: '"fallback-etag"',
              size: fallbackBody.byteLength,
              httpMetadata: {
                cacheControl: "public, max-age=31536000, immutable",
                contentType: "image/png",
              },
            };
          },
        },
      },
      { waitUntil(promise) { overlayWaitUntil.push(promise); } },
    );
    assert.equal(overlayResponse.status, 200);
    assert.equal(
      overlayResponse.headers.get("x-mir2-fallback-object-key"),
      "mir2/v/full-v1/original-ui/Monster/012/223.png",
    );
    assert.deepEqual(overlayReads, [
      "mir2/v/overlay-v2/original-ui/Monster/012/223.png",
      "mir2/v/full-v1/original-ui/Monster/012/223.png",
    ]);
    assert.deepEqual(new Uint8Array(await overlayResponse.arrayBuffer()), fallbackBody);
    await Promise.all(overlayWaitUntil);
  } finally {
    Object.defineProperty(globalThis, "caches", {
      configurable: true,
      value: previousCaches,
    });
  }
  console.log("domain proxy full-pack routing passed");
} finally {
  await fs.rm(temporaryModule, { force: true });
}
