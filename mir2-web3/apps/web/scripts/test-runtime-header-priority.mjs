import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const compile=source=>ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
function load(path,window){const exports={};new Function('exports','window',compile(readFileSync(new URL(path,import.meta.url),'utf8')))(exports,window);return exports;}
const{RuntimeHeaderPriority}=load('../lib/runtime-header-priority.ts');
const{initializeBevyRuntime}=load('../lib/bevy-runtime-initializer.ts');
const turn=()=>new Promise(setImmediate);
const binary=new Uint8Array([0,97,115,109,1,0,0,0]);

test('a real successful response releases background work before its unfinished binary body',async()=>{
 const priority=new RuntimeHeaderPriority();priority.beginCycle();const release=priority.captureRelease();
 let producer,initialized=0,background=false;
 const warm=priority.wait().then(result=>{background=true;return result;});
 const running=initializeBevyRuntime({loadModule:async()=>({async default(){initialized++;}}),wasmUrl:'https://app.example/pinned.wasm',timeoutMs:120000,
  waitForCanvas:async()=>{},onResponseHeaders:()=>release('headers'),fetchImpl:async()=>new Response(new ReadableStream({start(c){producer=c;}}),{headers:{'content-type':'application/wasm'}})});
 assert.equal((await warm).reason,'headers');assert(background);assert.equal(initialized,0);
 producer.enqueue(binary);producer.close();await running;assert.equal(initialized,1);assert.equal(release('finished'),false);
});

test('one fixed eight-second allowance is shared by late background consumers',async t=>{
 t.mock.timers.enable({apis:['setTimeout']});const p=new RuntimeHeaderPriority();p.beginCycle();
 let done=0;const a=p.wait().then(x=>{done++;return x;});
 t.mock.timers.tick(7000);await turn();const b=p.wait().then(x=>{done++;return x;});
 t.mock.timers.tick(999);await turn();assert.equal(done,0);
 t.mock.timers.tick(1);assert.equal((await a).reason,'timeout');assert.equal((await b).reason,'timeout');
 assert.equal(done,2);assert.equal(p.captureRelease()('headers'),false);
});

test('a skipped or failed runtime releases prewarm immediately without waiting for a timeout',async()=>{
 for(const reason of ['finished','retired']){
  const p=new RuntimeHeaderPriority();p.beginCycle();const result=p.wait();
  assert.equal(p.captureRelease()(reason),true);assert.equal((await result).reason,reason);
 }
});

test('an old runtime cannot release a later adventure',async()=>{
 const p=new RuntimeHeaderPriority();p.beginCycle();const oldRelease=p.captureRelease(),first=p.wait();
 p.beginCycle();assert.equal((await first).reason,'superseded');let secondReleased=false;
 const second=p.wait().then(x=>{secondReleased=true;return x;});
 assert.equal(oldRelease('headers'),false);await turn();assert.equal(secondReleased,false);
 assert.equal(p.captureRelease()('headers'),true);assert.equal((await second).generation,2);
});

test('retiring one prewarm consumer does not release another consumer',async()=>{
 const p=new RuntimeHeaderPriority();p.beginCycle();const owner=new AbortController();
 const retired=p.wait(owner.signal).catch(e=>e),active=p.wait();
 owner.abort();assert.equal(await retired,owner.signal.reason);
 p.captureRelease()('headers');assert.equal((await active).reason,'headers');
});

test('an already canceled owner starts no wait and completed headers need no renewed allowance',async()=>{
 const p=new RuntimeHeaderPriority(),owner=new AbortController();owner.abort();
 await assert.rejects(p.wait(owner.signal),e=>e===owner.signal.reason);
 assert.equal((await p.wait()).reason,'inactive');p.beginCycle();p.captureRelease()('headers');
 assert.equal((await p.wait()).reason,'headers');p.cancelCycle();assert.equal((await p.wait()).reason,'cancelled');
});

test('server rendering has no shared gate and the browser document has one stable coordinator',()=>{
 assert.equal(load('../lib/runtime-header-priority.ts').getRuntimeHeaderPriority(),undefined);
 const browser=load('../lib/runtime-header-priority.ts',{});assert.equal(browser.getRuntimeHeaderPriority(),browser.getRuntimeHeaderPriority());
 for(const budget of [0,8001,Infinity,1.5])assert.throws(()=>new RuntimeHeaderPriority(budget),RangeError);
});

