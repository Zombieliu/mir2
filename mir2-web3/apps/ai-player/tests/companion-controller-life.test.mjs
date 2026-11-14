import assert from "node:assert/strict";
import test from "node:test";
import { CompanionController } from "../src/companion-controller.mjs";
import { fixture, grant, potion } from "./life-fixture.mjs";

function resident(t) {
  const f = fixture(t), actions = [];
  Object.assign(f.client, {
    async walk(direction) { actions.push(["walk", direction]); return { moved: true }; },
    async attackDirection(direction) { actions.push(["attack", direction]); return { sent: true, confirmed: false }; },
    async pickUpTile() { actions.push(["loot"]); return { changed: false }; },
    async useOwnedItem(uniqueId, grid) { actions.push(["potion", String(uniqueId), grid]); f.client.state.inventoryItems[0].quantity--; f.client.state.revision++; return { success: true, uniqueId }; },
  });
  f.supply.tick = async () => { actions.push(["supply-tick"]); return { performed: false }; };
  const controller = new CompanionController({ client: f.client, store: f.store, leaderName: "Owner", homePosition: { ...grant().home, mapFileName: "0" },
    clock: () => f.client.time, lifeController: f.controller, supplyController: f.supply, mapLoader: async () => ({ width: 800, height: 800, blocked: new Set() }) });
  t.after(() => controller.close()); f.controller.authorize(grant());
  return { ...f, life: f.controller, actor: controller, actions };
}

test("life hunts through the existing ordinary executor while grouped owner is outside view", async t => {
  const f = resident(t); f.client.state.party.members = ["Owner", "Alo"];
  f.client.state.entities.push({ objectId: 80, kind: "monster", name: "Deer", x: 288, y: 617, hp: 10, disposition: "neutral" });
  const result = await f.actor.tick();
  assert.equal(result.reason, "life-hunt-observed-monster"); assert.equal(result.performed, true); assert.equal(f.actions.length, 1); assert.equal(f.actions[0][0], "attack");
  assert.deepEqual(f.client.state.party.members, ["Owner", "Alo"]);
});

test("a child grant claim owns a whole tick and supply execution owns subsequent ticks", async t => {
  const f = resident(t); f.client.state.inventoryItems = [potion(1)];
  assert.equal((await f.actor.tick()).kind, "life");
  assert.equal(f.client.requests.length, 1); assert.deepEqual(f.actions, []);
  assert.equal((await f.actor.tick()).kind, "supply"); assert.deepEqual(f.actions, [["supply-tick"]]);
});

test("actual medicine, survival withdrawal and recall keep priority over life; unknown medicine blocks its purchase", async t => {
  const f = resident(t); f.client.state.playerHp = 10;
  assert.equal((await f.actor.tick()).kind, "usePotion"); assert.equal(f.actions[0][0], "potion"); assert.equal(f.client.requests.length, 0);
  f.client.state.playerHp = 18; f.client.time = 5000;
  f.client.useOwnedItem = async () => { throw new Error("synthetic uncertain medicine"); };
  f.client.state.inventoryItems = [potion(1)]; f.client.state.playerHp = 10;
  await assert.rejects(f.actor.tick(), /uncertain medicine/); f.client.state.playerHp = 18; f.client.time = 15_000;
  assert.equal((await f.actor.tick()).kind, "wait"); assert.equal(f.client.requests.length, 0);
  const resources = f.actor.resourceView(); assert.equal(resources.medicineUnresolved, true); assert.equal(resources.usableHpPotionCount, null);
  f.actor.recallRequested = true; f.client.state.player.y = 620;
  assert.equal((await f.actor.tick()).reason, "owner-recall"); assert.equal(f.client.requests.length, 0);
});

test("active or uncertain shared trade blocks gameplay and life supply until resolved", async t => {
  for (const trade of [{ active: true, uncertain: false }, { active: false, uncertain: true }]) {
    const f = resident(t); f.client.state.inventoryItems = [potion(1)]; f.client.state.trade = trade;
    const result = await f.actor.tick(); assert.equal(result.kind, "wait");
    assert.equal(result.reason, trade.uncertain ? "trade-outcome-unknown" : "trade-in-progress"); assert.deepEqual(f.actions, []); assert.equal(f.client.requests.length, 0);
  }
});

test("awaited map loading cannot act for an expired, recalled, changed-policy, changed-world or replaced life", async t => {
  for (const change of [f => { f.client.time = 1_200_000; }, f => { f.actor.recallRequested = true; }, f => { f.policy.policyVersion++; },
    f => { f.client.state.connectionEpoch++; }, f => { f.life.cancel(); }, f => { f.client.state.trade = { uncertain: true }; },
    f => { f.life.authorize(grant({ lifePermitId: "life:replacement" })); }]) {
    const f = resident(t); let release;
    f.actor.mapLoader = () => new Promise(resolve => { release = resolve; });
    const pending = f.actor.tick(); assert.equal(typeof release, "function"); change(f); release({ width: 800, height: 800, blocked: new Set() });
    assert.equal((await pending).performed, false); assert.deepEqual(f.actions, []);
  }
});

test("genuine self-defence remains available but cannot chase beyond the life movement boundary", async t => {
  const f = resident(t); f.client.state.player.x = 310; f.client.state.player.y = 630; f.client.state.inSafeZone = false;
  f.client.state.entities.push({ objectId: 80, kind: "monster", name: "OtherHostile", x: 312, y: 630, hp: 10, disposition: "hostile" });
  f.client.state.engagements = [{ attackerId: 80, targetId: 50001, observedAt: 0 }];
  const blocked = await f.actor.tick(); assert.equal(blocked.reason, "defend-confirmed-engagement"); assert.equal(blocked.performed, false); assert.deepEqual(f.actions, []);
  f.client.state.entities.find(e => e.objectId === 80).x = 311;
  assert.equal((await f.actor.tick()).reason, "defend-confirmed-engagement"); assert.equal(f.actions.at(-1)[0], "attack");
});

test("survival withdrawal remains visible as a return and records actual safe arrival before resuming", async t => {
  const f = resident(t); f.client.state.player.y = 628; f.client.state.playerHp = 5; f.client.state.inventoryItems = []; f.client.state.inSafeZone = false;
  assert.equal((await f.actor.tick()).reason, "unsafe-withdraw"); assert.equal(f.life.view().phase, "returning-home");
  f.client.state.player.y = 616; f.client.state.inSafeZone = true;
  await f.actor.tick(); assert.equal(f.life.view().phase, "resting");
  assert.equal(f.facts.filter(event => event.kind === "life-returned").length, 1); assert.equal(f.client.requests.length, 0);
  f.client.state.playerHp = 18; f.client.state.inventoryItems = [potion(2)];
  assert.equal((await f.actor.tick()).reason, "life-route-waypoint");
  assert.equal(f.facts.filter(event => event.kind === "life-returned").length, 1);
});
