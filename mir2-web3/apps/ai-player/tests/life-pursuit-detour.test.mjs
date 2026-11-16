import assert from "node:assert/strict";
import test from "node:test";
import { CompanionController } from "../src/companion-controller.mjs";
import { loadClientCollision, nextWalkDirection } from "../src/skills/navigation.mjs";
import { fixture, grant } from "./life-fixture.mjs";

const OFFSETS = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] };
const map = loadClientCollision("0");
const shortDetour = async () => ({ width: 400, height: 700, blocked: new Set(["287,629", "288,629", "289,629", "287,630", "289,630"]) });
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const budget = f => { const v = f.life.view(); return { startedAtMs: v.startedAtMs, expiresAtMs: v.expiresAtMs, budgetGold: v.budgetGold, spentGold: v.spentGold,
  childSupplyClaimed: v.childSupplyClaimed, purchasedQuantity: v.purchasedQuantity, modelDecisionsUsed: v.modelDecisionsUsed, gold: f.client.state.gold }; };

function resident(t, { mapLoader = shortDetour } = {}) {
  const f = fixture(t, { path: ":memory:" }), actions = [];
  Object.assign(f.client.state.player, { x: 288, y: 630 }); f.client.state.inSafeZone = false; f.client.state.party.members = ["Owner", "Alo"];
  const target = { objectId: 80, kind: "monster", name: "Scarecrow", x: 288, y: 624, hp: 20, dead: false, disposition: "neutral" };
  f.client.state.entities.push(target);
  Object.assign(f.client, {
    async walk(direction) { const [dx, dy] = OFFSETS[direction]; actions.push({ kind: "walk", direction }); this.state.player.x += dx; this.state.player.y += dy; this.state.revision++; return { moved: true }; },
    async attackDirection(direction) { actions.push({ kind: "attack", direction }); return { sent: true, confirmed: false }; },
    async useOwnedItem(uniqueId) { actions.push({ kind: "potion" }); this.state.inventoryItems[0].quantity--; this.state.revision++; return { success: true, uniqueId }; },
  });
  const actor = new CompanionController({ client: f.client, store: f.store, leaderName: "Owner", homePosition: { ...grant().home, mapFileName: "0" },
    lifeController: f.controller, clock: () => f.client.time, mapLoader });
  t.after(() => actor.close()); f.controller.authorize(grant());
  return { ...f, life: f.controller, actor, target, actions };
}

test("the public thirty-six-step Bichon route is skipped before the first chase step, while normal routing continues", async t => {
  const f = resident(t, { mapLoader: () => map }), before = budget(f), collision = await map;
  const request = { player: f.client.state.player, target: f.target, desiredDistance: 1, collision, movementBounds: grant().bounds, occupied: [f.target] };
  assert.equal(nextWalkDirection(request), "DownRight"); assert.equal(nextWalkDirection({ ...request, maxPathSteps: 12 }), null);
  const first = await f.actor.tick(); assert.equal(first.targetId, 80); assert.equal(first.performed, false); assert.equal(first.blocked, true);
  assert.deepEqual(f.actions, []); assert.match(f.life.view().reason, /路线过长或受阻/);
  const second = await f.actor.tick(); assert.equal(second.reason, "life-route-waypoint"); assert.equal(second.performed, true); assert.equal(f.actions.length, 1);
  assert.deepEqual(budget(f), before); assert.deepEqual(f.client.requests, []); assert.deepEqual(f.facts, []);
});

test("a short collision detour keeps its observed target when the first legal step increases distance from six to seven", async t => {
  const f = resident(t), before = budget(f);
  const first = await f.actor.tick(); assert.equal(first.reason, "life-hunt-observed-monster"); assert.equal(first.targetId, 80);
  assert.deepEqual(f.actions[0], { kind: "walk", direction: "Down" });
  assert.deepEqual({ x: f.client.state.player.x, y: f.client.state.player.y }, { x: 288, y: 631 }); assert.equal(distance(f.client.state.player, f.target), 7);
  const second = await f.actor.tick(); assert.equal(second.reason, "life-hunt-observed-monster"); assert.equal(second.targetId, 80);
  assert.deepEqual(f.actions[1], { kind: "walk", direction: "Right" });
  assert.deepEqual({ x: f.client.state.player.x, y: f.client.state.player.y }, { x: 289, y: 631 });
  assert.deepEqual(budget(f), before); assert.deepEqual(f.client.requests, []);
});

