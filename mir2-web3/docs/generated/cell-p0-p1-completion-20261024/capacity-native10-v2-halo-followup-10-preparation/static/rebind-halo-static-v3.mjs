import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';
const repo='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3',root='C:/mir2-build/p01-completion-20261024';
const expected=process.argv[2],frozenArgument=process.argv[3],expectedManifestSha=process.argv[4],expectedAggregate=process.argv[5];
if(process.argv.length!==6||!path.isAbsolute(frozenArgument??'')||path.resolve(frozenArgument)!==path.resolve(path.join(root,'capacity-native-survival-pursuit-witness-driver-frozen-v1.json'))||!/^[a-f0-9]{64}$/.test(expectedManifestSha??'')||!/^[a-f0-9]{64}$/.test(expectedAggregate??''))throw Error('Explicit final new-driver manifest path/rawSHA/aggregate required; draft must not run before root commit');if(!/^[0-9a-f]{40}$/.test(expected??'')||expected==='46880bd1ef415131c4cf85aeb4dd2fa4f6fd4f9b')throw Error('Explicit new committed driverHEAD required');
const git=(...args)=>execFileSync('git',args,{cwd:repo,encoding:'utf8',windowsHide:true,maxBuffer:8*1024*1024});
if(git('rev-parse','HEAD').trim()!==expected)throw Error('Expected driverHEAD differs');git('diff','--exit-code','HEAD','--');
const digest=b=>crypto.createHash('sha256').update(b).digest('hex'),read=file=>JSON.parse(fs.readFileSync(file));
const pin=file=>{const b=fs.readFileSync(file);return{path:file,bytes:b.length,sha256:digest(b)}};
const oldFile=path.join(root,'partition-span4-halo-static-validation-v1.json'),oldReadyFile=path.join(root,'partition-span4-halo-scene-readiness-v1.json');
const validationFile=path.join(root,'partition-span4-halo-static-validation-v3.json'),readyFile=path.join(root,'partition-span4-halo-static-readiness-v3.json');
for(const f of[validationFile,readyFile])if(fs.existsSync(f))throw Error('Refuse overwrite staticv3');
const old=read(oldFile),ready=read(oldReadyFile),frozenFile=frozenArgument,frozen=read(frozenFile);
if(pin(frozenFile).sha256!==expectedManifestSha||frozen.files?.length!==9)throw Error('Final new-driver manifest raw identity differs');
if(frozen.sourceSha256!==expectedAggregate)throw Error('Unexpected final new-driver source');
const finalRaw=frozen.files.map(f=>{const file=path.join(repo,f.file),p=pin(file);if(p.bytes!==f.bytes||p.sha256!==f.sha256)throw Error('Final raw9 differs');const raw=fs.readFileSync(file,'utf8').replace(/\r\n/g,'\n'),blob=git('show',expected+':mir2-web3/'+f.file);if(digest(raw)!==digest(blob))throw Error('Final raw9 does not match new committed GitLF');return f;});
if(digest(JSON.stringify(finalRaw))!==frozen.sourceSha256)throw Error('Final source9 aggregate differs');
const scene100File=path.join(root,'partition-ordinary-two-name-span4-halo-100-v1.json'),scene10File=path.join(root,'partition-ordinary-two-name-span4-halo-10-v1.json');
if(pin(scene100File).sha256!=='19c80f2722a9fc992f6834304c4fc5bf54f2fe86dbcf7c53887589d11aa9f8fb'||pin(scene10File).sha256!=='539c8abab8a3930a33a5a130601591c3bfdaede166e5b93cec995479df1c745c')throw Error('Approved scene bytes differ');
const scene=read(scene100File),prefix=read(scene10File),trimmed={...scene,anchors:scene.anchors.slice(0,10),combatClusters:scene.combatClusters.map(c=>({combatActorIndices:c.combatActorIndices.filter(i=>i<10),movementActorIndices:c.movementActorIndices.filter(i=>i<10)}))};
if(JSON.stringify(prefix)!==JSON.stringify(trimmed))throw Error('Exact10 prefix mismatch');
const soakFile=path.join(repo,'apps/web/scripts/playtest-capacity-soak.mjs'),source=await import(pathToFileURL(soakFile));
const preimage=fs.readFileSync(path.join(root,'phase-window-timing-v1/playtest-capacity-soak.mjs.preimage'),'utf8');
const unchangedFunctions=['insideCombatCoverage','validateNativeExplicitHomes','validateNativeCombatClusters'];
for(const name of unchangedFunctions)if(!preimage.includes(source[name].toString()))throw Error('Historical geometry function changed: '+name);
const currentPins=old.sourcePins.map(p=>{const current=pin(p.path);if(path.resolve(p.path)!==path.resolve(soakFile)&&(current.sha256!==p.sha256||current.bytes!==p.bytes))throw Error('Historical geometry/data import changed');return current;});
const nav=await import(pathToFileURL(path.join(repo,'apps/web/scripts/quest-agent/protocol-navigation.mjs')));
const map=await nav.loadProtocolCollisionMap('0',{packagedMapRoot:path.join(repo,'apps/web/lib/generated/crystal-map-pack')});
const npcs=await source.loadStationaryNpcObstacles('0'),metadata=read(path.join(repo,'apps/web/lib/generated/crystal_respawn_manifest.json')).maps.find(m=>m.map_file_name==='0');
const transfers=metadata.movements.map(m=>({bounds:{minX:m.source.x,maxX:m.source.x,minY:m.source.y,maxY:m.source.y}}));
const body=source.validateNativeExplicitHomes(map,{x:288,y:616},100,scene.anchors,4,transfers,npcs);
source.validateNativeCombatClusters(scene.combatClusters,scene.anchors,new Set([5,15,25,35,45,55,65,75,85,95]),4);
const key=p=>p.x+','+p.y,npcSet=new Set(npcs.map(key)),transferSet=new Set(metadata.movements.map(m=>key(m.source))),unique=new Set();let checks=0;
for(const home of scene.anchors)for(let dy=-1;dy<=5;dy++)for(let dx=-1;dx<=5;dx++){
 const p={x:home.x+dx,y:home.y+dy};checks++;unique.add(key(p));if(!nav.protocolMapCellIsWalkable(map,p)||npcSet.has(key(p))||transferSet.has(key(p)))throw Error('Halo static blockage');
}
const fresh={exact10Prefix:true,explicitHomeValidator:{passed:true,footprintCells:body.footprintCells,visitedCells:body.visitedCells,mapCells:body.mapCells},clusterValidatorPassed:true,haloChecks:checks,uniqueHaloCells:unique.size,haloStaticBlockages:0,
 geometryFunctionsExactOriginalPreimage:unchangedFunctions,canonicalDataAndNavigationInputsUnchanged:true,finalRaw9MatchesFreeze:true};
