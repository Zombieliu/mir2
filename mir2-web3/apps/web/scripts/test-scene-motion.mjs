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

const sceneMotionPath = new URL("../app/components/original-client-scene-motion.ts", import.meta.url);
const source = readFileSync(sceneMotionPath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(sceneMotionPath),
});

const module = { exports: {} };
const load = new Function("exports", "module", "require", compiled.outputText);
load(module.exports, module, (specifier) => {
  if (specifier === "./original-client-scene-layout") {
    return {
      CRYSTAL_MOVE_FRAME_COUNT: 6,
      CRYSTAL_MOVE_FRAME_INTERVAL_MS: 100,
      EMPTY_VIEWPORT_OFFSET: { x: 0, y: 0 },
      VIEWPORT_CELL_HEIGHT: 32,
      VIEWPORT_CELL_WIDTH: 48,
    };
  }
  throw new Error(`Unexpected test import: ${specifier}`);
});

const {
  cameraMotionOffsetForEntity,
  crystalSteppedMovementProgressRatio,
  entityMotionOffsetForEntity,
  rebaseViewportEntitiesToRenderPlayer,
  refreshEntityMotionSnapshots,
} = module.exports;

assert.equal(crystalSteppedMovementProgressRatio(0), 1 / 6);
assert.equal(crystalSteppedMovementProgressRatio(99), 1 / 6);
assert.equal(crystalSteppedMovementProgressRatio(100), 2 / 6);
assert.equal(crystalSteppedMovementProgressRatio(500), 1);
assert.equal(crystalSteppedMovementProgressRatio(600), 1);
assert.equal(crystalSteppedMovementProgressRatio(0, 8), 1 / 8);
assert.equal(crystalSteppedMovementProgressRatio(600, 8), 7 / 8);
assert.equal(crystalSteppedMovementProgressRatio(700, 8), 1);
assert.equal(crystalSteppedMovementProgressRatio(800, 8), 1);