test("first acquisition remains limited to six tiles and a newly visible seven-tile target is skipped", async t => {
  const f = resident(t); f.client.state.player.y = 631;
  const result = await f.actor.tick(); assert.equal(distance({ x: 288, y: 631 }, f.target), 7);
  assert.equal(result.reason, "life-route-waypoint"); assert.match(result.targetId, /^life-waypoint:/);
});

test("a valid already tracked target retains priority over a newly visible closer neutral target", async t => {
  const f = resident(t); await f.actor.tick();
  f.client.state.entities.push({ objectId: 81, kind: "monster", name: "Deer", x: 287, y: 631, hp: 10, dead: false, disposition: "neutral" });
  const result = await f.actor.tick(); assert.equal(result.targetId, 80); assert.equal(result.reason, "life-hunt-observed-monster");
});

test("a short detour actually reaches attack range within twelve steps without returning to its first tile", async t => {
  const f = resident(t), before = budget(f), positions = [];
  for (let i = 0; i < 9; i++) { const result = await f.actor.tick(); assert.equal(result.targetId, 80); assert.equal(result.performed, true); positions.push(`${f.client.state.player.x},${f.client.state.player.y}`); }
  assert.equal(distance(f.client.state.player, f.target), 1); assert.equal(new Set(positions).size, positions.length);
  const result = await f.actor.tick(); assert.equal(result.targetId, 80); assert.equal(f.actions.at(-1).kind, "attack");
  assert.deepEqual(budget(f), before); assert.deepEqual(f.client.requests, []);
});

test("a previously observed target moving beyond twelve tiles is immediately abandoned", async t => {
  const f = resident(t), before = budget(f);
  await f.actor.tick(); Object.assign(f.client.state.player, { x: 288, y: 637 });
  assert.equal(distance(f.client.state.player, f.target), 13);
  const result = await f.actor.tick(); assert.equal(result.reason, "life-route-waypoint"); assert.match(result.targetId, /^life-waypoint:/);
  assert.deepEqual(budget(f), before); assert.equal(f.actions.some(value => value.kind === "attack"), false);
});

test("disappearance, death, zero HP, a disallowed name or an out-of-bounds target abandon pursuit immediately", async t => {
  for (const invalidate of [f => { f.client.state.entities = [f.client.state.player]; }, f => { f.target.dead = true; }, f => { f.target.hp = 0; },
    f => { f.target.name = "OtherHostile"; }, f => { Object.assign(f.client.state.player, { x: 300, y: 630 }); Object.assign(f.target, { x: 311, y: 624 }); }]) {
    const f = resident(t); await f.actor.tick(); invalidate(f);
    const result = await f.actor.tick(); assert.equal(result.reason, "life-route-waypoint"); assert.notEqual(result.targetId, 80);
  }
});

test("loss of a target while loading the public map prevents a cached pursuit step", async t => {
  const f = resident(t); let release; f.actor.mapLoader = () => new Promise(resolve => { release = resolve; });
  const pending = f.actor.tick(); assert.equal(typeof release, "function");
  f.client.state.entities = [f.client.state.player]; release(await map);
  assert.equal((await pending).performed, false); assert.deepEqual(f.actions, []);
});

