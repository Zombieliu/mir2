import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";

const registrarPath = fileURLToPath(new URL("../app/components/asset-cache-registrar.tsx", import.meta.url));
const requestPath = fileURLToPath(new URL("../lib/asset-prewarm-request.ts", import.meta.url));
function loadCode(source, dependencies = {}) {
  const code = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const module = { exports: {} };
  new Function("module", "exports", "require", ...Object.keys(dependencies), code)(module, module.exports,
    () => { throw Error("Unexpected dependency"); }, ...Object.values(dependencies));
  return module.exports;
}
function loadPrewarmUrls(fetchImpl) {
  const source = readFileSync(registrarPath, "utf8");
  const ast = ts.createSourceFile(registrarPath, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const functions = ["prewarmUrls", "fetchWithRetry", "createAbortError", "sleep"].map(name =>
    ast.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === name)?.getText(ast) ?? "");
  const request = existsSync(requestPath) ? loadCode(readFileSync(requestPath, "utf8")) : {};
  return loadCode(functions.join("\n") + "\nexports.prewarmUrls = prewarmUrls;", {
    fetch: fetchImpl, publishPrewarmProgress() {},
    isAbortError: error => error?.name === "AbortError",
    fetchPrewarmAsset: (url, options) => request.fetchPrewarmAsset(url, { ...options, fetchImpl }),
  }).prewarmUrls;
}
const turn = () => new Promise(resolve => setImmediate(resolve));
const request = existsSync(requestPath) ? loadCode(readFileSync(requestPath, "utf8")) : {};
const bytes = values => new Response(new Uint8Array(values));
const brokenBody = () => new Response(new ReadableStream({ start(stream) {
  stream.enqueue(new Uint8Array([9]));
  queueMicrotask(() => stream.error(new TypeError("connection terminated")));
} }));

function loadPrewarmPack(fetchImpl) {
  const source=readFileSync(registrarPath,'utf8'),ast=ts.createSourceFile(registrarPath,source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const names=['createPrewarmMetric','prewarmPack','prewarmUrls','createAbortError'];
  const functions=names.map(name=>{const fn=ast.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text===name);assert(fn,name);return fn.getText(ast);});
  return loadCode(functions.join('\n')+'\nexports.prewarmPack=prewarmPack;',{
    publishPrewarmProgress(){},hintAssetCacheTier(){},cacheTierForPack:()=> 'critical',
    isAbortError:error=>error?.name==='AbortError',
    fetchPrewarmAsset:(url,options)=>request.fetchPrewarmAsset(url,{...options,fetchImpl}),
  }).prewarmPack;
}

test('HUD recovery reads only failed URLs after worker configuration and keeps first failures',async()=>{
  const urls=Array.from({length:6},(_,n)=>'/hud/'+n+'.png'),calls=new Map(),metrics={prewarmRuns:[]};let configured=false,active=0,peak=0;
  const pack=loadPrewarmPack(async url=>{
    const count=(calls.get(url)??0)+1;calls.set(url,count);
    if(url!==urls[0]&&count<=3)return new Response(null,{status:503});
    if(count>3){assert(configured);active++;peak=Math.max(peak,active);await turn();active--;}
    return bytes([1,2]);
  });
  await pack({name:'hud-core',label:'HUD',urls},metrics,6,null,'high',undefined,false,undefined,async()=>{configured=true;});
  const result=metrics.prewarmRuns[0];assert.equal(metrics.prewarmRuns.length,1);
  assert.deepEqual({requested:result.requested,ok:result.ok,failed:result.failed,failedUrls:result.failedUrls},{requested:6,ok:6,failed:0,failedUrls:[]});
  assert.equal(calls.get(urls[0]),1);for(const url of urls.slice(1))assert.equal(calls.get(url),4);
  assert.deepEqual(result.recovery.initialFailedUrls.sort(),urls.slice(1).sort());assert.equal(result.requestFailures.length,15);assert(peak<=2);
});

test('recovery does not erase a failure before its replacement body reaches EOF',async()=>{
  let calls=0,stream;const metrics={prewarmRuns:[]};
  const pack=loadPrewarmPack(async()=>++calls<=3?new Response(null,{status:503}):new Response(new ReadableStream({start(c){stream=c;}})));
  const pending=pack({name:'hud-core',label:'HUD',urls:['/held.png']},metrics,1,null,'high',undefined,false);
  for(const end=Date.now()+2000;!stream&&Date.now()<end;)await new Promise(resolve=>setTimeout(resolve,10));
  if(!stream){await pending;assert.fail('failed HUD body must receive a recovery read');}
  assert.equal(metrics.prewarmRuns[0].ok,0);assert.equal(metrics.prewarmRuns[0].failed,1);
  stream.enqueue(new Uint8Array([1]));await turn();assert.equal(metrics.prewarmRuns[0].ok,0);
  stream.close();await pending;assert.equal(metrics.prewarmRuns[0].ok,1);assert.equal(metrics.prewarmRuns[0].failed,0);
});

