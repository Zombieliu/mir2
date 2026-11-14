import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";
import ts from "typescript";
function load() {
  const source = readFileSync(new URL("../app/promotions/promotion-checkout-recovery.ts", import.meta.url), "utf8");
  const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText;
  const module = { exports: {} }; vm.runInNewContext(compiled, { module, exports: module.exports, crypto }); return module.exports;
}
class SessionStorage {
  constructor(data = new Map()) { this.data = data; }
  get length() { return this.data.size; }
  key(i) { return [...this.data.keys()][i] ?? null; }
  getItem(key) { return this.data.get(key) ?? null; }
  setItem(key, value) { this.data.set(key, value); }
  removeItem(key) { this.data.delete(key); }
}
const scope = { credentialId: "wallet-current-credential", characterIndex: 7 };
const ID = "original-intent-request-0001";
const NEXT = "second-intent-request-0002";
const order = (changes = {}) => ({ id: "usdc_existing", orderRequestId: ID, offerId: "test-usdc-1", state: "paymentSubmitted", ...changes });

test("Windows and legacy payment authorities retain separate unresolved orders for an identical wallet and character", () => {
  const h = load(), storage = new SessionStorage();
  const windows = { ...scope, authority: "windows" };
  h.prepareCheckoutIntent(storage, scope, [], "test-usdc-1", undefined, () => ID);
  assert.equal(h.recoverCheckoutIntents(storage, windows, []).length, 0);
  h.prepareCheckoutIntent(storage, windows, [], "test-usdc-1", undefined, () => NEXT);
  assert.equal(h.recoverCheckoutIntents(storage, scope, [order({ state: "applied" })]).length, 0);
  assert.equal(h.recoverCheckoutIntents(storage, windows, [order({ state: "applied" })])[0].orderRequestId, NEXT);
  assert.equal(h.recoverCheckoutIntents(storage, windows, [order({ state: "applied", orderRequestId: NEXT })]).length, 0);
});

test("a timed-out checkout survives page reload and retries the original request without another order", async () => {
  const data = new Map(), backend = new Map(); const calls = [];
  async function send(helper, storage, original, timeout = false) {
    const intent = helper.prepareCheckoutIntent(storage, scope, [], "test-usdc-1", original, () => original ? NEXT : ID);
    calls.push(intent.orderRequestId);
    if (!backend.has(intent.orderRequestId)) backend.set(intent.orderRequestId, order());
    if (timeout) throw new Error("network result unknown after server commit");
    return backend.get(intent.orderRequestId);
  }
  await assert.rejects(send(load(), new SessionStorage(data), undefined, true), /unknown/);
  const reloaded = load(), storage = new SessionStorage(data);
  const unknown = reloaded.recoverCheckoutIntents(storage, scope, []);
  assert.equal(unknown.length, 1); assert.equal(unknown[0].orderRequestId, ID);
  assert.throws(() => reloaded.prepareCheckoutIntent(storage, scope, [], "different-offer", undefined, () => NEXT), /unresolved/);
  const result = await send(reloaded, storage, unknown[0].orderRequestId);
  assert.equal(result.orderRequestId, ID); assert.deepEqual(calls, [ID, ID]); assert.equal(backend.size, 1);
});

test("successful checkout keeps a recovery intent until its authoritative order is terminal", () => {
  const h = load(), storage = new SessionStorage();
  h.prepareCheckoutIntent(storage, scope, [], "test-usdc-1", undefined, () => ID);
  for (const state of ["prepared", "invoicePending", "awaitingPayment", "paymentSubmitted", "paidPending", "review", "unknown"]) {
    const pending = h.recoverCheckoutIntents(storage, scope, [order({ state })]);
    assert.equal(pending[0].orderRequestId, ID); assert.equal(storage.length, 1);
    assert.throws(() => h.prepareCheckoutIntent(storage, scope, [order({ state })], "test-usdc-2", undefined, () => NEXT), /unresolved/);
  }
});

