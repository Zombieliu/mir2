import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTsModule(url) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const mod = { exports: {} };
  new Function("exports", "module", "require", compiled.outputText)(
    mod.exports,
    mod,
    (specifier) => {
      throw new Error(`Unexpected dependency: ${specifier}`);
    },
  );
  return mod.exports;
}

const combat = loadTsModule(
  new URL("../lib/world-model/actor-combat-state.ts", import.meta.url),
);

const actor = {
  objectId: "player-7",
  kind: "player",
  x: 288,
  y: 616,
  direction: "Down",
  hp: 60,
  dead: false,
  attackAnimation: "melee1",
  attackStartedAt: 900,
  attackUntil: 1_500,
};

assert.equal(combat.CRYSTAL_PLAYER_STRUCK_DURATION_MS, 300);
assert.equal(combat.CRYSTAL_PLAYER_DIE_DURATION_MS, 400);
assert.equal(combat.CRYSTAL_PLAYER_REVIVE_DURATION_MS, 400);
assert.equal(combat.CRYSTAL_PLAYER_REVIVE_EFFECT_DURATION_MS, 2_000);

const struck = combat.applyActorStruck(actor, 1_000, combat.CRYSTAL_PLAYER_STRUCK_DURATION_MS, {
  x: 289,
  direction: "Right",
}, "monster-1");
assert.equal(struck.x, 289);
assert.equal(struck.y, 616);
assert.equal(struck.direction, "Right");
assert.equal(struck.struckStartedAt, 1_000);
assert.equal(struck.struckUntil, 1_300);
assert.equal(
  combat.actorStruckIsAlreadyPending(struck, 1_299),
  false,
  "a second real hit may queue while the current Struck action is playing",
);
assert.equal(combat.actorStruckIsAlreadyPending(struck, 1_300), false);

const queuedStruck = combat.applyActorStruck(
  struck,
  1_100,
  combat.CRYSTAL_PLAYER_STRUCK_DURATION_MS,
  { x: 290, y: 615, direction: "Up" },
  "monster-2",
);
assert.equal(queuedStruck.struckStartedAt, 1_000, "queueing cannot restart the current action");
assert.equal(queuedStruck.struckUntil, 1_300, "queueing cannot extend the current action");
assert.equal(queuedStruck.x, 289, "queued packet location is applied when its action starts");
assert.deepEqual(queuedStruck.pendingStruck, {
  x: 290,
  y: 615,
  direction: "Up",
  attackerId: "monster-2",
  durationMs: 300,
});
assert.equal(combat.actorStruckIsAlreadyPending(queuedStruck, 1_101), true);
assert.equal(
  combat.applyActorStruck(queuedStruck, 1_200, 300, { x: 999 }, "monster-3"),
  queuedStruck,
  "a third Struck is dropped while the ActionFeed tail already contains Struck",
);
assert.equal(combat.advanceActorStruck(queuedStruck, 1_299), queuedStruck);

const advancedStruck = combat.advanceActorStruck(queuedStruck, 1_300);
assert.equal(advancedStruck.x, 290);
assert.equal(advancedStruck.y, 615);
assert.equal(advancedStruck.direction, "Up");
assert.equal(advancedStruck.struckStartedAt, 1_300);
assert.equal(advancedStruck.struckUntil, 1_600);
assert.equal(advancedStruck.pendingStruck, undefined);

const delayedAdvance = combat.advanceActorStruck(queuedStruck, 1_350);
assert.equal(delayedAdvance.struckStartedAt, 1_350, "a suspended renderer starts queued work on resume");
assert.equal(delayedAdvance.struckUntil, 1_650);

const thirdQueuedStruck = combat.applyActorStruck(
  advancedStruck,
  1_350,
  300,
  { direction: "Left" },
  "monster-3",
);
const thirdAdvancedStruck = combat.advanceActorStruck(thirdQueuedStruck, 1_600);
assert.equal(thirdAdvancedStruck.struckStartedAt, 1_600);
assert.equal(thirdAdvancedStruck.struckUntil, 1_900);
assert.equal(thirdAdvancedStruck.direction, "Left");
assert.equal(thirdAdvancedStruck.pendingStruck, undefined);

const mapChangedActor = combat.clearActorActionFeed({
  ...thirdQueuedStruck,
  dieStartedAt: 1_400,
  dieUntil: 1_800,
  deathHandled: true,
  reviveStartedAt: 1_500,
  reviveUntil: 1_900,
});
assert.equal(mapChangedActor.attackAnimation, undefined);
assert.equal(mapChangedActor.attackUntil, undefined);
assert.equal(mapChangedActor.struckUntil, undefined);
assert.equal(mapChangedActor.pendingStruck, undefined, "MapChanged clears the ActionFeed tail");
assert.equal(mapChangedActor.dieUntil, undefined);
assert.equal(mapChangedActor.reviveUntil, undefined);
assert.equal(mapChangedActor.deathHandled, false);

