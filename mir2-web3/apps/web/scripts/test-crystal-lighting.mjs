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
const globalCss = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");
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

const {
  crystalLightTexturePath,
  crystalMapLightSpec,
  crystalMapLightTopLeft,
  crystalObjectLightSpec,
  crystalObjectLightTopLeft,
  crystalSceneLightClassName,
} = module.exports;

assert.equal(crystalSceneLightClassName(1), "dawn");
assert.equal(crystalSceneLightClassName(2), null);
assert.equal(crystalSceneLightClassName(3), "evening");
assert.equal(crystalSceneLightClassName(4), "night");
assert.equal(crystalLightTexturePath(0), "/original-effects/Lighting/0.png");
assert.equal(crystalLightTexturePath(9), "/original-effects/Lighting/9.png");
assert.throws(() => crystalLightTexturePath(10), /must be 0\.\.9/);

assert.deepEqual(crystalObjectLightSpec({ kind: "npc" }, false), {
  value: 10,
  range: 9,
  strengthBucket: 0,
  width: 925,
  height: 703,
  placementWidth: 845,
  placementHeight: 642,
  opacity: 120 / 255,
  tone: "merchant",
});
assert.equal(crystalObjectLightSpec({ kind: "selfPlayer" }, true)?.value, 3);
assert.equal(crystalObjectLightSpec({ kind: "player", light: 31 }, false)?.strengthBucket, 2);
assert.equal(crystalObjectLightSpec({ kind: "player", light: 31 }, false)?.opacity, 180 / 255);
assert.equal(crystalObjectLightSpec({ kind: "monster", light: 4, dead: true }, false), null);
assert.equal(crystalObjectLightSpec({ kind: "selfPlayer", light: 4, dead: true }, true)?.value, 4);
assert.equal(crystalObjectLightSpec({ kind: "monster", light: 0 }, false), null);

assert.deepEqual(crystalMapLightSpec(1), {
  value: 1,
  range: 3,
  width: 445,
  height: 338,
  placementWidth: 365,
  placementHeight: 277,
  opacity: 1,
  tone: "neutral",
});
assert.deepEqual(crystalMapLightSpec(2), {
  value: 2,
  range: 6,
  width: 685,
  height: 521,
  placementWidth: 605,
  placementHeight: 460,
  opacity: 1,
  tone: "neutral",
});
assert.equal(crystalMapLightSpec(10), null);
assert.equal(crystalMapLightSpec(0), null);

const npcLight = crystalObjectLightSpec({ kind: "npc" }, false);
assert.deepEqual(crystalObjectLightTopLeft(480, 352, npcLight), { left: 34, top: 10 });
assert.deepEqual(
  crystalMapLightTopLeft(480, 352, -51, -113, crystalMapLightSpec(5)),
  { left: -7, top: -71 },
);

const lightBlockStart = globalCss.indexOf(".viewport-map-light,\n.viewport-object-light {");
assert.notEqual(lightBlockStart, -1, "shared light texture CSS block must exist");
const lightBlockEnd = globalCss.indexOf("\n}", lightBlockStart);
assert.notEqual(lightBlockEnd, -1, "shared light texture CSS block must close");
const lightBlock = globalCss.slice(lightBlockStart, lightBlockEnd);
assert.match(lightBlock, /mix-blend-mode:\s*plus-lighter/);
assert.doesNotMatch(lightBlock, /radial-gradient/);

const mapLightBlockStart = globalCss.indexOf(
  ".viewport-crystal-light-overlay.dawn .viewport-map-light,\n" +
    ".viewport-crystal-light-overlay.evening .viewport-map-light {",
);
assert.notEqual(mapLightBlockStart, -1, "map light calibration CSS block must exist");
const mapLightBlockEnd = globalCss.indexOf("\n}", mapLightBlockStart);
assert.notEqual(mapLightBlockEnd, -1, "map light calibration CSS block must close");
const mapLightBlock = globalCss.slice(mapLightBlockStart, mapLightBlockEnd);
assert.match(mapLightBlock, /filter:\s*brightness\(1\.9\)/);
assert.match(mapLightBlock, /transform:\s*translateY\(24px\)/);
assert.doesNotMatch(mapLightBlock, /\.night/);

const expectedTextureSizes = [
  [205, 156],
  [285, 217],
  [365, 277],
  [445, 338],
  [525, 399],
  [605, 460],
  [685, 521],
  [765, 581],
  [845, 642],
  [925, 703],
];
for (let range = 0; range < expectedTextureSizes.length; range += 1) {
  const texture = readFileSync(new URL(`../public/original-effects/Lighting/${range}.png`, import.meta.url));
  assert.equal(texture.toString("ascii", 1, 4), "PNG");
  assert.deepEqual(
    [texture.readUInt32BE(16), texture.readUInt32BE(20)],
    expectedTextureSizes[range],
    `light texture ${range} dimensions`,
  );
}

console.log("crystal lighting logic tests passed");
