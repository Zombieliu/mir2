import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { constants as zlibConstants, gzipSync } from "node:zlib";
import { computeBevyRuntimeVersion } from "./lib/bevy-runtime-version.mjs";
import { ASSET_BASE_URL, ASSET_VERSION, OBJECT_PREFIX, checkRemoteLeaves, prepareMissingUploadPlan,
  verifyPinnedRuntime, verifyRuntimeUploadPlan } from "./verify-bevy-runtime-r2-upload.mjs";

const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const clone = value => JSON.parse(JSON.stringify(value));
const runFile = promisify(execFile);

async function fixture(t, { shared = true } = {}) {
  const tempParent = path.resolve(os.tmpdir());
  const webRoot = await fs.mkdtemp(path.join(tempParent, "mir2-runtime-r2-upload-"));
  t.after(async () => {
    assert.equal(path.dirname(path.resolve(webRoot)), tempParent);
    assert.ok(path.basename(webRoot).startsWith("mir2-runtime-r2-upload-"));
    await fs.rm(webRoot, { recursive: true, force: true });
  });
  const ids = ["webgpu", "webgl2", ...(shared ? ["webgl2-shared"] : [])];
  const packages = ids.map(id => ({ id, backend: id === "webgpu" ? "webgpu" : "webgl2", packageDir: `pkg-${id}`,
    questUiAbiVersion: shared && id !== "webgl2" ? 1 : 0,
    bagUiAbiVersion: shared && id !== "webgl2" ? 1 : 0, primarySharedUiCompiled: id === "webgl2-shared" }));
  const bytesByPath = new Map();
  for (const id of ids) {
    bytesByPath.set(`pkg-${id}/mir2_bevy_runtime.js`, Buffer.from(`export const backend = ${JSON.stringify(id)};\n`));
    bytesByPath.set(`pkg-${id}/mir2_bevy_runtime_bg.wasm`, Buffer.concat([Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]), Buffer.from(id)]));
  }
  const manifest = { schemaVersion: 2, version: "", packages,
    files: [...bytesByPath].map(([suffix, bytes]) => ({ path: `public/bevy-runtime/${suffix}`, sha256: hash(bytes) })) };
  manifest.version = computeBevyRuntimeVersion(manifest);
  const manifestBytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`);
  const versionDirectory = path.join(webRoot, "public", "bevy-runtime-releases", manifest.version);
  const manifestPath = path.join(webRoot, "lib", "generated", "bevy_runtime_version.json");
  await fs.mkdir(path.dirname(manifestPath), { recursive: true });
  await fs.mkdir(versionDirectory, { recursive: true });
  await fs.writeFile(manifestPath, manifestBytes);
  await fs.writeFile(path.join(versionDirectory, "runtime-manifest.json"), manifestBytes);
  for (const [suffix, bytes] of bytesByPath) {
    const target = path.join(versionDirectory, ...suffix.split("/"));
    await fs.mkdir(path.dirname(target), { recursive: true });
    await fs.writeFile(target, bytes);
  }
  const options = { webRoot, runtimeVersion: manifest.version, manifestSha256: hash(manifestBytes) };
  if (!shared) return { options };
  const pinned = verifyPinnedRuntime(options);
  const files = pinned.files.map(source => {
    const bytes = bytesByPath.get(source.relativePath.split("/").slice(3).join("/"));
    const wasm = source.relativePath.endsWith(".wasm");
    const encoded = wasm ? gzipSync(bytes, { level: zlibConstants.Z_BEST_COMPRESSION, mtime: 0 }) : null;
    return { path: `/${source.relativePath}`, relativePath: source.relativePath, stagePath: source.localPath,
      objectKey: `${OBJECT_PREFIX}/${source.relativePath}`, size: bytes.length, sha256: source.sha256,
      contentType: wasm ? "application/wasm" : "text/javascript; charset=utf-8",
      cacheControl: "public, max-age=31536000, immutable", sources: ["bevy-runtime"],
      ...(encoded ? { contentEncoding: "gzip", encodedSize: encoded.length, encodedSha256: hash(encoded) } : {}) };
  });
  const release = { schemaVersion: 1, kind: "mir2-bevy-runtime-r2-release", publishReleaseManifest: false,
    version: ASSET_VERSION, generatedAt: new Date(0).toISOString(), assetBaseUrl: ASSET_BASE_URL, objectPrefix: OBJECT_PREFIX,
    cacheControl: "public, max-age=31536000, immutable", bevyRuntime: { enabled: true, version: manifest.version,
      schemaVersion: 2, packages, contentEncoding: "gzip", logicalBytes: files.reduce((sum, f) => sum + f.size, 0),
      storageBytes: files.reduce((sum, f) => sum + (f.encodedSize ?? f.size), 0) }, files };
  return { pinned, options, release, bytesByPath };
}

test("accepts exactly three immutable packages and six gzip-bound upload leaves", async t => {
  const f = await fixture(t);
  assert.equal(verifyRuntimeUploadPlan(f.pinned, f.release).length, 6);
});

test("rejects a different manifest digest, version or incomplete package set", async t => {
  const f = await fixture(t);
  assert.throws(() => verifyPinnedRuntime({ ...f.options, manifestSha256: "0".repeat(64) }), /SHA256 mismatch/);
  assert.throws(() => verifyPinnedRuntime({ ...f.options, runtimeVersion: "bevy-0000000000000000" }), /version mismatch/);
  const two = await fixture(t, { shared: false });
  assert.throws(() => verifyPinnedRuntime(two.options), /all three runtime packages/);
});

test("rejects missing or altered immutable bytes", async t => {
  const f = await fixture(t);
  await fs.appendFile(f.pinned.files[0].localPath, "changed");
  assert.throws(() => verifyPinnedRuntime(f.options), /hash mismatch/);
  assert.throws(() => verifyRuntimeUploadPlan(f.pinned, f.release), /bytes changed/);
  await fs.rm(f.pinned.files[0].localPath);
  assert.throws(() => verifyPinnedRuntime(f.options), /missing or undeclared/);
});

test("rejects manifest publication, extra payloads, duplicate leaves and escaped object keys", async t => {
  const f = await fixture(t);
  for (const mutate of [
    release => { release.publishReleaseManifest = true; },
    release => { release.cas = {}; },
    release => { release.objectPrefix = "mir2/v/elsewhere"; },
    release => { release.files[0].objectKey = `${OBJECT_PREFIX}/remote-asset-release.json`; },
    release => { release.files[0].objectKey = `mir2/v/elsewhere/${release.files[0].relativePath}`; },
    release => { release.files[0].stagePath = path.join(f.options.webRoot, "unreviewed.js"); },
    release => { release.files[0] = clone(release.files[1]); },
    release => { release.files.pop(); },
  ]) {
    const changed = clone(f.release);
    mutate(changed);
    assert.throws(() => verifyRuntimeUploadPlan(f.pinned, changed));
  }
});

test("rejects missing gzip metadata and mismatched compressed bytes", async t => {
  const f = await fixture(t);
  for (const mutate of [
    file => { delete file.contentEncoding; },
    file => { file.encodedSha256 = "0".repeat(64); },
    file => { file.encodedSize++; },
    file => { file.sha256 = "0".repeat(64); },
  ]) {
    const changed = clone(f.release);
    mutate(changed.files.find(file => file.relativePath.endsWith(".wasm")));
    assert.throws(() => verifyRuntimeUploadPlan(f.pinned, changed));
  }
});

function publishedResponse(file, bytes, status = 200) {
  return new Response(bytes, { status, headers: { "content-type": file.contentType,
    ...(file.contentEncoding ? { "content-encoding": "gzip", "x-mir2-storage-content-encoding": "gzip" } : {}) } });
}

test("missing-object preflight retains all six new leaves and fails closed on lookup errors", async t => {
  const { pinned, release } = await fixture(t);
  const seen = [];
  const plan = await prepareMissingUploadPlan(pinned, release, { fetchImpl: async (url, options) => {
    seen.push(url); assert.equal(options.method, "HEAD"); assert.equal(options.redirect, "error");
    return new Response(null, { status: 404 });
  } });
  assert.deepEqual(seen, release.files.map(file => `${ASSET_BASE_URL}/${file.relativePath}`));
  assert.deepEqual(plan, release);
  for (const status of [301, 403, 500])
    await assert.rejects(prepareMissingUploadPlan(pinned, release, { fetchImpl: async () => new Response(null, { status }) }), /is unavailable/);
  await assert.rejects(prepareMissingUploadPlan(pinned, release, { fetchImpl: async () => { throw Error("network failure"); } }), /network failure/);
});

test("partial and complete retries skip existing leaves only after actual decoded hash checks", async t => {
  const f = await fixture(t);
  const before = clone(f.release);
  for (const existingCount of [3, 6]) {
    const verified = [];
    const plan = await prepareMissingUploadPlan(f.pinned, f.release, { fetchImpl: async (url, options) => {
      const index = f.release.files.findIndex(file => url === `${ASSET_BASE_URL}/${file.relativePath}`);
      const file = f.release.files[index];
      if (index >= existingCount) return new Response(null, { status: 404 });
      if (options.method === "HEAD") return publishedResponse(file, null);
      verified.push(file.objectKey);
      return publishedResponse(file, await fs.readFile(file.stagePath));
    } });
    assert.deepEqual(plan.files, f.release.files.slice(existingCount));
    assert.equal(plan.publishReleaseManifest, false);
    assert.equal(plan.bevyRuntime.logicalBytes, plan.files.reduce((sum, file) => sum + file.size, 0));
    assert.equal(plan.bevyRuntime.storageBytes, plan.files.reduce((sum, file) => sum + (file.encodedSize ?? file.size), 0));
    assert.deepEqual(verified, f.release.files.slice(0, existingCount).map(file => file.objectKey));
    const subsetPath = path.join(f.options.webRoot, `missing-${existingCount}.json`);
    await fs.writeFile(subsetPath, JSON.stringify(plan));
    const uploader = path.join(path.dirname(fileURLToPath(import.meta.url)), "upload-r2-assets.mjs");
    const { stdout } = await runFile(process.execPath, [uploader, "--manifest", subsetPath,
      "--driver", "worker", "--bucket", "mir2-web3-assets", "--includeReleaseManifest", "false", "--dryRun", "true"]);
    const uploadReport = JSON.parse(stdout.slice(stdout.indexOf("{\n")));
    assert.equal(uploadReport.uploadCount, 6 - existingCount, "the original uploader accepts only the derived missing subset");
  }
  assert.deepEqual(f.release, before, "retain the complete original six-leaf manifest");
});

test("resume rejects existing objects with wrong bytes, type or storage encoding", async t => {
  const f = await fixture(t);
  for (const corruption of ["bytes", "type", "storageEncoding"]) {
    await assert.rejects(prepareMissingUploadPlan(f.pinned, f.release, { fetchImpl: async (url, options) => {
      const file = f.release.files.find(file => url === `${ASSET_BASE_URL}/${file.relativePath}`);
      const response = publishedResponse(file, options.method === "HEAD" ? null
        : corruption === "bytes" ? "wrong" : await fs.readFile(file.stagePath));
      if (corruption === "type") response.headers.set("content-type", "text/plain");
      if (corruption === "storageEncoding") response.headers.set("x-mir2-storage-content-encoding", "br");
      return response;
    } }), /SHA256 mismatch|content type mismatch|storage encoding mismatch/);
  }
});

test("published verification checks every decoded hash and WASM storage encoding", async t => {
  const f = await fixture(t);
  const fetchImpl = async url => {
    const file = f.release.files.find(file => url === `${ASSET_BASE_URL}/${file.relativePath}`);
    return publishedResponse(file, await fs.readFile(file.stagePath));
  };
  await checkRemoteLeaves(f.release.files, { fetchImpl });
  await assert.rejects(checkRemoteLeaves(f.release.files, { fetchImpl: async url => {
    const response = await fetchImpl(url);
    return new Response("wrong", { headers: response.headers });
  } }), /SHA256 mismatch/);
  await assert.rejects(checkRemoteLeaves(f.release.files, { fetchImpl: async url => {
    const response = await fetchImpl(url);
    response.headers.delete("x-mir2-storage-content-encoding");
    return response;
  } }), /storage encoding mismatch/);
});

test("workflow binds the reviewed commit and isolates credentials behind a checked missing subset", async () => {
  const workflow = await fs.readFile(path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../../.github/workflows/bevy-runtime-r2-release.yml"), "utf8");
  assert.match(workflow, /workflow_dispatch:/);
  assert.doesNotMatch(workflow, /^  pull_request:/m);
  assert.match(workflow, /push:\s+branches: \[codex\/gameplay-bugfix-release-20261025\]\s+paths: \[\.github\/workflows\/bevy-runtime-r2-release\.yml\]/);
  const bootstrap = workflow.slice(workflow.indexOf("  validate-bootstrap:"), workflow.indexOf("  publish-runtime-leaves:"));
  assert.match(bootstrap, /if: \$\{\{ github\.event_name == 'push' \}\}/);
  assert.doesNotMatch(bootstrap, /secrets\.|upload-r2-assets|verify-bevy-runtime-r2-upload\.mjs|--verifyRemote/);
  assert.match(workflow, /publish-runtime-leaves:\s+if: \$\{\{ github\.event_name == 'workflow_dispatch' \}\}/);
  assert.match(workflow, /ref: \$\{\{ github\.sha \}\}/);
  assert.match(workflow, /persist-credentials: false/);
  assert.match(workflow, /actions\/checkout@[0-9a-f]{40}/);
  assert.match(workflow, /actions\/setup-node@[0-9a-f]{40}/);
  assert.match(workflow, /EXPECTED_RUNTIME_MANIFEST_SHA256: \$\{\{ inputs\.runtime_manifest_sha256 \}\}/);
  assert.ok(workflow.indexOf("--dryRun true") < workflow.indexOf("secrets.MIR2_R2_UPLOAD_SECRET"));
  assert.ok(workflow.indexOf("--prepareMissing") < workflow.indexOf("secrets.MIR2_R2_UPLOAD_SECRET"));
  assert.equal([...workflow.matchAll(/if: \$\{\{ steps\.missing\.outputs\.missing_count != '0' \}\}/g)].length, 2);
  assert.equal([...workflow.matchAll(/\$\{\{ secrets\./g)].length, 2);
  assert.doesNotMatch(workflow, /(?:wrangler|vercel)\s+(?:deploy|secret|promote)|CLOUDFLARE_API_TOKEN|VERCEL_TOKEN/);
  assert.equal([...workflow.matchAll(/--includeReleaseManifest false/g)].length, 3);
});
