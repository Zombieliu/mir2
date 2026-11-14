import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, mkdtempSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { SupplyController, validateSupplyGrant } from "../src/supply/supply-controller.mjs";
import { grant, world, producer, intent, receipt, operation, ownerFrame, shop, goods } from "./supply-fixture.mjs";

const ROOT = join(tmpdir(), "numeron-supply-tests");
mkdirSync(ROOT, { recursive: true });
const database = () => join(mkdtempSync(`${ROOT}/run-`), "supply.sqlite");
const position = state => state.entities.find(e => e.kind === "selfPlayer");

class Client {
  worldEpoch = "epoch:test"; activeCharacterIndex = 2; calls = []; committed = new Map(); mode = "normal"; quotePrice = 40; clock = 0;
  constructor(changes = {}) { this.raw = world(changes); }
  snapshot() {
    return structuredClone({ ...this.raw, ready: true, connectionEpoch: 1, revision: this.calls.length, player: position(this.raw), inventoryStale: false,
      nativeNpcShop: this.raw.nativeNpcShop ? { ...this.raw.nativeNpcShop, list: this.raw.nativeNpcShop.displayGoods } : null,
      npcPurchaseAuthority: this.authority ?? null, party: { members: [] } });
  }
  apply(frame) { if (frame.snapshot) { this.raw = structuredClone(frame.snapshot); this.authority = structuredClone(frame.authority); } return frame; }
  async interactNpc(id) { this.calls.push(["interact", id]); this.raw.activeNpcDialog = world().activeNpcDialog; }
  async selectNpcBuyLink() {
    this.calls.push(["link"]);
    this.raw.activeNpcDialog = null;
    this.raw.nativeNpcShop = shop({ displayGoods: [goods({ price: this.quotePrice })] });
  }
  async beginNpcPurchase() { this.calls.push(["begin"]); return this.apply(ownerFrame({ requestId: "1" }, { kind: "producer", producer: producer() }, this.raw, producer())); }
  async quoteNpcPurchase(request) { this.calls.push(["quote", structuredClone(request)]); return this.apply(ownerFrame({ requestId: "2" }, { kind: "quote", intent: intent({ request }) }, this.raw, producer())); }
  commit(op) {
    const key = JSON.stringify(op), paid = receipt(op);
    if (!this.committed.has(key)) {
      this.raw.gold -= 40;
      this.raw.inventoryItems = [{ key: "hp", name: "(HP)DrugSmall", uniqueId: 9007199254740993n, quantity: 1, container: "bag1", slot: 0 }];
      this.committed.set(key, paid);
    }
    return this.committed.get(key);
  }
  async purchaseNpcItem(op) {
    this.calls.push(["purchase", structuredClone(op)]);
    if (this.mode === "timeout-before-commit") throw new Error("timeout");
    const paid = this.commit(op);
    if (this.mode === "timeout-after-commit") throw new Error("timeout");
    if (this.mode === "over-budget") paid.entry.outcome.committed.charged = 51;
    if (this.mode === "wrong-operation") paid.entry.operation.sequence = "999";
    if (this.mode === "rejected") paid.entry.outcome = { rejected: { request: op.intent.request, reason: "invalidDelivery" } };
    const noPair = this.mode === "no-pair";
    const frame = ownerFrame({ requestId: "3" }, { kind: "purchase", receipt: paid, replayed: false }, noPair ? null : this.raw, noPair ? null : producer({ serverRevision: "2" }));
    if (this.mode === "hold") return await new Promise(resolve => { this.release = () => resolve(this.apply(frame)); });
    return this.apply(frame);
  }
  async queryNpcPurchase(op) {
    this.calls.push(["query", structuredClone(op)]);
    return this.apply(ownerFrame({ requestId: "4" }, { kind: "recovery", receipt: this.committed.get(JSON.stringify(op)) ?? null }, this.raw, producer({ serverRevision: "2" })));
  }
  async walk(direction) {
    this.calls.push(["walk", direction]);
    const [dx, dy] = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] }[direction];
    position(this.raw).x += dx; position(this.raw).y += dy; return { moved: true };
  }
}

