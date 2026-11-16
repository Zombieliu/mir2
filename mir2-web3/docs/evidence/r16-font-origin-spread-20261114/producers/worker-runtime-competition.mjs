import {execFileSync} from 'node:child_process';
import {startWorkerFontNetwork} from 'file:///C:/r16-font-lanes-20261114/release/signed-public/worker-font-network.mjs';
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
const root='C:/r16-network-isolation-20261114',origin='https://numeron.obelisk.build',revision='e6c923b7c669cb061a7a03703f05fadd91211c97';
const paths=['/original-ui/fonts/NotoSansTC.ttf','/original-ui/fonts/NotoSans-Regular.ttf'];
const pins=await Promise.all(paths.map(async path=>{const b=await readFile('E:/mir2-player-journey/mir2-web3/apps/web/public'+path);return{path,bytes:b.length,sha256:createHash('sha256').update(b).digest('hex')};}));
const runtimeMetadata=JSON.parse(execFileSync('git',['show',revision+':mir2-web3/apps/web/lib/generated/bevy_runtime_version_numeron_r16.json'],{cwd:'E:/mir2-player-journey',encoding:'utf8'}));
assert.equal(runtimeMetadata.version,'bevy-0c553a754308e5b8');const runtimeFile=runtimeMetadata.files.find(f=>f.path==='public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime_bg.wasm');assert(runtimeFile);
pins.push({path:'/bevy-runtime/v/bevy-0c553a754308e5b8/pkg-webgpu/mir2_bevy_runtime_bg.wasm',url:'https://assets.mir2.obelisk.build/mir2/v/20261108-r16-08ec86d5105c/bevy-runtime/v/bevy-0c553a754308e5b8/pkg-webgpu/mir2_bevy_runtime_bg.wasm',bytes:31452436,sha256:runtimeFile.sha256});
const require=createRequire('F:/mir2-builds/web-r16-20261104/tools/package.json'),{chromium}=require('playwright');
const out=root+'/worker-runtime-competition';await mkdir(out,{recursive:false});
const proof={kind:'isolated-public-font-reads-with-published-worker-and-fixed-runtime-transfer-without-game',startedAt:new Date().toISOString(),passed:false,measurementCompleted:false,origin,expectedRevision:revision,cacheMode:'reload',serviceWorkers:'allow',gameLogin:false,runtimeInstantiated:false,networkInterception:false,limits:{totalPerResourceMs:120000,maxDecodedBytes:33554432,observedOriginalHeaderMs:7000,observedOriginalPositiveByteIdleMs:15000},pins,network:[],browserClosed:false};
let browser,cdp,workerFontObserver;const requests=new Map();let started;
const at=()=>Math.round(performance.now()-started);
try{
 browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true});proof.chromeVersion=browser.version();
 const context=await browser.newContext({serviceWorkers:'allow',ignoreHTTPSErrors:false,acceptDownloads:false}),page=await context.newPage();
 const version=await page.goto(origin+'/version',{waitUntil:'domcontentloaded',timeout:30000});assert.equal(version.status(),200);proof.versionBefore=(await version.json()).revision;assert.equal(proof.versionBefore,revision);
 assert.equal(await page.evaluate(()=>!!navigator.serviceWorker.controller),false);
 const sourceWorker=execFileSync('git',['show',revision+':mir2-web3/apps/web/public/mir2-asset-worker.js'],{cwd:'E:/mir2-player-journey'}),sourceConfig=JSON.parse(execFileSync('git',['show',revision+':mir2-web3/config/numeron-r16-web-assets.json'],{cwd:'E:/mir2-player-journey',encoding:'utf8'}));
 const expectedWorker={bytes:sourceWorker.length,sha256:createHash('sha256').update(sourceWorker).digest('hex')};
 proof.workerBody=await page.evaluate(async()=>{const r=await fetch('/mir2-asset-worker.js',{cache:'reload',signal:AbortSignal.timeout(20000)}),b=await r.arrayBuffer();return{status:r.status,bytes:b.byteLength,sha256:[...new Uint8Array(await crypto.subtle.digest('SHA-256',b))].map(x=>x.toString(16).padStart(2,'0')).join('')}});assert.equal(proof.workerBody.status,200);assert.equal(proof.workerBody.bytes,expectedWorker.bytes);assert.equal(proof.workerBody.sha256,expectedWorker.sha256);
 proof.workerConfiguration=await page.evaluate(async config=>{
  await navigator.serviceWorker.register('/mir2-asset-worker.js');await navigator.serviceWorker.ready;
  if(!navigator.serviceWorker.controller)await new Promise(resolve=>navigator.serviceWorker.addEventListener('controllerchange',resolve,{once:true}));
  return await new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('configuration timeout')),5000),listener=e=>{if(e.data?.type!=='MIR2_ASSET_CACHE_CONFIGURED')return;clearTimeout(timer);navigator.serviceWorker.removeEventListener('message',listener);resolve(e.data)};navigator.serviceWorker.addEventListener('message',listener);
   navigator.serviceWorker.controller.postMessage({type:'MIR2_ASSET_CACHE_CONFIG',manifestVersion:config.version,cacheNamespace:'isolated-font-read',fontMirrorFirst:false,manifest:{version:config.version,remoteAssets:{enabled:true,assetBaseUrl:config.assetBaseUrl,browserFallbackBaseUrls:config.browserFallbackBaseUrls}}});});
 },sourceConfig);
 workerFontObserver=await startWorkerFontNetwork({context,elapsed:at,workerUrl:origin+'/mir2-asset-worker.js'});
 cdp=await context.newCDPSession(page);await cdp.send('Network.enable');
 cdp.on('Network.requestWillBeSent',e=>{const u=new URL(e.request.url);if(!pins.some(p=>(p.url??origin+p.path)===e.request.url))return;const row={origin:u.origin,path:u.pathname,requestAtMs:at(),priority:e.request.initialPriority??null,type:e.type,encodedProgress:[]};requests.set(e.requestId,row);proof.network.push(row);});
 cdp.on('Network.responseReceived',e=>{const r=requests.get(e.requestId);if(!r)return;const v=e.response;const h=Object.fromEntries(Object.entries(v.headers).map(([k,v])=>[k.toLowerCase(),v]));Object.assign(r,{headersAtMs:at(),status:v.status,protocol:v.protocol,serviceWorker:v.fromServiceWorker??false,diskCache:v.fromDiskCache??false,remoteIsLoopback:/^(?:127\.|::1$)/.test(v.remoteIPAddress??''),headers:Object.fromEntries(['content-type','content-length','content-encoding','cache-control','etag','age','x-vercel-cache','cf-cache-status','accept-ranges'].filter(k=>h[k]!==undefined).map(k=>[k,h[k]])),timing:v.timing?Object.fromEntries(['requestTime','proxyStart','proxyEnd','dnsStart','dnsEnd','connectStart','connectEnd','sslStart','sslEnd','sendStart','sendEnd','receiveHeadersStart','receiveHeadersEnd'].filter(k=>v.timing[k]!==undefined).map(k=>[k,v.timing[k]])):null});});
 cdp.on('Network.dataReceived',e=>{const r=requests.get(e.requestId);if(!r)return;r.decodedObserved=(r.decodedObserved??0)+(e.dataLength??0);r.encodedObserved=(r.encodedObserved??0)+(e.encodedDataLength??0);if(r.encodedProgress.length<4096)r.encodedProgress.push({atMs:at(),decodedTotal:r.decodedObserved,encodedTotal:r.encodedObserved});else r.progressDropped=(r.progressDropped??0)+1;});
 cdp.on('Network.loadingFinished',e=>{const r=requests.get(e.requestId);if(r)Object.assign(r,{finishedAtMs:at(),completeEncodedBytes:e.encodedDataLength});});
 cdp.on('Network.loadingFailed',e=>{const r=requests.get(e.requestId);if(r)Object.assign(r,{failedAtMs:at(),error:e.errorText});});
 started=performance.now();
 proof.transfers=await page.evaluate(async pins=>Promise.all(pins.map(async pin=>{
  const start=performance.now(),controller=new AbortController(),timer=setTimeout(()=>controller.abort(),120000),progress=[];let total=0,reader;const row={path:pin.path,status:null,complete:false,progress};
  try{
   const r=await fetch(pin.url??pin.path,{cache:'reload',redirect:'error',signal:controller.signal});row.headersMs=performance.now()-start;row.status=r.status;row.contentEncoding=r.headers.get('content-encoding');assertResponse: {if(!r.ok||!r.body)throw Error('HTTP_OR_BODY_UNAVAILABLE');}
   reader=r.body.getReader();const chunks=[];
   for(;;){const{done,value}=await reader.read();if(done)break;if(!value.byteLength)continue;total+=value.byteLength;if(total>33554432)throw Error('BODY_TOO_LARGE');chunks.push(value);if(progress.length<4096)progress.push({atMs:performance.now()-start,bytes:total});else row.progressDropped=(row.progressDropped??0)+1;}
   const bytes=new Uint8Array(total);let offset=0;for(const b of chunks){bytes.set(b,offset);offset+=b.length;}row.sha256=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(x=>x.toString(16).padStart(2,'0')).join('');row.complete=total===pin.bytes&&row.sha256===pin.sha256;
  }catch(e){row.errorName=e.name;row.errorCode=controller.signal.aborted?'TOTAL_120S_EXPIRED':'TRANSFER_FAILED';}
  finally{clearTimeout(timer);row.bytes=total;row.totalMs=performance.now()-start;try{reader?.releaseLock();}catch{}}
  let previous=row.headersMs??0,maxGap=0;for(const p of progress){maxGap=Math.max(maxGap,p.atMs-previous);previous=p.atMs;}maxGap=Math.max(maxGap,row.totalMs-previous);row.maxDecodedGapMs=maxGap;row.originalHeaderBudgetMet=row.headersMs!==undefined&&row.headersMs<=7000;row.originalIdleBudgetMet=maxGap<=15000;return row;
 })),pins);
 await page.waitForTimeout(100);proof.versionAfter=await page.evaluate(async()=>{const r=await fetch('/version',{cache:'no-store',signal:AbortSignal.timeout(20000)});return(await r.json()).revision;});assert.equal(proof.versionAfter,revision);
 assert.equal(await page.evaluate(()=>!!navigator.serviceWorker.controller),true);
 proof.measurementCompleted=true;proof.passed=proof.transfers.every(r=>r.complete);proof.allOriginalBudgetsMet=null;proof.consumerTimingNote='Worker intentionally returns full-font headers after body validation; consumer headers are not the upstream7s budget. Network-only observer is separate; it does not observe Worker decoded-reader idle directly.';for(const r of proof.transfers){delete r.originalHeaderBudgetMet;delete r.originalIdleBudgetMet;}
}catch(e){proof.failure={name:e.name,code:typeof e.code==='string'?e.code:'DIAGNOSTIC_FAILED'};}
finally{if(workerFontObserver)proof.workerNetwork=await workerFontObserver.finish();proof.workerObserverSha256=createHash('sha256').update(await readFile('C:/r16-font-lanes-20261114/release/signed-public/worker-font-network.mjs')).digest('hex');await cdp?.detach().catch(()=>{});await browser?.close();proof.browserClosed=true;proof.finishedAt=new Date().toISOString();proof.producerSha256=createHash('sha256').update(await readFile(new URL(import.meta.url))).digest('hex');await writeFile(out+'/receipt.json',JSON.stringify(proof,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({measurementCompleted:proof.measurementCompleted,passed:proof.passed,allOriginalBudgetsMet:proof.allOriginalBudgetsMet,failure:proof.failure,transfers:proof.transfers?.map(({path,complete,status,bytes,headersMs,totalMs,maxDecodedGapMs,errorCode})=>({path,complete,status,bytes,headersMs,totalMs,maxDecodedGapMs,errorCode})),browserClosed:proof.browserClosed}));}
if(!proof.measurementCompleted)process.exitCode=1;
