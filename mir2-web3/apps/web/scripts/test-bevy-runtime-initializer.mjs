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
const customBinary = new Uint8Array([...binary, 0, 1, 0]);
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

test("a valid advancing binary can finish beyond the header budget without a second GET", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let source, cancelled=false, requests=0, input;
  const pending=initialize({loadModule:async()=>({async default(value){input=value;}}),
    wasmUrl:'https://assets.example.test/runtime.wasm',timeoutMs:120000,waitForCanvas:async()=>{},
    fetchImpl:async()=>{requests++;return new Response(new ReadableStream({start(c){source=c;},cancel(){cancelled=true;}}),{headers:{'content-type':'application/wasm'}});}});
  const observed=pending.then(value=>({value}),error=>({error}));await flush();
  for(const byte of customBinary){t.mock.timers.tick(12000);if(!cancelled)source.enqueue(new Uint8Array([byte]));await flush();}
  if(!cancelled)source.close();const result=await observed;
  assert.equal(result.error,undefined,'132 seconds of advancing valid bytes must not be treated as a120-second stall');
  assert(input.module_or_path instanceof WebAssembly.Module);assert.equal(requests,1);
});

test("a body without positive bytes stops at the idle boundary, including empty chunks",async t=>{
  t.mock.timers.enable({apis:['setTimeout']});let source,signal,cancelled=false,settled=false;
  const pending=initialize({loadModule:async()=>({}),wasmUrl:'https://assets.example.test/runtime.wasm',timeoutMs:120000,
    waitForCanvas:async()=>{},fetchImpl:async request=>{signal=request.signal;return new Response(new ReadableStream({start(c){source=c;},cancel(){cancelled=true;}}),{headers:{'content-type':'application/wasm'}});}});
  const observed=pending.then(()=>({}),error=>{settled=true;return{error};});await flush();
  t.mock.timers.tick(10000);source.enqueue(new Uint8Array());await flush();
  t.mock.timers.tick(4999);await flush();assert.equal(settled,false);
  t.mock.timers.tick(1);await flush();const stoppedOnTime=settled;
  if(!settled){t.mock.timers.tick(120000);await flush();}if(!cancelled)source.close();const result=await observed;
  assert(stoppedOnTime,'zero-length chunks must not renew the15-second positive-byte deadline');assert.equal(result.error.name,'TimeoutError');assert(signal.aborted);assert(cancelled);
});

test("decoded bytes are capped before runtime initialization without trusting Content-Length",async()=>{
  let initialized=false;
  await assert.rejects(initialize({loadModule:async()=>({async default(){initialized=true;}}),
    wasmUrl:'https://assets.example.test/runtime.wasm',timeoutMs:120000,maxBodyBytes:binary.length,waitForCanvas:async()=>{},
    fetchImpl:async()=>new Response(customBinary,{headers:{'content-type':'application/wasm'}})}),RangeError);
  assert.equal(initialized,false);
});

test("a progressing body still has an absolute lifetime cap",async t=>{
  t.mock.timers.enable({apis:['setTimeout']});let source,cancelled=false,settled=false;
  const pending=initialize({loadModule:async()=>({}),wasmUrl:'https://assets.example.test/runtime.wasm',timeoutMs:120000,bodyTimeoutMs:25000,
    waitForCanvas:async()=>{},fetchImpl:async()=>new Response(new ReadableStream({start(c){source=c;},cancel(){cancelled=true;}}),{headers:{'content-type':'application/wasm'}})});
  const observed=pending.then(()=>({}),error=>{settled=true;return{error};});await flush();
  for(let i=0;i<2;i++){t.mock.timers.tick(10000);source.enqueue(new Uint8Array([customBinary[i]]));await flush();}
  t.mock.timers.tick(5000);await flush();const stoppedOnTime=settled;
  if(!settled){t.mock.timers.tick(120000);await flush();}if(!cancelled)source.close();await observed;
  assert(stoppedOnTime,'positive bytes must not extend the absolute body window');assert(cancelled);
});

