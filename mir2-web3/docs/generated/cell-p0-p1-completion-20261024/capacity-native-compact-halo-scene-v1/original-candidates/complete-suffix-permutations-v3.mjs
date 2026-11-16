import fs from 'node:fs';
import { loadGeometry, ROOT, key, dist, pin, writeNew } from './static-lib.mjs';
const originalFile=ROOT+'/candidate-100-compact-v2.json',tenFile=ROOT+'/candidate-10-compact-v2.json',original=JSON.parse(fs.readFileSync(originalFile)),g=await loadGeometry(),out=[];
for(const mode of ['nearest','continuous']){
  const inputFile=ROOT+`/suffix-${mode}-order-v3.json`,prior=JSON.parse(fs.readFileSync(inputFile)),anchors=[...original.anchors.slice(0,10),...prior.choices.map(c=>c.home)];
  const selected=new Set(anchors.map(key)),remaining=original.anchors.filter(p=>!selected.has(key(p))),added=[];
  let previous=anchors.findLast((_,i)=>i%10!==5);
  while(remaining.length){
    const index=anchors.length,cluster=g.cluster(anchors),fighter=index%10===5;
    remaining.sort((a,b)=>{
      if(fighter){const coverA=g.driver.insideCombatCoverage(cluster,a),coverB=g.driver.insideCombatCoverage(cluster,b);if(coverA!==coverB)return Number(coverB)-Number(coverA);}
      if(mode==='nearest'||fighter)return dist(a,previous)-dist(b,previous)||a.y-b.y||a.x-b.x;
      const bandA=Math.floor((a.y-512)/12),bandB=Math.floor((b.y-512)/12);return bandA-bandB||(bandA%2?b.x-a.x:a.x-b.x)||a.y-b.y;
    });
    const home=remaining.shift(),covered=g.driver.insideCombatCoverage(cluster,home);
    added.push({actorIndex:index,role:fighter?'combat':'movement',home,coveredByPreviouslyActiveObservers:covered});anchors.push(home);if(!fighter)previous=home;
  }
  if(anchors.length!==100||new Set(anchors.map(key)).size!==100||anchors.some(p=>!original.anchors.some(q=>key(p)===key(q)))||JSON.stringify(anchors.slice(0,10))!==JSON.stringify(original.anchors.slice(0,10)))throw new Error('Coordinate permutation identity failed');
  const file=ROOT+`/candidate-100-${mode}-ordered-final-v3.json`,ten=ROOT+`/candidate-10-${mode}-ordered-final-v3.json`;
  const outputs=[writeNew(file,{...original,anchors})];fs.writeFileSync(ten,fs.readFileSync(tenFile),{flag:'wx'});outputs.push(pin(ten));
  const receipt=writeNew(ROOT+`/suffix-${mode}-completion-v3.json`,{schemaVersion:1,createdAt:new Date().toISOString(),mode,
    scope:'Completion of the SAME one of two bounded suffix permutations. No new coordinate/ordering search. Previous overly strict per-observer frontier construction refusal is retained.',
    originalScene:pin(originalFile),originalPrefix:pin(tenFile),priorIncompleteAttempt:pin(inputFile),
    firstTenRawExact:true,entireCoordinateSetExact:true,remainingCoordinatesAppended:added,
    unprovenEarlierCoverageForMovementObserverAllowedForAuthoringOnly:true,
    needsIndependentPrefixValidation:true,outputs,sourcePins:[...g.sourcePins,pin(import.meta.filename)],accepted:false,capacityAccepted:false});out.push(receipt);
}
console.log(JSON.stringify({out}));
