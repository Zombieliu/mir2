import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
const base='C:/mir2-build/p01-completion-20261024';
const project='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const head='46880bd1ef415131c4cf85aeb4dd2fa4f6fd4f9b';
const same=(a,b)=>path.resolve(a).toLowerCase()===path.resolve(b).toLowerCase();
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const allowedSourceFiles=[
 path.join(base,'partition-ordinary-two-name-span4-100-proposal-v1.json'),path.join(base,'partition-span4-static-proposal-v1.json'),path.join(base,'partition-span4-static-proposal-v1.mjs'),
 path.join(project,'apps/web/lib/generated/crystal_respawn_manifest.json'),path.join(project,'packages/game-data/data/generated/crystal_monster_manifest.json'),path.join(project,'packages/game-data/data/generated/crystal_npc_info_manifest.json'),path.join(project,'apps/web/lib/generated/crystal-map-pack/0.map.gz'),
 path.join(project,'apps/web/scripts/playtest-capacity-soak.mjs'),path.join(project,'apps/web/scripts/quest-agent/protocol-navigation.mjs'),path.join(base,'partition-span4-halo-static-author-v1.mjs'),path.join(base,'rebind-halo-static-v3.mjs')];
async function pinned(entry,expected){
 if(!entry||!same(entry.file??'',expected)||!Number.isSafeInteger(entry.bytes)||entry.bytes<1||!/^[a-f0-9]{64}$/.test(entry.sha256??''))throw Error('Invalid exact halo scene/static pin');
 const actual=await fs.realpath(expected),stat=await fs.lstat(expected);
 if(!same(actual,expected)||stat.isSymbolicLink()||!stat.isFile())throw Error('Redirected halo scene/static input forbidden');
 const bytes=await fs.readFile(expected);if(bytes.length!==entry.bytes||sha(bytes)!==entry.sha256)throw Error('Pinned halo scene/static input changed');
 return bytes;
}
const roles=(scene,key)=>scene.combatClusters.flatMap(c=>c[key]);
function shape(scene,count){
 if(scene.schemaVersion!==1||scene.mapFileName!=='0'||scene.patrolSpan!==4||JSON.stringify(scene.monsterNames)!=='["Deer","Scarecrow"]'||!Array.isArray(scene.anchors)||scene.anchors.length!==count||!Array.isArray(scene.combatClusters)||scene.combatClusters.length!==1)throw Error('Approved halo scene shape differs');
 const combat=roles(scene,'combatActorIndices'),movement=roles(scene,'movementActorIndices'),expectedCombat=Array.from({length:count/10},(_,i)=>5+i*10),expectedMovement=Array.from({length:count},(_,i)=>i).filter(i=>!expectedCombat.includes(i));
 if(JSON.stringify(combat)!==JSON.stringify(expectedCombat)||JSON.stringify(movement)!==JSON.stringify(expectedMovement)||scene.anchors.some(p=>!Number.isSafeInteger(p.x)||!Number.isSafeInteger(p.y))||new Set(scene.anchors.map(p=>`${p.x},${p.y}`)).size!==count)throw Error('Halo scene ordinary role indices/unique homes differ');
}
export async function verifySceneContract(root,kind){
 if(!['probe','formal'].includes(kind))throw Error('Explicit halo scene kind required');
 const bindingFile=path.join(root,'scene-binding.json'),bindingBytes=await fs.readFile(bindingFile),binding=JSON.parse(bindingBytes);
 if(binding.schemaVersion!==1||binding.state!=='final-static-scene-bound-not-executed'||binding.backendSourceCommit!==head||binding.driverSourceCommit!=='6e05932000740abc88997db9496c57b301e0a4a9'||binding.staticOnly!==true||binding.dynamicSupplyVerified!==false||binding.capacityAccepted!==false)throw Error('Halo static scene binding is not final');
 if(binding.probeScene?.bytes!==804||binding.probeScene.sha256!=='539c8abab8a3930a33a5a130601591c3bfdaede166e5b93cec995479df1c745c'||binding.formalScene?.bytes!==5844||binding.formalScene.sha256!=='19c80f2722a9fc992f6834304c4fc5bf54f2fe86dbcf7c53887589d11aa9f8fb')throw Error('Root-approved halo scene raw identity differs');
 if(binding.analysis?.bytes!==420086||binding.analysis.sha256!=='8be09cc4cbd02a099d0fdf9ae46b966c5985ad7ffb04ab86856011e0f6c7a231'||binding.readiness?.bytes!==17982||binding.readiness.sha256!=='82dc6e22f13786f4d3eb94c4978ca46695fcf3bde23cf84a8c68618fe2282105')throw Error('Final v3 static source identity differs');
 const readiness=JSON.parse(await pinned(binding.readiness,path.join(base,'partition-span4-halo-static-readiness-v3.json')));
 if(readiness.staticOnly!==true||readiness.capacityAccepted!==false||readiness.driverSourceCommit!==binding.driverSourceCommit||readiness.driverSourceSha256!=='c7928e91e0528132c470ad1d87ffd63656eb59b2ce35b602b5e84f4603434298')throw Error('Static readiness is not bound to the final timing driver');
 const probeBytes=await pinned(binding.probeScene,path.join(base,'partition-ordinary-two-name-span4-halo-10-v1.json')),formalBytes=await pinned(binding.formalScene,path.join(base,'partition-ordinary-two-name-span4-halo-100-v1.json')),analysisBytes=await pinned(binding.analysis,path.join(base,'partition-span4-halo-static-validation-v3.json'));
 const probe=JSON.parse(probeBytes),formal=JSON.parse(formalBytes),analysis=JSON.parse(analysisBytes);shape(probe,10);shape(formal,100);
 if(JSON.stringify(formal.anchors.slice(0,10))!==JSON.stringify(probe.anchors)||JSON.stringify(roles(formal,'combatActorIndices').filter(i=>i<10))!==JSON.stringify(roles(probe,'combatActorIndices'))||JSON.stringify(roles(formal,'movementActorIndices').filter(i=>i<10))!==JSON.stringify(roles(probe,'movementActorIndices')))throw Error('Halo formal first10 does not match exact probe scene');
 const inv=analysis.invariants;
 if(analysis.schemaVersion!==1||analysis.staticOnly!==true||analysis.capacityAccepted!==false||analysis.sourceCommit!==binding.driverSourceCommit||analysis.validatorResult?.passed!==true||analysis.validatorResult.footprintCells!==2500||
   inv?.mapFileName!=='0'||inv.span!==4||inv.footprint!=='5x5'||inv.staticHalo!=='7x7 terrain/NPC/transfer clear'||inv.maxExpanded!==4000||inv.nativeOpportunityMs!==750||inv.minimumRunFraction!==.5||inv.attacksPerFighterMinute!==18||inv.positiveHitsPerFighterMinute!==12||inv.existingGateUnchanged!==true||inv.noTeleport!==true||inv.ordinaryProtocolOnly!==true||JSON.stringify(inv.monsterNames)!=='["Deer","Scarecrow"]'||JSON.stringify(analysis.stages?.map(s=>s.count))!=='[2,10,25,50,100]')throw Error('Halo static analysis does not describe the original ordinary workload');
 if(!same(analysis.candidateFile??'',binding.formalScene.file)||!same(analysis.prefixFile??'',binding.probeScene.file)||!Array.isArray(analysis.sourcePins)||analysis.sourcePins.length!==allowedSourceFiles.length||!Array.isArray(binding.staticClosure)||binding.staticClosure.length!==allowedSourceFiles.length)throw Error('Halo static analysis source closure differs');
 for(const file of allowedSourceFiles){const source=analysis.sourcePins.find(p=>same(p.path??'',file)),entry=binding.staticClosure.find(p=>same(p.file??'',file));if(!source||!entry||source.bytes!==entry.bytes||source.sha256!==entry.sha256)throw Error('Halo analysis raw input is not explicitly bound');await pinned(entry,file);}
 const local=await fs.readFile(path.join(root,'scenario.json')),expected=kind==='probe'?probeBytes:formalBytes;if(!local.equals(expected))throw Error('Local scenario is not exact approved halo bytes');
 const sharedFiles=[binding.probeScene,binding.formalScene,binding.analysis,binding.readiness,...binding.staticClosure],files=[...sharedFiles,{file:bindingFile,bytes:bindingBytes.length,sha256:sha(bindingBytes)}];
 return{schemaVersion:1,profile:'ordinary-native-span4-seven-by-seven-static-halo-v1',kind,probe,formal,probeScene:binding.probeScene,formalScene:binding.formalScene,analysis:binding.analysis,staticOnly:true,dynamicSupplyVerified:false,capacityAccepted:false,files,sourceSha256:sha(JSON.stringify(sharedFiles)),scope:'5x5 patrol body and7x7 terrain/NPC/transfer clear halo per home; halo cells may overlap and canonical monsters may occupy halo. Static geometry is not live supply, navigation or capacity acceptance.'};
}
