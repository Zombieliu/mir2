import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { ExperienceLedger, participantCsv, participantEventOf } from "../src/experience/experience-ledger.mjs";

const OWNER = "owner:one", ARCHIVE = "archive:alo";
const OFFER = { offerId: "fixed-two", currency: "CNY", priceMicros: "2000000", durationDays: 1, adventureCount: 2, paymentInstructions: "Operator-confirmed test receipt only" };
const BINDING = { experimentId: "trial-test", realmId: "world-test", worldEpoch: "epoch-test", currency: "CNY", environment: "test" };

function fixture(t, { offer = OFFER, start = 1000 } = {}) {
  const root = mkdtempSync(join(tmpdir(), "mir2-experience-test-")), stores = [];
  let now = start, counter = 0;
  const path = join(root, "ledger.sqlite");
  const open = options => { const ledger = new ExperienceLedger({ path, ...BINDING, offer, clock: () => now, ...options }); stores.push(ledger); return ledger; };
  const ledger = open(); ledger.registerParticipant(OWNER, "P-test");
  t.after(() => {
    for (const store of stores) store.close();
    const absolute = resolve(root);
    assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("mir2-experience-test-"));
    rmSync(absolute, { recursive: true, force: true });
  });
  return { ledger, path, open, setTime(value) { now = value; }, advance(value = 100) { now += value; },
    participant(event, options = {}) { return ledger.recordParticipantEvent({ operationId: "participant-" + ++counter, ownerRef: OWNER, archiveId: ARCHIVE, event, ...options }); },
    operator(kind, payload, options = {}) { return ledger.recordOperator({ source: "local-operator", operationId: "operator-" + ++counter, ownerRef: OWNER, archiveId: ARCHIVE, kind, payload, ...options }); },
    visit(id, source = "unknown", count = 0) { return this.participant({ kind: "visit", visitId: id, entrySource: source, reminderCount: count }); },
    choice(id, options = {}) { return this.participant({ kind: "choice", visitId: id, reason: "shared-history", reasonText: "我想继续陪伴同一个角色", ...options }); },
    payment(id, options = {}) { return this.operator("payment", { paymentId: id, offerId: OFFER.offerId, amountMicros: OFFER.priceMicros, currency: "CNY", paidAt: now, receiptRef: "receipt-" + id }, options); },
    grant(id, paymentId, startsAt = now) { return this.operator("grant", { grantId: id, paymentId, startsAt }); },
    observed(evidenceRef = "strike:test") { return ledger.recordObservedAdventure({ ownerRef: OWNER, archiveId: ARCHIVE, evidenceRef }); },
    view() { return ledger.ownerView(OWNER, ARCHIVE); },
  };
}

test("participant inputs preserve intent without manufacturing payments, expenses or measured activity", t => {
  const f = fixture(t); f.visit("v1"); f.choice("v1");
  f.participant({ kind: "teachingCheck", visitId: "v1", understood: false, reasonText: "我还没看懂" });
  f.participant({ kind: "purchaseIntent", visitId: "v1", offerId: OFFER.offerId });
  for (const event of [{ kind: "payment", visitId: "v1" }, { kind: "cost", visitId: "v1" }, { kind: "observedAdventure", visitId: "v1" }, { kind: "choice", visitId: "v1", reason: "shared-history", amountMicros: "1" }]) assert.throws(() => f.participant(event));
  assert.throws(() => f.operator("payment", {}, { source: "participant" }), /Operator/);
  const view = f.view();
  assert.equal(view.purchaseIntents[0].status, "intent"); assert.equal(view.purchases.length, 0);
  assert.equal(view.costs.revenueMicros, "0"); assert.equal(view.usage.confirmedAdventureCount, 0);
  assert.equal(view.teachingChecks[0].understood, false);
  assert.equal(view.choices[0].reasonText, "我想继续陪伴同一个角色");
  for (const reason of ["shared-history", "stronger", "setup-ease", "curiosity", "other"]) assert.equal(participantEventOf({ kind: "choice", visitId: "v1", reason }).reason, reason);
});

test("self-reported return source/reminder count remains distinct from operator contact records", t => {
  const f = fixture(t); f.visit("v1", "unknown", 7); f.choice("v1", { reason: "curiosity", reasonText: "收到消息后想看看" });
  f.operator("invitation", { channel: "manual", occurredAt: 900, reference: "invite-note" });
  f.operator("reminder", { channel: "wechat", occurredAt: 950, reference: "reminder-note" });
  const view = f.view();
  assert.equal(view.visits[0].entrySource, "unknown"); assert.equal(view.visits[0].reminderCount, 7);
  assert.equal(view.visits[0].source, "participant-self-report");
  assert.deepEqual(view.contacts.map(row => [row.kind, row.source]), [["invitation", "operator-confirmed"], ["reminder", "operator-confirmed"]]);
  assert.equal(view.visitCount, 1); assert.equal(view.choices[0].reason, "curiosity");
});

