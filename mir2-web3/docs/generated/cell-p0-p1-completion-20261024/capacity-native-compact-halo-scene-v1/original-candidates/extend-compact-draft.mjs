import fs from 'node:fs';
import { loadGeometry, PARAMETERS, ROOT, key, dist, pin, writeNew } from './static-lib.mjs';
// Alternate explicit draft retains more compact97-step scout cycle than the
// first supply-ranked draft123. Both remain unadopted and fully retained.
const draftFile=ROOT+'/draft-02.json', draft=JSON.parse(fs.readFileSync(draftFile)), g=await loadGeometry();
if(!draft.passed||draft.anchors.length!==10)throw new Error('Expected statically connected ten-home draft02');
const anchors=draft.anchors.map(p=>({...p})),used=new Set(anchors.flatMap(h=>g.body(h).map(key))),candidates=[],rejected=[];
for(let y=512;y<=682;y++)for(let x=208;x<=376;x++){
  const p={x,y};if(g.issues(p).length)continue;const targets=g.localTargets(p);
  candidates.push({p,targets,weight:targets.reduce((s,t)=>s+Math.ceil(t.hp/5)/Math.max(3,t.delayMinutes),0)});
}
const failed=new Set();
for(let actorIndex=10;actorIndex<100;actorIndex++){
  const cluster=g.cluster(anchors),connected=g.component(anchors[0],p=>g.driver.insideCombatCoverage(cluster,p),anchors.slice(1));
  const covered=new Set(g.targets.filter(t=>cluster.observerHomes.some(h=>g.witnessGoals(h,t).length)).map(t=>t.objectId)),fighter=actorIndex%10===5;
  const ranked=candidates.filter(h=>!failed.has(key(h.p))&&!g.issues(h.p,used).length&&connected.has(h.p)).map(h=>{
    const gain=h.targets.filter(t=>!covered.has(t.objectId)).reduce((s,t)=>s+Math.ceil(t.hp/5)/Math.max(3,t.delayMinutes),0);
    return{...h,gain,score:fighter?h.weight*2-Math.min(...cluster.observerHomes.map(p=>dist(p,h.p)))*.15:gain*25+h.weight*.15-Math.min(...anchors.map(p=>dist(p,h.p)))*.025};
  }).sort((a,b)=>b.score-a.score||dist(a.p,PARAMETERS.origin)-dist(b.p,PARAMETERS.origin)||a.p.y-b.p.y||a.p.x-b.p.x);
  let selected;for(const h of ranked){const proof=g.bootstrap(h.p);if(!proof.found||!proof.noTransfer||!proof.noInitialDanger){failed.add(key(h.p));rejected.push({home:h.p,proof});continue;}selected=h;break;}
  if(!selected)throw new Error('Alternate compact extension exhausted at actor'+actorIndex);
  anchors.push(selected.p);g.body(selected.p).forEach(p=>used.add(key(p)));
}
const scene=count=>({schemaVersion:1,mapFileName:'0',patrolSpan:2,monsterNames:PARAMETERS.monsterNames,anchors:anchors.slice(0,count),combatClusters:[{
  combatActorIndices:Array.from({length:count},(_,i)=>i).filter(i=>i%10===5),movementActorIndices:Array.from({length:count},(_,i)=>i).filter(i=>i%10!==5)}]});
const outputs=[writeNew(ROOT+'/candidate-100-compact-v2.json',scene(100)),writeNew(ROOT+'/candidate-10-compact-v2.json',scene(10))];
const result=writeNew(ROOT+'/compact-author-result-v2.json',{schemaVersion:1,createdAt:new Date().toISOString(),scope:'Alternate compact draft, not adoption or live acceptance',
  sourcePins:[...g.sourcePins,pin(draftFile),pin(import.meta.filename)],parameters:PARAMETERS,selectedDraft:2,selectedFirstTen:anchors.slice(0,10),
  rejectedBootstrapCandidates:rejected,outputs,capacityAccepted:false,accepted:false});
console.log(JSON.stringify({result,outputs}));
