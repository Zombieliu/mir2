import assert from 'node:assert/strict';
import {verifyPrewarm} from './verify-prewarm-session.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {verifyPublicQuest} from './verify-public-quest.mjs';
const actions=new Set(['walk','run','turn','attack','attackV2','magic','interact','acceptQuest','finishQuest','pickUpTile','useItem']);
export async function verifySceneState({page,run,sent,frames,stage,proxyRequests,runtimeNetwork,browserRequests,recovery=false,warm=false,runtimeRetry=false,phaseStartAt=0,firstPlayableAt=0,startPosition={x:288,y:616},targetPosition={x:289,y:616},movementKey="ArrowRight"}){
 const read=proxyRequests.find(row=>row.scope==='static'&&row.status===200&&row.controlledHeaderDelayMs===17000);assert(read);
 assert.equal(read.deliveredSha256,'5c08d1fd4e81d92a7be20eb98c4c6288515749d5d459978dcf78fe2b86bdf325');
 stage('verify-hosted-font-policy-and-wait-native-hud');
 const worker=page.context().serviceWorkers().find(w=>w.url().includes('mir2-asset-worker.js'));assert(worker);
 const policy=await worker.evaluate(()=>({enabled:runtimeConfig.fontMirrorFirst,assetVersion:runtimeConfig.assetVersion,mirrorCode:typeof currentReleaseFontMirrors==='function',bodyIdleMs:FONT_MIRROR_BODY_IDLE_TIMEOUT_MS,bodyLimit:FONT_MIRROR_MAX_BYTES}));
 assert.equal(policy.enabled,true);assert.equal(policy.assetVersion,'20261108-r16-08ec86d5105c');assert(policy.mirrorCode);assert.equal(policy.bodyIdleMs,15000);assert.equal(policy.bodyLimit,33554432);
 await writeFile(join(run,(recovery?'recovery-':'')+'font-route-policy.json'),JSON.stringify(policy,null,2)+'\n',{flag:'wx'});
 const nativeWaitStarted=firstPlayableAt||Date.now(),runtimeProgress=[];const phase=recovery?'recovery-':'';
 const monitor=setInterval(async()=>{
  const row=await page.evaluate(()=>({booted:window.__mir2BevyRuntimeBooted??false,failed:window.__mir2BevyRuntimeFailed??false,fonts:document.fonts.status,prewarm:window.__mir2CacheMetrics?.snapshot().summary.prewarmOk,hud:window.__mir2BevyRuntime?.getMir2HudUiStatus?.()})).catch(()=>null);
  if(row){row.waitMs=Date.now()-nativeWaitStarted;runtimeProgress.push(row);console.log(JSON.stringify({phase:'native-readiness-progress',...row}));}
 },15000);
 try {
 await page.waitForFunction(()=>{
  try{return JSON.parse(window.__mir2BevyRuntime?.getMir2HudUiStatus?.()??'null')?.ready
    &&JSON.parse(window.__mir2BevyRuntime?.getMir2QuestUiStatus?.()??'null')?.ready;}catch{return false;}
 },null,{timeout:Math.max(1,(recovery?180000:120000)-(Date.now()-nativeWaitStarted))});
 } catch(error) {
  if(!recovery&&error.name==='TimeoutError')error.code='NATIVE_READINESS_EXCEEDED_120_SECONDS';
  throw error;
 } finally { clearInterval(monitor);await writeFile(join(run,phase+'runtime-readiness-progress.json'),JSON.stringify(runtimeProgress,null,2)+'\n',{flag:'wx'}); }
 const nativeReadyWaitMs=Date.now()-nativeWaitStarted;
 const native=await page.evaluate(()=>({hud:JSON.parse(window.__mir2BevyRuntime.getMir2HudUiStatus()),quest:JSON.parse(window.__mir2BevyRuntime.getMir2QuestUiStatus()),backend:window.__mir2BevyRuntimeDebug?.selectedBackend,packageId:window.__mir2BevyRuntimeDebug?.packageId}));
 if(process.env.R16_QA_MODE==='webgl2')assert.equal(native.packageId,'webgl2-shared');
 const stalledAppFontRequests=proxyRequests.filter(row=>row.scope==='blocked-app-font'&&row.requestedAt>=phaseStartAt).length;if(!recovery)assert.equal(stalledAppFontRequests,0,'hosted SC font must bypass the stalled app source');
 const expected=JSON.parse(await readFile('C:/r16-hud-font-route-20261111/public-font-readback.json','utf8')).rows;
 for(let attempt=0;attempt<40;attempt++){
   if(await worker.evaluate(()=>fontAssetInflightFetches.size===0))break;
   await delay(250);
 }
 const fontCache=await worker.evaluate(async names=>{
   const cache=await caches.open(cacheName('static-critical')),rows=[];
   for(const name of names){
     const response=await cache.match(new Request(location.origin+'/original-ui/fonts/'+name));
     if(!response){rows.push({name,missing:true});continue;}
     const bytes=await response.arrayBuffer(),sha256=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(v=>v.toString(16).padStart(2,'0')).join('');
     rows.push({name,bytes:bytes.byteLength,sha256});
   }
   return rows;
 },expected.map(row=>row.name));
 for(const row of fontCache){const pin=expected.find(v=>v.name===row.name);assert.equal(row.bytes,pin.bytes,row.name);assert.equal(row.sha256,pin.sha256,row.name);}
 const fontTimings=await worker.evaluate(()=>performance.getEntriesByType('resource').filter(row=>row.name.includes('/original-ui/fonts/')).map(row=>({url:row.name,durationMs:row.duration,decodedBodySize:row.decodedBodySize})));
 const scPath='/original-ui/fonts/NotoSansCJKsc-Regular.otf',sc=runtimeNetwork.find(row=>row.path===scPath&&row.status===200&&row.serviceWorker===true&&Number.isFinite(row.finishedAt));
 assert(sc,'completed browser delivery must come from the actual worker');
 assert.equal(sc.decodedBytes,expected.find(row=>row.name==='NotoSansCJKsc-Regular.otf').bytes);
 const complete=path=>runtimeNetwork.filter(row=>row.path.endsWith(path)&&row.status===200&&Number.isFinite(row.finishedAt)&&row.bodySha256&&Number.isFinite(row.completeBodyBytes)).at(-1);
 const observationStarted=Date.now();
 for(const until=Date.now()+5000;Date.now()<until&&(!complete('/mir2_bevy_runtime.js')||!complete('/mir2_bevy_runtime_bg.wasm'));await delay(50)){}
 const script=complete('/mir2_bevy_runtime.js'),wasm=complete('/mir2_bevy_runtime_bg.wasm');
 assert(script&&wasm,'the actually completed exact package script and binary must be observed');
 for(const row of [script,wasm]){assert.equal(row.completeBodyBytes,row.expectedBytes);assert.equal(row.bodySha256,row.expectedSha256);}
 await writeFile(join(run,'runtime-body-observation.json'),JSON.stringify({nativeGateUnchanged:true,observationWaitMs:Date.now()-observationStarted,script,wasm},null,2)+'\n',{flag:'wx'});
 const firstScript=runtimeNetwork.find(row=>row.observer==='playwright-complete-body'&&row.path.endsWith('/mir2_bevy_runtime.js'));
 const firstWasm=runtimeNetwork.find(row=>row.observer==='playwright-complete-body'&&row.path.endsWith('/mir2_bevy_runtime_bg.wasm'));
 if(!warm&&!runtimeRetry)assert(Math.abs(firstScript.requestedAt-firstWasm.requestedAt)<1000,'first runtime script and binary attempts must start together; a later exact-source fallback is not a serialized first attempt');
 await writeFile(join(run,'runtime-parallel-delivery.json'),JSON.stringify({runtimeRetry,parallelGateApplicable:!warm&&!runtimeRetry,script,wasm,requestGapMs:Math.abs(script.requestedAt-wasm.requestedAt)},null,2)+'\n',{flag:'wx'});
 const selectedSource=await worker.evaluate(path=>currentReleaseFontMirrors(new Request(location.origin+path),createRemoteAssetRequests(new Request(location.origin+path)))[0]?.url,scPath);
 assert.equal(new URL(selectedSource).origin,'http://localhost:20631');
 assert(new URL(selectedSource).pathname.endsWith('/mir2/v/20261108-r16-08ec86d5105c'+scPath));
 await writeFile(join(run,'native-font-delivery.json'),JSON.stringify({sc,selectedSource,sourceEvidence:warm?'persisted six-font cache present before warm Start; actual worker bodies and exact hashes':'separate fixed current-release HTTP mirror; actual worker bodies and exact hashes; not WAN coverage'},null,2)+'\n',{flag:'wx'});
 await writeFile(join(run,'native-font-readiness.json'),JSON.stringify({passed:true,policy,native,nativeReadyWaitMs,fontCache,fontTimings,stalledAppFontRequests},null,2)+'\n',{flag:'wx'});
 if(!warm){
   const scRead=runtimeNetwork.find(row=>row.path==='/original-ui/fonts/NotoSansCJKsc-Regular.otf'&&row.status===200&&row.serviceWorker===true&&Number.isFinite(row.finishedAt));
   assert(scRead);assert(Number.isFinite(wasm.finishedAt));
   const overlapMs=wasm.finishedAt-scRead.requestedAt;
   if(process.env.R16_FONT_MIRROR_FAULT==='1')assert(overlapMs>0,'recovered CJK prewarm must still overlap the pending renderer');
   else if(process.env.R16_FONT_CASE==='candidate')assert(overlapMs>10000,'CJK prewarm must overlap the deliberately pending renderer');
   else assert(overlapMs<1000,'committed baseline must retain the observed post-renderer font waterfall');
   await writeFile(join(run,'font-overlap-verification.json'),JSON.stringify({passed:true,case:process.env.R16_FONT_CASE,overlapMs,nativeReadyWaitMs,sc:scRead,wasm},null,2)+'\n',{flag:'wx'});
 }
 const prewarm=await verifyPrewarm({page,run,browserRequests,stage});
 stage('ordinary-q-with-original-native-fonts');
 const proof={get ws(){return sent.filter(row=>actions.has(row.type)).map(row=>row.type);}};
 const diary=await verifyPublicQuest({page,proof,output:run});assert.equal(diary.renderer,'ordinary-shared-native');
 stage('one-real-step-after-native-fonts-ready');
 const frame=page.locator('.client-stage-frame'),before=frames.length;
 await frame.focus();await page.keyboard.press(movementKey);
 let finalTile;
 for(const end=Date.now()+12000;Date.now()<end&&!finalTile;await delay(60)){
  for(const row of frames.slice(before).reverse()){
   if(row.type==='worldSnapshot'){const value=row.payload.entities?.find(e=>String(e.objectId)===String(row.payload.playerObjectId));if(value&&(value.x!==startPosition.x||value.y!==startPosition.y)){finalTile={x:value.x,y:value.y};break;}}
   if(row.packet==='UserLocation'){const value=row.payload.location??row.payload.position??row.payload;if(Number.isInteger(value.x)&&(value.x!==startPosition.x||value.y!==startPosition.y)){finalTile={x:value.x,y:value.y};break;}}
  }
 }
 const keyRows=await page.evaluate(()=>window.__r16InputTrace??[]);await writeFile(join(run,'movement-input-trace.json'),JSON.stringify({startPosition,targetPosition,finalTile,keys:keyRows,sent:sent.filter(row=>actions.has(row.type)).map(row=>({type:row.type,seq:row.seq,at:row.at,direction:row.direction})),authority:frames.slice(before).filter(row=>['UserLocation','ObjectWalk','ObjectRun','UserAttackMove'].includes(row.packet)).map(row=>({packet:row.packet,at:row.at,payload:row.payload}))},null,2)+'\n',{flag:'wx'});assert.deepEqual(finalTile,targetPosition);
 const stress=[];
 if(!warm){for(let index=0;index<8;index++){
  await delay(2000);const from=index%2===0?targetPosition:startPosition,to=index%2===0?startPosition:targetPosition,key=index%2===0?'ArrowLeft':'ArrowRight';
  const canvas=await page.evaluate(()=>window.__mir2BevyRuntime?.getMir2RuntimeCanvasSelector?.());await page.locator(canvas).focus();await page.keyboard.press('q');
  await page.waitForFunction(()=>{try{return JSON.parse(window.__mir2BevyRuntime.getMir2QuestUiStatus()).questLogOpen===true;}catch{return false;}},null,{timeout:5000,polling:'raf'});
  await page.keyboard.press('q');await page.waitForFunction(()=>{try{const q=JSON.parse(window.__mir2BevyRuntime.getMir2QuestUiStatus());return q.ready&&q.questLogOpen===false;}catch{return false;}},null,{timeout:5000,polling:'raf'});
  const at=Date.now(),marker=frames.length;await frame.focus();await page.keyboard.press(key);
  let arrived;for(const end=Date.now()+12000;Date.now()<end&&!arrived;await delay(40))for(const edge of frames.slice(marker)){if(edge.packet==='UserLocation'){const p=edge.payload.location??edge.payload.position??edge.payload;if(p.x===to.x&&p.y===to.y){arrived={x:p.x,y:p.y};break;}}}
  const state=await page.evaluate(()=>({movement:document.querySelector('[data-movement-input-status]')?.getAttribute('data-movement-input-status'),quest:window.__mir2BevyRuntime?.getMir2QuestUiStatus?.(),hud:window.__mir2BevyRuntime?.getMir2HudUiStatus?.(),focus:{tag:document.activeElement?.tagName,id:document.activeElement?.id},keys:window.__r16InputTrace?.slice(-8)}));
  stress.push({index,at,from,to,key,arrived,state,wire:sent.filter(r=>actions.has(r.type)&&r.at>=at).map(r=>({type:r.type,at:r.at,direction:r.direction}))});
  await writeFile(join(run,'immediate-q-close-movement.json'),JSON.stringify(stress,null,2)+'\n');assert.deepEqual(arrived,to,'original immediate arrow after Native Q close');
 }}
 await frame.screenshot({path:join(run,'native-hud-after-walk.png')});
 return{passed:true,prewarm,nativeReadyWaitMs,policy,native,fontCache,stalledAppFontRequests,diary,finalTile,ordinaryLogouts:0};
}
