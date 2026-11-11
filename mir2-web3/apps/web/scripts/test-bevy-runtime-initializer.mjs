import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../lib/bevy-runtime-initializer.ts", import.meta.url), "utf8");
function load(wasm = WebAssembly) {
  const exports = {};
  const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } });
  new Function("exports", "WebAssembly", compiled.outputText)(exports, wasm);
  return exports.initializeBevyRuntime;
}
const initialize = load();
const binary = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);
const response = (type = "application/wasm") => new Response(binary, { headers: { "content-type": type } });
const flush = () => new Promise(resolve => setImmediate(resolve));

test("binary compilation overlaps a pending script import and canvas gates initialization", async () => {
  let resolveScript, resolveCanvas, input;
  const events = [], runtime = { async default(value) { input = value; events.push("initialize"); } };
  const pending = initialize({
    loadModule: () => { events.push("script"); return new Promise(resolve => { resolveScript = resolve; }); },
    wasmUrl: "https://assets.example.test/runtime.wasm", timeoutMs: 120000,
    fetchImpl: async () => { events.push("binary"); return response(); },
    waitForCanvas: () => { events.push("canvas"); return new Promise(resolve => { resolveCanvas = resolve; }); },
  });
  assert.deepEqual(events, ["script", "binary"]);
  await flush(); assert.equal(input, undefined);
  resolveScript(runtime);
  while (!resolveCanvas) await flush();
  assert.equal(input, undefined); resolveCanvas();
  assert.equal(await pending, runtime); assert(input.module_or_path instanceof WebAssembly.Module);
  assert.deepEqual(events, ["script", "binary", "canvas", "initialize"]);
});

test("wrong MIME and missing streaming support compile the original single response", async () => {
  for (const [wasm, type] of [[WebAssembly, "application/octet-stream"], [{ compile: WebAssembly.compile }, "application/wasm"]]) {
    let requests = 0, initialized;
    await load(wasm)({ loadModule: async () => ({ async default(input) { initialized = input; } }),
      wasmUrl: "https://assets.example.test/runtime.wasm", timeoutMs: 120000, waitForCanvas: async () => {},
      fetchImpl: async () => { requests++; return response(type); } });
    assert.equal(requests, 1); assert(initialized.module_or_path instanceof WebAssembly.Module);
  }
});

test("an import failure aborts the independent binary fetch", async () => {
  let signal, initialized = false;
  await assert.rejects(initialize({ loadModule: async () => { throw new Error("script unavailable"); },
    wasmUrl: "https://assets.example.test/runtime.wasm", timeoutMs: 120000,
    waitForCanvas: async () => { initialized = true; },
    fetchImpl: request => { signal = request.signal; return new Promise((_, reject) => signal.addEventListener("abort", () => reject(signal.reason), { once: true })); },
  }), /script unavailable/);
  assert(signal.aborted); assert.equal(initialized, false);
});

test("unavailable or invalid binaries never initialize a runtime", async () => {
  for (const result of [new Response(null, { status: 503 }), new Response("broken", { headers: { "content-type": "application/wasm" } })]) {
    let canvas = false, signal;
    await assert.rejects(initialize({ loadModule: async () => ({ async default() { assert.fail("invalid binary initialized"); } }),
      wasmUrl: "https://assets.example.test/runtime.wasm", timeoutMs: 120000,
      waitForCanvas: async () => { canvas = true; }, fetchImpl: async request => { signal = request.signal; return result; },
    }));
    assert.equal(canvas, false); assert(signal.aborted);
  }
});

test("the existing 120-second budget cancels a stalled binary", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let signal;
  const pending = initialize({ loadModule: async () => ({}), wasmUrl: "https://assets.example.test/runtime.wasm", timeoutMs: 120000,
    waitForCanvas: async () => {}, fetchImpl: request => { signal = request.signal; return new Promise((_, reject) => signal.addEventListener("abort", () => reject(signal.reason), { once: true })); },
  });
  const rejected = assert.rejects(pending, error => error.name === "AbortError");
  t.mock.timers.tick(119999); assert.equal(signal.aborted, false);
  t.mock.timers.tick(1); await rejected; assert(signal.aborted);
});

test("a stalled script also exits after the binary has finished compiling", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let finishImport, signal, settled = false, error, initialized = false;
  const pending = initialize({
    loadModule: () => new Promise(resolve => { finishImport = resolve; }),
    wasmUrl: "https://assets.example.test/runtime.wasm", timeoutMs: 120000,
    fetchImpl: async request => { signal = request.signal; return response(); },
    waitForCanvas: async () => { initialized = true; },
  });
  const observed = pending.then(() => { settled = true; }, value => { settled = true; error = value; });
  await flush();
  t.mock.timers.tick(119999); await flush(); assert.equal(settled, false);
  t.mock.timers.tick(1); await flush();
  assert.equal(settled, true, "the script import must not retain a pending initialization forever");
  await observed; assert.equal(error.name, "AbortError"); assert(signal.aborted);
  finishImport({ async default() { initialized = true; } }); await flush();
  assert.equal(initialized, false, "a late module cannot initialize an expired attempt");
});

test("actual page loader shares pending/successful work and retries a failed URL", async () => {
  const page = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
  const ast = ts.createSourceFile("page.tsx", page, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const fn = ast.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === "loadBevyRuntimeModule");
  assert(fn);
  const compiled = ts.transpileModule("const bevyRuntimeModuleLoads=new Map();\n" + fn.getText(ast), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } });
  let requests = 0, resolve;
  const actual = new Function("getBevyRuntimePackageForBackend", "createBevyRuntimeUrls", "initializeBevyRuntimeModule", "BEVY_RUNTIME_ASSET_BASE_URL", compiled.outputText + "\nreturn loadBevyRuntimeModule;")(
    () => ({ id: "webgpu" }), () => ({ moduleUrl: "https://assets.example.test/runtime.js", wasmUrl: "https://assets.example.test/runtime.wasm" }),
    () => { requests++; return requests === 1 ? Promise.reject(new Error("offline")) : new Promise(done => { resolve = done; }); }, "https://assets.example.test",
  );
  const startup = { manifest: { version: "version" } };
  await assert.rejects(actual("webgpu", startup), /offline/);
  const second = actual("webgpu", startup), third = actual("webgpu", startup);
  assert.equal(requests, 2); resolve({ initialized: true });
  assert.deepEqual(await second, await third);
  await actual("webgpu", startup); assert.equal(requests, 2);
});
