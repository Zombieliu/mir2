import test from "node:test";
import assert from "node:assert/strict";
import { ObservationReducer } from "../src/observation.mjs";

const packet = (name, payload = {}) => ({ type: "packet", packet: name, payload });
const world = (changes = {}) => ({
  type: "worldSnapshot", payload: {
    tick: 20, mapFileName: "0", mapTitle: "Bichon", playerObjectId: 7, playerHp: 15, playerMaxHp: 20,
    playerMp: 10, playerMaxMp: 10, playerExperience: 0, playerMaxExperience: 100, gold: 100,
    entities: [{ objectId: 7, kind: "selfPlayer", name: "Resident", x: 284, y: 606, direction: "Down", hp: 15, dead: false }, { objectId: 8, kind: "player", name: "Visitor", x: 285, y: 606 }],
    inventoryItems: [{ uniqueId: 9007199254740993n, key: "hp-drug", name: "HP drug", slot: 1, container: "bag1", quantity: 2 }],
    beltItems: [], equipmentItems: [], questLog: [], groundDrops: [], activeNpcDialog: null,
    ...changes,
  },
});
const readyReducer = () => {
  const reducer = new ObservationReducer();
  reducer.apply(packet("StartGame", { result: 4 }));
  reducer.apply(world());
  return reducer;
};

test("readiness requires successful StartGame and matching authoritative self snapshot", () => {
  const reducer = new ObservationReducer();
  assert.equal(reducer.snapshot().ready, false);
  assert.equal(reducer.apply(world()).ready, false);
  assert.equal(reducer.apply(packet("StartGame", { result: 1 })).ready, false);
  assert.equal(reducer.apply(world()).ready, false);
  assert.equal(reducer.apply(packet("StartGame", { result: 4 })).ready, false);
  assert.equal(reducer.apply(world({ playerObjectId: 999 })).ready, false);
  assert.equal(reducer.apply(world({ entities: [{ objectId: 7, kind: "selfPlayer", name: "Resident" }] })).ready, false);
  assert.equal(reducer.apply(world()).ready, true);
});

test("snapshot projection preserves uid precision and strips non-observable/private trees", () => {
  const reducer = readyReducer();
  const result = reducer.apply(world({
    stage5Systems: { private: "secret" }, npcScriptDiagnostics: [{ secret: true }], otherAccounts: ["secret"],
    groundDrops: [{ objectId: 30, name: "Meat", x: 2, y: 3, quantity: 1, ownerObjectId: 7, loot: { kind: "inventoryItem", exactItem: { secret: true } } }],
    inventoryItems: [{ uniqueId: 18446744073709551615n, name: "Item", gmDiagnostic: "secret" }],
  }));
  assert.equal(result.inventoryItems[0].uniqueId, 18446744073709551615n);
  assert.equal(result.inventoryItems[0].gmDiagnostic, undefined);
  assert.equal(result.groundDrops[0].ownerObjectId, 7);
  assert.equal(result.groundDrops[0].loot, undefined);
  assert.equal(result.stage5Systems, undefined);
  assert.equal(result.npcScriptDiagnostics, undefined);
  assert.equal(result.otherAccounts, undefined);
  result.entities[0].name = "changed externally";
  assert.equal(reducer.snapshot().player.name, "Resident");
});

test("reset clears prior world and rejects an explicitly tagged old connection event", () => {
  const reducer = readyReducer();
  const before = reducer.snapshot();
  const cleared = reducer.resetConnection();
  assert.equal(cleared.connectionEpoch, before.connectionEpoch + 1);
  assert.ok(cleared.revision > before.revision);
  assert.equal(cleared.ready, false);
  assert.deepEqual(cleared.entities, []);
  assert.deepEqual(cleared.inventoryItems, []);
  assert.equal(cleared.player, null);
  reducer.apply({ ...world(), connectionEpoch: before.connectionEpoch });
  assert.deepEqual(reducer.snapshot(), cleared);
});

