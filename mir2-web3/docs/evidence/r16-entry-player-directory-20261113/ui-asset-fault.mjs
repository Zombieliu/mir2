import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';

export async function installUiAssetFault(page, kind, pins) {
  assert(['font','skins'].includes(kind));
  await page.addInitScript(({kind,pins})=>{
    const targets=kind==='font'?['/original-ui/fonts/NotoSansTC.ttf']:['/original-ui/Prguse/827.png','/original-ui/Prguse/961.png'];
    const proof=window.__r16UiAssetRecovery={kind,document:crypto.randomUUID(),targets,reads:[],samples:[],runtimeIdentities:0,frameRegressed:false};
    const modules=new WeakSet(), counts=new Map();
    let lastFrame=0,lastKey='',forwardDepth=0;
    const read=name=>{try{return JSON.parse(window.__mir2BevyRuntime?.[name]?.()??'null');}catch{return null;}};
    const sample=()=>{
      const module=window.__mir2BevyRuntime;
      if(module&&!modules.has(module)){modules.add(module);proof.runtimeIdentities++;}
      const q=read('getMir2QuestUiStatus'),h=read('getMir2HudUiStatus');
      if(q?.frame){if(q.frame<lastFrame)proof.frameRegressed=true;lastFrame=q.frame;}
      const row={at:Date.now(),generation:q?.generation??null,frame:q?.frame??null,questReady:q?.ready===true,hudReady:h?.ready===true,error:q?.error??null};
      const key=JSON.stringify([row.generation,row.questReady,row.hudReady,row.error]);
      if(key!==lastKey&&proof.samples.length<100){proof.samples.push(row);lastKey=key;}
      return row;
    };
    window.__r16UiAssetRecoveryTimer=setInterval(sample,20);
    const wrap=next=>function(input,init){
      const forward=()=>{forwardDepth++;try{return next.call(this,input,init);}finally{forwardDepth--;}};
      if(forwardDepth>0||init!==undefined)return forward();
      const url=typeof input==='string'?input:input instanceof Request?input.url:input instanceof URL?input.href:null;
      if(!url)return forward();
      const pathname=new URL(url,location.href).pathname;
      const path=targets.find(path=>pathname.endsWith(path));
      const nativeBridge=/mir2_bevy_runtime\.js/.test(new Error().stack??'');
      if(!path||!nativeBridge||localStorage.getItem('__r16UiAssetRecoveryCompleted:'+kind)==='1')return forward();
      return (async()=>{
      const attempt=(counts.get(path)??0)+1;counts.set(path,attempt);
      const row={path,attempt,source:'bevy-wasm-fetch-with-str',requestedAt:Date.now(),before:sample()};proof.reads.push(row);
      if(attempt===1){
        if(kind==='skins'&&path.endsWith('/961.png')){
          const deadline=Date.now()+120000;
          while(!read('getMir2HudUiStatus')?.ready&&Date.now()<deadline)await new Promise(ok=>setTimeout(ok,20));
          row.beforeFailure=sample();
          if(!row.beforeFailure.hudReady)throw Error('fixture HUD did not become ready while Quest skin was Loading');
        }
        row.status=503;row.completedAt=Date.now();
        return new Response('controlled first native asset failure',{status:503,headers:{'cache-control':'no-store'}});
      }
      const response=await forward();row.status=response.status;
      if(response.ok){
        const bytes=await response.clone().arrayBuffer();row.bytes=bytes.byteLength;
        row.sha256=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),x=>x.toString(16).padStart(2,'0')).join('');
        row.matchesPinnedBody=!!pins[path]&&row.bytes===pins[path].bytes&&row.sha256===pins[path].sha256;
      }
      row.completedAt=Date.now();return response;
      })();
    };
    let current=wrap(window.fetch);
    Object.defineProperty(window,'fetch',{configurable:true,get:()=>current,set:next=>{current=wrap(next);}});
  },{kind,pins});
}

export async function verifyUiAssetRecovery(page, run) {
  const proof=await page.evaluate(()=>{
    clearInterval(window.__r16UiAssetRecoveryTimer);
    const proof=window.__r16UiAssetRecovery;
    const read=name=>{try{return JSON.parse(window.__mir2BevyRuntime?.[name]?.()??'null');}catch{return null;}};
    return {...proof,final:{quest:read('getMir2QuestUiStatus'),hud:read('getMir2HudUiStatus')}};
  });
  await writeFile(join(run,'ui-asset-recovery-before-assert.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
  assert.equal(proof.runtimeIdentities,1,'same runtime module');assert.equal(proof.frameRegressed,false,'same App frame counter');
  const errors=proof.samples.filter(x=>x.error==='quest UI font or skin failed to load');
  assert(errors.length>0,'observe actual Quest publish failure');
  assert.equal(proof.final.quest.ready,true);assert.equal(proof.final.quest.error,null);assert.equal(proof.final.hud.ready,true);
  assert(errors.every(x=>x.generation===proof.final.quest.generation),'character generation retained');
  proof.wallIntervals=[];
  for(const path of proof.targets){
    const reads=proof.reads.filter(x=>x.path===path);assert.equal(reads.length,2,path);
    assert.deepEqual(reads.map(x=>x.status),[503,200]);assert.equal(reads[1].matchesPinnedBody,true,path);
    proof.wallIntervals.push({path,milliseconds:reads[1].requestedAt-reads[0].completedAt});
  }
  proof.retryClockScope='Exact 1s/2s boundaries are checked with Bevy Time<Real> in real AssetServer tests. Browser wall intervals are observations of different frame phases; original 8s movement and 120s Native gates remain enforced by the full-game fixture.';
  if(proof.kind==='skins'){
    assert.equal(proof.reads.find(x=>x.path.endsWith('/961.png')&&x.attempt===1).beforeFailure.hudReady,true);
    assert(errors.some(x=>x.hudReady),'common fonts and HUD ready while the Quest-only skin remains failed');
  } else assert(errors.some(x=>!x.hudReady),'font failure keeps shared HUD closed');
  proof.passed=true;
  await page.evaluate(kind=>localStorage.setItem('__r16UiAssetRecoveryCompleted:'+kind,'1'),proof.kind);
  await writeFile(join(run,'ui-asset-recovery.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
  return proof;
}
