import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
const module={exports:{}};
new Function('exports',ts.transpileModule(readFileSync(new URL('../lib/scene-bootstrap-race.ts',import.meta.url),'utf8'),{
 compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022},
}).outputText)(module.exports);
const {chooseSceneBootstrap}=module.exports;
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
const tick=()=>new Promise(resolve=>setTimeout(resolve,5));
const base=()=>({signal:new AbortController().signal,graceMs:0,acceptPinned:p=>p.valid,acceptRoute:r=>r.status===200});
test('unavailable pin preserves the ordinary route result and owner signal',async()=>{
 const b=base();const route={status:424};const r=await chooseSceneBootstrap({...b,pinned:null,loadRoute:async signal=>{assert.equal(signal,b.signal);return route;}});
 assert.equal(r.kind,'route');assert.equal(r.result,route);
});
test('warm verified pin does not issue a duplicate route',async()=>{
 const body={valid:true};const r=await chooseSceneBootstrap({...base(),pinned:Promise.resolve(body),loadRoute:()=>assert.fail('warm map must skip route')});
 assert.equal(r.body,body);
});
test('late valid pin cancels an already running route',async()=>{
 const p=deferred(),r=deferred();let signal;
 const result=chooseSceneBootstrap({...base(),pinned:p.promise,loadRoute:s=>{signal=s;return r.promise;}});
 await tick();assert(signal&&!signal.aborted);const body={valid:true};p.resolve(body);
 assert.equal((await result).body,body);assert.equal(signal.aborted,true);
 r.reject(new Error('cancelled route'));await tick();
});
test('ordinary route success wins immediately and ignores a late pin',async()=>{
 const p=deferred(),route={status:200};
 const result=await chooseSceneBootstrap({...base(),pinned:p.promise,loadRoute:async()=>route});
 assert.equal(result.result,route);p.resolve({valid:true});await tick();assert.equal(result.kind,'route');
});
test('a route failure does not discard an independently loading valid pin',async()=>{
 const p=deferred();let settled=false;
 const result=chooseSceneBootstrap({...base(),pinned:p.promise,loadRoute:async()=>{throw new TypeError('connection failed');}}).finally(()=>{settled=true;});
 await tick();assert.equal(settled,false);p.resolve({valid:true});assert.equal((await result).kind,'pinned');
});
test('a definitive error response is retained if no pin is usable',async()=>{
 const p=deferred(),route={status:424,error:'resource_missing'};
 const result=chooseSceneBootstrap({...base(),pinned:p.promise,loadRoute:async()=>route});
 await tick();p.resolve(null);assert.equal((await result).result,route);
});
test('the original transport error is preserved when both sources fail',async()=>{
 const p=deferred(),error=new TypeError('connection failed');
 const result=chooseSceneBootstrap({...base(),pinned:p.promise,loadRoute:async()=>{throw error;}});
 const rejected=assert.rejects(result,e=>e===error);await tick();p.reject(new Error('bad pinned bytes'));await rejected;
});
for(const phase of ['grace','route','failed-route'])test(`owner cancellation fences late data during ${phase}`,async()=>{
 const owner=new AbortController(),p=deferred(),route=deferred();let routeSignal,committed=false;
 const result=chooseSceneBootstrap({...base(),signal:owner.signal,graceMs:phase==='grace'?1000:0,
  pinned:p.promise,loadRoute:s=>{routeSignal=s;return phase==='failed-route'?Promise.reject(new Error('failed')):route.promise;},
 }).then(r=>{committed=true;return r;});
 const rejected=assert.rejects(result,e=>e==='superseded');await tick();owner.abort('superseded');await rejected;
 if(routeSignal)assert(routeSignal.aborted);p.resolve({valid:true});route.resolve({status:200});await tick();assert.equal(committed,false);
});
test('out-of-bounds static data cannot win against a valid ordinary map',async()=>{
 const route={status:200};const result=await chooseSceneBootstrap({...base(),pinned:Promise.resolve({valid:false}),loadRoute:async()=>route});
 assert.equal(result.result,route);
});