function subject(t, { path = database(), client = new Client(), clock = () => client.clock, onObserved, mapLoader = async () => ({ width: 800, height: 800, blocked: new Set(["288,608"]) }) } = {}) {
  const controller = new SupplyController({ client, dbPath: path, clock, onObserved, mapLoader });
  t.after(() => controller.close()); return { controller, client, path };
}
async function until(f, wanted, limit = 25) {
  for (let i = 0; i < limit && !wanted(f.controller.view()); i++) { await f.controller.tick(); f.client.clock += 1000; }
  assert.equal(wanted(f.controller.view()), true, JSON.stringify(f.controller.view()));
}

test("only a strict current actor-bound one-potion Bichon grant is executable", t => {
  const f = subject(t);
  for (const update of [{ mapFileName: "0141" }, { quantity: 2 }, { maxGold: 51 }, { itemIndex: 659 }, { npcName: "Other" }, { bounds: { minX: 0, minY: 0, maxX: 999, maxY: 999 } }, { expiresAtMs: 300_001 }, { actorCharacterIndex: 3 }, { worldEpoch: "epoch:other" }]) assert.throws(() => f.controller.authorize(grant(update)));
  assert.throws(() => validateSupplyGrant({ ...grant(), permissions: { sell: true } }));
  f.controller.authorize(grant());
  assert.equal(f.controller.view().status, "authorized"); assert.equal(f.client.calls.length, 0);
});

test("ordinary independent supply completes only after actual receipt, full snapshot, and return to its anchor", async t => {
  const observed = [], f = subject(t, { onObserved: e => observed.push(e) });
  f.controller.authorize(grant());
  await until(f, v => v.status === "completed");
  const view = f.controller.view();
  assert.equal(view.chargedGold, 40); assert.equal(view.acquiredQuantity, 1); assert.equal(view.receiptKnown, true); assert.equal(view.returnedToAnchor, true);
  const actual = position(f.client.raw);
  assert.equal(Math.max(Math.abs(actual.x - 288), Math.abs(actual.y - 616)), 1);
  assert.equal(f.client.calls.filter(c => c[0] === "purchase").length, 1);
  assert.deepEqual(observed.map(e => e.kind), ["supply-acquired", "supply-returned"]);
  assert.deepEqual(observed[1].actualPosition, { x: actual.x, y: actual.y });
  assert.deepEqual(observed[1].returnAnchor, grant().returnAnchor);
  assert.equal(observed[1].arrivalDistance, 1);
  assert.equal(f.client.snapshot().party.members.length, 0, "supply does not invent an owner or party visibility requirement");
  assert.equal(f.client.calls.some(c => ["pickUp", "sell", "trade", "teleport"].includes(c[0])), false);
});

test("opening the actual Buy page from a cold shop can purchase with a null dialogue", async t => {
  const client = new Client({ activeNpcDialog: null, nativeNpcShop: null }), f = subject(t, { client });
  f.controller.authorize(grant());
  await until(f, v => v.step === "ready-to-purchase");
  assert.equal(client.snapshot().activeNpcDialog, null);
  assert.equal(client.snapshot().nativeNpcShop.npcObjectId, 8);
  assert.deepEqual(client.calls.slice(0, 4).map(call => call[0]), ["interact", "link", "begin", "quote"]);
  await until(f, v => v.status === "completed");
  assert.equal(client.calls.filter(call => call[0] === "purchase").length, 1);
  assert.equal(client.raw.gold, 160);
});

test("the Buy page remains valid when its dialogue still names the same merchant", async t => {
  const client = new Client(), select = client.selectNpcBuyLink.bind(client), f = subject(t, { client });
  client.selectNpcBuyLink = async () => { await select(); client.raw.activeNpcDialog = world().activeNpcDialog; };
  f.controller.authorize(grant()); await until(f, v => v.status === "completed");
  assert.equal(client.calls.filter(call => call[0] === "purchase").length, 1);
});

const invalidCurrentShops = [
  ["other merchant object", state => { state.nativeNpcShop.npcObjectId = 9; }],
  ["other merchant script", state => { state.nativeNpcShop.scriptKey = "BichonProvince/BorderVillage/Weapon"; }],
  ["other service", state => { state.nativeNpcShop.service = "BUYBACK"; }],
  ["other currency panel", state => { state.nativeNpcShop.packetType = "NPCPearlGoods"; }],
  ["other shop panel", state => { state.nativeNpcShop.panelType = 1; }],
  ["other merchant name", state => { state.entities.find(e => e.kind === "npc").name = "OtherMerchant"; }],
  ["other merchant position", state => { state.entities.find(e => e.kind === "npc").x += 1; }],
  ["merchant outside interaction distance", state => { position(state).y = 610; }],
  ["merchant is not currently observed", state => { state.entities = state.entities.filter(e => e.kind !== "npc"); }],
  ["conflicting dialogue", state => { state.activeNpcDialog = { npcObjectId: 9 }; }],
  ["closed shop", state => { state.nativeNpcShop = null; }],
];

