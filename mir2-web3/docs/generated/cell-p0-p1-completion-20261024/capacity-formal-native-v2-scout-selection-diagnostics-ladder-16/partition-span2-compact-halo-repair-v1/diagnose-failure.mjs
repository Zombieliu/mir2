import fs from 'node:fs';
import { pathToFileURL } from 'node:url';
import { loadGeometry, ROOT as PRIOR, PROJECT, STEPS, key, dist, pin } from '../partition-span2-compact-halo-scene-v1/static-lib.mjs';
const ROOT=import.meta.dirname,navFile=PROJECT+'/apps/web/scripts/quest-agent/protocol-navigation.mjs';
const raw=fs.readFileSync(navFile,'utf8');let counted=raw;
const mutations=[
  ['export function findProtocolWalkPath({','export let latestWalkDiagnostic = null;\nexport function findProtocolWalkPath({'],
  ['  maxExpanded = map?.width * map?.height,\n}) {\n  validateCollisionMap(map);\n  const origin = normalizePoint(start, "start");','  maxExpanded = map?.width * map?.height,\n}) {\n  latestWalkDiagnostic = { expanded: 0, limit: null, status: "entered" };\n  validateCollisionMap(map);\n  const origin = normalizePoint(start, "start");'],
  ['  const startIndex = cellIndex(map, origin.x, origin.y);','  latestWalkDiagnostic.limit = limit;\n  const startIndex = cellIndex(map, origin.x, origin.y);'],
  ['    expanded += 1;','    expanded += 1;\n    latestWalkDiagnostic.expanded = expanded;'],
  ['      return reconstructPath(map, predecessor, currentNode.index);','      latestWalkDiagnostic.status = "found";\n      return reconstructPath(map, predecessor, currentNode.index);'],
  ['  return null;\n}\n\n/** Convert a point path','  latestWalkDiagnostic.status = expanded >= limit ? "node-limit" : "frontier-exhausted";\n  return null;\n}\n\n/** Convert a point path'],
];
for(const[a,b]of mutations){if(counted.split(a).length!==2)throw new Error('Unexpected original navigation source shape for counted-only adapter');counted=counted.replace(a,b);}
let inverse=counted;for(const[a,b]of[...mutations].reverse())inverse=inverse.replace(b,a);if(inverse!==raw)throw new Error('Instrumentation inverse differs');
const countedFile=ROOT+'/protocol-navigation-counted.mjs';fs.writeFileSync(countedFile,counted,{flag:'wx'});
const instrumented=await import(pathToFileURL(countedFile)),g=await loadGeometry();
const file=PRIOR+'/candidate-100-nearest-ordered-final-v3.json',homes=JSON.parse(fs.readFileSync(file)).anchors;
const prior=JSON.parse(fs.readFileSync(PRIOR+'/prefix-scout-cost-nearest-final-v3.json')).stages.find(s=>s.count===100);
const from=prior.legs.at(-1).goal,actorIndex=prior.failedObserverIndex,home=homes[actorIndex],cluster=g.cluster(homes),allowed=p=>g.driver.insideCombatCoverage(cluster,p),occupied=new Set(homes.map(key));
const goals=STEPS.map(([dx,dy])=>({x:home.x+dx,y:home.y+dy})).filter(p=>g.clear(p)&&allowed(p)&&dist(p,from)>0&&!occupied.has(key(p))).sort((a,b)=>dist(a,from)-dist(b,from));
const attempts=[];
for(const goal of goals.slice(0,4)){
  const obstacles=[...g.npcs,...homes.filter((h,i)=>i!==5&&key(h)!==key(from)&&key(h)!==key(goal))];
  const options={map:g.map,start:from,target:goal,dynamicObstacles:obstacles,maxExpanded:4000};
  const normal=g.nav.findProtocolWalkPath(options),points=instrumented.findProtocolWalkPath(options),same=JSON.stringify(normal)===JSON.stringify(points);
  if(!same)throw new Error('Counted adapter changed the returned original path');
  const outside=points?.filter(p=>!allowed(p))??[],transfers=points?.filter(p=>g.transferMask[g.index(p)])??[];
  attempts.push({goal,normalAndCountedPathsEqual:same,diagnostic:{...instrumented.latestWalkDiagnostic},found:Boolean(points),steps:points?points.length-1:null,
    coveragePassed:Boolean(points)&&outside.length===0,transferPassed:Boolean(points)&&transfers.length===0,
    outsideCoverageCells:outside,transferCells:transfers,path:points});
}
const result={schemaVersion:1,createdAt:new Date().toISOString(),scope:'Count-only inverse-exact local adapter for originalA*; no product change or increased node/goal budget',
  originalScene:pin(file),originalScoutResult:pin(PRIOR+'/prefix-scout-cost-nearest-final-v3.json'),from,actorIndex,home,firstFour:attempts,
  originalNavigation:pin(navFile),countedNavigation:pin(countedFile),sourceInverseRawExact:true,
  diagnosis:attempts.every(a=>a.found&&!a.coveragePassed)?'All first4 original4000A* paths were found, but each shortest path leaves declared active coverage; not node exhaustion or terrain destination failure.':
    'See exact per-goal originalpath/node/coverage evidence; do not infer one generic failure reason.',
  repairBudget:{maxChangedSuffixActors:4,candidatesPerActor:16,maxHomeDisplacement:6,firstTenByteExact:true,source9Unchanged:true},sourcePins:[...g.sourcePins,pin(import.meta.filename)],accepted:false,capacityAccepted:false};
const out=ROOT+'/original-failure-diagnosis-v1.json';fs.writeFileSync(out,JSON.stringify(result,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({out:pin(out),from,actorIndex,home,diagnosis:result.diagnosis,firstFour:attempts.map(({path,...a})=>a)}));
