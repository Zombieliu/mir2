import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { performance } from "node:perf_hooks";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../lib/bevy-runtime-initializer.ts", import.meta.url), "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
});
const moduleExports = {};
new Function("exports", compiled.outputText)(moduleExports);
const initialize = moduleExports.initializeBevyRuntime;
const binary = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);
const APP = "https://app.example.test/pinned/runtime.wasm";
const CDN = "https://cdn.example.test/pinned/runtime.wasm";
const THIRD = "https://third.example.test/pinned/runtime.wasm";
const flush = () => new Promise(resolve => setImmediate(resolve));

async function waitFor(condition, message) {
  const deadline = performance.now() + 5_000;
  while (!condition()) {
    assert(performance.now() < deadline, message);
    await flush();
  }
}

// These fixtures exercise the real streaming compiler and production timeouts.
// Only the clock and upstream transport are controlled; no runtime policy or
// WebAssembly implementation is replaced.
function scenario(t, plan, { sources = [APP, CDN], recovery = false } = {}) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const owner = new AbortController();
  t.after(async () => { owner.abort(); await flush(); });
  const calls = [], events = [], attempts = [];
  let active = 0, peak = 0, imports = 0, canvas = 0, initialized = 0;
  let initializationStarted = 0, recoveryCalls = 0, outcome;
  const event = (kind, record) => {
    const value = { kind, call: record?.number, url: record?.url, order: events.length };
    events.push(value);
    return value.order;
  };
  const finishTransport = record => {
    if (!record.active) return;
    record.active = false;
    active--;
    event("transport-ended", record);
  };
  const runtime = { async default(input) {
    initialized++;
    assert(input.module_or_path instanceof WebAssembly.Module);
    assert.deepEqual(WebAssembly.Module.exports(input.module_or_path), []);
    event("initialize");
  } };
  const fetchImpl = async request => {
    assert.equal(active, 0, "the previous transport must retire before another GET");
    for (const previous of calls) {
      assert(previous.signal.aborted, "a previous attempt's signal must already be aborted");
      if (["partial", "empty"].includes(previous.kind)) {
        assert(previous.readerCancels > 0, "a previous body reader must already be canceled");
        assert(previous.sourceCancels > 0, "cancel must reach the previous upstream body");
      }
    }
    const item = plan[calls.length];
    assert(item, `unexpected GET ${calls.length + 1}: ${request.url}`);
    assert.equal(request.url, item.url);
    const record = { ...item, number: calls.length + 1, url: request.url,
      signal: request.signal, active: true, pendingReads: 0, readBytes: 0,
      readerCancels: 0, sourceCancels: 0 };
    calls.push(record);
    record.fetchOrder = event("fetch", record);
    active++;
    peak = Math.max(peak, active);
    if (item.kind === "headers") {
      return new Promise((_, reject) => {
        request.signal.addEventListener("abort", () => {
          record.abortOrder = event("abort", record);
          finishTransport(record);
          reject(request.signal.reason);
        }, { once: true });
      });
    }
    request.signal.addEventListener("abort", () => {
      record.abortOrder = event("abort", record);
    }, { once: true });
    const stream = new ReadableStream({
      start(controller) {
        record.producer = controller;
        if (item.kind === "partial") controller.enqueue(binary.slice(0, 4));
        if (item.kind === "complete") {
          controller.enqueue(binary.slice());
          controller.close();
        }
      },
      cancel() {
        record.sourceCancels++;
        record.cancelOrder = event("source-canceled", record);
        finishTransport(record);
      },
    });
    const getReader = stream.getReader.bind(stream);
    stream.getReader = (...args) => {
      const reader = getReader(...args);
      return {
        read() {
          record.pendingReads++;
          return reader.read().then(chunk => {
            if (chunk.done) finishTransport(record);
            else record.readBytes += chunk.value.byteLength;
            return chunk;
          }).finally(() => { record.pendingReads--; });
        },
        cancel(reason) {
          record.readerCancels++;
          event("reader-canceled", record);
          return reader.cancel(reason);
        },
      };
    };
    return new Response(stream, { status: item.status ?? 200,
      headers: { "content-type": "application/wasm" } });
  };
  const pending = initialize({
    loadModule: async () => { imports++; return runtime; },
    wasmUrl: sources[0], wasmFallbackUrls: sources.slice(1),
    timeoutMs: 120_000,
    waitForCanvas: async () => { canvas++; },
    fetchImpl, ownerSignal: owner.signal,
    onDownloadAttempt: value => attempts.push(value),
    onInitializationStarted: () => { initializationStarted++; },
    takeRecovery: () => { recoveryCalls++; return recovery; },
  });
  // Observe rejection immediately, including owner cancellation before a caller
  // reaches finish(), so the test never relies on an unhandled rejection.
  void pending.then(value => { outcome = { value }; }, error => { outcome = { error }; });
  const waitCalls = n => waitFor(() => calls.length >= n || outcome,
    `expected GET ${n}; saw ${calls.length}`);
  const advance = async ms => { t.mock.timers.tick(ms); await flush(); };
  const expireHeaders = async (number, budget = 8_000) => {
    await waitCalls(number);
    const record = calls[number - 1];
    assert.equal(record?.kind, "headers");
    await advance(budget - 1);
    assert.equal(record.signal.aborted, false, "headers must retain their full budget");
    assert.equal(calls.length, number);
    await advance(1);
  };
  const expireBody = async number => {
    await waitCalls(number);
    const record = calls[number - 1];
    assert(record && ["partial", "empty"].includes(record.kind));
    const expectedBytes = record.kind === "partial" ? 4 : 0;
    await waitFor(() => record.pendingReads > 0 && record.readBytes === expectedBytes,
      `GET ${number} must have a pending body read after ${expectedBytes} bytes`);
    await advance(14_999);
    assert.equal(record.signal.aborted, false, "positive-byte idle budget must remain 15 seconds");
    assert.equal(calls.length, number);
    await advance(1);
  };
  const finish = async success => {
    await waitFor(() => outcome, "runtime did not settle");
    if (success) {
      assert.ifError(outcome.error);
      assert.equal(outcome.value, runtime);
      assert.deepEqual([imports, canvas, initializationStarted, initialized], [1, 1, 1, 1]);
      assert.equal(attempts.filter(item => item.state === "completed").length, 1);
    } else {
      assert(outcome.error, "runtime must reject");
      assert.deepEqual([imports, canvas, initializationStarted, initialized], [1, 0, 0, 0]);
    }
    assert.equal(active, 0);
    assert.equal(peak, 1, "there must be only one active upstream read");
    assert(calls.every(item => item.signal.aborted));
    await advance(600_000);
    assert.equal(calls.length, plan.length, "settled attempts cannot issue late GETs");
    return outcome.error;
  };
  return { owner, calls, events, attempts, waitCalls, advance, expireHeaders, expireBody, finish,
    get recoveryCalls() { return recoveryCalls; },
    get initialized() { return initialized; } };
}

