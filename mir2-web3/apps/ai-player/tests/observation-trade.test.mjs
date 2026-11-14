import assert from "node:assert/strict";
import test from "node:test";
import { giftFixture, OWNER, UID, packet, tradeState, world } from "./gift-fixture.mjs";

test("trade observation projects authoritative own empty offer and exact partner UID without private trees", () => {
  const f = giftFixture(); f.invite();
  const invite = f.client.snapshot().tradePendingInvite;
  assert.equal(invite.name, OWNER); assert.equal(invite.characterEpoch, f.client.snapshot().characterEpoch);
  f.apply(packet("TradeAccept", { partnerName: OWNER }));
  f.data.stage5Systems.trade = tradeState({ privateBudget: "hidden", offeredUniqueIds: { 1: UID } }); f.snapshot();
  f.apply(packet("TradeItem", { tradeItems: [null, { unique_id: UID + 1n, item_index: 658, count: 2, slots: [{ secret: "hidden" }], private: "hidden" }] }));
  const state = f.client.snapshot();
  assert.equal(state.trade.active, true); assert.equal(state.trade.ownOfferKnown, true); assert.equal(state.trade.ownGold, 0);
  assert.ok(state.trade.ownItems.length > 0);
  assert.deepEqual(state.trade.partnerItems, [{ uniqueId: UID + 1n, quantity: 2, itemIndex: 658 }]);
  assert.equal(state.trade.privateBudget, undefined); assert.equal(state.trade.partnerItems[0].slots, undefined);
  assert.equal(state.fullSnapshot.inventoryComplete, true); assert.equal(state.fullSnapshot.tradeKnown, true);
});

test("prepare and window closure cannot impersonate final delivery", () => {
  const f = giftFixture(); f.data.stage5Systems.trade = tradeState({ locked: true, escrowPrepared: true }); f.snapshot();
  assert.equal(f.client.snapshot().trade.lastFinalConfirm, null);
  f.data.stage5Systems.trade = null; f.snapshot();
  assert.equal(f.client.snapshot().trade.active, false); assert.equal(f.client.snapshot().trade.uncertain, true);
  assert.equal(f.client.snapshot().trade.lastFinalConfirm, null);
});

test("final TradeConfirm is source-bound and requires a later complete snapshot", () => {
  const f = giftFixture(); f.data.stage5Systems.trade = tradeState(); f.snapshot();
  f.apply(packet("GainedGold", { gold: 20 })); f.data.gold += 20;
  f.apply(packet("GainedItem", { item: { unique_id: UID + 2n, item_index: 658, count: 1 } }));
  f.apply(packet("TradeConfirm"));
  const before = f.client.snapshot();
  assert.equal(before.inventoryStale, true); assert.equal(before.trade.uncertain, true);
  assert.equal(before.trade.lastFinalConfirm.tradeId, "a".repeat(32));
  assert.equal(before.trade.deliveryGold[0].gold, 20); assert.equal(before.trade.deliveryItems[0].uniqueId, UID + 2n);
  f.data.stage5Systems.trade = null; f.snapshot();
  const after = f.client.snapshot();
  assert.equal(after.trade.uncertain, false); assert.ok(after.fullSnapshot.revision > after.trade.lastFinalConfirm.revision);
});

test("new character, connection and map cannot inherit trade invitation or terminal receipt", () => {
  const f = giftFixture(); f.invite(); const old = f.client.snapshot();
  f.apply(packet("StartGame", { result: 4 })); f.snapshot();
  assert.equal(f.client.snapshot().tradePendingInvite, null); assert.ok(f.client.snapshot().characterEpoch > old.characterEpoch);
  f.invite(); f.reducer.resetConnection();
  f.apply({ ...packet("TradeRequest", { name: OWNER }), connectionEpoch: old.connectionEpoch });
  assert.equal(f.client.snapshot().tradePendingInvite, null);
  f.enter(); f.invite(); f.apply(packet("MapChanged", { mapIndex: 141, fileName: "0141" }));
  assert.equal(f.client.snapshot().tradePendingInvite, null); assert.equal(f.client.snapshot().trade.lastFinalConfirm, null);
});

test("missing trade snapshot and malformed partner item are unknown rather than empty", () => {
  const f = giftFixture(); f.data.stage5Systems.trade = tradeState(); f.snapshot();
  f.apply(packet("TradeItem", { partnerItems: [{ name: "Potion", count: 1 }] }));
  assert.equal(f.client.snapshot().trade.uncertain, true);
  f.apply({ type: "worldSnapshot", payload: world({ stage5Systems: {} }) });
  assert.equal(f.client.snapshot().fullSnapshot.tradeKnown, false); assert.equal(f.client.snapshot().trade.ownOfferKnown, false);
});

test("unlock preserves the trade while cancellation never creates a final receipt", () => {
  const f = giftFixture(); f.data.stage5Systems.trade = tradeState({ locked: true }); f.snapshot();
  f.apply(packet("TradeCancel", { unlock: true }));
  assert.equal(f.client.snapshot().trade.active, true); assert.equal(f.client.snapshot().trade.locked, false);
  f.apply(packet("TradeCancel", { unlock: false })); f.data.stage5Systems.trade = null; f.snapshot();
  assert.equal(f.client.snapshot().trade.active, false); assert.equal(f.client.snapshot().trade.uncertain, false);
  assert.equal(f.client.snapshot().trade.lastFinalConfirm, null); assert.equal(f.client.snapshot().trade.lastCancel.unlock, false);
});

test("a previously unlocked trade closing without a terminal packet remains unknown", () => {
  const f = giftFixture(); f.data.stage5Systems.trade = tradeState(); f.snapshot();
  f.apply(packet("TradeCancel", { unlock: true })); f.data.stage5Systems.trade = null; f.snapshot();
  assert.equal(f.client.snapshot().trade.uncertain, true);
  assert.equal(f.client.snapshot().trade.lastFinalConfirm, null);
});
