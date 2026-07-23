import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const ts = require("typescript");
const moduleUrl = new URL("../lib/world-model/start-game-boundary.ts", import.meta.url);
const source = readFileSync(moduleUrl, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(moduleUrl),
});
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);
const { resetWorldPopulationForStartGame } = module.exports;

const self = { objectId: "1000", kind: "selfPlayer", name: "Scout", x: 330, y: 270 };
const staleNpc = { objectId: "4001", kind: "npc", name: "Village Guide", x: 326, y: 271 };
const terrainPatches = [{ x: 0, y: 0, width: 1, height: 1, kind: "grass" }];
const current = {
  playerObjectId: self.objectId,
  selectedObjectId: staleNpc.objectId,
  activeNpcDialog: { objectId: staleNpc.objectId },
  entities: [self, staleNpc],
  groundDrops: [{ objectId: "8001", x: 330, y: 270 }],
  terrainPatches,
  worldTick: 42,
};

const reset = resetWorldPopulationForStartGame(current);
assert.notStrictEqual(reset, current);
assert.deepEqual(reset.entities, [self], "the existing self entity and transient fields must survive");
assert.deepEqual(reset.groundDrops, [], "pre-auth drops must not survive StartGame");
assert.equal(reset.selectedObjectId, null);
assert.equal(reset.activeNpcDialog, null);
assert.strictEqual(reset.terrainPatches, terrainPatches, "unrelated scene state must remain untouched");
assert.equal(reset.worldTick, 42);

const withoutSelf = resetWorldPopulationForStartGame({
  ...current,
  playerObjectId: null,
});
assert.deepEqual(withoutSelf.entities, [], "a pre-auth population with no self must be cleared fully");

console.log("start-game population boundary tests passed");