test("a failed pinned binary source can use its supplied same-version fallback once",async()=>{
  const sources=[],runtime={async default(){}};
  const loaded=await initialize({loadModule:async()=>runtime,wasmUrl:'https://app.example.test/bevy-runtime/v/pinned/pkg-webgpu/runtime.wasm',
    wasmFallbackUrls:['https://assets.example.test/release/bevy-runtime/v/pinned/pkg-webgpu/runtime.wasm'],timeoutMs:120000,waitForCanvas:async()=>{},
    fetchImpl:async request=>{sources.push(request.url);return sources.length===1?new Response(null,{status:503}):response();}});
  assert.equal(loaded,runtime);assert.equal(sources.length,2);assert(sources.every(x=>x.includes('/bevy-runtime/v/pinned/pkg-webgpu/')));
});

test('actual Page selects versioned app binary with same-package remote fallback and glue',async()=>{
  const page=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ast=ts.createSourceFile('page.tsx',page,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const fn=ast.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text==='loadBevyRuntimeModule');assert(fn);
  const urlsSource=readFileSync(new URL('../lib/bevy-runtime-url.ts',import.meta.url),'utf8'),urlExports={};
  new Function('exports',ts.transpileModule(urlsSource,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText)(urlExports);
  const compiled=ts.transpileModule('const bevyRuntimeModuleLoads=new Map();\n'+fn.getText(ast),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
  for(const id of ['webgpu','webgl2','webgl2-shared']){
    let selected;
    const actual=new Function('getBevyRuntimePackageForBackend','createBevyRuntimeUrls','initializeBevyRuntimeModule','BEVY_RUNTIME_ASSET_BASE_URL',compiled+'\nreturn loadBevyRuntimeModule;')(
      ()=>({id}),urlExports.createBevyRuntimeUrls,async(...args)=>{selected=args;return{};},'https://assets.example.test/immutable-release');
    await actual(id,{manifest:{version:'bevy-fixed'}});
    const suffix='/bevy-runtime/v/bevy-fixed/pkg-'+id+'/';
    assert.equal(selected[0],'https://assets.example.test/immutable-release'+suffix+'mir2_bevy_runtime.js');
    assert.equal(selected[1],suffix+'mir2_bevy_runtime_bg.wasm');
    assert.equal(selected[2],'https://assets.example.test/immutable-release'+suffix+'mir2_bevy_runtime_bg.wasm');
  }
});

test('an unavailable first source yields to its fallback after8 seconds',async t=>{
  t.mock.timers.enable({apis:['setTimeout']});let firstSignal,calls=0,initialized=0;
  const pending=initialize({loadModule:async()=>({async default(){initialized++;}}),wasmUrl:'https://app.example.test/pinned.wasm',
    wasmFallbackUrls:['https://assets.example.test/pinned.wasm'],timeoutMs:120000,waitForCanvas:async()=>{},fetchImpl:request=>{
      calls++;if(calls>1)return Promise.resolve(response());firstSignal=request.signal;
      return new Promise((_,reject)=>firstSignal.addEventListener('abort',()=>reject(firstSignal.reason),{once:true}));}});
  await flush();t.mock.timers.tick(7999);await flush();assert.equal(calls,1);assert.equal(firstSignal.aborted,false);
  t.mock.timers.tick(1);await pending;assert(firstSignal.aborted);assert.equal(calls,2);assert.equal(initialized,1);
});

test('a stalled first source body is canceled before the next binary is read',async t=>{
  t.mock.timers.enable({apis:['setTimeout']});let calls=0,cancelled=0;
  const pending=initialize({loadModule:async()=>({}),wasmUrl:'https://app.example.test/pinned.wasm',wasmFallbackUrls:['https://assets.example.test/pinned.wasm'],
    timeoutMs:120000,waitForCanvas:async()=>{},fetchImpl:async()=>{calls++;if(calls>1){assert.equal(cancelled,1);return response();}
      return new Response(new ReadableStream({cancel(){cancelled++;}}),{headers:{'content-type':'application/wasm'}});}});
  await flush();t.mock.timers.tick(15000);await pending;assert.equal(calls,2);assert.equal(cancelled,1);
});

test('binary progress cannot renew a stalled script import deadline',async t=>{
  t.mock.timers.enable({apis:['setTimeout']});let source,finishImport,cancelled=false,initialized=false;
  const pending=initialize({loadModule:()=>new Promise(resolve=>{finishImport=resolve;}),wasmUrl:'https://assets.example.test/runtime.wasm',
    timeoutMs:120000,waitForCanvas:async()=>{},fetchImpl:async()=>new Response(new ReadableStream({start(c){source=c;},cancel(){cancelled=true;}}),{headers:{'content-type':'application/wasm'}})});
  const observed=pending.then(()=>({}),error=>({error}));await flush();
  for(let i=0;i<10;i++){t.mock.timers.tick(12000);if(!cancelled)source.enqueue(new Uint8Array([customBinary[i]]));await flush();}
  const result=await observed;assert.equal(result.error.name,'AbortError');assert(cancelled);
  finishImport({async default(){initialized=true;}});await flush();assert.equal(initialized,false);
});

test('a streaming compiler which never reads remains bounded',async t=>{
  t.mock.timers.enable({apis:['setTimeout']});let settled=false;
  const pending=load({compile:WebAssembly.compile,compileStreaming:()=>new Promise(()=>{})})({loadModule:async()=>({}),
    wasmUrl:'https://assets.example.test/runtime.wasm',timeoutMs:120000,waitForCanvas:async()=>{},fetchImpl:async()=>response()});
  const observed=pending.then(()=>({}),error=>{settled=true;return{error};});await flush();
  t.mock.timers.tick(119999);await flush();assert.equal(settled,false);t.mock.timers.tick(1);
  const result=await observed;assert.equal(result.error.name,'AbortError');
});

test('a late canvas cannot initialize an expired attempt',async t=>{
  t.mock.timers.enable({apis:['setTimeout']});let canvas,initialized=false;
  const pending=initialize({loadModule:async()=>({async default(){initialized=true;}}),wasmUrl:'https://assets.example.test/runtime.wasm',timeoutMs:120000,
    waitForCanvas:()=>new Promise(resolve=>{canvas=resolve;}),fetchImpl:async()=>response()});
  const observed=pending.then(()=>({}),error=>({error}));while(!canvas)await flush();
  t.mock.timers.tick(120000);const result=await observed;assert.equal(result.error.name,'AbortError');
  canvas();await flush();assert.equal(initialized,false);
});

test('a default initializer which has already started cannot be classified as a retryable download',async t=>{
  t.mock.timers.enable({apis:['setTimeout']});let finishDefault,entered=false;
  const pending=initialize({loadModule:async()=>({async default(){entered=true;await new Promise(resolve=>{finishDefault=resolve;});}}),
    wasmUrl:'https://assets.example.test/runtime.wasm',timeoutMs:120000,waitForCanvas:async()=>{},fetchImpl:async()=>response()});
  const observed=pending.then(()=>({}),error=>({error}));while(!entered)await flush();
  t.mock.timers.tick(120000);const result=await observed;assert.equal(result.error.runtimeInitializationUncertain,true);
  assert.notEqual(result.error.name,'AbortError');finishDefault();await flush();
});

test('invalid or oversized bytes fail closed instead of consulting another source',async()=>{
  for(const [bytes,limit] of [[new Uint8Array([1,2,3,4,5,6,7,8]),32*1024*1024],[customBinary,binary.length]]){
    let calls=0,cancelled=0,initialized=false;const oversize=bytes.length>limit;
    await assert.rejects(initialize({loadModule:async()=>({async default(){initialized=true;}}),wasmUrl:'https://app.example.test/pinned.wasm',
      wasmFallbackUrls:['https://assets.example.test/pinned.wasm'],timeoutMs:120000,maxBodyBytes:limit,waitForCanvas:async()=>{},
      fetchImpl:async()=>{calls++;return new Response(new ReadableStream({start(c){c.enqueue(bytes);if(!oversize)c.close();},cancel(){cancelled++;}}),{headers:{'content-type':'application/wasm'}});}
    }),error=>error instanceof WebAssembly.CompileError||error instanceof RangeError);
    assert.equal(calls,1);assert.equal(initialized,false);assert.equal(cancelled,oversize?1:0);
  }
});

test('two interrupted pinned bodies recover once before a single glue initialization', async () => {
  const urls = ['https://app.example.test/bevy-runtime/v/pinned/pkg-webgpu/runtime.wasm',
    'https://assets.example.test/release/bevy-runtime/v/pinned/pkg-webgpu/runtime.wasm'];
  const requests = [], events = [], signals = [];
  let imports = 0, canvases = 0, initialized = 0;
  const runtime = { async default({ module_or_path }) {
    initialized++; assert(module_or_path instanceof WebAssembly.Module);
  } };
  assert.equal(await initialize({
    loadModule: async () => { imports++; return runtime; }, wasmUrl: urls[0], wasmFallbackUrls: [urls[1]],
    timeoutMs: 120000, waitForCanvas: async () => { canvases++; }, onDownloadAttempt: value => events.push(value),
    fetchImpl: async request => {
      requests.push(request.url); signals.push(request.signal);
      if (requests.length > 2) {
        assert(signals.slice(0, 2).every(signal => signal.aborted));
        assert.equal(initialized, 0); return response();
      }
      return new Response(new ReadableStream({ start(controller) {
        controller.enqueue(binary.slice(0, 4));
        setImmediate(() => controller.error(new TypeError('Failed to fetch: connection reset')));
      } }), { headers: { 'content-type': 'application/wasm' } });
    },
  }), runtime);
  assert.deepEqual(requests, [urls[0], urls[1], urls[1]]);
  assert.deepEqual([imports, canvases, initialized], [1, 1, 1]);
  assert.deepEqual(events.filter(x => x.state === 'failed').map(x => [x.bytes, x.reason, x.retryable]),
    [[4, 'network', true], [4, 'network', true]]);
  assert.deepEqual(events.at(-1), { attempt: 3, sourceIndex: 1, retry: true,
    state: 'completed', phase: 'compile', bytes: binary.length, status: 200, reason: undefined, retryable: undefined });
});

test('a stalled last mirror is canceled at the unchanged idle boundary before its one retry', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let calls = 0, canceled = 0, initialized = 0, source, interruptedSignal;
  const events = [];
  const pending = initialize({ loadModule: async () => ({ async default() { initialized++; } }),
    wasmUrl: 'https://app.example.test/pinned.wasm', wasmFallbackUrls: ['https://assets.example.test/pinned.wasm'],
    timeoutMs: 120000, waitForCanvas: async () => {}, onDownloadAttempt: value => events.push(value),
    fetchImpl: async request => {
      calls++;
      if (calls === 1) return new Response(null, { status: 503 });
      if (calls === 3) { assert(interruptedSignal.aborted); assert.equal(canceled, 1); return response(); }
      interruptedSignal = request.signal;
      return new Response(new ReadableStream({ start(controller) { source = controller; controller.enqueue(binary.slice(0, 4)); },
        cancel() { canceled++; } }), { headers: { 'content-type': 'application/wasm' } });
    },
  });
  while (!source) await flush();
  await flush(); t.mock.timers.tick(14999); await flush(); assert.equal(calls, 2);
  t.mock.timers.tick(1); await pending;
  assert.equal(calls, 3); assert.equal(initialized, 1); assert.equal(canceled, 1);
  assert.throws(() => source.enqueue(binary), TypeError, 'a retired producer cannot supply the recovered binary');
  assert(events.some(x => x.state === 'failed' && x.sourceIndex === 1 && x.reason === 'body-idle' && x.bytes === 4));
});

test('transient exhaustion remains bounded and permanent responses do not add retries', async () => {
  for (const status of [503, 404, 403]) {
    let calls = 0, initialized = 0;
    await assert.rejects(initialize({ loadModule: async () => ({ async default() { initialized++; } }),
      wasmUrl: 'https://app.example.test/pinned.wasm', wasmFallbackUrls: ['https://assets.example.test/pinned.wasm'],
      timeoutMs: 120000, waitForCanvas: async () => {}, fetchImpl: async () => { calls++; return new Response(null, { status }); },
    }), new RegExp('HTTP ' + status));
    assert.equal(calls, status === 503 ? 3 : 2); assert.equal(initialized, 0);
  }
});

test('invalid or oversized last-mirror bytes cannot turn an earlier transient failure into a retry', async () => {
  for (const [body, limit, errorType] of [[new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]), 1024, WebAssembly.CompileError],
    [customBinary, binary.length, RangeError]]) {
    let calls = 0, initialized = false;
    await assert.rejects(initialize({ loadModule: async () => ({ async default() { initialized = true; } }),
      wasmUrl: 'https://app.example.test/pinned.wasm', wasmFallbackUrls: ['https://assets.example.test/pinned.wasm'],
      timeoutMs: 120000, maxBodyBytes: limit, waitForCanvas: async () => {}, fetchImpl: async () => {
        calls++; return calls === 1 ? new Response(null, { status: 503 }) : new Response(body, { headers: { 'content-type': 'application/wasm' } });
      },
    }), errorType);
    assert.equal(calls, 2); assert.equal(initialized, false);
  }
});

