import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {pathToFileURL} from 'node:url';
const root='C:/mir2-build/p01-completion-20261024',out=path.join(root,'probe08-analysis');
const repo='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const pin=file=>{const b=fs.readFileSync(file);return{file,bytes:b.length,sha256:crypto.createHash('sha256').update(b).digest('hex')}};
const names={scene100:'partition-ordinary-two-name-span4-halo-100-v1.json',scene10:'partition-ordinary-two-name-span4-halo-10-v1.json',validation:'partition-span4-halo-static-validation-v1.json',readiness:'partition-span4-halo-scene-readiness-v1.json'};
const files=Object.fromEntries(Object.entries(names).map(([k,v])=>[k,path.join(root,v)]));
const scene=JSON.parse(fs.readFileSync(files.scene100)),prefix=JSON.parse(fs.readFileSync(files.scene10)),v=JSON.parse(fs.readFileSync(files.validation));
const originalFile=path.join(root,'partition-ordinary-two-name-safe-home-100-candidate-v3.json'),original=JSON.parse(fs.readFileSync(originalFile));
const priorFile=path.join(root,'partition-ordinary-two-name-span4-100-proposal-v1.json'),prior=JSON.parse(fs.readFileSync(priorFile));
const prefixExpected={...scene,anchors:scene.anchors.slice(0,10),combatClusters:scene.combatClusters.map(c=>({combatActorIndices:c.combatActorIndices.filter(i=>i<10),movementActorIndices:c.movementActorIndices.filter(i=>i<10)}))};
if(JSON.stringify(prefix)!==JSON.stringify(prefixExpected))throw Error('Prefix mismatch');
const navFile=path.join(repo,'apps/web/scripts/quest-agent/protocol-navigation.mjs'),nav=await import(pathToFileURL(navFile));
const map=await nav.loadProtocolCollisionMap('0',{packagedMapRoot:path.join(repo,'apps/web/lib/generated/crystal-map-pack')});
const npcs=JSON.parse(fs.readFileSync(path.join(repo,'packages/game-data/data/generated/crystal_npc_info_manifest.json'))).npcs.filter(n=>n.map_file_name==='0').map(n=>n.location);
const respawns=JSON.parse(fs.readFileSync(path.join(repo,'apps/web/lib/generated/crystal_respawn_manifest.json'))).maps.find(m=>m.map_file_name==='0');
const key=p=>p.x+','+p.y,npc=new Set(npcs.map(key)),transfers=new Set(respawns.movements.map(m=>key(m.source))),halos=new Set(),bodies=new Set();
let haloChecks=0,bodyChecks=0;const bad=[];
for(let i=0;i<100;i++)for(let dy=-1;dy<=5;dy++)for(let dx=-1;dx<=5;dx++){
 const p={x:scene.anchors[i].x+dx,y:scene.anchors[i].y+dy};haloChecks++;halos.add(key(p));
 if(!nav.protocolMapCellIsWalkable(map,p)||npc.has(key(p))||transfers.has(key(p)))bad.push({actorIndex:i,p});
 if(dx>=0&&dx<=4&&dy>=0&&dy<=4){bodyChecks++;if(bodies.has(key(p)))throw Error('Body overlap');bodies.add(key(p));}
}
if(bad.length||haloChecks!==4900||bodyChecks!==2500)throw Error('Static halo violation');
const dist=(a,b)=>Math.max(Math.abs(a.x-b.x),Math.abs(a.y-b.y));
const density=(s,count)=>{let pairs=0,sum=0;for(let a=0;a<count;a++)for(let b=a+1;b<count;b++){const d=dist(s.anchors[a],s.anchors[b]);if(d<=16)pairs++;sum+=d;}return{homePairsWithin16:pairs,meanPairChebyshev:sum/(count*(count-1)/2)};};
const stageSupply=v.stages.map(s=>({count:s.count,combatActorIndices:s.fighterIndices,movementObserverCount:s.activeObserverIndices.length,observerDisconnected:s.observerDisconnected,
 fighterHomeCovered:s.fighters.every(f=>f.homeCoveredByActiveObserver),fighterIsolatedObserverCounts:s.fighters.map(f=>f.isolatedObserverIndices.length),
 uniqueCanonicalTargetCount:s.uniqueExact4000CanonicalTargets,targetCountsByName:s.targetCountsByName,
 conditionalPositiveHitsPerMinuteByFixedDamage:s.conditionalPositiveHitCapacityByFixedDamage,requiredAttacksPerMinute:s.requirementsPerMinute.attacks,requiredPositiveHitsPerMinute:s.requirementsPerMinute.positiveHits,
 ruleSupply:[...new Set(s.fighters.flatMap(f=>f.targetPaths.map(p=>Math.floor((p.objectId-200000)/100))))].sort((a,b)=>a-b).map(index=>{const r=respawns.respawns.find(r=>r.respawn_index===index);if(!r)throw Error('Unknown native rule');return{ruleIndex:index,monsterName:r.monster_name,nativeHp:r.monster_hp,delayMinutes:r.delay_minutes,randomDelayMinutes:r.random_delay_minutes,slotCount:r.count};})}));
