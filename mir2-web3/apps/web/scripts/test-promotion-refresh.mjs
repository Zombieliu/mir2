import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const temporary = mkdtempSync(path.join(os.tmpdir(), "mir2-promotion-refresh-"));
after(() => {
  assert.equal(path.dirname(path.resolve(temporary)), path.resolve(os.tmpdir()));
  assert.ok(path.basename(temporary).startsWith("mir2-promotion-refresh-"));
  rmSync(temporary, { recursive: true, force: true });
});
const source = fileURLToPath(new URL("../app/promotions/promotion-refresh.ts", import.meta.url));
const program = ts.createProgram([source], {
  target: ts.ScriptTarget.ES2022,
  module: ts.ModuleKind.CommonJS,
  strict: true,
  types: [],
  lib: ["lib.es2022.d.ts", "lib.dom.d.ts"],
  outDir: temporary,
  noEmitOnError: true,
});
const diagnostics = ts.getPreEmitDiagnostics(program);
assert.equal(diagnostics.length, 0, ts.formatDiagnosticsWithColorAndContext(diagnostics, {
  getCurrentDirectory: () => process.cwd(),
  getCanonicalFileName: file => file,
  getNewLine: () => "\n",
}));
assert.equal(program.emit().emitSkipped, false);
const { createPromotionRefresh, PROMOTION_REFRESH_INTERVAL_MS } =
  createRequire(import.meta.url)(path.join(temporary, "promotion-refresh.js"));

function scheduler() {
  let sequence = 0;
  const pending = new Map();
  const history = new Map();
  return {
    pending,
    schedule(callback, delayMs) {
      const handle = ++sequence;
      const entry = { callback, delayMs };
      pending.set(handle, entry);
      history.set(handle, entry);
      return handle;
    },
    cancel(handle) { pending.delete(handle); },
    fire(handle = pending.keys().next().value) {
      const entry = pending.get(handle);
      assert.ok(entry, "an armed timer must exist");
      pending.delete(handle);
      entry.callback();
    },
    fireCancelled(handle) { history.get(handle).callback(); },
    get scheduledCount() { return sequence; },
  };
}

function deferred() {
  let resolve, reject;
  const promise = new Promise((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
const settle = async () => { await new Promise(resolve => setImmediate(resolve)); };

test("creation waits ten seconds, and a completed read arms the next interval", async () => {
  const clock = scheduler();
  let reads = 0;
  const controller = createPromotionRefresh({ ...clock, canRefresh: () => true,
    refresh: async () => { reads++; } });
  assert.equal(reads, 0, "the app's initial fetch must not be duplicated");
  assert.equal(clock.pending.size, 1);
  assert.equal([...clock.pending.values()][0].delayMs, 10_000);
  assert.equal(PROMOTION_REFRESH_INTERVAL_MS, 10_000);
  clock.fire();
  assert.equal(reads, 1);
  assert.equal(clock.pending.size, 0, "no timer competes with the active read");
  await settle();
  assert.equal(clock.pending.size, 1);
  controller.stop();
});

test("a slow request plus a focus/visibility burst admits exactly one read", async () => {
  const clock = scheduler();
  const pending = deferred();
  let reads = 0;
  const controller = createPromotionRefresh({ ...clock, canRefresh: () => true,
    refresh: () => { reads++; return pending.promise; } });
  controller.request();
  for (let i = 0; i < 100; i++) controller.request();
  assert.equal(reads, 1);
  assert.equal(clock.pending.size, 0);
  pending.resolve();
  await settle();
  assert.equal(reads, 1, "burst events do not queue an immediate extra read");
  assert.equal(clock.pending.size, 1);
  controller.stop();
});

for (const blockedBy of ["hidden", "busy", "loading", "walletOpen"]) {
  test(`${blockedBy} skips reads until eligibility returns`, async () => {
    const clock = scheduler();
    let blocked = true;
    let reads = 0;
    const controller = createPromotionRefresh({ ...clock, canRefresh: () => !blocked,
      refresh: async () => { reads++; } });
    controller.request();
    clock.fire();
    assert.equal(reads, 0);
    assert.equal(clock.pending.size, 1, "a skipped interval keeps polling eligibility");
    blocked = false;
    controller.request();
    assert.equal(reads, 1);
    await settle();
    controller.stop();
  });
}

test("a cancelled timer callback cannot race a newly scheduled interval", async () => {
  const clock = scheduler();
  let reads = 0;
  const controller = createPromotionRefresh({ ...clock, canRefresh: () => true,
    refresh: async () => { reads++; } });
  const original = clock.pending.keys().next().value;
  controller.request();
  await settle();
  const next = clock.pending.keys().next().value;
  clock.fireCancelled(original);
  assert.equal(reads, 1);
  assert.equal(clock.pending.size, 1);
  assert.ok(clock.pending.has(next));
  controller.stop();
});

for (const outcome of ["resolve", "reject"]) {
  test(`stop aborts an in-flight read; its late ${outcome} never rearms`, async () => {
    const clock = scheduler();
    const pending = deferred();
    let signal;
    const controller = createPromotionRefresh({ ...clock, canRefresh: () => true,
      refresh: value => { signal = value; return pending.promise; } });
    controller.request();
    assert.equal(signal.aborted, false);
    controller.stop();
    controller.stop();
    assert.equal(signal.aborted, true);
    pending[outcome](outcome === "reject" ? new Error("late abort rejection") : undefined);
    await settle();
    controller.request();
    assert.equal(clock.pending.size, 0);
    assert.equal(clock.scheduledCount, 1);
  });
}

test("a rejected refresh is handled and allows the following interval", async () => {
  const clock = scheduler();
  let reads = 0;
  const controller = createPromotionRefresh({ ...clock, canRefresh: () => true,
    refresh: () => { reads++; return Promise.reject(new Error("offline")); } });
  assert.doesNotThrow(() => controller.request());
  await settle();
  assert.equal(clock.pending.size, 1);
  clock.fire();
  await settle();
  assert.equal(reads, 2);
  controller.stop();
});

test("synchronous refresh or eligibility failures cannot escape request", async () => {
  const clock = scheduler();
  let eligibilityFails = true;
  let reads = 0;
  const controller = createPromotionRefresh({ ...clock,
    canRefresh: () => { if (eligibilityFails) throw new Error("disposed view"); return true; },
    refresh: () => { reads++; throw new Error("synchronous failure"); } });
  assert.doesNotThrow(() => controller.request());
  assert.equal(reads, 0);
  eligibilityFails = false;
  assert.doesNotThrow(() => controller.request());
  await settle();
  assert.equal(reads, 1);
  assert.equal(clock.pending.size, 1);
  controller.stop();
});

test("stop before the first interval retires both queued and future requests", () => {
  const clock = scheduler();
  let reads = 0;
  const controller = createPromotionRefresh({ ...clock, canRefresh: () => true,
    refresh: async () => { reads++; } });
  const timer = clock.pending.keys().next().value;
  controller.stop();
  clock.fireCancelled(timer);
  controller.request();
  assert.equal(reads, 0);
  assert.equal(clock.pending.size, 0);
});
