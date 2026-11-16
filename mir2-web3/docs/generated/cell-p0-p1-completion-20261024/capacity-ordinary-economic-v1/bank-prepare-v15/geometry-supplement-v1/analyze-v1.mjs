import { readFile, writeFile } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';
import { createHash } from 'node:crypto';
import { pathToFileURL, fileURLToPath } from 'node:url';
import path from 'node:path';

const BASE='C:/mir2-build/p01-completion-20261024';
const PACKAGE=path.join(BASE,'ordinary-capacity-bank-prepare-v15');
const STEM='1793084170216-93e9fe64';
const assert=(yes,message)=>{if(!yes)throw new Error(message);};
const sha=b=>createHash('sha256').update(b).digest('hex');
const pin=async file=>{const b=await readFile(file);return {file,bytes:b.length,sha256:sha(b)};};
const json=async file=>JSON.parse(await readFile(file,'utf8'));
const inputPins=[];
async function input(file){inputPins.push(await pin(file));return json(file);}
const frozen=await input(path.join(PACKAGE,'frozen-source-v1.json'));
const repository=frozen.driver.repository;
const project=path.join(repository,'mir2-web3');
const imported=['apps/web/scripts/quest-agent/protocol-navigation.mjs','apps/web/scripts/capacity-ordinary-protection.mjs','apps/web/scripts/capacity-ordinary-supplies.mjs'];
for(const suffix of imported){
  const row=frozen.driver.files.find(r=>r.path===`mir2-web3/${suffix}`);assert(row,`missing frozen source ${suffix}`);
  const p=await pin(path.join(repository,row.path));assert(p.bytes===row.bytes&&p.sha256===row.sha256,`source changed ${suffix}`);inputPins.push(p);
}
for(const row of frozen.dataPins){const p=await pin(row.file);assert(p.bytes===row.bytes&&p.sha256===row.sha256,`frozen data changed ${row.file}`);}
const nav=await import(pathToFileURL(path.join(project,imported[0])));
const protection=await import(pathToFileURL(path.join(project,imported[1])));
const supplies=await import(pathToFileURL(path.join(project,imported[2])));
const spec=await protection.loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
const map=await nav.loadProtocolCollisionMap('0');inputPins.push(await pin(map.sourcePath));
inputPins.push(...Object.values(spec.sourceManifests).map(row=>({file:path.join(project,row.path),bytes:row.bytes,sha256:row.sha256})));
const reportFile=path.join(PACKAGE,'episode/reports',`${STEM}.preparation-report.json`);
const samplesFile=path.join(PACKAGE,'episode/reports',`${STEM}.preparation.jsonl`);
const report=await input(reportFile);inputPins.push(await pin(samplesFile));
assert(report.failedActor?.name==='L370e2a16'&&report.preparedCount===42,'unexpected actual W15 failure');
let failed=null;const transforms=[];
for await(const line of createInterface({input:createReadStream(samplesFile),crlfDelay:Infinity})){
  const row=JSON.parse(line);if(row.actor!==report.failedActor.name)continue;
  if(row.type==='ordinaryPreparation')failed=row.result;
  if(row.type==='selfTransform')transforms.push({sequence:row.sequence,at:row.at,monotonicMs:row.monotonicMs,position:row.after.position});
}
assert(failed?.fundingApproaches?.length===8,'missing actual eight approaches');
const offsetValues=transforms.map(r=>r.at-r.monotonicMs).sort((a,b)=>a-b);
const offset=offsetValues[Math.floor(offsetValues.length/2)];
const chosen=failed.fundingApproaches.filter(a=>a.reason==='missing'||a.objectId===206320);
assert(chosen.length===5,'unexpected projection cases');
const times=chosen.map(a=>a.monotonicMs+offset);
const begin=Math.min(...times)-20_000,end=Math.max(...times)+1_000;
const recordingFile=path.join(PACKAGE,'episode/recordings/0-498079.jsonl');inputPins.push(await pin(recordingFile));
const frames=[];
function publicEntity(e){
  return {objectId:e.objectId,kind:e.kind,...(['monster','npc'].includes(e.kind)?{name:e.name}:{}),x:e.x,y:e.y,
    hp:e.hp,dead:e.dead,ai:e.ai,disposition:e.disposition,ownerIsNull:e.ownerName==null,classIsNull:e.class==null};
}
for await(const line of createInterface({input:createReadStream(recordingFile),crlfDelay:Infinity})){
  const row=JSON.parse(line);if(row.capturedAtMs<begin)continue;if(row.capturedAtMs>end)break;
  frames.push({sequence:row.sequence,at:row.capturedAtMs,entities:row.world.entities.map(publicEntity),
    transfers:row.world.mapTransfers.map(t=>({bounds:t.bounds,mapFileName:t.mapFileName,toMapFileName:t.toMapFileName}))});
}
assert(frames.length>0&&frames.length<1_000,'unexpected bounded recording window');
const distance=(a,b)=>Math.max(Math.abs(a.x-b.x),Math.abs(a.y-b.y));
const key=p=>`${p.x},${p.y}`;
function normal(e){return e.kind==='monster'&&e.disposition==='hostile'&&e.ai===0&&e.dead===false&&e.ownerIsNull&&e.classIsNull&&spec.ai0MonsterNames.includes(e.name);}
function hostile(e){return e.kind==='monster'&&e.disposition==='hostile'&&e.dead!==true;}
function full(e){return {...e,ownerName:null,class:null};}
function enters(from,cell,p,g){
  const physical=distance(cell,p);if(physical===0)return true;
  const metric=q=>Math.max(Math.abs(q.x-p.x)*g.y,Math.abs(q.y-p.y)*g.x);
  const before=metric(from),next=metric(cell);
  return before<=g.x*g.y?next<before||physical<distance(from,p):next<=g.x*g.y;
}
function halos(points,start,forceSquare=false){
  const result=new Map();
  for(const e of points){
    const qualified=!forceSquare&&normal(e),g=supplies.ordinaryPreparationClearanceGeometry({qualifiedNormalAi0:qualified});
    for(let y=Math.max(0,e.y-8);y<=e.y+8;y++)for(let x=Math.max(0,e.x-8);x<=e.x+8;x++){
      const c={x,y};if(qualified&&distance(c,e)!==0&&protection.isProtectedOrdinaryCell(spec,'0',full(e),c))continue;
      if(enters(start,c,e,g))result.set(key(c),c);
    }
  }return [...result.values()];
}
function ring(target){const r=[];for(let y=target.y-17;y<=target.y+17;y++)for(let x=target.x-17;x<=target.x+17;x++)if(distance({x,y},target)===17&&x>=0&&y>=0)r.push({x,y});return r;}
function transfers(frame){return frame.transfers.flatMap(t=>{const r=[];for(let y=t.bounds.minY;y<=t.bounds.maxY;y++)for(let x=t.bounds.minX;x<=t.bounds.maxX;x++)r.push({x,y});return r;});}
function pathSummary(start,target,obstacles,confined){
  const blocked=confined?[...obstacles,...ring(target)]:obstacles;
  const p=nav.planProtocolNavigation({map,start,target,desiredDistance:1,dynamicObstacles:blocked});
  if(!p)return {found:false,aoi16Confined:confined,interpretation:'No path under this stated projection; not proof of actual historical impossibility.'};
  const maxDistance=Math.max(...p.path.map(c=>distance(c,target)));
  assert(!confined||maxDistance<=16,'AOI boundary escaped');
  assert(p.steps.length<512,'unexpected path evidence size');
  return {found:true,aoi16Confined:confined,steps:p.steps.length,maxTargetDistance:maxDistance,
    path:p.path,staticWalkabilityVerified:p.path.every(c=>nav.protocolMapCellIsWalkable(map,c))};
}
const results=[];
for(const a of chosen){
  const selectedWall=a.monotonicMs+offset;
  const nearest=frames.reduce((best,f)=>Math.abs(f.at-selectedWall)<Math.abs(best.at-selectedWall)?f:best);
  const prior=frames.filter(f=>f.at<=selectedWall&&f.at>selectedWall-20_000);
  const occupied=nearest.entities.filter(e=>e.objectId!==failed.before.ownerId&&e.dead!==true).map(e=>({objectId:e.objectId,kind:e.kind,x:e.x,y:e.y}));
  const current=nearest.entities.filter(hostile),union=new Map();
  for(const f of prior)for(const e of f.entities.filter(hostile))union.set(`${e.objectId}:${key(e)}`,{...e,lastSampleAt:f.at});
  // A nearest frame may be slightly later than the selected receipt. It is a
  // separate sampled projection, never an inferred exact client planning view.
  const transferCells=transfers(nearest),staticBase=transferCells;
  const project=(label,obs,scope)=>({label,scope,obstacleCells:new Set(obs.map(key)).size,
    unconstrained:pathSummary(a.from,a.target,obs,false),aoi16:pathSummary(a.from,a.target,obs,true)});
  const projections=[project('static-collision-and-authored-transfers',staticBase,'No dynamic occupancy or threat claim.'),
    project('sampled-current-occupancy', [...staticBase,...occupied],'Nearest spectator frame only, not per-client AOI or an exact receipt.'),
    project('sampled-current-normal-ai0', [...staticBase,...occupied,...halos(current,a.from)],'Source-valid normal AI0 8x6 plus protected-cell exemption; unknown/other hostile 8x8. Qualification availability in actual client is not inferred.'),
    project('sampled-prior20s-union', [...staticBase,...occupied,...halos([...union.values()],a.from)],'Conservative union over sampled points only. Not complete client history: unsampled points/identity changes/receipt clocks may differ.'),
    project('sampled-prior20s-all-square-failclosed', [...staticBase,...occupied,...halos([...union.values()],a.from,true)],'All sampled hostiles unqualified: original 8x8, no safe-cell exemption. Conservative over these samples, not guaranteed superset of unrecorded client memory.')];
  const last=transforms.filter(t=>t.sequence<=a.endedSequence).at(-1);
  results.push({objectId:a.objectId,actualResult:a.reason,start:a.from,target:a.target,sequence:a.sequence,observedRawSequence:a.observedSnapshotSequence,
    selectedMonotonicMs:a.monotonicMs,estimatedWallMs:selectedWall,nearestSample:{frame:nearest.sequence,at:nearest.at,deltaMs:nearest.at-selectedWall},
    selectedChebyshevDistance:distance(a.from,a.target),actualLastOwner:last?.position,actualLastDistance:distance(last?.position??a.from,a.target),
    exactHistoricalPlannerViewAvailable:a.objectId===206320,earlierCasesAreNotExactReplays:a.objectId!==206320,
    sampledOccupancy:occupied,sampledCurrentThreats:current,sampled20sThreatPoints:[...union.values()],transferCells,projections});
}
// Last failure carries the complete shared 7-point view and 27 occupancy.
// Reconstruct all 604 explicit cells from the pinned helper algorithm, rather
// than pretend the published 128-cell truncated prefix was complete.
const actual=failed.navigationNoPath,shared=actual.shared;
assert(actual.target.x===281&&actual.target.y===596&&!shared.hostileMemoryTruncated&&!shared.occupancyTruncated,'unexpected exact view');
const memory=shared.hostileMemory.map(p=>{
  const q=actual.qualifications.find(q=>q.objectId===p.objectId);
  assert(p.allowed===true&&q&&q.qualification==='raw'&&q.blockedRemainingMs===0&&q.positionCount===1,'unsupported exact memory qualification');
  return {...p,kind:'monster',name:q.current?.name??spec.ai0MonsterNames.find(n=>n.toLowerCase()===p.name),ai:0,dead:false,disposition:'hostile',ownerIsNull:true,classIsNull:true};
});
assert(memory.every(e=>spec.ai0MonsterNames.includes(e.name)),'unknown exact normal name');
const explicit=halos(memory,actual.from),explicitKeys=new Set(explicit.map(key));
assert(explicit.length===shared.explicitForbiddenTotal&&shared.explicitForbidden.every(p=>explicitKeys.has(key(p))),'explicit reconstruction disagrees with actual count/prefix');
const lastCase=results.find(r=>r.objectId===206320);
assert(lastCase.transferCells.length===69&&explicit.length+shared.occupancy.length+69===shared.obstacleCount,'exact dynamic total mismatch');
const exactObstacles=[...explicit,...shared.occupancy,...lastCase.transferCells];
const rangeOne=[];for(let y=actual.target.y-1;y<=actual.target.y+1;y++)for(let x=actual.target.x-1;x<=actual.target.x+1;x++){
  const cell={x,y},blockers=memory.filter(e=>halos([e],actual.from).some(p=>key(p)===key(cell))).map(e=>e.objectId);
  rangeOne.push({...cell,staticWalkable:nav.protocolMapCellIsWalkable(map,{x,y}),explicitForbidden:explicitKeys.has(key(cell)),blockerIds:blockers});
}
const exact={source:'complete-shared-memory-plus-pinned-helper-reconstruction',notAnEarlierView:true,map:actual.shared.mapFileName,
  start:actual.from,target:actual.target,sequence:actual.sequence,rawSequence:actual.observedRawSequence,
  sharedMemory:memory,occupancy:shared.occupancy,explicitReconstructedTotal:explicit.length,publishedPrefixMatches:true,
  explicitOriginalTotal:shared.explicitForbiddenTotal,originalPrefixTruncated:shared.explicitForbiddenTruncated,transfers:69,dynamicArrayTotal:exactObstacles.length,
  transferProvenance:'Nearest spectator frame authored transfer list; frozen map/data source verified. No per-client transfer visibility inference.',
  rangeOne,allRangeOneExplicitForbidden:rangeOne.every(c=>c.explicitForbidden),unconstrained:pathSummary(actual.from,actual.target,exactObstacles,false),
  aoi16:pathSummary(actual.from,actual.target,exactObstacles,true),noMemoryOrTtlRemoved:true};
