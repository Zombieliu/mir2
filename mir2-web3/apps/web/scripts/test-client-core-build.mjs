import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const projectRoot = fileURLToPath(new URL("../../../", import.meta.url));
const webPath = "apps/web";
const bridgePath = "apps/game-client/platform-web";
const corePath = "apps/game-client/client-core";
const manifestRelative = `${webPath}/lib/generated/client_core_runtime.json`;
const manifest = JSON.parse(fs.readFileSync(path.join(projectRoot, manifestRelative), "utf8"));

function fixture(t) {
  const temporaryRoot = path.resolve(os.tmpdir());
  const root = fs.mkdtempSync(path.join(temporaryRoot, "mir2-client-core-verify-"));
  t.after(() => {
    if (path.dirname(path.resolve(root)) !== temporaryRoot || !path.basename(root).startsWith("mir2-client-core-verify-")) {
      throw new Error("Refusing cleanup outside the generated verification fixture");
    }
    fs.rmSync(root, { recursive: true, force: true });
  });
  for (const relative of [
    `${webPath}/scripts/build-client-core.mjs`,
    `${webPath}/scripts/lib/renderer-wasm-opt.mjs`, `${webPath}/scripts/lib/client-core-release-files.mjs`, manifestRelative,
    `${webPath}/public/client-core/${manifest.version}`,
    ...(manifest.presentation ? [`${webPath}/public/client-core/${manifest.presentation.version}`] : []),
    `${corePath}/Cargo.toml`, `${corePath}/src`,
    `${bridgePath}/Cargo.toml`, `${bridgePath}/Cargo.lock`,
    `${bridgePath}/rust-toolchain.toml`, `${bridgePath}/src`,
  ]) {
    fs.mkdirSync(path.dirname(path.join(root, relative)), { recursive: true });
    fs.cpSync(path.join(projectRoot, relative), path.join(root, relative), { recursive: true });
  }
  return root;
}

function run(root, overrides = {}) {
  return spawnSync(process.execPath, [path.join(root, webPath, "scripts/build-client-core.mjs")], {
    cwd: root, encoding: "utf8", shell: false,
    env: { ...process.env, MIR2_USE_PREBUILT_CLIENT_CORE: "1", MIR2_USE_PREBUILT_BEVY_RUNTIME: "0",
      CARGO_BIN: path.join(root, "missing-cargo"), WASM_BINDGEN_BIN: path.join(root, "missing-bindgen"), ...overrides },
  });
}

test("clean source/package copy supports both prebuilt switches without Rust", (t) => {
  const root = fixture(t);
  for (const overrides of [{}, { MIR2_USE_PREBUILT_CLIENT_CORE: undefined, MIR2_USE_PREBUILT_BEVY_RUNTIME: "1" }]) {
    const result = run(root, overrides);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /\[client-core\] verified/);
  }
  const explicitBuild = run(root, { MIR2_USE_PREBUILT_CLIENT_CORE: "0", MIR2_USE_PREBUILT_BEVY_RUNTIME: "1" });
  assert.notEqual(explicitBuild.status, 0);
  assert.match(explicitBuild.stderr, /requires wasm-bindgen/);
});

test("stale shared source cannot reuse a prebuilt package", (t) => {
  const root = fixture(t);
  fs.appendFileSync(path.join(root, corePath, "src/lib.rs"), "\n// changed source\n");
  const result = run(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Shared client source changed/);
});

test("corrupted WASM is rejected before publication", (t) => {
  const root = fixture(t);
  fs.appendFileSync(path.join(root, webPath, "public/client-core", manifest.version, "mir2_platform_web_bg.wasm"), "bad");
  const result = run(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /artifact mismatch/);
});

test("artifact hashes cannot bless bytes under the wrong immutable URL", (t) => {
  const root = fixture(t);
  const wrongVersion = manifest.version === "0".repeat(64) ? "1".repeat(64) : "0".repeat(64);
  const packages = path.join(root, webPath, "public/client-core");
  fs.cpSync(path.join(packages, manifest.version), path.join(packages, wrongVersion), { recursive: true });
  fs.writeFileSync(path.join(root, manifestRelative), JSON.stringify({ ...manifest, version: wrongVersion }));
  const result = run(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /immutable version URL/);
});

// These are data-only --verify checks: no WASM API or module instantiation.
test("new builder requires Presentation even though the legacy release reader supports old Core", (t) => {
  const root = fixture(t); const legacy = { ...manifest }; delete legacy.presentation;
  fs.writeFileSync(path.join(root, manifestRelative), JSON.stringify(legacy));
  const result = run(root);
  assert.notEqual(result.status, 0); assert.match(result.stderr, /requires its Presentation bundle/);
});

test("Presentation source fingerprint cannot reuse a stale prebuilt sibling", (t) => {
  assert.ok(manifest.presentation, "run the guarded dual build before this package verifier");
  const root = fixture(t); const changed = JSON.parse(JSON.stringify(manifest));
  changed.presentation.sourceSha256 = changed.sourceSha256 === "0".repeat(64) ? "1".repeat(64) : "0".repeat(64);
  fs.writeFileSync(path.join(root, manifestRelative), JSON.stringify(changed));
  const result = run(root);
  assert.notEqual(result.status, 0); assert.match(result.stderr, /Shared client source changed/);
});

test("Presentation byte drift is rejected independently of an intact Core", (t) => {
  assert.ok(manifest.presentation);
  const root = fixture(t);
  fs.appendFileSync(path.join(root, webPath, "public/client-core", manifest.presentation.version, "mir2_platform_web_bg.wasm"), "bad");
  const result = run(root);
  assert.notEqual(result.status, 0); assert.match(result.stderr, /artifact mismatch/);
});

test("Presentation hashes cannot bless bytes under an incorrect immutable URL", (t) => {
  assert.ok(manifest.presentation);
  const root = fixture(t); const wrong = "0".repeat(64) === manifest.version || "0".repeat(64) === manifest.presentation.version
    ? "1".repeat(64) : "0".repeat(64);
  const packages = path.join(root, webPath, "public/client-core");
  fs.cpSync(path.join(packages, manifest.presentation.version), path.join(packages, wrong), { recursive: true });
  fs.writeFileSync(path.join(root, manifestRelative), JSON.stringify({ ...manifest, presentation: { ...manifest.presentation, version: wrong } }));
  const result = run(root);
  assert.notEqual(result.status, 0); assert.match(result.stderr, /immutable version URL/);
});

test("builder rejects undeclared Presentation metadata and exact per-package byte boundaries", (t) => {
  assert.ok(manifest.presentation);
  const root = fixture(t);
  for (const mutate of [
    (value) => { value.presentation.extra = true; },
    (value) => { value.presentation.version = value.version; },
    (value) => { value.presentation.files["mir2_platform_web_bg.wasm"].bytes = 262144; },
    (value) => { value.presentation.files["mir2_platform_web.js"].bytes = 204801; },
  ]) {
    const value = JSON.parse(JSON.stringify(manifest)); mutate(value);
    fs.writeFileSync(path.join(root, manifestRelative), JSON.stringify(value));
    const result = run(root);
    assert.notEqual(result.status, 0); assert.match(result.stderr, /undeclared fields|distinct immutable versions|byte budget/);
  }
});