test("server status restores original offer and request ID even with empty session storage or a removed offer", () => {
  const h = load(), storage = new SessionStorage();
  const existing = order({ offerId: "old-removed-offer", state: "invoicePending" });
  const recovered = h.recoverCheckoutIntents(storage, scope, [existing]);
  assert.equal(recovered[0].orderId, existing.id); assert.equal(recovered[0].offerId, "old-removed-offer");
  const intent = h.prepareCheckoutIntent(new SessionStorage(storage.data), scope, [existing], existing.offerId, ID, () => assert.fail("must not generate a new request"));
  assert.equal(intent.orderRequestId, ID);
});

test("wallet credential, character and offer isolate persisted non-secret identifiers", () => {
  const h = load(), storage = new SessionStorage(); h.rememberCheckoutIntent(storage, scope, { offerId: "test-usdc-1", orderRequestId: ID, token: "must-not-persist", checkoutUrl: "must-not-persist" });
  assert.equal(h.recoverCheckoutIntents(storage, { ...scope, credentialId: "another-wallet" }, []).length, 0);
  assert.equal(h.recoverCheckoutIntents(storage, { ...scope, characterIndex: 8 }, []).length, 0);
  const [key, value] = [...storage.data][0]; assert.ok(key.includes(scope.credentialId)); assert.ok(key.endsWith(":7:test-usdc-1")); assert.equal(value, ID);
  assert.ok(!JSON.stringify([...storage.data]).includes("must-not-persist"));
});

test("terminal status clears only its matching intent; absent status cannot clear an unknown request", () => {
  const h = load(), storage = new SessionStorage(); h.rememberCheckoutIntent(storage, scope, { offerId: "test-usdc-1", orderRequestId: ID });
  assert.equal(h.recoverCheckoutIntents(storage, scope, [order({ orderRequestId: NEXT, state: "applied" })]).length, 1);
  assert.equal(h.recoverCheckoutIntents(storage, scope, []).length, 1);
  assert.equal(h.recoverCheckoutIntents(storage, scope, [order({ state: "applied" })]).length, 0);
  assert.equal(storage.length, 0);
  const fresh = h.prepareCheckoutIntent(storage, scope, [order({ state: "applied" })], "test-usdc-1", undefined, () => NEXT);
  assert.equal(fresh.orderRequestId, NEXT);
});

test("multiple legacy unresolved orders preserve every original ID and an earlier unknown intent", () => {
  const h = load(), storage = new SessionStorage(); h.rememberCheckoutIntent(storage, scope, { offerId: "test-usdc-1", orderRequestId: ID });
  const second = order({ id: "usdc_other", orderRequestId: NEXT });
  const recovered = h.recoverCheckoutIntents(storage, scope, [second]);
  assert.equal(recovered.length, 2); assert.ok(recovered.some(item => item.orderRequestId === ID)); assert.ok(recovered.some(item => item.orderRequestId === NEXT));
  assert.equal(h.prepareCheckoutIntent(storage, scope, [second], second.offerId, NEXT).orderRequestId, NEXT);
  assert.ok(h.recoverCheckoutIntents(storage, scope, [second]).some(item => item.orderRequestId === ID));
});

test("malformed recovery input and unavailable session storage fail before a new checkout", () => {
  const h = load(), storage = new SessionStorage();
  assert.throws(() => h.prepareCheckoutIntent(storage, { ...scope, credentialId: "" }, [], "test-usdc-1", undefined, () => ID));
  assert.throws(() => h.prepareCheckoutIntent(storage, scope, [order({ orderRequestId: undefined })], "test-usdc-1", undefined, () => ID), /recovery/);
  assert.throws(() => h.prepareCheckoutIntent(storage, scope, [], "test-usdc-1", "not-an-original-request"), /original/);
  const denied = new SessionStorage(); denied.setItem = () => { throw new Error("storage denied"); };
  assert.throws(() => h.prepareCheckoutIntent(denied, scope, [], "test-usdc-1", undefined, () => ID), /storage denied/);
});

test("an invalid stale local value is replaced by the exact new request before I/O", () => {
  const h = load(), storage = new SessionStorage(); h.rememberCheckoutIntent(storage, scope, { offerId: "test-usdc-1", orderRequestId: ID });
  const key = [...storage.data.keys()][0]; storage.data.set(key, "corrupt");
  const created = h.prepareCheckoutIntent(storage, scope, [], "test-usdc-1", undefined, () => NEXT);
  assert.equal(created.orderRequestId, NEXT); assert.equal(storage.getItem(key), NEXT);
});
