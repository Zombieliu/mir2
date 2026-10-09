import assert from "node:assert/strict";
import { test } from "node:test";
import { computeBevyRuntimeVersion, verifyBevyRuntimeVersion } from "./lib/bevy-runtime-version.mjs";
import { evaluateRuntimePackageEvidence, runtimeSmokeScenarios, scenarioAssertions } from "./smoke-bevy-runtime-backends.mjs";

const artifactNames = ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"];
const ids = ["webgpu", "webgl2", "webgl2-shared"];
const rawManifest = (shared = true) => {
  const included = shared ? ids : ids.slice(0, 2);
  const manifest = {
    schemaVersion: 2,
    version: "",
    packages: included.map((id) => ({
      id, backend: id === "webgpu" ? "webgpu" : "webgl2", packageDir: `pkg-${id}`,
      questUiAbiVersion: 1, bagUiAbiVersion: 1, primarySharedUiCompiled: id === "webgl2-shared",
    })),
    files: included.flatMap((id) => artifactNames.map((artifact) => ({
      path: `public/bevy-runtime/pkg-${id}/${artifact}`, sha256: "a".repeat(64),
    }))),
  };
  manifest.version = computeBevyRuntimeVersion(manifest);
  return verifyBevyRuntimeVersion(manifest);
};

const url = (manifest, id, name = artifactNames[0]) =>
  `https://assets.example/mir2/v/release/bevy-runtime/v/${manifest.version}/pkg-${id}/${name}`;

function fixture(manifest, id, options = {}) {
  const backend = id === "webgpu" ? "webgpu" : "webgl2";
  const selected = manifest.packages.find((item) => item.id === id);
  const selectedUrls = artifactNames.map((name) => url(manifest, id, name));
  return {
    snapshot: {
      runtime: { selectedBackend: backend, compiledBackend: backend, runtimeVersion: manifest.version,
        fallbackFrom: options.fallbackFrom ?? null },
      packageId: id,
      runtimeUiCapabilities: { schemaVersion: 1, backend, questUiAbiVersion: selected.questUiAbiVersion,
        bagUiAbiVersion: selected.bagUiAbiVersion,
        primarySharedUiCompiled: selected.primarySharedUiCompiled,
        primarySharedUiStartup: id === "webgl2-shared" },
      stickyBoot: { attempted: true, booted: true, failed: false, backend, hasRuntime: true },
      runtimeResources: selectedUrls,
    },
    responses: selectedUrls.map((entry) => ({ url: entry, status: 200 })),
  };
}

test("verified three-package manifest adds exact shared scene; two-package and legacy omit it", () => {
  const shared = rawManifest(true);
  assert.throws(() => verifyBevyRuntimeVersion({ ...shared, version: "bevy-0000000000000000" }));
  assert.equal(runtimeSmokeScenarios(shared).find((item) => item.name === "force-webgl2-shared")?.query,
    "bevyRuntime=1&bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=1&bevyBagUi=1");
  assert.equal(runtimeSmokeScenarios(rawManifest(false)).some((item) => item.name === "force-webgl2-shared"), false);
  assert.equal(runtimeSmokeScenarios({ schemaVersion: 1, packages: [] }).some((item) => item.name === "force-webgl2-shared"), false);
});

test("actual schema2 package, six-field getter, sticky boot and current URL pair agree", () => {
  const manifest = rawManifest(true);
  for (const [name, id] of [["default", "webgpu"], ["force-webgl2", "webgl2"],
    ["force-webgl2-shared", "webgl2-shared"]]) {
    const { snapshot, responses } = fixture(manifest, id);
    assert.deepEqual(evaluateRuntimePackageEvidence(name, snapshot, responses, manifest), {
      currentRuntimePackageAgrees: true, runtimeUrlsUseCurrentVersion: true,
    });
  }
  const shared = fixture(manifest, "webgl2-shared");
  const assertions = scenarioAssertions("force-webgl2-shared", shared.snapshot,
    { responses: shared.responses, consoleErrors: [] }, manifest);
  assert.equal(assertions.movementShadowApiAvailable, true,
    "shared package smoke measures capability and URLs without injecting the offline motion fixture");
  assert.equal(assertions.usesRequestedSharedWebGl2, true);
});