test("visit idempotency, exact operation conflicts and superseded visit boundaries remain durable", t => {
  const f = fixture(t), visit = { kind: "visit", visitId: "v1", entrySource: "unprompted", reminderCount: 0 };
  const input = { operationId: "stable-op", ownerRef: OWNER, archiveId: ARCHIVE, event: visit };
  f.ledger.recordParticipantEvent(input); f.ledger.recordParticipantEvent(input);
  assert.equal(f.view().visitCount, 1);
  assert.throws(() => f.ledger.recordParticipantEvent({ ...input, event: { ...visit, reminderCount: 1 } }), /conflicts/);
  f.participant(visit, { operationId: "same-visit-new-op" });
  assert.equal(f.view().visitCount, 1);
  assert.throws(() => f.participant({ ...visit, visitId: "different" }, { operationId: "same-visit-new-op" }), /conflict/i);
  f.advance(); f.visit("v2", "reminder", 1);
  const view = f.view(); assert.equal(view.visits[0].endReason, "superseded"); assert.equal(view.visits[0].endedAt, 1100);
  assert.throws(() => f.choice("v1"), /current participant visit/);
  f.participant({ kind: "visitEnd", visitId: "v2" }); assert.equal(f.view().activeVisitId, null);
  f.ledger.close();
  assert.equal(f.open().ownerView(OWNER).visitCount, 2);
});

test("a visit ended at timestamp zero is closed and cannot accept later participant responses", t => {
  const f = fixture(t, { start: 0 }); f.visit("epoch-zero"); f.participant({ kind: "visitEnd", visitId: "epoch-zero" });
  assert.equal(f.view().visits[0].endedAt, 0);
  assert.equal(f.view().activeVisitId, null);
  assert.throws(() => f.choice("epoch-zero"), /current participant visit/);
});

test("the fixed offer cannot change, mixed currencies and duplicate receipts cannot become revenue", t => {
  const f = fixture(t); f.payment("payment-one");
  const exact = { source: "local-operator", operationId: "payment-idempotent", ownerRef: OWNER, archiveId: ARCHIVE, kind: "payment",
    payload: { paymentId: "payment-two", offerId: OFFER.offerId, amountMicros: OFFER.priceMicros, currency: "CNY", paidAt: 1000, receiptRef: "receipt-two" } };
  f.ledger.recordOperator(exact); f.ledger.recordOperator(exact); assert.equal(f.view().purchases.length, 2);
  assert.throws(() => f.open({ offer: { ...OFFER, priceMicros: "3000000" } }), /fixed offer/);
  for (const patch of [{ currency: "USD" }, { amountMicros: "1" }, { amountMicros: 2_000_000 }, { amountMicros: "02000000" }]) assert.throws(() => f.operator("payment", { ...exact.payload, paymentId: "bad", receiptRef: "bad-receipt", ...patch }));
  f.ledger.registerParticipant("owner:two", "P-two");
  assert.throws(() => f.operator("payment", { ...exact.payload, paymentId: "another" }, { ownerRef: "owner:two" }), /receipt/);
  assert.throws(() => f.grant("grant-absent", "unpaid"), /confirmed payment/);
  f.operator("refund", { paymentId: "payment-one", amountMicros: "1000000", currency: "CNY", occurredAt: 1000, receiptRef: "refund-one" });
  assert.throws(() => f.operator("refund", { paymentId: "payment-one", amountMicros: "1000001", currency: "CNY", occurredAt: 1000, receiptRef: "refund-excess" }), /exceeds/);
  assert.throws(() => f.grant("grant-refunded", "payment-one"), /Refunded/);
  assert.equal(f.view().costs.revenueMicros, "3000000");
});

test("purchase intent is disabled until a positive fixed-price offer exists", t => {
  const f = fixture(t, { offer: null }); f.visit("v1");
  assert.equal(f.view().offer, null);
  assert.throws(() => f.participant({ kind: "purchaseIntent", visitId: "v1", offerId: OFFER.offerId }), /not enabled/);
  assert.throws(() => f.open({ offer: { ...OFFER, priceMicros: "0" } }), /positive price/);
  const enabled = f.open({ offer: OFFER }); assert.deepEqual(enabled.offer(), OFFER);
});

