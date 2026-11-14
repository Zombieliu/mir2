import fs from 'node:fs';
import { loadGeometry, PARAMETERS, pin } from '../partition-span2-compact-halo-scene-v1/static-lib.mjs';
const ROOT=import.meta.dirname,g=await loadGeometry(),file=ROOT+'/candidate-100-repaired-v4.json',scene=JSON.parse(fs.readFileSync(file)),rows=[];
for(const count of PARAMETERS.prefixes){
  const active=scene.anchors.slice(0,count),fighters=active.map((_,i)=>i).filter(i=>i%10===5),movement=active.map((_,i)=>i).filter(i=>i%10!==5);
  const explicit=g.driver.validateNativeExplicitHomes(g.map,PARAMETERS.origin,count,active,2,g.transfers,g.npcs);
  let cluster;
  if(fighters.length){
    const validated=g.driver.validateNativeCombatClusters([{combatActorIndices:fighters,movementActorIndices:movement}],active,new Set(fighters),2);
    if(validated.length!==1||validated[0].observerHomes.length!==movement.length)throw new Error('Original prefix cluster unexpectedly changed');
    cluster={passed:true,actualOriginalFunctionCalled:true,combatActorIndices:fighters,movementActorIndices:movement,activeOnlyObserverHomes:validated[0].observerHomes};
  }else cluster={passed:true,actualOriginalFunctionCalled:false,notApplicable:'Prefix2hasnoactivecombat actor. Original cluster helper requires a nonempty fighter list; runtime only uses explicit movement homes at this stage.',combatActorIndices:[],movementActorIndices:movement};
  rows.push({count,originalExplicitHomesValidatorPassed:true,explicitResult:{footprintCells:explicit.footprintCells,visitedCells:explicit.visitedCells,mapCells:explicit.mapCells},cluster});
}
const out=ROOT+'/original-prefix-contracts-v4.json';fs.writeFileSync(out,JSON.stringify({schemaVersion:1,createdAt:new Date().toISOString(),scope:'Fresh direct calls of original exported validators at every declared prefix; no copied validator or service execution',scene:pin(file),prefixes:rows,sourcePins:[...g.sourcePins,pin(import.meta.filename)],accepted:false,capacityAccepted:false},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output:pin(out),prefixes:rows.map(r=>({count:r.count,explicit:r.originalExplicitHomesValidatorPassed,footprint:r.explicitResult.footprintCells,cluster:r.cluster.actualOriginalFunctionCalled?'originalpassed':'no-fighter-notapplicable'}))}));
