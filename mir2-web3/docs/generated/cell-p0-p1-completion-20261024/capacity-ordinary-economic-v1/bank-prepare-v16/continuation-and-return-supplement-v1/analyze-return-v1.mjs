import{readFile,writeFile,stat}from'node:fs/promises';import{createReadStream}from'node:fs';import{createInterface}from'node:readline';import{createHash}from'node:crypto';import{pathToFileURL,fileURLToPath}from'node:url';import path from'node:path';
const DIRECTORY=path.dirname(fileURLToPath(import.meta.url)),PACKAGE=path.dirname(DIRECTORY),assert=(v,m)=>{if(!v)throw Error(m)},hash=b=>createHash('sha256').update(b).digest('hex');
const pin=async file=>{const b=await readFile(file);return{file,bytes:b.length,sha256:hash(b)}},read=async f=>JSON.parse(await readFile(f,'utf8'));
const frozenFile=path.join(PACKAGE,'frozen-source-v1.json'),frozen=await read(frozenFile),repo=frozen.driver.repository,project=path.join(repo,'mir2-web3');
const inputs=[await pin(frozenFile)];
for(const suffix of ['protocol-navigation.mjs','capacity-ordinary-supplies.mjs','capacity-ordinary-protection.mjs']){
 const old=frozen.driver.files.find(r=>r.path.endsWith('/'+suffix));assert(old,'missing source');const p=await pin(path.join(repo,old.path));assert(p.bytes===old.bytes&&p.sha256===old.sha256,'source differs');inputs.push(p);
}
const nav=await import(pathToFileURL(inputs.find(p=>p.file.endsWith('protocol-navigation.mjs')).file));
const supplies=await import(pathToFileURL(inputs.find(p=>p.file.endsWith('capacity-ordinary-supplies.mjs')).file));
const protection=await import(pathToFileURL(inputs.find(p=>p.file.endsWith('capacity-ordinary-protection.mjs')).file));
const map=await nav.loadProtocolCollisionMap('0'),spec=await protection.loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
for(const f of [map.sourcePath,...Object.values(spec.sourceManifests).map(p=>path.join(project,p.path))]){
 const old=frozen.dataPins.find(p=>path.resolve(p.file).toLowerCase()===path.resolve(f).toLowerCase()),p=await pin(f);assert(old&&old.bytes===p.bytes&&old.sha256===p.sha256,'data differs');inputs.push(p);
}
const stem='1793090167771-5bd2d680',reportFile=path.join(PACKAGE,'episode/reports',stem+'.preparation-report.json'),samplesFile=path.join(PACKAGE,'episode/reports',stem+'.preparation.jsonl');
inputs.push(await pin(reportFile),await pin(samplesFile));const report=await read(reportFile);assert(report.preparedCount===16&&report.historicalPreparedCount===42&&report.failedActor.characterIndex===59,'unexpected original outcome');
let status;for await(const line of createInterface({input:createReadStream(samplesFile),crlfDelay:Infinity})){const r=JSON.parse(line);if(r.type==='ordinaryPreparation'&&r.actor===report.failedActor.name)status=r.result;}
assert(status?.navigationNoPath?.state==='selling'&&status.soldVenison===2&&status.purchased===0&&status.goldEarned===521,'unexpected failed stage');
const actual=status.navigationNoPath,s=actual.shared;assert(!s.hostileMemoryTruncated&&!s.occupancyTruncated&&!actual.qualificationTruncated&&actual.globalQuarantineRemainingMs===0,'incomplete view');
assert(s.hostileMemoryTotal===11&&s.occupancyTotal===16&&s.hostileMemory.every(p=>p.allowed===true),'unexpected exact cache');
const manifestFile=Object.values(spec.sourceManifests).find(p=>p.path.startsWith('apps/web/')).path;
const metadata=(await read(path.join(project,manifestFile))).maps.find(m=>String(m.map_file_name)==='0');
// Authored manifest has72 declarations; the actual protocol emits69 active
// rows. Read one public recording line and retain transfer scalars only.
const recordingFile=path.join(PACKAGE,'episode/recordings/0-498081.jsonl');let transferFrame,firstLine;
for await(const line of createInterface({input:createReadStream(recordingFile),crlfDelay:Infinity})){firstLine=line;transferFrame=JSON.parse(line);break;}
inputs.push({file:recordingFile,sourceFileBytes:(await stat(recordingFile)).size,excerpt:'first UTF8 JSONL line excluding delimiter',excerptBytes:Buffer.byteLength(firstLine),excerptSha256:hash(Buffer.from(firstLine))});
const transfers=transferFrame.world.mapTransfers.flatMap(t=>{const out=[];for(let y=t.bounds.minY;y<=t.bounds.maxY;y++)for(let x=t.bounds.minX;x<=t.bounds.maxX;x++)out.push({x,y});return out;});
assert(transfers.length===69,'actual protocol transfer set differs');
const points=s.hostileMemory.map(p=>{const q=actual.qualifications.find(q=>q.objectId===p.objectId),name=q?.current?.name??spec.ai0MonsterNames.find(n=>n.toLowerCase()===p.name);
 assert(q&&q.positionCount===1&&['raw','wire'].includes(q.qualification)&&q.blockedRemainingMs===0&&spec.ai0MonsterNames.includes(name),'unsupported memory');
 return{objectId:p.objectId,name,x:p.x,y:p.y,remainingMs:p.remainingMs,currentlyVisible:p.currentlyVisible,live:q.live,
  historyRemainingMs:q.historyRemainingMs,qualification:q.qualification};});
