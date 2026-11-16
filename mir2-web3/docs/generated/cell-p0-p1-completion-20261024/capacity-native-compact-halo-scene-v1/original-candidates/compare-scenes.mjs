import fs from 'node:fs';
import { loadGeometry, PARAMETERS, ROOT, BASE, PROJECT, STEPS, key, dist, pin, writeNew, fixedDamageSupply } from './static-lib.mjs';
const g=await loadGeometry();
const topic=PROJECT+'/docs/generated/cell-p0-p1-completion-20261024';
const inputs=[
  {name:'historical-span2-v3',file:BASE+'/partition-ordinary-two-name-safe-home-100-candidate-v3.json'},
  {name:'probe09-10-11-span4-halo',file:BASE+'/partition-ordinary-two-name-span4-halo-100-v1.json'},
  {name:'new-supply-draft04',file:ROOT+'/candidate-100-v1.json'},
  {name:'new-compact-draft02',file:ROOT+'/candidate-100-compact-v2.json'},
];
const geometry=[];
for(const input of inputs){
  const scene=JSON.parse(fs.readFileSync(input.file)),active=scene.anchors.slice(0,10),span=scene.patrolSpan;
  const observers=active.map((_,i)=>i).filter(i=>i!==5),body=h=>Array.from({length:(span+1)**2},(_,i)=>({x:h.x+i%(span+1),y:h.y+Math.floor(i/(span+1))}));
  const cluster={observerHomes:observers.map(i=>active[i]),patrolSpan:span},allowed=p=>g.driver.insideCombatCoverage(cluster,p);
  const targets=g.targets.filter(t=>allowed(t)&&observers.some(i=>g.staticGoals.get(t.objectId).some(goal=>allowed(goal)&&body(active[i]).every(p=>dist(p,t)<=6&&dist(p,goal)<=6))));
  const homeKeys=new Set(active.map(key)),legs=[];let current=active[5],failed=null;
  for(let i=0;i<18;i++){
    const actorIndex=observers[i%9],home=active[actorIndex];
    const goals=STEPS.map(([dx,dy])=>({x:home.x+dx,y:home.y+dy})).filter(p=>dist(p,current)>0&&g.clear(p)&&allowed(p)&&!homeKeys.has(key(p))).sort((a,b)=>dist(a,current)-dist(b,current));
    let accepted;for(const goal of goals.slice(0,4)){const points=g.route(current,goal,active,5,allowed);if(points){accepted={observerIndex:actorIndex,from:current,goal,steps:points.length-1};break;}}
    if(!accepted){failed=actorIndex;break;}legs.push(accepted);current=accepted.goal;
  }
  const haloIssues=active.map((h,i)=>({actorIndex:i,blockedHaloCells:Array.from({length:(span+3)**2},(_,n)=>({x:h.x-1+n%(span+3),y:h.y-1+Math.floor(n/(span+3))})).filter(p=>!g.clear(p))})).filter(x=>x.blockedHaloCells.length);
  geometry.push({name:input.name,scenePin:pin(input.file),span,firstTen:active,stableWitnessRange:6,positionsPerObserver:(span+1)**2,
    canonicalAllBodyPositionStableTargets:targets.length,targetCounts:Object.fromEntries(PARAMETERS.monsterNames.map(n=>[n,targets.filter(t=>t.name===n).length])),
    targetIds:targets.map(t=>t.objectId),conditionalSupplyByFixedDamage:fixedDamageSupply(targets),staticHaloFailures:haloIssues,
    originalAstarRepresentativeScout:{passed:failed===null,failedObserverIndex:failed,firstCircuitSteps:legs.length>=9?legs.slice(0,9).reduce((s,l)=>s+l.steps,0):null,
      secondCircuitSteps:legs.length===18?legs.slice(9).reduce((s,l)=>s+l.steps,0):null,legs},
    scope:'Fresh canonical geometry comparison only; historical report live spawn/actor states and old16range attack-witness rule differ. Representative scout route is not a live action/ACK count.'});
}
const reportInputs=[
  ['06','capacity-native10-v2-coordinated-diagnostics/release-native-10-v2-coordinated-feasibility-06/reports/1792870589933-af9ef086.report.json'],
  ['07','capacity-native10-v2-patrol-diagnostics/release-native-10-v2-patrol-feasibility-07/reports/1792878747018-da931241.report.json'],
  ['08','capacity-native10-v2-socket-diagnostics/release-native-10-v2-socket-feasibility-08/reports/1792885245847-0368d2eb.report.json'],
  ['10','capacity-native10-v2-halo-followup-10-diagnostics/release-native-10-v2-halo-feasibility-10/reports/1792898022018-efb7a76a.report.json'],
  ['11','capacity-native10-v2-halo-followup-11-diagnostics/release-native-10-v2-halo-feasibility-11/reports/1792902924884-3b7017e1.report.json'],
];
const historicalReports=reportInputs.map(([probe,relative])=>{
  const file=topic+'/'+relative,r=JSON.parse(fs.readFileSync(file));
  return{probe,pin:pin(file),runId:r.runId,ok:r.ok,highestPassedStage:r.highestPassedStage,
    attackWitnessRule:r.nativeAttackWitness??'No new stable6 rule declared in this historical report; do not infer it',
    stages:r.stages.filter(s=>s.controlledActors===10).map(s=>({name:s.name,completed:s.completed,passed:s.passed,
      requestedSeconds:s.seconds,actualSeconds:(s.endedAtMs-s.startedAtMs)/1000,
      fighters:s.activity.filter(a=>a.role==='combat').map(a=>({actor:a.actor,attacks:a.attacks,hits:a.hits,moves:a.moves,
        nativeOpportunities:a.nativeOpportunities,nativeCompleted:a.nativeCompleted,actualHitsPerMinute:a.hits*60000/(s.endedAtMs-s.startedAtMs)})),
      windows:(s.rollingWindows??[]).map(w=>({seconds:w.seconds,passed:w.passed,fighters:(w.activity??[]).filter(a=>a.role==='combat').map(a=>({attacks:a.attacks,hits:a.hits,moves:a.moves,nativeOpportunities:a.nativeOpportunities,nativeCompleted:a.nativeCompleted}))}))})),
    inheritedAcceptance:false};
});
const summaryFile=BASE+'/probe11-combat-analysis-v1/summary-v1.json';
const result=writeNew(ROOT+'/scene-comparison-v1.json',{schemaVersion:1,createdAt:new Date().toISOString(),geometry,historicalReports,
  probe11AnalysisPin:pin(summaryFile),probe11SelectedFacts:{source:'Pinned independent capacity worker summary; no new raw/video scan by this author',lastFiveHits:[32,34,25,15,11],
    longestAttackGapMs:49392,longestGapSuccessfulMoves:62,scoutMoves:53,chaseMoves:9,routeLoopDeferrals:0},
  limitations:['Old06/07/08 used the original span2 geometry but also older driver contracts; some60s windows were shorter than60s. Their31hit/min observations cannot be inherited by this scene/current strict gates.',
    'No comparison controls fresh spawn phase, live wandering, account growth, snapshot membership or scene age. It cannot attribute a report improvement to one source/geometry change.',
    'All scene target tables here use the SAME current canonical initial data and all legal body positions, not the actual historical target coordinates.',
    'Scouting lengths retain fixed actor-index order and actual unchanged4000A*, with other homes as representative obstacles. Live moving occupancy and24ACK/20s waypoint advances can alter routes.'],
  sourcePins:[...g.sourcePins,pin(import.meta.filename)],accepted:false,capacityAccepted:false});
console.log(JSON.stringify({result,geometry:geometry.map(x=>({name:x.name,targets:x.canonicalAllBodyPositionStableTargets,counts:x.targetCounts,haloFailures:x.staticHaloFailures.length,tour:x.originalAstarRepresentativeScout.secondCircuitSteps}))}));
