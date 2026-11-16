import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

function load(path) {
  const module = { exports: {} };
  const js = ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText;
  new Function("module", "exports", js)(module, module.exports);
  return module.exports;
}
const { compileBevyRuntimeBinary, initializeBevyRuntime } = load("../lib/bevy-runtime-initializer.ts");
const { BevyRuntimePreparation } = load("../lib/bevy-runtime-preparation.ts");
const binary = Uint8Array.from(Buffer.from("0061736d010000000105016000017f03020100070a0106616e7377657200000a06010400412a0b", "hex"));
const key = "https://assets.example.test/bevy-runtime/v/fixed/pkg-webgpu/mir2_bevy_runtime.js";
const turn = () => new Promise(setImmediate);
const deferred = () => { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; };

function fixture({ delayedHeaders = false, invalid = false } = {}) {
  const headers = deferred(), requests = [], events = [];
  let stream, imports = 0, initialized = 0, answer;
  const response = () => new Response(invalid ? new Uint8Array([1, 2, 3]) : new ReadableStream({
    start(controller) { stream = controller; controller.enqueue(binary.subarray(0, 8)); },
    cancel() { events.push("body-cancelled"); },
  }), { headers: { "content-type": "application/wasm" } });
  const preparation = new BevyRuntimePreparation(key, (signal, onResponseHeaders) => compileBevyRuntimeBinary({
    wasmUrl: "https://app.example.test/fixed.wasm", timeoutMs: 120000, ownerSignal: signal,
    onResponseHeaders, onDownloadAttempt: event => events.push(event),
    fetchImpl: async request => { requests.push(request); if (delayedHeaders) await headers.promise; return response(); },
  }));
  const consume = (owner = new AbortController(), requestedKey = key) => initializeBevyRuntime({
    wasmUrl: "https://unused.example.test/fixed.wasm", timeoutMs: 120000, ownerSignal: owner.signal,
    getPreparedBinary: signal => preparation.take(requestedKey, signal, () => events.push("consumer-headers")),
    loadModule: async () => { imports++; return { async default({ module_or_path }) {
      initialized++; answer = (await WebAssembly.instantiate(module_or_path)).exports.answer();
    } }; },
    waitForCanvas: async () => events.push("canvas"),
    fetchImpl: async () => { throw Error("A prepared package must not cause a second GET"); },
  });
  return { preparation, requests, events, headers, consume,
    finish: () => { stream.enqueue(binary.subarray(8)); stream.close(); },
    stats: () => ({ imports, initialized, answer }),
  };
}

test("selection streams and compiles without glue, then game adopts the same in-flight body", async () => {
  const f = fixture();
  try {
    await turn(); assert.equal(f.requests.length, 1); assert.equal(f.stats().imports, 0);
    const pending = f.consume(); await turn(); assert.equal(f.stats().initialized, 0);
    assert.equal(f.events.filter(e => e === "consumer-headers").length, 1);
    f.finish(); await pending;
    assert.deepEqual(f.stats(), { imports: 1, initialized: 1, answer: 42 });
    assert.equal(f.requests.length, 1);
  } finally { f.preparation.dispose(); }
});

test("a completed selection compilation remains reusable without retaining a network reader", async () => {
  const f = fixture();
  try {
    await turn(); f.finish();
    const compiled = await f.preparation.take(key, new AbortController().signal);
    assert(compiled instanceof WebAssembly.Module); assert.equal(f.stats().imports, 0);
    assert.equal(f.events.filter(e => e?.state === "completed").length, 1);
    await f.consume(); assert.equal(f.stats().answer, 42); assert.equal(f.requests.length, 1);
  } finally { f.preparation.dispose(); }
});

test("cancel before headers aborts selection and suppresses late header admission", async () => {
  const f = fixture({ delayedHeaders: true });
  try {
    await turn(); let called = 0;
    const pending = f.preparation.take(key, new AbortController().signal, () => called++).catch(e => e);
    f.preparation.dispose(); assert(f.requests[0].signal.aborted);
    assert.equal((await pending).name, "AbortError");
    f.headers.resolve(); await turn(); assert.equal(called, 0);
    assert.equal(f.preparation.take(key, new AbortController().signal), undefined);
    assert.equal(f.stats().initialized, 0);
  } finally { f.preparation.dispose(); }
});

test("retiring the game consumer also cancels the selection body, with no late initializer", async () => {
  const f = fixture();
  try {
    await turn(); const owner = new AbortController(), pending = f.consume(owner).catch(e => e);
    await turn(); owner.abort(); assert.equal((await pending).name, "AbortError");
    await turn(); assert(f.requests[0].signal.aborted); assert.equal(f.stats().initialized, 0);
    assert.equal(f.requests.length, 1); assert(f.events.includes("body-cancelled"));
  } finally { f.preparation.dispose(); }
});

test("invalid prepared bytes stay terminal on adoption without a hidden fresh download", async () => {
  const f = fixture({ invalid: true });
  try {
    await turn(); await assert.rejects(f.consume(), e => e instanceof WebAssembly.CompileError);
    assert.equal(f.requests.length, 1); assert.equal(f.stats().initialized, 0);
  } finally { f.preparation.dispose(); }
});

test("changing the immutable package discards only its obsolete preparation", async () => {
  const f = fixture();
  try {
    await turn(); assert.equal(f.preparation.take(key.replace("pkg-webgpu", "pkg-webgl2"), new AbortController().signal), undefined);
    await turn(); assert(f.requests[0].signal.aborted); assert.equal(f.stats().imports, 0);
    assert.equal(f.preparation.take(key, new AbortController().signal), undefined);
  } finally { f.preparation.dispose(); }
});

