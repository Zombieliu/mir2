import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/render-tier.ts", import.meta.url));
const compiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: sourcePath,
  reportDiagnostics: true,
});
const errors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(errors, []);

const loadedModule = { exports: {} };
new Function("exports", "module", compiled.outputText)(loadedModule.exports, loadedModule);
const { normalizeDeviceMemoryGiB, resolveRenderTier } = loadedModule.exports;

test("normalizes optional Device Memory values", () => {
  assert.equal(normalizeDeviceMemoryGiB(undefined), null);
  assert.equal(normalizeDeviceMemoryGiB(0), null);
  assert.equal(normalizeDeviceMemoryGiB("4"), 4);
});

test("classifies low-memory, touch, and constrained-texture devices as low tier", () => {
  assert.equal(resolveRenderTier({ deviceMemoryGiB: 2 }), "low");
  assert.equal(resolveRenderTier({ deviceMemoryGiB: 8, coarsePointer: true }), "low");
  assert.equal(resolveRenderTier({ deviceMemoryGiB: 8, maxTextureSize: 4096 }), "low");
});

test("uses conservative defaults and allows deterministic QA overrides", () => {
  assert.equal(resolveRenderTier({}), "medium");
  assert.equal(resolveRenderTier({ deviceMemoryGiB: 16 }), "high");
  assert.equal(resolveRenderTier({ deviceMemoryGiB: 2, forcedTier: "high" }), "high");
  assert.equal(resolveRenderTier({ deviceMemoryGiB: 16, forcedTier: "low" }), "low");
});