test("wrong current version, package identity and compiled capability fail closed", () => {
  const manifest = rawManifest(true);
  const good = fixture(manifest, "webgl2");
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", {
    ...good.snapshot, runtime: { ...good.snapshot.runtime, runtimeVersion: "bevy-0000000000000000" },
  }, good.responses, manifest).currentRuntimePackageAgrees, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", {
    ...good.snapshot, packageId: "webgl2-shared",
  }, good.responses, manifest).currentRuntimePackageAgrees, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", {
    ...good.snapshot, runtimeUiCapabilities: { ...good.snapshot.runtimeUiCapabilities,
      primarySharedUiCompiled: true },
  }, good.responses, manifest).currentRuntimePackageAgrees, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", {
    ...good.snapshot, runtimeUiCapabilities: null,
  }, good.responses, manifest).currentRuntimePackageAgrees, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", {
    ...good.snapshot, stickyBoot: { ...good.snapshot.stickyBoot, attempted: false },
  }, good.responses, manifest).currentRuntimePackageAgrees, false);
});

test("wrong version or undeclared package in resources or responses is rejected", () => {
  const manifest = rawManifest(true);
  const good = fixture(manifest, "webgl2");
  const wrongVersion = url(manifest, "webgl2").replace(manifest.version, "bevy-0000000000000000");
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", {
    ...good.snapshot, runtimeResources: [wrongVersion],
  }, good.responses, manifest).runtimeUrlsUseCurrentVersion, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", good.snapshot,
    [{ url: wrongVersion, status: 200 }], manifest).runtimeUrlsUseCurrentVersion, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", { ...good.snapshot,
    runtimeResources: [good.snapshot.runtimeResources[0]],
  }, good.responses, manifest).runtimeUrlsUseCurrentVersion, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", good.snapshot,
    [good.responses[0]], manifest).runtimeUrlsUseCurrentVersion, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", {
    ...good.snapshot, runtimeResources: [...good.snapshot.runtimeResources, url(manifest, "webgl2-shared")],
  }, good.responses, manifest).runtimeUrlsUseCurrentVersion, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2-shared",
    fixture(manifest, "webgl2-shared").snapshot,
    [{ url: url(manifest, "webgpu"), status: 200 }], manifest).runtimeUrlsUseCurrentVersion, false);
});

test("only a recorded preboot GPU failure permits GPU plus lean GL2 URLs", () => {
  const manifest = rawManifest(true);
  const good = fixture(manifest, "webgl2", { fallbackFrom: "webgpu" });
  const gpuUrl = url(manifest, "webgpu");
  const mixedSnapshot = { ...good.snapshot, runtimeResources: [gpuUrl, ...good.snapshot.runtimeResources] };
  const mixedResponses = [{ url: gpuUrl, status: 500 }, ...good.responses];
  assert.equal(evaluateRuntimePackageEvidence("force-webgpu", mixedSnapshot, mixedResponses,
    manifest).runtimeUrlsUseCurrentVersion, true);
  assert.equal(evaluateRuntimePackageEvidence("force-webgl2", mixedSnapshot, mixedResponses,
    manifest).runtimeUrlsUseCurrentVersion, false);
  assert.equal(evaluateRuntimePackageEvidence("force-webgpu", {
    ...mixedSnapshot, runtime: { ...mixedSnapshot.runtime, fallbackFrom: null },
  }, mixedResponses, manifest).runtimeUrlsUseCurrentVersion, false);
  const expectedConsole = { source: "log", text: "Failed to load resource", url: gpuUrl };
  const client = { responses: mixedResponses, consoleErrors: [expectedConsole] };
  assert.equal(scenarioAssertions("force-webgpu", mixedSnapshot, client, manifest).packageFetchSucceeded, true);
  assert.equal(scenarioAssertions("force-webgpu", mixedSnapshot, client, manifest).noCriticalConsoleErrors, true);
  assert.equal(scenarioAssertions("force-webgl2", mixedSnapshot, client, manifest).packageFetchSucceeded, false);
  assert.equal(scenarioAssertions("force-webgl2", mixedSnapshot, client, manifest).noCriticalConsoleErrors, false);
});

test("legacy ordinary route may omit the additive getter", () => {
  const legacyRaw = { version: "", files: rawManifest(false).files };
  legacyRaw.version = computeBevyRuntimeVersion(legacyRaw);
  const legacy = verifyBevyRuntimeVersion(legacyRaw);
  const good = fixture(legacy, "webgl2");
  good.snapshot.runtimeUiCapabilities = null;
  assert.deepEqual(evaluateRuntimePackageEvidence("force-webgl2", good.snapshot, good.responses, legacy), {
    currentRuntimePackageAgrees: true, runtimeUrlsUseCurrentVersion: true,
  });
});
