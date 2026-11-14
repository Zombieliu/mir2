import assert from "node:assert/strict";
import test from "node:test";
import { decideCompanionAction } from "../src/companion-policy.mjs";
import { ObservationReducer } from "../src/observation.mjs";

const originalPolicy = () => ({
  policyVersion: 1,
  style: "collection",
  solo: { considerLootWhenSafe: true },
  party: { assistBeforeLoot: false, lootOnlyAfterCombat: false, askBeforeLeaving: false },
  permissions: { spendLimit: 0, mapAllowlist: ["0", "0141"] },
});
const taughtPolicy = () => ({ ...originalPolicy(), policyVersion: 2, party: { assistBeforeLoot: true, lootOnlyAfterCombat: true, askBeforeLeaving: true } });
function world() {
  const self = { objectId: 1, kind: "selfPlayer", name: "Alo", x: 10, y: 10, hp: 100, maxHp: 100, dead: false };
  return {
    ready: true, mapFileName: "0", playerObjectId: 1, player: self,
    playerHp: 100, playerMaxHp: 100, inventoryStale: false,
    inventoryItems: [], beltItems: [], freeBagSlots: 20,
    entities: [self,
      { objectId: 2, kind: "player", name: "Owner", x: 12, y: 10, hp: 90, maxHp: 100 },
      { objectId: 10, kind: "monster", name: "Scarecrow", x: 12, y: 11, hp: 20, disposition: "hostile" },
    ],
    party: { leaderName: "Owner", members: ["Owner", "Alo"], pendingInvite: null },
    engagements: [{ attackerId: 2, targetId: 10, observedAt: 9_000 }],
    groundDrops: [{ objectId: 50, name: "Candle", x: 11, y: 10, ownerObjectId: 1, ownershipRemainingTicks: 100 }],
  };
}
const decide = (state = world(), policy = originalPolicy(), options = {}) => decideCompanionAction({ state, policy, leaderName: "Owner", nowMs: 10_000, homePosition: { x: 5, y: 5, mapFileName: "0" }, ...options });

test("the same safe party scene changes from collection to support after feedback", () => {
  const state = world();
  const before = structuredClone(state);
  assert.equal(decide(state).kind, "loot");
  const trained = decide(state, taughtPolicy());
  assert.equal(trained.kind, "attack");
  assert.equal(trained.targetId, 10);
  assert.equal(trained.policyVersion, 2);
  assert.deepEqual(state, before, "policy cannot mutate facts to manufacture a different outcome");
});

test("ordinary ObjectPlayer plus GroupMemberInfo provides the real leader health for collection", () => {
  const reducer = new ObservationReducer();
  const initial = world();
  delete initial.entities[1].hp;
  delete initial.entities[1].maxHp;
  reducer.apply({ type: "packet", packet: "StartGame", payload: { result: 4 } }, 9_000);
  reducer.apply({ type: "worldSnapshot", payload: initial }, 9_000);
  reducer.apply({ type: "packet", packet: "GroupMemberInfo", payload: {
    leaderName: "Owner",
    members: [
      { name: "Owner", level: 1, class: "Warrior", hp: 90, maxHp: 100, online: true },
      { name: "Alo", level: 1, class: "Warrior", hp: 100, maxHp: 100, online: true },
    ],
  } }, 9_100);
  reducer.apply({ type: "packet", packet: "ObjectStruck", payload: {
    objectId: 10, attackerId: 2, location: { x: 12, y: 11 }, direction: "Up",
  } }, 9_200);
  const state = reducer.snapshot(10_000);
  assert.equal(state.entities.find(value => value.objectId === 2).hp, undefined);
  assert.equal(decide(state).kind, "loot", "healthy roster makes the initial collection choice observable");
  assert.equal(decide(state, taughtPolicy()).kind, "attack", "teaching changes the same ordinary network scene");
});

test("matching roster health wins, unknown health stays unknown and only known AOI health is a fallback", () => {
  const state = world();
  state.party.memberInfo = [{ name: "Owner", hp: 50, maxHp: 100, online: true }];
  assert.equal(decide(state).kind, "attack", "a healthy AOI value cannot hide an injured roster member");
  state.party.memberInfo[0].hp = 0;
  assert.equal(decide(state).kind, "attack", "zero HP remains known unsafe health");
  state.party.memberInfo = [{ name: "Owner", level: 1, online: true }];
  assert.equal(decide(state).kind, "loot", "known entity health remains usable if roster health is absent");
  delete state.entities[1].hp;
  delete state.entities[1].maxHp;
  assert.equal(decide(state).kind, "attack", "missing health is not assumed healthy");
  state.party.memberInfo = [{ name: "Stranger", hp: 100, maxHp: 100, online: true }];
  assert.equal(decide(state).kind, "attack", "another member's health cannot stand in for the leader");
  state.party.memberInfo = [{ name: "Owner", hp: 100, maxHp: 100, online: false }];
  assert.equal(decide(state).kind, "attack", "offline roster information cannot authorize opportunistic collection");
});