test("8s App headers then positive CDN body-idle rotates the one retry back to App", async t => {
  const h = scenario(t, [
    { url: APP, kind: "headers" }, { url: CDN, kind: "partial" }, { url: APP, kind: "complete" },
  ]);
  await h.expireHeaders(1);
  await h.expireBody(2);
  await h.finish(true);
  assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, APP]);
  assert.deepEqual(h.attempts.filter(item => item.state === "failed")
    .map(item => [item.sourceIndex, item.phase, item.bytes, item.reason, item.retryable]),
  [[0, "headers", 0, "headers-timeout", true], [1, "body", 4, "body-idle", true]]);
  assert(h.calls[1].cancelOrder < h.calls[2].fetchOrder);
  assert.throws(() => h.calls[1].producer.enqueue(binary), TypeError,
    "a retired CDN producer cannot append to the replacement binary");
  assert.equal(h.recoveryCalls, 0);
});

test("a zero-byte CDN body-idle retains the existing CDN retry order", async t => {
  const h = scenario(t, [
    { url: APP, kind: "headers" }, { url: CDN, kind: "empty" }, { url: CDN, kind: "complete" },
  ]);
  await h.expireHeaders(1);
  await h.expireBody(2);
  await h.finish(true);
  assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, CDN]);
  assert(h.attempts.some(item => item.state === "failed" && item.phase === "body"
    && item.bytes === 0 && item.reason === "body-idle"));
});

test("a final-source HTTP 503 retains the existing last-transient retry", async t => {
  const h = scenario(t, [
    { url: APP, kind: "headers" }, { url: CDN, kind: "http", status: 503 },
    { url: CDN, kind: "complete" },
  ]);
  await h.expireHeaders(1);
  await h.finish(true);
  assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, CDN]);
  assert.equal(h.calls[1].sourceCancels, 1);
  assert(h.attempts.some(item => item.state === "failed" && item.reason === "http"
    && item.status === 503 && item.bytes === 0 && item.retryable));
});

