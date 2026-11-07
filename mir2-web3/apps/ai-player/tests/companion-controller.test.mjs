import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { CompanionController } from "../src/companion-controller.mjs";
import { CompanionStore } from "../src/memory/companion-store.mjs";

const POTION_UID = 18446744073709551615n;
const BINDING = { realmId: "controller-test", residentId: "companion", characterName: "Alo", characterIndex: 1 };

function scene() {
  const self = { objectId: 1, kind: "selfPlayer", name: "Alo", x: 10, y: 10, hp: 100, maxHp: 100, dead: false };
  return {
    ready: true, connectionEpoch: 1, revision: 1, mapFileName: "0", mapIndex: 0,
    playerObjectId: 1, selfZoneObjectId: 50_000, player: self, playerHp: 100, playerMaxHp: 100,
    inventoryStale: false, inventoryItems: [], beltItems: [], freeBagSlots: 20, currentWeight: 0, maxWeight: 100,
    entities: [self, { objectId: 2, kind: "player", name: "Owner", x: 11, y: 10, hp: 90, maxHp: 100 },
      { objectId: 10, kind: "monster", name: "Deer", x: 11, y: 11, hp: 20, dead: false, disposition: "neutral" }],
    party: { allowGroup: true, leaderName: "Owner", members: ["Owner", "Alo"], pendingInvite: null },
    engagements: [{ attackerId: 2, targetId: 10, observedAt: 9000 }],
    groundDrops: [{ objectId: 50, name: "Candle", x: 10, y: 10, ownerObjectId: 50_000, ownershipRemainingTicks: 100 }],
  };
}

class StubClient {
  constructor(state = scene()) { this.state = state; this.calls = []; }
  snapshot() { return structuredClone(this.state); }
  async walk(direction) { this.calls.push({ type: "walk", direction }); return { moved: false }; }
  async attackDirection(direction) { this.calls.push({ type: "attackDirection", direction }); return { sent: true, confirmed: false }; }
  async pickUpTile() { this.calls.push({ type: "pickUpTile" }); return { changed: false }; }
  async useOwnedItem(uniqueId, grid) { this.calls.push({ type: "useOwnedItem", uniqueId, grid }); return { success: true, uniqueId }; }
  async acceptGroupInvite() { this.calls.push({ type: "acceptGroupInvite" }); return this.state.party; }
}

function fixture(t, { state = scene(), report = () => {} } = {}) {
  const root = mkdtempSync(join(tmpdir(), "mir2-companion-controller-"));
  const path = join(root, "memory.sqlite");
  const stores = [];
  t.after(() => {
    for (const store of stores) store.close();
    const absolute = resolve(root);
    assert.ok(absolute.startsWith(`${resolve(tmpdir())}${sep}`) && basename(absolute).startsWith("mir2-companion-controller-"));
    rmSync(absolute, { recursive: true, force: true });
  });
  const client = new StubClient(state);
  const time = { now: 10_000 };
  const open = () => {
    const store = new CompanionStore({ path, binding: BINDING });
    stores.push(store);
    return store;
  };
  const controller = store => new CompanionController({ client, store, leaderName: "Owner", homePosition: { x: 10, y: 10, mapFileName: "0" }, clock: () => time.now, report,
    mapLoader: async () => ({ width: 30, height: 30, blocked: new Set() }) });
  const store = open();
  return { client, time, store, open, controller, actor: controller(store) };
}

function carryingPotion(state = scene(), quantity = 2) {
  state.playerHp = 60;
  state.inventoryItems = [{ name: "(HP)DrugSmall", quantity, uniqueId: POTION_UID, container: "bag1" }];
  return state;
}

function attachSupply(f, { tick, state = {} } = {}) {
  const status = { status: "running", permitId: "supply-test-permit", expiresAtMs: f.time.now + 100_000, acquiredQuantity: 0, ...state }, calls = [];
  f.actor.supplyController = {
    view: () => structuredClone(status),
    async tick(snapshot) { calls.push({ kind: "tick", snapshot }); return tick ? await tick(snapshot) : { performed: true, status: status.status }; },
    cancel(reason) { calls.push({ kind: "cancel", reason }); status.status = "cancelled"; },
  };
  return { status, calls };
}

