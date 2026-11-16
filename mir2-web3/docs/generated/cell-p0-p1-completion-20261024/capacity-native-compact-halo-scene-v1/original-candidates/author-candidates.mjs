import fs from 'node:fs';
import { loadGeometry, PARAMETERS, ROOT, BASE, key, dist, pin, writeNew } from './static-lib.mjs';
const g = await loadGeometry();
const inventory = JSON.parse(fs.readFileSync(ROOT + '/candidate-inventory-v1.json'));
const homes = [];
for (let y = PARAMETERS.bounds.minY; y <= PARAMETERS.bounds.maxY; y++) for (let x = PARAMETERS.bounds.minX; x <= PARAMETERS.bounds.maxX; x++) {
  const p = { x, y }; if (g.issues(p).length) continue;
  const targets = g.localTargets(p);
  homes.push({ p, targets, weight: targets.reduce((sum, t) => sum + Math.ceil(t.hp / 5) / Math.max(3, t.delayMinutes), 0) });
}
const legalBootstrap = p => { const proof = g.bootstrap(p); return proof.found && proof.noTransfer && proof.noInitialDanger; };
const mark = (used, p) => g.body(p).forEach(q => used.add(key(q)));
const covers = (a, target) => a.targets.some(t => t.objectId === target.objectId);
const routeAllowed = cluster => p => g.driver.insideCombatCoverage(cluster, p);

// Finite nine-node Held-Karp cycle over actual unchanged4000-node A* edge
// costs. Edges use the first feasible one of the original first four adjacent
// scout goals, with representative other-home occupancy. They are not ACKs.
function orderObservers(observers, fighter) {
  const all = [...observers, fighter], cluster = { observerHomes: observers, patrolSpan: 2 }, allowed = routeAllowed(cluster);
  const goalKeys = new Set(all.map(key)), edges = Array.from({ length: 9 }, () => new Array(9).fill(Infinity));
  function scoutRoute(start, home) {
    const goals = [[0,-1],[1,-1],[1,0],[1,1],[0,1],[-1,1],[-1,0],[-1,-1]].map(([dx,dy]) => ({ x: home.x + dx, y: home.y + dy }))
      .filter(p => dist(p, start) > 0 && g.clear(p) && allowed(p) && !goalKeys.has(key(p))).sort((a,b) => dist(a,start)-dist(b,start));
    for (const goal of goals.slice(0,4)) {
      const points = g.route(start, goal, all, -1, allowed); if (points) return { goal, points };
    }
    return null;
  }
  for (let i=0;i<9;i++) for(let j=0;j<9;j++) if(i!==j) edges[i][j] = scoutRoute(observers[i],observers[j])?.points.length - 1 || Infinity;
  const count=1<<9, dp=Array.from({length:count},()=>new Float64Array(9).fill(Infinity)), previous=Array.from({length:count},()=>new Int16Array(9).fill(-1));
  dp[1][0]=0;
  for(let mask=1;mask<count;mask++) for(let last=0;last<9;last++) if(Number.isFinite(dp[mask][last])) for(let next=1;next<9;next++) {
    if(mask&(1<<next))continue; const nextMask=mask|(1<<next), cost=dp[mask][last]+edges[last][next];
    if(cost<dp[nextMask][next]){dp[nextMask][next]=cost;previous[nextMask][next]=last;}
  }
  let last=-1,total=Infinity;for(let i=1;i<9;i++){const cost=dp[count-1][i]+edges[i][0];if(cost<total){total=cost;last=i;}}
  if(last<0)return {passed:false,reason:'no finite nine-observer scout cycle'};
  const reversed=[];let mask=count-1;while(last!==0){reversed.push(last);const before=previous[mask][last];mask^=1<<last;last=before;}reversed.push(0);const order=reversed.reverse();
  // A rotation preserves the cycle but begins closest to the fighter.
  let rotation=0,best=Infinity;for(let i=0;i<9;i++){const r=scoutRoute(fighter,observers[order[i]]);const steps=r?.points.length-1??Infinity;if(steps<best){best=steps;rotation=i;}}
  const ordered=[...order.slice(rotation),...order.slice(0,rotation)].map(i=>observers[i]);
  const legs=[];let current=fighter;
  for(let i=0;i<18;i++){const home=ordered[i%9],r=scoutRoute(current,home);if(!r)return{passed:false,reason:'ordered representative scout traversal failed',legs};legs.push({homeIndex:i%9,from:current,goal:r.goal,steps:r.points.length-1,path:r.points});current=r.goal;}
  return {passed:true,observerHomes:ordered,optimizedCycleEdgeSteps:total,firstCircuitSteps:legs.slice(0,9).reduce((s,l)=>s+l.steps,0),
    secondCircuitSteps:legs.slice(9).reduce((s,l)=>s+l.steps,0),legs,scope:'Static representative-home occupancy and actual original A* paths. Runtime current AOI, moving obstacles, priming and receipt budgets are not simulated.'};
}