for (const status of [403, 404]) {
  test(`an initial HTTP ${status} source cannot acquire partial-body retry authority`, async t => {
    const h = scenario(t, [
      { url: APP, kind: "http", status }, { url: CDN, kind: "partial" },
      { url: CDN, kind: "complete" },
    ]);
    await h.expireBody(2);
    await h.finish(true);
    assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, CDN]);
    assert(h.attempts.some(item => item.state === "failed" && item.sourceIndex === 0
      && item.status === status && item.retryable === false));
  });

  test(`a final HTTP ${status} must not rotate the later owner recovery group`, async t => {
    const h = scenario(t, [
      { url: APP, kind: "http", status: 503 }, { url: CDN, kind: "http", status },
      { url: APP, kind: "http", status: 503 }, { url: APP, kind: "complete" },
    ], { recovery: true });
    await waitFor(() => h.recoveryCalls === 1, "the existing one recovery group must be requested");
    assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, APP]);
    await h.advance(999);
    assert.equal(h.calls.length, 3);
    await h.advance(1);
    await h.finish(true);
    assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, APP, APP]);
    assert.equal(h.recoveryCalls, 1);
  });
}

test("a failed rotated retry recovers from CDN with one owner grant and at most six GETs", async t => {
  const h = scenario(t, [
    { url: APP, kind: "headers" }, { url: CDN, kind: "partial" },
    { url: APP, kind: "partial" }, { url: CDN, kind: "partial" },
    { url: APP, kind: "partial" }, { url: CDN, kind: "partial" },
  ], { recovery: true });
  await h.expireHeaders(1);
  await h.expireBody(2);
  await h.expireBody(3);
  await waitFor(() => h.recoveryCalls === 1, "the owner must authorize recovery once");
  assert.equal(h.calls.length, 3);
  await h.advance(999);
  assert.equal(h.calls.length, 3, "the original recovery cooldown must remain intact");
  await h.advance(1);
  for (const number of [4, 5, 6]) await h.expireBody(number);
  assert.equal((await h.finish(false)).name, "TimeoutError");
  assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, APP, CDN, APP, CDN]);
  assert.equal(h.recoveryCalls, 1);
  assert.deepEqual(h.attempts.filter(item => item.state === "started")
    .map(item => [item.attempt, item.sourceIndex, item.retry]),
  [[1, 0, false], [2, 1, false], [3, 0, true], [4, 1, true], [5, 0, true], [6, 1, true]]);
});

test("owner cancellation during a partial CDN read prevents the rotated retry", async t => {
  const h = scenario(t, [{ url: APP, kind: "headers" }, { url: CDN, kind: "partial" }],
    { recovery: true });
  await h.expireHeaders(1);
  await h.waitCalls(2);
  await waitFor(() => h.calls[1].readBytes === 4 && h.calls[1].pendingReads > 0,
    "CDN must have supplied only a partial body");
  const cancelled = new Error("route owner retired");
  h.owner.abort(cancelled);
  assert.equal(await h.finish(false), cancelled);
  assert.equal(h.recoveryCalls, 0);
});

test("owner cancellation during the authorized cooldown prevents every recovery GET", async t => {
  const h = scenario(t, [
    { url: APP, kind: "http", status: 503 }, { url: CDN, kind: "partial" },
    { url: APP, kind: "http", status: 503 },
  ], { recovery: true });
  await h.expireBody(2);
  await waitFor(() => h.recoveryCalls === 1, "recovery must be waiting in its cooldown");
  h.owner.abort(new Error("owner retired during recovery"));
  assert.match((await h.finish(false)).message, /owner retired during recovery/);
  assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, APP]);
  assert.equal(h.recoveryCalls, 1);
});

test("three-source rotation chooses the earliest transient source, excluding a denied source", async t => {
  const h = scenario(t, [
    { url: APP, kind: "http", status: 403 }, { url: CDN, kind: "headers" },
    { url: THIRD, kind: "partial" }, { url: CDN, kind: "complete" },
  ], { sources: [APP, CDN, THIRD] });
  await h.expireHeaders(2);
  await h.expireBody(3);
  await h.finish(true);
  assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, THIRD, CDN]);
  assert.equal(h.attempts.find(item => item.attempt === 4).sourceIndex, 1);
});

test("three transient sources rotate to the first source rather than the last failed mirror", async t => {
  const h = scenario(t, [
    { url: APP, kind: "http", status: 503 }, { url: CDN, kind: "headers" },
    { url: THIRD, kind: "partial" }, { url: APP, kind: "complete" },
  ], { sources: [APP, CDN, THIRD] });
  await h.expireHeaders(2);
  await h.expireBody(3);
  await h.finish(true);
  assert.deepEqual(h.calls.map(item => item.url), [APP, CDN, THIRD, APP]);
});

test("permanent denial on every source grants neither retry nor owner recovery", async t => {
  const h = scenario(t, [
    { url: APP, kind: "http", status: 403 }, { url: CDN, kind: "http", status: 404 },
  ], { recovery: true });
  assert.match((await h.finish(false)).message, /HTTP 404/);
  assert.equal(h.recoveryCalls, 0);
  assert.deepEqual(h.calls.map(item => item.url), [APP, CDN]);
});