test("a null dialogue never permits a mismatched, distant or closed current shop to quote", async t => {
  for (const [name, invalidate] of invalidCurrentShops) await t.test(name, async sub => {
    const f = subject(sub); f.controller.authorize(grant()); await until(f, v => v.step === "quote");
    // A previously valid tick input must not override the freshest shop view.
    const previous = f.client.snapshot(); invalidate(f.client.raw);
    await f.controller.tick(previous);
    assert.equal(f.controller.view().status, "failed");
    assert.equal(f.client.calls.some(call => ["quote", "purchase"].includes(call[0])), false);
    assert.equal(f.client.raw.gold, 200); assert.deepEqual(f.client.raw.inventoryItems, []);
  });
});

test("shop binding is rechecked after the quote and immediately before purchase", async t => {
  for (const phase of ["after-quote", "before-purchase"]) {
    for (const [name, invalidate] of invalidCurrentShops.filter(([name]) => ["other merchant object", "other merchant position", "closed shop"].includes(name))) await t.test(`${phase}: ${name}`, async sub => {
      const f = subject(sub); f.controller.authorize(grant());
      if (phase === "after-quote") {
        const quote = f.client.quoteNpcPurchase.bind(f.client);
        f.client.quoteNpcPurchase = async request => { const frame = await quote(request); invalidate(f.client.raw); return frame; };
        await until(f, v => v.status === "failed");
      } else {
        await until(f, v => v.step === "ready-to-purchase"); invalidate(f.client.raw); await f.controller.tick();
      }
      assert.equal(f.controller.view().status, "failed");
      assert.equal(f.client.calls.filter(call => call[0] === "quote").length, 1);
      assert.equal(f.client.calls.some(call => call[0] === "purchase"), false);
      assert.equal(f.client.raw.gold, 200); assert.deepEqual(f.client.raw.inventoryItems, []);
    });
  }
});

test("a valid null-dialogue shop refuses its observed 40-gold potion against a 39-gold grant", async t => {
  const f = subject(t); f.controller.authorize(grant({ maxGold: 39 }));
  await until(f, v => v.status === "failed");
  assert.equal(f.client.snapshot().activeNpcDialog, null);
  assert.equal(f.client.snapshot().nativeNpcShop.list[0].price, 40);
  assert.equal(f.client.calls.filter(call => call[0] === "begin").length, 1);
  assert.equal(f.client.calls.some(call => ["quote", "purchase"].includes(call[0])), false);
  assert.equal(f.client.raw.gold, 200); assert.deepEqual(f.client.raw.inventoryItems, []);
});

test("expiry, revocation and death stop before the economic request", async t => {
  for (const mutation of [(f) => f.client.clock = 300_000, (f) => f.controller.cancel(), (f) => { f.client.raw.playerHp = 0; position(f.client.raw).dead = true; }]) {
    const f = subject(t); f.controller.authorize(grant()); await until(f, v => v.step === "ready-to-purchase"); mutation(f);
    await f.controller.tick(); assert.equal(f.client.calls.some(c => c[0] === "purchase"), false);
    assert.equal(["paused", "cancelled"].includes(f.controller.view().status), true);
  }
});

test("stale goods, over-budget quote and insufficient actual gold cannot send a purchase", async t => {
  for (const update of [{ quotePrice: 51 }, { raw: { gold: 39 } }, { raw: { nativeNpcShop: shop({ displayGoods: [goods({ itemIndex: 659 })] }) } }]) {
    const client = new Client(); if (update.quotePrice) client.quotePrice = update.quotePrice;
    if (update.raw) Object.assign(client.raw, update.raw);
    const f = subject(t, { client }); f.controller.authorize(grant());
    if (update.raw?.nativeNpcShop) client.selectNpcBuyLink = async () => { client.calls.push(["link"]); };
    await until(f, v => v.status === "failed");
    assert.equal(client.calls.some(c => c[0] === "purchase"), false);
  }
});