test("an already retired consumer cannot take or cancel someone else's preparation", async () => {
  const f = fixture();
  try {
    await turn(); const owner = new AbortController(); owner.abort();
    assert.throws(() => f.preparation.take(key, owner.signal), e => e.name === "AbortError");
    assert.equal(f.requests[0].signal.aborted, false); f.finish(); await f.consume();
    assert.equal(f.stats().answer, 42);
  } finally { f.preparation.dispose(); }
});

test("a header observer exception cannot alter the compiled body or initialize twice", async () => {
  const f = fixture({ delayedHeaders: true });
  try {
    await turn(); const pending = f.preparation.take(key, new AbortController().signal, () => { throw Error("observer"); });
    f.headers.resolve(); await turn(); f.finish(); await pending; await f.consume();
    assert.equal(f.requests.length, 1); assert.equal(f.stats().initialized, 1);
  } finally { f.preparation.dispose(); }
});

const pageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const ast = ts.createSourceFile("page.tsx", pageSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
let selectionEffect;
const visit = node => {
  if (ts.isCallExpression(node) && node.expression.getText(ast) === "useEffect" && node.arguments[0]?.name?.text === "prepareRuntimeAtCharacterSelection") selectionEffect = node;
  ts.forEachChild(node, visit);
};
visit(ast); assert(selectionEffect);

function actualSelection({ screen = "select", retained = true, mode = "eager", allowed = true, backend = "webgpu", packageExists = true, booted = false, poisoned = false, existing = false, pendingSupport = null } = {}) {
  const requests = [], events = [], ref = { current: null }, budget = new Set();
  const env = {
    retainRuntimePreparation: retained, screenRef: { current: screen }, runtimeBinaryPreparationRef: ref,
    window: { location: { search: "" }, matchMedia: () => ({ matches: false }), __mir2BevyRuntimeBooted: booted, __mir2BevyRuntimeBootAttempted: poisoned },
    navigator: { maxTouchPoints: 0, userAgent: "Chrome" }, URLSearchParams,
    clientProfile: { layout: "desktop", input: "keyboardMouse" },
    bevyRuntimeStartup: { runtimeAllowed: allowed, manifest: { version: "fixed" } },
    resolveBevyRuntimeBootDecision: () => ({ mode }), detectBevyRuntimeSupport: () => pendingSupport?.promise ?? Promise.resolve({}),
    selectBevyRuntimeBackend: () => backend,
    getBevyRuntimePackageForBackend: () => packageExists ? { id: "webgpu" } : null,
    createBevyRuntimeUrls: (_version, _id, base) => ({ moduleUrl: key, wasmUrl: (base ?? "https://app.example.test") + "/fixed.wasm" }),
    BEVY_RUNTIME_ASSET_BASE_URL: "https://assets.example.test", BEVY_RUNTIME_BOOT_TIMEOUT_MS: 120000,
    BevyRuntimePreparation, runtimeDownloadRecoveryRef: { current: budget },
    bevyRuntimeModuleLoads: new Map(existing ? [[key, {}]] : []),
    markMir2CacheMilestone: (name, detail) => events.push({ name, detail }),
    compileBevyRuntimeBinary: options => compileBevyRuntimeBinary({ ...options, fetchImpl: async request => {
      requests.push(request); return new Response(binary, { headers: { "content-type": "application/wasm" } });
    } }),
  };
  const js = ts.transpileModule(selectionEffect.arguments[0].getText(ast), { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  const cleanup = new Function(...Object.keys(env), js + "\nreturn prepareRuntimeAtCharacterSelection();")(...Object.values(env));
  return { env, requests, events, ref, cleanup: () => cleanup?.() };
}

test("actual Page preparation runs at selection, survives the game transition and does not start fonts or HUD", async () => {
  const f = actualSelection();
  try {
    await turn(); assert(f.ref.current); f.env.screenRef.current = "game";
    const compiled = await f.ref.current.take(key, new AbortController().signal);
    assert.equal((await WebAssembly.instantiate(compiled)).exports.answer(), 42);
    assert.equal(f.requests.length, 1); assert.equal(f.events.filter(e => e.name === "bevyRuntimePreparationStart").length, 1);
    assert(!f.events.some(e => /Font|Ready|Foreground|Warm/.test(e.name)));
    const dependencies = selectionEffect.arguments[1].elements.map(e => e.getText(ast));
    assert(dependencies.includes("retainRuntimePreparation")); assert(!dependencies.includes("screen"));
  } finally { f.cleanup(); assert.equal(f.ref.current, null); }
});

test("actual Page skips login, direct-game entry, disabled/touch/invalid packages, missing GPU and existing runtime loads", async () => {
  for (const options of [{ screen: "login", retained: false }, { screen: "openingLogin", retained: false }, { screen: "game" },
    { mode: "disabled" }, { mode: "compatibility" }, { allowed: false }, { backend: null }, { packageExists: false }, { booted: true }, { poisoned: true }, { existing: true }]) {
    const f = actualSelection(options);
    try { await turn(); assert.equal(f.requests.length, 0, JSON.stringify(options)); assert.equal(f.ref.current, null); }
    finally { f.cleanup(); }
  }
});

test("a late capability result after leaving selection cannot create a preparation", async () => {
  const pendingSupport = deferred(), f = actualSelection({ pendingSupport });
  f.cleanup(); pendingSupport.resolve({}); await turn();
  assert.equal(f.requests.length, 0); assert.equal(f.ref.current, null); assert.equal(f.events.length, 0);
});
