import fs from 'node:fs';
import { loadGeometry, PARAMETERS, ROOT, STEPS, key, dist, pin, writeNew } from './static-lib.mjs';
const file=ROOT+'/candidate-100-compact-v2.json',scene=JSON.parse(fs.readFileSync(file)),g=await loadGeometry(),stages=[];
for(const count of [10,25,50,100]){
  const homes=scene.anchors.slice(0,count),observers=homes.map((_,i)=>i).filter(i=>i%10!==5),cluster=g.cluster(homes),allowed=p=>g.driver.insideCombatCoverage(cluster,p),occupied=new Set(homes.map(key));
  let current=homes[5];const legs=[];let failed=null;
  for(let i=0;i<observers.length*2;i++){
    const actorIndex=observers[i%observers.length],h=homes[actorIndex];
    const goals=STEPS.map(([dx,dy])=>({x:h.x+dx,y:h.y+dy})).filter(p=>g.clear(p)&&allowed(p)&&!occupied.has(key(p))&&dist(p,current)>0).sort((a,b)=>dist(a,current)-dist(b,current));
    let chosen;for(const goal of goals.slice(0,4)){const route=g.route(current,goal,homes,5,allowed);if(route){chosen={observerIndex:actorIndex,from:current,goal,steps:route.length-1};break;}}
    if(!chosen){failed=actorIndex;break;}legs.push(chosen);current=chosen.goal;
  }
  const second=legs.length===observers.length*2?legs.slice(observers.length).reduce((s,l)=>s+l.steps,0):null;
  stages.push({count,scoutActorIndex:5,observerCount:observers.length,passed:failed===null,failedObserverIndex:failed,secondCircuitUnitWalkSteps:second,
    emptyPathIdealTwoCellRunOpportunityLowerBound:second===null?null:Math.ceil(second/2),allWalkOpportunityUpperIllustration:second,
    emptyPathIdealRunTravelSecondsLowerBound:second===null?null:Math.ceil(second/2)*.75,legs});
}
const result=writeNew(ROOT+'/prefix-scout-cost-compact-final-v2.json',{schemaVersion:1,createdAt:new Date().toISOString(),scenePin:pin(file),sourcePins:[...g.sourcePins,pin(import.meta.filename)],stages,
  scope:'Original first-four scout goal selection, original4000A*, representative known home occupancy. Not live movement, budgets or Run evidence.',
  limitations:['Other fighters may begin elsewhere and chase between waypoints; this actor5 tour is not their actual tour.',
    'Two-cell-Run travel bound ignores priming, turns, chat, collisions, replan overhead and waypoint24ACK/20s advance. It is only an ideal lower bound.',
    'All observers share one declared cluster. Large prefix scout tours may cause long attack gaps despite static aggregate target supply; no additional cluster or driver change is made.'],accepted:false,capacityAccepted:false});
console.log(JSON.stringify({result,stages:stages.map(({legs,...s})=>s)}));