test('persistent HUD failures get at most one bounded recovery sweep',async()=>{
  const urls=Array.from({length:9},(_,n)=>'/failed/'+n),calls=new Map(),metrics={prewarmRuns:[]};
  await loadPrewarmPack(async url=>{calls.set(url,(calls.get(url)??0)+1);return new Response(null,{status:503});})(
    {name:'hud-core',label:'HUD',urls},metrics,9,null,'high',undefined,false);
  for(const url of urls.slice(0,8))assert.equal(calls.get(url),6);assert.equal(calls.get(urls[8]),3);
  const result=metrics.prewarmRuns[0];assert.equal(result.requested,9);assert.equal(result.ok,0);assert.equal(result.failed,9);
  assert.equal(result.recovery.requested,8);assert.deepEqual(result.recovery.unattemptedUrls,[urls[8]]);
});

test('owner cancellation during the configuration refresh prevents every recovery request',async()=>{
  const owner=new AbortController(),metrics={prewarmRuns:[]};let calls=0,release;
  const pending=loadPrewarmPack(async()=>{calls++;return new Response(null,{status:503});})(
    {name:'hud-core',label:'HUD',urls:['/failed.png']},metrics,1,null,'high',undefined,false,owner.signal,()=>new Promise(resolve=>{release=resolve;}));
  for(const end=Date.now()+2000;!release&&Date.now()<end;)await new Promise(resolve=>setTimeout(resolve,10));
  if(!release){await pending;assert.fail('HUD recovery must request bounded worker configuration');}
  const rejected=assert.rejects(pending,error=>error.name==='AbortError');owner.abort();release();await rejected;
  assert.equal(calls,3);assert.equal(metrics.prewarmRuns[0].status,'cancelled');
});

test('healthy HUD and non-HUD packs do not configure or retry',async()=>{
  let calls=0,hooks=0;
  for(const [name,ok] of [['hud-core',true],['ordinary-pack',false]]){
    const metrics={prewarmRuns:[]};await loadPrewarmPack(async()=>{calls++;return ok?bytes([1]):new Response(null,{status:503});})(
      {name,label:name,urls:['/asset.png']},metrics,1,null,'high',undefined,false,undefined,async()=>{hooks++;});
  }
  assert.equal(calls,4);assert.equal(hooks,0);
});

test('a failed configuration refresh cannot manufacture success or retain the stage forever',async()=>{
  let calls=0;const metrics={prewarmRuns:[]};
  await loadPrewarmPack(async()=>{calls++;return new Response(null,{status:503});})(
    {name:'hud-core',label:'HUD',urls:['/failed.png']},metrics,1,null,'high',undefined,false,undefined,async()=>{throw new Error('configuration unavailable');});
  assert.equal(calls,6);assert.equal(metrics.prewarmRuns[0].ok,0);assert.equal(metrics.prewarmRuns[0].failed,1);
  assert.equal(metrics.prewarmRuns[0].recovery.configurationError,'Error');
});

test('canceling a recovery body preserves its attempt diagnostics and cancels its bytes',async()=>{
  const owner=new AbortController(),metrics={prewarmRuns:[]};let calls=0,stream,cancelled=false;
  const pending=loadPrewarmPack(async()=>++calls<=3?new Response(null,{status:503}):new Response(new ReadableStream({start(c){stream=c;},cancel(){cancelled=true;}})))(
    {name:'hud-core',label:'HUD',urls:['/held.png']},metrics,1,null,'high',undefined,false,owner.signal);
  for(const end=Date.now()+2000;!stream&&Date.now()<end;)await new Promise(resolve=>setTimeout(resolve,10));
  if(!stream){await pending;assert.fail('recovery body must start');}
  const rejected=assert.rejects(pending,error=>error.name==='AbortError');owner.abort();await rejected;await turn();
  const metric=metrics.prewarmRuns[0];assert(cancelled);assert.equal(calls,4);assert.equal(metric.status,'cancelled');
  assert.equal(metric.ok,0);assert.equal(metric.failed,1);assert.equal(metric.recovery.status,'cancelled');
  assert(metric.recovery.requestFailures.some(x=>x.url==='/held.png'&&x.phase==='body'));
});

test("public prewarm progress waits for the complete response body", async () => {
  let stream;
  const response = new Response(new ReadableStream({ start(controller) { stream = controller; } }));
  const metric = { requested: 1, ok: 0, failed: 0, failedUrls: [] };
  const run = loadPrewarmUrls(async () => response)(["/original-ui/Prguse/986.png"], metric, {}, 1);
  await turn();
  assert.equal(metric.ok, 0, "200 headers alone must not count as a warmed image");
  stream.enqueue(new Uint8Array([1, 2]));
  await turn();
  assert.equal(metric.ok, 0, "a body prefix must not count as a warmed image");
  stream.enqueue(new Uint8Array([3]));
  stream.close();
  await run;
  assert.equal(metric.ok, 1);
  assert.equal(metric.failed, 0);
});

