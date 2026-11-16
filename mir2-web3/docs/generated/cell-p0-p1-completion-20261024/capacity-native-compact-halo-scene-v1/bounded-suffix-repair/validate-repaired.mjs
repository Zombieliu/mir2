import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { loadGeometry, PARAMETERS, BASE, PROJECT, STEPS, key, dist, pin, fixedDamageSupply, geometrySelfChecks } from '../partition-span2-compact-halo-scene-v1/static-lib.mjs';
const ROOT=import.meta.dirname;
const writeNew=(file,value)=>{if(path.dirname(path.resolve(file))!==path.resolve(ROOT))throw new Error('Output must be an immediate child of dedicated repairdirectory');fs.writeFileSync(file,JSON.stringify(value,null,2)+'\n',{flag:'wx'});return pin(file);};

// Read-only map/source inputs; only a NEW dedicated external result is written.
const name=process.argv[2]??'candidate-100-repaired-v4.json', output=process.argv[3]??'validation-repaired-v4.json';
for(const value of[name,output])if(!/^[A-Za-z0-9_-]+\.json$/.test(value))throw new Error('Only immediate-child JSON basenames are allowed');
const file=ROOT+'/'+name, g=await loadGeometry(), before=g.sourcePins, scenario=JSON.parse(fs.readFileSync(file));
const assert=(test,message)=>{if(!test)throw new Error(message);};
assert(scenario.schemaVersion===1&&scenario.mapFileName==='0'&&scenario.patrolSpan===2,'Unexpected scene identity/span');
assert(JSON.stringify(scenario.monsterNames)===JSON.stringify(PARAMETERS.monsterNames),'Unexpected target whitelist');
assert(Array.isArray(scenario.anchors)&&scenario.anchors.length===100,'Full candidate must have exactly100 anchors');
const homes=scenario.anchors, combat=new Set(PARAMETERS.fighterIndices), used=new Set(), actorProofs=[];
for(let i=0;i<homes.length;i++){
  const home=homes[i],issues=g.issues(home,used);assert(!issues.length,`Actor${i} unsafe: ${issues.join(',')}`);
  g.body(home).forEach(p=>used.add(key(p)));const bootstrap=g.bootstrap(home);
  assert(bootstrap.found&&bootstrap.noTransfer&&bootstrap.noInitialDanger,`Actor${i} unsafe/unreachable bootstrap`);
  actorProofs.push({actorIndex:i,home,bodyCells:9,haloChecks:25,bootstrap});
}
const whole=g.driver.validateNativeExplicitHomes(g.map,PARAMETERS.origin,100,homes,2,g.transfers,g.npcs);
g.driver.validateNativeCombatClusters(scenario.combatClusters,homes,combat,2);
const allHalo=new Set(homes.flatMap(h=>g.halo(h).map(key)));

function scoutCircuit(active, fighterIndex) {
  const cluster=g.cluster(active),allowed=p=>g.driver.insideCombatCoverage(cluster,p),occupied=new Set(active.map(key));
  const observerIndices=active.map((_,i)=>i).filter(i=>i%10!==5),legs=[];let current=active[fighterIndex];
  for(let i=0;i<observerIndices.length*2;i++){
    const observerIndex=observerIndices[i%observerIndices.length],home=active[observerIndex];
    const goals=STEPS.map(([dx,dy])=>({x:home.x+dx,y:home.y+dy})).filter(p=>g.clear(p)&&allowed(p)&&dist(p,current)>0&&!occupied.has(key(p))).sort((a,b)=>dist(a,current)-dist(b,current));
    let result;
    for(const goal of goals.slice(0,4)){const points=g.route(current,goal,active,fighterIndex,allowed);if(points){result={observerIndex,from:current,goal,steps:points.length-1,path:points};break;}}
    if(!result)return{passed:false,failedObserverIndex:observerIndex,legs};
    legs.push(result);current=result.goal;
  }
  const n=observerIndices.length;
  return{passed:true,observerIndices,firstCircuitSteps:legs.slice(0,n).reduce((s,l)=>s+l.steps,0),secondCircuitSteps:legs.slice(n).reduce((s,l)=>s+l.steps,0),legs,
    scope:'Representative known home occupancy; original first-four goal ordering and original4000-node A*. Paths include full cells/diagonal checks via original navigation. No live Run/ACK/clock/occupancy guarantee.'};
}