test("actual packet location formats update only currently visible objects", () => {
  const reducer = readyReducer();
  reducer.apply(packet("ObjectMonster", { objectId: 19, name: "Deer", location: { x: 280, y: 608 }, direction: "Up", ai: 9, internal: "secret" }));
  reducer.apply(packet("ObjectWalk", { objectId: 19, x: 281, y: 609, direction: "Right" }));
  reducer.apply(packet("UserLocation", { x: 285, y: 607, direction: "DownRight" }));
  const state = reducer.snapshot();
  assert.equal(state.player.x, 285);
  assert.equal(state.player.y, 607);
  assert.equal(state.entities.find(value => value.objectId === 19).x, 281);
  assert.equal(state.entities.find(value => value.objectId === 19).internal, undefined);
  reducer.apply(packet("ObjectRemove", { objectId: 19 }));
  reducer.apply(packet("ObjectRun", { objectId: 19, x: 300, y: 300 }));
  assert.equal(reducer.snapshot().entities.some(value => value.objectId === 19), false);
});

test("map boundaries, full snapshots and logout remove stale entity bindings", () => {
  const reducer = readyReducer();
  reducer.apply(world({ entities: [{ objectId: 7, kind: "selfPlayer", name: "Resident", x: 284, y: 606 }] }));
  assert.equal(reducer.snapshot().entities.length, 1);
  const changed = reducer.apply(packet("MapChanged", { mapIndex: 141, fileName: "0141", title: "Shelter", location: { x: 10, y: 10 } }));
  assert.equal(changed.ready, false);
  assert.deepEqual(changed.entities, []);
  assert.deepEqual(changed.groundDrops, []);
  assert.equal(changed.mapFileName, "0141");
  assert.equal(reducer.apply(world({ mapFileName: "0141" })).ready, true);
  const loggedOut = reducer.apply(packet("LogOutSuccess", { characters: [] }));
  assert.equal(loggedOut.ready, false);
  assert.equal(loggedOut.playerObjectId, null);
  assert.deepEqual(loggedOut.inventoryItems, []);
});

test("HP packets, death and revive never invent healing or turn percent into absolute HP", () => {
  const reducer = readyReducer();
  assert.equal(reducer.apply(packet("HealthChanged", { hp: 8, mp: 6, typed: true })).playerHp, 8);
  const percentage = reducer.apply(packet("ObjectHealth", { objectId: 7, percent: 50 }));
  assert.equal(percentage.playerHp, 8);
  assert.equal(percentage.player.healthPercent, 50);
  const dead = reducer.apply(packet("Death", { location: { x: 280, y: 610 }, direction: "Up" }));
  assert.equal(dead.playerHp, 0);
  assert.equal(dead.player.dead, true);
  const revived = reducer.apply(packet("Revived"));
  assert.equal(revived.player.dead, false);
  assert.equal(revived.playerHp, null);
});

test("gold is a delta, full snapshots reconcile it, incomplete item packets only invalidate inventory", () => {
  const reducer = readyReducer();
  assert.equal(reducer.apply(packet("GainedGold", { gold: 20 })).gold, 120);
  assert.equal(reducer.apply(packet("LoseGold", { gold: 40 })).gold, 80);
  const invalidated = reducer.apply(packet("GainedItem", { item: { unique_id: 18446744073709551615n, item_index: 658, count: 4 } }));
  assert.equal(invalidated.inventoryStale, true);
  assert.equal(invalidated.inventoryItems.length, 1);
  assert.equal(invalidated.inventoryItems[0].uniqueId, 9007199254740993n);
  assert.equal(reducer.apply(world({ gold: 80 })).inventoryStale, false);
});

test("quest ready-to-hand-in is distinct from completed and remove is respected", () => {
  const reducer = readyReducer();
  const changed = reducer.apply(packet("ChangeQuest", { questId: 2, questState: 0, taken: true, completed: true, current: 1, required: 1, objectives: [] }));
  assert.equal(changed.questLog[0].stage, "readyToTurnIn");
  assert.equal(reducer.apply(packet("CompleteQuest", { completedQuests: [2] })).questLog[0].stage, "completed");
  assert.deepEqual(reducer.apply(packet("ChangeQuest", { questId: 2, questState: 2 })).questLog, []);
});

