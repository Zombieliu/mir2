import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";

import ts from "typescript";

const sourceUrl = new URL("../lib/bevy-entity-atlas-policy.ts", import.meta.url);
const source = readFileSync(sourceUrl, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(sourceUrl),
  reportDiagnostics: true,
});
assert.deepEqual(
  (compiled.diagnostics ?? []).filter(
    (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
  ),
  [],
);
const loaded = { exports: {} };
new Function("exports", "module", compiled.outputText)(loaded.exports, loaded);

const {
  bevyEntityAtlasCandidateHasCoverage,
  resolveBevyEntityAtlasPolicy,
} = loaded.exports;

test("defaults to stable prebuilt atlases without dynamic repacking", () => {
  assert.equal(resolveBevyEntityAtlasPolicy({}), "stable");
  assert.equal(resolveBevyEntityAtlasPolicy({ storedValue: "0" }), "disabled");
  assert.equal(resolveBevyEntityAtlasPolicy({ queryValue: "0", storedValue: "1" }), "disabled");
  assert.equal(resolveBevyEntityAtlasPolicy({ queryValue: "1", storedValue: "0" }), "dynamic");
  assert.equal(resolveBevyEntityAtlasPolicy({ queryValue: "dynamic" }), "dynamic");
});

test("accepts a stable prebuilt atlas when it covers any visible layer", () => {
  const visible = new Set(["player", "monster"]);
  assert.equal(bevyEntityAtlasCandidateHasCoverage(["player", "npc"], visible), true);
  assert.equal(bevyEntityAtlasCandidateHasCoverage(["npc"], visible), false);
});
