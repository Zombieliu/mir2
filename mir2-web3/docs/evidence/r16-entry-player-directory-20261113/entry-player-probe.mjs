import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
export async function installPlayerProbe(page){
 await page.addInitScript(()=>{
  const proof=window.__r16EntryPlayerProbe={rows:[]};let previous='';
  const sample=()=>{
   const gate=window.__mir2SceneGate;if(gate?.screen!=='game')return;
   const first=(window.__mir2CacheMetrics?.milestones??window.__mir2PendingCacheMilestones??[]).find(row=>row.name==='firstPlayableFrame');
   const own=first?.detail?.playerObjectId;
   const stack=own?document.querySelector('.entity-sprite-stack[data-object-id="'+CSS.escape(String(own))+'"]'):
    document.querySelector('.entity-sprite-stack.self-player, .entity-sprite-stack.selfPlayer');
   const bodies=[...(stack?[stack]:document.querySelectorAll('.entity-sprite-stack'))].flatMap(el=>[...el.querySelectorAll('img.entity-sprite-layer.body')]);
   const body=bodies.find(image=>/CArmour\/00\//.test(image.getAttribute('src')??''))??bodies[0];
   const row={at:Date.now(),waitingForEntryPlayer:gate.waitingForEntryPlayer,ready:gate.sceneInteractionReady,
    pending:document.querySelector('.client-stage-frame')?.classList.contains('scene-assets-pending'),
    hasBody:!!body,bodyDecoded:!!body?.complete&&body.naturalWidth>0&&body.style.visibility!=='hidden',
    bodyPath:body?.currentSrc?.startsWith('data:')?'data':body?new URL(body.currentSrc||body.src,location.href).pathname:null,
    firstPlayable:!!first};
   const key=JSON.stringify({...row,at:0});if(key!==previous){previous=key;proof.rows.push(row);}
  };
  window.__r16EntryPlayerProbeTimer=setInterval(sample,20);
 });
}
export async function verifyEarlyPlayer(page,run,server,firstPlayableAt,startAckAt){
 const proof=await page.evaluate(()=>window.__r16EntryPlayerProbe);
 await writeFile(run+'/early-player-before-assert.json',JSON.stringify({server,proof,firstPlayableAt,startAckAt},null,2)+'\n',{flag:'wx'});
 assert.equal(server.releasedAt,null,'full pack and remote directory remain withheld');
 const complete=server.reads.filter(row=>row.kind==='complete-player-pack');assert(complete.length>=1);
 assert(complete.every(row=>!row.headersAt&&!row.finishedAt));
 const loose=server.reads.filter(row=>row.kind==='loose-player-frame'&&row.status===200&&row.finishedAt<=firstPlayableAt);
 assert(loose.some(row=>row.path.startsWith('/original-ui/CArmour/00/')),'first playable precedes full pack but follows a real body PNG');
 const gated=proof.rows.filter(row=>row.waitingForEntryPlayer===true&&!row.ready&&!row.firstPlayable);
 assert(gated.length>0);
 assert(firstPlayableAt-Math.min(...gated.map(row=>row.at))>=3000,'no 3s metadata-only escape');
 assert(firstPlayableAt-startAckAt>=6000,'actual delayed-body gate retained');
 const visible=await page.evaluate(()=>{
  const first=(window.__mir2CacheMetrics?.milestones??window.__mir2PendingCacheMilestones??[]).find(row=>row.name==='firstPlayableFrame');
  const stack=document.querySelector('.entity-sprite-stack[data-object-id="'+CSS.escape(String(first?.detail?.playerObjectId))+'"]');
  const body=stack?.querySelector('img.entity-sprite-layer.body');
  const rect=body?.getBoundingClientRect();
  return {decoded:!!body?.complete&&body.naturalWidth>0,visible:!!rect&&rect.width>0&&rect.height>0&&getComputedStyle(body).visibility!=='hidden',
   source:body?.currentSrc?.startsWith('data:')?'data':body?new URL(body.currentSrc,location.href).pathname:null,
   gate:window.__mir2SceneGate?.sceneInteractionReady===true};
 });
 assert.equal(visible.decoded,true);assert.equal(visible.visible,true);assert.equal(visible.gate,true);assert.match(visible.source,/^\/original-ui\/CArmour\/00\//);
 const result={passed:true,rows:proof.rows,visible,withheldFullPack:true,firstPlayableAt,startAckAt,clickToPlayableScope:'controlled 6s initial body unavailability; not public latency'};
 await writeFile(run+'/early-player.json',JSON.stringify(result,null,2)+'\n',{flag:'wx'});return result;
}
