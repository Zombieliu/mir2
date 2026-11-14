import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import test from 'node:test';
import{startStaticTransportProbe,staticCacheMetadata}from'./lib/r16-static-transport.mjs';
const app='https://numeron.obelisk.build',cdn='https://assets.example/fixed';
const turn=()=>new Promise(setImmediate);
class Session extends EventEmitter{commands=[];detached=0;async send(name){this.commands.push(name);}async detach(){this.detached++;}}
function context(){const c=new EventEmitter(),pages=[{}],sessions=[];c.pages=()=>pages;c.newCDPSession=async()=>{const s=new Session();sessions.push(s);return s;};return{c,pages,sessions};}
async function fixture(extra={}){const f=context();let at=0;const probe=await startStaticTransportProbe({context:f.c,elapsed:()=>at,sourceBases:[app,cdn],...extra});return{...f,probe,clock:n=>{at=n;},s:f.sessions[0]};}
const request=(url=app+'/bevy-runtime/v/fixed/pkg/runtime.wasm',extra={})=>({requestId:'r1',request:{method:'GET',url,initialPriority:'High'},...extra});
const response=(extra={})=>({requestId:'r1',response:{status:200,protocol:'h2',headers:{Age:'42','Cf-Cache-Status':'HIT','Content-Encoding':'br','Cache-Control':'public, max-age=31536000, immutable'},timing:{sendStart:0.2,receiveHeadersEnd:10}},...extra});

test('records named cache metadata, initial/final priorities and bounded protocol timings',async()=>{
 const f=await fixture();f.s.emit('Network.requestWillBeSent',request());f.clock(10);f.s.emit('Network.responseReceived',response());
 f.s.emit('Network.resourceChangedPriority',{requestId:'r1',newPriority:'VeryHigh'});f.clock(50);f.s.emit('Network.loadingFinished',{requestId:'r1',encodedDataLength:123});
 const p=await f.probe.stop(),r=p.rows[0];assert.equal(r.initialPriority,'High');assert.equal(r.finalPriority,'VeryHigh');assert.equal(r.protocol,'h2');
 assert.deepEqual(r.cache,{age:42,'cf-cache-status':'HIT','content-encoding':'br','cache-control':'public, max-age=31536000, immutable'});
 assert.equal(r.networkTimingMs.receiveHeadersEnd,10);assert.equal(r.browserReportedEncodedBytes,123);assert.equal(r.outcome,'finished');assert(p.cleanupComplete);
 assert.deepEqual(f.s.commands,['Network.enable']);assert.equal(f.s.detached,1);
});

test('never reads request headers/payloads or private response field values',async()=>{
 const f=await fixture(),r=request();
 for(const name of['headers','postData'])Object.defineProperty(r.request,name,{get(){throw Error('private request field');}});
 const headers={age:'1','cf-ray':'abcdef0123456789-NRT'};
 for(const name of['Set-Cookie','Authorization','X-Private-Token'])Object.defineProperty(headers,name,{enumerable:true,get(){throw Error('private response field');}});
 f.s.emit('Network.requestWillBeSent',r);f.s.emit('Network.responseReceived',response({response:{status:200,headers}}));
 const p=await f.probe.stop();assert.deepEqual(p.rows[0].cache,{age:1,cfPop:'NRT'});assert(!JSON.stringify(p).includes('abcdef0123456789'));
});

test('excludes auth, billing, POST, other origins and credentials; removes static queries',async()=>{
 const f=await fixture();
 for(const path of['/api/wallet/dubhe/session','/api/channels/session/exchange','/api/billing/order','/playtest/ws'])f.s.emit('Network.requestWillBeSent',request(app+path+'?token=synthetic-private'));
 f.s.emit('Network.requestWillBeSent',request(app+'/original-ui/a.png',{request:{method:'POST',url:app+'/original-ui/a.png'}}));
 f.s.emit('Network.requestWillBeSent',request('https://untrusted.example/original-ui/a.png'));
 f.s.emit('Network.requestWillBeSent',request('https://user:synthetic-private@numeron.obelisk.build/original-ui/a.png'));
 f.s.emit('Network.requestWillBeSent',request(cdn+'/original-ui/a.png?token=synthetic-private#secret'));
 const p=await f.probe.stop();assert.equal(p.rows.length,1);assert.equal(p.rows[0].url,cdn+'/original-ui/a.png');assert(!JSON.stringify(p).includes('synthetic-private'));
});

test('an asset redirect to auth retires its binding before any auth response inspection',async()=>{
 const f=await fixture();f.s.emit('Network.requestWillBeSent',request());
 f.s.emit('Network.requestWillBeSent',request(app+'/api/wallet/dubhe/session?token=synthetic-private'));
 const privateResponse={};Object.defineProperty(privateResponse,'headers',{get(){throw Error('auth headers read');}});
 f.s.emit('Network.responseReceived',{requestId:'r1',response:privateResponse});
 const p=await f.probe.stop();assert.equal(p.rows.length,1);assert.equal(p.rows[0].outcome,'redirected');assert.equal(p.rows[0].cache,undefined);
});

test('row and page budgets retain drops without observing unbounded sessions',async()=>{
 const f=await fixture({maxRows:1,maxPages:1});f.s.emit('Network.requestWillBeSent',request());f.s.emit('Network.requestWillBeSent',request(app+'/original-ui/b.png',{requestId:'r2'}));
 f.c.emit('page',{});await turn();const p=await f.probe.stop();assert.equal(p.droppedRows,1);assert(p.truncated);assert.equal(p.skippedPages,1);assert.equal(f.sessions.length,1);
});

test('late setup is detached after its deadline and its uncertainty is retained',async t=>{
 t.mock.timers.enable({apis:['setTimeout']});const f=context();let resolve;f.c.newCDPSession=()=>new Promise(done=>{resolve=done;});
 const loading=startStaticTransportProbe({context:f.c,elapsed:()=>0,sourceBases:[app]});await turn();t.mock.timers.tick(3000);await turn();const probe=await loading;
 const p=await probe.stop();assert.equal(p.observerErrors.length,1);assert.equal(p.cleanupComplete,false);
 const late=new Session();resolve(late);await turn();assert.equal(late.detached,1);assert.equal(late.commands.length,0);
});

test('stop detaches its context listener and freezes all late events',async()=>{
 const f=await fixture();f.s.emit('Network.requestWillBeSent',request());const p=await f.probe.stop(),before=JSON.stringify(p.rows);
 assert.equal(f.c.listenerCount('page'),0);f.s.emit('Network.responseReceived',response());f.s.emit('Network.loadingFailed',{requestId:'r1',errorText:'private exception'});
 assert.equal(JSON.stringify(f.probe.snapshot().rows),before);p.rows[0].url='changed';assert.notEqual(f.probe.snapshot().rows[0].url,'changed');
});

test('only canonical public cache values are exported, including hostile header objects',()=>{
 assert.deepEqual(staticCacheMetadata(null),{});assert.deepEqual(staticCacheMetadata(new Proxy({},{ownKeys(){throw Error('private');}})),{});
 const result=staticCacheMetadata({'Age':'-2','Content-Length':'secret','Content-Encoding':'private encoding','Cf-Cache-Status':'secret','Cache-Control':'public, token=synthetic-private','Cf-Ray':'secret-NRT'});
 assert.deepEqual(result,{});assert.deepEqual(staticCacheMetadata({'X-Vercel-Cache':'hit','Cache-Control':'public, max-age=60','Content-Length':'37'}),{'content-length':37,'x-vercel-cache':'HIT','cache-control':'public, max-age=60'});
});
