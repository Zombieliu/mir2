#!/usr/bin/env node

import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { installPrebuiltRuntime, validateRuntimeManifest } from "./fetch-prebuilt-bevy-runtime.mjs";

const version = "bevy-0123456789abcdef";
const fixtureFiles = new Map([
  ["pkg-webgpu/mir2_bevy_runtime.js", Buffer.from("export const backend = 'webgpu';\n")],
  ["pkg-webgpu/mir2_bevy_runtime_bg.wasm", Buffer.from([0, 97, 115, 109, 1, 0, 0, 0])],
  ["pkg-webgl2/mir2_bevy_runtime.js", Buffer.from("export const backend = 'webgl2';\n")],
  ["pkg-webgl2/mir2_bevy_runtime_bg.wasm", Buffer.from([0, 97, 115, 109, 1, 0, 0, 0, 1])],
]);

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
  assert.throws(() => validateRuntimeManifest(manifest, "fixture"), /Invalid Bevy runtime file entry/);
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
    files: [...fixtureFiles].map(([relativePath, bytes]) => ({
      path: `public/bevy-runtime/${relativePath}`,
      sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    })),
  };
}
