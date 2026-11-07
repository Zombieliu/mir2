import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { DatabaseSync } from "node:sqlite";
import { GiftController } from "../src/gifts/gift-controller.mjs";
import { giftFixture, OWNER, UID, packet, tradeState } from "./gift-fixture.mjs";

const controller = (f, options = {}) => new GiftController({ client: f.client, ownerName: OWNER, dbPath: ":memory:", clock: () => 1000, ...options });
const accept = async (f, subject) => { f.invite(); await subject.tick(f.client.snapshot()); assert.equal(subject.view().status, "receiving"); };
const rawItem = (uniqueId = UID + 9n) => ({ unique_id: uniqueId, item_index: 658, count: 1 });

test("only the current configured owner invitation is accepted with a zero outgoing offer", async t => {
  const f = giftFixture(), subject = controller(f); t.after(() => subject.close());
  f.invite("Stranger"); await subject.tick(); assert.equal(f.calls.length, 0);
  f.invite(); await subject.tick();
  assert.deepEqual(f.calls.map(call => call.type), ["tradeReply"]);
  assert.equal(f.calls[0].acceptInvite, true); assert.equal(subject.view().status, "receiving");
  await subject.tick(); assert.equal(f.calls.length, 1); // Empty owner offer is never locked.
});

test("real final confirmation plus complete same-source gold and UID gain emits one gift receipt", async t => {
  const f = giftFixture(), seen = [], subject = controller(f, { onObserved: event => seen.push(event) }); t.after(() => subject.close());
  await accept(f, subject); f.offerGold(40); f.offerItems([null, rawItem()]);
  f.client.confirmHook = async () => { f.deliver({ gold: 40, items: [rawItem()] }); return { sent: true, confirmed: false }; };
  await subject.tick();
  assert.equal(subject.view().status, "completed"); assert.equal(subject.view().receivedGold, 40);
  assert.deepEqual(subject.view().receivedItems, [{ uniqueId: String(UID + 9n), quantity: 1, itemIndex: 658 }]);
  assert.equal(seen.length, 1); assert.equal(seen[0].kind, "owner-gift-received"); assert.ok(seen[0].sourceEventRef.startsWith("trade:"));
  for (let i = 0; i < 4; i++) await subject.tick();
  assert.equal(seen.length, 1); assert.equal(f.calls.filter(call => call.type === "tradeConfirm").length, 1);
  assert.equal(f.calls.some(call => ["tradeGold", "depositTradeItem"].includes(call.type)), false);
});

test("accepting and escrow preparation are not a gift and confirm is never replayed", async t => {
  const f = giftFixture(), seen = [], subject = controller(f, { onObserved: event => seen.push(event) }); t.after(() => subject.close());
  await accept(f, subject); f.offerGold(30);
  await subject.tick(); assert.equal(subject.view().status, "unknown"); assert.equal(seen.length, 0);
  for (let i = 0; i < 8; i++) await subject.tick();
  assert.equal(f.calls.filter(call => call.type === "tradeConfirm").length, 1); assert.equal(subject.view().receivedGold, null);
});

test("final packet without its later full snapshot waits instead of reporting receipt", async t => {
  const f = giftFixture(), seen = [], subject = controller(f, { onObserved: event => seen.push(event) }); t.after(() => subject.close());
  await accept(f, subject); f.offerGold(20);
  f.client.confirmHook = async () => { f.deliver({ gold: 20, full: false }); };
  await subject.tick(); assert.equal(subject.view().status, "unknown"); assert.equal(seen.length, 0);
  f.snapshot(); await subject.tick();
  assert.equal(subject.view().status, "completed"); assert.equal(seen.length, 1);
});

test("window closure and wallet change without a final packet cannot prove a gift", async t => {
  const f = giftFixture(), seen = [], subject = controller(f, { onObserved: event => seen.push(event) }); t.after(() => subject.close());
  await accept(f, subject); f.offerGold(40);
  f.client.confirmHook = async () => { f.data.gold += 40; f.data.stage5Systems.trade = null; f.snapshot(); };
  await subject.tick(); await subject.tick();
  assert.equal(subject.view().status, "unknown"); assert.equal(seen.length, 0); assert.equal(subject.view().receivedGold, null);
});

test("nonzero own assets and malformed offer state prevent economic confirmation", async t => {
  for (const patch of [{ offeredGold: 1 }, { offeredItems: ["hp-drug"] }, { offeredSlots: { 0: 1 } }, { offeredUniqueIds: { 0: UID } }]) {
    const f = giftFixture(), subject = controller(f); t.after(() => subject.close()); await accept(f, subject);
    Object.assign(f.data.stage5Systems.trade, patch); f.offerGold(40);
    await subject.tick();
    assert.equal(subject.view().status, "unknown"); assert.equal(f.calls.some(call => call.type === "tradeConfirm"), false);
  }
});

