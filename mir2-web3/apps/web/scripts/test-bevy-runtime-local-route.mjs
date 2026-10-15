import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";
import * as manifestModule from "../lib/bevy-runtime-manifest.mjs";

const module = { exports: {} };
const output = ts.transpileModule(readFileSync(new URL("../lib/bevy-runtime-local-route.ts", import.meta.url), "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;
new Function("exports", "module", "require", output)(module.exports, module, () => manifestModule);
const { bevyRuntimeLocalRewrite } = module.exports;

function fixture(shared = true) {
  const ids = shared ? ["webgpu", "webgl2", "webgl2-shared"] : ["webgpu", "webgl2"];
  return { schemaVersion: 2, version: "bevy-0123456789abcdef", packages: ids.map((id) => ({
    id, backend: id === "webgpu" ? "webgpu" : "webgl2", packageDir: `pkg-${id}`,
    questUiAbiVersion: shared ? 1 : 0, bagUiAbiVersion: shared ? 1 : 0,
    primarySharedUiCompiled: id === "webgl2-shared",
  })), files: ids.flatMap((id) => [".js", "_bg.wasm"].map((suffix) => ({
    path: `public/bevy-runtime/pkg-${id}/mir2_bevy_runtime${suffix}`, sha256: "a".repeat(64),
  }))) };
}

test("current declared pairs rewrite; stale versions, foreign artifacts and undeclared shared never do", () => {
  const bundle = fixture();
  for (const entry of bundle.files) {
    const suffix = entry.path.replace("public/bevy-runtime/", "");
    assert.equal(bevyRuntimeLocalRewrite(bundle, `/bevy-runtime/v/${bundle.version}/${suffix}`), `/bevy-runtime-releases/${bundle.version}/${suffix}`);
    assert.equal(bevyRuntimeLocalRewrite(bundle, `/bevy-runtime/v/bevy-ffffffffffffffff/${suffix}`), null);
  }
  for (const path of ["pkg-webgl2-shared/other.js", "pkg-other/mir2_bevy_runtime.js",
    "pkg-webgl2%2fmir2_bevy_runtime.js", "pkg-webgl2/../mir2_bevy_runtime.js"]) {
    assert.equal(bevyRuntimeLocalRewrite(bundle, `/bevy-runtime/v/${bundle.version}/${path}`), null);
  }
  assert.equal(bevyRuntimeLocalRewrite(fixture(false), `/bevy-runtime/v/${bundle.version}/pkg-webgl2-shared/mir2_bevy_runtime.js`), null);
  const incomplete = structuredClone(bundle);
  incomplete.files.pop();
  assert.equal(bevyRuntimeLocalRewrite(incomplete, `/bevy-runtime/v/${bundle.version}/pkg-webgpu/mir2_bevy_runtime.js`), null);
});

test("complete legacy pairs are guarded by their exact version; proxy does not expose an unconditional rewrite", () => {
  const manifest = fixture(false);
  const legacy = { version: manifest.version, files: manifest.files };
  assert.equal(bevyRuntimeLocalRewrite(legacy, `/bevy-runtime/v/${legacy.version}/pkg-webgl2/mir2_bevy_runtime.js`), `/bevy-runtime-releases/${legacy.version}/pkg-webgl2/mir2_bevy_runtime.js`);
  const proxy = readFileSync(new URL("../proxy.ts", import.meta.url), "utf8");
  assert.match(proxy, /bevyRuntimeLocalRewrite\(runtimeManifest, request\.nextUrl\.pathname\)/);
  assert.match(proxy, /status: 404/);
  assert.match(proxy, /"Cache-Control": "no-store"/);
  const config = readFileSync(new URL("../next.config.ts", import.meta.url), "utf8");
  assert.doesNotMatch(config, /destination: "\/bevy-runtime\/:backend/);
});