test("visible gold and item drops are removed on AOI removal without inventing ownership", () => {
  const reducer = readyReducer();
  reducer.apply(packet("ObjectGold", { objectId: 20, gold: 51, location: { x: 280, y: 607 } }));
  reducer.apply(packet("ObjectItem", { objectId: 21, name: "Venison", image: 99, location: { x: 281, y: 607 } }));
  const state = reducer.snapshot();
  assert.equal(state.groundDrops[0].quantity, 51);
  assert.equal(state.groundDrops[1].icon, 99);
  assert.equal(state.groundDrops[1].ownerObjectId, undefined);
  reducer.apply(packet("ObjectRemove", { objectId: 20 }));
  assert.deepEqual(reducer.snapshot().groundDrops.map(value => value.objectId), [21]);
});

test("NPC service packets retain safe shop fields and bind to the subsequent dialog snapshot", () => {
  const reducer = readyReducer();
  reducer.apply(packet("NPCResponse", { page: ["<Buy/@BuySell>", "hello"] }));
  reducer.apply(packet("NPCGoods", { list: [{ id: 18446744073709551615n, uniqueId: 18446744073709551615n, itemIndex: 658, price: 40, count: 1, name: "HP", secret: "hidden" }], rate: 1, panelType: 0 }));
  const state = reducer.apply(world({ activeNpcDialog: { npcObjectId: 9, npcName: "Merchant", title: "Shop", body: ["Hello"], links: [{ text: "Buy", target: "@BuySell", adminData: true }] } }));
  assert.equal(state.npcGoods.list[0].uniqueId, 18446744073709551615n);
  assert.equal(state.npcGoods.list[0].secret, undefined);
  assert.deepEqual(state.activeNpcDialog.links, [{ text: "Buy", target: "@BuySell" }]);
  assert.equal(reducer.apply(world()).npcGoods, null);
});

test("only currently observed speakers can populate nearby chat and unknown/admin events are ignored", () => {
  const reducer = readyReducer();
  reducer.apply(packet("ObjectChat", { objectId: 999, text: "I am Visitor", chatType: "Normal" }));
  assert.equal(reducer.snapshot().lastChat, null);
  const spoken = reducer.apply(packet("ObjectChat", { objectId: 8, text: "Hi", chatType: "Normal", accountId: "secret" }));
  assert.equal(spoken.lastChat.name, "Visitor");
  assert.equal(spoken.lastChat.accountId, undefined);
  const before = reducer.snapshot();
  reducer.apply({ type: "adminSnapshot", payload: { entities: [{ objectId: 999 }] } });
  reducer.apply(packet("UnknownAdminPacket", { secret: true }));
  assert.deepEqual(reducer.snapshot(), before);
});

test("a group invitation stays pending until an authoritative roster includes this character", () => {
  const reducer = readyReducer();
  assert.deepEqual(reducer.snapshot().party, { allowGroup: false, members: [], leaderName: null, pendingInvite: null, memberInfo: [] });
  reducer.apply(packet("SwitchGroup", { allowGroup: true }), 1000);
  const invited = reducer.apply(packet("GroupInvite", { name: "Visitor" }), 1100);
  assert.deepEqual(invited.party.pendingInvite, { name: "Visitor", observedAt: 1100 });
  assert.deepEqual(invited.party.members, []);
  const unjoined = reducer.apply(world({ stage5Systems: { group: { allowGroup: true, members: [], lootMode: "free" }, privateAdminData: "not copied" } }), 1200);
  assert.deepEqual(unjoined.party.members, []);
  assert.equal(unjoined.party.pendingInvite.name, "Visitor");
  assert.equal(unjoined.stage5Systems, undefined);
  assert.equal(unjoined.party.lootMode, undefined);
  reducer.apply(packet("AddMember", { name: "Visitor" }), 1300);
  assert.equal(reducer.snapshot(1300).party.pendingInvite.name, "Visitor");
  const joined = reducer.apply(packet("AddMember", { name: "Resident" }), 1400);
  assert.deepEqual(joined.party.members, ["Visitor", "Resident"]);
  assert.equal(joined.party.leaderName, "Visitor");
  assert.equal(joined.party.pendingInvite, null);
  assert.deepEqual(reducer.apply(packet("AddMember", { name: "Resident" }), 1500).party.members, ["Visitor", "Resident"]);
});

