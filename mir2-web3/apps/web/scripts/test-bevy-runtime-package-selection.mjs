import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";
import { validateBevyRuntimeManifest } from "../lib/bevy-runtime-manifest.mjs";

function loadTs(path, dependencies = {}) {
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  const compiled = ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  }, reportDiagnostics: true });
  assert.deepEqual((compiled.diagnostics ?? []).filter((item) => item.category === ts.DiagnosticCategory.Error), []);
  const module = { exports: {} };
  new Function("exports", "module", "require", compiled.outputText)(module.exports, module,
    (id) => { if (Object.hasOwn(dependencies, id)) return dependencies[id]; throw new Error(`unexpected import: ${id}`); });
  return module.exports;
}

const mode = loadTs("../lib/bevy-shared-canvas-mode.ts");
const selection = loadTs("../lib/bevy-runtime-package-selection.ts", {
  "./bevy-runtime-manifest.mjs": { validateBevyRuntimeManifest },
  "./bevy-shared-canvas-mode": mode,
});
const urls = loadTs("../lib/bevy-runtime-url.ts");
const exact = "?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1";
const version = "bevy-0123456789abcdef";
const hash = "a".repeat(64);
const pair = (id) => ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"].map((name) => ({
  path: `public/bevy-runtime/pkg-${id}/${name}`, sha256: hash,
}));
const pkg = (id, abi = 1) => ({ id, backend: id === "webgpu" ? "webgpu" : "webgl2",
  packageDir: `pkg-${id}`, questUiAbiVersion: abi, bagUiAbiVersion: abi,
  primarySharedUiCompiled: id === "webgl2-shared" });
const manifest = (shared = true) => {
  const ids = shared ? ["webgpu", "webgl2", "webgl2-shared"] : ["webgpu", "webgl2"];
  return { schemaVersion: 2, version, packages: ids.map((id) => pkg(id)), files: ids.flatMap(pair) };
};
const capabilities = (backend, compiled, startup, abi = 1) => JSON.stringify({
  schemaVersion: 1, backend, questUiAbiVersion: abi, bagUiAbiVersion: abi,
  primarySharedUiCompiled: compiled, primarySharedUiStartup: startup,
});
const stageSelector = () => "#mir2-quest-ui-canvas";

test("invalid metadata falls back to DOM before runtime import", () => {
  const chosen = selection.selectBevyRuntimeStartup({ ...manifest(), files: [] }, exact);
  assert.equal(chosen.runtimeAllowed, false);
  assert.equal(chosen.sharedCanvasPrototype, false);
  assert.equal(selection.getBevyRuntimePackageForBackend(chosen, "webgl2"), null);
});

test("exact shared mode selects shared GL2 only when the complete package exists", () => {
  const shared = selection.selectBevyRuntimeStartup(manifest(), exact);
  assert.equal(shared.runtimeAllowed, true);
  assert.equal(shared.sharedCanvasPrototype, true);
  assert.equal(selection.getBevyRuntimePackageForBackend(shared, "webgl2").id, "webgl2-shared");
  assert.equal(selection.getBevyRuntimePackageForBackend(shared, "webgpu"), null);
  assert.equal(selection.getBevyRuntimePackageForBackend(shared, "other"), null);
  const lean = selection.selectBevyRuntimeStartup(manifest(false), exact);
  assert.equal(lean.sharedCanvasPrototype, false);
  assert.equal(selection.getBevyRuntimePackageForBackend(lean, "webgl2").id, "webgl2");
  assert.equal(selection.getBevyRuntimePackageForBackend(lean, "webgpu"), null,
    "an exact GL2 request cannot silently boot GPU when shared GL2 is absent");
  assert.equal(selection.assertBevyRuntimeStartupAgreement(lean, "webgl2",
    () => capabilities("webgl2", false, false), exact).primarySharedUiStartup, false);
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(lean, "webgpu",
    () => capabilities("webgpu", false, false), exact));
});