test("authorized supply has one action lane without an owner or party and supersedes normal loot and models", async t => {
  const f = fixture(t); f.client.state.party.members = []; f.client.state.engagements = []; f.client.state.entities = [];
  let modelDecisions = 0; f.actor.modelDriver = { decide({ baseAction }) { modelDecisions++; return baseAction; } };
  const supply = attachSupply(f);
  assert.equal((await f.actor.tick()).kind, "supply"); assert.equal(supply.calls.length, 1);
  assert.equal(f.client.calls.length, 0); assert.equal(modelDecisions, 0);
  assert.equal(f.store.growth().episodes.some(row => row.payload?.kind === "supply-acquired"), false, "selecting supply is not a purchase fact");
  supply.status.status = "completed"; await f.actor.tick();
  assert.equal(f.client.calls.at(-1).type, "pickUpTile"); assert.equal(supply.calls.length, 1);
});

test("supply cannot bypass unknown exact-UID medicine or its confirmed recovery window", async t => {
  const f = fixture(t, { state: carryingPotion() }); const supply = attachSupply(f);
  f.client.useOwnedItem = async uniqueId => { f.client.calls.push({ type: "useOwnedItem", uniqueId }); throw new Error("synthetic medicine uncertainty"); };
  await assert.rejects(f.actor.tick(), /medicine uncertainty/); assert.equal(supply.calls.length, 0);
  f.client.state.playerHp = 100; f.time.now += 10_000;
  assert.equal((await f.actor.tick()).reason, "supply-waits-for-potion-reconciliation");
  assert.equal(supply.calls.length, 0); assert.equal(f.client.calls.length, 1);
  f.client.state.lastItemUse = { uniqueId: POTION_UID, grid: "inventory", observedAt: f.time.now, success: true };
  f.client.state.inventoryItems[0].quantity--; f.client.state.revision++;
  assert.equal((await f.actor.tick()).reason, "supply-waits-for-potion-reconciliation");
  f.time.now += 4_000;
  assert.equal((await f.actor.tick()).kind, "supply"); assert.equal(supply.calls.length, 1);
  assert.equal(f.client.calls.filter(call => call.type === "useOwnedItem").length, 1);
});

test("survival retreat, recall and genuine self-defence preempt supply; death cancels its remaining activity", async t => {
  const f = fixture(t); const supply = attachSupply(f); f.client.state.player.x = 16; f.client.state.playerHp = 30;
  assert.equal((await f.actor.tick()).reason, "unsafe-withdraw"); assert.equal(supply.calls.length, 0);
  f.client.state.playerHp = 100; f.client.state.inSafeZone = true; f.actor.recallRequested = true;
  assert.equal((await f.actor.tick()).reason, "owner-recall"); assert.equal(supply.calls.length, 0);
  f.actor.recallRequested = false; f.client.state.inSafeZone = false;
  const monster = f.client.state.entities.find(entity => entity.kind === "monster"); monster.x = 15; monster.y = 10;
  f.client.state.engagements = [{ attackerId: monster.objectId, targetId: 50_000, observedAt: f.time.now }];
  assert.equal((await f.actor.tick()).reason, "defend-confirmed-engagement"); assert.equal(f.client.calls.at(-1).type, "attackDirection");
  assert.equal(supply.calls.length, 0);
  f.client.state.playerHp = 0; f.client.state.player.dead = true;
  assert.equal((await f.actor.tick()).reason, "dead"); assert.equal(supply.calls.at(-1).kind, "cancel");
  assert.equal(supply.status.status, "cancelled");
});

test("supply movement revalidates safety, permit, policy and world after awaited collision loading", async t => {
  for (const change of [
    f => { f.client.state.playerHp = 60; f.client.state.inventoryItems = carryingPotion().inventoryItems; },
    f => { f.actor.recallRequested = true; },
    (f, supply) => { supply.status.status = "cancelled"; },
    f => { f.client.state.connectionEpoch++; },
    f => { f.store.teachSupportBeforeLoot({ feedbackId: "new-during-supply" }); },
  ]) {
    const f = fixture(t); let resolveMap;
    f.actor.mapLoader = () => new Promise(resolveCollision => { resolveMap = resolveCollision; });
    const supply = attachSupply(f, { tick: snapshot => f.actor.stepForSupply({ snapshot, target: { x: 16, y: 10 }, desiredDistance: 1, bounds: { minX: 0, minY: 0, maxX: 29, maxY: 29 } }) });
    const pending = f.actor.tick(); change(f, supply); resolveMap({ width: 30, height: 30, blocked: new Set() });
    assert.equal((await pending).performed, false); assert.equal(f.client.calls.length, 0);
  }
});

