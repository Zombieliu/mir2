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
});
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

const healthZeroBeforeDeath = { ...struck, hp: 0, dead: true };
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

console.log("actor combat state: Crystal struck/death/revive contracts passed");
