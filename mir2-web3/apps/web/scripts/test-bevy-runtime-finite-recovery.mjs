import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../lib/bevy-runtime-initializer.ts", import.meta.url), "utf8");
function load(wasm = WebAssembly) {
  const exports = {};
  const code = ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText;
  new Function("exports", "WebAssembly", code)(exports, wasm);
  return exports.initializeBevyRuntime;
}
const initialize = load();
const binary = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);
const urls = ["https://app.example.test/pinned/runtime.wasm", "https://assets.example.test/pinned/runtime.wasm"];
const ok = () => new Response(binary, { headers: { "content-type": "application/wasm" } });
const turn = () => new Promise(resolve => setImmediate(resolve));
const turns = async () => { for (let n = 0; n < 4; n++) await turn(); };
const tick = async (t, ms) => { t.mock.timers.tick(ms); await turns(); };
const observe = promise => promise.then(value => ({ value }), error => ({ error }));
function settings(extra = {}) {
  return { loadModule: async () => ({}), wasmUrl: urls[0], wasmFallbackUrls: [urls[1]],
    timeoutMs: 120_000, waitForCanvas: async () => {}, ...extra };
}

test("R2 header timeout and two zero-byte idle bodies recover in one extra group before one initialization", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const requests = [], signals = [], events = [];
  let imports = 0, canvases = 0, starts = 0, defaults = 0, spent = 0, cancelledBodies = 0;
  const runtime = { async default({ module_or_path }) { defaults++; assert(module_or_path instanceof WebAssembly.Module); } };
  const result = initialize(settings({
    loadModule: async () => { imports++; return runtime; },
    waitForCanvas: async () => { canvases++; },
    onInitializationStarted: () => { starts++; },
    takeRecovery: () => { spent++; return true; },
    onDownloadAttempt: event => events.push(event),
    fetchImpl: request => {
      requests.push(request.url); signals.push(request.signal);
      if (requests.length === 1) return new Promise((_, reject) => request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true }));
      if (requests.length < 4) return new Promise(resolve => setTimeout(() => resolve(new Response(new ReadableStream({
        cancel() { cancelledBodies++; },
      }), { headers: { "content-type": "application/wasm", "content-encoding": "gzip" } })), requests.length === 2 ? 4509 : 3215));
      assert(signals.slice(0, 3).every(signal => signal.aborted));
      assert.equal(cancelledBodies, 2); assert.equal(defaults, 0); return Promise.resolve(ok());
    },
  }));
  await turns(); await tick(t, 7999); assert.equal(requests.length, 1);
  await tick(t, 1); assert.equal(requests.length, 2);
  await tick(t, 4509); await tick(t, 14999); assert.equal(requests.length, 2);
  await tick(t, 1); assert.equal(requests.length, 3);
  await tick(t, 3215); await tick(t, 15000);
  assert.equal(spent, 1, "owner budget is charged before the cooldown");
  assert.deepEqual([canvases, starts, defaults], [0, 0, 0]);
  await tick(t, 999); assert.equal(requests.length, 3);
  await tick(t, 1); assert.equal(await result, runtime);
  assert.deepEqual(requests, [urls[0], urls[1], urls[1], urls[0]]);
  assert.deepEqual([imports, canvases, starts, defaults, spent], [1, 1, 1, 1, 1]);
  assert.deepEqual(events.filter(e => e.state === "failed").map(e => [e.attempt, e.phase, e.reason, e.bytes]),
    [[1, "headers", "headers-timeout", 0], [2, "body", "body-idle", 0], [3, "body", "body-idle", 0]]);
  assert.deepEqual(events.at(-1), { attempt: 4, sourceIndex: 0, retry: true, state: "completed", phase: "compile",
    bytes: binary.length, status: 200, reason: undefined, retryable: undefined });
});

test("an always-granting owner cannot authorize a third transport group", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let calls = 0, spent = 0, imports = 0;
  const events = [];
  const result = observe(initialize(settings({ loadModule: async () => { imports++; return {}; },
    takeRecovery: () => { spent++; return true; }, onDownloadAttempt: e => events.push(e),
    fetchImpl: async () => { calls++; return new Response(null, { status: 503 }); },
  })));
  await turns(); assert.equal(calls, 3); assert.equal(spent, 1);
  await tick(t, 1000); assert.match((await result).error.message, /HTTP 503/);
  await tick(t, 600000); assert.equal(calls, 6); assert.equal(imports, 1); assert.equal(spent, 1);
  assert.deepEqual(events.filter(e => e.state === "started").map(e => e.attempt), [1, 2, 3, 4, 5, 6]);
  assert(events.filter(e => e.attempt > 3).every(e => e.retry));
});

test("missing, denied and throwing recovery guards retain the original bounded requests", async () => {
  for (const guard of [undefined, () => false, () => { throw Error("owner retired"); }]) {
    let calls = 0;
    await assert.rejects(initialize(settings({ takeRecovery: guard,
      fetchImpl: async () => { calls++; return new Response(null, { status: 503 }); },
    })), /HTTP 503/);
    assert.equal(calls, 3);
  }
});