test("closing waits for the active supply tick and forbids new normal actions", async t => {
  const f = fixture(t); let finishTick;
  const supply = attachSupply(f, { tick: () => new Promise(resolveTick => { finishTick = resolveTick; }) });
  const running = f.actor.tick(); let closed = false; const closing = f.actor.close().then(() => { closed = true; });
  await Promise.resolve(); assert.equal(closed, false);
  assert.equal((await f.actor.tick()).reason, "controller-closed"); assert.equal(supply.calls.length, 1);
  finishTick({ performed: false, status: "paused" }); await running; await closing;
  assert.equal(closed, true); assert.equal(f.client.calls.length, 0);
});

test("unsafe withdrawal continues after recovery and loss of the leader until authoritative safety", async t => {
  const state = scene();
  state.player.x = 16;
  state.playerHp = 30;
  state.inSafeZone = false;
  const f = fixture(t, { state });
  assert.equal((await f.actor.tick()).reason, "unsafe-withdraw");
  f.client.state.playerHp = 100;
  f.client.state.entities = f.client.state.entities.filter(value => value.kind !== "player");
  f.client.state.engagements = [];
  f.client.state.player.x = 15;
  f.time.now += 10_000;
  const continuing = await f.actor.tick();
  assert.equal(continuing.reason, "continue-unsafe-withdraw");
  assert.deepEqual(continuing.target, { x: 10, y: 10 });
  assert.equal(f.client.calls.at(-1).type, "walk");
  f.actor.recallRequested = true;
  assert.equal((await f.actor.tick()).reason, "owner-recall");
  f.actor.recallRequested = false;
  assert.equal((await f.actor.tick()).reason, "continue-unsafe-withdraw");
  f.client.state.inSafeZone = true;
  assert.equal((await f.actor.tick()).reason, "leader-not-visible");
});

test("withdrawal permits medicine, finishes at the observed home and does not resume from a stale world", async t => {
  const state = scene();
  state.player.x = 16;
  state.playerHp = 30;
  const f = fixture(t, { state });
  assert.equal((await f.actor.tick()).reason, "unsafe-withdraw");
  f.client.state.inventoryItems = [{ name: "(HP)DrugSmall", quantity: 1, uniqueId: POTION_UID, container: "bag1" }];
  assert.equal((await f.actor.tick()).kind, "usePotion");
  f.client.state.playerHp = 100;
  assert.equal((await f.actor.tick()).reason, "continue-unsafe-withdraw");
  f.client.state.player.x = 10;
  assert.notEqual((await f.actor.tick()).reason, "continue-unsafe-withdraw");
  for (const change of [
    current => current.connectionEpoch++,
    current => current.playerObjectId++,
    current => current.selfZoneObjectId++,
    current => current.mapIndex++,
    current => { current.mapFileName = "0141"; },
    current => { current.player.dead = true; current.playerHp = 0; },
  ]) {
    Object.assign(f.client.state, scene(), { inventoryItems: [], inSafeZone: false, playerHp: 30 });
    f.client.state.player.x = 16;
    f.time.now += 10_000;
    assert.equal((await f.actor.tick()).reason, "unsafe-withdraw");
    change(f.client.state);
    f.client.state.playerHp = f.client.state.player.dead ? 0 : 100;
    assert.notEqual((await f.actor.tick()).reason, "continue-unsafe-withdraw");
  }
});

test("loading collision data cannot send an old direction into a newer world", async t => {
  const state = scene(); state.player.x = 16; state.playerHp = 30;
  const f = fixture(t, { state });
  let finishMap;
  const actor = new CompanionController({ client: f.client, store: f.store, leaderName: "Owner", clock: () => f.time.now,
    homePosition: { x: 10, y: 10, mapFileName: "0" }, mapLoader: () => new Promise(resolveMap => { finishMap = resolveMap; }) });
  const pending = actor.tick();
  assert.equal(typeof finishMap, "function");
  f.client.state.connectionEpoch++;
  f.client.state.mapFileName = "0141";
  f.client.state.playerHp = 100;
  finishMap({ width: 30, height: 30, blocked: new Set() });
  assert.equal((await pending).blocked, true);
  assert.deepEqual(f.client.calls, []);
});

