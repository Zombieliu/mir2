import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join, resolve, sep } from "node:path";
import { tmpdir } from "node:os";
import { CompanionController } from "../src/companion-controller.mjs";
import { SupplyController } from "../src/supply/supply-controller.mjs";
import { SupplyStore } from "../src/supply/supply-store.mjs";
import { loadClientCollision } from "../src/skills/navigation.mjs";
import { fixture, grant, childGrant, potion } from "./life-fixture.mjs";
import { producer, intent, receipt, goods } from "./supply-fixture.mjs";

const OFFSETS = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] };
const collision = async () => ({ width: 400, height: 700, blocked: new Set() });
const immutableEconomy = view => ({ expiresAtMs: view.expiresAtMs, budgetGold: view.budgetGold, spentGold: view.spentGold,
  childSupplyClaimed: view.childSupplyClaimed, purchasedQuantity: view.purchasedQuantity, modelDecisionsUsed: view.modelDecisionsUsed });

function resident(t, { life = true, mapLoader = collision } = {}) {
  const f = fixture(t, { path: ":memory:" }), actions = [], reports = [], responses = [];
  f.client.state.player.x = 300; f.client.state.player.y = 638; f.client.state.inSafeZone = false;
  f.client.state.party.members = ["Owner", "Alo"]; f.policy.permissions.mapAllowlist = ["0", "0141"];
  Object.assign(f.client, {
    async walk(direction) {
      const destination = { x: this.state.player.x + OFFSETS[direction][0], y: this.state.player.y + OFFSETS[direction][1] };
      actions.push({ kind: "walk", direction, destination });
      const response = responses.length ? responses.shift() : false;
      if (response instanceof Error) throw response;
      if (response === true) { Object.assign(this.state.player, destination); this.state.revision++; }
      return typeof response === "boolean" ? { moved: response } : response;
    },
    async useOwnedItem(uniqueId) { actions.push({ kind: "potion" }); this.state.inventoryItems[0].quantity--; this.state.revision++; return { success: true, uniqueId }; },
    async attackDirection() { actions.push({ kind: "attack" }); return { sent: true, confirmed: false }; },
  });
  const actor = new CompanionController({ client: f.client, store: f.store, leaderName: "Owner", homePosition: { ...grant().home, mapFileName: "0" },
    lifeController: life ? f.controller : null, clock: () => f.client.time, mapLoader, report: event => reports.push(event) });
  t.after(() => actor.close());
  if (life) f.controller.authorize(grant()); else actor.recallRequested = true;
  return { ...f, life: f.controller, actor, actions, reports, responses };
}

async function ticks(f, count) { for (let i = 0; i < count; i++) await f.actor.tick(); }

test("an acknowledged rejected life step routes around that observed tile on the next tick", async t => {
  const f = resident(t); f.responses.push(false, true);
  const first = await f.actor.tick(); assert.equal(first.moved, false); assert.equal(f.life.view().navigation.noPathCount, 1);
  const rejected = f.actions[0]; assert.deepEqual({ x: f.client.state.player.x, y: f.client.state.player.y }, { x: 300, y: 638 });
  const next = await f.actor.tick(); assert.equal(next.moved, true); assert.notEqual(f.actions[1].direction, rejected.direction);
  assert.notDeepEqual(f.actions[1].destination, rejected.destination); assert.equal(f.life.view().navigation.noPathCount, 0);
  assert.equal(f.reports.filter(event => event.type === "companionMovementRejected").length, 1);
});

test("the actual Bichon return corner never repeats a refused chokepoint and pauses if no alternate exists", async t => {
  const f = resident(t, { mapLoader: () => loadClientCollision("0") });
  Object.assign(f.client.state.player, { x: 290, y: 631 }); f.life.cancel(); f.responses.push(false, true);
  const first = await f.actor.tick(); assert.equal(first.reason, "life-return-home"); assert.equal(first.moved, false); assert.equal(f.actions[0].direction, "Down");
  const next = await f.actor.tick(); assert.equal(next.blocked, true); assert.equal(f.actions.length, 1);
  assert.equal(f.life.view().navigation.homeNoPathCount, 2); await f.actor.tick();
  assert.equal(f.life.view().status, "paused"); assert.equal(f.actions.length, 1); assert.equal(f.facts.length, 0);
});