function registrarRunner(priority,events){
 const source=readFileSync(new URL('../app/components/asset-cache-registrar.tsx',import.meta.url),'utf8');
 const ast=ts.createSourceFile('registrar.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let callback;
 const visit=node=>{if(ts.isNewExpression(node)&&node.expression.getText(ast)==='AssetPrewarmOrchestrator'){
  callback=node.arguments[0].properties.find(p=>p.name?.getText(ast)==='run').initializer;
 }ts.forEachChild(node,visit);};visit(ast);assert(callback);
 const code=compile('exports.run='+callback.getText(ast));const exports={};
 new Function('exports','getRuntimeHeaderPriority','metrics','prewarmAssetPacks','manifest','prewarmPolicy','reconfigureWorker','disposed','setSnapshot',code)(
  exports,()=>priority,{markMilestone:(name,value)=>events.push({kind:'milestone',name,...value}),snapshot:()=>({})},
  async(_manifest,_metrics,_policy,stage,lane)=>events.push({kind:'work',stage,lane}),{}, {},undefined,false,()=>{});
 return exports.run;
}

test('actual registrar delays only game background admission while critical and select work proceed',async()=>{
 const p=new RuntimeHeaderPriority();p.beginCycle();const events=[],run=registrarRunner(p,events);
 const background=run('game','background',new AbortController().signal);await turn();
 assert.equal(events.length,0);await run('game','critical',new AbortController().signal);await run('character-select','background',new AbortController().signal);
 assert.equal(events.filter(e=>e.kind==='work').length,2);
 p.captureRelease()('headers');await background;
 assert.deepEqual(events.filter(e=>e.kind==='work').map(e=>[e.stage,e.lane]),[['game','critical'],['character-select','background'],['game','background']]);
 assert.equal(events.find(e=>e.kind==='milestone').reason,'headers');
});

test('actual registrar never starts a retired game prewarm, even when login prewarm is disabled',async()=>{
 const p=new RuntimeHeaderPriority();p.beginCycle();const events=[],run=registrarRunner(p,events);
 const background=run('game','background',new AbortController().signal);const rejected=assert.rejects(background,e=>e.name==='AbortError');
 p.cancelCycle();await rejected;assert.equal(events.filter(e=>e.kind==='work').length,0);
});

test('actual orchestrator callbacks queued before a screen retirement cannot start later',async()=>{
 const{AssetPrewarmOrchestrator}=load('../lib/asset-orchestrator.ts');
 for(const earlyHeaders of[false,true]){
  const p=new RuntimeHeaderPriority();p.beginCycle();if(earlyHeaders)p.captureRelease()('headers');
  const events=[],run=registrarRunner(p,events),o=new AssetPrewarmOrchestrator({backgroundMode:'afterPlayable',hasWork:(_s,lane)=>lane==='background',run});
  const pending=o.requestStage('game');void o.markFirstPlayable();p.cancelCycle();await pending;o.dispose();
  assert.equal(events.filter(e=>e.kind==='work').length,0);assert.equal(events[0].reason,'cancelled');
 }
});

test('actual screen effect arms priority before requesting game prewarm and cancels on select/login',()=>{
 const source=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ast=ts.createSourceFile('page.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let effect;
 const visit=node=>{if(ts.isCallExpression(node)&&node.expression.getText(ast)==='useEffect'&&node.arguments[0]?.getText(ast).includes('priority?.beginCycle()'))effect=node.arguments[0];ts.forEachChild(node,visit);};visit(ast);assert(effect);
 const code=compile('exports.effect='+effect.getText(ast));
 for(const screen of ['game','select','login']){
  const calls=[],exports={};new Function('exports','screen','getRuntimeHeaderPriority','requestAssetPrewarmStage',code)(exports,screen,
   ()=>({beginCycle:()=>calls.push('begin'),cancelCycle:()=>calls.push('cancel')}),stage=>calls.push(stage));exports.effect();
  assert.deepEqual(calls,screen==='game'?['begin','game']:['cancel',screen==='select'?'character-select':'login']);
 }
});

test('denied and owner-retired responses cannot announce successful runtime headers',async()=>{
 let headers=0;
 await assert.rejects(initializeBevyRuntime({loadModule:async()=>({}),wasmUrl:'https://app.example/pinned.wasm',timeoutMs:120000,waitForCanvas:async()=>{},
  onResponseHeaders:()=>headers++,fetchImpl:async()=>new Response(null,{status:403})}));assert.equal(headers,0);
 const owner=new AbortController();owner.abort();await assert.rejects(initializeBevyRuntime({loadModule:async()=>({}),wasmUrl:'https://app.example/pinned.wasm',timeoutMs:120000,waitForCanvas:async()=>{},
  ownerSignal:owner.signal,onResponseHeaders:()=>headers++,fetchImpl:async()=>new Response(binary)}));assert.equal(headers,0);
});

test('a scheduling callback exception cannot fail compilation or repeat initialization',async()=>{
 let initialized=0;await initializeBevyRuntime({loadModule:async()=>({async default(){initialized++;}}),wasmUrl:'https://app.example/pinned.wasm',timeoutMs:120000,
  waitForCanvas:async()=>{},onResponseHeaders:()=>{throw Error('observer failed');},fetchImpl:async()=>new Response(binary,{headers:{'content-type':'application/wasm'}})});
 assert.equal(initialized,1);
});