test("a lost post-commit reply is queried under its original operation without charging twice", async t => {
  const client = new Client(); client.mode = "timeout-after-commit"; const f = subject(t, { client }); f.controller.authorize(grant());
  await until(f, v => v.step === "reconcile");
  const original = client.calls.find(c => c[0] === "purchase")[1];
  await until(f, v => v.status === "completed");
  assert.equal(client.raw.gold, 160); assert.equal(client.calls.filter(c => c[0] === "purchase").length, 1);
  assert.deepEqual(client.calls.find(c => c[0] === "query")[1], original);
});

test("unknown-before-commit and missing recovery receipt never authorize a replacement operation", async t => {
  const client = new Client(); client.mode = "timeout-before-commit"; const f = subject(t, { client }); f.controller.authorize(grant());
  await until(f, v => v.step === "reconcile");
  for (let i = 0; i < 20; i++) { client.clock += 5000; await f.controller.tick(); }
  assert.equal(f.controller.view().status, "unknown"); assert.equal(f.controller.view().receiptKnown, false);
  assert.equal(client.calls.filter(c => c[0] === "purchase").length, 1); assert.equal(client.raw.gold, 200);
  assert.throws(() => f.controller.authorize(grant({ permitId: "supply:replacement", expiresAtMs: client.clock + 300_000 })), /Reconcile/);
});

test("receipt without its applied complete authority remains unknown and cannot complete the task", async t => {
  const client = new Client(); client.mode = "no-pair"; const f = subject(t, { client }); f.controller.authorize(grant());
  await until(f, v => v.receiptKnown);
  assert.equal(f.controller.view().status, "unknown"); assert.equal(f.controller.view().acquiredQuantity, 0);
  assert.equal(f.controller.view().returnedToAnchor, false);
  await until(f, v => v.status === "completed"); assert.equal(client.calls.filter(c => c[0] === "purchase").length, 1);
});

test("malformed or unauthorized terminal outcomes never become an acquisition", async t => {
  for (const mode of ["over-budget", "wrong-operation"]) {
    const client = new Client(); client.mode = mode; const f = subject(t, { client }); f.controller.authorize(grant());
    await until(f, v => v.step === "reconcile");
    assert.equal(f.controller.view().acquiredQuantity, 0); assert.equal(f.controller.view().returnedToAnchor, false);
    assert.equal(client.calls.filter(c => c[0] === "purchase").length, 1);
  }
});

test("restart pauses a not-yet-attempted authorization and duplicate authorization cannot revive it", async t => {
  const path = database(), client = new Client();
  const old = new SupplyController({ client, dbPath: path, clock: () => client.clock }); old.authorize(grant()); await old.tick(); await old.close();
  const f = subject(t, { client, path }); assert.equal(f.controller.view().status, "paused");
  f.controller.authorize(grant()); await f.controller.tick();
  assert.equal(client.calls.some(c => c[0] === "purchase"), false); assert.equal(f.controller.view().status, "paused");
});

test("restart with a pending operation queries only that ID and does not resume spending or movement", async t => {
  const path = database(), client = new Client(); client.mode = "timeout-after-commit";
  const old = new SupplyController({ client, dbPath: path, clock: () => client.clock }); old.authorize(grant());
  for (let i = 0; i < 12 && old.view().step !== "reconcile"; i++) await old.tick();
  const original = client.calls.find(c => c[0] === "purchase")[1]; await old.close();
  client.calls = [];
  const f = subject(t, { client, path }); await f.controller.tick(); client.clock += 5000; await f.controller.tick();
  assert.equal(f.controller.view().status, "paused"); assert.equal(f.controller.view().acquiredQuantity, 1);
  assert.deepEqual(client.calls.find(c => c[0] === "query")[1], original);
  assert.equal(client.calls.some(c => ["purchase", "walk"].includes(c[0])), false);
});

test("cancelling during an in-flight request preserves the one original economic outcome", async t => {
  const client = new Client(); client.mode = "hold"; const f = subject(t, { client }); f.controller.authorize(grant());
  await until(f, v => v.step === "ready-to-purchase");
  const pending = f.controller.tick();
  assert.equal(f.controller.view().step, "purchase-pending"); f.controller.cancel(); client.release(); await pending;
  assert.equal(f.controller.view().status, "cancelled"); assert.equal(f.controller.view().chargedGold, 40); assert.equal(f.controller.view().acquiredQuantity, 1);
  assert.equal(f.controller.view().returnedToAnchor, false);
  await f.controller.tick(); assert.equal(client.calls.filter(c => c[0] === "purchase").length, 1);
});

