import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";

import ts from "typescript";

const sourceUrl = new URL(
  "../app/components/original-client-entity-animation-runtime.ts",
  import.meta.url,
);
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

const { animationEventForEntity } = loaded.exports;

test("maps the death window to Die and the corpse state to Dead", () => {
  const monster = {
    objectId: "monster-12",
    kind: "monster",
    direction: "Down",
    dead: true,
    dieStartedAt: 12_345,
  };

  assert.deepEqual(animationEventForEntity(monster, "dying"), {
    action: "die",
    actionToken: "life:12345",
  });
  assert.deepEqual(animationEventForEntity(monster, "dead"), {
    action: "dead",
    actionToken: "life:12345",
  });
});

test("a new death incarnation receives a new action token", () => {
  const monster = {
    objectId: "monster-12",
    kind: "monster",
    direction: "Down",
    dead: true,
    dieStartedAt: 12_346,
  };

  assert.equal(
    animationEventForEntity(monster, "dying").actionToken,
    "life:12346",
  );
});