test("ordinary empty query freezes the shared GL2 package while preserving GPU priority", () => {
  const chosen = selection.selectBevyRuntimeStartup(manifest(), "");
  assert.equal(chosen.requestedSharedCanvas, false);
  assert.equal(chosen.sharedCanvasPrototype, false);
  assert.equal(chosen.sharedWebGl2, true);
  assert.equal(selection.getBevyRuntimePackageForBackend(chosen, "webgpu").id, "webgpu");
  assert.equal(selection.getBevyRuntimePackageForBackend(chosen, "webgl2").id, "webgl2-shared");
  assert.equal(selection.assertBevyRuntimeStartupAgreement(chosen, "webgpu",
    () => capabilities("webgpu", false, false), "").primarySharedUiStartup, false);
  assert.equal(selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
    () => capabilities("webgl2", true, true), "", stageSelector).primarySharedUiStartup, true);
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgpu",
    () => capabilities("webgpu", false, true), ""));
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
    () => capabilities("webgl2", true, false), ""));
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
    () => capabilities("webgl2", true, true), ""));
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
    () => capabilities("webgl2", true, true), "", () => "#mir2-web3-canvas"));
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
    () => capabilities("webgl2", true, true), "", () => { throw new Error("unavailable"); }));
});

test("heterogeneous packages select GPU ABI1 or default and exact shared GL2 ABI1", () => {
  const mixed = { ...manifest(), packages: [pkg("webgpu", 1), pkg("webgl2", 0), pkg("webgl2-shared", 1)] };
  for (const search of ["", "?bevyBackend=webgl2", exact]) {
    const chosen = selection.selectBevyRuntimeStartup(mixed, search);
    assert.equal(chosen.runtimeAllowed, true, search);
    assert.equal(chosen.sharedWebGl2, true, search);
    const gl = selection.getBevyRuntimePackageForBackend(chosen, "webgl2");
    assert.equal(gl.id, "webgl2-shared");
    assert.equal(gl.questUiAbiVersion, 1);
    let getterCalls = 0;
    const actual = selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
      () => { getterCalls++; return capabilities("webgl2", true, true, 1); }, search, stageSelector);
    assert.equal(getterCalls, 1);
    assert.deepEqual(actual, JSON.parse(capabilities("webgl2", true, true, 1)));
    assert.ok(Object.isFrozen(actual));
    assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
      () => capabilities("webgl2", false, false, 0), search, stageSelector));
    assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
      () => capabilities("webgl2", true, true, 0), search, stageSelector));
    const gpu = selection.getBevyRuntimePackageForBackend(chosen, "webgpu");
    if (search === exact) assert.equal(gpu, null);
    else {
      assert.equal(gpu.id, "webgpu");
      assert.equal(gpu.bagUiAbiVersion, 1);
      assert.equal(selection.assertBevyRuntimeStartupAgreement(chosen, "webgpu",
        () => capabilities("webgpu", false, false, 1), search).questUiAbiVersion, 1);
      assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgpu",
        () => capabilities("webgpu", false, false, 0), search));
    }
  }
});

test("heterogeneous explicit lean fallback requires exact ABI0 getter and frozen opt-out", () => {
  const mixed = { ...manifest(), packages: [pkg("webgpu", 1), pkg("webgl2", 0), pkg("webgl2-shared", 1)] };
  for (const search of ["?bevySharedCanvas=0", "?bevyBackend=webgl2&bevySharedCanvas=0", "?bevyQuestUi=0&bevyBagUi=0"]) {
    const chosen = selection.selectBevyRuntimeStartup(mixed, search);
    assert.equal(chosen.runtimeAllowed, true, search);
    assert.equal(chosen.sharedWebGl2, false, search);
    assert.equal(chosen.sharedCanvasPrototype, false, search);
    assert.equal(selection.getBevyRuntimePackageForBackend(chosen, "webgl2").id, "webgl2");
    let getterCalls = 0;
    const actual = selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
      () => { getterCalls++; return capabilities("webgl2", false, false, 0); }, search);
    assert.equal(getterCalls, 1);
    assert.deepEqual(actual, JSON.parse(capabilities("webgl2", false, false, 0)));
    const good = JSON.parse(capabilities("webgl2", false, false, 0));
    for (const getter of [undefined, () => capabilities("webgl2", false, false, 1),
      () => capabilities("webgl2", true, true, 1), () => capabilities("webgpu", false, false, 0),
      () => JSON.stringify({ ...good, bagUiAbiVersion: 1 }),
      () => JSON.stringify({ ...good, questUiAbiVersion: "0" }),
      () => JSON.stringify({ ...good, primarySharedUiCompiled: 0 }),
      () => JSON.stringify({ ...good, extra: true }), () => { throw new Error("getter failed"); }]) {
      assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2", getter, search));
    }
    assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
      () => capabilities("webgl2", false, false, 0), ""));
  }
  const defaultShared = selection.selectBevyRuntimeStartup(mixed, "");
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(defaultShared, "webgl2",
    () => capabilities("webgl2", true, true, 1), "?bevySharedCanvas=0", stageSelector));
  for (const packages of [[pkg("webgpu", 1), pkg("webgl2", 0)], [pkg("webgpu", 0), pkg("webgl2", 1)]]) {
    const denied = selection.selectBevyRuntimeStartup({ ...manifest(false), packages }, "?bevySharedCanvas=0");
    assert.equal(denied.runtimeAllowed, false);
    assert.equal(selection.getBevyRuntimePackageForBackend(denied, "webgl2"), null);
  }
});

