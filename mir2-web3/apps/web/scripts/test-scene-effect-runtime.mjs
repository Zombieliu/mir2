import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTsModule(url, deps = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    fileName: fileURLToPath(url),
  });
  const mod = { exports: {} };
  const require = (specifier) => {
    if (specifier in deps) return deps[specifier];
    throw new Error(`Unexpected dependency: ${specifier}`);
  };
  new Function("exports", "module", "require", compiled.outputText)(mod.exports, mod, require);
  return mod.exports;
}

const animation = {
  name: "FireBall",
  kind: "cast",
  frames: [
    { path: "/fx/0.png", width: 16, height: 20, x: -8, y: -15 },
    { path: "/fx/1.png", width: 18, height: 22, x: -9, y: -16 },
  ],
  interval: 100,
  blend: true,
  light: 2,
  repeat: false,
  offset: { x: 1, y: 2 },
  durationMs: 200,
};
const crystal = {
  effectNameForNumber: (_assets, value) => (value === 31 ? "FireBall" : null),
  resolveSpellEffect: (_assets, name, direction) => (name === "FireBall" && direction === 3 ? animation : null),
  resolveMapEffect: (_assets, name) => name === "FireBall" ? { ...animation, repeat: true } : null,
  resolveMapEffectByNumber: () => null,
  effectFrameAt: (instance, now) => {
    let index = Math.floor((now - instance.startedAt) / instance.animation.interval);
    if (instance.animation.repeat) index %= instance.animation.frames.length;
    return instance.animation.frames[index] ?? null;
  },
};
const runtime = loadTsModule(new URL("../lib/scene-effect-runtime.ts", import.meta.url), {
  "./crystal-magic-effects": crystal,
});
const visualLayersSource = readFileSync(
  new URL("../app/components/original-client-scene-visual-layers.tsx", import.meta.url),
  "utf8",
);
const shellSource = readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8");
const globalCssSource = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");
const resolvedEffectLayerSource = visualLayersSource.slice(
  visualLayersSource.indexOf("resolvedEffectFrames.map"),
  visualLayersSource.indexOf("viewportProjectiles.map"),
);
assert.match(
  resolvedEffectLayerSource,
  /VIEWPORT_ENTITY_LEFT_ORIGIN[\s\S]*VIEWPORT_ENTITY_TOP_ORIGIN/,
  "Crystal effect frames must anchor from the tile top-left DrawLocation",
);
assert.doesNotMatch(
  resolvedEffectLayerSource,
  /VIEWPORT_TILE_CENTER_[XY]/,
  "effect metadata offsets must not receive a second half-cell center offset",
);
assert.equal(runtime.CRYSTAL_ADDITIVE_MIX_BLEND_MODE, "plus-lighter");
assert.match(shellSource, /className={`game-world-composite/, "world renderers share one compositing root");
assert.match(
  globalCssSource,
  /\.game-world-composite\s*\{[^}]*isolation:\s*isolate;/s,
  "the shared world compositor bounds blending away from the HUD",
);
assert.match(
  globalCssSource,
  /\.viewport-sprite-overlay\s*\{[^}]*z-index:\s*auto;/s,
  "the sprite overlay must not isolate additive effects from the map backdrop",
);
const base = {
  key: "fx-1",
  source: "spell",
  spellOrEffect: 31,
  x: 10,
  y: 20,
  direction: 3,
  value: 0,
  startedAt: 1_000,
  expiresAt: 2_000,
};

assert.equal(runtime.resolveSceneEffectFrame({}, base, 999), null, "delay is authoritative");
assert.equal(runtime.resolveSceneEffectFrame({}, base, 1_000).frame.path, "/fx/0.png");
assert.equal(runtime.resolveSceneEffectFrame({}, base, 1_100).frame.path, "/fx/1.png");
assert.equal(runtime.resolveSceneEffectFrame({}, base, 1_200), null, "non-repeat animation ends");
assert.equal(runtime.resolveSceneEffectFrame({}, base, 2_000), null, "packet lifetime expires");
assert.equal(
  runtime.collectResolvedSceneEffectFrames({}, [base, { ...base, key: "unknown", spellOrEffect: 999 }], 1_000).length,
  1,
);

const worldSpell = {
  ...base,
  key: "crystal-world-spell:239",
  source: "objectSpell",
  objectId: "239",
  expiresAt: Number.MAX_SAFE_INTEGER,
};
assert.equal(
  runtime.resolveSceneEffectFrame({}, worldSpell, 1_200).frame.path,
  "/fx/0.png",
  "ObjectSpell resolves the repeating ground animation after the cast animation would end",
);

console.log("scene effect runtime: 6 passed");