const shellSource = readFileSync(
  new URL("../app/original-client-shell.tsx", import.meta.url),
  "utf8",
);
const pageSource = readFileSync(
  new URL("../app/page.tsx", import.meta.url),
  "utf8",
);
const renderingSource = readFileSync(
  new URL("../app/components/original-client-scene-rendering.tsx", import.meta.url),
  "utf8",
);
const cameraDriverSource = readFileSync(
  new URL("../app/components/original-client-scene-camera-motion-driver.ts", import.meta.url),
  "utf8",
);
assert.match(
  shellSource,
  /setSceneSpriteFrameIndex\(\(current\) => current \+ 1\);\s*\}, 100\);/,
  "scene sprite invalidation must use Crystal's 100 ms movement cadence",
);
assert.match(
  renderingSource,
  /const SCENE_SPRITE_FRAME_INTERVAL_MS = 100;/,
  "scene animation timing metadata must match the 100 ms invalidation cadence",
);
assert.match(
  cameraDriverSource,
  /onLocalSelfMotionChangeRef\.current\?\.\(motion\)/,
  "the committed Bevy pose must publish its local self-motion phase",
);
assert.match(
  shellSource,
  /setBevyLocalSelfMotion/,
  "the shell must retain the committed local self-motion phase",
);
assert.match(
  renderingSource,
  /Math\.min\(presentationMotion\.frameIndex, frameCount - 1\)/,
  "the self sprite must use Bevy's latched movement phase instead of an independent wall clock",
);
assert.match(
  shellSource,
  /packetAnimationState === "standing"[\s\S]*packetAnimationState === "walking"[\s\S]*packetAnimationState === "running"/,
  "Bevy movement presentation must not override attack, struck, or death actions",
);
assert.match(
  pageSource,
  /const latestWorld = worldRef\.current;[\s\S]{0,500}\bsetWorld\(latestWorld\);/,
  "the rAF-coalesced world must still commit to React",
);
assert.doesNotMatch(
  pageSource,
  /const latestWorld = worldRef\.current;\s*startTransition\(\(\) => setWorld\(latestWorld\)\);/,
  "short combat action windows must not be deferred as a React transition",
);
assert.match(
  pageSource,
  /function beginLocalPlayerMeleeAttack\(objectId: string\)[\s\S]*attackAnimation: animation,[\s\S]*attackStartedAt: now,[\s\S]*attackUntil: readyAt/,
  "the local player must begin the melee action without waiting for the server ObjectAttack echo",
);
assert.match(
  pageSource,
  /function attackTarget\(objectId: string\)[\s\S]{0,240}if \(!beginLocalPlayerMeleeAttack\(objectId\)\) \{\s*return false;\s*\}[\s\S]{0,240}send\(\{ type: "attack"/,
  "target attacks must start the Crystal-local action before sending an in-range command",
);

const entity = {
  objectId: "self",
  kind: "player",
  dead: false,
  x: 11,
  y: 20,
};
const snapshots = {
  self: {
    fromX: 10,
    fromY: 20,
    toX: 11,
    toY: 20,
    animationState: "walking",
    startedAt: 1_000,
    expiresAt: 1_600,
  },
};

assert.deepEqual(entityMotionOffsetForEntity(entity, snapshots, 1_099), { x: -40, y: 0 });
assert.deepEqual(cameraMotionOffsetForEntity(entity, snapshots, 1_099), { x: 40, y: 0 });
assert.deepEqual(entityMotionOffsetForEntity(entity, snapshots, 1_100), { x: -32, y: 0 });
assert.deepEqual(cameraMotionOffsetForEntity(entity, snapshots, 1_100), { x: 32, y: 0 });
assert.deepEqual(entityMotionOffsetForEntity(entity, snapshots, 1_500), { x: 0, y: 0 });
assert.deepEqual(cameraMotionOffsetForEntity(entity, snapshots, 1_500), { x: 0, y: 0 });
assert.deepEqual(entityMotionOffsetForEntity(entity, snapshots, 1_600), { x: 0, y: 0 });
assert.deepEqual(cameraMotionOffsetForEntity(entity, snapshots, 1_600), { x: 0, y: 0 });

const rebasedViewport = rebaseViewportEntitiesToRenderPlayer(
  [
    { ...entity, dx: 0, dy: 0 },
    { objectId: "remote", kind: "player", name: "Remote", x: 13, y: 21, dx: 2, dy: 1 },
  ],
  {
    ...entity,
    x: 10,
    direction: "Left",
    movementAnimation: "walking",
    movementStartedAt: 2_000,
    movementUntil: 2_600,
  },
);
assert.deepEqual(
  rebasedViewport.map(({ objectId, x, y, dx, dy }) => ({ objectId, x, y, dx, dy })),
  [
    { objectId: "self", x: 10, y: 20, dx: 0, dy: 0 },
    { objectId: "remote", x: 13, y: 21, dx: 3, dy: 1 },
  ],
  "all entity layers must share the live map/player center",
);
assert.equal(rebasedViewport[0].movementStartedAt, 2_000);
assert.equal(rebasedViewport[0].direction, "Left");

const commandTimedEntity = {
  ...entity,
  direction: "Right",
  movementAnimation: "walking",
  movementStartedAt: 2_000,
  movementUntil: 2_600,
};
const delayedCommandSnapshot = refreshEntityMotionSnapshots(
  "game",
  [commandTimedEntity],
  commandTimedEntity,
  {
    self: {
      fromX: 10,
      fromY: 20,
      toX: 10,
      toY: 20,
      animationState: "standing",
      startedAt: 1_900,
      expiresAt: 0,
    },
  },
  2_167,
).self;
assert.equal(delayedCommandSnapshot.startedAt, 2_000, "a delayed shell render must preserve command sentAt");
assert.equal(delayedCommandSnapshot.expiresAt, 2_600, "the command window must not restart on render");
assert.deepEqual(
  entityMotionOffsetForEntity(commandTimedEntity, { self: delayedCommandSnapshot }, 2_167),
  { x: -32, y: 0 },
  "a delayed render must join the existing Crystal phase instead of replaying frame zero",
);

const activeLeftSnapshot = {
  self: {
    fromX: 330,
    fromY: 275,
    toX: 329,
    toY: 275,
    animationState: "walking",
    startedAt: 1_000,
    expiresAt: 1_600,
  },
};
const staleSourceEcho = {
  objectId: "self",
  kind: "selfPlayer",
  dead: false,
  x: 330,
  y: 275,
  direction: "Left",
  movementAnimation: "walking",
  movementStartedAt: 1_000,
  movementUntil: 1_600,
};
assert.deepEqual(
  refreshEntityMotionSnapshots(
    "game",
    [staleSourceEcho],
    staleSourceEcho,
    activeLeftSnapshot,
    1_200,
  ).self,
  activeLeftSnapshot.self,
  "same-direction source echo must not create a reverse self-camera window",
);
const intentionalReverse = { ...staleSourceEcho, direction: "Right" };
const reverseSnapshot = refreshEntityMotionSnapshots(
  "game",
  [intentionalReverse],
  intentionalReverse,
  activeLeftSnapshot,
  1_200,
).self;
assert.equal(reverseSnapshot.toX, 330, "opposite-direction reversal must remain valid");
assert.notDeepEqual(reverseSnapshot, activeLeftSnapshot.self);

console.log("scene motion tests passed");
