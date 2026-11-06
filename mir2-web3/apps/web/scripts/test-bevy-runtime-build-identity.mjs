import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";
import ts from "typescript";
import { validateBevyRuntimeManifest } from "../lib/bevy-runtime-manifest.mjs";
import { computeBevyRuntimeVersion } from "./lib/bevy-runtime-version.mjs";
import { assertCompiledBevyRuntimeManifest, resolveNextBuildDistDirectory } from "./lib/bevy-runtime-build-identity.mjs";

function loadTs(relative, dependencies = {}) {
  const compiled = ts.transpileModule(readFileSync(new URL(relative, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }, reportDiagnostics: true,
  });
  assert.deepEqual((compiled.diagnostics ?? []).filter((item) => item.category === ts.DiagnosticCategory.Error), []);
  const module = { exports: {} };
  new Function("exports", "module", "require", compiled.outputText)(module.exports, module, (id) => {
    if (Object.hasOwn(dependencies, id)) return dependencies[id];
    throw new Error("Unexpected fixture dependency: " + id);
  });
  return module.exports;
}
const parser = loadTs("../lib/bevy-runtime-build-manifest.ts", {
  "./bevy-runtime-manifest.mjs": { validateBevyRuntimeManifest },
});
const mode = loadTs("../lib/bevy-shared-canvas-mode.ts");
const selection = loadTs("../lib/bevy-runtime-package-selection.ts", {
  "./bevy-runtime-manifest.mjs": { validateBevyRuntimeManifest }, "./bevy-shared-canvas-mode": mode,
});
function fixture(kind = "ui1", hash = "a".repeat(64)) {
  const ids = kind === "ui1" ? ["webgpu", "webgl2", "webgl2-shared"] : ["webgpu", "webgl2"];
  const files = ids.flatMap((id) => ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"].map((name) => ({
    path: "public/bevy-runtime/pkg-" + id + "/" + name, sha256: hash,
  })));
  const raw = kind === "legacy" ? { version: "", files } : { schemaVersion: 2, version: "", files,
    packages: ids.map((id) => ({ id, backend: id === "webgpu" ? "webgpu" : "webgl2", packageDir: "pkg-" + id,
      questUiAbiVersion: kind === "ui1" ? 1 : 0, bagUiAbiVersion: kind === "ui1" ? 1 : 0,
      primarySharedUiCompiled: id === "webgl2-shared" })) };
  raw.version = computeBevyRuntimeVersion(raw);
  return raw;
}
const record = (raw) => ({ config: { env: { MIR2_BEVY_RUNTIME_BUILD_MANIFEST: JSON.stringify(raw) } } });

