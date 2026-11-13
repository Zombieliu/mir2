import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import test from "node:test";
import ts from "typescript";
const lib = new URL("../lib/", import.meta.url);
const read = url => readFileSync(url, "utf8");
const compile = source => ts.transpileModule(source, { compilerOptions: {
  module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
} }).outputText;
const chunkApi = {};
new Function("exports", compile(read(new URL("pinned-bootstrap-chunk.ts", lib))))(chunkApi);
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const scene = JSON.parse(read(new URL("generated/r16-entry-scene.json", lib)));
const floor = JSON.parse(read(new URL("generated/r16-entry-floor.json", lib)));
const locale = JSON.parse(read(new URL("generated/r16-native-localization-delivery.json", lib)));
const body = pin => readFileSync(new URL("../public" + pin.path, lib));
const settle = async () => { for (let i = 0; i < 8; i++) await new Promise(setImmediate); };
function deferred() { let resolve, reject; const promise = new Promise((a,b) => { resolve=a; reject=b; }); return {promise,resolve,reject}; }

function fixture(fetcher, changeChunk) {
  const modules = new Map(), imports = [];
  function load(file) {
    if (file.pathname.endsWith(".json")) {
      const value = JSON.parse(read(file));
      if (file.pathname.includes("/public/bootstrap/") || file.pathname.endsWith("r16-native-localization-gameplay.json")) {
        imports.push(file.pathname);
        return changeChunk ? Promise.resolve(changeChunk(value, file)).then(defaultValue => ({ default: defaultValue }))
          : { default: value };
      }
      return { default: value };
    }
    if (modules.has(file.href)) return modules.get(file.href);
    const exports = {}; modules.set(file.href, exports);
    const require = name => load(new URL(name + (name.endsWith(".json") ? "" : ".ts"), file));
    new Function("exports", "require", "fetch", "window", compile(read(file)))(exports, require, fetcher, {});
    return exports;
  }
  return { entry: load(new URL("r16-entry-scene.ts", lib)), locale: load(new URL("r16-native-localization.ts", lib)), imports };
}

test("generated literal imports and each original serialization format retain the exact pins", async () => {
  execFileSync(process.execPath, [new URL("generate-r16-bootstrap-chunks.mjs", import.meta.url).pathname.replace(/^\/(\w:)/,"$1"), "--check"], { windowsHide: true });
  for (const [pin, format] of [[scene,"json"],[floor,"pretty-json-line"],[locale,"json-line"]]) {
    const raw=body(pin),data=JSON.parse(raw),calls=[];
    const get=chunkApi.createPinnedBootstrapChunk(pin, async()=>{calls.push(true);return {default:data};},format);
    assert.equal(calls.length,0);const first=get();assert.equal(get(),first);
    assert.deepEqual(await first,data);assert.equal(calls.length,1);assert.equal(get(),first);
    assert.equal(raw.length,pin.bytes);assert.equal(sha(raw),pin.sha256);
  }
});

test("normal verified HTTP does not request any compiled fallback", async () => {
  const f=fixture(async url=>new Response(body([scene,floor,locale].find(p=>p.path===url))));
  assert.deepEqual(f.imports,[]);
  await f.entry.preloadR16EntryScene(scene.assetReleaseId);
  await f.entry.preloadR16EntryFloor(floor.assetReleaseId);
  await f.locale.loadR16GameplayLocalization();
  assert.deepEqual(f.imports,[]);
});

test("two 20s scene header failures recover the exact scene through one shared chunk", async t => {
  t.mock.timers.enable({apis:["setTimeout"]});
  const calls=[],f=fixture((url,init)=>{calls.push({url,init});return new Promise(()=>{});});
  const a=f.entry.preloadR16EntryScene(scene.assetReleaseId),b=f.entry.preloadR16EntryScene(scene.assetReleaseId);
  assert.equal(a,b);await settle();t.mock.timers.tick(20000);await settle();t.mock.timers.tick(200);await settle();t.mock.timers.tick(20000);
  assert.deepEqual(await a,JSON.parse(body(scene)));assert.equal(calls.length,2);assert.equal(f.imports.length,1);
  assert.deepEqual(calls.map(c=>c.init.cache),["force-cache","reload"]);
});

