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

const renderingSource = readFileSync(
  new URL("../app/components/original-client-scene-rendering.tsx", import.meta.url),
  "utf8",
);
const shellSource = readFileSync(
  new URL("../app/original-client-shell.tsx", import.meta.url),
  "utf8",
);
assert.match(
  renderingSource,
  /frameLayersForIndices\(libraries\[mountLibraryKey\], mountFrameIndices, fallbackMountFrameIndex\)/,
  "mounted phases must be included in the preload frame set",
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
