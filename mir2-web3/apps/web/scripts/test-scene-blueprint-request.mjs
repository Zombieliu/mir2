import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/scene-blueprint-request.ts", import.meta.url));
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

const {
  SCENE_BLUEPRINT_SCHEMA_VERSION,
  createCrystalSceneBlueprintRequestKey,
  createCrystalSceneBlueprintRequestUrl,
  normalizeCrystalSceneBlueprintRequest,
} = loadedModule.exports;

test("nearby player positions share one canonical scene request", () => {
  const a = { mapFileName: "Map/0.map", centerX: 344, centerY: 74, width: 66, height: 68 };
  const b = { mapFileName: "0", centerX: 351, centerY: 83, width: 67, height: 65 };
  assert.equal(createCrystalSceneBlueprintRequestKey(a), createCrystalSceneBlueprintRequestKey(b));
  assert.deepEqual(normalizeCrystalSceneBlueprintRequest(a), {
    mapFileName: "0",
    centerX: 344,
    centerY: 76,
    width: 72,
    height: 72,
  });
});

test("the public request URL is canonical and schema-versioned", () => {
  const request = { mapFileName: "0.map", centerX: 347, centerY: 77, width: 67, height: 65 };
  const url = new URL(createCrystalSceneBlueprintRequestUrl(request), "https://mir2.example");
  assert.equal(url.pathname, "/api/scene/crystal");
  assert.equal(url.searchParams.get("v"), SCENE_BLUEPRINT_SCHEMA_VERSION);
  assert.equal(url.searchParams.get("map"), "0");
  assert.equal(url.searchParams.get("x"), "344");
  assert.equal(url.searchParams.get("y"), "76");
  assert.equal(url.searchParams.get("width"), "72");
  assert.equal(url.searchParams.get("height"), "72");
});
