import fs from 'node:fs';
import { loadGeometry, PARAMETERS, ROOT, STEPS, key, dist, pin, writeNew } from './static-lib.mjs';

// Exactly two bounded ordering attempts, no new homes or search over geometry.
// The first ten coordinates and their serialized ten-scene stay byte-exact.
const sourceFile=ROOT+'/candidate-100-compact-v2.json',prefixFile=ROOT+'/candidate-10-compact-v2.json';
const source=JSON.parse(fs.readFileSync(sourceFile)),prefixBytes=fs.readFileSync(prefixFile),g=await loadGeometry();
const allKeys=new Set(source.anchors.map(key));if(allKeys.size!==100)throw new Error('Original coordinate set differs');
const reserved=new Set(source.anchors.map(key));
function edge(start,home,active){
  const cluster=g.cluster([...active,home]),allowed=p=>g.driver.insideCombatCoverage(cluster,p);
  const goals=STEPS.map(([dx,dy])=>({x:home.x+dx,y:home.y+dy})).filter(p=>g.clear(p)&&allowed(p)&&dist(p,start)>0&&!reserved.has(key(p))).sort((a,b)=>dist(a,start)-dist(b,start));
  for(const goal of goals.slice(0,4)){const points=g.route(start,goal,source.anchors,-1,allowed);if(points)return{steps:points.length-1,goal};}
  return null;
}
const outputs=[];
for(const mode of ['nearest','continuous']){
  const active=source.anchors.slice(0,10).map(p=>({...p})),remaining=new Map(source.anchors.slice(10).map(p=>[key(p),p])),choices=[];
  let previous=active[9],failed=null;
  while(active.length<100){
    const index=active.length,cluster=g.cluster(active),allowed=p=>g.driver.insideCombatCoverage(cluster,p),component=g.component(active[0],allowed,active.slice(1));
    const available=[...remaining.values()].filter(p=>component.has(p));
    const fighter=index%10===5;
    if(mode==='nearest'||fighter)available.sort((a,b)=>dist(a,previous)-dist(b,previous)||a.y-b.y||a.x-b.x);
    else available.sort((a,b)=>{
      const bandA=Math.floor((a.y-512)/12),bandB=Math.floor((b.y-512)/12);
      return bandA-bandB||((bandA%2)?b.x-a.x:a.x-b.x)||a.y-b.y;
    });
    let chosen,proof;
    if(fighter){chosen=available[0];}
    else if(mode==='nearest'){
      // Geographic nearest8 only; each original scout edge tries at most four
      // original adjacent goals. This offline sort does not change runtime.
      for(const p of available.slice(0,8)){const result=edge(previous,p,active);if(result&&(!proof||result.steps<proof.steps)){chosen=p;proof=result;}}
    }else{
      for(const p of available){const result=edge(previous,p,active);if(result){chosen=p;proof=result;break;}}
    }
    if(!chosen){failed={actorIndex:index,remaining:remaining.size,reason:'No prefix-connected suffix point with original static scout edge'};break;}
    active.push({...chosen});remaining.delete(key(chosen));choices.push({actorIndex:index,role:fighter?'combat':'movement',home:chosen,precedingObserver:previous,edge:proof??null});
    if(!fighter)previous=chosen;
  }
  const sameSet=active.length===100&&active.every(p=>allKeys.has(key(p)))&&new Set(active.map(key)).size===100;
  const firstTenUnchanged=JSON.stringify(active.slice(0,10))===JSON.stringify(source.anchors.slice(0,10));
  const result={schemaVersion:1,createdAt:new Date().toISOString(),mode,scope:'Suffix coordinate permutation only, not a new geometry search or live acceptance',
    sameCoordinateSet:sameSet,firstTenUnchanged,selectedCount:active.length,failed,choices,
    originalScene:pin(sourceFile),originalTenScene:pin(prefixFile),sourcePins:[...g.sourcePins,pin(import.meta.filename)],accepted:false,capacityAccepted:false};
  if(sameSet&&firstTenUnchanged){
    const output=ROOT+`/candidate-100-${mode}-ordered-v3.json`,tenOutput=ROOT+`/candidate-10-${mode}-ordered-v3.json`;
    const candidate={...source,anchors:active};result.outputs=[writeNew(output,candidate)];fs.writeFileSync(tenOutput,prefixBytes,{flag:'wx'});result.outputs.push(pin(tenOutput));
  }
  result.receipt=ROOT+`/suffix-${mode}-order-v3.json`;
  outputs.push(writeNew(result.receipt,result));
}
console.log(JSON.stringify({outputs,pathSearches:g.pathSearches}));