test("one compiled snapshot preserves raw legacy and known two/three-package semantics", () => {
  for (const kind of ["legacy", "ui0", "ui1"]) {
    const raw = fixture(kind);
    const parsed = parser.parseBevyRuntimeBuildManifest(JSON.stringify(raw));
    assert.deepEqual(parsed, raw);
    assert.equal(Object.isFrozen(parsed), true);
    assert.equal(assertCompiledBevyRuntimeManifest(record(raw), raw).version, raw.version);
    const startup = selection.selectBevyRuntimeStartup(parsed, "?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1");
    assert.equal(startup.sharedCanvasPrototype, kind === "ui1");
    assert.equal(startup.runtimeAllowed, kind !== "legacy");
    assert.equal(Object.hasOwn(parsed, "schemaVersion"), kind !== "legacy");
    if (kind === "legacy") assert.equal(selection.selectBevyRuntimeStartup(parsed,
      "?bevyBackend=webgl2&bevySharedCanvas=1").runtimeAllowed, true);
  }
});
test("invalid compiled metadata leaves the actual selector on DOM without a runtime", () => {
  for (const value of [undefined, "", "{", "null", "{}", JSON.stringify({ ...fixture(), files: [] })]) {
    const parsed = parser.parseBevyRuntimeBuildManifest(value);
    const startup = selection.selectBevyRuntimeStartup(parsed, "?bevyBackend=webgl2&bevySharedCanvas=1");
    assert.equal(startup.runtimeAllowed, false);
    assert.equal(startup.sharedCanvasPrototype, false);
  }
});
test("stale or unstamped Next outputs cannot be combined with a current thin runtime", () => {
  assert.throws(() => assertCompiledBevyRuntimeManifest({}, fixture()), /no compiled runtime manifest identity/);
  assert.throws(() => assertCompiledBevyRuntimeManifest({ config: { env: { MIR2_BEVY_RUNTIME_BUILD_MANIFEST: "{" } } }, fixture()), /invalid JSON/);
  assert.throws(() => assertCompiledBevyRuntimeManifest(record(fixture("legacy")), fixture("ui1")), /differs/);
  assert.throws(() => assertCompiledBevyRuntimeManifest(record(fixture("ui0")), fixture("ui1")), /differs/);
  assert.throws(() => assertCompiledBevyRuntimeManifest(record(fixture("ui1", "b".repeat(64))), fixture("ui1")), /differs/);
});
test("all capability/file metadata is verified even if a stale version string is retained", () => {
  const raw = fixture();
  assert.throws(() => assertCompiledBevyRuntimeManifest(record({ ...raw,
    files: raw.files.map((file, index) => index === 0 ? { ...file, sha256: "c".repeat(64) } : file),
  }), raw), /version does not match/);
  assert.throws(() => assertCompiledBevyRuntimeManifest(record({ ...raw,
    packages: raw.packages.filter((item) => item.id !== "webgl2-shared"),
  }), raw));
});
test("package/file enumeration order does not create a false compiled identity mismatch", () => {
  const raw = fixture();
  assert.equal(assertCompiledBevyRuntimeManifest(record({ ...raw, packages: [...raw.packages].reverse(),
    files: [...raw.files].reverse() }), raw).version, raw.version);
});
test("Next source distDir selects the copied metadata and static destination within the standalone app", () => {
  const appRoot = path.resolve("fixture-standalone-app");
  assert.equal(resolveNextBuildDistDirectory(record(fixture()), appRoot), path.join(appRoot, ".next"));
  const source = { ...record(fixture()), config: { ...record(fixture()).config, distDir: ".next-package-selection-49d3" } };
  const copied = { ...record(fixture()), config: { ...record(fixture()).config, distDir: ".next-package-selection-49d3" } };
  const sourceDirectory = resolveNextBuildDistDirectory(source, appRoot);
  assert.equal(sourceDirectory, path.join(appRoot, ".next-package-selection-49d3"));
  assert.equal(resolveNextBuildDistDirectory(copied, appRoot), sourceDirectory);
  assert.equal(assertCompiledBevyRuntimeManifest(copied, fixture()).version, fixture().version);
  assert.notEqual(resolveNextBuildDistDirectory({ config: { distDir: ".next-stale" } }, appRoot), sourceDirectory);
  assert.equal(resolveNextBuildDistDirectory({ config: { distDir: "build/next" } }, appRoot), path.join(appRoot, "build", "next"));
});
test("Next distDir refuses absolute, ambiguous and escaping paths", () => {
  const appRoot = path.resolve("fixture-standalone-app");
  for (const distDir of ["", null, 5, ".", "..", "../outside", "build/../outside", "build//next",
    "/tmp/next", "C:/next", "C:next", "\\\\server\\share\\next", "build\\next", "build\0next"]) {
    assert.throws(() => resolveNextBuildDistDirectory({ config: { distDir } }, appRoot), /Next build distDir/, String(distDir));
  }
  assert.throws(() => resolveNextBuildDistDirectory({}, "relative-app"), /app root must be absolute/);
});
test("Page and proxy consume the same Next config snapshot and thin checks both source/output builds", () => {
  const config = readFileSync(new URL("../next.config.ts", import.meta.url), "utf8");
  const page = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
  const proxy = readFileSync(new URL("../proxy.ts", import.meta.url), "utf8");
  const thin = readFileSync(new URL("./build-thin-client.mjs", import.meta.url), "utf8");
  assert.match(config, /MIR2_BEVY_RUNTIME_BUILD_MANIFEST: compiledRuntimeManifest/);
  assert.match(page, /import bevyRuntimeVersion from "\.\.\/lib\/bevy-runtime-build-manifest"/);
  assert.match(proxy, /import runtimeManifest from "\.\/lib\/bevy-runtime-build-manifest"/);
  assert.doesNotMatch(page, /import .*from ".*generated\/bevy_runtime_version\.json"/);
  assert.doesNotMatch(proxy, /import .*from ".*generated\/bevy_runtime_version\.json"/);
  assert.match(thin, /path\.join\(nextRoot, "required-server-files\.json"\)/);
  assert.match(thin, /resolveNextBuildDistDirectory\(sourceRequiredServerFiles, appRoot\)/);
  assert.match(thin, /path\.join\(copiedNextRoot, "required-server-files\.json"\)/);
  assert.match(thin, /path\.join\(copiedNextRoot, "static"\)/);
});
