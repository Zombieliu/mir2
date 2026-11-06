import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { computeBevyRuntimeVersion } from "../apps/web/scripts/lib/bevy-runtime-version.mjs";
import { verifyDeveloperBevyRuntimeLock } from "./lib/verify-developer-bevy-runtime-lock.mjs";

const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");

function fixture(t, { legacy = false, shared = false } = {}) {
  const webRoot = mkdtempSync(path.join(tmpdir(), "mir2-developer-runtime-lock-"));
  t.after(() => {
    assert.equal(path.dirname(path.resolve(webRoot)), path.resolve(tmpdir()));
    assert.ok(path.basename(webRoot).startsWith("mir2-developer-runtime-lock-"));
    rmSync(webRoot, { recursive: true, force: true });
  });
  const ids = shared ? ["webgpu", "webgl2", "webgl2-shared"] : ["webgpu", "webgl2"];
  const files = ids.flatMap((id) => [".js", "_bg.wasm"].map((suffix) => {
    const relativePath = `public/bevy-runtime/pkg-${id}/mir2_bevy_runtime${suffix}`;
    const bytes = suffix === ".js"
      ? Buffer.from(`export const backend = ${JSON.stringify(id)};\n`)
      : Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
    const absolutePath = path.join(webRoot, ...relativePath.split("/"));
    mkdirSync(path.dirname(absolutePath), { recursive: true });
    writeFileSync(absolutePath, bytes);
    return { path: relativePath, sha256: hash(bytes) };
  }));
  const manifest = legacy ? { version: "", files } : {
    schemaVersion: 2,
    version: "",
    packages: ids.map((id) => ({
      id,
      backend: id === "webgpu" ? "webgpu" : "webgl2",
      packageDir: `pkg-${id}`,
      questUiAbiVersion: shared && id !== "webgl2" ? 1 : 0,
      bagUiAbiVersion: shared && id !== "webgl2" ? 1 : 0,
      primarySharedUiCompiled: id === "webgl2-shared",
    })),
    files,
  };
  manifest.version = computeBevyRuntimeVersion(manifest);
  return { webRoot, manifest, artifact: (file) => path.join(webRoot, ...file.path.split("/")) };
}

test("developer lock accepts the legacy four-file identity without inventing capabilities", (t) => {
  const { webRoot, manifest } = fixture(t, { legacy: true });
  const verified = verifyDeveloperBevyRuntimeLock(manifest, webRoot);
  assert.equal(verified.schemaVersion, 1);
  assert.equal(verified.version, manifest.version);
  assert.ok(verified.packages.every((entry) => entry.primarySharedUiCompiled === null));
  manifest.files.reverse();
  assert.throws(() => verifyDeveloperBevyRuntimeLock(manifest, webRoot), /version does not match/);
});

for (const shared of [false, true]) {
  test(`developer lock accepts a real schema2 ${shared ? "three" : "two"}-package bundle`, (t) => {
    const { webRoot, manifest } = fixture(t, { shared });
    const verified = verifyDeveloperBevyRuntimeLock(manifest, webRoot);
    assert.equal(verified.files.length, shared ? 6 : 4);
    assert.deepEqual(verified.packages, manifest.packages);
    manifest.files.reverse();
    manifest.packages.reverse();
    assert.equal(verifyDeveloperBevyRuntimeLock(manifest, webRoot).version, verified.version);
  });
}

test("unchanged files cannot reuse an immutable version after capability metadata changes", (t) => {
  const { webRoot, manifest } = fixture(t);
  for (const entry of manifest.packages) {
    entry.questUiAbiVersion = entry.bagUiAbiVersion = 1;
  }
  assert.throws(() => verifyDeveloperBevyRuntimeLock(manifest, webRoot), /version does not match/);
  manifest.version = computeBevyRuntimeVersion(manifest);
  assert.equal(verifyDeveloperBevyRuntimeLock(manifest, webRoot).packages[0].questUiAbiVersion, 1);
});

test("actual artifact bytes and their locked hashes both bind the immutable version", (t) => {
  const { webRoot, manifest, artifact } = fixture(t, { shared: true });
  const changed = manifest.files[0];
  writeFileSync(artifact(changed), "export const replaced = true;\n");
  assert.throws(() => verifyDeveloperBevyRuntimeLock(manifest, webRoot), /hash mismatch/);
  changed.sha256 = hash(readFileSync(artifact(changed)));
  assert.throws(() => verifyDeveloperBevyRuntimeLock(manifest, webRoot), /version does not match/);
});

test("a declared shared package cannot omit an artifact or silently discard metadata", (t) => {
  const { webRoot, manifest } = fixture(t, { shared: true });
  const missingFile = structuredClone(manifest);
  missingFile.files.pop();
  missingFile.version = computeBevyRuntimeVersion(missingFile);
  assert.throws(() => verifyDeveloperBevyRuntimeLock(missingFile, webRoot), /incomplete package files/);
  manifest.packages[2].unknownCapability = true;
  assert.throws(() => verifyDeveloperBevyRuntimeLock(manifest, webRoot), /invalid package record/);
});

test("even a correctly rehashed bundle must contain WebAssembly rather than an error body", (t) => {
  const { webRoot, manifest, artifact } = fixture(t);
  const wasm = manifest.files.find((file) => file.path.endsWith(".wasm"));
  const bytes = Buffer.from("<html>not a runtime</html>");
  writeFileSync(artifact(wasm), bytes);
  wasm.sha256 = hash(bytes);
  manifest.version = computeBevyRuntimeVersion(manifest);
  assert.throws(() => verifyDeveloperBevyRuntimeLock(manifest, webRoot), /invalid WebAssembly header/);
});