test("actual enriched group roster is safe, survives ordinary snapshots, and handles removal", () => {
  const reducer = readyReducer();
  reducer.apply(packet("SwitchGroup", { allowGroup: true }), 1000);
  const roster = reducer.apply(packet("GroupMemberInfo", {
    leaderName: "Visitor",
    members: [{ name: "Visitor", level: 12, class: 0, hp: 90, maxHp: 100, online: true, accountId: "private" }, { name: "Resident", level: 1, online: true }],
  }), 1100);
  assert.deepEqual(roster.party.members, ["Visitor", "Resident"]);
  assert.equal(roster.party.leaderName, "Visitor");
  assert.equal(roster.party.memberInfo[0].hp, 90);
  assert.equal(roster.party.memberInfo[0].accountId, undefined);
  assert.deepEqual(reducer.apply(world(), 1200).party, roster.party);
  const pruned = reducer.apply(world({ stage5Systems: { group: { allowGroup: true, members: ["Resident"] } } }), 1300);
  assert.deepEqual(pruned.party.members, ["Resident"]);
  assert.deepEqual(pruned.party.memberInfo.map(value => value.name), ["Resident"]);
  assert.equal(pruned.party.leaderName, "Resident");
  reducer.apply(packet("AddMember", { name: "Visitor" }), 1400);
  assert.deepEqual(reducer.apply(packet("DeleteMember", { name: "Visitor" }), 1500).party.members, ["Resident"]);
  const deleted = reducer.apply(packet("DeleteGroup"), 1600);
  assert.deepEqual(deleted.party.members, []);
  assert.equal(deleted.party.allowGroup, true);
  assert.equal(deleted.party.leaderName, null);
  assert.equal(deleted.party.pendingInvite, null);
  reducer.apply(packet("GroupInvite", { name: "Visitor" }), 1700);
  assert.deepEqual(reducer.resetConnection().party, { allowGroup: false, members: [], leaderName: null, pendingInvite: null, memberInfo: [] });
});

function visibleCombatReducer() {
  const reducer = readyReducer();
  reducer.apply(packet("ObjectMonster", { objectId: 19, name: "Scarecrow", location: { x: 286, y: 606 }, dead: false }), 1000);
  return reducer;
}

test("ObjectStruck records only two actually visible actors and does not map an unknown Zone id to self", () => {
  const reducer = visibleCombatReducer();
  const hit = reducer.apply(packet("ObjectStruck", { attackerId: 8, objectId: 19, location: { x: 286, y: 606 }, direction: "Down" }), 1100);
  assert.deepEqual(hit.engagements, [{ attackerId: 8, targetId: 19, observedAt: 1100 }]);
  const repeated = reducer.apply(packet("ObjectStruck", { attackerId: 8, objectId: 19 }), 1200);
  assert.deepEqual(repeated.engagements, [{ attackerId: 8, targetId: 19, observedAt: 1200 }]);
  reducer.apply(packet("ObjectStruck", { attackerId: 700, objectId: 19 }), 1300);
  reducer.apply(packet("ObjectStruck", { attackerId: 8, objectId: 999 }), 1400);
  reducer.apply(packet("ObjectStruck", { attackerId: 8, objectId: 8 }), 1500);
  reducer.apply(packet("Struck", { attackerId: 19 }), 1600);
  assert.deepEqual(reducer.snapshot(1600).engagements, [{ attackerId: 8, targetId: 19, observedAt: 1200 }]);
  assert.equal(reducer.snapshot(1600).lastDamageAt, null);
});

