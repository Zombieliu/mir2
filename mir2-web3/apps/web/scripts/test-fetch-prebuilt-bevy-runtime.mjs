#!/usr/bin/env node

import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { installPrebuiltRuntime, validateRuntimeManifest, localPrebuiltRuntimeReady } from "./fetch-prebuilt-bevy-runtime.mjs";
import { computeBevyRuntimeVersion } from "./lib/bevy-runtime-version.mjs";

const fixtureFiles = new Map([
  ["pkg-webgpu/mir2_bevy_runtime.js", Buffer.from("export const backend = 'webgpu';\n")],
  ["pkg-webgpu/mir2_bevy_runtime_bg.wasm", Buffer.from([0, 97, 115, 109, 1, 0, 0, 0])],
  ["pkg-webgl2/mir2_bevy_runtime.js", Buffer.from("export const backend = 'webgl2';\n")],
  ["pkg-webgl2/mir2_bevy_runtime_bg.wasm", Buffer.from([0, 97, 115, 109, 1, 0, 0, 0, 1])],
]);
const version = computeBevyRuntimeVersion({ files: fileRecords(fixtureFiles) });

test("downloads, verifies, reuses, and repairs the pinned runtime", async (context) => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-runtime-fetch-test-"));
  context.after(() => fs.rm(tempRoot, { recursive: true, force: true }));
  const manifestPath = path.join(tempRoot, "runtime.json");
  const outputDir = path.join(tempRoot, "public", "bevy-runtime");
  const manifest = createManifest();
  await fs.writeFile(manifestPath, JSON.stringify(manifest));
  let requestCount = 0;
  const server = http.createServer((request, response) => {
    requestCount += 1;
    const prefix = `/release/bevy-runtime/v/${version}/`;
    const key = decodeURIComponent(request.url ?? "").replace(prefix, "");
    const bytes = fixtureFiles.get(key);
    if (!request.url?.startsWith(prefix) || !bytes) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { "content-type": key.endsWith(".wasm") ? "application/wasm" : "text/javascript" });
    response.end(bytes);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  context.after(() => new Promise((resolve) => server.close(resolve)));
  const address = server.address();
  assert(address && typeof address === "object");
  const assetBaseUrl = `http://127.0.0.1:${address.port}/release`;

  const first = await installPrebuiltRuntime({ manifestPath, outputDir, assetBaseUrl, attempts: 1 });
  assert.equal(first.reused, false);
  assert.equal(requestCount, 4);
  for (const [relativePath, expected] of fixtureFiles) {
    assert.deepEqual(await fs.readFile(path.join(outputDir, relativePath)), expected);
  }

  const second = await installPrebuiltRuntime({ manifestPath, outputDir, assetBaseUrl, attempts: 1 });
  assert.equal(second.reused, true);
  assert.equal(requestCount, 4);

  await fs.writeFile(path.join(outputDir, "pkg-webgpu", "mir2_bevy_runtime.js"), "corrupt");
  const repaired = await installPrebuiltRuntime({ manifestPath, outputDir, assetBaseUrl, attempts: 1 });
  assert.equal(repaired.reused, false);
  assert.equal(requestCount, 8);
  assert.deepEqual(
    await fs.readFile(path.join(outputDir, "pkg-webgpu", "mir2_bevy_runtime.js")),
    fixtureFiles.get("pkg-webgpu/mir2_bevy_runtime.js"),
  );
});

test("clean checkouts fetch an overlay runtime from its pinned base release", async (context) => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-runtime-overlay-fetch-test-"));
  context.after(() => fs.rm(tempRoot, { recursive: true, force: true }));
  const manifestPath = path.join(tempRoot, "runtime.json");
  const configPath = path.join(tempRoot, "production-web-assets.json");
  const outputDir = path.join(tempRoot, "public", "bevy-runtime");
  await fs.writeFile(manifestPath, JSON.stringify(createManifest()));

  const requests = [];
  const server = http.createServer((request, response) => {
    requests.push(request.url);
    const prefix = `/mir2/v/full-base/bevy-runtime/v/${version}/`;
    const key = decodeURIComponent(request.url ?? "").replace(prefix, "");
    const bytes = fixtureFiles.get(key);
    if (!request.url?.startsWith(prefix) || !bytes) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { "content-type": key.endsWith(".wasm") ? "application/wasm" : "text/javascript" });
    response.end(bytes);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  context.after(() => new Promise((resolve) => server.close(resolve)));
  const address = server.address();
  assert(address && typeof address === "object");
  await fs.writeFile(configPath, JSON.stringify({
    assetBaseUrl: `http://127.0.0.1:${address.port}/mir2/v/new-overlay`,
    objectPrefix: "mir2/v/new-overlay",
    fallbackObjectPrefix: "mir2/v/full-base",
  }));

  const result = await installPrebuiltRuntime({ manifestPath, configPath, outputDir, attempts: 1 });
  assert.equal(result.reused, false);
  assert.equal(requests.length, 4);
  assert.ok(requests.every((requestUrl) => requestUrl.startsWith("/mir2/v/full-base/")));
  for (const [relativePath, expected] of fixtureFiles) {
    assert.deepEqual(await fs.readFile(path.join(outputDir, relativePath)), expected);
  }
});

