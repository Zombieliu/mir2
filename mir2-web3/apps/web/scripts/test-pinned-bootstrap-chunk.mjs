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
const visuals = JSON.parse(read(new URL("generated/r16-entry-visuals.json", lib)));
const walking = visuals.monsterWalking;
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
  for (const [pin, format] of [[scene,"json"],[floor,"pretty-json-line"],[locale,"json-line"],[walking,"json-line"]]) {
    const raw=body(pin),data=JSON.parse(raw),calls=[];
    const get=chunkApi.createPinnedBootstrapChunk(pin, async()=>{calls.push(true);return {default:data};},format);
    assert.equal(calls.length,0);const first=get();assert.equal(get(),first);
    assert.deepEqual(await first,data);assert.equal(calls.length,1);assert.equal(get(),first);
    assert.equal(raw.length,pin.bytes);assert.equal(sha(raw),pin.sha256);
  }
});

test("normal verified HTTP does not request any compiled fallback", async () => {
  const f=fixture(async url=>new Response(body([scene,floor,locale,walking].find(p=>p.path===url))));
  assert.deepEqual(f.imports,[]);
  await f.entry.preloadR16EntryScene(scene.assetReleaseId);
  await f.entry.preloadR16EntryFloor(floor.assetReleaseId);
  await f.entry.preloadR16EntryMonsterFrames(visuals.assetReleaseId,"walking");
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


function assertWalkingPack(value) {
  assert(value);
  const bytes=Buffer.from(JSON.stringify(value)+"\n");
  assert.equal(bytes.length,764724);
  assert.equal(sha(bytes),"41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f");
  assert.deepEqual(bytes,body(walking));
  assert.equal(Object.keys(value.images).length,240);
  assert.deepEqual(Object.keys(value.libraries),["Monster/000","Monster/003","Monster/004","Monster/005","Monster/139"]);
  for(const library of Object.values(value.libraries)) {
    const action=library.frameSet.actions.find(value=>value.actionName==="Walking");
    assert(action);assert.equal(action.count,6);assert.equal(library.frames.length,48);
    const expected=Array.from({length:8},(_,direction)=>Array.from({length:6},(_,frame)=>
      action.start+direction*(action.count+action.skip)+frame)).flat();
    assert.deepEqual(library.frames.map(frame=>frame.index),expected);
    for(const frame of library.frames)assert(value.images[frame.path]?.startsWith("data:image/png;base64,"));
  }
}

// Reproduce both positive-byte idle failures from the public receipt. No partial
// buffer is a valid pack, and no third JSON request is authorized by the chunk.
test("two positive partial walking bodies recover the exact original pack through one compiled observation",async t=>{
  t.mock.timers.enable({apis:["setTimeout"]});
  const calls=[],cancellations=[],raw=body(walking),prefixBytes=[624575,625832];
  const f=fixture(async(url,init)=>{
    assert.equal(url,walking.path);const ordinal=calls.length;calls.push({url,init});
    assert(ordinal<2,"compiled recovery must not launch a third JSON request");
    return new Response(new ReadableStream({
      start(controller){controller.enqueue(raw.subarray(0,prefixBytes[ordinal]));},
      cancel(reason){cancellations.push({ordinal,reason});},
    }));
  });
  const first=f.entry.preloadR16EntryMonsterFrames(visuals.assetReleaseId,"walking");
  assert.equal(f.entry.preloadR16EntryMonsterFrames(visuals.assetReleaseId,"walking"),first);
  await settle();t.mock.timers.tick(7999);await settle();
  assert.equal(calls.length,1);assert.equal(calls[0].init.signal.aborted,false);assert.deepEqual(f.imports,[]);
  t.mock.timers.tick(1);await settle();assert.equal(calls[0].init.signal.reason.name,"TimeoutError");
  t.mock.timers.tick(199);await settle();assert.equal(calls.length,1);
  t.mock.timers.tick(1);await settle();assert.equal(calls.length,2);assert.deepEqual(f.imports,[]);
  t.mock.timers.tick(7999);await settle();assert.equal(calls[1].init.signal.aborted,false);
  t.mock.timers.tick(1);const pack=await first;assertWalkingPack(pack);
  assert.deepEqual(calls.map(({url,init})=>[url,init.cache,init.credentials,init.priority]),[
    [walking.path,"force-cache","omit","low"],[walking.path,"reload","omit","low"],
  ]);
  assert.equal(f.imports.length,1);assert(f.imports[0].endsWith(walking.path));
  assert.deepEqual(cancellations.map(value=>value.ordinal),[0,1]);
  for(const cancelled of cancellations)assert.equal(cancelled.reason,calls[cancelled.ordinal].init.signal.reason);
  assert.equal(await f.entry.preloadR16EntryMonsterFrames(visuals.assetReleaseId,"walking"),pack);
  t.mock.timers.tick(600000);await settle();assert.equal(calls.length,2);assert.equal(f.imports.length,1);
});

test("walking definitive responses and rejected complete bytes never grant a compiled fallback",async()=>{
  const raw=body(walking),corrupt=Buffer.from(raw);corrupt[100]^=1;
  const responses=[
    ...[401,403,404].map(status=>()=>new Response(null,{status})),
    ...[429,503].map(status=>()=>new Response(null,{status,headers:{"retry-after":"120"}})),
    ()=>new Response(raw.subarray(1)),()=>new Response(corrupt),
    ()=>new Response(Buffer.concat([raw,Buffer.from(" ")])),
  ];
  for(const response of responses){
    let calls=0;const f=fixture(async()=>{calls++;return response();});
    assert.equal(await f.entry.preloadR16EntryMonsterFrames(visuals.assetReleaseId,"walking"),null);
    assert.equal(calls,1);assert.deepEqual(f.imports,[]);
  }
});

test("a final terminal response after one transport failure does not authorize walking fallback",async t=>{
  t.mock.timers.enable({apis:["setTimeout"]});
  let calls=0;const f=fixture(async()=>{if(++calls===1)throw new TypeError("connection interrupted");return new Response(null,{status:404});});
  const pending=f.entry.preloadR16EntryMonsterFrames(visuals.assetReleaseId,"walking");
  await settle();t.mock.timers.tick(200);assert.equal(await pending,null);
  t.mock.timers.tick(600000);await settle();assert.equal(calls,2);assert.deepEqual(f.imports,[]);
});

test("a corrupt compiled walking payload is rejected after exactly two transient HTTP attempts",async t=>{
  t.mock.timers.enable({apis:["setTimeout"]});
  let calls=0;const f=fixture(async()=>{calls++;return new Response(null,{status:503});},data=>({...data,unexpected:true}));
  const pending=f.entry.preloadR16EntryMonsterFrames(visuals.assetReleaseId,"walking");
  await settle();t.mock.timers.tick(200);assert.equal(await pending,null);
  t.mock.timers.tick(600000);await settle();assert.equal(calls,2);assert.equal(f.imports.length,1);
});

test("walking compiled timeout remains twenty seconds and a late valid module cannot publish",async t=>{
  t.mock.timers.enable({apis:["setTimeout"]});
  const blocked=deferred();let calls=0,settled=false;
  const f=fixture(async()=>{calls++;return new Response(null,{status:503});},data=>blocked.promise.then(()=>data));
  const pending=f.entry.preloadR16EntryMonsterFrames(visuals.assetReleaseId,"walking");
  void pending.then(()=>{settled=true;});
  await settle();t.mock.timers.tick(200);await settle();assert.equal(f.imports.length,1);
  t.mock.timers.tick(19999);await settle();assert.equal(settled,false);
  t.mock.timers.tick(1);assert.equal(await pending,null);assert.equal(settled,true);
  blocked.resolve();await settle();assert.equal(await pending,null);
  t.mock.timers.tick(600000);await settle();assert.equal(calls,2);assert.equal(f.imports.length,1);
});

test("standing failure and another release cannot start the walking compiled fallback",async t=>{
  t.mock.timers.enable({apis:["setTimeout"]});
  let calls=0;const f=fixture(async()=>{calls++;return new Response(null,{status:503});});
  assert.equal(f.entry.preloadR16EntryMonsterFrames("another-release","walking"),null);
  assert.equal(calls,0);assert.deepEqual(f.imports,[]);
  const pending=f.entry.preloadR16EntryMonsterFrames(visuals.assetReleaseId,"standing");
  await settle();t.mock.timers.tick(200);assert.equal(await pending,null);
  assert.equal(calls,2);assert.deepEqual(f.imports,[]);
});

test("the unchanged Shell monster effect suppresses a retired observer and shares verified walking recovery with the live entry",async t=>{
  t.mock.timers.enable({apis:["setTimeout"]});
  const shell=read(new URL("../app/original-client-shell.tsx",lib));
  const tree=ts.createSourceFile("original-client-shell.tsx",shell,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  let effect;
  const visit=node=>{
    if(ts.isCallExpression(node)&&node.expression.getText(tree)==="useEffect"&&
       node.arguments[1]?.getText(tree)==="[warmEntryMonsters]")effect=node.arguments[0].getText(tree);
    ts.forEachChild(node,visit);
  };visit(tree);assert(effect);
  const blocked=deferred(),walkingPromises=[],retired=[],live=[],calls=[];
  const f=fixture(async(url,init)=>{
    calls.push({url,init});return url===visuals.monsterStanding.path?new Response(body(visuals.monsterStanding))
      :new Response(null,{status:503});
  },data=>blocked.promise.then(()=>data));
  const start=updates=>{
    const exports={};
    new Function("exports","warmEntryMonsters","process","preloadR16EntryMonsterFrames","setEntryMonsterStanding","setEntryMonsterWalking",
      compile(`exports.effect=${effect};`))(
      exports,true,{env:{MIR2_PINNED_ASSET_VERSION:visuals.assetReleaseId}},
      (...args)=>{const promise=f.entry.preloadR16EntryMonsterFrames(...args);if(args[1]==="walking")walkingPromises.push(promise);return promise;},
      value=>updates.push({action:"standing",value}),value=>updates.push({action:"walking",value}),
    );
    return exports.effect();
  };
  const cleanup=start(retired);
  for(let i=0;i<100&&!walkingPromises.length;i++)await settle();assert.equal(walkingPromises.length,1);
  t.mock.timers.tick(200);await settle();assert.equal(f.imports.length,1);
  cleanup();const liveCleanup=start(live);await settle();assert.equal(walkingPromises.length,2);
  assert.equal(walkingPromises[0],walkingPromises[1]);blocked.resolve();await Promise.all(walkingPromises);await settle();
  assert.equal(retired.filter(value=>value.action==="walking").length,0);
  assert.equal(live.filter(value=>value.action==="walking").length,1);
  assertWalkingPack(live.find(value=>value.action==="walking").value);
  assert.equal(calls.filter(value=>value.url===walking.path).length,2);assert.equal(f.imports.length,1);
  liveCleanup();
});
