import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import {setTimeout as delay} from 'node:timers/promises';

export async function verifyPrewarm({page,run,browserRequests,stage}) {
  stage('verify-complete-body-HUD-prewarm-and-quest-markers');
  let snapshot;
  const started=Date.now();
  for(const end=Date.now()+60000;Date.now()<end;await delay(500)){
    snapshot=await page.evaluate(()=>{
      const value=window.__mir2CacheMetrics?.snapshot();
      return value?{runs:value.prewarmRuns,summary:{requested:value.summary.prewarmRequested,ok:value.summary.prewarmOk,failed:value.summary.prewarmFailed},orchestrator:window.__mir2AssetOrchestrator?.snapshot()}:null;
    });
    const hud=snapshot?.runs.filter(row=>row.name==='hud-core').at(-1);
    if(hud&&['done','error'].includes(hud.status)&&!snapshot.orchestrator?.activeBackgroundStage)break;
  }
  const samples=[];
  for(let n=0;n<14;n++){
    samples.push(await page.evaluate(()=>[...document.querySelectorAll('img.entity-quest-icon')].map(image=>({
      path:image.dataset.mir2OriginalSrc,complete:image.complete,width:image.naturalWidth,height:image.naturalHeight,
      failed:image.dataset.mir2LoadFailed??null,hidden:image.style.visibility==='hidden'
    }))));
    await delay(120);
  }
  const pins=await Promise.all([985,986].map(async frame=>{
    const body=await readFile('E:/mir2-player-journey/mir2-web3/apps/web/public/original-ui/Prguse/'+frame+'.png');
    return {frame,bytes:body.length,sha256:createHash('sha256').update(body).digest('hex')};
  }));
  const cached=await page.evaluate(async pins=>Promise.all(pins.map(async pin=>{
    const response=await caches.match(location.origin+'/original-ui/Prguse/'+pin.frame+'.png');
    if(!response)return {frame:pin.frame,missing:true};
    const body=await response.arrayBuffer();
    return {frame:pin.frame,bytes:body.byteLength,sha256:[...new Uint8Array(await crypto.subtle.digest('SHA-256',body))].map(x=>x.toString(16).padStart(2,'0')).join('')};
  })),pins);
  const stale=browserRequests.filter(row=>{
    const text=String(row.path??row.url??'');
    return text.includes('/bevy-entity-atlases/manifest.json')||text.includes('/bevy-entity-atlases/starter-bichon-base.png')||(/\/api\/scene\/crystal\?/.test(text)&&text.includes('x=398')&&text.includes('y=333'));
  }).map(row=>({path:row.path??row.url}));
  const proof={passed:false,selection:'latest current-session hud-core; prior cancelled runs retained',waitMs:Date.now()-started,snapshot,originalMarkerPins:pins,cached,samples,staleRequests:stale};
  const save=()=>writeFile(join(run,'prewarm-and-markers.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
  const hud=snapshot?.runs.filter(row=>row.name==='hud-core').at(-1);
  try{
    assert(hud&&hud.status==='done','HUD prewarm must reach a complete terminal state');
    assert.equal(hud.requested,113,'the current-session full HUD set remains required');assert.equal(hud.failed,0);assert.equal(hud.ok,hud.requested);
    assert(!snapshot.runs.some(row=>row.name==='bichon-spawn'));
    assert.equal(stale.length,0,'no retired showcase region or static entity atlas requests');
    assert.deepEqual(cached,pins,'both complete original animation frames must be cached');
    const markers=samples.flat().filter(row=>/\/Prguse\/(985|986)\.png$/.test(row.path??''));
    for(const frame of [985,986])assert(markers.some(row=>row.path.endsWith('/'+frame+'.png')&&row.complete&&row.width===28&&row.height===29&&!row.failed&&!row.hidden),'original quest phase '+frame+' must be visible and decoded');
    assert(markers.every(row=>!row.failed&&!row.hidden),'no negative-cache missing marker after complete prewarm');
    proof.passed=true;
  }finally{await save();}
  return proof;
}