const distance=(a,b)=>Math.max(Math.abs(a.x-b.x),Math.abs(a.y-b.y)),key=p=>p.x+','+p.y;
function explicit(points){const out=new Map();for(const p of points){const g=supplies.ordinaryPreparationClearanceGeometry({qualifiedNormalAi0:true}),metric=c=>Math.max(Math.abs(c.x-p.x)*g.y,Math.abs(c.y-p.y)*g.x),before=metric(actual.from),beforeD=distance(actual.from,p);
 for(let y=p.y-8;y<=p.y+8;y++)for(let x=p.x-8;x<=p.x+8;x++){const c={x,y},d=distance(c,p);
  if(d!==0&&spec.safeZones.some(z=>Math.abs(c.x-z.location.x)<=z.size&&Math.abs(c.y-z.location.y)<=z.size))continue;
  if(d===0||(before<=g.x*g.y?metric(c)<before||d<beforeD:metric(c)<=g.x*g.y))out.set(key(c),c);
 }}return[...out.values()];}
const all=explicit(points),allKeys=new Set(all.map(key));assert(all.length===s.explicitForbiddenTotal&&s.explicitForbidden.every(p=>allKeys.has(key(p))),'reconstructed forbidden cells differ');
assert(all.length+s.occupancyTotal+transfers.length===s.obstacleCount,'dynamic array count differs');
function plan(label,remaining){const forbidden=explicit(remaining),p=nav.planProtocolNavigation({map,start:actual.from,target:actual.target,desiredDistance:16,dynamicObstacles:[...forbidden,...s.occupancy,...transfers]});
 return{label,retainedIds:remaining.map(p=>p.objectId),explicitCount:forbidden.length,pathFound:!!p,steps:p?.steps.length??null,
  path:p?.path??null,endpoint:p?.path.at(-1)??null,endpointDistance:p?distance(p.path.at(-1),actual.target):null};}
const baseline=plan('actual-complete-retained-view',points);assert(!baseline.pathFound,'actual NoPath not reproduced');
const closed=points.filter(p=>!p.live&&p.currentlyVisible===false),live=points.filter(p=>p.live&&p.currentlyVisible===true);assert(closed.length===6&&live.length===5,'closed/live split differs');
const thresholds=[...new Set(closed.map(p=>p.remainingMs+1))].sort((a,b)=>a-b);
const naturalExpiryProjections=thresholds.map(elapsed=>({elapsedMs:elapsed,removedOnlyPreviouslyClosedIds:closed.filter(p=>p.remainingMs<elapsed).map(p=>p.objectId),
 ...plan('original-closed-ttl-naturally-elapsed-live-held-fixed',points.filter(p=>p.live||p.remainingMs>=elapsed))}));