test("retiring an owner during cooldown clears it without another GET", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const owner = new AbortController(), reason = new DOMException("entry retired", "AbortError");
  let calls = 0, spent = 0, defaults = 0;
  const result = observe(initialize(settings({ ownerSignal: owner.signal,
    loadModule: async () => ({ async default() { defaults++; } }),
    takeRecovery: () => { spent++; return true; },
    fetchImpl: async () => { calls++; return new Response(null, { status: 503 }); },
  })));
  await turns(); assert.equal(calls, 3); assert.equal(spent, 1);
  owner.abort(reason); assert.equal((await result).error, reason);
  await tick(t, 130000); assert.equal(calls, 3); assert.equal(defaults, 0);
});

test("retiring an owner cancels the pending body and prevents late bytes from initializing", async () => {
  const owner = new AbortController(), reason = new DOMException("new entry", "AbortError");
  let producer, cancelled = 0, defaults = 0, spent = 0;
  const result = observe(initialize(settings({ ownerSignal: owner.signal,
    loadModule: async () => ({ async default() { defaults++; } }),
    takeRecovery: () => { spent++; return true; },
    fetchImpl: async () => new Response(new ReadableStream({ start(c) { producer = c; }, cancel() { cancelled++; } }),
      { headers: { "content-type": "application/wasm" } }),
  })));
  await turns(); owner.abort(reason); assert.equal((await result).error, reason); await turns();
  assert.equal(cancelled, 1); assert.equal(defaults, 0); assert.equal(spent, 0);
  assert.throws(() => producer.enqueue(binary), TypeError);
});

test("retiring an owner in the second group's headers cancels that exact request", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const owner = new AbortController(); let calls = 0, pendingSignal;
  const result = observe(initialize(settings({ ownerSignal: owner.signal, takeRecovery: () => true,
    fetchImpl: request => {
      calls++; if (calls <= 3) return Promise.resolve(new Response(null, { status: 503 }));
      pendingSignal = request.signal;
      return new Promise((_, reject) => pendingSignal.addEventListener("abort", () => reject(pendingSignal.reason), { once: true }));
    },
  })));
  await turns(); await tick(t, 1000); assert.equal(calls, 4);
  owner.abort(); assert.equal((await result).error.name, "AbortError"); assert(pendingSignal.aborted);
  await tick(t, 130000); assert.equal(calls, 4);
});

test("an already retired owner starts neither script nor binary", async () => {
  const owner = new AbortController(); owner.abort(); let calls = 0, imports = 0;
  await assert.rejects(initialize(settings({ ownerSignal: owner.signal,
    loadModule: async () => { imports++; return {}; }, fetchImpl: async () => { calls++; return ok(); },
  })), error => error === owner.signal.reason);
  assert.deepEqual([calls, imports], [0, 0]);
});

test("owner cancellation while waiting for canvas cannot enter default later", async () => {
  const owner = new AbortController(); let finishCanvas, starts = 0, defaults = 0;
  const result = observe(initialize(settings({ ownerSignal: owner.signal, fetchImpl: async () => ok(),
    loadModule: async () => ({ async default() { defaults++; } }), onInitializationStarted: () => { starts++; },
    waitForCanvas: () => new Promise(resolve => { finishCanvas = resolve; }),
  })));
  while (!finishCanvas) await turn(); owner.abort(); assert.equal((await result).error.name, "AbortError");
  finishCanvas(); await turns(); assert.deepEqual([starts, defaults], [0, 0]);
});

test("owner abort after default entry is indeterminate and cannot authorize a retry", async () => {
  const owner = new AbortController(); let finishDefault, starts = 0, defaults = 0, spent = 0, calls = 0;
  const result = observe(initialize(settings({ ownerSignal: owner.signal,
    loadModule: async () => ({ async default() { defaults++; await new Promise(resolve => { finishDefault = resolve; }); } }),
    onInitializationStarted: () => { starts++; }, takeRecovery: () => { spent++; return true; },
    fetchImpl: async () => { calls++; return ok(); },
  })));
  while (!finishDefault) await turn(); owner.abort(); const error = (await result).error;
  assert.equal(error.runtimeInitializationUncertain, true); assert.notEqual(error.name, "AbortError");
  finishDefault(); await turns(); assert.deepEqual([starts, defaults, spent, calls], [1, 1, 0, 1]);
});

test("retirement in the initialization-start notification prevents default and poisons ownership", async () => {
  const owner = new AbortController(); let defaults = 0, starts = 0;
  const result = await observe(initialize(settings({ ownerSignal: owner.signal, fetchImpl: async () => ok(),
    loadModule: async () => ({ async default() { defaults++; } }),
    onInitializationStarted: () => { starts++; owner.abort(); },
  })));
  assert.equal(result.error.runtimeInitializationUncertain, true); assert.deepEqual([starts, defaults], [1, 0]);
});

