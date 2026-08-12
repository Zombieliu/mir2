import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourcePath = new URL("../lib/quest-entity-binding.ts", import.meta.url);
const source = readFileSync(sourcePath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(sourcePath),
});
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);

const { questIdsFromPacket } = module.exports;
const previous = [1001];

assert.equal(questIdsFromPacket(undefined, previous), previous);
assert.equal(questIdsFromPacket(null, previous), previous);
assert.deepEqual(questIdsFromPacket([1, 2, 1, 5], previous), [1, 2, 5]);
assert.deepEqual(questIdsFromPacket([1, "2", null, 3.5, Number.NaN], previous), [1]);
assert.deepEqual(questIdsFromPacket([], previous), []);

const pageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
assert.match(
  pageSource,
  /questIds:\s*questIdsFromPacket\(\s*payload\.questIds,\s*previousEntity\?\.questIds,?\s*\)/,
  "NPC packet ingestion must retain the server-provided quest binding",
);

console.log("quest entity binding tests passed");
