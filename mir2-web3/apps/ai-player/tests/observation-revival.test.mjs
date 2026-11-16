import assert from "node:assert/strict";
import test from "node:test";
import { ObservationReducer } from "../src/observation.mjs";

const packet = (name, payload = {}) => ({ type: "packet", packet: name, payload });
const world = (changes = {}) => ({ type: "worldSnapshot", payload: { mapFileName: "0", mapIndex: 0, playerObjectId: 7, selfZoneObjectId: 700,
  playerHp: 18, playerMaxHp: 18, entities: [{ objectId: 7, kind: "selfPlayer", name: "Alo", x: 288, y: 630, hp: 18, dead: false },
    { objectId: 19, kind: "monster", name: "Scarecrow", x: 288, y: 632, hp: 20, maxHp: 20, dead: false }],
  inventoryItems: [], beltItems: [], equipmentItems: [], questLog: [], groundDrops: [], ...changes } });

function reducer() {
  const observed = new ObservationReducer(); observed.apply(packet("StartGame", { result: 4 }), 1000); observed.apply(world(), 1001); return observed;
}
const monster = observed => observed.snapshot(2000).entities.find(entity => entity.objectId === 19);
const livingForMovement = entity => entity && entity.dead !== true && (entity.kind === "npc" || entity.hp == null || entity.hp > 0);

test("explicit monster revival discards corpse HP through spawn, walk and attack until an authoritative live HP snapshot", () => {
  const observed = reducer();
  observed.apply(packet("ObjectDied", { objectId: 19, location: { x: 288, y: 632 }, direction: "Down" }), 1100);
  const corpse = world(); corpse.payload.entities[1].hp = 0; corpse.payload.entities[1].dead = true; observed.apply(corpse, 1200);
  assert.equal(monster(observed).dead, true); assert.equal(monster(observed).hp, 0); assert.equal(livingForMovement(monster(observed)), false);
  observed.apply(packet("ObjectRevived", { objectId: 19, effect: true }), 1300);
  assert.equal(monster(observed).dead, false); assert.equal(monster(observed).hp, null); assert.equal(livingForMovement(monster(observed)), true);
  observed.apply(packet("ObjectMonster", { objectId: 19, name: "Scarecrow", location: { x: 288, y: 632 }, direction: "Up", dead: false, ai: 0 }), 1400);
  assert.equal(monster(observed).hp, null);
  observed.apply(packet("ObjectWalk", { objectId: 19, location: { x: 288, y: 631 }, direction: "Up" }), 1500);
  observed.apply(packet("ObjectAttack", { objectId: 19, location: { x: 288, y: 631 }, direction: "Up", spell: 0 }), 1600);
  const attacking = monster(observed); assert.equal(attacking.y, 631); assert.equal(attacking.dead, false); assert.equal(attacking.hp, null); assert.equal(attacking.lastAction.kind, "attack");
  const live = world(); live.payload.entities[1].hp = 12; live.payload.entities[1].y = 631; observed.apply(live, 1700);
  assert.equal(monster(observed).hp, 12); assert.equal(monster(observed).dead, false);
});

test("walking, attacking, percentages and incomplete metadata never revive a corpse or invent positive HP", () => {
  const observed = reducer(), corpse = world(); corpse.payload.entities[1].hp = 0; corpse.payload.entities[1].dead = true; observed.apply(corpse, 1100);
  for (const event of [packet("ObjectWalk", { objectId: 19, location: { x: 288, y: 631 }, direction: "Up" }),
    packet("ObjectAttack", { objectId: 19, location: { x: 288, y: 631 }, direction: "Up", spell: 0 }),
    packet("ObjectHealth", { objectId: 19, percent: 100 }),
    packet("ObjectMonster", { objectId: 19, name: "Scarecrow", location: { x: 288, y: 631 }, direction: "Up" })]) {
    observed.apply(event, 1200); const value = monster(observed);
    assert.equal(value.dead, true); assert.equal(value.hp, 0); assert.equal(value.lastAction, undefined); assert.equal(livingForMovement(value), false);
  }
});

test("unknown IDs and removed monsters cannot be created by a revival signal", () => {
  const observed = reducer(), before = observed.snapshot(1100).entities.length;
  observed.apply(packet("ObjectRevived", { objectId: 999, effect: true }), 1200);
  assert.equal(observed.snapshot(1200).entities.length, before); assert.equal(observed.snapshot(1200).entities.some(entity => entity.objectId === 999), false);
  observed.apply(packet("ObjectRemove", { objectId: 19 }), 1300);
  observed.apply(packet("ObjectRevived", { objectId: 19, effect: true }), 1400);
  assert.equal(observed.snapshot(1400).entities.some(entity => entity.objectId === 19), false);
});

test("a monster-only HP invalidation preserves existing NPC and remote-player revival semantics", () => {
  const observed = reducer(), initial = world();
  initial.payload.entities.push({ objectId: 20, kind: "player", name: "Visitor", x: 289, y: 631, hp: 0, dead: true },
    { objectId: 21, kind: "npc", name: "Merchant", x: 290, y: 631, hp: 0, dead: true }); observed.apply(initial, 1100);
  for (const objectId of [20, 21]) {
    observed.apply(packet("ObjectRevived", { objectId, effect: true }), 1200);
    const value = observed.snapshot(1200).entities.find(entity => entity.objectId === objectId);
    assert.equal(value.hp, 0); assert.equal(value.dead, false);
  }
});

test("local self and its trusted Zone alias retain the self HP fence until actual health is observed", () => {
  for (const id of [7, 700]) {
    const observed = reducer(); observed.apply(packet("ObjectDied", { objectId: id }), 1100);
    assert.equal(observed.snapshot(1100).playerHp, 0);
    observed.apply(packet("ObjectRevived", { objectId: id, effect: true }), 1200);
    const self = observed.snapshot(1200); assert.equal(self.player.dead, false); assert.equal(self.player.hp, 0); assert.equal(self.playerHp, 0);
    assert.equal(self.entities.some(entity => entity.objectId === 700), false);
    const restored = observed.apply(packet("HealthChanged", { hp: 9, mp: 10 }), 1300);
    assert.equal(restored.playerHp, 9); assert.equal(restored.player.hp, 9);
  }
});

test("an explicit new monster life never retains a positive HP estimate from the former incarnation", () => {
  const observed = reducer(); observed.apply(packet("ObjectDied", { objectId: 19 }), 1100);
  assert.equal(monster(observed).hp, 20);
  observed.apply(packet("ObjectRevived", { objectId: 19, effect: false }), 1200);
  assert.equal(monster(observed).hp, null); assert.equal(monster(observed).dead, false);
});