test("a walk uses the latest observed position after loading collision data", async t => {
  const state = scene(); state.player.x = 16; state.playerHp = 30;
  const f = fixture(t, { state });
  let finishMap;
  const actor = new CompanionController({ client: f.client, store: f.store, leaderName: "Owner", clock: () => f.time.now,
    homePosition: { x: 10, y: 10, mapFileName: "0" }, mapLoader: () => new Promise(resolveMap => { finishMap = resolveMap; }) });
  const pending = actor.tick();
  f.client.state.player.x = 9;
  f.client.state.playerHp = 100;
  finishMap({ width: 30, height: 30, blocked: new Set() });
  assert.equal((await pending).performed, true);
  assert.deepEqual(f.client.calls, [{ type: "walk", direction: "Right" }]);
});

test("real controller sends pickup before teaching and support attack after SQLite-backed teaching/restart", async t => {
  const f = fixture(t);
  const initialFacts = f.client.snapshot();
  const before = await f.actor.tick();
  assert.equal(before.kind, "loot");
  assert.deepEqual(f.client.calls, [{ type: "pickUpTile" }]);
  const taught = f.actor.teachSupportBeforeLoot("lesson:one");
  assert.equal(taught.policyVersion, 2);
  const supported = await f.actor.tick();
  assert.equal(supported.kind, "attack");
  assert.deepEqual(f.client.calls[1], { type: "attackDirection", direction: "DownRight" });
  assert.deepEqual(f.client.snapshot(), initialFacts, "teaching must not change server facts to manufacture the choice");
  f.store.close();
  const reopened = f.open();
  const restarted = f.controller(reopened);
  await restarted.tick();
  assert.deepEqual(f.client.calls[2], { type: "attackDirection", direction: "DownRight" });
  assert.equal(reopened.profile().policyVersion, 2);
  assert.deepEqual(reopened.profile().solo, { considerLootWhenSafe: true });
  assert.deepEqual(reopened.profile().permissions, { spendLimit: 0, mapAllowlist: ["0", "0141"] });
  f.client.state.party.members = [];
  f.client.state.engagements = [];
  assert.equal((await restarted.tick()).kind, "loot");
  assert.deepEqual(f.client.calls.at(-1), { type: "pickUpTile" });
});

test("accepted potion waits four seconds after the actual receipt, even if the request itself was slow", async t => {
  const f = fixture(t, { state: carryingPotion() });
  f.client.useOwnedItem = async (uniqueId, grid) => {
    f.client.calls.push({ type: "useOwnedItem", uniqueId, grid });
    f.time.now += 2000;
    f.client.state.inventoryItems[0].quantity -= 1;
    f.client.state.revision += 1;
    return { success: true, uniqueId };
  };
  assert.equal((await f.actor.tick()).kind, "usePotion");
  assert.equal(f.time.now, 12_000);
  f.time.now = 15_999;
  assert.notEqual((await f.actor.tick()).kind, "usePotion");
  assert.equal(f.client.calls.filter(call => call.type === "useOwnedItem").length, 1);
  f.time.now = 16_000;
  assert.equal((await f.actor.tick()).kind, "usePotion");
  assert.equal(f.client.calls.filter(call => call.type === "useOwnedItem").length, 2);
  assert.equal(f.store.recentEpisodes().find(event => event.kind === "item-use").outcome, "accepted");
});

test("rejected potion UID backs off for ten seconds after rejection without blocking another owned medicine", async t => {
  const f = fixture(t, { state: carryingPotion() });
  f.client.useOwnedItem = async (uniqueId, grid) => {
    f.client.calls.push({ type: "useOwnedItem", uniqueId, grid });
    f.time.now += 3000;
    return { success: false, uniqueId };
  };
  assert.equal((await f.actor.tick()).confirmed, false);
  f.time.now = 22_999;
  assert.notEqual((await f.actor.tick()).kind, "usePotion");
  assert.equal(f.client.calls.filter(call => call.type === "useOwnedItem").length, 1);
  f.time.now = 23_000;
  await f.actor.tick();
  assert.equal(f.client.calls.filter(call => call.type === "useOwnedItem").length, 2);
  f.client.state.beltItems = [{ name: "(HP)DrugSmall", uniqueId: 99, quantity: 1, container: "belt" }];
  await f.actor.tick();
  assert.deepEqual(f.client.calls.at(-1), { type: "useOwnedItem", uniqueId: 99, grid: "belt" });
});