const next={...old,createdAt:new Date().toISOString(),scope:'Fresh halo/explicit-home/cluster revalidation on committed final ordinary native follow-up driver; unchanged canonical target-path analysis reused only after exact geometry-function/data equality.',
 sourceCommit:expected,driverSourceCommit:expected,sourcePins:[...currentPins,pin(import.meta.filename)],driverFrozenManifest:pin(frozenFile),driverSourceSha256:frozen.sourceSha256,
 historicalGeometryProof:pin(oldFile),freshRevalidation:fresh,geometryTargetPathAnalysis:'stages and conditional supply retained from pinnedv1 because scene/data/navigation and the exact coverage/validator function bodies are unchanged. No live supply or capacity claim.',
 validationHelper:pin(import.meta.filename)};
fs.writeFileSync(validationFile,JSON.stringify(next,null,2)+'\n',{flag:'wx'});
const nextReady={...ready,status:'Static geometry revalidated against committed final ordinary native follow-up driver; fresh strict live gate remains pending',driverSourceCommit:expected,driverSourceSha256:frozen.sourceSha256,driverFrozenManifest:pin(frozenFile),
 inputPins:{...ready.inputPins,scene100:pin(scene100File),exactScene10:pin(scene10File),validation:pin(validationFile),authorInputs:next.sourcePins,summaryHelper:pin(import.meta.filename)},
 historicalReadiness:pin(oldReadyFile),freshRevalidation:fresh,sourceAndGeometryTimingScope:'Backend Release unchanged. Scene bytes unchanged. Staticv1 remains an exact historical record; only explicitly revalidated static checks and final follow-up driver/source pins are current.'};
fs.writeFileSync(readyFile,JSON.stringify(nextReady,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({validation:pin(validationFile),readiness:pin(readyFile),driverSourceCommit:expected,driverSourceSha256:frozen.sourceSha256,freshRevalidation:fresh}));