test("unmatched incoming amount, UID or source cannot become a successful receipt", async t => {
  for (const kind of ["wrong-gold", "missing-uid", "new-character"]) {
    const f = giftFixture(), seen = [], subject = controller(f, { onObserved: event => seen.push(event) }); t.after(() => subject.close());
    await accept(f, subject); f.offerGold(40); f.offerItems([rawItem()]);
    f.client.confirmHook = async () => {
      f.deliver({ gold: kind === "wrong-gold" ? 39 : 40, items: kind === "missing-uid" ? [] : [rawItem()] });
      if (kind === "new-character") { f.apply(packet("StartGame", { result: 4 })); f.snapshot(); }
    };
    await subject.tick(); await subject.tick(); assert.equal(subject.view().status, "unknown"); assert.equal(seen.length, 0);
  }
});

test("a genuinely delivered collision-repaired UID is checked against final GainedItem and full inventory", async t => {
  const f = giftFixture(), seen = [], subject = controller(f, { onObserved: event => seen.push(event) }); t.after(() => subject.close());
  await accept(f, subject); f.offerItems([rawItem(UID)]);
  f.client.confirmHook = async () => f.deliver({ items: [rawItem(UID + 99n)] });
  await subject.tick();
  assert.equal(subject.view().status, "completed"); assert.equal(seen[0].receivedItems[0].uniqueId, String(UID + 99n));
});

test("unknown confirmation is durable across a different controller and never replays economic packets", async t => {
  const directory = mkdtempSync(join(tmpdir(), "mir2-gift-test-")), dbPath = join(directory, "execution.sqlite");
  let second;
  t.after(async () => { await second?.close(); rmSync(directory, { recursive: true, force: true }); });
  const f = giftFixture(), first = controller(f, { dbPath });
  await accept(f, first); f.offerGold(40); f.client.confirmHook = async () => { throw new Error("result lost"); };
  await first.tick(); assert.equal(first.view().status, "unknown"); await first.close();
  second = controller(f, { dbPath });
  f.client.confirmHook = async () => f.deliver({ gold: 40 });
  for (let i = 0; i < 5; i++) await second.tick();
  assert.equal(second.view().status, "unknown"); assert.equal(second.view().receivedGold, null);
  assert.equal(f.calls.filter(call => call.type === "tradeConfirm").length, 1);
});

test("danger cancellation uses ordinary cancel and never records refunded or closed trade as a gift", async t => {
  const f = giftFixture(), seen = [], subject = controller(f, { onObserved: event => seen.push(event) }); t.after(() => subject.close());
  await accept(f, subject); f.offerGold(10);
  await subject.cancel("遇险，先取消交易再治疗。");
  assert.equal(subject.view().status, "cancelled"); assert.equal(seen.length, 0);
  assert.equal(f.calls.at(-1).type, "tradeCancel"); assert.equal(f.calls.some(call => call.type === "tradeConfirm"), false);
});

test("duplicate controller ticks cannot race an in-flight acceptance or confirm", async t => {
  const f = giftFixture(), subject = controller(f); t.after(() => subject.close());
  let resolve;
  f.client.acceptHook = () => new Promise(done => { resolve = done; });
  f.invite(); const first = subject.tick(); await subject.tick();
  assert.equal(f.calls.length, 1);
  f.apply(packet("TradeAccept", { partnerName: OWNER })); f.data.stage5Systems.trade = tradeState(); f.snapshot();
  resolve({ accepted: true, tradeId: "a".repeat(32) }); await first;
  assert.equal(subject.view().status, "receiving");
});

test("confirmation attempt is durably committed before the ordinary economic packet", async t => {
  const directory = mkdtempSync(join(tmpdir(), "mir2-gift-persist-test-")), dbPath = join(directory, "execution.sqlite");
  const f = giftFixture(), subject = controller(f, { dbPath });
  t.after(async () => { await subject.close(); rmSync(directory, { recursive: true, force: true }); });
  await accept(f, subject); f.offerGold(40);
  let durableAttempt = false;
  f.client.confirmHook = async () => {
    const reader = new DatabaseSync(dbPath, { readOnly: true });
    try {
      const entry = JSON.parse(reader.prepare("SELECT entry_json FROM received_gifts ORDER BY sequence DESC LIMIT 1").get().entry_json);
      durableAttempt = entry.confirmAttempted === true && entry.status === "unknown" && entry.baseline.gold === 10;
    } finally { reader.close(); }
    throw new Error("drop result");
  };
  await subject.tick(); assert.equal(durableAttempt, true); assert.equal(subject.view().status, "unknown");
});

test("same settlement identity cannot produce a second economic confirmation", async t => {
  const f = giftFixture(), seen = [], subject = controller(f, { onObserved: event => seen.push(event) }); t.after(() => subject.close());
  await accept(f, subject); f.offerGold(1); f.client.confirmHook = async () => f.deliver({ gold: 1 }); await subject.tick();
  assert.equal(subject.view().status, "completed");
  f.invite(); await subject.tick(); f.offerGold(1); await subject.tick();
  assert.equal(subject.view().status, "unknown"); assert.equal(seen.length, 1);
  assert.equal(f.calls.filter(call => call.type === "tradeConfirm").length, 1);
});
