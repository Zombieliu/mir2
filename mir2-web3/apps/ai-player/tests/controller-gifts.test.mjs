import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { GiftController } from "../src/gifts/gift-controller.mjs";
import { CompanionController } from "../src/companion-controller.mjs";
import { giftFixture, OWNER, packet } from "./gift-fixture.mjs";

// Real observation, gift journal and companion execution are composed here;
// only the ordinary network actions are synthetic. No external service runs.
function resident(t, { dbPath = ":memory:" } = {}) {
  const f = giftFixture(), facts = [], decisions = [], time = { now: 1000 }, gifts = [];
  f.data.inventoryItems[0].name = "(HP)DrugSmall";
  Object.assign(f.data, { gold: 85, freeBagSlots: 40, currentWeight: 1, maxWeight: 100, inSafeZone: true }); f.snapshot();
  const store = { profile: () => ({ policyVersion: 1, solo: { considerLootWhenSafe: true }, party: { assistBeforeLoot: true, lootOnlyAfterCombat: true, askBeforeLeaving: true }, permissions: { mapAllowlist: ["0"] } }), recordEpisode: value => decisions.push(value) };
  const createGift = () => { const gift = new GiftController({ client: f.client, ownerName: OWNER, dbPath, clock: () => time.now, onObserved: event => facts.push(event) }); gifts.push(gift); return gift; };
  let gift = createGift();
  f.client.useOwnedItem = async (uniqueId, grid) => {
    f.calls.push({ type: "useItem", uniqueId: String(uniqueId), grid });
    const item = f.data.inventoryItems.find(item => String(item.uniqueId) === String(uniqueId)); assert.ok(item && item.quantity > 0);
    item.quantity--; f.apply(packet("UseItem", { uniqueId, grid, success: true })); f.snapshot(); return { success: true, uniqueId };
  };
  f.client.walk = async direction => {
    f.calls.push({ type: "walk", direction });
    const [dx, dy] = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] }[direction];
    const player = f.data.entities.find(entity => entity.kind === "selfPlayer"); player.x += dx; player.y += dy; f.snapshot(); return { moved: true };
  };
  f.client.attackDirection = async direction => { f.calls.push({ type: "attackDirection", direction }); return { sent: true, confirmed: false }; };
  f.client.pickUpTile = async () => { f.calls.push({ type: "pickUpTile" }); return { changed: false }; };
  const actor = new CompanionController({ client: f.client, store, leaderName: OWNER, homePosition: { x: 288, y: 616, mapFileName: "0" },
    giftController: gift, clock: () => time.now, mapLoader: async () => ({ width: 800, height: 800, blocked: new Set() }) });
  t.after(async () => { await actor.close(); for (const value of gifts) await value.close(); });
  return { ...f, actor, facts, decisions, time, store, get gift() { return gift; },
    async accept() { f.invite(); const action = await actor.tick(); assert.equal(action.kind, "gift"); assert.equal(gift.view().status, "receiving"); },
    async restartGift() { await gift.close(); gift = createGift(); actor.giftController = gift; return gift; },
    place({ hp = 18, potions = 1, y = 616, safe = y === 616 } = {}) {
      f.data.playerHp = hp; f.data.entities.find(e => e.kind === "selfPlayer").y = y; f.data.inventoryItems[0].quantity = potions; f.data.inSafeZone = safe; f.snapshot();
    },
  };
}

