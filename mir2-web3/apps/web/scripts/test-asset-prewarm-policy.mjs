import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/asset-prewarm-policy.ts", import.meta.url));
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
const { resolveAssetPrewarmPolicy } = loadedModule.exports;

test("low tier bounds scene prewarm and disables background packs by default", () => {
  assert.deepEqual(resolveAssetPrewarmPolicy("low"), {
    tier: "low",
    criticalConcurrency: 3,
    backgroundConcurrency: 1,
    maxSceneFrames: 192,
    backgroundMode: "off",
  });
});

test("medium and high tiers preserve progressively more eager prewarming", () => {
  assert.deepEqual(resolveAssetPrewarmPolicy("medium"), {
    tier: "medium",
    criticalConcurrency: 5,
    backgroundConcurrency: 2,
    maxSceneFrames: 480,
    backgroundMode: "afterPlayable",
  });
  assert.deepEqual(resolveAssetPrewarmPolicy("high"), {
    tier: "high",
    criticalConcurrency: 8,
    backgroundConcurrency: 3,
    maxSceneFrames: null,
    backgroundMode: "afterPlayable",
  });
});

test("an explicit background mode remains available for deterministic QA", () => {
  assert.equal(resolveAssetPrewarmPolicy("low", "immediate").backgroundMode, "immediate");
  assert.equal(resolveAssetPrewarmPolicy("high", "off").backgroundMode, "off");
});