const liveOnly=plan('all-original-closed-points-naturally-expired-live-occupancy-transfers-unchanged',live);
const staticPath=nav.planProtocolNavigation({map,start:actual.from,target:actual.target,desiredDistance:16,dynamicObstacles:transfers});
const result={schemaVersion:1,offlineOnly:true,commandsSent:0,productionSourceModified:false,inputs,scriptPin:await pin(fileURLToPath(import.meta.url)),
 originalOutcome:{historicalPrepared:42,currentPrepared:16,combinedSaved:58,originalFailedOrdinal:59,originalError:report.error,after:null,readiness:null},
 actualPhase:{diagnosticState:actual.state,phase:'After completed John sale, navigating to Ruben for a fresh quote before purchase',
  callChain:['runPreparationBank -> prepareCapacityActor -> ordinarySupplies.prepare','collectSafeFundingVenison completed:2hunts/4attempts','refresh -> sellUntil(requiredGold) -> shop(JOHN,true) ->2exact sales','goods=await shop(RUBEN) -> navigate(RUBEN,16)','shared createNavigator terminal NoPath -> helper navigate catch','waitForClosedPlannerHistory refuses state selling -> original NoPath rethrown'],
  nextStateAssignmentNotReached:'state=buying occurs only after awaited shop(RUBEN) returns',
  soldVenison:status.soldVenison,goldEarned:status.goldEarned,purchased:status.purchased,proofs:status.proofs,capitalLast:status.fundingCapitalProofs.at(-1),
  commands:status.commands,movementCommands:status.movementCommands,commandCounts:status.commandCounts,elapsedMs:status.elapsedMs,pathReplans:status.pathReplans.length},
 exactPlanningProjection:{sequence:actual.sequence,rawSequence:actual.observedRawSequence,from:actual.from,target:actual.target,range:16,successfulSteps:actual.successfulSteps,attempts:actual.attempts,
  shared11Points:points,occupancy:s.occupancy,transfers,explicitReconstructedTotal:all.length,originalExplicitTotal:s.explicitForbiddenTotal,
  allPublishedPrefixCellsMatched:s.explicitForbidden.length,originalExplicitPrefixTruncated:s.explicitForbiddenTruncated,dynamicArrayTotal:s.obstacleCount,
  noUnknownOrDisallowedSharedPoints:true,globalQuarantine:0,baseline,staticOnly:{found:!!staticPath,steps:staticPath?.steps.length??null},naturalExpiryProjections,liveOnly},
 limitations:['This reconstructs only the final actual planning view, not earlier route states or exact receipt replay.',
  'Transfers come from one earlier actual public recording frame (69 active protocol rows), not all72 authored declarations. Only first-line bytes were inspected, and only transfer coordinates retained.',
  'Natural-expiry paths are offline counterfactuals with five current live positions and all16occupancy held fixed. Real later movement/newAOI can change the result.',
  'Shared relative TTLs are diagnostic only. Production waits must use existing helper-owned trail expiresAt and row identity, never regenerate/extend trust or expiry from this JSON.',
  'All11points were already raw/wire qualified in the actual helper view. Static safe-zone membership is a geometry calculation from the pinned spec; no new raw DTO or wire certificate was synthesized.',
  'No cache was cleared and no live episode or test was run. Original failure and absent final purchase/readiness remain unchanged.']};
const out=path.join(DIRECTORY,'geometry-result-v1.json');await writeFile(out,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output:await pin(out),script:result.scriptPin,actual:{explicit:all.length,dynamicTotal:s.obstacleCount,closed:closed.length,live:live.length},
 baseline:baseline.pathFound,naturalExpiry:naturalExpiryProjections.map(p=>({elapsedMs:p.elapsedMs,pathFound:p.pathFound,steps:p.steps,removed:p.removedOnlyPreviouslyClosedIds})),liveOnly:liveOnly.pathFound},null,2));