const stages=[];
for(const count of PARAMETERS.prefixes){
  const active=homes.slice(0,count),cluster=g.cluster(active),observerIndices=active.map((_,i)=>i).filter(i=>i%10!==5),fighterIndices=active.map((_,i)=>i).filter(i=>i%10===5);
  const allowed=p=>g.driver.insideCombatCoverage(cluster,p),connected=g.component(active[0],allowed,active.slice(1));
  const disconnected=active.map((_,i)=>i).filter(i=>i!==0&&!STEPS.some(([dx,dy])=>connected.has({x:active[i].x+dx,y:active[i].y+dy})));
  assert(disconnected.length===0,`Prefix${count} selected homes disconnected inside active coverage: ${disconnected}`);
  const witnesses=g.targets.filter(t=>allowed(t)).map(target=>{
    const eligible=observerIndices.flatMap(actorIndex=>{
      const goals=g.witnessGoals(active[actorIndex],target).filter(p=>allowed(p)&&!active.some((q,i)=>i!==actorIndex&&key(q)===key(p)));
      return goals.length?[{actorIndex,allNinePositionsChecked:9,goals}]:[];
    });
    return{target,eligible};
  });
  const possibleIds=new Set(g.targets.filter(t=>allowed(t)&&observerIndices.some(i=>g.witnessGoals(active[i],t,'some').length)).map(t=>t.objectId));
  const stable=witnesses.filter(w=>w.eligible.length),pathIds=new Set(),perFighter=[];
  for(const actorIndex of fighterIndices){
    const home=active[actorIndex],occupied=active.filter((_,i)=>i!==actorIndex),component=g.component(home,allowed,occupied),paths=[];
    for(const {target,eligible} of stable){
      const goals=[...new Map(eligible.flatMap(w=>w.goals.map(goal=>[key(goal),{goal,witnessActorIndex:w.actorIndex}]))).values()].filter(({goal})=>component.has(goal)).sort((a,b)=>dist(a.goal,home)-dist(b.goal,home));
      let accepted;
      for(const {goal,witnessActorIndex} of goals){
        const route=g.route(home,goal,active,actorIndex,allowed);
        if(route){accepted={objectId:target.objectId,name:target.name,target:{x:target.x,y:target.y},goal,witnessActorIndex,steps:route.length-1,path:route};break;}
      }
      if(accepted){paths.push(accepted);pathIds.add(target.objectId);}
    }
    const reachableTargets=g.targets.filter(t=>paths.some(p=>p.objectId===t.objectId));
    perFighter.push({actorIndex,home,coverageConnectedCells:component.count,allNineStableExact4000TargetPaths:paths.length,
      conditionalPositiveHitsPerMinuteByFixedDamage:fixedDamageSupply(reachableTargets),paths});
  }
  const exact=g.targets.filter(t=>pathIds.has(t.objectId));
  const counts=list=>Object.fromEntries(PARAMETERS.monsterNames.map(n=>[n,list.filter(t=>t.name===n).length]));
  const ruleSupply=[...new Set(exact.map(t=>t.ruleIndex))].map(ruleIndex=>{
    const ts=exact.filter(t=>t.ruleIndex===ruleIndex),rule=g.metadata.respawns.find(r=>r.respawn_index===ruleIndex);
    return{ruleIndex,name:rule.monster_name,hp:rule.monster_hp,delayMinutes:rule.delay_minutes,randomDelayMinutes:rule.random_delay_minutes,
      corpseTtlSeconds:180,conditionalEarliestRefreshMinutes:Math.max(3,rule.delay_minutes),deduplicatedStableSlots:ts.length,objectIds:ts.map(t=>t.objectId)};
  });
  stages.push({count,activeObserverIndices:observerIndices,fighterIndices,coverageConnectedCells:connected.count,disconnectedSelectedHomes:disconnected,
    canonicalTargetsInOriginal14Coverage:g.targets.filter(t=>allowed(t)).length,potentialSomeBodyPositionWitnessTargets:possibleIds.size,
    allNinePositionStableGeometricTargets:stable.length,stableGeometricCounts:counts(stable.map(w=>w.target)),
    uniqueAllNineStableExact4000Targets:exact.length,uniqueCounts:counts(exact),ruleSupply,perFighter,
    conditionalPositiveHitsPerMinuteByFixedDamage:fixedDamageSupply(exact),requiredPerMinute:{attacks:18*fighterIndices.length,positiveHits:12*fighterIndices.length},
    initialFiniteHp:exact.reduce((s,t)=>s+t.hp,0),initialPositiveHitsByFixedDamage:Object.fromEntries([1,3,5,10,15,25].map(d=>[d,exact.reduce((s,t)=>s+Math.ceil(t.hp/d),0)])),
    witnesses:stable.map(({target,eligible})=>({objectId:target.objectId,name:target.name,position:{x:target.x,y:target.y},eligible})),
    scoutTour:count===10?scoutCircuit(active,5):null,
    constraintsPass:true,liveSupplyProven:false,capacityAccepted:false});
}
const prefixFile=ROOT+'/'+name.replace('100','10');
assert(fs.existsSync(prefixFile),'Matching explicit10prefix file missing');
const prefix=JSON.parse(fs.readFileSync(prefixFile));assert(prefix.patrolSpan===2&&JSON.stringify(prefix.anchors)===JSON.stringify(homes.slice(0,10)),'Ten-prefix differs from full candidate');
g.driver.validateNativeExplicitHomes(g.map,PARAMETERS.origin,10,prefix.anchors,2,g.transfers,g.npcs);
g.driver.validateNativeCombatClusters(prefix.combatClusters,prefix.anchors,new Set([5]),2);
const after=before.map(p=>pin(p.path));assert(JSON.stringify(before)===JSON.stringify(after),'Source/data changed during static validation');
const out=writeNew(ROOT+'/'+output,{schemaVersion:1,createdAt:new Date().toISOString(),scope:'Independent read-only recomputation of external scene static constraints and canonical supply geometry. No service, client, gameplay or gate execution.',
  sourceCommit:execFileSync('git',['rev-parse','HEAD'],{cwd:PROJECT,encoding:'utf8',windowsHide:true}).trim(),parameters:PARAMETERS,geometrySelfChecks:geometrySelfChecks(),
  staticSafetyPassed:true,scenePins:[pin(file),pin(prefixFile)],sourcePins:[...before,pin(import.meta.filename)],sourcePinsUnchangedAfterRun:true,
  staticChecks:{bodyCellsChecked:used.size,haloChecks:homes.length*25,uniqueHaloCells:allHalo.size,wholeMapBfsVisited:whole.visitedCells,mapCells:whole.mapCells,
    prefixEquality:true,allDeclaredRoles:true,ordinaryWhitelist:true,canonicalInitialMonsters:g.allInitial.length,canonicalWeakTargets:g.targets.length,initialDangerEntities:g.danger.length},
  actorProofs,stages,pathSearches:g.pathSearches,liveSupplyProven:false,accepted:false,capacityAccepted:false,
  limitations:[
    'All-nine witness geometry is conservative over nine legal observer body positions, using one observer and one statically clear prospective attack tile. It does not establish actual snapshot membership, active/living status or simultaneous runtime positions; unchanged sender rechecks remain required.',
    'Fighter target paths use representative active-home occupancy, not all9^N actor configurations. The full path is checked within prefix-active14coverage and excludes transfers, but dynamic monster/player occupancy can change.',
    'The canonical tick0 mirror follows pinned Rust placement, not a snapshot of the future run. Weak monsters wander; initial unsafe-monster rectangles are not future aggro bounds.',
    'Fixed-damage ceil(HP/damage)/max(180s,delay) is an optimistic conditional positive-hit supply estimate; no misses, armour, travel, corpse/respawn delay overhead or contested allocation is charged. Aggregate target IDs are deduplicated; per-fighter lists overlap and must not be summed.',
    'No rate proves18attempts/min or12positivehits/min in every actual60s window. Level/DPS growth, finite initial HP, finite held healing and survival remain live concerns.',
    'Original4targets×8adjacent×4000per-path budgets,40moves/30s/3visits pursuit limits, stable6 actualsend checks,900ms dual-end receipts,95%offeredcompletion,750ms opportunities,50%Run and80cell rules are unchanged.',
    'No old06/07/08/09/10/11 result transfers to this new scenario. Fresh root-approved probe then formal evidence is necessary; P0 remains pending.'
  ]});
console.log(JSON.stringify({out,staticSafetyPassed:true,stages:stages.map(s=>({count:s.count,targets:s.uniqueAllNineStableExact4000Targets,counts:s.uniqueCounts,supply:s.conditionalPositiveHitsPerMinuteByFixedDamage,
  minimumPerFighterTargets:s.perFighter.length?Math.min(...s.perFighter.map(f=>f.allNineStableExact4000TargetPaths)):0,tour:s.scoutTour&&{passed:s.scoutTour.passed,first:s.scoutTour.firstCircuitSteps,second:s.scoutTour.secondCircuitSteps}})),pathSearches:g.pathSearches}));
