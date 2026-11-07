import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { basename, join, resolve, sep } from "node:path";
import { tmpdir } from "node:os";
import { CompanionController } from "../src/companion-controller.mjs";
import { loadClientCollision } from "../src/skills/navigation.mjs";
import { fixture, grant, potion } from "./life-fixture.mjs";

const OFFSETS = [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]];
const DIRECTIONS = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] };

function occupyNeighbours(f) {
  const state = f.client.state;
  state.entities = [state.player, ...OFFSETS.map(([dx, dy], i) => ({ objectId: 100 + i, kind: "npc", name: `Observed${i}`, x: state.player.x + dx, y: state.player.y + dy, dead: false }))];
}

function resident(t, options = {}) {
  const f = fixture(t, { path: ":memory:", ...options }), actions = [], reports = [];
  f.client.state.player.x = 300; f.client.state.player.y = 638; f.client.state.inSafeZone = false;
  f.client.state.party.members = ["Owner", "Alo"];
  Object.assign(f.client, {
    async walk(direction) { actions.push(["walk", direction]); const [dx, dy] = DIRECTIONS[direction]; f.client.state.player.x += dx; f.client.state.player.y += dy; f.client.state.revision++; return { moved: true }; },
    async attackDirection(direction) { actions.push(["attack", direction]); return { sent: true, confirmed: false }; },
    async useOwnedItem(uniqueId) { actions.push(["potion"]); return { success: true, uniqueId }; },
  });
  const actor = new CompanionController({ client: f.client, store: f.store, leaderName: "Owner", homePosition: { ...grant().home, mapFileName: "0" },
    lifeController: f.controller, supplyController: f.supply, clock: () => f.client.time,
    mapLoader: options.mapLoader ?? (async () => ({ width: 800, height: 800, blocked: new Set() })), report: value => reports.push(value) });
  t.after(() => actor.close());
  f.controller.authorize(grant());
  return { ...f, life: f.controller, actor, actions, reports };
}

async function ticks(f, count) { for (let i = 0; i < count; i++) await f.actor.tick(); }

test("only three revalidated no-path results rotate a fixed life waypoint without spending or planning", async t => {
  const f = resident(t); occupyNeighbours(f);
  const before = f.life.view();
  await ticks(f, 2); assert.equal(f.life.view().waypointIndex, 0); assert.equal(f.life.view().navigation.noPathCount, 2);
  await ticks(f, 1); assert.equal(f.life.view().waypointIndex, 1); assert.deepEqual(f.life.view().navigation.failedWaypoints, [0]);
  assert.equal(f.life.view().expiresAtMs, before.expiresAtMs); assert.equal(f.life.view().budgetGold, 50);
  assert.equal(f.life.view().childSupplyClaimed, false); assert.equal(f.life.view().modelDecisionsUsed, before.modelDecisionsUsed);
  assert.deepEqual(f.actions, []); assert.deepEqual(f.client.requests, []);
});

test("an acknowledged real step clears consecutive no-path counts before a later obstruction", async t => {
  const f = resident(t); occupyNeighbours(f); await ticks(f, 2);
  f.client.state.entities = [f.client.state.player]; await ticks(f, 1);
  assert.equal(f.actions.at(-1)[0], "walk"); assert.equal(f.life.view().navigation.noPathCount, 0);
  assert.deepEqual(f.life.view().navigation.failedWaypoints, []);
  occupyNeighbours(f); await ticks(f, 2); assert.equal(f.life.view().waypointIndex, 0); assert.equal(f.life.view().navigation.noPathCount, 2);
});

test("three blocked fixed waypoints return home and three blocked home paths pause honestly", async t => {
  const f = resident(t); occupyNeighbours(f); await ticks(f, 9);
  assert.deepEqual(f.life.view().navigation.failedWaypoints, [0, 1, 2]); assert.equal(f.life.view().navigation.returning, true);
  assert.equal(f.life.view().phase, "returning-home");
  await ticks(f, 3); assert.equal(f.life.view().status, "paused"); assert.match(f.life.view().reason, /返家.*不可达/);
  assert.equal(f.life.view().navigation.homeNoPathCount, 3); assert.equal(f.facts.some(value => value.kind === "life-returned" || value.kind === "life-completed"), false);
  await ticks(f, 4); assert.deepEqual(f.actions, []); assert.deepEqual(f.client.requests, []);
});

test("after route obstruction, an actually observed safe arrival waits for the owner", async t => {
  const f = resident(t); occupyNeighbours(f); await ticks(f, 9);
  f.client.state.entities = [f.client.state.player];
  f.client.state.player.x = 288; f.client.state.player.y = 616; f.client.state.inSafeZone = true;
  const result = await f.actor.tick(); assert.equal(result.kind, "wait"); assert.equal(f.life.view().status, "paused"); assert.match(f.life.view().reason, /路线.*阻塞/);
  assert.equal(f.facts.filter(value => value.kind === "life-returned").length, 1); assert.equal(f.facts.some(value => value.kind === "life-completed"), false);
});