test("rejects unexpected paths before making a request", async () => {
  const manifest = createManifest();
  manifest.files[0].path = "public/bevy-runtime/../../package.json";
  assert.throws(() => validateRuntimeManifest(manifest, "fixture"), /malformed package file/);
});

test("does not replace an existing runtime when a hash is wrong", async (context) => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-runtime-fetch-rollback-test-"));
  context.after(() => fs.rm(tempRoot, { recursive: true, force: true }));
  const outputDir = path.join(tempRoot, "bevy-runtime");
  const manifestPath = path.join(tempRoot, "runtime.json");
  const sentinelPath = path.join(outputDir, "keep.txt");
  await fs.mkdir(outputDir, { recursive: true });
  await fs.writeFile(sentinelPath, "existing-runtime");
  await fs.writeFile(manifestPath, JSON.stringify(createManifest()));

  const fetchImpl = async () => new Response(Buffer.from("corrupt"), { status: 200 });
  await assert.rejects(
    installPrebuiltRuntime({ manifestPath, outputDir, assetBaseUrl: "https://assets.example.test/release", fetchImpl, attempts: 1 }),
    /SHA-256 mismatch/,
  );
  assert.equal(await fs.readFile(sentinelPath, "utf8"), "existing-runtime");
});

function createManifest() {
  return {
    version,
    files: fileRecords(fixtureFiles),
  };
}

function fileRecords(files) {
  return [...files].map(([relativePath, bytes]) => ({
      path: `public/bevy-runtime/${relativePath}`,
      sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    }));
}

for (const uiEnabled of [false, true]) {
  test(`schema2 ${uiEnabled ? "six" : "four"}-file install preserves capability metadata and checks complete local bytes`, async (context) => {
    const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-schema2-fetch-"));
    context.after(() => fs.rm(tempRoot, { recursive: true, force: true }));
    const files = new Map(fixtureFiles);
    const ids = uiEnabled ? ["webgpu", "webgl2", "webgl2-shared"] : ["webgpu", "webgl2"];
    if (uiEnabled) {
      files.set("pkg-webgl2-shared/mir2_bevy_runtime.js", Buffer.from("export const shared = true;\n"));
      files.set("pkg-webgl2-shared/mir2_bevy_runtime_bg.wasm", Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]));
    }
    const manifest = { schemaVersion: 2, version: "", packages: ids.map((id) => ({
      id, backend: id === "webgpu" ? "webgpu" : "webgl2", packageDir: `pkg-${id}`,
      questUiAbiVersion: uiEnabled ? 1 : 0, bagUiAbiVersion: uiEnabled ? 1 : 0,
      primarySharedUiCompiled: id === "webgl2-shared",
    })), files: fileRecords(files) };
    manifest.version = computeBevyRuntimeVersion(manifest);
    const manifestText = JSON.stringify(manifest, null, 2);
    const manifestPath = path.join(tempRoot, "runtime.json");
    const outputDir = path.join(tempRoot, "bevy-runtime");
    await fs.writeFile(manifestPath, manifestText);
    const requests = [];
    const fetchImpl = async (url) => {
      requests.push(url);
      const prefix = `https://assets.example.test/release/bevy-runtime/v/${manifest.version}/`;
      assert.ok(url.startsWith(prefix));
      const bytes = files.get(url.slice(prefix.length));
      assert.ok(bytes);
      return new Response(bytes, { status: 200 });
    };
    const options = { manifestPath, outputDir, assetBaseUrl: "https://assets.example.test/release", fetchImpl, attempts: 1 };
    await installPrebuiltRuntime(options);
    assert.equal(requests.length, files.size);
    assert.equal(await fs.readFile(manifestPath, "utf8"), manifestText);
    assert.equal(await localPrebuiltRuntimeReady(options), true);
    await fs.unlink(path.join(outputDir, ids.at(-1) === "webgl2-shared" ? "pkg-webgl2-shared" : "pkg-webgl2", "mir2_bevy_runtime.js"));
    assert.equal(await localPrebuiltRuntimeReady(options), false);
    await installPrebuiltRuntime(options);
    assert.equal(requests.length, files.size * 2);
    assert.equal(await fs.readFile(manifestPath, "utf8"), manifestText);
    assert.equal(await localPrebuiltRuntimeReady(options), true);
    const tampered = structuredClone(manifest);
    tampered.version = "bevy-0000000000000000";
    await fs.writeFile(manifestPath, JSON.stringify(tampered));
    const priorRequests = requests.length;
    await assert.rejects(installPrebuiltRuntime(options), /version does not match/);
    assert.equal(requests.length, priorRequests);
  });
}