test("attack animation records pose without inventing a target, damage, or assistance", () => {
  const reducer = visibleCombatReducer();
  const attack = reducer.apply(packet("ObjectAttack", { objectId: 8, location: { x: 285, y: 606 }, direction: "Right", spell: 0, level: 1, attackType: 0, targetId: 19 }), 1100);
  assert.deepEqual(attack.engagements, []);
  assert.deepEqual(attack.entities.find(value => value.objectId === 8).lastAction, { kind: "attack", observedAt: 1100, direction: "Right", spell: 0, level: 1, attackType: 0 });
  assert.equal(attack.lastDamageAt, null);
  const missing = reducer.apply(packet("ObjectAttack", { objectId: 700, targetId: 19 }), 1200);
  assert.equal(missing.entities.some(value => value.objectId === 700), false);
  assert.deepEqual(missing.engagements, []);
});

test("same-map snapshots retain recent combat but revalidate AOI, and snapshot reads expire it at six seconds", () => {
  const reducer = visibleCombatReducer();
  reducer.apply(packet("ObjectAttack", { objectId: 8, direction: "Right", spell: 0 }), 1000);
  const hit = reducer.apply(packet("ObjectStruck", { attackerId: 8, objectId: 19 }), 1100);
  const refreshed = reducer.apply(world({ entities: hit.entities }), 1500);
  assert.equal(refreshed.engagements.length, 1);
  assert.equal(refreshed.entities.find(value => value.objectId === 8).lastAction.observedAt, 1000);
  assert.equal(reducer.snapshot(7099).engagements.length, 1);
  const expired = reducer.snapshot(7100);
  assert.deepEqual(expired.engagements, []);
  assert.equal(expired.entities.find(value => value.objectId === 8).lastAction, undefined);
  reducer.apply(packet("ObjectStruck", { attackerId: 8, objectId: 19 }), 8000);
  assert.deepEqual(reducer.apply(world(), 8100).engagements, []);
});

test("combat observations are bounded and disappear with either endpoint or the world lifecycle", () => {
  const reducer = readyReducer();
  for (let id = 20; id < 90; id += 1) {
    reducer.apply(packet("ObjectMonster", { objectId: id, name: "Monster", location: { x: 286, y: 606 }, dead: false }), 1000);
    reducer.apply(packet("ObjectStruck", { attackerId: 8, objectId: id }), 1000);
  }
  const bounded = reducer.snapshot(1100).engagements;
  assert.equal(bounded.length, 64);
  assert.equal(bounded[0].targetId, 26);
  reducer.apply(packet("ObjectDied", { objectId: 26 }), 1200);
  assert.equal(reducer.snapshot(1200).engagements.some(value => value.targetId === 26), false);
  reducer.apply(packet("ObjectHide", { objectId: 27 }), 1300);
  assert.equal(reducer.snapshot(1300).engagements.some(value => value.targetId === 27), false);
  reducer.apply(packet("ObjectRemove", { objectId: 8 }), 1400);
  assert.deepEqual(reducer.snapshot(1400).engagements, []);

  const other = visibleCombatReducer();
  other.apply(packet("ObjectStruck", { attackerId: 8, objectId: 19 }), 1100);
  assert.deepEqual(other.apply(packet("MapChanged", { fileName: "0141", mapIndex: 141 }), 1200).engagements, []);
  assert.deepEqual(other.resetConnection().engagements, []);
});

