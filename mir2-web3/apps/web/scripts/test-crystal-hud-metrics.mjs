import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const ts = require("typescript");
const moduleUrl = new URL("../lib/crystal-hud-metrics.ts", import.meta.url);
const compiled = ts.transpileModule(readFileSync(moduleUrl, "utf8"), {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(moduleUrl),
});
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);

const { crystalMainHudExperienceBarFillWidth } = module.exports;
assert.equal(crystalMainHudExperienceBarFillWidth(0), 0);
assert.equal(crystalMainHudExperienceBarFillWidth(0.5), 500);
assert.equal(crystalMainHudExperienceBarFillWidth(1), 1001);
assert.equal(crystalMainHudExperienceBarFillWidth(-1), 0);
assert.equal(crystalMainHudExperienceBarFillWidth(2), 1001);
assert.equal(crystalMainHudExperienceBarFillWidth(Number.NaN), 0);
assert.equal(crystalMainHudExperienceBarFillWidth(Number.POSITIVE_INFINITY), 0);

console.log("Crystal HUD metric tests passed");