const healthZeroBeforeDeath = { ...queuedStruck, hp: 0, dead: true };
const deathAfterHealthZero = combat.applyActorDeath(
  healthZeroBeforeDeath,
  2_000,
  combat.CRYSTAL_PLAYER_DIE_DURATION_MS,
  { y: 617 },
);
assert.equal(deathAfterHealthZero.dieStartedAt, 2_000);
assert.equal(deathAfterHealthZero.y, 617);

const healthOnlyZero = combat.applyActorHealth(struck, 0, 60);
assert.equal(healthOnlyZero.hp, 0);
assert.equal(healthOnlyZero.dead, false, "ObjectHealth cannot start the death lifecycle");

const dying = deathAfterHealthZero;
assert.equal(dying.dead, true);
assert.equal(dying.hp, 0);
assert.equal(dying.y, 617);
assert.equal(dying.dieStartedAt, 2_000);
assert.equal(dying.dieUntil, 2_400);
assert.equal(dying.deathHandled, true);
assert.equal(dying.attackAnimation, undefined);
assert.equal(dying.struckUntil, undefined);
assert.equal(dying.pendingStruck, undefined, "death clears queued Struck actions");
assert.equal(
  combat.applyActorDeath(dying, 2_100, combat.CRYSTAL_PLAYER_DIE_DURATION_MS),
  dying,
  "replayed death preserves the original corpse clock by reference",
);

const remoteRevived = combat.applyActorRevive(
  dying,
  3_000,
  combat.CRYSTAL_PLAYER_REVIVE_DURATION_MS,
  "animated",
);
assert.equal(remoteRevived.dead, false);
assert.equal(remoteRevived.reviveStartedAt, 3_000);
assert.equal(remoteRevived.reviveUntil, 3_400);
assert.equal(remoteRevived.dieUntil, undefined);
assert.equal(remoteRevived.deathHandled, false);
assert.equal(remoteRevived.hp, 0, "ObjectRevived cannot invent an authoritative HP value");
assert.equal(remoteRevived.pendingStruck, undefined, "revive cannot retain stale queued Struck actions");

const selfRevived = combat.applyActorRevive(
  dying,
  3_000,
  combat.CRYSTAL_PLAYER_REVIVE_DURATION_MS,
  "standing",
);
assert.equal(selfRevived.dead, false);
assert.equal(selfRevived.reviveStartedAt, undefined);
assert.equal(selfRevived.reviveUntil, undefined);
assert.equal(selfRevived.hp, 0, "Revived cannot invent an authoritative HP value");

const effect = combat.createPlayerReviveSceneEffect(remoteRevived, 3_000);
assert.deepEqual(effect, {
  key: "crystal-player-revive:player-7",
  source: "actorEffect",
  spellOrEffect: "PlayerRevive",
  objectId: "player-7",
  x: 289,
  y: 617,
  direction: 0,
  value: 0,
  startedAt: 3_000,
  expiresAt: 5_000,
});

const pageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
assert.match(
  pageSource,
  /function markWorldEntityDead[\s\S]*?currentEntity\.deathHandled === true/,
  "ObjectDied must not be discarded merely because a preceding ObjectHealth carried zero",
);
assert.match(
  pageSource,
  /function mergeSnapshotEntityIntoPacketRuntime[\s\S]*?deathHandled: currentEntity\.deathHandled/,
  "snapshot refreshes must preserve the consumed death incarnation marker",
);
assert.match(
  pageSource,
  /function applyObjectHealthPacket[\s\S]*?return applyActorHealth\(entity, nextHp, nextMaxHp\);/,
  "ObjectHealth must remain a numeric health update",
);
const reviveHandlers = pageSource.slice(
  pageSource.indexOf("function markSelfPlayerRevived"),
  pageSource.indexOf("function applyMagicDelayPacket"),
);
assert.doesNotMatch(
  reviveHandlers,
  /playerMaxHp|maxHp\) \? Math\.max\(1/,
  "Revived/ObjectRevived must wait for authoritative health instead of fabricating max HP",
);
assert.match(
  pageSource,
  /function advanceQueuedActorStruckActions[\s\S]*?advanceActorStruck\(entity, now\)[\s\S]*?emitInlineEntityStruck/,
  "the renderer must consume a queued Struck action before emitting its deferred sound event",
);
assert.match(
  pageSource,
  /function markWorldEntityStruck[\s\S]*?const queued = actorStruckIsActive[\s\S]*?if \(!queued\) \{[\s\S]*?emitInlineEntityStruck/,
  "remote queued Struck audio must not play until the queued action starts",
);
assert.match(
  pageSource,
  /function markPlayerStruck[\s\S]*?queued = actorStruckIsActive[\s\S]*?if \(accepted && !queued && playerObjectId\)/,
  "self queued Struck audio must not play until the queued action starts",
);
assert.match(
  pageSource,
  /const tickMovementPlan = \(\) => \{[\s\S]*?advanceQueuedActorStruckActions\(tickNow\)/,
  "the live render clock must advance the queued Struck action",
);
assert.match(
  pageSource,
  /case "MapChanged"[\s\S]*?clearActorActionFeed\(preservedSelfEntity\)/,
  "MapChanged must clear the preserved self actor's Crystal ActionFeed",
);

console.log("actor combat state: Crystal ActionFeed struck/death/revive contracts passed");
