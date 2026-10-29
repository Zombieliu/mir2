import fs from 'node:fs';
import { pathToFileURL } from 'node:url';
import { loadGeometry, ROOT as PRIOR, STEPS, key, dist, pin } from '../partition-span2-compact-halo-scene-v1/static-lib.mjs';
const ROOT=import.meta.dirname,g=await loadGeometry(),file=ROOT+'/candidate-100-repaired-v4.json',homes=JSON.parse(fs.readFileSync(file)).anchors;
const old=JSON.parse(fs.readFileSync(PRIOR+'/candidate-100-nearest-ordered-final-v3.json')).anchors;
const result=JSON.parse(fs.readFileSync(ROOT+'/repair-result-v1.json')).representativePrefixScoutResults.find(r=>r.count===100);
const leg=result.legs.find(l=>l.observerIndex===92),from=leg.from,home=homes[92],cluster=g.cluster(homes),oldCluster=g.cluster(old),allowed=p=>g.driver.insideCombatCoverage(cluster,p);
const counted=await import(pathToFileURL(ROOT+'/protocol-navigation-counted.mjs')),occupied=new Set(homes.map(key));
const goals=STEPS.map(([dx,dy])=>({x:home.x+dx,y:home.y+dy})).filter(p=>g.clear(p)&&allowed(p)&&!occupied.has(key(p))&&dist(p,from)>0).sort((a,b)=>dist(a,from)-dist(b,from));
const attempts=[];
for(const goal of goals.slice(0,4)){
  const options={map:g.map,start:from,target:goal,dynamicObstacles:[...g.npcs,...homes.filter((p,i)=>i!==5&&key(p)!==key(from)&&key(p)!==key(goal))],maxExpanded:4000};
  const normal=g.nav.findProtocolWalkPath(options),points=counted.findProtocolWalkPath(options);
  if(JSON.stringify(points)!==JSON.stringify(normal))throw new Error('Count-only adapter changed originalpath');
  const outsideNew=points?.filter(p=>!allowed(p))??[],outsideOld=points?.filter(p=>!g.driver.insideCombatCoverage(oldCluster,p))??[];
  attempts.push({goal,diagnostic:{...counted.latestWalkDiagnostic},steps:points?points.length-1:null,found:Boolean(points),outsideNewCoverageCells:outsideNew,
    outsideOldCoverageCells:outsideOld,path:points});
}
const chosen=attempts.find(a=>a.found&&!a.outsideNewCoverageCells.length);
if(!chosen||key(chosen.goal)!==key(leg.goal))throw new Error('Recomputed first-success goal differs from original scout result');
const out=ROOT+'/repaired-leg-proof-v4.json';fs.writeFileSync(out,JSON.stringify({schemaVersion:1,createdAt:new Date().toISOString(),scope:'Same originalfirst4/4000path and all returned cells; local diagnostic only',
  scene:pin(file),oldScene:pin(PRIOR+'/candidate-100-nearest-ordered-final-v3.json'),from,home,actualFirstSuccessGoal:leg.goal,attempts,
  importantBoundary:'A moved home changes both coverage and the exact shortest-route destination. Do not claim every originally uncovered cell became covered; this proof checks the actual new returned path.',
  sourcePins:[...g.sourcePins,pin(ROOT+'/protocol-navigation-counted.mjs'),pin(import.meta.filename)],accepted:false,capacityAccepted:false},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({out:pin(out),from,home,firstSuccess:chosen.goal,expanded:chosen.diagnostic.expanded,steps:chosen.steps,outsideNew:chosen.outsideNewCoverageCells.length,outsideOld:chosen.outsideOldCoverageCells.length}));
