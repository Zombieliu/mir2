import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { constants as zlibConstants, gzipSync } from "node:zlib";
import { readImmutableBevyRuntimeRelease } from "./lib/bevy-runtime-release-files.mjs";

export const ASSET_VERSION = "20260818-main-97887dcd";
export const OBJECT_PREFIX = `mir2/v/${ASSET_VERSION}`;
export const ASSET_BASE_URL = `https://assets.mir2.obelisk.build/${OBJECT_PREFIX}`;
const CACHE_CONTROL = "public, max-age=31536000, immutable";
const PACKAGE_IDS = ["webgpu", "webgl2", "webgl2-shared"];
const hash = bytes => createHash("sha256").update(bytes).digest("hex");

function exactKeys(value, keys, label) {
  assert.ok(value && typeof value === "object" && !Array.isArray(value), `${label}: expected object`);
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), `${label}: unexpected or missing fields`);
}

/** Verify the reviewed file bytes before adopting any version or local paths. */
export function verifyPinnedRuntime({ webRoot, runtimeVersion, manifestSha256 }) {
  assert.match(runtimeVersion ?? "", /^bevy-[0-9a-f]{16}$/, "invalid expected runtime version");
  assert.match(manifestSha256 ?? "", /^[0-9a-f]{64}$/, "invalid expected manifest SHA256");
  const manifestPath = path.join(webRoot, "lib", "generated", "bevy_runtime_version.json");
  assert.equal(hash(fs.readFileSync(manifestPath)), manifestSha256, "reviewed runtime manifest SHA256 mismatch");
  const pinned = readImmutableBevyRuntimeRelease({ webRoot, manifestPath });
  assert.equal(pinned.normalized.version, runtimeVersion, "reviewed runtime version mismatch");
  assert.equal(pinned.normalized.schemaVersion, 2, "three-package runtime requires schemaVersion 2");
  assert.deepEqual(pinned.normalized.packages.map(pkg => pkg.id), PACKAGE_IDS, "expected all three runtime packages");
  assert.equal(pinned.files.length, 6, "expected exactly six runtime leaves");
  return pinned;
}

/** Reject uploader side channels and any path outside the six reviewed leaves. */
export function verifyRuntimeUploadPlan(pinned, release) {
  exactKeys(release, ["schemaVersion", "kind", "publishReleaseManifest", "version", "generatedAt", "assetBaseUrl",
    "objectPrefix", "cacheControl", "bevyRuntime", "files"], "runtime release");
  assert.equal(release.schemaVersion, 1);
  assert.equal(release.kind, "mir2-bevy-runtime-r2-release");
  assert.equal(release.publishReleaseManifest, false, "asset release manifest publication is forbidden");
  assert.equal(release.version, ASSET_VERSION);
  assert.equal(release.objectPrefix, OBJECT_PREFIX);
  assert.equal(release.assetBaseUrl, ASSET_BASE_URL);
  assert.equal(release.cacheControl, CACHE_CONTROL);
  assert.ok(typeof release.generatedAt === "string" && Number.isFinite(Date.parse(release.generatedAt)), "invalid generatedAt");
  const expected = new Map(pinned.files.map(file => [file.relativePath, file]));
  assert.equal(release.files?.length, 6, "upload plan must contain exactly six leaves");
  let logicalBytes = 0, storageBytes = 0;
  for (const file of release.files) {
    const source = expected.get(file.relativePath);
    assert.ok(source, "unknown or duplicate runtime upload leaf");
    expected.delete(file.relativePath);
    const wasm = file.relativePath.endsWith(".wasm");
    exactKeys(file, ["path", "relativePath", "stagePath", "objectKey", "size", "sha256", "contentType",
      "cacheControl", "sources", ...(wasm ? ["contentEncoding", "encodedSize", "encodedSha256"] : [])], "upload leaf");
    assert.equal(file.path, `/${source.relativePath}`);
    assert.equal(file.stagePath, source.localPath, "upload source must be the immutable reviewed file");
    assert.equal(file.objectKey, `${OBJECT_PREFIX}/${source.relativePath}`, "runtime object key escapes the reviewed prefix");
    assert.equal(file.sha256, source.sha256);
    assert.equal(file.contentType, wasm ? "application/wasm" : "text/javascript; charset=utf-8");
    assert.equal(file.cacheControl, CACHE_CONTROL);
    assert.deepEqual(file.sources, ["bevy-runtime"]);
    const bytes = fs.readFileSync(source.localPath);
    assert.equal(hash(bytes), source.sha256, "runtime bytes changed after pinned validation");
    assert.equal(file.size, bytes.length);
    logicalBytes += bytes.length;
    if (wasm) {
      assert.ok(bytes.subarray(0, 4).equals(Buffer.from([0, 97, 115, 109])), "invalid WASM magic");
      const encoded = gzipSync(bytes, { level: zlibConstants.Z_BEST_COMPRESSION, mtime: 0 });
      assert.equal(file.contentEncoding, "gzip");
      assert.equal(file.encodedSize, encoded.length);
      assert.equal(file.encodedSha256, hash(encoded), "encoded runtime SHA256 mismatch");
      storageBytes += encoded.length;
    } else storageBytes += bytes.length;
  }
  assert.equal(expected.size, 0, "missing runtime upload leaves");
  assert.deepEqual(release.bevyRuntime, { enabled: true, version: pinned.normalized.version,
    schemaVersion: 2, packages: pinned.normalized.packages, contentEncoding: "gzip", logicalBytes, storageBytes });
  return release.files;
}

