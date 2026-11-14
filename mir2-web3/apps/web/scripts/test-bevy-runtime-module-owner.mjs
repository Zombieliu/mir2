import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

function loader(initialize) {
  const source = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
  const ast = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const fn = ast.statements.find(n => ts.isFunctionDeclaration(n) && n.name?.text === "loadBevyRuntimeModule"); assert(fn);
  const js = ts.transpileModule("const bevyRuntimeModuleLoads=new Map();\n" + fn.getText(ast), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText;
  return new Function("getBevyRuntimePackageForBackend", "createBevyRuntimeUrls", "initializeBevyRuntimeModule", "BEVY_RUNTIME_ASSET_BASE_URL", js + "\nreturn loadBevyRuntimeModule;")(
    (_startup, backend) => ({ id: backend }), (version, id, base = "https://app.example.test") => ({ moduleUrl: `${base}/${version}/${id}.js`, wasmUrl: `${base}/${version}/${id}.wasm` }),
    initialize, "https://assets.example.test",
  );
}
const startup = { manifest: { version: "fixed-version" } };
const deferred = () => { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; };
const flush = () => new Promise(setImmediate);

test("owner replacement evicts only its pending source before initialization", async () => {
  const records = [], retired = new AbortController(), active = new AbortController();
  const actual = loader((_js, _wasm, _fallback, options) => {
    const pending = deferred(); records.push({ options, pending }); return pending.promise;
  });
  const first = actual("webgpu", startup, { ownerSignal: retired.signal });
  const firstObserved = first.catch(e => e);
  assert.equal(records.length, 1); retired.abort();
  const second = actual("webgpu", startup, { ownerSignal: active.signal });
  assert.equal(records.length, 2, "new owner does not inherit the retired pending promise");
  records[0].pending.reject(retired.signal.reason); await firstObserved;
  const third = actual("webgpu", startup, { ownerSignal: active.signal });
  assert.equal(records.length, 2, "old failure cannot evict the new owner's work");
  records[1].options.onInitializationStarted(); records[1].pending.resolve({ initialized: true });
  assert.deepEqual(await second, await third); active.abort();
  await actual("webgpu", startup); assert.equal(records.length, 2, "finished glue remains reusable");
});

test("cancellation after glue initialization starts fences every backend until reload", async () => {
  const owner = new AbortController(), pending = deferred(); let calls = 0, options;
  const actual = loader((_js, _wasm, _fallback, value) => { calls++; options = value; return pending.promise; });
  const first = actual("webgpu", startup, { ownerSignal: owner.signal });
  const observed = first.catch(e => e); options.onInitializationStarted(); owner.abort();
  const uncertain = e => e.runtimeInitializationUncertain === true;
  await assert.rejects(actual("webgpu", startup), uncertain);
  await assert.rejects(actual("webgl2", startup), uncertain);
  assert.equal(calls, 1); pending.reject(owner.signal.reason); assert(uncertain(await observed));
  await assert.rejects(actual("webgpu", startup), uncertain); assert.equal(calls, 1);
});

test("any failed default initialization poisons the cache without a second default", async () => {
  let calls = 0;
  const actual = loader(async (_js, _wasm, _fallback, options) => {
    calls++; options.onInitializationStarted(); throw new TypeError("Failed to fetch during default");
  });
  await assert.rejects(actual("webgpu", startup), e => e.runtimeInitializationUncertain === true);
  await assert.rejects(actual("webgl2", startup), e => e.runtimeInitializationUncertain === true);
  assert.equal(calls, 1);
});

test("a known fulfilled initializer survives retirement before its cache callback runs", async () => {
  const owner = new AbortController(), pending = deferred(); let options, calls = 0;
  const actual = loader((_js, _wasm, _fallback, value) => { calls++; options = value; return pending.promise; });
  const first = actual("webgpu", startup, { ownerSignal: owner.signal });
  options.onInitializationStarted(); pending.resolve({ initialized: true }); owner.abort();
  assert.deepEqual(await first, { initialized: true });
  assert.deepEqual(await actual("webgpu", startup), { initialized: true });
  assert.equal(calls, 1, "fully initialized glue remains reusable without another default");
});

test("retired inputs never acquire a cache slot or spend a recovery allowance", async () => {
  let calls = 0, spends = 0;
  const actual = loader(async () => { calls++; return {}; });
  const owner = new AbortController(); owner.abort();
  await assert.rejects(actual("webgpu", startup, { ownerSignal: owner.signal, takeRecovery: () => { spends++; return true; } }), e => e.name === "AbortError");
  assert.equal(calls, 0); assert.equal(spends, 0);
  await actual("webgpu", startup); assert.equal(calls, 1);
});

test("an initialization timeout marker is retained even before the boundary callback arrives", async () => {
  let calls = 0;
  const actual = loader(async () => { calls++; throw Object.assign(new Error("uncertain"), { runtimeInitializationUncertain: true }); });
  await assert.rejects(actual("webgpu", startup), e => e.runtimeInitializationUncertain === true);
  await assert.rejects(actual("webgpu", startup), e => e.runtimeInitializationUncertain === true);
  assert.equal(calls, 1);
});

test("the source-specific budget is offered to the initializer without spending it eagerly", async () => {
  const spent = new Set(), observed = [], records = [];
  const actual = loader(async (...args) => { records.push(args); return {}; });
  await actual("webgpu", startup, { takeRecovery: key => { observed.push(key); if (spent.has(key)) return false; spent.add(key); return true; } });
  assert.deepEqual(observed, []); assert.equal(records[0][3].takeRecovery(), true);
  assert.equal(records[0][3].takeRecovery(), false); assert.deepEqual(observed, [records[0][0], records[0][0]]);
  await flush();
});
