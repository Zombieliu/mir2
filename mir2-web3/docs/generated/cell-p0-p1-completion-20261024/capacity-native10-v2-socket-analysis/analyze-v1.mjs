import fs from 'node:fs';
import path from 'node:path';
import readline from 'node:readline';
import crypto from 'node:crypto';
import {pathToFileURL} from 'node:url';
const base='C:/mir2-build/p01-completion-20261024',runRoot=path.join(base,'release-native-10-v2-socket-feasibility-08'),out=path.join(base,'probe08-analysis');
const repo='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const runId='1792885245847-0368d2eb',actor='La5189302',fighter='La5189305';
const pin=file=>{const bytes=fs.readFileSync(file);return{file,bytes:bytes.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')}};
const reportFile=path.join(runRoot,'reports',runId+'.report.json'),rawFile=path.join(runRoot,'reports',runId+'.samples.jsonl'),report=JSON.parse(fs.readFileSync(reportFile));
const counts={},blocked=[],nav=[],actions=[],transforms=[],lifecycle=[],attacks=[],traces=[],coverage=[],monitors=[],windows=[],controls=[],corrections=[];
const hash=crypto.createHash('sha256');let rawBytes=0,rows=0;const input=fs.createReadStream(rawFile);input.on('data',b=>{rawBytes+=b.length;hash.update(b)});
const lo=1792886269000,hi=1792886289000;
for await(const line of readline.createInterface({input,crlfDelay:Infinity})){
 const row=JSON.parse(line),entry={line:++rows,...row};counts[row.type]=(counts[row.type]??0)+1;
 if(row.type==='navigationBlocked'&&row.actor===actor)nav.push(entry);
 if(row.type==='action'){
  if(row.actor===actor&&row.status==='navigation-blocked')blocked.push(entry);
  if([actor,fighter].includes(row.actor)&&row.at>=lo&&row.at<=hi)actions.push(entry);
  if(row.kind==='attack'&&row.targetId===206636&&row.at>=lo&&row.at<=hi)attacks.push(entry);
 }
 if(row.type==='pendingAttackEvidence'&&row.target?.objectId===206636&&row.at>=lo&&row.at<=hi)traces.push(entry);
 if(row.type==='selfTransform'&&[actor,fighter].includes(row.actor)&&row.at>=lo&&row.at<=hi)transforms.push(entry);
 if(row.type==='playerLifecycle'&&row.actor===actor&&row.at>=lo-5000&&row.at<=hi)lifecycle.push(entry);
 if(row.type==='movementCorrection'&&row.actor===actor)corrections.push(entry);
 if(row.type==='coverage'&&row.at>=lo&&row.at<=hi)coverage.push(entry);
 if(row.type==='monitor'&&row.sample.sampledAt>=lo&&row.sample.sampledAt<=hi)monitors.push(entry);
 if(row.type==='continuousWindow'&&row.actor===actor)windows.push(entry);
 if(['phaseStarted','phaseEnded','continuousFailure','safetyStop','finished'].includes(row.type))controls.push(entry);
}
const rawPin={file:rawFile,bytes:rawBytes,sha256:hash.digest('hex'),rows},failure=windows.find(r=>!r.passed),prior=windows[windows.indexOf(failure)-1];
if(blocked.length!==12||nav.length!==1||failure?.activity?.nativeCompleted!==68||corrections.length!==0)throw Error('Unexpected08 diagnostic shape');
const navigation=await import(pathToFileURL(path.join(repo,'apps/web/scripts/quest-agent/protocol-navigation.mjs')));
const map=await navigation.loadProtocolCollisionMap('0',{packagedMapRoot:path.join(repo,'apps/web/lib/generated/crystal-map-pack')});
const origin=nav[0].self,occupants=nav[0].nearbyOccupants,occupied=new Set(occupants.map(p=>p.x+','+p.y));
const steps=[[0,-1,'Up'],[1,-1,'UpRight'],[1,0,'Right'],[1,1,'DownRight'],[0,1,'Down'],[-1,1,'DownLeft'],[-1,0,'Left'],[-1,-1,'UpLeft']];
const clear=p=>navigation.protocolMapCellIsWalkable(map,p)&&!occupied.has(p.x+','+p.y);
const neighbors=steps.map(([dx,dy,direction])=>{const target={x:origin.x+dx,y:origin.y+dy},sides=dx&&dy?[{x:origin.x+dx,y:origin.y},{x:origin.x,y:origin.y+dy}]:[];
 return{direction,target,staticWalkable:navigation.protocolMapCellIsWalkable(map,target),occupied:occupied.has(target.x+','+target.y),diagonalSides:sides.map(p=>({...p,staticWalkable:navigation.protocolMapCellIsWalkable(map,p),occupied:occupied.has(p.x+','+p.y)})),legalFirstStep:clear(target)&&sides.every(clear)};});
const goals=nav[0].attempts.map(a=>a.target),routes=goals.map(target=>({target,originalLoggedReason:nav[0].attempts.find(a=>a.target.x===target.x&&a.target.y===target.y).reason,
 routeWithObservedTwoBlockers:navigation.findProtocolWalkPath({map,start:origin,target,dynamicObstacles:occupants,maxExpanded:4000}),
 routeWithoutDynamicBlockers:navigation.findProtocolWalkPath({map,start:origin,target,dynamicObstacles:[],maxExpanded:4000})}));
if(neighbors.some(n=>n.legalFirstStep)||routes.some(r=>r.routeWithObservedTwoBlockers!==null))throw Error('First blocked snapshot is not an actual zero-exit geometry');
const slowFile=path.join(runRoot,'slow-stage-public.jsonl'),slow=fs.readFileSync(slowFile,'utf8').trim().split(/\r?\n/).filter(Boolean).map((s,i)=>({line:i+1,...JSON.parse(s)}));
const phase=at=>report.phaseWindows.find(p=>at>=p.startedAtMs&&at<=p.endedAtMs)?.name??'outside-or-drain';
const slowGroups={};for(const r of slow){const key=phase(r.observedAtMs)+'|'+r.stage;(slowGroups[key]??=[]).push(r.durationMs);}
const census=Object.entries(slowGroups).map(([group,values])=>({group,count:values.length,minMs:Math.min(...values),maxMs:Math.max(...values),atLeast500:values.filter(v=>v>=500).length}));
const slowNear=slow.filter(r=>r.observedAtMs>=lo&&r.observedAtMs<=hi),queueRows=slow.filter(r=>r.stage==='socket_input.queue_residence');
const deathPackets=traces.flatMap(t=>t.packets.filter(p=>p.packet==='ObjectDied').map(p=>({traceLine:t.line,requestId:t.requestId,...p})));
const beforeBlocked=actions.filter(r=>r.actor===actor&&r.at<blocked[0].at),firstRecovery=actions.find(r=>r.actor===actor&&r.kind==='walk'&&r.status==='success'&&r.at>blocked.at(-1).at);
const selected={schemaVersion:1,runId,actor,scope:'Bounded public08 raw projections. No private/account data or recording payload read.',
 inputPins:{report:pin(reportFile),raw:rawPin,slowStage:pin(slowFile),standaloneMonitor:pin(path.join(runRoot,'monitor-history.jsonl')),cleanup:pin(path.join(runRoot,'realm-cleanup.json')),sourcePins:pin(path.join(runRoot,'source-pins-admitted-v2.json'))},
 originalNavigationBlocked:nav[0],all12BlockedActions:blocked,nearbyOwnerActions:actions,ownerTransforms:transforms,ownPlayerLifecycle:lifecycle,
 actor02MovementCorrectionsWholeRun:corrections,pairedTargetDamageTraces:traces,originalFailedWindow:failure,precedingSameActorWindow:prior,
 controls,driverCoverage:coverage,nearbyRawMonitor:monitors,slowStageRecordsAll98:slow,
 firstBlockedMapProof:{map:pin(map.sourcePath),planner:pin(path.join(repo,'apps/web/scripts/quest-agent/protocol-navigation.mjs')),origin,observedOccupants:occupants,neighbors,legalExitCount:0,originalCornerRoutes:routes,
  limitation:'Only the first blocked opportunity has a complete untruncated local navigation/AOI diagnostic. Subsequent action rows do not retain all per-decision obstacle unions. The geometry proof establishes the first no-path, not a reconstructed snapshot for each later opportunity.'}};
const selectedFile=path.join(out,'selected-events-v1.json');fs.writeFileSync(selectedFile,JSON.stringify(selected,null,2)+'\n',{flag:'wx'});
const summary={schemaVersion:1,runId,actor,inputPins:selected.inputPins,selectedEvents:pin(selectedFile),rawTypeCounts:counts,
 verdict:{ok:false,originalFailurePreserved:true,completedStages:report.stages.map(s=>({name:s.name,passed:s.passed,seconds:s.seconds})),soak:report.phaseWindows.find(p=>p.name==='soak'),
  continuousFailureCount:report.continuousAcceptance.failureCount,failedWindow:{actor:failure.actor,seconds:failure.seconds,final:failure.final,at:failure.at,previousSameActorWindowAt:prior.at,
   opportunities:80,completed:68,completionRatio:.85,gaps:12,moves:65,runs:59,runRatio:59/65,cells:124,activityPassed:false,latencyPassed:true,fanoutPassed:true,timeoutRatio:failure.timeoutRatio,correctionRatio:failure.correctionRatio}},
 blockedEpisode:{count:blocked.length,firstAt:blocked[0].at,lastAt:blocked.at(-1).at,firstToLastMs:blocked.at(-1).at-blocked[0].at,firstRecoveredAt:firstRecovery.at,
  firstToRecoveredMs:firstRecovery.at-blocked[0].at,allWithinFailedWindow:blocked.every(r=>r.at>prior.at&&r.at<=failure.at),
  preparedSendTimeConflictRows:blocked.filter(r=>r.navigationCoordinationConflict===true).length,
  allBlockedRowsHaveNoSentTimestamp:blocked.every(r=>r.sentAtMs===undefined),actor02MovementCorrectionRowsWholeRun:corrections.length},
 exactPlanningBranch:{firstNavigationDiagnosticLine:nav[0].line,origin,home:nav[0].home,targetCursor:nav[0].targetCursor,
  attempts:nav[0].attempts,attemptsTruncated:nav[0].attemptsTruncated,nearbyOccupants:occupants,occupantsTruncated:nav[0].occupantsTruncated,
  legalExitCount:0,explanation:'Static north and west neighbours are blocked. East contains the observed live Scarecrow; south contains the observed player. Every diagonal fails its exact side-cell rule. Thus there is no legal first step even before adding any hints or rejected cells. The primed fast path cannot send, all three non-self original corner A* routes return null, and the own-corner candidate is skipped. Native patrol returns null and records the original navigation-blocked offered opportunity. The first A* failure is true no-path, not exhaustion of4000 nodes.',
  scope:'The complete first snapshot proves zero exits. The other11 entries are original null-plan opportunities in the same episode; later owner/attack/death records support the same blockers remaining, but no exact full obstacle set for each of those11 decisions is claimed.'},
 coordinationAndAvoidance:{originalAoiPlayer:occupants.find(p=>p.kind==='player'),actualControlledOwnerArrival:transforms.find(t=>t.actor===fighter&&t.after.position.x===237&&t.after.position.y===666),
  whyHintNotRequired:'The player was already in the blocked actor original AOI and its own real UserLocation confirms237666 before the episode. Additional conservative hints could add obstacles but are unnecessary to establish this first zero-exit condition. None of12 action rows is the prepared-plan synchronous conflict branch.',
  avoidance:'No actor02 movementCorrection record appears anywhere in the raw run. No per-decision full hint/avoidance map is logged; absence of a hidden map is not asserted, and none is needed for the first geometry proof.'},
 recovery:{lastPreBlockActor02Movement:beforeBlocked.findLast(r=>['walk','run'].includes(r.kind)),
  damageSequence:attacks.map(r=>({line:r.line,at:r.at,requestId:r.requestId,targetId:r.targetId,damage:r.damage,healthPercent:r.healthPercent,sharedHealthVerified:r.sharedHealthVerified,status:r.status})),
  pairedDeathPackets:deathPackets,firstRecovery,
  laterFighterDeparture:transforms.find(t=>t.actor===fighter&&t.at>deathPackets[0].atMs&&!(t.after.position.x===237&&t.after.position.y===666)),
  interpretation:'Six verified hits produce20 damage and paired HP0/ObjectDied at238665. Actor02 enters that exact cell294ms later, before the player moves off237666. This is consistent with the monster death opening the east exit; original action and raw packet receipts are retained, not synthesized.'},
 slowStage:{pin:pin(slowFile),rows:slow.length,census,queueResidenceRows:queueRows,recordsNearBlockedEpisode:slowNear,
  phaseAndTimeScope:'ObservedAt is launcher receive time. All98 rows were counted. There are zero socket_input.queue_residence records and none near this blocked episode; this does not prove all server work was free of latency or identify why another run stalled.'},
 driver:{samplesNearEpisode:coverage.length,maxP99Ms:Math.max(...coverage.map(r=>r.driver.eventLoopLagP99Ms)),maxMs:Math.max(...coverage.map(r=>r.driver.eventLoopLagMaxMs)),originalThresholdMs:250},
 nextStep:{minimumReasonableChange:'Do not change the planner, occupancy, completion denominator or95% gate to turn legal body-blocking into a success. Select and freeze a scene whose full patrol routes are statically open with alternative exits, prove original role prefixes and actual observer combat coverage, then rerun a fresh gate.',
  span4:'Changing only span2 to4 at237665 cannot remove the first-step trap. The existing span4 proposal relocates this actor to241666 and changes many other homes, so it is a new scene rather than a repair of this result. An independent per-rectangle/route/hazard/AOI comparison is separate and must not be mistaken for dynamic occupancy acceptance.'},
 limitations:['Only the first blocked opportunity has the complete bounded local AOI/planning diagnostic.','No spectator frames were read; existing actual AOI, owner receipts and paired death packets were sufficient.','Dynamic player/monster body-blocking is valid authoritative server behavior. This failure is insufficient offered/completed workload, not proof of overload or a movement correction bug.','No model here guarantees100 players, future monster wander, two-hour supply or production capacity.'],
 mutations:{sourceChanged:false,thresholdChanged:false,sceneChanged:false,testStarted:false,realmStarted:false,capacityAccepted:false}};
const summaryFile=path.join(out,'summary-v1.json');fs.writeFileSync(summaryFile,JSON.stringify(summary,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({summary:pin(summaryFile),selected:pin(selectedFile),raw:rawPin,blocked:summary.blockedEpisode,death:deathPackets.map(p=>({receiver:p.receiver,atMs:p.atMs,objectId:p.payload.objectId})),slow:{rows:slow.length,queueRows:queueRows.length,near:slowNear.length},driver:summary.driver}));