test("a danger cancellation owns one tick and the following tick heals even while final cancellation is still unknown", async t => {
  const f = resident(t); await f.accept(); f.offerGold(40); f.place({ hp: 10 });
  f.client.cancelHook = async () => { f.apply(packet("TradeCancel", { unlock: false })); f.data.stage5Systems.trade = null; }; // Full snapshot deliberately arrives later.
  const before = f.calls.length, cancelled = await f.actor.tick();
  assert.equal(cancelled.kind, "gift"); assert.deepEqual(f.calls.slice(before).map(call => call.type), ["tradeCancel"]);
  assert.equal(f.gift.view().status, "unknown"); assert.equal(f.gift.view().cancelAttempted, true);
  const healed = await f.actor.tick();
  assert.equal(healed.kind, "usePotion"); assert.deepEqual(f.calls.slice(before).map(call => call.type), ["tradeCancel", "useItem"]);
  assert.equal(f.calls.filter(call => call.type === "tradeConfirm").length, 0); assert.equal(f.facts.length, 0);
  f.time.now += 5000; f.place({ hp: 18, potions: 0 }); await f.actor.tick();
  assert.equal(f.gift.view().status, "cancelled"); assert.equal(f.calls.filter(call => call.type === "tradeCancel").length, 1);
});

test("danger and recall cancel an active gift once before an ordinary withdrawal step", async t => {
  for (const recall of [false, true]) {
    const f = resident(t); await f.accept(); f.place({ hp: recall ? 18 : 5, potions: recall ? 1 : 0, y: 628, safe: false }); f.actor.recallRequested = recall;
    const before = f.calls.length;
    assert.equal((await f.actor.tick()).kind, "gift"); assert.deepEqual(f.calls.slice(before).map(call => call.type), ["tradeCancel"]);
    const step = await f.actor.tick(); assert.equal(step.reason, recall ? "owner-recall" : "unsafe-withdraw");
    assert.deepEqual(f.calls.slice(before).map(call => call.type), ["tradeCancel", "walk"]); assert.equal(f.facts.length, 0);
  }
});

test("genuine self-defence follows the single cancellation lane without becoming another trade confirmation", async t => {
  const f = resident(t); await f.accept();
  f.data.entities.push({ kind: "monster", objectId: 80, name: "Scarecrow", x: 288, y: 617, hp: 10, disposition: "hostile" }); f.snapshot();
  const before = f.calls.length; assert.equal((await f.actor.tick()).kind, "gift");
  assert.equal((await f.actor.tick()).reason, "defend-confirmed-engagement");
  assert.deepEqual(f.calls.slice(before).map(call => call.type), ["tradeCancel", "attackDirection"]); assert.equal(f.facts.length, 0);
});

test("a persistent unknown gift after restart blocks new supply while allowing medicine and retreat", async t => {
  const directory = mkdtempSync(join(tmpdir(), "alo-controller-gift-restart-")), f = resident(t, { dbPath: join(directory, "gifts.sqlite") });
  await f.accept(); f.offerGold(40); f.client.confirmHook = async () => { throw new Error("synthetic lost confirmation result"); };
  assert.equal((await f.actor.tick()).kind, "gift"); assert.equal(f.gift.view().status, "unknown");
  // A new physical session has a complete empty trade projection, but it does
  // not resolve the older durable journal or justify a new economic attempt.
  f.data.stage5Systems.trade = null; f.enter(); await f.restartGift();
  assert.equal(f.client.snapshot().trade.active, false); assert.equal(f.client.snapshot().trade.uncertain, false); assert.equal(f.gift.view().status, "unknown");
  let purchases = 0;
  f.actor.supplyController = { view: () => ({ status: "running", permitId: "supply:must-stay-blocked" }), async tick() { purchases++; return { performed: true }; } };
  const before = f.calls.length;
  assert.equal((await f.actor.tick()).reason, "trade-outcome-unknown"); assert.equal(purchases, 0); assert.equal(f.calls.length, before);
  f.place({ hp: 10, potions: 1 }); assert.equal((await f.actor.tick()).kind, "usePotion"); assert.equal(purchases, 0);
  f.time.now += 5000; f.place({ hp: 5, potions: 0, y: 628, safe: false });
  assert.equal((await f.actor.tick()).reason, "unsafe-withdraw"); assert.equal(purchases, 0);
  assert.deepEqual(f.calls.slice(before).map(call => call.type), ["useItem", "walk"]);
  assert.equal(f.calls.filter(call => call.type === "tradeConfirm").length, 1); assert.equal(f.facts.length, 0);
});