test("bounds and map changes cannot cause unapproved movement", async t => {
  const client = new Client(); position(client.raw).x = 289; position(client.raw).y = 616;
  const f = subject(t, { client }); f.controller.authorize(grant());
  client.raw.mapFileName = "0141"; await f.controller.tick();
  assert.equal(f.controller.view().status, "paused"); assert.equal(client.calls.some(c => c[0] === "walk"), false);
});

test("returning beside an owner-occupied anchor never steps into the owner's tile or invents exact arrival", async t => {
  const client = new Client();
  const target = grant().returnAnchor, visited = [], observed = [];
  client.raw.entities.push({ objectId: 99, kind: "player", name: "Owner", x: target.x, y: target.y, hp: 18, dead: false });
  const originalWalk = client.walk.bind(client);
  client.walk = async direction => {
    const result = await originalWalk(direction), actual = position(client.raw);
    visited.push({ x: actual.x, y: actual.y });
    assert.notDeepEqual(visited.at(-1), target, "normal pathfinding must not enter the occupied anchor");
    return result;
  };
  const f = subject(t, { client, onObserved: e => observed.push(e) });
  f.controller.authorize(grant()); await until(f, v => v.status === "completed");
  const returned = observed.find(e => e.kind === "supply-returned");
  assert.equal(f.controller.view().returnedToAnchor, true);
  assert.equal(returned.arrivalDistance, 1); assert.notDeepEqual(returned.actualPosition, target);
  assert.deepEqual(returned.actualPosition, visited.at(-1));
  assert.deepEqual(returned.scope, { mapFileName: "0", bounds: grant().bounds, maximumDistanceFromAnchor: 1 });
});

test("distance two remains returning until a later authoritative adjacent observation", async t => {
  const observed = [], f = subject(t, { onObserved: e => observed.push(e) });
  f.controller.authorize(grant()); await until(f, v => v.status === "returning");
  position(f.client.raw).x = 288; position(f.client.raw).y = 614;
  await f.controller.tick();
  assert.equal(f.controller.view().status, "returning"); assert.equal(f.controller.view().returnedToAnchor, false);
  assert.equal(observed.some(e => e.kind === "supply-returned"), false);
  await f.controller.tick();
  assert.equal(f.controller.view().status, "completed");
  assert.deepEqual(observed.find(e => e.kind === "supply-returned").actualPosition, { x: 288, y: 615 });
});

test("a zero-length return records the actual authorized position without an extra walk", async t => {
  const observed = [], f = subject(t, { onObserved: e => observed.push(e) });
  f.controller.authorize(grant({ returnAnchor: { x: 288, y: 609 } }));
  await until(f, v => v.status === "completed");
  assert.equal(f.client.calls.some(c => c[0] === "walk"), false);
  const returned = observed.find(e => e.kind === "supply-returned");
  assert.equal(returned.arrivalDistance, 0); assert.deepEqual(returned.actualPosition, returned.returnAnchor);
});

test("authorization expiry during collision loading is rechecked before any movement", async t => {
  const client = new Client(); position(client.raw).y = 616;
  let release;
  const f = subject(t, { client, mapLoader: () => new Promise(resolve => { release = () => resolve({ width: 800, height: 800, blocked: new Set() }); }) });
  f.controller.authorize(grant()); const pending = f.controller.tick();
  client.clock = 300_000; release(); await pending;
  assert.equal(client.calls.some(c => c[0] === "walk"), false);
});

test("closing during pending navigation waits for the tick and never dispatches a new action", async () => {
  const client = new Client(); position(client.raw).y = 616;
  let release;
  const controller = new SupplyController({ client, dbPath: database(), clock: () => 0, mapLoader: () => new Promise(resolve => { release = () => resolve({ width: 800, height: 800, blocked: new Set() }); }) });
  controller.authorize(grant()); const pending = controller.tick(); const closing = controller.close();
  release(); await pending; await closing;
  assert.equal(client.calls.length, 0); assert.throws(() => controller.authorize(grant()), /closed/);
});