test("a rejected tile expires after three seconds and can be reconsidered from the same live position", async t => {
  const f = resident(t, { life: false }); f.responses.push(false, {}); await f.actor.tick(); const direction = f.actions[0].direction;
  f.client.time = 2_999; await f.actor.tick(); assert.notEqual(f.actions[1].direction, direction);
  f.client.time = 3_000; await f.actor.tick(); assert.equal(f.actions[2].direction, direction);
});

test("rejected tiles never survive a world, character, connection, map or self-identity change", async t => {
  for (const change of [f => { f.client.worldEpoch = "world:replacement"; }, f => { f.client.activeCharacterIndex++; }, f => { f.client.state.connectionEpoch++; },
    f => { f.client.state.mapFileName = "0141"; }, f => { f.client.state.mapIndex++; }, f => { f.client.state.playerObjectId++; }, f => { f.client.state.selfZoneObjectId++; }]) {
    const f = resident(t, { life: false }); await f.actor.tick(); const first = f.actions[0].direction;
    change(f); f.actor.homePosition.mapFileName = f.client.state.mapFileName;
    await f.actor.tick(); assert.equal(f.actions[1].direction, first);
  }
  const f = resident(t, { life: false }); await f.actor.tick(); const first = f.actions[0].direction;
  f.client.state.ready = false; assert.equal((await f.actor.tick()).reason, "world-not-ready");
  f.client.state.ready = true; await f.actor.tick(); assert.equal(f.actions[1].direction, first);
});

test("timeouts, missing acknowledgements and sent-only success do not invent rejected tiles or progress", async t => {
  for (const response of [new Error("synthetic movement timeout"), {}, { moved: true }]) {
    const f = resident(t); f.responses.push(response, false);
    if (response instanceof Error) await assert.rejects(f.actor.tick(), /synthetic movement timeout/);
    else assert.equal((await f.actor.tick()).moved, false);
    assert.equal(f.life.view().navigation.noPathCount, 0);
    await f.actor.tick(); assert.equal(f.actions[1].direction, f.actions[0].direction); assert.equal(f.life.view().navigation.noPathCount, 1);
  }
});

test("a late refusal from a previous connection cannot quarantine a tile or increment the current life", async t => {
  const f = resident(t); let finish;
  f.client.walk = async direction => { f.actions.push({ direction }); return new Promise(resolveResult => { finish = resolveResult; }); };
  const pending = f.actor.tick(); await new Promise(resolve => setImmediate(resolve));
  f.client.state.connectionEpoch++; finish({ moved: false });
  assert.equal((await pending).moved, false); assert.equal(f.life.view().navigation.noPathCount, 0);
  assert.equal(f.reports.some(event => event.type === "companionMovementRejected"), false);
});

test("repeated authoritative refusals rotate the three fixed points then pause the failed return in finite ticks", async t => {
  const f = resident(t), before = immutableEconomy(f.life.view()); await ticks(f, 12);
  assert.equal(f.life.view().status, "paused"); assert.equal(f.life.view().navigation.homeNoPathCount, 3);
  assert.match(f.life.view().reason, /返家.*拒绝.*尚未到家/); assert.ok(f.actions.length <= 12);
  const count = f.actions.length; await ticks(f, 8); assert.equal(f.actions.length, count);
  assert.deepEqual(immutableEconomy(f.life.view()), before); assert.deepEqual(f.client.requests, []);
  assert.equal(f.facts.some(event => ["life-returned", "life-completed"].includes(event.kind)), false);
});