test("an unknown gift with no active window blocks the life claim action and regular loot", async t => {
  const f = resident(t); await f.accept(); f.offerGold(40); f.client.confirmHook = async () => { throw new Error("synthetic unknown gift"); }; await f.actor.tick();
  f.data.stage5Systems.trade = null; f.snapshot(); // Missing final TradeConfirm is intentionally not success.
  assert.equal(f.gift.view().status, "unknown");
  let claims = 0;
  f.actor.lifeController = { view: () => ({ status: "running", lifePermitId: "life:must-not-spend" }),
    decide: ({ policy }) => ({ kind: "life", reason: "life-claim-one-supply", policyVersion: policy.policyVersion, lifePermitId: "life:must-not-spend" }),
    movementConstraint: () => null, async tick() { claims++; return { performed: true }; } };
  f.data.groundDrops = [{ objectId: 90, name: "Allowed loot", x: 288, y: 616, ownerObjectId: 50001 }]; f.snapshot();
  const before = f.calls.length; assert.equal((await f.actor.tick()).reason, "trade-outcome-unknown");
  assert.equal(claims, 0); assert.equal(f.calls.length, before);
});

test("an unrelated invitation cannot starve normal loot or medicine and is never accepted", async t => {
  const f = resident(t); f.invite("Stranger"); f.data.groundDrops = [{ objectId: 90, name: "Allowed loot", x: 288, y: 616, ownerObjectId: 50001 }]; f.snapshot();
  assert.equal(f.gift.hasWork(), false); assert.equal((await f.actor.tick()).kind, "loot");
  f.place({ hp: 10 }); assert.equal((await f.actor.tick()).kind, "usePotion");
  assert.deepEqual(f.calls.map(call => call.type), ["pickUpTile", "useItem"]);
  assert.equal(f.client.snapshot().tradePendingInvite.name, "Stranger"); assert.equal(f.gift.view().status, "idle");
});

test("a late authoritative cancellation is observed without sending another cancel or fabricating a gift", async t => {
  const f = resident(t); await f.accept(); f.place({ hp: 10 });
  f.client.cancelHook = async () => { throw new Error("synthetic cancellation timeout"); };
  await f.actor.tick(); assert.equal(f.gift.view().status, "unknown");
  f.apply(packet("TradeCancel", { unlock: false })); f.data.stage5Systems.trade = null; f.snapshot();
  const before = f.calls.length; assert.equal((await f.actor.tick()).kind, "usePotion");
  assert.equal(f.gift.view().status, "cancelled"); assert.deepEqual(f.calls.slice(before).map(call => call.type), ["useItem"]);
  assert.equal(f.calls.filter(call => call.type === "tradeCancel").length, 1); assert.equal(f.facts.length, 0);
});

test("ordinary owner receipt completion releases the life lane only after final event and full snapshot", async t => {
  const f = resident(t); await f.accept(); f.offerGold(40);
  let lifeTicks = 0;
  f.actor.lifeController = { view: () => ({ status: "running", lifePermitId: "life:after-receipt" }),
    decide: ({ policy }) => ({ kind: "life", reason: "life-claim-one-supply", policyVersion: policy.policyVersion, lifePermitId: "life:after-receipt" }),
    movementConstraint: () => null, async tick() { lifeTicks++; return { performed: false }; } };
  f.client.confirmHook = async () => { f.deliver({ gold: 40, full: false }); };
  await f.actor.tick(); assert.equal(f.gift.view().status, "unknown"); assert.equal(lifeTicks, 0);
  await f.actor.tick(); assert.equal(lifeTicks, 0); assert.equal(f.facts.length, 0);
  f.snapshot(); assert.equal((await f.actor.tick()).kind, "life");
  assert.equal(f.gift.view().status, "completed"); assert.equal(lifeTicks, 1); assert.equal(f.facts.length, 1);
  assert.equal(f.facts[0].receivedGold, 40); assert.equal(f.calls.filter(call => call.type === "tradeConfirm").length, 1);
});