test("real HP loss tracks recent damage across snapshots; healing does not imply a helper or death", () => {
  const reducer = readyReducer();
  const low = reducer.apply(packet("HealthChanged", { hp: 4, mp: 10 }), 1000);
  assert.equal(low.lastDamageAt, 1000);
  assert.equal(low.player.dead, false);
  const lowerSnapshot = reducer.apply(world({ playerHp: 3 }), 1100);
  assert.equal(lowerSnapshot.lastDamageAt, 1100);
  const healed = reducer.apply(packet("HealthChanged", { hp: 15, mp: 10 }), 1200);
  assert.equal(healed.lastDamageAt, 1100);
  assert.deepEqual(healed.engagements, []);
  assert.equal(healed.helperId, undefined);
  assert.equal(reducer.apply(world(), 1300).lastDamageAt, 1100);
  assert.equal(reducer.apply(packet("Death", { location: { x: 285, y: 606 } }), 1400).player.dead, true);
  assert.equal(reducer.apply(packet("MapChanged", { fileName: "0141" }), 1500).lastDamageAt, null);
  assert.equal(reducer.resetConnection().lastDamageAt, null);
});

test("UseItem acknowledgement preserves exact UID and success while a snapshot alone reconciles inventory", () => {
  const reducer = readyReducer();
  const used = reducer.apply(packet("UseItem", { uniqueId: 18446744073709551615n, success: true, grid: "Inventory" }), 1000);
  assert.deepEqual(used.lastItemUse, { uniqueId: 18446744073709551615n, success: true, grid: "Inventory", observedAt: 1000 });
  assert.equal(used.inventoryStale, true);
  assert.equal(used.inventoryItems[0].quantity, 2);
  const fresh = reducer.apply(world(), 1100);
  assert.equal(fresh.inventoryStale, false);
  assert.equal(fresh.lastItemUse.observedAt, 1000);
  const failed = reducer.apply(packet("UseItem", { uniqueId: 9007199254740993n, success: false, grid: "Belt" }), 1200);
  assert.equal(failed.lastItemUse.success, false);
  assert.equal(failed.inventoryStale, false);
  assert.equal(reducer.resetConnection().lastItemUse, null);
});

function sharedSelfReducer(zoneId = 700) {
  const reducer = visibleCombatReducer();
  const entities = reducer.snapshot(1000).entities;
  reducer.apply(world({ selfZoneObjectId: zoneId, entities }), 1000);
  return reducer;
}

test("a trusted self Zone alias validates raw engagement IDs without creating a remote self entity", () => {
  const reducer = sharedSelfReducer();
  assert.equal(reducer.snapshot(1000).selfZoneObjectId, 700);
  assert.equal(reducer.snapshot(1000).player.objectId, 7);
  assert.equal(reducer.snapshot(1000).entities.some(value => value.objectId === 700), false);
  const attacking = reducer.apply(packet("ObjectStruck", { attackerId: 700, objectId: 19 }), 1100);
  assert.deepEqual(attacking.engagements, [{ attackerId: 700, targetId: 19, observedAt: 1100 }]);
  const attacked = reducer.apply(packet("ObjectStruck", { attackerId: 19, objectId: 700, location: { x: 284, y: 607 } }), 1200);
  assert.equal(attacked.player.objectId, 7);
  assert.equal(attacked.player.y, 606, "a struck pose is not an authoritative self movement");
  assert.deepEqual(attacked.engagements[1], { attackerId: 19, targetId: 700, observedAt: 1200 });
  // Two names for the same self actor cannot become a self-versus-self hit.
  assert.equal(reducer.apply(packet("ObjectStruck", { attackerId: 7, objectId: 700 }), 1300).engagements.length, 2);
  reducer.apply(packet("ObjectPlayer", { objectId: 700, name: "Resident", location: { x: 284, y: 607 } }), 1400);
  assert.equal(reducer.snapshot(1400).entities.some(value => value.objectId === 700), false);
  const posed = reducer.apply(packet("ObjectAttack", { objectId: 700, direction: "Right", spell: 0 }), 1500);
  assert.equal(posed.player.objectId, 7);
  assert.equal(posed.player.lastAction.kind, "attack");
  const refreshed = reducer.apply(world({ selfZoneObjectId: 700, entities: posed.entities }), 1600);
  assert.equal(refreshed.engagements.length, 2);
  assert.deepEqual(reducer.snapshot(7200).engagements, []);
});