test("observed progress clears failure counts and rejected movement cannot displace medicine or recall", async t => {
  const f = resident(t); f.responses.push(false, true, false);
  await ticks(f, 2); assert.equal(f.life.view().navigation.noPathCount, 0);
  await ticks(f, 1); assert.equal(f.life.view().navigation.noPathCount, 1); const walks = f.actions.length;
  f.client.state.playerHp = 10; assert.equal((await f.actor.tick()).kind, "usePotion"); assert.equal(f.actions.length, walks + 1); assert.equal(f.actions.at(-1).kind, "potion");
  f.client.state.playerHp = 18; f.actor.recallRequested = true;
  assert.equal((await f.actor.tick()).reason, "owner-recall"); assert.equal(f.actions.at(-1).kind, "walk"); assert.equal(f.client.requests.length, 0);
});

function supplyResident(t, { path = ":memory:" } = {}) {
  const f = resident(t, { life: false }); f.actor.recallRequested = false; f.client.state.party.members = [];
  Object.assign(f.client.state.player, { x: 300, y: 630 }); f.client.state.equipmentItems = []; f.client.state.gold = 200;
  f.client.state.entities.push({ objectId: 8, kind: "npc", name: "Merchant_Ruben", x: 288, y: 608 });
  f.client.state.nativeNpcShop = { npcObjectId: 8, scriptKey: "BichonProvince/BorderVillage/Grocery", service: "BUYSELL", packetType: "NPCGoods", panelType: 0, list: [goods()] };
  const economyCalls = [], paid = new Map(); let lostReply = false;
  const frame = reply => { f.client.state.npcPurchaseAuthority = producer({ serverRevision: reply.receipt ? "2" : "1" });
    return { reply, authority: structuredClone(f.client.state.npcPurchaseAuthority), snapshot: f.client.snapshot() }; };
  Object.assign(f.client, {
    async interactNpc() { economyCalls.push({ kind: "interact" }); },
    async selectNpcBuyLink() { economyCalls.push({ kind: "link" }); },
    async beginNpcPurchase() { economyCalls.push({ kind: "begin" }); return frame({ kind: "producer", producer: producer() }); },
    async quoteNpcPurchase(request) { economyCalls.push({ kind: "quote" }); return frame({ kind: "quote", intent: intent({ request }) }); },
    async purchaseNpcItem(operation) {
      economyCalls.push({ kind: "purchase", operation: structuredClone(operation) });
      const key = JSON.stringify(operation); if (!paid.has(key)) { paid.set(key, receipt(operation)); f.client.state.gold -= 40; f.client.state.inventoryItems.push({ ...potion(1), uniqueId: 9007199254740993n }); }
      if (lostReply) throw new Error("synthetic lost purchase reply");
      return frame({ kind: "purchase", receipt: paid.get(key) });
    },
    async queryNpcPurchase(operation) { economyCalls.push({ kind: "query", operation: structuredClone(operation) }); return frame({ kind: "recovery", receipt: paid.get(JSON.stringify(operation)) ?? null }); },
  });
  const supply = new SupplyController({ client: f.client, dbPath: path, clock: () => f.client.time, stepTo: request => f.actor.stepForSupply(request) });
  t.after(() => supply.close()); f.actor.supplyController = supply; supply.authorize(childGrant());
  return { ...f, supply, economyCalls, setLostReply: value => { lostReply = value; } };
}

test("supply uses an alternate observed route then durably pauses after three failed steps without spending", async t => {
  const f = supplyResident(t); await ticks(f, 2);
  assert.notEqual(f.actions[1].direction, f.actions[0].direction);
  await ticks(f, 1); assert.equal(f.supply.view().status, "paused"); assert.equal(f.supply.view().step, "navigation-blocked");
  assert.equal(f.supply.view().acquiredQuantity, 0); assert.equal(f.supply.view().budgetGold, 50); assert.equal(f.client.state.gold, 200);
  const count = f.actions.length; await ticks(f, 4); assert.equal(f.actions.length, count); assert.deepEqual(f.economyCalls, []);
  f.client.state.playerHp = 10; assert.equal((await f.actor.tick()).kind, "usePotion"); assert.equal(f.actions.at(-1).kind, "potion");
  f.client.state.playerHp = 18; f.actor.recallRequested = true; assert.equal((await f.actor.tick()).reason, "owner-recall");
  assert.equal(f.economyCalls.length, 0);
});