test("a policy reloaded from persistent JSON retains the taught choice and solo preference", () => {
  const reloaded = JSON.parse(JSON.stringify(taughtPolicy()));
  assert.equal(decide(world(), reloaded).kind, "attack");
  const solo = world();
  solo.party.members = [];
  solo.engagements = [];
  assert.equal(decide(solo, reloaded).kind, "loot");
  assert.equal(reloaded.permissions.spendLimit, 0);
});

test("battle-ended/dead target permits normal post-combat pickup", () => {
  const state = world();
  state.entities[2].dead = true;
  state.entities[2].hp = 0;
  assert.equal(decide(state, taughtPolicy()).kind, "loot");
});

test("expired, future and out-of-AOI engagement cannot become a support target", () => {
  for (const observedAt of [2_000, 11_000]) {
    const state = world();
    state.engagements[0].observedAt = observedAt;
    assert.equal(decide(state, taughtPolicy()).kind, "loot");
  }
  const absent = world();
  absent.entities = absent.entities.filter(value => value.objectId !== 10);
  assert.equal(decide(absent, taughtPolicy()).kind, "loot");
});

test("bare attack poses and player targets cannot manufacture a PvP engagement", () => {
  const state = world();
  state.groundDrops = [];
  state.engagements = [];
  state.entities[1].lastAction = { kind: "attack", direction: "Down" };
  assert.equal(decide(state, taughtPolicy()).kind, "wait");
  state.engagements = [{ attackerId: 2, targetId: 1, observedAt: 9_000 }];
  assert.equal(decide(state, taughtPolicy()).kind, "wait");
});

test("low health uses an actual owned HP potion by exact UID before choosing loot or attack", () => {
  const state = world();
  state.playerHp = 60;
  state.beltItems = [{ name: "(HP)DrugSmall", quantity: 1, uniqueId: 18_446_744_073_709_551_615n, container: "belt" }];
  const action = decide(state, taughtPolicy());
  assert.equal(action.kind, "usePotion");
  assert.equal(action.uniqueId, 18_446_744_073_709_551_615n);
  assert.equal(action.grid, "belt");
});

test("quest/storage/empty/rounded IDs and unknown item location cannot be used as medicine", () => {
  for (const item of [
    { name: "(HP)DrugSmall", quantity: 1, uniqueId: 4, container: "quest" },
    { name: "(HP)DrugSmall", quantity: 1, uniqueId: 4, container: "storage" },
    { name: "(HP)DrugSmall", quantity: 0, uniqueId: 4, container: "bag1" },
    { name: "(HP)DrugSmall", quantity: 1, uniqueId: Number.MAX_SAFE_INTEGER + 1, container: "bag1" },
    { name: "Fake Health Potion", quantity: 1, uniqueId: 4, container: "bag1" },
  ]) {
    const state = world();
    state.playerHp = 60;
    state.inventoryItems = [item];
    assert.equal(decide(state).kind, "retreat");
  }
  const stale = world();
  stale.playerHp = 60;
  stale.inventoryStale = true;
  stale.inventoryItems = [{ name: "(HP)DrugSmall", quantity: 1, uniqueId: 4, container: "bag1" }];
  assert.equal(decide(stale).kind, "retreat");
});

test("unknown HP waits, depleted health retreats, and death never generates another attack", () => {
  for (const hp of [null, NaN, -1]) {
    const state = world(); state.playerHp = hp;
    assert.equal(decide(state).kind, "wait");
  }
  const hurt = world(); hurt.playerHp = 30;
  assert.deepEqual(decide(hurt).target, { x: 5, y: 5 });
  const dead = world(); dead.playerHp = 0;
  assert.equal(decide(dead).reason, "dead");
});

test("actual self danger suppresses collection, and two active attackers force withdrawal", () => {
  const state = world();
  state.engagements = [{ attackerId: 10, targetId: 1, observedAt: 9_000 }];
  assert.equal(decide(state).kind, "attack");
  state.entities.push({ objectId: 11, kind: "monster", name: "Scarecrow", x: 9, y: 10, hp: 20, disposition: "hostile" });
  state.engagements.push({ attackerId: 11, targetId: 1, observedAt: 9_000 });
  assert.equal(decide(state).kind, "retreat");
});