test("persisted navigation counts survive a reboot without changing the child receipt, deadline or budget", async t => {
  const root = mkdtempSync(join(tmpdir(), "numeron-navigation-"));
  const path = join(root, "life.sqlite"), callbacks = [], f = resident({ after: fn => callbacks.push(fn) }, { path });
  f.client.state.player.x = 288; f.client.state.player.y = 616; f.client.state.inSafeZone = true; f.client.state.inventoryItems = [potion(1)];
  await f.actor.tick(); assert.equal(f.life.view().childSupplyClaimed, true);
  Object.assign(f.supply.current, { status: "completed", receiptKnown: true, acquiredQuantity: 1, chargedGold: 40 });
  f.client.state.inventoryItems = [potion(2)]; f.client.state.player.x = 300; f.client.state.player.y = 638; f.client.state.inSafeZone = false;
  occupyNeighbours(f); await ticks(f, 2); const before = f.life.view();
  await f.actor.close(); await f.life.close();
  const reopened = fixture(t, { path, client: f.client });
  assert.equal(reopened.controller.view().status, "paused"); assert.equal(reopened.controller.view().navigation.noPathCount, 2);
  reopened.controller.resume(grant());
  assert.equal(reopened.controller.view().expiresAtMs, before.expiresAtMs); assert.equal(reopened.controller.view().spentGold, 40);
  assert.equal(reopened.controller.view().purchasedQuantity, 1); assert.equal(reopened.controller.view().childSupplyClaimed, true);
  assert.equal(reopened.controller.view().modelDecisionsUsed, before.modelDecisionsUsed); assert.equal(reopened.controller.view().navigation.noPathCount, 2);
  t.after(() => { const absolute = resolve(root); assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("numeron-navigation-")); rmSync(absolute, { recursive: true, force: true }); });
});

test("zero-HP monster corpses cannot seal navigation while live zero-HP NPCs remain blockers", async t => {
  for (const kind of ["monster", "npc"]) {
    const f = resident(t); occupyNeighbours(f);
    for (const entity of f.client.state.entities) if (entity.objectId >= 100) { entity.kind = kind; entity.hp = 0; }
    const result = await f.actor.tick();
    assert.equal(result.performed === true, kind === "monster");
    assert.equal(f.actions.length, kind === "monster" ? 1 : 0);
  }
});

test("an expired or cancelled life at home cannot keep submitting attacks after permission ends", async t => {
  for (const mode of ["expired", "cancelled"]) {
    const f = resident(t); Object.assign(f.client.state.player, grant().home); f.client.state.inSafeZone = true;
    f.client.state.entities = [f.client.state.player, { kind: "monster", objectId: 81, name: "Scarecrow", x: 288, y: 617, hp: 10, dead: false, disposition: "hostile" }];
    if (mode === "expired") f.client.time = grant().expiresAtMs; else f.life.cancel();
    const result = await f.actor.tick();
    assert.equal(result.kind, "wait"); assert.equal(result.reason, "life-session-finished"); assert.equal(f.actions.length, 0);
  }
});

test("world/policy changes during map loading never count as verified route obstruction", async t => {
  for (const change of [f => f.policy.policyVersion++, f => f.client.state.connectionEpoch++, f => f.actor.recallRequested = true]) {
    const f = resident(t); occupyNeighbours(f); let release;
    f.actor.mapLoader = () => new Promise(resolve => { release = resolve; });
    const pending = f.actor.tick(); change(f); release({ width: 800, height: 800, blocked: new Set() });
    assert.equal((await pending).performed, false); assert.equal(f.life.view().navigation.noPathCount, 0); assert.deepEqual(f.actions, []);
  }
});

test("life expiry, cancellation and route-recovery return beat self-defence chasing, preserving healing and danger priorities", async t => {
  for (const mode of ["expired", "cancelled", "blocked-route"]) {
    const f = resident(t);
    if (mode === "blocked-route") { occupyNeighbours(f); await ticks(f, 9); }
    if (mode === "expired") f.client.time = grant().expiresAtMs;
    if (mode === "cancelled") f.life.cancel();
    f.client.state.entities = [f.client.state.player, { objectId: 80, kind: "monster", name: "Scarecrow", x: 300, y: 639, hp: 10, disposition: "hostile" }];
    f.client.state.engagements = [{ attackerId: 80, targetId: 50001, observedAt: f.client.time }];
    const result = await f.actor.tick(); assert.equal(result.kind, "retreat"); assert.equal(result.reason, "life-return-home"); assert.equal(f.actions.at(-1)[0], "walk");
    assert.equal(f.actions.some(value => value[0] === "attack"), false);
  }
  const healing = resident(t); healing.client.time = grant().expiresAtMs; healing.client.state.playerHp = 10;
  assert.equal((await healing.actor.tick()).kind, "usePotion"); assert.deepEqual(healing.actions, [["potion"]]);
  const danger = resident(t); danger.life.cancel(); danger.client.state.playerHp = 4; danger.client.state.inventoryItems = [];
  assert.equal((await danger.actor.tick()).reason, "unsafe-withdraw"); assert.equal(danger.actions.at(-1)[0], "walk");
  const dead = resident(t); dead.client.time = grant().expiresAtMs; dead.client.state.playerHp = 0;
  assert.equal((await dead.actor.tick()).reason, "dead"); assert.deepEqual(dead.actions, []);
});

test("public Bichon map at the live stall coordinate can route and a goal-domain blockage does not block home", async t => {
  const f = resident(t, { mapLoader: () => loadClientCollision("0") });
  assert.equal((await f.actor.tick()).performed, true); assert.equal(f.actions[0][1], "UpLeft");
  f.client.state.player.x = 300; f.client.state.player.y = 638;
  const collision = await loadClientCollision("0");
  const occupants = [];
  for (let x = 285; x <= 287; x++) for (let y = 627; y <= 629; y++) if (!collision.blocked.has(`${x},${y}`)) occupants.push({ objectId: 200 + occupants.length, kind: "npc", name: "Observed", x, y, dead: false });
  f.client.state.entities = [f.client.state.player, ...occupants]; await ticks(f, 3);
  assert.equal(f.life.view().waypointIndex, 1); assert.equal(f.actions.length, 1);
  f.life.cancel(); const result = await f.actor.tick(); assert.equal(result.reason, "life-return-home"); assert.equal(result.performed, true); assert.equal(f.actions.at(-1)[1], "UpLeft");
});
