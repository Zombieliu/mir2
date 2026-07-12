import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
let ts;
try {
  ts = require("typescript");
} catch {
  ts = require("../node_modules/.ignored/typescript/lib/typescript.js");
}

const modulePath = new URL(
  "../app/components/original-client-scene-lighting.ts",
  import.meta.url,
);
const source = readFileSync(modulePath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(modulePath),
});
const module = { exports: {} };
new Function("exports", "module", "require", compiled.outputText)(
  module.exports,
  module,
  () => ({}),
);

const { crystalObjectLightSpec, crystalSceneLightClassName } = module.exports;

assert.equal(crystalSceneLightClassName(1), "dawn");
assert.equal(crystalSceneLightClassName(2), null);
assert.equal(crystalSceneLightClassName(3), "evening");
assert.equal(crystalSceneLightClassName(4), "night");

assert.deepEqual(crystalObjectLightSpec({ kind: "npc" }, false), {
  value: 10,
  range: 9,
  strengthBucket: 0,
  width: 845,
  height: 642,
  opacity: (120 / 255) * 0.28,
  tone: "merchant",
});
assert.equal(crystalObjectLightSpec({ kind: "selfPlayer" }, true)?.value, 3);
assert.equal(crystalObjectLightSpec({ kind: "player", light: 31 }, false)?.strengthBucket, 2);
assert.equal(crystalObjectLightSpec({ kind: "player", light: 31 }, false)?.opacity, (180 / 255) * 0.28);
assert.equal(crystalObjectLightSpec({ kind: "monster", light: 4, dead: true }, false), null);
assert.equal(crystalObjectLightSpec({ kind: "selfPlayer", light: 4, dead: true }, true)?.value, 4);
assert.equal(crystalObjectLightSpec({ kind: "monster", light: 0 }, false), null);

console.log("crystal lighting logic tests passed");