const seeds = [...inventory.topSpatiallySeparatedHomes.slice(0,8).map(x=>x.home), {x:260,y:670}, {x:277,y:666}];
const drafts=[];
for (let seedIndex=0;seedIndex<seeds.length;seedIndex++) {
  const center=seeds[seedIndex], used=new Set(), selected=[], covered=new Set();
  const fighter=homes.filter(h=>dist(h.p,center)<=4).sort((a,b)=>dist(a.p,center)-dist(b.p,center)||b.weight-a.weight).find(h=>legalBootstrap(h.p))?.p;
  if(!fighter){drafts.push({seedIndex,center,passed:false,reason:'No lawful bootstrap fighter near seed'});continue;}
  mark(used,fighter);
  for(let i=0;i<9;i++){
    const ranked=homes.filter(h=>dist(h.p,center)<=18&&!g.issues(h.p,used).length).map(h=>{
      const gain=h.targets.filter(t=>!covered.has(t.objectId)).reduce((s,t)=>s+Math.ceil(t.hp/5)/Math.max(3,t.delayMinutes),0);
      const travel=selected.length?Math.min(...selected.map(q=>dist(q,h.p))):dist(h.p,fighter);
      return {...h,gain,score:gain*20+h.weight*.2-travel*.18-dist(h.p,center)*.04};
    }).sort((a,b)=>b.score-a.score||a.p.y-b.p.y||a.p.x-b.p.x);
    const chosen=ranked.find(h=>legalBootstrap(h.p));if(!chosen)break;
    selected.push(chosen.p);mark(used,chosen.p);chosen.targets.forEach(t=>covered.add(t.objectId));
  }
  if(selected.length!==9){drafts.push({seedIndex,center,passed:false,reason:'Fewer than nine legal observer homes',selected});continue;}
  const tour=orderObservers(selected,fighter);
  const points=tour.passed?tour.observerHomes:selected;
  const anchors=[...points.slice(0,5),fighter,...points.slice(5)];
  const targets=g.targets.filter(t=>points.some(h=>g.witnessGoals(h,t).length));
  const ids=new Set(targets.map(t=>t.objectId)), known=g.targets.filter(t=>ids.has(t.objectId));
  const cluster=g.cluster(anchors), connected=g.component(anchors[0],routeAllowed(cluster),anchors.slice(1));
  const isolated=anchors.map((h,i)=>i).filter(i=>i!==0&&![[0,-1],[1,0],[0,1],[-1,0]].some(([dx,dy])=>connected.has({x:anchors[i].x+dx,y:anchors[i].y+dy})));
  let first2;try{g.driver.validateNativeExplicitHomes(g.map,PARAMETERS.origin,2,anchors.slice(0,2),2,g.transfers,g.npcs);first2=true;}catch{first2=false;}
  const passed=tour.passed&&isolated.length===0&&first2&&g.driver.insideCombatCoverage(cluster,fighter);
  const score=known.reduce((s,t)=>s+Math.ceil(t.hp/5)/Math.max(3,t.delayMinutes),0)*10-(tour.secondCircuitSteps??1000)*.045;
  const draft={seedIndex,center,passed,score,fighter,anchors,canonicalAllNineStableTargets:targets.length,
    counts:Object.fromEntries(PARAMETERS.monsterNames.map(n=>[n,targets.filter(t=>t.name===n).length])),targetIds:targets.map(t=>t.objectId),isolated,first2,tour};
  writeNew(ROOT+`/draft-${String(seedIndex+1).padStart(2,'0')}.json`,{schemaVersion:1,scope:'Unadopted ten-home static draft, not a capacity result',accepted:false,...draft});drafts.push(draft);
}
const ranked=drafts.filter(d=>d.passed).sort((a,b)=>b.score-a.score||a.seedIndex-b.seedIndex);
if(!ranked.length)throw new Error('No statically connected compact ten-home draft; all failures retained');
const chosen=ranked[0],anchors=chosen.anchors.map(p=>({...p})),used=new Set();anchors.forEach(p=>mark(used,p));
const selections=anchors.map((home,actorIndex)=>({actorIndex,home,role:actorIndex%10===5?'combat':'movement',source:'selected-first-ten-draft',bootstrap:g.bootstrap(home)}));
const rejected=new Map();
for(let actorIndex=10;actorIndex<100;actorIndex++){
  const cluster=g.cluster(anchors),allowed=routeAllowed(cluster),connected=g.component(anchors[0],allowed,anchors.slice(1));
  const covered=new Set(g.targets.filter(t=>cluster.observerHomes.some(h=>g.witnessGoals(h,t).length)).map(t=>t.objectId));
  const fighter=actorIndex%10===5;
  const ranked=homes.filter(h=>!rejected.has(key(h.p))&&!g.issues(h.p,used).length&&connected.has(h.p)).map(h=>{
    const gain=h.targets.filter(t=>!covered.has(t.objectId)).reduce((s,t)=>s+Math.ceil(t.hp/5)/Math.max(3,t.delayMinutes),0);
    const closest=Math.min(...anchors.map(p=>dist(p,h.p)));
    return {...h,gain,score:fighter?h.weight*2-Math.min(...cluster.observerHomes.map(p=>dist(p,h.p)))*.15:gain*25+h.weight*.15-closest*.025};
  }).sort((a,b)=>b.score-a.score||dist(a.p,PARAMETERS.origin)-dist(b.p,PARAMETERS.origin)||a.p.y-b.p.y||a.p.x-b.p.x);
  let selected;
  for(const h of ranked){if(!legalBootstrap(h.p)){rejected.set(key(h.p),g.bootstrap(h.p));continue;}selected=h;break;}
  if(!selected)break;
  anchors.push(selected.p);mark(used,selected.p);selections.push({actorIndex,home:selected.p,role:fighter?'combat':'movement',newAllNineStableTargetWeight:selected.gain,bootstrap:g.bootstrap(selected.p)});
}
const definitions=count=>[{combatActorIndices:Array.from({length:count},(_,i)=>i).filter(i=>i%10===5),movementActorIndices:Array.from({length:count},(_,i)=>i).filter(i=>i%10!==5)}];
const candidate=count=>({schemaVersion:1,mapFileName:'0',patrolSpan:2,monsterNames:PARAMETERS.monsterNames,anchors:anchors.slice(0,count),combatClusters:definitions(count)});
const outputs=[];
if(anchors.length===100){outputs.push(writeNew(ROOT+'/candidate-100-v1.json',candidate(100)));outputs.push(writeNew(ROOT+'/candidate-10-v1.json',candidate(10)));}
const result=writeNew(ROOT+'/author-result-v1.json',{schemaVersion:1,createdAt:new Date().toISOString(),scope:'External static scene author only; no product source, gates or live realm changed',parameters:PARAMETERS,
  selectedCount:anchors.length,selectedDraft:chosen.seedIndex+1,selectedFirstTen:chosen.anchors,selectionScoreRule:'first10: fixedDamage5 all-nine-position stable target supply*10 minus actual4000-A* scout second-circuit steps*.045; outside first10: marginal all-nine stable target supply, reachable expansion in the same active cluster',
  draftSummaries:drafts.map(({tour,...d})=>({...d,tour:tour?{passed:tour.passed,firstCircuitSteps:tour.firstCircuitSteps,secondCircuitSteps:tour.secondCircuitSteps}:null,adoptedAsLiveScene:false})),
  failedBootstrapCandidates:[...rejected].map(([home,proof])=>({home,proof})),selections,outputs,
  sourcePins:[...g.sourcePins,pin(ROOT+'/candidate-inventory-v1.json'),pin(import.meta.filename)],pathSearches:g.pathSearches,capacityAccepted:false,accepted:false,
  limitations:['Selection optimizes pinned initial canonical geometry. Monsters can move, die and respawn at future positions; actual snapshot membership and active positions are mandatory in unchanged runtime.',
    'A* scout routes use representative home occupancy and fixed initial terrain. No Run fraction, latency,95%completion or survival guarantee follows.',
    'The second-circuit score is not a live movement count. Each actual runtime decision still has its original first-four goal/4000-node budget.',
    'Scene construction is separate from independent validation. Neither the selected draft nor100homes are authorized for real load by this author result.']});
console.log(JSON.stringify({result,outputs,chosenDraft:chosen.seedIndex+1,selectedCount:anchors.length,drafts:drafts.map(d=>({seed:d.seedIndex+1,center:d.center,passed:d.passed,targets:d.canonicalAllNineStableTargets,tour:d.tour?.secondCircuitSteps,score:d.score})),pathSearches:g.pathSearches}));
