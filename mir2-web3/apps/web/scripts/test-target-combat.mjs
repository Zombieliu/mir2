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

const combatPath = new URL("../app/components/original-client-target-combat.ts", import.meta.url);
const pagePath = new URL("../app/page.tsx", import.meta.url);
const source = readFileSync(combatPath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(combatPath),
});

const module = { exports: {} };
const load = new Function("exports", "module", compiled.outputText);
load(module.exports, module);

const {
  LOCKED_MONSTER_REPATH_MIN_MS,
  createLockedMonsterAttack,
  decideLockedMonsterAttack,
} = module.exports;

const target = { objectId: "50001", kind: "monster", dead: false, x: 14, y: 10 };
const self = { x: 10, y: 10 };
const baseLock = createLockedMonsterAttack(target.objectId, target);

const approach = decideLockedMonsterAttack({
  lock: baseLock,
  selectedObjectId: target.objectId,
  self,
  target,
  approachDestination: { x: 13, y: 10 },
  queuedApproach: null,
  movementPending: false,
  nextAttackAt: 0,
  now: 1_000,
});
assert.equal(approach.kind, "approach");
assert.deepEqual(approach.destination, { x: 13, y: 10 });
assert.equal(approach.lock.nextApproachAt, 1_000 + LOCKED_MONSTER_REPATH_MIN_MS);

const queuedWait = decideLockedMonsterAttack({
  lock: approach.lock,
  selectedObjectId: target.objectId,
  self,
  target,
  approachDestination: { x: 13, y: 10 },
  queuedApproach: { x: 13, y: 10 },
  movementPending: false,
  nextAttackAt: 0,
  now: 1_050,
});
assert.equal(queuedWait.kind, "wait", "an active route must not be requeued every combat tick");

const movedTarget = { ...target, x: 15 };
const movingRepath = decideLockedMonsterAttack({
  lock: approach.lock,
  selectedObjectId: target.objectId,
  self,
  target: movedTarget,
  approachDestination: { x: 14, y: 10 },
  queuedApproach: { x: 13, y: 10 },
  movementPending: true,
  nextAttackAt: 0,
  now: 1_060,
});
assert.equal(movingRepath.kind, "approach", "a moving monster must replace the stale chase destination");
assert.deepEqual(movingRepath.destination, { x: 14, y: 10 });

const adjacentTarget = { ...target, x: 11 };
const adjacentMovingWait = decideLockedMonsterAttack({
  lock: baseLock,
  selectedObjectId: target.objectId,
  self,
  target: adjacentTarget,
  approachDestination: self,
  queuedApproach: null,
  movementPending: true,
  nextAttackAt: 0,
  now: 2_000,
});
assert.equal(adjacentMovingWait.kind, "wait", "attack must wait for the accepted movement action to settle");

const adjacentAttack = decideLockedMonsterAttack({
  lock: baseLock,
  selectedObjectId: target.objectId,
  self,
  target: adjacentTarget,
  approachDestination: self,
  queuedApproach: null,
  movementPending: false,
  nextAttackAt: 1_900,
  now: 2_000,
});
assert.equal(adjacentAttack.kind, "attack");

const cooldownWait = decideLockedMonsterAttack({
  lock: baseLock,
  selectedObjectId: target.objectId,
  self,
  target: adjacentTarget,
  approachDestination: self,
  queuedApproach: null,
  movementPending: false,
  nextAttackAt: 2_500,
  now: 2_000,
});
assert.equal(cooldownWait.kind, "wait", "auto-hit must respect the local Crystal attack cadence");

const postSwingChaseWait = decideLockedMonsterAttack({
  lock: baseLock,
  selectedObjectId: target.objectId,
  self,
  target,
  approachDestination: { x: 13, y: 10 },
  queuedApproach: null,
  movementPending: false,
  nextAttackAt: 2_500,
  now: 2_000,
});
assert.equal(postSwingChaseWait.kind, "wait", "a fleeing target must not cancel the active attack animation");

for (const invalid of [
  { selectedObjectId: null, target: adjacentTarget },
  { selectedObjectId: target.objectId, target: null },
  { selectedObjectId: target.objectId, target: { ...adjacentTarget, dead: true } },
]) {
  const decision = decideLockedMonsterAttack({
    lock: baseLock,
    self,
    approachDestination: self,
    queuedApproach: null,
    movementPending: false,
    nextAttackAt: 0,
    now: 2_000,
    ...invalid,
  });
  assert.equal(decision.kind, "clear");
}

const pageSource = readFileSync(pagePath, "utf8");
assert.match(
  pageSource,
  /if \(!beginLocalPlayerMeleeAttack\(objectId\)\) \{\s*return false;/,
  "out-of-range attacks must not reach the websocket",
);
assert.match(
  pageSource,
  /function activateEntity\(objectId: string\)[\s\S]*?if \(entity\.kind === "monster"\) \{\s*lockMonsterAttack\(objectId\);/,
  "clicking a live monster must enter the lock-and-chase pipeline",
);
assert.match(
  pageSource,
  /function moveToTile\([\s\S]*?source: "manual" \| "locked-monster"[\s\S]*?cancelLockedMonsterAttack\(\);/,
  "manual movement must cancel the active monster chase",
);
assert.match(
  pageSource,
  /function lockMonsterAttack\(objectId: string\)[\s\S]*?cancelLockedMonsterAttack\(\);[\s\S]*?createLockedMonsterAttack\(objectId, target\)/,
  "switching monster targets must discard the previous queued chase route",
);

console.log("target combat tests passed");