const readiness={schemaVersion:1,status:'Static candidate pending root adoption and fresh strict probe',staticOnly:true,capacityAccepted:false,
 inputPins:{scene100:pin(files.scene100),exactScene10:pin(files.scene10),validation:pin(files.validation),originalFormalSpan2:pin(originalFile),priorUnadoptedSpan4:pin(priorFile),authorInputs:v.sourcePins,summaryHelper:pin(import.meta.filename)},
 exactPrefix:{passed:true,count:10,anchorsEqual:true,clusterIndicesTrimmedOnly:true},
 staticChecks:{actorCount:100,patrolSpan:4,bodyCellsChecked:bodyChecks,uniqueBodyCells:bodies.size,haloCellsCheckedPerHome:haloChecks,uniqueHaloCells:halos.size,terrainNpcTransferViolations:bad.length,
 originalCornerStaticCardinalExitsPerCorner:4,twoSingleCellBlockerScope:'At each original corner, two single-cell occupants can block at most2 of4 otherwise-clear cardinal first steps. Does not model more blockers, multi-cell hints or future route availability.',
 perimeterRunSegmentsPerHome:8,perimeterScope:'Four4-cell cardinal edges, each divided into two legal2-cell segments entirely within its own5x5 rectangle. No forcedRun or change to live planner.',
 bootstrapMaxExpanded:4000,bootstrapMaximumPathSteps:Math.max(...v.selections.map(s=>s.bootstrap.pathSteps)),runtimeTargetMaxExpanded:4000,
 bodyCanonicalSpawnAndDangerExcluded:'Validated by source-pinned author; halo exclusion is terrain/NPC/transfer only. No live monster safety promise.'},
 changes:{originalHaloLawfulHomes:58,finalUnchangedFromPriorSpan4:v.homeChanges.unchanged,finalChangedFromPriorSpan4:v.homeChanges.changed,maxChebyshevDisplacement:v.homeChanges.maxChebyshev,sumChebyshevDisplacement:v.homeChanges.sumChebyshev,
 reasonForAdditionalSix:'Six statically halo-lawful homes also moved because current-prefix coverage connectivity or already pre-reserved body exclusions required it; final selections, not the58 initial pre-reservations, determine actual unchanged count.'},
 staticDensityComparison:[2,10,25,50,100].map(count=>({count,originalFormalSpan2:density(original,count),priorUnadoptedSpan4:density(prior,count),newHaloSpan4:density(scene,count)})),
 densityScope:'Home-centre Chebyshev proximity is a static comparability indicator, not currentAOI, visibility, live witness or workload acceptance. Full100 pairs within16 fall775->621; first10 rise12->24. The new scene is not density-identical.',
 prefixes:stageSupply,
 conditionalSupplyLimits:['Rates deduplicate canonical target identities and assume fixed damage, full slot refresh and ideal allocation; they are not live attack or hit supply.','At fixed damage10,100-fighter aggregate hypothetical rate82.783/min is below required120positivehits/min; at damage5 hypothetical143.483/min exceeds120 with limited margin. Actual level/DPS growth, failedTargets quarantine, pursuit and corpse/respawn timing may invalidate a two-hour workload.','Each fighter still independently needs18launched attacks and12positive dual-end hits per minute; aggregate ideal supply cannot prove that distribution.','Fresh strict09 must independently pass2/10 stages and6 rolling combat windows before formal06. No oldprobe result is promoted or rewritten.']};