test("explicit opt-outs and absent shared packages retain lean GL2 selection", () => {
  for (const search of ["?bevySharedCanvas=0", "?bevyQuestUi=0&bevyBagUi=0",
    "?%62evySharedCanvas=1", "?bevySharedCanvas=0&bevySharedCanvas=1"]) {
    const chosen = selection.selectBevyRuntimeStartup(manifest(), search);
    assert.equal(chosen.sharedWebGl2, false, search);
    assert.equal(selection.getBevyRuntimePackageForBackend(chosen, "webgl2").id, "webgl2", search);
  }
  const twoPackage = selection.selectBevyRuntimeStartup(manifest(false), "");
  assert.equal(twoPackage.sharedWebGl2, false);
  assert.equal(selection.getBevyRuntimePackageForBackend(twoPackage, "webgpu").id, "webgpu");
  assert.equal(selection.getBevyRuntimePackageForBackend(twoPackage, "webgl2").id, "webgl2");
  const old = { version, files: ["webgpu", "webgl2"].flatMap(pair) };
  const legacy = selection.selectBevyRuntimeStartup(old, "");
  assert.equal(legacy.sharedWebGl2, false);
  assert.equal(selection.getBevyRuntimePackageForBackend(legacy, "webgl2").id, "webgl2");
});

test("live startup agreement rejects relevant query changes without losing unrelated flags", () => {
  const chosen = selection.selectBevyRuntimeStartup(manifest(), "?bevyBagUi=1&mapAtlas=0");
  const good = () => capabilities("webgl2", true, true);
  assert.equal(selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2", good,
    "?bevyBagUi=1&mapAtlas=1", stageSelector).primarySharedUiStartup, true);
  for (const search of ["?bevyBagUi=0&mapAtlas=0", "?bevyBagUi=%31&mapAtlas=0",
    "?bevyBagUi=1&bevySharedCanvas=0&mapAtlas=0", "?bevyQuestUi=1&bevyBagUi=1&mapAtlas=0"]) {
    assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2", good, search, stageSelector), undefined, search);
  }
  const explicit = selection.selectBevyRuntimeStartup(manifest(), exact);
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(explicit, "webgl2",
    () => capabilities("webgl2", true, true), "?bevyBackend=webgl2&bevyBagUi=1", stageSelector));
});

test("encoded and duplicate raw opt-ins retain ordinary package selection", () => {
  for (const search of [
    "?bevyBackend=webgl2&bevySharedCanvas=%31&bevyBagUi=1",
    "?%62evyBackend=webgl2&bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1",
    "?bevyBackend=webgl2&bevySharedCanvas=0&bevySharedCanvas=1&bevyBagUi=1",
    "??bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1",
  ]) {
    const chosen = selection.selectBevyRuntimeStartup(manifest(), search);
    assert.equal(chosen.sharedCanvasPrototype, false, search);
    assert.equal(selection.getBevyRuntimePackageForBackend(chosen, "webgl2").id, "webgl2");
  }
});

