import assert from "node:assert/strict";
import { test } from "node:test";
import { validateBevyRuntimeManifest } from "../lib/bevy-runtime-manifest.mjs";

const hash = (digit) => digit.repeat(64);
const version = "bevy-0123456789abcdef";
const ids = ["webgpu", "webgl2", "webgl2-shared"];
const pair = (id) => [
  { path: `public/bevy-runtime/pkg-${id}/mir2_bevy_runtime.js`, sha256: hash("a") },
  { path: `public/bevy-runtime/pkg-${id}/mir2_bevy_runtime_bg.wasm`, sha256: hash("b") },
];
const pkg = (id, abi = 1) => ({ id, backend: id === "webgpu" ? "webgpu" : "webgl2",
  packageDir: `pkg-${id}`, questUiAbiVersion: abi, bagUiAbiVersion: abi,
  primarySharedUiCompiled: id === "webgl2-shared" });
const schema2 = (withShared = true) => {
  const selected = withShared ? ids : ids.slice(0, 2);
  return { schemaVersion: 2, version, packages: selected.map((id) => pkg(id)),
    files: selected.flatMap(pair) };
};
const legacy = () => ({ version, files: ids.slice(0, 2).flatMap(pair) });

test("schema2 normalizes complete declared packages and immutable metadata", () => {
  const normalized = validateBevyRuntimeManifest(schema2(), "fixture");
  assert.equal(normalized.schemaVersion, 2);
  assert.deepEqual(normalized.packages.map((item) => item.id), ids);
  assert.equal(normalized.version, version);
  assert.ok(Object.isFrozen(normalized) && Object.isFrozen(normalized.packages)
    && normalized.packages.every(Object.isFrozen) && normalized.files.every(Object.isFrozen));
  assert.deepEqual(validateBevyRuntimeManifest(schema2(false)).packages.map((item) => item.id), ids.slice(0, 2));
});

test("legacy has exactly four valid files and unknown UI capabilities", () => {
  const normalized = validateBevyRuntimeManifest(legacy());
  assert.equal(normalized.schemaVersion, 1);
  assert.equal(normalized.files.length, 4);
  assert.deepEqual(normalized.packages.map((item) => item.primarySharedUiCompiled), [null, null]);
  assert.deepEqual(normalized.packages.map((item) => item.questUiAbiVersion), [null, null]);
});

test("invalid versions and malformed legacy metadata are rejected", () => {
  for (const changed of [
    { ...legacy(), version: "bevy-0123456789abcdeF" },
    { ...legacy(), version: "local" },
    { ...legacy(), schemaVersion: 2 },
    { ...legacy(), packages: [] },
    { ...legacy(), files: legacy().files.slice(0, 3) },
    { ...legacy(), extra: true },
  ]) assert.throws(() => validateBevyRuntimeManifest(changed));
});

test("schema2 rejects missing, duplicate, undeclared, and noncanonical artifact files", () => {
  const valid = schema2();
  for (const files of [
    valid.files.slice(0, -1),
    [...valid.files.slice(0, -1), valid.files[0]],
    [...valid.files.slice(0, -1), { path: "public/bevy-runtime/pkg-other/mir2_bevy_runtime_bg.wasm", sha256: hash("c") }],
    [...valid.files.slice(0, -1), { path: "public/bevy-runtime/pkg-webgl2-shared/../pkg-webgl2/mir2_bevy_runtime_bg.wasm", sha256: hash("c") }],
    [...valid.files.slice(0, -1), { ...valid.files.at(-1), sha256: hash("A") }],
  ]) assert.throws(() => validateBevyRuntimeManifest({ ...valid, files }));
});

test("schema2 enforces IDs, backends, strict booleans, and ABI pairing", () => {
  const valid = schema2();
  const withPackage = (candidate, index = 2) => ({ ...valid,
    packages: valid.packages.map((item, at) => at === index ? candidate : item) });
  for (const changed of [
    withPackage({ ...pkg("webgl2-shared"), primarySharedUiCompiled: 1 }),
    withPackage({ ...pkg("webgl2-shared"), primarySharedUiCompiled: false }),
    withPackage({ ...pkg("webgl2-shared"), questUiAbiVersion: 0, bagUiAbiVersion: 0 }),
    withPackage({ ...pkg("webgl2-shared"), backend: "webgpu" }),
    withPackage({ ...pkg("webgl2-shared"), packageDir: "../pkg-webgl2-shared" }),
    withPackage(pkg("webgl2")),
    withPackage({ ...pkg("webgl2"), bagUiAbiVersion: 0 }, 1),
    withPackage({ ...pkg("webgpu"), questUiAbiVersion: 0, bagUiAbiVersion: 0 }, 0),
    { ...valid, packages: [pkg("webgpu", 0), pkg("webgl2", 0), pkg("webgl2-shared", 1)] },
  ]) assert.throws(() => validateBevyRuntimeManifest(changed));
  assert.equal(validateBevyRuntimeManifest({ ...schema2(false),
    packages: [pkg("webgpu", 0), pkg("webgl2", 0)] }).packages[0].questUiAbiVersion, 0);
});

test("schema2 allows truthful lean ABI0 only beside GPU ABI1 and shared ABI1", () => {
  const mixed = { ...schema2(), packages: [pkg("webgpu", 1), pkg("webgl2", 0), pkg("webgl2-shared", 1)] };
  const normalized = validateBevyRuntimeManifest(mixed);
  assert.deepEqual(normalized.packages.map(({ questUiAbiVersion, bagUiAbiVersion, primarySharedUiCompiled }) =>
    [questUiAbiVersion, bagUiAbiVersion, primarySharedUiCompiled]), [[1, 1, false], [0, 0, false], [1, 1, true]]);
  assert.deepEqual(normalized.files, mixed.files);
  assert.ok(normalized.packages.every(Object.isFrozen));
  assert.deepEqual(validateBevyRuntimeManifest(schema2()).packages.map((item) => item.questUiAbiVersion), [1, 1, 1]);
  for (const abi of [0, 1]) assert.deepEqual(validateBevyRuntimeManifest({ ...schema2(false),
    packages: [pkg("webgpu", abi), pkg("webgl2", abi)] }).packages.map((item) => item.bagUiAbiVersion), [abi, abi]);
  for (const packages of [[pkg("webgpu", 1), pkg("webgl2", 0)], [pkg("webgpu", 0), pkg("webgl2", 1)]]) {
    assert.throws(() => validateBevyRuntimeManifest({ ...schema2(false), packages }));
  }
  for (const candidate of [
    { ...pkg("webgl2", 0), questUiAbiVersion: "0" },
    { ...pkg("webgl2", 0), bagUiAbiVersion: 1 },
    { ...pkg("webgl2", 0), primarySharedUiCompiled: true },
    { ...pkg("webgl2", 0), primarySharedUiCompiled: 0 },
    { ...pkg("webgl2", 0), extra: true },
  ]) assert.throws(() => validateBevyRuntimeManifest({ ...mixed,
    packages: [pkg("webgpu", 1), candidate, pkg("webgl2-shared", 1)] }));
  assert.throws(() => validateBevyRuntimeManifest({ ...mixed,
    packages: [pkg("webgpu", 0), pkg("webgl2", 0), pkg("webgl2-shared", 1)] }));
});

export { schema2, legacy };