fs.writeFileSync(files.readiness,JSON.stringify(readiness,null,2)+'\n',{flag:'wx'});
const readme=path.join(out,'README-v1.md');
fs.writeFileSync(readme,`# Probe08 bounded failure analysis\n\nRun1792885245847-0368d2eb remains failed. The two300-second stages passed; soak ended before completion after actorLa5189302 recorded12 navigation-blocked opportunities in a60-second window (68/80 native completion). No actor02 owner correction or timeout caused this failure.\n\nThe first complete local AOI diagnostic proves zero legal first step at237,665: terrain blocks north/west, a live Scarecrow occupies east238,665, and actual owner-acknowledged fighter05 occupies south237,666. The diagonal side-cell rule blocks all diagonal escapes too. The other11 blocked opportunities remain failed; their complete per-decision AOI/hints were not recorded and are not reconstructed.\n\nPaired ObjectDied forScarecrow206636 occurs at1792886284492; actor02 enters its old cell294ms later, while fighter05 leaves the south cell another1008ms later. This supports the actual unblock chain. It is legitimate dynamic authoritative body blocking, insufficient completed workload rather than proof of server overload.\n\nAll98 public slow-stage rows were counted. None is socket_input.queue_residence and none falls in the selected blocked episode. This does not rule out uninstrumented server work or explain an earlier run's stall. No recordings or private/account payloads were read.\n\nPublic whitelist: analyze-v1.mjs, summary-v1.json, selected-events-v1.json, thisREADME, finalize-public-analysis-v1.mjs and public-whitelist-v1.json. The raw driver samples remain external and bound bySHA/bytes/line numbers in the analysis. Prepared but unexecuted review-span4-padding-v1.mjs is not needed evidence and is excluded.\n\nA separately authored new7x7-halo scene is static-only and requires root adoption plus fresh probe. It changes no08 verdict or source/threshold. See partition-span4-halo-scene-readiness-v1.json outside this directory for exact scene pins and density/supply limitations.\n`,{flag:'wx'});
const whitelist=['analyze-v1.mjs','summary-v1.json','selected-events-v1.json','README-v1.md','finalize-public-analysis-v1.mjs'].map(n=>pin(path.join(out,n)));
const sensitive=[];function scan(o,where=''){if(!o||typeof o!=='object')return;for(const[k,value]of Object.entries(o)){if(/password|secret|credential|access.?token|refresh.?token|authorization|cookie|email|chat.?content|chat.?text/i.test(k))sensitive.push(where+'.'+k);scan(value,where+'.'+k);}}
for(const n of ['summary-v1.json','selected-events-v1.json'])scan(JSON.parse(fs.readFileSync(path.join(out,n))),n);
if(sensitive.length)throw Error('Unexpected sensitive projection keys');
const publicFile=path.join(out,'public-whitelist-v1.json');
fs.writeFileSync(publicFile,JSON.stringify({schemaVersion:1,scope:'Explicit public08 derived artifacts only; no private stores, credentials, executables, recordings or complete external raw included.',files:whitelist,structuredSensitiveKeyMatches:sensitive,rawRemainsExternal:true},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({readiness:pin(files.readiness),publicWhitelist:pin(publicFile),files:whitelist,changes:readiness.changes,staticChecks:readiness.staticChecks}));