test("a timed-out item use is not replayed until a newer authoritative inventory reconciles it", async t => {
  const f = fixture(t, { state: carryingPotion() });
  let attempts = 0;
  f.client.useOwnedItem = async (uniqueId, grid) => {
    f.client.calls.push({ type: "useOwnedItem", uniqueId, grid });
    if (++attempts === 1) throw new Error("gateway result unknown");
    return { success: true, uniqueId };
  };
  await assert.rejects(f.actor.tick(), /result unknown/);
  f.time.now = 30_000;
  assert.notEqual((await f.actor.tick()).kind, "usePotion");
  f.client.state.revision += 1;
  assert.notEqual((await f.actor.tick()).kind, "usePotion", "new revision with unchanged stock is not proof of the old result");
  assert.equal(attempts, 1);
  f.client.state.revision += 1;
  f.client.state.inventoryItems[0].quantity = 1;
  f.time.now = 31_000;
  assert.equal((await f.actor.tick()).kind, "usePotion");
  assert.equal(attempts, 2);
  assert.deepEqual(f.store.recentEpisodes(100).filter(event => event.kind === "item-use").map(event => event.outcome), ["accepted", "uncertain"]);
});

test("a success receipt for another UID cannot resolve the attempted potion or authorize a replay", async t => {
  const f = fixture(t, { state: carryingPotion() });
  f.client.useOwnedItem = async (uniqueId, grid) => {
    f.client.calls.push({ type: "useOwnedItem", uniqueId, grid });
    return { success: true, uniqueId: 99 };
  };
  await assert.rejects(f.actor.tick(), /matching authoritative receipt/);
  f.time.now = 30_000;
  assert.notEqual((await f.actor.tick()).kind, "usePotion");
  assert.equal(f.client.calls.filter(call => call.type === "useOwnedItem").length, 1);
  assert.equal(f.store.recentEpisodes(100).find(event => event.kind === "item-use").outcome, "uncertain");
});

test("only the current observed configured-leader invitation can be accepted", async t => {
  const state = scene();
  state.party.members = [];
  state.engagements = [];
  state.groundDrops = [];
  const f = fixture(t, { state });
  for (const pendingInvite of [null, { name: "Stranger", observedAt: 9000 }, { name: "Owner", observedAt: -30_000 }, { name: "Owner", observedAt: 11_000 }]) {
    f.client.state.party.pendingInvite = pendingInvite;
    await f.actor.tick();
  }
  assert.deepEqual(f.client.calls, []);
  f.client.state.party.pendingInvite = { name: "Owner", observedAt: 9000 };
  assert.equal((await f.actor.tick()).kind, "acceptInvite");
  assert.deepEqual(f.client.calls, [{ type: "acceptGroupInvite" }]);
});

test("an invite replaced between decision and execution is not accepted on the old leader's authority", async t => {
  let client;
  const state = scene();
  state.party.members = [];
  state.party.pendingInvite = { name: "Owner", observedAt: 9000 };
  const f = fixture(t, { state, report: event => {
    if (event.type === "companionDecision") client.state.party.pendingInvite = { name: "Stranger", observedAt: 10_000 };
  } });
  client = f.client;
  const result = await f.actor.tick();
  assert.equal(result.blocked, true);
  assert.deepEqual(client.calls, []);
});

test("unknown invitation acceptance and tile pickup are not blindly replayed", async t => {
  const invitation = scene();
  invitation.party.members = [];
  invitation.party.pendingInvite = { name: "Owner", observedAt: 9000 };
  const f = fixture(t, { state: invitation });
  f.client.acceptGroupInvite = async () => { f.client.calls.push({ type: "acceptGroupInvite" }); throw new Error("unknown invite result"); };
  await assert.rejects(f.actor.tick(), /unknown invite/);
  f.time.now += 1;
  assert.equal((await f.actor.tick()).blocked, true);
  assert.equal(f.client.calls.length, 1);

  const loot = fixture(t);
  let count = 0;
  loot.client.pickUpTile = async () => {
    loot.client.calls.push({ type: "pickUpTile" });
    if (++count === 1) throw new Error("unknown pickup result");
    return { changed: false };
  };
  await assert.rejects(loot.actor.tick(), /unknown pickup/);
  await loot.actor.tick();
  assert.equal(count, 1);
  assert.equal(loot.store.recentEpisodes(100).find(event => event.kind === "inventory-change").outcome, "uncertain");
});