test("world changes, cancellation and real permit expiry stop pursuit before another monster action", async t => {
  for (const change of [f => { f.client.state.connectionEpoch++; }, f => { f.life.cancel(); }, f => { f.client.time = grant().expiresAtMs; }]) {
    const f = resident(t); await f.actor.tick(); const before = budget(f); change(f);
    const result = await f.actor.tick(); assert.notEqual(result.kind, "attack"); assert.notEqual(result.targetId, 80);
    assert.deepEqual(budget(f), before); assert.deepEqual(f.client.requests, []);
  }
});

test("genuine immediate self-defence, owned medicine and recall retain priority over continued pursuit", async t => {
  const danger = resident(t); await danger.actor.tick();
  danger.client.state.entities.push({ objectId: 81, kind: "monster", name: "OtherHostile", x: 289, y: 631, hp: 10, dead: false, disposition: "hostile" });
  danger.client.state.engagements = [{ attackerId: 81, targetId: 50001, observedAt: 0 }];
  const defence = await danger.actor.tick(); assert.equal(defence.reason, "defend-confirmed-engagement"); assert.equal(defence.targetId, 81); assert.equal(danger.actions.at(-1).kind, "attack");
  const healing = resident(t); await healing.actor.tick(); healing.client.state.playerHp = 10;
  assert.equal((await healing.actor.tick()).kind, "usePotion"); assert.equal(healing.actions.at(-1).kind, "potion"); assert.equal(healing.client.requests.length, 0);
  const recalled = resident(t); await recalled.actor.tick(); recalled.actor.recallRequested = true;
  assert.equal((await recalled.actor.tick()).reason, "owner-recall"); assert.equal(recalled.client.requests.length, 0);
});

test("an obstructed target is deferred for thirty seconds while another newly observed reachable target can be used", async t => {
  const f = resident(t, { mapLoader: () => map }); await f.actor.tick();
  f.client.state.entities.push({ objectId: 81, kind: "monster", name: "Hen", x: 289, y: 631, hp: 10, dead: false, disposition: "neutral" });
  const alternate = await f.actor.tick(); assert.equal(alternate.targetId, 81); assert.equal(f.actions.at(-1).kind, "attack");
  f.client.state.entities = [f.client.state.player, f.target]; f.client.time = 29_999; Object.assign(f.client.state.player, { x: 288, y: 630 });
  assert.equal((await f.actor.tick()).reason, "life-route-waypoint");
  f.client.time = 30_000; Object.assign(f.client.state.player, { x: 288, y: 630 });
  const retried = await f.actor.tick(); assert.equal(retried.targetId, 80); assert.equal(retried.performed, false); assert.equal(retried.blocked, true);
  assert.equal(f.client.requests.length, 0); assert.equal(f.life.view().modelDecisionsUsed, 0);
});

test("deferred targets are bounded to thirty-two current observations, and a death clears the old deferral", async t => {
  const surround = new Set(Object.values(OFFSETS).map(([dx, dy]) => `${288 + dx},${630 + dy}`));
  const f = resident(t, { mapLoader: async () => ({ width: 400, height: 700, blocked: surround }) });
  const targets = [];
  for (let x = 282; x <= 294 && targets.length < 33; x++) for (let y = 624; y <= 636 && targets.length < 33; y++) {
    if (distance({ x, y }, f.client.state.player) < 2) continue;
    targets.push({ objectId: 100 + targets.length, kind: "monster", name: "Deer", x, y, hp: 10, dead: false, disposition: "neutral" });
  }
  f.client.state.entities = [f.client.state.player, ...targets];
  const first = await f.actor.tick(); const seen = new Set([first.targetId]);
  for (let i = 1; i < 33; i++) { const result = await f.actor.tick(); assert.equal(seen.has(result.targetId), false); seen.add(result.targetId); }
  assert.equal((await f.actor.tick()).targetId, first.targetId); assert.deepEqual(f.actions, []);
  const died = targets.find(target => target.objectId === first.targetId); died.dead = true; await f.actor.tick(); died.dead = false;
  assert.equal((await f.actor.tick()).targetId, first.targetId); assert.deepEqual(f.client.requests, []); assert.deepEqual(f.facts, []);
});