const output={schemaVersion:1,createdAt:new Date().toISOString(),diagnosticOnly:true,liveRunPerformed:false,productionSourceModified:false,
  actualEpisode:{packageId:frozen.packageId,driverCommit:frozen.driver.driverSourceCommit,prepared:42,failedActorOrdinal:43,originalFailure:report.error},
  scope:'Offline path queries, no commands, no memory mutation, no TTL expiry/removal, no changed budgets or qualification.',
  timing:{basis:'median at-minus-monotonic of actual public selfTransform rows',offset,offsetMin:offsetValues[0],offsetMax:offsetValues.at(-1),spectatorClockIsNotClientReceipt:true},
  map:{width:map.width,height:map.height,doorsBlocked:true,sourcePath:map.sourcePath},sourcePins:inputPins,
  scriptPin:await pin(fileURLToPath(import.meta.url)),boundedRecording:{file:recordingFile,startMs:begin,endMs:end,frames:frames.length,playerNamesCopied:false,credentialsRead:false},
  coverageLimits:['Earlier full helper/shared planning views were not retained. Their projections cannot prove actual NoPath or actual counterfactual success.',
    'Spectator sampling is not client receipt history. A union of sampled points may omit intervening positions or conflicts; all-square variant only conservatively covers its enumerated samples.',
    'A found path is legal under its frozen projection only. Later moving actors, wire identity loss, unknown obstacles, combat retained/forced effects and budgets remain real guards.',
    'AOI16 ring is a target-position-local planning constraint, not target liveness, safety, attack or preparation readiness proof.'],
  cases:results,lastNoPathReconstruction:exact};
const out=path.join(path.dirname(fileURLToPath(import.meta.url)),'geometry-result-v1.json');
await writeFile(out,JSON.stringify(output,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output:await pin(out),script:output.scriptPin,summary:results.map(c=>({id:c.objectId,modes:c.projections.map(p=>({label:p.label,free:p.unconstrained.found,confined:p.aoi16.found,steps:p.aoi16.steps}))})),exact:{explicit:explicit.length,rangeOneBlocked:exact.allRangeOneExplicitForbidden,free:exact.unconstrained.found,confined:exact.aoi16.found}},null,2));