test("permanent responses, corrupt bytes and decoded byte limits never spend recovery", async () => {
  for (const scenario of ["403", "404", "invalid", "oversize"]) {
    let calls = 0, spent = 0;
    await assert.rejects(initialize(settings({ takeRecovery: () => { spent++; return true; },
      maxBodyBytes: scenario === "oversize" ? binary.length - 1 : 32 * 1024 * 1024,
      fetchImpl: async () => { calls++;
        return /^\d+$/.test(scenario) ? new Response(null, { status: Number(scenario) })
          : scenario === "invalid" ? new Response(new Uint8Array([1, 2, 3]), { headers: { "content-type": "application/wasm" } }) : ok();
      },
    })));
    assert.equal(calls, /^\d+$/.test(scenario) ? 2 : 1, scenario); assert.equal(spent, 0, scenario);
  }
});

test("a network-like compiler error after EOF cannot authorize an additional group", async () => {
  let calls = 0, spent = 0;
  const compiler = { CompileError: WebAssembly.CompileError, compile: WebAssembly.compile,
    async compileStreaming(body) { await body.arrayBuffer(); throw new TypeError("Failed to fetch while compiling"); } };
  await assert.rejects(load(compiler)(settings({ takeRecovery: () => { spent++; return true; },
    fetchImpl: async () => { calls++; return ok(); },
  })), /while compiling/);
  assert.equal(calls, 3, "only the established first-group classification remains"); assert.equal(spent, 0);
});

test("the fresh-read header deadline is terminal even with an available recovery budget", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let calls = 0, spent = 0;
  const result = observe(initialize(settings({ takeRecovery: () => { spent++; return true; }, fetchImpl: request => {
    calls++; if (calls === 1) return Promise.resolve(new Response(null, { status: 503 }));
    return new Promise((_, reject) => request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true }));
  } })));
  await turns(); await tick(t, 8000);assert.equal(calls,3);
  await tick(t, 120000); assert.equal((await result).error.name, "AbortError");
  assert.equal(calls, 3); assert.equal(spent, 0);
});

test("body absolute and compiler backpressure deadlines cannot spend recovery", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const mode of ["body", "consumer"]) {
    let calls = 0, spent = 0;
    const compiler = mode === "consumer" ? { CompileError: WebAssembly.CompileError, compile: WebAssembly.compile,
      compileStreaming: () => new Promise(() => {}) } : WebAssembly;
    const result = observe(load(compiler)(settings({ bodyTimeoutMs: mode === "body" ? 1000 : 600000,
      takeRecovery: () => { spent++; return true; }, fetchImpl: async () => { calls++;
        return new Response(new ReadableStream({ start(c) { if (mode === "consumer") c.enqueue(binary); } }),
          { headers: { "content-type": "application/wasm" } });
      },
    })));
    await turns(); await tick(t, mode === "body" ? 1000 : 120000);
    assert((await result).error); assert.equal(calls, 1, mode); assert.equal(spent, 0, mode);
  }
});

test("script failure during cooldown cancels the cycle without reimporting", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let rejectScript, calls = 0, imports = 0, spent = 0;
  const failure = new TypeError("Failed to fetch module");
  const result = observe(initialize(settings({ loadModule: () => { imports++; return new Promise((_, reject) => { rejectScript = reject; }); },
    takeRecovery: () => { spent++; return true; }, fetchImpl: async () => { calls++; return new Response(null, { status: 503 }); },
  })));
  await turns(); assert.equal(spent, 1); rejectScript(failure); assert.equal((await result).error, failure);
  await tick(t, 130000); assert.deepEqual([calls, imports], [3, 1]);
});

test("script, canvas and default failures do not turn into transport recovery", async () => {
  for (const stage of ["script", "canvas", "default"]) {
    let calls = 0, imports = 0, spent = 0, defaults = 0, starts = 0;
    const error = new TypeError("Failed to fetch " + stage);
    await assert.rejects(initialize(settings({
      loadModule: () => { imports++; if (stage === "script") throw error;
        return Promise.resolve({ async default() { defaults++; if (stage === "default") throw error; } }); },
      waitForCanvas: async () => { if (stage === "canvas") throw error; },
      onInitializationStarted: () => { starts++; }, takeRecovery: () => { spent++; return true; },
      fetchImpl: async () => { calls++; return ok(); },
    })), failure => failure === error);
    assert.equal(imports, 1, stage); assert.equal(spent, 0, stage); assert(calls <= 1, stage);
    assert.equal(defaults, stage === "default" ? 1 : 0, stage); assert.equal(starts, stage === "default" ? 1 : 0, stage);
  }
});

test("completed initialization detaches owner cancellation without changing the module", async () => {
  const owner = new AbortController(); let defaults = 0;
  const runtime = { async default() { defaults++; } };
  assert.equal(await initialize(settings({ ownerSignal: owner.signal, loadModule: async () => runtime, fetchImpl: async () => ok() })), runtime);
  owner.abort(); await turns(); assert.equal(defaults, 1);
});