test("following requires the real roster and a visible configured leader", () => {
  const state = world();
  state.engagements = [];
  state.groundDrops = [];
  state.entities[1].x = 15;
  assert.equal(decide(state).kind, "follow");
  state.party.members = ["Owner"];
  assert.equal(decide(state).kind, "wait");
  state.party.members = ["Owner", "Alo"];
  state.entities = state.entities.filter(value => value.objectId !== 2);
  assert.equal(decide(state).reason, "leader-not-visible");
});

test("only a fresh invitation from the configured leader is accepted", () => {
  const state = world();
  state.party.members = [];
  state.groundDrops = [];
  for (const pendingInvite of [{ name: "Stranger", observedAt: 9_000 }, { name: "Owner", observedAt: -30_000 }, { name: "Owner", observedAt: 11_000 }]) {
    state.party.pendingInvite = pendingInvite;
    assert.notEqual(decide(state).kind, "acceptInvite");
  }
  state.party.pendingInvite = { name: "Owner", observedAt: 9_000 };
  assert.equal(decide(state).kind, "acceptInvite");
});

test("loot respects ownership, capacity and the party leash", () => {
  const state = world();
  state.engagements = [];
  state.groundDrops[0].ownerObjectId = 99;
  assert.equal(decide(state).kind, "wait");
  state.groundDrops[0].ownershipRemainingTicks = 0;
  assert.equal(decide(state).kind, "loot");
  state.freeBagSlots = 0;
  assert.equal(decide(state).kind, "wait");
  state.freeBagSlots = 20;
  state.entities[1].x = 20;
  assert.notEqual(decide(state).kind, "loot");
});

test("hard map authorization still wins after preference training", () => {
  const state = world();
  state.mapFileName = "D022";
  assert.equal(decide(state, taughtPolicy()).reason, "map-not-authorized");
});

test("the explicit Gateway Zone self alias binds own drops and received hits", () => {
  const state = world();
  state.selfZoneObjectId = 50_000;
  state.groundDrops[0].ownerObjectId = 50_000;
  state.engagements = [];
  assert.equal(decide(state).kind, "loot");
  state.engagements = [{ attackerId: 10, targetId: 50_000, observedAt: 9_000 }];
  assert.equal(decide(state).kind, "attack", "an actual hit on Zone self suppresses safe collection");
  state.engagements = [{ attackerId: 50_000, targetId: 10, observedAt: 9_000 }];
  assert.equal(decide(state, taughtPolicy()).targetId, 10);
});

test("an unknown drop owner or struck target cannot invent a Zone self alias", () => {
  const state = world();
  state.engagements = [{ attackerId: 10, targetId: 50_000, observedAt: 9_000 }];
  assert.equal(decide(state).kind, "loot", "unknown target is not proof of self danger");
  state.groundDrops[0].ownerObjectId = 50_000;
  assert.equal(decide(state).kind, "wait", "an unknown owner remains foreign");
});

test("teaching asks before a loot target leaves the party leash while ordinary nearby loot remains allowed", () => {
  const state = world();
  state.engagements = [];
  state.entities[1].x = 6;
  state.groundDrops[0].x = 13;
  assert.equal(decide(state).kind, "follow");
  const taught = decide(state, taughtPolicy());
  assert.equal(taught.kind, "requestApproval");
  assert.equal(taught.reason, "loot-would-leave-party-leash");
  assert.equal(taught.targetId, 50);
  state.groundDrops[0].x = 9;
  assert.equal(decide(state, taughtPolicy()).kind, "loot", "followDistance is a formation goal, not a loot prohibition");
});

test("approval preferences never block emergency withdrawal, support or returning toward a distant leader", () => {
  const state = world();
  state.engagements = [];
  state.entities[1].x = 6;
  state.groundDrops[0].x = 13;
  state.playerHp = 30;
  assert.equal(decide(state, taughtPolicy()).kind, "retreat");
  state.playerHp = 100;
  state.engagements = [{ attackerId: 2, targetId: 10, observedAt: 9_000 }];
  assert.equal(decide(state, taughtPolicy()).kind, "attack");
  state.engagements = [];
  state.entities[1].x = 2;
  assert.equal(decide(state, taughtPolicy()).kind, "follow");
});

test("unknown foreign-drop expiry is not treated as free loot", () => {
  const state = world();
  state.engagements = [];
  state.groundDrops[0].ownerObjectId = 99;
  state.groundDrops[0].ownershipRemainingTicks = null;
  assert.equal(decide(state).kind, "wait");
  state.groundDrops[0].ownershipRemainingTicks = 0;
  assert.equal(decide(state).kind, "loot");
});

test("invalid formation/leash configuration fails instead of widening movement scope", () => {
  for (const options of [{ followDistance: 0 }, { followDistance: 7 }, { leashDistance: Infinity }, { leashDistance: 100 }, { followDistance: NaN }]) {
    assert.equal(decide(world(), taughtPolicy(), options).reason, "invalid-party-distance-config");
  }
});