test("legacy exact shared request is unsafe while ordinary legacy remains usable", () => {
  const old = { version, files: ["webgpu", "webgl2"].flatMap(pair) };
  const shared = selection.selectBevyRuntimeStartup(old, exact);
  assert.equal(shared.runtimeAllowed, false);
  assert.equal(shared.reason, "legacy-shared-unknown");
  const ordinary = selection.selectBevyRuntimeStartup(old, "?bevyBackend=webgl2");
  assert.equal(ordinary.runtimeAllowed, true);
  assert.equal(selection.assertBevyRuntimeStartupAgreement(ordinary, "webgl2", undefined,
    "?bevyBackend=webgl2"), null);
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(ordinary, "webgl2", undefined, exact));
});

test("getter agreement checks backend, ABI, compiled primary, startup, and live query", () => {
  const shared = selection.selectBevyRuntimeStartup(manifest(), exact);
  const good = () => capabilities("webgl2", true, true);
  assert.equal(selection.assertBevyRuntimeStartupAgreement(shared, "webgl2", good, exact, stageSelector).primarySharedUiStartup, true);
  for (const getter of [undefined, () => capabilities("webgpu", true, true),
    () => capabilities("webgl2", false, true), () => capabilities("webgl2", true, false),
    () => capabilities("webgl2", true, true, 0), () => "{bad"])
    assert.throws(() => selection.assertBevyRuntimeStartupAgreement(shared, "webgl2", getter, exact));
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(shared, "webgl2", good,
    "?bevyBackend=webgl2"));
  const lean = selection.selectBevyRuntimeStartup(manifest(false), exact);
  assert.equal(selection.assertBevyRuntimeStartupAgreement(lean, "webgl2",
    () => capabilities("webgl2", false, false), exact).primarySharedUiCompiled, false);
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(lean, "webgl2",
    () => capabilities("webgl2", true, true), exact));
});

test("legacy getter may report unknown compiled GL2 capability but cannot start shared", () => {
  const old = { version, files: ["webgpu", "webgl2"].flatMap(pair) };
  const chosen = selection.selectBevyRuntimeStartup(old, "?bevyBagUi=1");
  assert.equal(selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
    () => capabilities("webgl2", true, false), "?bevyBagUi=1").primarySharedUiCompiled, true);
  assert.throws(() => selection.assertBevyRuntimeStartupAgreement(chosen, "webgl2",
    () => capabilities("webgl2", true, true), "?bevyBagUi=1"));
});

test("one document gate forbids a second boot even after synchronous or asynchronous failure", async () => {
  let calls = 0;
  const gate = selection.createBevyRuntimeBootGate();
  assert.throws(() => selection.runBevyRuntimeBootOnce(gate, () => { calls++; throw new Error("boot failed"); }));
  assert.throws(() => selection.runBevyRuntimeBootOnce(gate, () => { calls++; }));
  assert.equal(selection.createBevyRuntimeBootGate(), gate);
  assert.throws(() => selection.runBevyRuntimeBootOnce(selection.createBevyRuntimeBootGate(), () => { calls++; }));
  assert.equal(calls, 1);
  const freshSelection = loadTs("../lib/bevy-runtime-package-selection.ts", {
    "./bevy-runtime-manifest.mjs": { validateBevyRuntimeManifest },
    "./bevy-shared-canvas-mode": mode,
  });
  const asyncGate = freshSelection.createBevyRuntimeBootGate();
  await assert.rejects(freshSelection.runBevyRuntimeBootOnce(asyncGate, async () => { throw new Error("async failure"); }));
  assert.throws(() => freshSelection.runBevyRuntimeBootOnce(freshSelection.createBevyRuntimeBootGate(), () => { calls++; }));
  assert.throws(() => selection.runBevyRuntimeBootOnce({ kind: "bevy-runtime-boot-gate" }, () => { calls++; }));
});

test("third URL variant preserves old positional callers and immutable package path", () => {
  const old = urls.createBevyRuntimeUrls(version, "webgl2", null);
  const next = urls.createBevyRuntimeUrls(version, "webgl2-shared", null);
  assert.match(old.moduleUrl, /\/pkg-webgl2\/mir2_bevy_runtime\.js$/);
  assert.match(next.moduleUrl, /\/pkg-webgl2-shared\/mir2_bevy_runtime\.js$/);
  assert.match(next.wasmUrl, /\/pkg-webgl2-shared\/mir2_bevy_runtime_bg\.wasm$/);
});