test("global self death and removal affect the local self actor and all its engagement aliases", () => {
  const reducer = sharedSelfReducer();
  reducer.apply(packet("ObjectStruck", { attackerId: 700, objectId: 19 }), 1100);
  reducer.apply(packet("ObjectStruck", { attackerId: 8, objectId: 7 }), 1200);
  const dead = reducer.apply(packet("ObjectDied", { objectId: 700, location: { x: 284, y: 607 } }), 1300);
  assert.equal(dead.player.objectId, 7);
  assert.equal(dead.player.dead, true);
  assert.equal(dead.playerHp, 0);
  assert.deepEqual(dead.engagements, []);
  const revived = reducer.apply(packet("ObjectRevived", { objectId: 700 }), 1400);
  assert.equal(revived.player.dead, false);
  assert.equal(revived.playerHp, 0);
  const removed = reducer.apply(packet("ObjectRemove", { objectId: 700 }), 1500);
  assert.equal(removed.player, null);
  assert.equal(removed.selfZoneObjectId, null);
  assert.equal(removed.ready, false);
});

test("snapshot rebinding, map changes and reset discard an obsolete self Zone identity", () => {
  const reducer = sharedSelfReducer();
  reducer.apply(packet("ObjectStruck", { attackerId: 700, objectId: 19 }), 1100);
  const entities = reducer.snapshot(1100).entities;
  const rebound = reducer.apply(world({ selfZoneObjectId: 701, entities }), 1200);
  assert.equal(rebound.selfZoneObjectId, 701);
  assert.deepEqual(rebound.engagements, []);
  assert.deepEqual(reducer.apply(packet("ObjectStruck", { attackerId: 700, objectId: 19 }), 1300).engagements, []);
  assert.equal(reducer.apply(packet("ObjectStruck", { attackerId: 701, objectId: 19 }), 1400).engagements[0].attackerId, 701);
  const moved = reducer.apply(packet("MapChanged", { fileName: "0141", mapIndex: 141 }), 1500);
  assert.equal(moved.selfZoneObjectId, null);
  assert.deepEqual(moved.engagements, []);
  assert.equal(reducer.apply(world({ mapFileName: "0141", selfZoneObjectId: 702 }), 1600).selfZoneObjectId, 702);
  assert.equal(reducer.resetConnection().selfZoneObjectId, null);
});

test("only a valid worldSnapshot field can introduce the trusted self Zone alias", () => {
  const reducer = visibleCombatReducer();
  reducer.apply(packet("UserInformation", { objectId: 7, name: "Resident", selfZoneObjectId: 700, location: { x: 284, y: 606 } }), 1100);
  assert.equal(reducer.snapshot(1100).selfZoneObjectId, null);
  reducer.apply(packet("ObjectChat", { objectId: 8, text: "selfZoneObjectId=700", selfZoneObjectId: 700 }), 1200);
  assert.equal(reducer.snapshot(1200).selfZoneObjectId, null);
  assert.equal(reducer.apply(world({ selfZoneObjectId: "700" }), 1300).selfZoneObjectId, null);
  const duplicateProjection = world({ selfZoneObjectId: 700 });
  duplicateProjection.payload.entities.push({ objectId: 700, kind: "player", name: "Resident", x: 284, y: 606 });
  const snapshot = reducer.apply(duplicateProjection, 1400);
  assert.equal(snapshot.selfZoneObjectId, 700);
  assert.equal(snapshot.entities.some(value => value.objectId === 700), false);
});

