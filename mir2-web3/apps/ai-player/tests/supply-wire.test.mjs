import assert from "node:assert/strict";
import test from "node:test";
import { parsePurchaseFrame, purchaseOperation, purchaseReceipt } from "../src/supply/purchase-wire.mjs";
import { operation, receipt, producer, ownerFrame, world, intent } from "./supply-fixture.mjs";

const request = action => ({ type: "npcPurchaseOwner", protocolVersion: 1, requestId: "4", action });

test("wire retains exact u64 selectors, scopes and correlated complete purchase receipts", () => {
  const op = operation({ sequence: "18446744073709551615", intent: intent({ request: { itemIndex: "18446744073709551615", count: 1, panelType: 0 } }) });
  const command = request({ kind: "purchase", operation: op });
  const paid = receipt(op);
  const frame = ownerFrame(command, { kind: "purchase", receipt: paid, replayed: false }, world(), producer({ serverRevision: "2" }));
  assert.deepEqual(parsePurchaseFrame(JSON.stringify(frame), command).reply.receipt, paid);
  for (const bad of [1, "01", "+1", "-1", "18446744073709551616"]) assert.throws(() => purchaseOperation({ ...op, sequence: bad }));
});

test("wire refuses wrong action, request ID, operation, admitted count and revision", () => {
  const op = operation(), command = request({ kind: "purchase", operation: op });
  const valid = ownerFrame(command, { kind: "purchase", receipt: receipt(op), replayed: false }, world(), producer({ serverRevision: "2" }));
  for (const change of [f => f.requestId = "5", f => f.reply.kind = "quote", f => f.reply.receipt.entry.operation.sequence = "2", f => f.reply.receipt.entry.outcome.committed.admittedCount = 2, f => f.authority.serverRevision = "1", f => delete f.authority, f => f.snapshot = null, f => f.extra = true]) {
    const frame = structuredClone(valid); change(frame); assert.throws(() => parsePurchaseFrame(JSON.stringify(frame), command));
  }
});

test("wire rejects duplicate escaped receipt fields and does not accept fabricated enum alternatives", () => {
  const command = request({ kind: "purchase", operation: operation() });
  const valid = JSON.stringify(ownerFrame(command, { kind: "purchase", receipt: receipt(), replayed: false }, world(), producer({ serverRevision: "2" })));
  assert.throws(() => parsePurchaseFrame(valid.replace('"charged":40', '"charged":40,"char\\u0067ed":1'), command));
  assert.throws(() => purchaseReceipt({ ...receipt(), entry: { ...receipt().entry, outcome: { rejected: { request: operation().intent.request, reason: "notEnoughGold" } } } }, operation()));
});

test("a known receipt without a complete pair is distinct from an applied outcome", () => {
  const command = request({ kind: "query", operation: operation() });
  const frame = ownerFrame(command, { kind: "recovery", receipt: receipt() }, null, null);
  assert.equal(parsePurchaseFrame(JSON.stringify(frame), command).snapshot, null);
  const missing = ownerFrame(command, { kind: "recovery", receipt: null }, null, null);
  assert.equal(parsePurchaseFrame(JSON.stringify(missing), command).reply.receipt, null);
  delete missing.reply.receipt;
  assert.throws(() => parsePurchaseFrame(JSON.stringify(missing), command));
});
