import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const ts = (() => {
  try {
    return require("typescript");
  } catch {
    return require("../node_modules/.ignored/typescript/lib/typescript.js");
  }
})();
const modulePath = new URL("../app/components/original-client-weather.ts", import.meta.url);
const compiled = ts.transpileModule(readFileSync(modulePath, "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, strict: true },
  fileName: fileURLToPath(modulePath),
});
const module = { exports: {} };
new Function("exports", "module", "require", compiled.outputText)(module.exports, module, () => ({}));

const { CRYSTAL_WEATHER, crystalWeatherLayers, crystalWeatherTexturePath } = module.exports;
const visualLayersSource = readFileSync(
  new URL("../app/components/original-client-scene-visual-layers.tsx", import.meta.url),
  "utf8",
);
const globalCss = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");
assert.deepEqual(crystalWeatherLayers(0), []);
assert.deepEqual(crystalWeatherLayers(CRYSTAL_WEATHER.rain), [
  { key: "rain", frame: 164, className: "rain" },
]);
assert.equal(crystalWeatherLayers(CRYSTAL_WEATHER.snow | CRYSTAL_WEATHER.fog).length, 2);
assert.equal(crystalWeatherLayers(CRYSTAL_WEATHER.leaves).length, 3);
assert.equal(crystalWeatherLayers(CRYSTAL_WEATHER.fireParticle).length, 0);
assert.equal(crystalWeatherTexturePath(43), "/original-effects/Weather/43.png");
assert.throws(() => crystalWeatherTexturePath(42), /Unsupported/);
assert.match(visualLayersSource, /OriginalClientWeatherLayer weatherParticles=\{world\.weatherParticles\}/);
assert.match(globalCss, /\.viewport-crystal-weather-overlay/);
assert.match(globalCss, /pointer-events: none/);
assert.match(globalCss, /prefers-reduced-motion: reduce/);

for (const frame of [0, 1, 43, 164, 359, 531, 587]) {
  assert.ok(existsSync(new URL(`../public/original-effects/Weather/${frame}.png`, import.meta.url)));
}
console.log("Crystal weather rendering tests passed.");