test("payment and grant require actual observed use, bounded once per visit, with limits and expiry", t => {
  const f = fixture(t); f.visit("v1"); f.choice("v1"); f.payment("p1");
  assert.equal(f.view().purchases[0].serviceOpened, false); assert.equal(f.view().usage.paidAdventureCount, 0);
  assert.equal(f.observed("before-grant"), true); assert.equal(f.view().usage.paidAdventureCount, 0);
  f.grant("g1", "p1");
  assert.equal(f.view().purchases[0].serviceOpened, true);
  assert.equal(f.observed("after-grant-same-visit"), false, "grant cannot retroactively turn the earlier visit into paid usage");
  for (const id of ["v2", "v3", "v4"]) { f.advance(); f.visit(id); f.choice(id); assert.equal(f.observed("strike-" + id), true); assert.equal(f.observed("second-" + id), false); }
  assert.equal(f.view().usage.confirmedAdventureCount, 4); assert.equal(f.view().usage.paidAdventureCount, 2);
  f.payment("renewal"); f.grant("g2", "renewal");
  assert.equal(f.view().visitCount, 4); assert.equal(f.view().usage.paidAdventureCount, 2, "renewal is not retention or consumption");
  f.advance(); f.visit("v5"); f.choice("v5"); f.observed("new-actual-use");
  assert.equal(f.view().usage.paidAdventureCount, 3);
  f.advance(2 * 86_400_000); f.visit("expired"); f.choice("expired"); f.observed("outside-package-period");
  assert.equal(f.view().usage.paidAdventureCount, 3);
  assert.throws(() => f.grant("duplicate-payment-grant", "p1"), /already exists/);
});

test("unselected/ended visits do not manufacture an observed adventure or runtime attention", t => {
  const f = fixture(t);
  assert.equal(f.observed(), false); f.visit("v1"); assert.equal(f.observed(), false);
  f.choice("v1"); f.participant({ kind: "visitEnd", visitId: "v1" }); assert.equal(f.observed(), false);
  const session = f.ledger.startRuntime({ ownerRef: OWNER, archiveId: ARCHIVE, runtimeId: "runtime-A", sessionId: "session-one" });
  f.ledger.measureRuntime(session, { elapsedMs: 2200, sharedParty: true });
  f.ledger.measureRuntime(session, { elapsedMs: 1300, sharedParty: false, coverageGap: true });
  f.ledger.stopRuntime(session); f.ledger.stopRuntime(session);
  f.ledger.measureRuntime(session, { elapsedMs: 60_000, sharedParty: true });
  assert.equal(f.view().usage.runtimeSeconds, 3); assert.equal(f.view().usage.sharedPartySeconds, 2);
  assert.equal(f.view().usage.coverageGaps, 1); assert.equal(f.view().usage.modelCalls, 0);
  assert.match(f.view().usage.coverage, /no human-attention inference/);
});

test("contribution remains unknown until every actual cost is known; estimates and support stay separate", t => {
  const f = fixture(t); f.payment("paid");
  assert.equal(f.view().costs.complete, false); assert.equal(f.view().costs.contributionMicros, null);
  f.operator("cost", { category: "hosting", amountMicros: "999999", currency: "CNY", basis: "estimate", reference: "host-rate-estimate" });
  f.operator("support", { seconds: 45, reference: "operator-support" });
  assert.equal(f.view().costs.hostingMicros, null); assert.equal(f.view().costs.supportSeconds, 45);
  for (const [category, amountMicros] of [["model", "1000"], ["hosting", "10000"], ["payment", "20000"]]) {
    f.operator("cost", { category, amountMicros, currency: "CNY", basis: "actual", reference: "actual-" + category });
    assert.equal(f.view().costs.contributionMicros, null);
  }
  f.operator("cost", { category: "revenueShare", amountMicros: "30000", currency: "CNY", basis: "actual", reference: "share-actual" });
  const costs = f.view().costs;
  assert.equal(costs.complete, true); assert.equal(costs.contributionMicros, "1939000");
  assert.deepEqual(costs.estimates, [{ category: "hosting", amountMicros: "999999", reference: "host-rate-estimate" }]);
  assert.equal(costs.supportSeconds, 45);
  assert.throws(() => f.operator("cost", { category: "hosting", amountMicros: "0", currency: "USD", basis: "actual", reference: "other-currency" }), /currencies/);
});

test("CSV escapes spreadsheet formulas, quotes and leading whitespace before a formula", () => {
  for (const value of ["=SUM(1,2)", "+cmd", "-1+2", "@cmd", "\tcmd", "\rcmd", "   =1+1"]) {
    const csv = participantCsv([{ participantRef: value, environment: "test" }]);
    assert.ok(csv.startsWith("participantRef,environment,"));
    assert.ok(csv.includes('"\'' + value.replaceAll('"', '""') + '"'), JSON.stringify(value));
  }
  assert.ok(participantCsv([{ participantRef: 'quote"inside', environment: "test" }]).includes('"quote""inside"'));
});

test("reopening rejects foreign experiment/world/currency and preserves committed source distinctions", t => {
  const f = fixture(t); f.visit("v1"); f.payment("one"); f.ledger.close();
  for (const changes of [{ experimentId: "foreign" }, { realmId: "other" }, { worldEpoch: "other-epoch" }, { currency: "USD", offer: null }, { environment: "trial" }]) assert.throws(() => f.open(changes), /different experiment/);
  const reopened = f.open(); assert.equal(reopened.ownerView(OWNER).visitCount, 1); assert.equal(reopened.ownerView(OWNER).purchases.length, 1);
  assert.throws(() => reopened.registerParticipant(OWNER, "different-label"), /already bound/);
  assert.throws(() => reopened.registerParticipant("owner:foreign", "P-test"), /already bound/);
});
