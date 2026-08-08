import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const manifestPath = path.join(webRoot, "lib", "generated", "bevy_runtime_version.json");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const filesByPath = new Map(manifest.files.map((entry) => [entry.path, entry]));

const MAX_WASM_BYTES = 31 * 1024 * 1024;
const MAX_GZIP_BYTES = 7 * 1024 * 1024;
const MAX_WRAPPER_BYTES = 200 * 1024;

const packages = [
  { backend: "webgpu", directory: "pkg-webgpu" },
  { backend: "webgl2", directory: "pkg-webgl2" },
];

for (const runtimePackage of packages) {
  const relativeWasmPath = `public/bevy-runtime/${runtimePackage.directory}/mir2_bevy_runtime_bg.wasm`;
  const relativeJsPath = `public/bevy-runtime/${runtimePackage.directory}/mir2_bevy_runtime.js`;
  const wasm = readAndVerifyManifestFile(relativeWasmPath);
  const js = readAndVerifyManifestFile(relativeJsPath);
  const gzipBytes = gzipSync(wasm, { level: 9 }).byteLength;

  assert.equal(WebAssembly.validate(wasm), true, `${runtimePackage.backend} WASM must validate`);
  assert.ok(
    wasm.byteLength <= MAX_WASM_BYTES,
    `${runtimePackage.backend} WASM ${wasm.byteLength} exceeds ${MAX_WASM_BYTES}`,
  );
  assert.ok(
    gzipBytes <= MAX_GZIP_BYTES,
    `${runtimePackage.backend} gzip ${gzipBytes} exceeds ${MAX_GZIP_BYTES}`,
  );
  assert.ok(
    js.byteLength <= MAX_WRAPPER_BYTES,
    `${runtimePackage.backend} JS wrapper ${js.byteLength} exceeds ${MAX_WRAPPER_BYTES}`,
  );

  console.log(
    `${runtimePackage.backend}: raw=${wasm.byteLength} gzip=${gzipBytes} wrapper=${js.byteLength}`,
  );
}

assert.equal(
  filesByPath.has("public/bevy-runtime/pkg/mir2_bevy_runtime_bg.wasm"),
  false,
  "the legacy WebGL2 mirror must stay out of the runtime manifest and deployments",
);

console.log(`bevy runtime budget passed (${manifest.version})`);

function readAndVerifyManifestFile(relativePath) {
  const manifestEntry = filesByPath.get(relativePath);
  assert.ok(manifestEntry, `runtime manifest is missing ${relativePath}`);
  const absolutePath = path.join(webRoot, ...relativePath.split("/"));
  const bytes = fs.readFileSync(absolutePath);
  assert.equal(sha256(bytes), manifestEntry.sha256, `${relativePath} hash differs from manifest`);
  return bytes;
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}