test("late self attack and struck poses cannot roll back a newer UserLocation", () => {
  const reducer = new ObservationReducer();
  reducer.apply(packet("StartGame", { result: 4, resolution: 1024 }), 1000);
  const initial = world({ selfZoneObjectId: 700 });
  initial.payload.entities.push({ objectId: 19, kind: "monster", name: "Scarecrow", x: 286, y: 606, hp: 20, dead: false });
  reducer.apply(initial, 1100);
  reducer.apply(packet("ObjectAttack", { objectId: 700, location: { x: 284, y: 606 }, direction: "Right", spell: 0 }), 1200);
  const arrived = reducer.apply(packet("UserLocation", { x: 285, y: 607, direction: "DownRight" }), 1300);
  assert.deepEqual({ x: arrived.player.x, y: arrived.player.y, direction: arrived.player.direction }, { x: 285, y: 607, direction: "DownRight" });
  const lateAttack = reducer.apply(packet("ObjectAttack", { objectId: 700, location: { x: 284, y: 606 }, direction: "Right", spell: 0 }), 1400);
  assert.deepEqual({ x: lateAttack.player.x, y: lateAttack.player.y, direction: lateAttack.player.direction }, { x: 285, y: 607, direction: "DownRight" });
  assert.equal(lateAttack.player.lastAction.direction, "Right", "retain the observed pose separately from the owner's transform");
  const lateStruck = reducer.apply(packet("ObjectStruck", { objectId: 700, attackerId: 19, location: { x: 284, y: 606 }, direction: "Left" }), 1500);
  assert.deepEqual({ x: lateStruck.player.x, y: lateStruck.player.y, direction: lateStruck.player.direction }, { x: 285, y: 607, direction: "DownRight" });
  assert.deepEqual(lateStruck.engagements, [{ attackerId: 19, targetId: 700, observedAt: 1500 }]);
  assert.equal(lateStruck.playerHp, 15);
  assert.equal(lateStruck.lastDamageAt, null, "struck animation does not invent an HP loss");
  const hurt = reducer.apply(packet("HealthChanged", { hp: 10, mp: 10 }), 1600);
  assert.equal(hurt.playerHp, 10);
  assert.equal(hurt.lastDamageAt, 1600);
  assert.deepEqual({ x: hurt.player.x, y: hurt.player.y }, { x: 285, y: 607 });
  const died = reducer.apply(packet("Death", { location: { x: 285, y: 607 }, direction: "DownRight" }), 1700);
  assert.equal(died.player.dead, true);
  assert.equal(died.playerHp, 0);
});

test("local self pose IDs and observer movement broadcasts also leave the owner ACK intact", () => {
  const reducer = sharedSelfReducer();
  reducer.apply(packet("UserLocation", { x: 285, y: 607, direction: "DownRight" }), 1100);
  for (const objectId of [7, 700]) {
    reducer.apply(packet("ObjectAttack", { objectId, location: { x: 284, y: 606 }, direction: "Left" }), 1200);
    reducer.apply(packet("ObjectStruck", { objectId, attackerId: 19, location: { x: 284, y: 606 }, direction: "Up" }), 1300);
    for (const movement of ["ObjectWalk", "ObjectRun", "ObjectTurn", "ObjectBackStep"]) {
      const current = reducer.apply(packet(movement, { objectId, x: 284, y: 606, direction: "Right" }), 1400);
      assert.deepEqual({ x: current.player.x, y: current.player.y, direction: current.player.direction }, { x: 285, y: 607, direction: "DownRight" });
    }
  }
});

test("remote actors still take their observed attack and struck transforms", () => {
  const reducer = visibleCombatReducer();
  const attacked = reducer.apply(packet("ObjectAttack", { objectId: 8, location: { x: 285, y: 607 }, direction: "UpRight", spell: 0 }), 1100);
  const remote = attacked.entities.find(value => value.objectId === 8);
  assert.deepEqual({ x: remote.x, y: remote.y, direction: remote.direction }, { x: 285, y: 607, direction: "UpRight" });
  const struck = reducer.apply(packet("ObjectStruck", { attackerId: 19, objectId: 8, location: { x: 285, y: 608 }, direction: "Left" }), 1200);
  const hit = struck.entities.find(value => value.objectId === 8);
  assert.deepEqual({ x: hit.x, y: hit.y, direction: hit.direction }, { x: 285, y: 608, direction: "Left" });
  assert.deepEqual(struck.engagements, [{ attackerId: 19, targetId: 8, observedAt: 1200 }]);
  assert.deepEqual({ x: struck.player.x, y: struck.player.y }, { x: 284, y: 606 });
});
