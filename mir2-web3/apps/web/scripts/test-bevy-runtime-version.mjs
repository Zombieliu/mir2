import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { computeBevyRuntimeVersion, verifyBevyRuntimeVersion } from "./lib/bevy-runtime-version.mjs";

function fixture(ui = 1, shared = true) {
  const ids = shared ? ["webgpu", "webgl2", "webgl2-shared"] : ["webgpu", "webgl2"];
  const manifest = { schemaVersion: 2, version: "", packages: ids.map((id) => ({
    id, backend: id === "webgpu" ? "webgpu" : "webgl2", packageDir: `pkg-${id}`,
    questUiAbiVersion: ui, bagUiAbiVersion: ui, primarySharedUiCompiled: id === "webgl2-shared",
  })), files: ids.flatMap((id, index) => [".js", "_bg.wasm"].map((suffix) => ({
    path: `public/bevy-runtime/pkg-${id}/mir2_bevy_runtime${suffix}`, sha256: String(index + 1).repeat(64),
  }))) };
  manifest.version = computeBevyRuntimeVersion(manifest);
  return manifest;
}

test("schema2 capability changes get different immutable identities for the same files", () => {
  const enabled = fixture(1, false);
  const disabled = fixture(0, false);
  assert.deepEqual(enabled.files, disabled.files);
  assert.notEqual(enabled.version, disabled.version);
  assert.equal(verifyBevyRuntimeVersion(enabled).packages.length, 2);
  const wrongMetadata = structuredClone(enabled);
  for (const entry of wrongMetadata.packages) entry.questUiAbiVersion = entry.bagUiAbiVersion = 0;
  assert.throws(() => verifyBevyRuntimeVersion(wrongMetadata), /version does not match/);
});

test("schema2 order is canonical and file mutation cannot reuse its immutable URL", () => {
  const manifest = fixture();
  assert.equal(verifyBevyRuntimeVersion(manifest).files.length, 6);
  const reordered = structuredClone(manifest);
  reordered.files.reverse();
  reordered.packages.reverse();
  assert.equal(computeBevyRuntimeVersion(reordered), manifest.version);
  assert.equal(verifyBevyRuntimeVersion(reordered).version, manifest.version);
  reordered.files[0].sha256 = "a".repeat(64);
  assert.throws(() => verifyBevyRuntimeVersion(reordered), /version does not match/);
});

test("schema2 immutable identity is equal in fresh Node processes with different locales", () => {
  const manifest = fixture();
  const helperUrl = new URL("./lib/bevy-runtime-version.mjs", import.meta.url).href;
  const source = `import fs from 'node:fs';import {computeBevyRuntimeVersion} from ${JSON.stringify(helperUrl)};process.stdout.write(computeBevyRuntimeVersion(JSON.parse(fs.readFileSync(0,'utf8'))));`;
  for (const locale of ["en_US.UTF-8", "sv_SE.UTF-8", "zh_CN.UTF-8"]) {
    const result = spawnSync(process.execPath, ["--input-type=module", "-e", source], {
      env: { ...process.env, LANG: locale, LC_ALL: locale }, input: JSON.stringify(manifest), encoding: "utf8", shell: false,
    });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout, manifest.version, locale);
  }
});

test("legacy95e4 keeps its original four-file URL and does not invent capability metadata", () => {
  const legacy = { version: "bevy-95e4e2c8eb61d95d", files: [
    ["webgpu", ".js", "bd769ebdfb096243175a7a15bedfc5f82c1ea6f2049ff5456d23c9ce2eeb8fdb"],
    ["webgpu", "_bg.wasm", "a27cde6e048a40f3f5c7122efee797d7bd8f992ab41a5cb864932dfb28f28967"],
    ["webgl2", ".js", "26464ea6856457f95de02ce757c3cc53e7ad7802dfe760aa2e9cb030f7480aae"],
    ["webgl2", "_bg.wasm", "ab906cc58d252ef239734848f3621b478c18d4ea1a1fec02492e33badca35513"],
  ].map(([id, suffix, sha256]) => ({ path: `public/bevy-runtime/pkg-${id}/mir2_bevy_runtime${suffix}`, sha256 })) };
  assert.equal(computeBevyRuntimeVersion(legacy), legacy.version);
  const normalized = verifyBevyRuntimeVersion(legacy);
  assert.equal(normalized.schemaVersion, 1);
  assert.ok(normalized.packages.every((entry) => entry.primarySharedUiCompiled === null));
  const reversed = structuredClone(legacy);
  reversed.files.reverse();
  assert.throws(() => verifyBevyRuntimeVersion(reversed), /version does not match/);
});