test("an unconfirmed no-change pickup receipt quarantines the drop without stopping the controller", async t => {
  const f = fixture(t);
  let pickups = 0;
  f.client.pickUpTile = async () => {
    f.client.calls.push({ type: "pickUpTile" });
    return ++pickups === 1 ? { changed: false, confirmed: false } : { changed: true };
  };
  const first = await f.actor.tick();
  assert.equal(first.kind, "loot");
  assert.equal(first.performed, true);
  assert.equal(first.confirmed, false);
  assert.equal(first.uncertain, true);
  assert.equal(f.store.recentEpisodes(100).find(event => event.kind === "inventory-change").outcome, "uncertain");
  await f.actor.tick();
  assert.equal(pickups, 1);
  // A newer inventory snapshot does not settle this still-visible ground object.
  f.client.state.revision += 1;
  f.client.state.inventoryItems.push({ uniqueId: 99, name: "Another item", quantity: 1, container: "bag1" });
  await f.actor.tick();
  assert.equal(pickups, 1);
  assert.equal(f.store.recentEpisodes(100).filter(event => event.kind === "inventory-change").length, 1);
  // A full AOI disappearance ends this object's observed lifecycle.
  const drop = f.client.state.groundDrops[0];
  f.client.state.groundDrops = [];
  await f.actor.tick();
  f.client.state.groundDrops = [drop];
  const next = await f.actor.tick();
  assert.equal(next.kind, "loot");
  assert.equal(next.changed, true);
  assert.equal(pickups, 2);
  assert.equal(f.store.recentEpisodes(100).find(event => event.kind === "inventory-change").outcome, "observed");
});

test("attack re-reads the moved monster and sends its new direction while a player occupies the old tile", async t => {
  let client;
  const f = fixture(t, { report: event => {
    if (event.type !== "companionDecision" || event.kind !== "attack") return;
    const target = client.state.entities.find(value => value.objectId === 10);
    target.x = 9; target.y = 10;
    client.state.entities.push({ objectId: 3, kind: "player", name: "Bystander", x: 11, y: 11, hp: 100 });
  } });
  client = f.client;
  f.actor.teachSupportBeforeLoot("lesson:moved-target");
  const result = await f.actor.tick();
  assert.equal(result.kind, "attack");
  assert.deepEqual(client.calls, [{ type: "attackDirection", direction: "Left" }]);
});

test("attack direction also uses the latest self position", async t => {
  let client;
  const f = fixture(t, { report: event => {
    if (event.type !== "companionDecision" || event.kind !== "attack") return;
    client.state.player.y = 11;
    client.state.entities.find(value => value.objectId === 1).y = 11;
  } });
  client = f.client;
  f.actor.teachSupportBeforeLoot("lesson:moved-self");
  await f.actor.tick();
  assert.deepEqual(client.calls, [{ type: "attackDirection", direction: "Right" }]);
});

test("an attack invalidated after selection cannot hit an absent, distant, friendly, or player target", async t => {
  const changes = [
    state => { state.entities = state.entities.filter(value => value.objectId !== 10); },
    state => { state.entities.find(value => value.objectId === 10).dead = true; },
    state => { state.entities.find(value => value.objectId === 10).hp = 0; },
    state => { state.entities.find(value => value.objectId === 10).kind = "player"; },
    state => { state.entities.find(value => value.objectId === 10).disposition = "friendly"; },
    state => { state.entities.find(value => value.objectId === 10).x = 14; },
    state => { state.entities.push({ objectId: 3, kind: "player", name: "Bystander", x: 11, y: 11, hp: 100 }); },
    state => { state.connectionEpoch += 1; },
    state => { state.mapFileName = "0141"; },
    state => { state.playerHp = 0; state.player.dead = true; },
  ];
  for (const [index, change] of changes.entries()) {
    let client;
    const f = fixture(t, { report: event => { if (event.type === "companionDecision" && event.kind === "attack") change(client.state); } });
    client = f.client;
    f.actor.teachSupportBeforeLoot(`lesson:invalid-${index}`);
    const result = await f.actor.tick();
    assert.equal(result.performed, false, `case ${index}`);
    assert.equal(result.blocked, true, `case ${index}`);
    assert.equal(client.calls.some(call => call.type === "attackDirection"), false, `case ${index}`);
  }
});

test("an action still awaiting its receipt cannot start a second overlapping item use", async t => {
  const f = fixture(t, { state: carryingPotion() });
  let resolve;
  f.client.useOwnedItem = (uniqueId, grid) => {
    f.client.calls.push({ type: "useOwnedItem", uniqueId, grid });
    return new Promise(done => { resolve = done; });
  };
  const pending = f.actor.tick();
  assert.deepEqual(await f.actor.tick(), { kind: "wait", reason: "previous-action-in-flight" });
  assert.equal(f.client.calls.length, 1);
  resolve({ success: true, uniqueId: POTION_UID });
  assert.equal((await pending).kind, "usePotion");
});