function verifyRemoteHeaders(file, response) {
  assert.equal(response.status, 200, `published runtime leaf is unavailable: ${file.objectKey} (HTTP ${response.status})`);
  assert.equal(response.headers.get("content-type"), file.contentType, `published runtime content type mismatch: ${file.objectKey}`);
  // JS may be compressed by HTTP transport; it must remain an identity object in R2.
  assert.equal(response.headers.get("x-mir2-storage-content-encoding"), file.contentEncoding ?? null,
    `runtime storage encoding mismatch: ${file.objectKey}`);
  if (file.contentEncoding === "gzip")
    assert.equal(response.headers.get("content-encoding"), "gzip", "published WASM must use gzip");
}

const requestLeaf = (file, method, fetchImpl) => fetchImpl(`${ASSET_BASE_URL}/${file.relativePath}`,
  { method, cache: "no-store", redirect: "error", signal: AbortSignal.timeout(60_000) });

async function verifyRemoteLeaf(file, fetchImpl) {
  const response = await requestLeaf(file, "GET", fetchImpl);
  verifyRemoteHeaders(file, response);
  const bytes = Buffer.from(await response.arrayBuffer());
  assert.equal(hash(bytes), file.sha256, `published runtime SHA256 mismatch: ${file.objectKey}`);
  if (file.contentEncoding === "gzip")
    assert.ok(bytes.subarray(0, 4).equals(Buffer.from([0, 97, 115, 109])), "published WASM magic mismatch");
}

/** A retry skips existing leaves only after verifying their actual decoded bytes. */
export async function prepareMissingUploadPlan(pinned, release, { fetchImpl = globalThis.fetch } = {}) {
  const files = verifyRuntimeUploadPlan(pinned, release);
  const missing = [];
  for (const file of files) {
    const response = await requestLeaf(file, "HEAD", fetchImpl);
    if (response.status === 404) { missing.push(file); continue; }
    verifyRemoteHeaders(file, response);
    await verifyRemoteLeaf(file, fetchImpl);
  }
  return { ...release, files: missing, bevyRuntime: { ...release.bevyRuntime,
    logicalBytes: missing.reduce((sum, file) => sum + file.size, 0),
    storageBytes: missing.reduce((sum, file) => sum + (file.encodedSize ?? file.size), 0) } };
}

export async function checkRemoteLeaves(files, { fetchImpl = globalThis.fetch } = {}) {
  for (const file of files) await verifyRemoteLeaf(file, fetchImpl);
}

async function main() {
  const args = process.argv.slice(2);
  const allowed = new Set(["--releaseManifest", "--prepareMissing", "--verifyRemote"]);
  for (let index = 0; index < args.length; index++) {
    assert.ok(allowed.has(args[index]), `unknown argument: ${args[index]}`);
    if (["--releaseManifest", "--prepareMissing"].includes(args[index]))
      assert.ok(args[++index] && !args[index].startsWith("--"), "missing manifest path");
  }
  assert.ok(!(args.includes("--prepareMissing") && args.includes("--verifyRemote")), "choose one remote check");
  const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const pinned = verifyPinnedRuntime({ webRoot, runtimeVersion: process.env.EXPECTED_RUNTIME_VERSION,
    manifestSha256: process.env.EXPECTED_RUNTIME_MANIFEST_SHA256 });
  const manifestIndex = args.indexOf("--releaseManifest");
  let missingCount = null;
  if (manifestIndex >= 0) {
    const releasePath = path.resolve(args[manifestIndex + 1]);
    const release = JSON.parse(fs.readFileSync(releasePath, "utf8"));
    const files = verifyRuntimeUploadPlan(pinned, release);
    if (args.includes("--prepareMissing")) {
      const outputPath = path.resolve(args[args.indexOf("--prepareMissing") + 1]);
      assert.notEqual(outputPath, releasePath, "retain the complete six-leaf release manifest");
      const missing = await prepareMissingUploadPlan(pinned, release);
      fs.writeFileSync(outputPath, `${JSON.stringify(missing, null, 2)}\n`, { flag: "wx" });
      missingCount = missing.files.length;
      if (process.env.GITHUB_OUTPUT) fs.appendFileSync(process.env.GITHUB_OUTPUT, `missing_count=${missingCount}\n`);
    } else if (args.includes("--verifyRemote")) await checkRemoteLeaves(files);
  } else assert.equal(args.length, 0, "remote validation requires a release manifest");
  console.log(JSON.stringify({ ok: true, runtimeVersion: pinned.normalized.version, fileCount: 6,
    assetVersion: ASSET_VERSION, objectPrefix: OBJECT_PREFIX, missingCount, check: args.includes("--prepareMissing") ? "missing-subset"
      : args.includes("--verifyRemote") ? "published" : manifestIndex >= 0 ? "upload-plan" : "pinned" }));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