test("a supply step that actually moves clears earlier rejection counts", async t => {
  const f = supplyResident(t); f.responses.push(false, true, false, true);
  await ticks(f, 4); assert.equal(f.supply.view().status, "running"); assert.equal(f.supply.view().acquiredQuantity, 0);
  assert.equal(f.economyCalls.length, 0); assert.equal(f.client.state.gold, 200);
});

test("pausing after a committed purchase preserves its exact receipt and never claims a return", async t => {
  const root = mkdtempSync(join(tmpdir(), "numeron-rejected-return-")), path = join(root, "supply.sqlite");
  let f, reopened;
  t.after(async () => { await f?.actor.close(); await f?.supply.close(); await reopened?.close(); assert.ok(resolve(root).startsWith(`${resolve(tmpdir())}${sep}`)); rmSync(root, { recursive: true, force: true }); });
  f = supplyResident(t, { path }); Object.assign(f.client.state.player, { x: 288, y: 609 });
  for (let i = 0; i < 8 && f.supply.view().status !== "returning"; i++) await f.actor.tick();
  assert.equal(f.supply.view().status, "returning"); const before = f.supply.view();
  await ticks(f, 3); assert.equal(f.supply.view().status, "paused"); assert.equal(f.supply.view().chargedGold, 40);
  assert.equal(f.supply.view().acquiredQuantity, 1); assert.equal(f.supply.view().receiptKnown, true); assert.equal(f.supply.view().returnedToAnchor, false);
  assert.equal(f.client.state.gold, 160); assert.equal(f.economyCalls.filter(call => call.kind === "purchase").length, 1);
  assert.equal(f.supply.view().expiresAtMs, before.expiresAtMs); const journal = new SupplyStore(path), saved = journal.current(); journal.close();
  assert.equal(saved.task.attempted, true); assert.equal(saved.task.receiptApplied, true); assert.equal(saved.task.cancelled, false);
  assert.deepEqual(saved.task.operation, f.economyCalls.find(call => call.kind === "purchase").operation);
  reopened = new SupplyController({ client: f.client, dbPath: path, clock: () => f.client.time }); t.after(() => reopened.close());
  assert.equal(reopened.view().status, "paused"); assert.equal(reopened.view().step, "navigation-blocked"); assert.equal(reopened.view().chargedGold, 40);
});

test("a navigation pause during an unknown purchase can only query the same operation and remains paused after proof", async t => {
  const f = supplyResident(t); Object.assign(f.client.state.player, { x: 288, y: 609 }); f.setLostReply(true);
  for (let i = 0; i < 8 && f.supply.view().status !== "unknown"; i++) await f.actor.tick();
  assert.equal(f.supply.view().status, "unknown"); const original = f.economyCalls.find(call => call.kind === "purchase").operation;
  f.supply.pauseNavigation(); assert.equal(f.supply.view().status, "unknown"); assert.equal(f.supply.view().step, "reconcile");
  await f.actor.tick(); assert.equal(f.supply.view().status, "paused"); assert.equal(f.supply.view().step, "navigation-blocked");
  const queries = f.economyCalls.filter(call => call.kind === "query"); assert.equal(queries.length, 1); assert.deepEqual(queries[0].operation, original);
  assert.equal(f.economyCalls.filter(call => call.kind === "purchase").length, 1); assert.equal(f.client.state.gold, 160);
  assert.equal(f.supply.view().chargedGold, 40); assert.equal(f.supply.view().acquiredQuantity, 1); assert.equal(f.supply.view().returnedToAnchor, false);
  const count = f.actions.length; await ticks(f, 5); assert.equal(f.actions.length, count);
});