test("a disconnected body is retried and only the final complete bytes are returned", async () => {
  let calls = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { fetchImpl: async () =>
    ++calls === 1 ? brokenBody() : bytes([1, 2, 3]) });
  assert.equal(calls, 2);
  assert.deepEqual([...result.body], [1, 2, 3]);
});

test("three broken bodies produce one failed prewarm metric and its URL", async () => {
  let calls = 0;
  const metric = { ok: 0, failed: 0, failedUrls: [] };
  await loadPrewarmUrls(async () => { calls++; return brokenBody(); })(["/asset.png"], metric, {}, 1);
  assert.equal(calls, 3);
  assert.deepEqual({ ok: metric.ok, failed: metric.failed, failedUrls: metric.failedUrls },
    { ok: 0, failed: 1, failedUrls: ["/asset.png"] });
  assert.equal(metric.requestFailures.length, 3);
  assert.deepEqual(metric.requestFailures.map(({ url, attempt, phase, status, errorName }) =>
    ({ url, attempt, phase, status, errorName })), [1, 2, 3].map(attempt =>
    ({ url: "/asset.png", attempt, phase: "body", status: 200, errorName: "TypeError" })));
});

test("body inactivity cancels the incomplete stream before retrying", async () => {
  let calls = 0, cancelled = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { bodyIdleTimeoutMs: 30, fetchImpl: async () => {
    if (++calls > 1) return bytes([7, 8]);
    return new Response(new ReadableStream({ start(stream) { stream.enqueue(new Uint8Array([1])); },
      cancel() { cancelled++; } }));
  } });
  assert.equal(calls, 2);
  assert.equal(cancelled, 1);
  assert.deepEqual([...result.body], [7, 8]);
});

test("continuing chunks renew the idle deadline instead of imposing a total body deadline", async () => {
  let timer, n = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { bodyIdleTimeoutMs: 200, fetchImpl: async () =>
    new Response(new ReadableStream({ start(stream) {
      timer = setInterval(() => {
        stream.enqueue(new Uint8Array([++n]));
        if (n === 6) { clearInterval(timer); stream.close(); }
      }, 50);
    }, cancel() { clearInterval(timer); } })) });
  assert.deepEqual([...result.body], [1, 2, 3, 4, 5, 6]);
});

test("an owner cancellation cannot turn a cancelled body prefix into success", async () => {
  const owner = new AbortController();
  let cancelled = 0, calls = 0;
  const result = request.fetchPrewarmAsset("/asset.png", { signal: owner.signal, fetchImpl: async () => {
    calls++;
    return new Response(new ReadableStream({ start(stream) {
      stream.enqueue(new Uint8Array([1]));
      setImmediate(() => owner.abort("screen-left"));
    }, cancel() { cancelled++; } }));
  } });
  await assert.rejects(result, { name: "AbortError" });
  assert.equal(cancelled, 1);
  assert.equal(calls, 1);
});

test("an owner cancellation during backoff prevents another request", async () => {
  const owner = new AbortController();
  let calls = 0;
  const result = request.fetchPrewarmAsset("/asset.png", { signal: owner.signal, fetchImpl: async () => {
    calls++;
    setTimeout(() => owner.abort(), 20);
    throw new TypeError("network failed");
  } });
  await assert.rejects(result, { name: "AbortError" });
  assert.equal(calls, 1);
});

test("a header timeout retires and retries the request", async () => {
  let calls = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { headerTimeoutMs: 20,
    fetchImpl: async (_url, init) => {
      if (++calls > 1) return bytes([8]);
      return new Promise((_resolve, reject) => init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true }));
    } });
  assert.equal(calls, 2);
  assert.deepEqual([...result.body], [8]);
});

test("non-success responses are cancelled, bounded and never counted as warmed", async () => {
  let calls = 0, cancelled = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { fetchImpl: async () => {
    calls++;
    return new Response(new ReadableStream({ cancel() { cancelled++; } }), { status: 503 });
  } });
  assert.equal(result.response.status, 503);
  assert.equal(result.body, null);
  assert.equal(calls, 3);
  assert.equal(cancelled, 3);
});

test("oversized bodies are cancelled once without retaining or retrying their prefix", async () => {
  let calls = 0, cancelled = 0;
  await assert.rejects(request.fetchPrewarmAsset("/asset.png", { maxBodyBytes: 2, fetchImpl: async () => {
    calls++;
    return new Response(new ReadableStream({ start(stream) { stream.enqueue(new Uint8Array([1, 2, 3])); },
      cancel() { cancelled++; } }));
  } }), { name: "RangeError" });
  assert.equal(calls, 1);
  assert.equal(cancelled, 1);
});
