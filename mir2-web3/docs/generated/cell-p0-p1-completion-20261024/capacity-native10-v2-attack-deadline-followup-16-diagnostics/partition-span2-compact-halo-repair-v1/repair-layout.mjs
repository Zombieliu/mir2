import fs from 'node:fs';
import { loadGeometry, ROOT as PRIOR, PARAMETERS, STEPS, key, dist, pin } from '../partition-span2-compact-halo-scene-v1/static-lib.mjs';
const ROOT=import.meta.dirname,g=await loadGeometry();
const baselineFile=PRIOR+'/candidate-100-nearest-ordered-final-v3.json',tenFile=PRIOR+'/candidate-10-compact-v2.json',baseline=JSON.parse(fs.readFileSync(baselineFile));
const original=baseline.anchors,tenRaw=fs.readFileSync(tenFile),combat=new Set(PARAMETERS.fighterIndices),changes=[],attempts=[],perActorTrials=new Map();
let homes=original.map(p=>({...p}));
const write=(name,value)=>{const file=ROOT+'/'+name;fs.writeFileSync(file,JSON.stringify(value,null,2)+'\n',{flag:'wx'});return pin(file);};
function scout(active){
  const cluster=g.cluster(active),allowed=p=>g.driver.insideCombatCoverage(cluster,p),occupied=new Set(active.map(key)),indices=active.map((_,i)=>i).filter(i=>i%10!==5),legs=[];
  if(active.length<6)return{count:active.length,passed:true,scope:'No active fighter or scout at this prefix'};
  let from=active[5];
  for(let i=0;i<indices.length*2;i++){
    const actorIndex=indices[i%indices.length],home=active[actorIndex];
    const goals=STEPS.map(([dx,dy])=>({x:home.x+dx,y:home.y+dy})).filter(p=>g.clear(p)&&allowed(p)&&dist(p,from)>0&&!occupied.has(key(p))).sort((a,b)=>dist(a,from)-dist(b,from));
    let accepted;const goalAttempts=[];
    for(const goal of goals.slice(0,4)){
      const options={map:g.map,start:from,target:goal,dynamicObstacles:[...g.npcs,...active.filter((p,n)=>n!==5&&key(p)!==key(from)&&key(p)!==key(goal))],maxExpanded:4000};
      const points=g.nav.findProtocolWalkPath(options),outside=points?.filter(p=>!allowed(p))??[],transfer=points?.filter(p=>g.transferMask[g.index(p)])??[];
      goalAttempts.push({goal,found:Boolean(points),steps:points?points.length-1:null,outsideCoverageCells:outside,transferCells:transfer});
      if(points&&!outside.length&&!transfer.length){accepted={observerIndex:actorIndex,from,goal,steps:points.length-1};break;}
    }
    if(!accepted)return{count:active.length,passed:false,failedObserverIndex:actorIndex,from,home,firstFour:goalAttempts,legs};
    legs.push(accepted);from=accepted.goal;
  }
  const n=indices.length;
  return{count:active.length,passed:true,firstCircuitSteps:legs.slice(0,n).reduce((s,l)=>s+l.steps,0),secondCircuitSteps:legs.slice(n).reduce((s,l)=>s+l.steps,0),legs,
    scope:'Original first4goals/original4000A*/full active14coverage/representative known home occupancy; not live ACK or capacity evidence'};
}
const prefixes=()=>PARAMETERS.prefixes.map(count=>scout(homes.slice(0,count)));
let results=prefixes(),failure=results.find(r=>!r.passed),stopped=null;
while(failure){
  const actorIndex=failure.failedObserverIndex;
  if(actorIndex<10||actorIndex>99||(!changes.some(c=>c.actorIndex===actorIndex)&&changes.length>=4)){
    stopped='Failure lies outside allowed first10/suffix or four changed-actor budget';break;
  }
  if(perActorTrials.has(actorIndex)){stopped='An already-tested actor failed again; its finite candidate budget is not reset';break;}
  const old=original[actorIndex],from=failure.from,used=new Set(homes.flatMap((h,i)=>i===actorIndex?[]:g.body(h).map(key)));
  const legal=[];
  for(let y=old.y-6;y<=old.y+6;y++)for(let x=old.x-6;x<=old.x+6;x++){
    const home={x,y};if(key(home)===key(homes[actorIndex])||g.issues(home,used).length)continue;
    const trial=homes.map((p,i)=>i===actorIndex?home:p);
    try{g.driver.validateNativeCombatClusters(baseline.combatClusters,trial,combat,2);}catch{continue;}
    legal.push(home);
  }
  legal.sort((a,b)=>dist(a,from)-dist(b,from)||dist(a,old)-dist(b,old)||a.y-b.y||a.x-b.x);
  // At most16 lawful points enter original path ranking. No new geometry or
  // expanded A* allowance is introduced by the bounded author.
  const candidates=legal.slice(0,16).map(home=>{
    const trial=homes.map((p,i)=>i===actorIndex?home:p),occupied=new Set(trial.map(key)),cluster=g.cluster(trial),allowed=p=>g.driver.insideCombatCoverage(cluster,p);
    const goals=STEPS.map(([dx,dy])=>({x:home.x+dx,y:home.y+dy})).filter(p=>g.clear(p)&&allowed(p)&&!occupied.has(key(p))&&dist(p,from)>0).sort((a,b)=>dist(a,from)-dist(b,from));
    const paths=goals.slice(0,4).map(goal=>{const points=g.nav.findProtocolWalkPath({map:g.map,start:from,target:goal,dynamicObstacles:[...g.npcs,...trial.filter((p,i)=>i!==5&&key(p)!==key(from)&&key(p)!==key(goal))],maxExpanded:4000});return{goal,found:Boolean(points),steps:points?points.length-1:null};});
    return{home,paths,bestOriginalPathSteps:Math.min(...paths.filter(p=>p.found).map(p=>p.steps))};
  }).sort((a,b)=>a.bestOriginalPathSteps-b.bestOriginalPathSteps||dist(a.home,old)-dist(b.home,old)||a.home.y-b.home.y||a.home.x-b.home.x);
  perActorTrials.set(actorIndex,candidates.length);
  write(`actor-${actorIndex}-candidate-list-v1.json`,{schemaVersion:1,actorIndex,originalHome:old,currentHome:homes[actorIndex],from,
    radius:6,legalAnchorCount:legal.length,candidatesEnteredOriginalPathRanking:candidates.length,candidates,
    ranking:'Preselect at most16 lawful closest Chebyshev candidates; rank these by original A* first4 adjacent-goal path length, then minimum displacement. No path budget increased.',accepted:false,capacityAccepted:false});
  let chosen=null;
  for(let i=0;i<candidates.length;i++){
    const candidate=candidates[i],trial=homes.map((p,n)=>n===actorIndex?candidate.home:p);
    const result=scout(trial.slice(0,failure.count));
    const previousIndex=original.findIndex((p,n)=>n===actorIndex&&key(p)===key(old));
    const localRepaired=result.passed||result.failedObserverIndex!==actorIndex;
    const receipt={schemaVersion:1,createdAt:new Date().toISOString(),actorIndex,candidateNumber:i+1,fromHome:homes[actorIndex],originalHome:old,proposedHome:candidate.home,
      displacementFromOriginal:dist(old,candidate.home),pathRanking:candidate,localFailureRepaired:localRepaired,
      exactFirstTen:true,roleIndicesUnchanged:true,originalFullClusterPassed:true,affectedPrefixResult:result,
      sourceScene:pin(baselineFile),accepted:false,capacityAccepted:false};
    const receiptPin=write(`trial-actor-${actorIndex}-${String(i+1).padStart(2,'0')}-v1.json`,receipt);attempts.push(receiptPin);
    if(localRepaired){chosen=candidate;break;}
  }
  if(!chosen){stopped='All sixteen-or-fewer lawful local candidates failed the original first4/4000 scout leg';break;}
  changes.push({actorIndex,from:homes[actorIndex],originalHome:old,to:chosen.home,displacement:dist(old,chosen.home)});
  homes=homes.map((p,i)=>i===actorIndex?chosen.home:p);results=prefixes();failure=results.find(r=>!r.passed);
}
const changed=homes.map((p,i)=>key(p)===key(original[i])?null:i).filter(i=>i!==null);
if(changed.length>4||changed.some(i=>i<10||dist(homes[i],original[i])>6)||JSON.stringify(homes.slice(0,10))!==JSON.stringify(original.slice(0,10)))throw new Error('Local repair budget/identity violated');
let sceneOutputs=[];
if(!failure){
  sceneOutputs.push(write('candidate-100-repaired-v4.json',{...baseline,anchors:homes}));
  const ten=ROOT+'/candidate-10-repaired-v4.json';fs.writeFileSync(ten,tenRaw,{flag:'wx'});sceneOutputs.push(pin(ten));
}
const result={schemaVersion:1,createdAt:new Date().toISOString(),scope:'Bounded suffix-coordinate repair only; no product/threshold/source change, no live realm or acceptance',
  baselineScene:pin(baselineFile),baselineTenScene:pin(tenFile),parameters:PARAMETERS,limits:{maxChangedActors:4,maxCandidatesPerActor:16,maxDisplacementFromOriginal:6},
  changedActors:changed,changes,attempts,candidateCounts:Object.fromEntries(perActorTrials),representativePrefixScoutResults:results,
  allRepresentativePrefixScoutsPassed:!failure,stopped,sceneOutputs,needsFullIndependentStaticValidator:true,sourcePins:[...g.sourcePins,pin(import.meta.filename)],accepted:false,capacityAccepted:false};
const out=write('repair-result-v1.json',result);console.log(JSON.stringify({out,changes,allRepresentativePrefixScoutsPassed:!failure,stopped,sceneOutputs,
  prefixes:results.map(({legs,...r})=>r)}));
