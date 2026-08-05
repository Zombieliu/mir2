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
