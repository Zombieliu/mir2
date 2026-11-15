import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import {auditStaticCacheBodies} from './lib/static-cache-audit.mjs';
const bytes=new Uint8Array([1,2,3]),path='/original-ui/Prguse/985.png';
const pin={path,bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')};
const worker=readFileSync(new URL('../public/mir2-asset-worker.js',import.meta.url),'utf8');
function actualCacheName(kind,version){const context=vm.createContext({self:{location:{origin:'https://game.example.test'},addEventListener(){}},URL,Map,Set});vm.runInContext(worker,context);return vm.runInContext(`runtimeConfig.version=${JSON.stringify(version)}; cacheName(${JSON.stringify(kind)})`,context);}
function fixture(t,{body=bytes,oldOnly=false}={}){
 const saved=new Map(['window','location','caches','fetch'].map(k=>[k,Object.getOwnPropertyDescriptor(globalThis,k)]));
 const calls=[];globalThis.window={__mir2AssetCache:{cacheNamespace:'current'}};globalThis.location={origin:'https://game.example.test'};
 globalThis.fetch=()=>{throw Error('Audit must never fetch');};
 globalThis.caches={async keys(){return [actualCacheName('static-runtime','old'),...(oldOnly?[]:[actualCacheName('static-critical','current')])];},async open(name){calls.push({open:name});return{async match(url){calls.push({match:url});return new Response(body);}};}};
 t.after(()=>{for(const[k,descriptor]of saved)if(descriptor)Object.defineProperty(globalThis,k,descriptor);else delete globalThis[k];});
 return{page:{evaluate(fn,arg){return fn(arg);}},calls};
}
test('cache audit reads exact complete bytes only in the current static namespace',async t=>{
 const f=fixture(t),result=await auditStaticCacheBodies(f.page,[pin]);assert(result.observed);assert.equal(result.matched,1);assert.equal(result.missing,0);assert.equal(result.mismatched,0);assert.equal(result.rows[0].sha256,pin.sha256);
 assert.deepEqual(f.calls,[{open:actualCacheName('static-critical','current')},{match:'https://game.example.test'+path}]);
});
test('an older namespace cannot substitute for the missing current body',async t=>{
 const f=fixture(t,{oldOnly:true}),result=await auditStaticCacheBodies(f.page,[pin]);assert.equal(result.observed,false);assert.equal(result.reason,'current-static-cache-unavailable');assert.equal(result.missing,undefined);assert.deepEqual(f.calls,[]);
});
test('an existing current cache with no matching body reports an observed missing entry',async t=>{
 const f=fixture(t);caches.open=async()=>({async match(){return undefined;}});
 const result=await auditStaticCacheBodies(f.page,[pin]);assert(result.observed);assert.equal(result.missing,1);assert.equal(result.matched,0);
});
test('a prior cache schema with the same namespace cannot satisfy the current Worker',async t=>{
 const f=fixture(t);caches.keys=async()=>['mir2-asset-cache-sw7-static-critical-current'];
 assert.equal((await auditStaticCacheBodies(f.page,[pin])).reason,'current-static-cache-unavailable');
});
for(const body of[new Uint8Array([1,2]),new Uint8Array([3,2,1]),new Uint8Array([1,2,3,4])])test(`a ${body.length}-byte truncated, corrupt or oversized cache entry cannot pass`,async t=>{
 const f=fixture(t,{body}),result=await auditStaticCacheBodies(f.page,[pin]);assert.equal(result.matched,0);assert.equal(result.mismatched,1);assert.equal(result.rows[0].matches,false);
});
test('auth URLs, arbitrary paths, duplicate pins and excessive bodies reject before browser work',async()=>{
 const page={evaluate(){throw Error('Must not evaluate');}};
 for(const pins of[[{...pin,path:'/api/dubhe-connect/login?intent=private'}],[{...pin,path:'https://other.example.test'+path}],[pin,pin],[{...pin,bytes:33554433}]])await assert.rejects(auditStaticCacheBodies(page,pins),/Invalid/);
});
test('a missing namespace and an unavailable page are explicit observations, never matches',async t=>{
 const f=fixture(t);delete window.__mir2AssetCache;
 assert.equal((await auditStaticCacheBodies(f.page,[pin])).reason,'cache-namespace-unavailable');
 assert.equal((await auditStaticCacheBodies({evaluate(){throw TypeError('closed');}},[pin])).reason,'audit-unavailable');
});
test('a bounded observation timeout does not report a missing asset or perform a fetch',async()=>{
 const result=await auditStaticCacheBodies({evaluate(){return new Promise(()=>{});}},[pin],{timeoutMs:10});assert.equal(result.observed,false);assert.equal(result.reason,'audit-timeout');
});
