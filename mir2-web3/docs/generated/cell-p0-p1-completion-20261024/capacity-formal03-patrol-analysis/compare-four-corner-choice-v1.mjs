import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
const base='C:/mir2-build/p01-completion-20261024', output=path.join(base,'formal03-analysis');
const project='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const moduleFile=path.join(project,'apps/web/scripts/quest-agent/protocol-navigation.mjs');
const {loadProtocolCollisionMap,protocolMapCellIsWalkable,findProtocolWalkPath}=await import(pathToFileURL(moduleFile));
const map=await loadProtocolCollisionMap('0');
const frames=JSON.parse(fs.readFileSync(path.join(output,'five-public-position-frames-v1.json'),'utf8'));
const home={x:252,y:671},goals=[{x:254,y:671},{x:254,y:673},{x:252,y:673},{x:252,y:671}],caseDefinitions=[
  {frameIndex:0,origin:{x:252,y:671},initialCursor:0},
  {frameIndex:0,origin:{x:252,y:670},initialCursor:0},
  {frameIndex:0,origin:{x:254,y:670},initialCursor:0},
  {frameIndex:0,origin:{x:254,y:671},initialCursor:0},
  {frameIndex:3,origin:{x:254,y:673},initialCursor:2},
  {frameIndex:3,origin:{x:252,y:674},initialCursor:2}
];
const results=caseDefinitions.map(def=>{
  const frame=frames.frames[def.frameIndex],obstacles=frame.entities.filter(e=>!e.dead&&(e.kind==='monster'||(e.kind==='player'&&e.name!=='Lc7785700')));
  const occupied=new Set(obstacles.map(p=>`${p.x},${p.y}`));
  const clear=p=>protocolMapCellIsWalkable(map,p)&&!occupied.has(`${p.x},${p.y}`);
  const order=Array.from({length:4},(_,i)=>(def.initialCursor+i)%4);
  let fast=null;
  if(def.origin.x>=home.x&&def.origin.x<=home.x+2&&def.origin.y>=home.y&&def.origin.y<=home.y+2){
    for(const index of order){const corner=goals[index],dx=corner.x-def.origin.x,dy=corner.y-def.origin.y,length=Math.max(Math.abs(dx),Math.abs(dy));
      if(length<2||(dx&&dy&&Math.abs(dx)!==Math.abs(dy)))continue;
      const x=Math.sign(dx),y=Math.sign(dy),first={x:def.origin.x+x,y:def.origin.y+y},target={x:def.origin.x+2*x,y:def.origin.y+2*y};
      const cells=[first,target,...(x&&y?[{x:def.origin.x+x,y:def.origin.y},{x:def.origin.x,y:def.origin.y+y},{x:first.x+x,y:first.y},{x:first.x,y:first.y+y}]:[])];
      if(cells.every(clear)){fast={goalIndex:index,kind:'run',first,target};break;}
    }
  }
  const routes=order.map(index=>{const goal=goals[index];if(goal.x===def.origin.x&&goal.y===def.origin.y)return{goalIndex:index,goal,kind:null,reason:'already-at-corner'};
    const route=findProtocolWalkPath({map,start:def.origin,target:goal,dynamicObstacles:obstacles,maxExpanded:4000});
    if(!route||route.length<2)return{goalIndex:index,goal,kind:null,reason:'no-path-or-4000-node-limit'};
    const first=route[1],second=route[2],run=Boolean(second&&second.x-first.x===first.x-def.origin.x&&second.y-first.y===first.y-def.origin.y);
    return{goalIndex:index,goal,kind:run?'run':'walk',first,target:run?second:first,route};
  });
  const old=fast??routes.find(r=>r.kind),candidate=fast??routes.find(r=>r.kind==='run')??routes.find(r=>r.kind);
  return{...def,frameSequence:frame.header.sequence,obstacles:obstacles.map(({objectId,name,kind,x,y,dead,hp})=>({objectId,name,kind,x,y,dead,hp})),
    primedAssumed:true,fastPlan:fast,routes,oldChoice:old,candidateChoice:candidate,addedRealRun:old?.kind==='walk'&&candidate?.kind==='run'};
});
const sha=file=>{const b=fs.readFileSync(file);return{file,bytes:b.length,sha256:crypto.createHash('sha256').update(b).digest('hex')}};
const result={schemaVersion:1,createdAt:new Date().toISOString(),scope:'Read-only approximate-obstacle fixture computation on the real packaged map0. No harness/source changes or live server. Original4 corners and4000-node existing planner only.',
  candidate:'Keep current primed native fastpath. In native+primed fallback only, inspect the existing4corner routes; choose first genuinely collinear2-step Run, else retain original first Walk. No extra targets, fabricated positions or changed denominator.',
  originalGoals:goals,cases:results,
  addedRealRunCases:results.filter(r=>r.addedRealRun).length,
  limitations:['The five public frames are merged observation snapshots, not actor00 exact send-time AOI. The fixture uses their nearby live obstacles and current known peer, excluding the spectator self/remote duplication of actor00. It does not reconstruct every original decision.',
    'Transfer list is empty for this local fixture; these candidate cell segments were traversed by ordinary successful movement in the original run. A product change must still reuse the real existing movement/transfer/routeAllowed and conservative-union checks, with no relaxation.',
    'Priming is explicitly assumed true to isolate route choice. Real mode/priming/cadence checks must remain and can still yield Walk or fail load quality.',
    'A larger number of real Run choices is not proof of a future60s window or sustainable100-player workload. Fresh tests, source closure and a new live feasibility gate are required.'],
  inputPins:[sha(moduleFile),sha(path.join(project,'apps/web/lib/generated/crystal-map-pack/0.map.gz')),sha(path.join(output,'five-public-position-frames-v1.json'))]};
const out=path.join(output,'four-corner-choice-fixture-v1.json');fs.writeFileSync(out,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({artifact:sha(out),cases:results.map(r=>({origin:r.origin,seq:r.frameSequence,fast:r.fastPlan,old:r.oldChoice&&{kind:r.oldChoice.kind,goalIndex:r.oldChoice.goalIndex,first:r.oldChoice.first,target:r.oldChoice.target},candidate:r.candidateChoice&&{kind:r.candidateChoice.kind,goalIndex:r.candidateChoice.goalIndex,first:r.candidateChoice.first,target:r.candidateChoice.target},addedRealRun:r.addedRealRun}))}));
