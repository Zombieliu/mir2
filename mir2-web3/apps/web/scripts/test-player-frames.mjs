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
  "../app/components/original-client-player-frames.ts",
  import.meta.url,
);
const compiled = ts.transpileModule(readFileSync(modulePath, "utf8"), {
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

const { crystalMountFrameIndex, crystalPlayerAnimationMeta, crystalPlayerFrameIndex } = module.exports;
const entityFrameModulePath = new URL(
  "../app/components/original-client-entity-frames.ts",
  import.meta.url,
);
const entityFrameCompiled = ts.transpileModule(readFileSync(entityFrameModulePath, "utf8"), {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(entityFrameModulePath),
});
const entityFrameModule = { exports: {} };
new Function("exports", "module", "require", entityFrameCompiled.outputText)(
  entityFrameModule.exports,
  entityFrameModule,
  () => ({}),
);
const { crystalEntityAnimationMeta } = entityFrameModule.exports;
const frameSetCatalog = JSON.parse(
  readFileSync(new URL("../public/original-ui/frame-sets.generated.json", import.meta.url), "utf8"),
);
const frameSetFor = (libraryKey) => ({
  count: frameSetCatalog.libraries[libraryKey].actionCount,
  actions: frameSetCatalog.libraries[libraryKey].actions,
});
const right = 2;

assert.deepEqual(crystalPlayerAnimationMeta("running", 0, 0), {
  frameBaseOffset: 80,
  mountFrameBaseOffset: undefined,
  weaponFrameOffset: 80,
  frameCount: 6,
  directionStride: 6,
  frameIntervalMs: 100,
  reverse: undefined,
});
assert.equal(crystalPlayerAnimationMeta("attack1", 0, 0).weaponFrameOffset, 136);
assert.equal(crystalPlayerAnimationMeta("struck", 0, 0).weaponFrameOffset, 360);
assert.equal(crystalPlayerAnimationMeta("dead", 0, 0).directionStride, 4);
assert.deepEqual(crystalPlayerAnimationMeta("spell", 0, 0), {
  frameBaseOffset: 296,
  mountFrameBaseOffset: undefined,
  weaponFrameOffset: 296,
  frameCount: 6,
  directionStride: 6,
  frameIntervalMs: 100,
  reverse: undefined,
});

assert.deepEqual(
  Array.from({ length: 8 }, (_, phase) => crystalPlayerFrameIndex("mountWalking", right, phase)),
  [464, 465, 466, 467, 468, 469, 470, 471],
);
assert.deepEqual(
  Array.from({ length: 8 }, (_, phase) => crystalMountFrameIndex("mountWalking", right, phase)),
  [48, 49, 50, 51, 52, 53, 54, 55],
);
assert.deepEqual(
  Array.from({ length: 6 }, (_, phase) => crystalPlayerFrameIndex("mountRunning", right, phase)),
  [524, 525, 526, 527, 528, 529],
);
assert.deepEqual(
  Array.from({ length: 6 }, (_, phase) => crystalMountFrameIndex("mountRunning", right, phase)),
  [108, 109, 110, 111, 112, 113],
);
assert.deepEqual(
  Array.from({ length: 6 }, (_, phase) => crystalPlayerFrameIndex("running", right, phase)),
  [92, 93, 94, 95, 96, 97],
);
assert.deepEqual(
  Array.from({ length: 6 }, (_, phase) => crystalPlayerFrameIndex("attack1", right, phase)),
  [148, 149, 150, 151, 152, 153],
);
assert.deepEqual(
  Array.from({ length: 3 }, (_, phase) => crystalPlayerFrameIndex("struck", right, phase)),
  [366, 367, 368],
);
assert.deepEqual(
  Array.from({ length: 4 }, (_, phase) => crystalPlayerFrameIndex("dying", right, phase)),
  [392, 393, 394, 395],
);
assert.equal(crystalPlayerFrameIndex("dead", right, 0), 395);
assert.deepEqual(
  Array.from({ length: 6 }, (_, phase) => crystalPlayerFrameIndex("spell", right, phase)),
  [308, 309, 310, 311, 312, 313],
);

assert.deepEqual(crystalEntityAnimationMeta(frameSetFor("Monster/000"), "walking"), {
  frameBaseOffset: 32,
  frameCount: 6,
  directionStride: 6,
  frameIntervalMs: 100,
  reverse: undefined,
  blend: undefined,
});
assert.equal(crystalEntityAnimationMeta(frameSetFor("Monster/003"), "reviving").reverse, true);
assert.equal(crystalEntityAnimationMeta(frameSetFor("Dragon"), "standing").directionStride, 0);
assert.equal(crystalEntityAnimationMeta(frameSetFor("Monster/182"), "standing").blend, true);
assert.deepEqual(crystalEntityAnimationMeta(frameSetFor("NPC/155"), "standing").effect, {
  frameBaseOffset: 2,
  frameCount: 8,
  directionStride: 0,
  frameIntervalMs: 100,
  reverse: undefined,
  blend: undefined,
});

const renderingSource = readFileSync(
  new URL("../app/components/original-client-scene-rendering.tsx", import.meta.url),
  "utf8",
);
const shellSource = readFileSync(
  new URL("../app/original-client-shell.tsx", import.meta.url),
  "utf8",
);
const visualLayersSource = readFileSync(
  new URL("../app/components/original-client-scene-visual-layers.tsx", import.meta.url),
  "utf8",
);
const globalCssSource = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");
assert.match(
  renderingSource,
  /frameLayersForIndices\(libraries\[mountLibraryKey\], mountFrameIndices, fallbackMountFrameIndex\)/,
  "mounted phases must be included in the preload frame set",
);
assert.match(
  renderingSource,
  /entity\.kind === "npc" \|\| entity\.kind === "monster" \? 24 : 25/,
  "nameplates must use Crystal's fixed 48/50px DisplayRectangle centers",
);
assert.match(
  renderingSource,
  /displayRectangleOffset[\s\S]*\? -18 : -17/,
  "nameplates must use Crystal's fixed player/NPC vertical formulas",
);
assert.match(
  visualLayersSource,
  /className="entity-health-bar entity-overlay-health-bar"[\s\S]*\+ 8\}px`[\s\S]*- 64\}px`/,
  "self HP must remain in the independent overlay at Crystal's X+8, Y-64 anchor",
);
assert.match(
  globalCssSource,
  /\.entity-nameplate \{[\s\S]*transform: translate\(-50%, 0\);/,
  "nameplate top coordinates must not receive a second label-height translation",
);
assert.match(
  renderingSource,
  /crystalEntityAnimationMeta\([\s\S]*bodyLibrary\?\.frameSet/,
  "NPC and monster actions must prefer their original library FrameSet",
);
assert.match(
  renderingSource,
  /const stride = Math\.max\(directionStride, 0\)/,
  "negative Skip must be allowed to produce a zero direction stride",
);
assert.match(
  renderingSource,
  /animation\.effect\.frameBaseOffset[\s\S]*animation\.effect\.directionStride/,
  "FrameSet secondary effect tracks must select frames with their own timing and stride",
);
const spriteMetaSource = readFileSync(
  new URL("../lib/original-scene-sprite-meta.ts", import.meta.url),
  "utf8",
);
assert.match(
  spriteMetaSource,
  /frame-sets\.generated\.json/,
  "legacy per-library metadata must be augmented from the generated FrameSet catalog",
);
assert.match(
  shellSource,
  /addLayer\(sprite\.mount\)/,
  "the current mount frame must enter the packed Bevy atlas",
);
assert.match(
  renderingSource,
  /const ENTITY_ATLAS_PRELOAD_STATES:[\s\S]*"attackMelee"[\s\S]*"struck"[\s\S]*"dying"/,
  "transient combat frames must be resident before an attack packet arrives",
);
assert.match(
  renderingSource,
  /for \(const action of \["attack1", "attack2", "attack3", "attack4"\] as const\)/,
  "all Crystal melee variants must share the stable player atlas source set",
);
assert.match(
  renderingSource,
  /playerAnimationMetaForAction\(sprite, "spell"\)/,
  "Crystal Spell frames must already be resident before a cast packet arrives",
);
assert.match(
  renderingSource,
  /function atlasPreloadDirectionsForEntity\([^)]*\) \{[\s\S]*return ENTITY_ATLAS_PRELOAD_DIRECTIONS;/,
  "monster turns must not create a new entity atlas key",
);

for (const library of ["CArmour/00", "CArmour/01", "CHair/00", "CHair/01", "CWeapon/00", "CWeapon/01", "Mount/05"]) {
  const metaUrl = new URL(`../public/original-ui/${library}/meta.json`, import.meta.url);
  const meta = JSON.parse(readFileSync(metaUrl, "utf8"));
  const exported = new Set(meta.frames.map((frame) => Number(frame.index)));
  const required = library.startsWith("Mount/")
    ? [48, 55, 108, 113]
    : library.startsWith("CWeapon/")
      ? [92, 97, 148, 153, 366, 368, 392, 395, 508, 513]
      : [464, 471, 524, 529, 1272, 1279, 1332, 1337];
  for (const index of required) {
    assert.ok(exported.has(index), `${library} must export Crystal frame ${index}`);
  }
}

console.log("Crystal player frame/index coverage tests passed");