test("two zero-body stalls recover the floor with all 124 original PNG strings", async t => {
  t.mock.timers.enable({apis:["setTimeout"]});
  const calls=[],f=fixture((url,init)=>{calls.push({url,init});return new Response(new ReadableStream({start(){}}));});
  const pending=f.entry.preloadR16EntryFloor(floor.assetReleaseId);
  await settle();t.mock.timers.tick(8000);await settle();t.mock.timers.tick(200);await settle();t.mock.timers.tick(8000);
  assert.deepEqual(await pending,JSON.parse(body(floor)));assert.equal(calls.length,2);assert.equal(f.imports.length,1);
});

test("two transient HTTP failures recover all ten locale vectors without changing matcher input", async t => {
  t.mock.timers.enable({apis:["setTimeout"]});
  let calls=0;const f=fixture(async()=>{calls++;return new Response(null,{status:503});});
  const pending=f.locale.loadR16GameplayLocalization();await settle();t.mock.timers.tick(200);await pending;
  assert.equal(calls,2);assert.equal(f.imports.length,1);assert.equal(f.locale.hasR16GameplayLocalization(),true);
  const data=JSON.parse(body(locale));
  for(const entry of data.entries)for(const [i,language]of f.locale.R16_NATIVE_LANGUAGES.entries())assert.equal(f.locale.nativeText(language,entry.key),entry.values[i]);
});

test("a retired scene observer cannot receive the fallback while a live observer still can", async t => {
  t.mock.timers.enable({apis:["setTimeout"]});
  const blocked=deferred();let reads=0;
  const f=fixture(async()=>new Response(null,{status:503}),value=>{reads++;return blocked.promise.then(()=>value);});
  const firstOwner=new AbortController(),otherOwner=new AbortController();
  const request={...scene.request,assetReleaseId:scene.assetReleaseId};
  const first=f.entry.readR16EntryScene(request,scene.assetReleaseId,firstOwner.signal,{untilSettled:true});
  const other=f.entry.readR16EntryScene(request,scene.assetReleaseId,otherOwner.signal,{untilSettled:true});
  firstOwner.abort();assert.equal(await first,null);
  await settle();t.mock.timers.tick(200);blocked.resolve();
  assert.deepEqual(await other,JSON.parse(body(scene)));assert.equal(reads,1);
});

test("definitive status, Retry-After, short/corrupt/oversized HTTP do not grant a fallback", async () => {
  const raw=body(scene),corrupt=Buffer.from(raw);corrupt[100]^=1;
  const responses=[()=>new Response(null,{status:404}),()=>new Response(null,{status:503,headers:{"retry-after":"1"}}),()=>new Response(raw.subarray(1)),()=>new Response(corrupt),()=>new Response(Buffer.concat([raw,Buffer.from(" ")]))];
  for(const response of responses){const f=fixture(async()=>response());assert.equal(await f.entry.preloadR16EntryScene(scene.assetReleaseId),null);assert.deepEqual(f.imports,[]);}
});

test("a corrupted compiled payload cannot install locale rows", async t => {
  t.mock.timers.enable({apis:["setTimeout"]});
  const f=fixture(async()=>new Response(null,{status:503}),data=>({...data,gameplaySha256:"0".repeat(64)}));
  const pending=f.locale.loadR16GameplayLocalization();await settle();t.mock.timers.tick(200);
  await assert.rejects(pending,/digest mismatch/);assert.equal(f.locale.hasR16GameplayLocalization(),false);assert.equal(f.imports.length,1);
});

test("the shared chunk observation times out once and cannot publish a late module", async t => {
  t.mock.timers.enable({apis:["setTimeout"]});
  const module=deferred(),data={fixed:true},raw=JSON.stringify(data),pin={bytes:Buffer.byteLength(raw),sha256:sha(raw)};let imports=0;
  const get=chunkApi.createPinnedBootstrapChunk(pin,()=>{imports++;return module.promise;},"json");
  const first=get(),rejected=assert.rejects(first,e=>e.name==="TimeoutError");await settle();t.mock.timers.tick(20000);await rejected;
  module.resolve({default:data});await settle();assert.equal(get(),first);await assert.rejects(get(),e=>e.name==="TimeoutError");assert.equal(imports,1);
});

test("release and request geometry exclusions do not start a chunk", async () => {
  const f=fixture(()=>assert.fail("excluded entry cannot fetch"));const owner=new AbortController();
  assert.equal(f.entry.preloadR16EntryScene("other-release"),null);
  assert.equal(f.entry.preloadR16EntryFloor("other-release"),null);
  assert.equal(f.entry.readR16EntryScene({...scene.request,centerX:1000,assetReleaseId:scene.assetReleaseId},scene.assetReleaseId,owner.signal),null);
  assert.deepEqual(f.imports,[]);
});