test('the original final header deadline is terminal even after an earlier retryable failure', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let calls = 0, lastSignal, settled = false;
  const pending = initialize({ loadModule: async () => ({}), wasmUrl: 'https://app.example.test/pinned.wasm',
    wasmFallbackUrls: ['https://assets.example.test/pinned.wasm'], timeoutMs: 120000, waitForCanvas: async () => {},
    fetchImpl: async request => {
      calls++; if (calls === 1) return new Response(null, { status: 503 });
      lastSignal = request.signal;
      return new Promise((_, reject) => lastSignal.addEventListener('abort', () => reject(lastSignal.reason), { once: true }));
    },
  });
  const observed = pending.then(() => assert.fail('stalled headers initialized'), error => { settled = true; return error; });
  while (!lastSignal) await flush();
  t.mock.timers.tick(119999); await flush(); assert.equal(settled, false);
  t.mock.timers.tick(1); assert.equal((await observed).name, 'AbortError'); assert.equal(calls, 2);
});

test('a diagnostic callback cannot fail or repeat initialization', async () => {
  let calls = 0, initialized = 0;
  await initialize({ loadModule: async () => ({ async default() { initialized++; } }),
    wasmUrl: 'https://app.example.test/pinned.wasm', wasmFallbackUrls: ['https://assets.example.test/pinned.wasm'],
    timeoutMs: 120000, waitForCanvas: async () => {}, onDownloadAttempt: () => { throw new Error('observer unavailable'); },
    fetchImpl: async () => { calls++; return calls === 1 ? new Response(null, { status: 503 }) : response(); },
  });
  assert.equal(calls, 2); assert.equal(initialized, 1);
});

test('a browser compiler wrapping an over-limit stream as a network error cannot authorize a retry', async () => {
  let calls = 0, initialized = 0;
  const browserCompiler = { compile: WebAssembly.compile, CompileError: WebAssembly.CompileError,
    async compileStreaming(body) {
      try { return await WebAssembly.compile(await body.arrayBuffer()); }
      catch { throw new TypeError('WebAssembly response has been aborted'); }
    },
  };
  await assert.rejects(load(browserCompiler)({ loadModule: async () => ({ async default() { initialized++; } }),
    wasmUrl: 'https://app.example.test/pinned.wasm', wasmFallbackUrls: ['https://assets.example.test/pinned.wasm'],
    timeoutMs: 120000, maxBodyBytes: binary.length, waitForCanvas: async () => {}, fetchImpl: async () => {
      calls++; return calls === 1 ? new Response(null, { status: 503 })
        : new Response(customBinary, { headers: { 'content-type': 'application/wasm' } });
    },
  }), RangeError);
  assert.equal(calls, 2); assert.equal(initialized, 0);
});
