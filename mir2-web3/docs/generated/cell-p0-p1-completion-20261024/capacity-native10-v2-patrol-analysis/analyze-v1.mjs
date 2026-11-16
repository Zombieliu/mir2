import fs from 'node:fs';
import path from 'node:path';
import readline from 'node:readline';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
const base='C:/mir2-build/p01-completion-20261024', runRoot=path.join(base,'release-native-10-v2-patrol-feasibility-07'), out=path.join(base,'probe07-analysis');
const project='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const {zoneMetricsDelta}=await import(pathToFileURL(path.join(project,'apps/web/scripts/playtest-capacity-telemetry.mjs')));
const runId='1792878747018-da931241', actor='La809f605';
const reportFile=path.join(runRoot,'reports',runId+'.report.json'), rawFile=path.join(runRoot,'reports',runId+'.samples.jsonl');
const report=JSON.parse(fs.readFileSync(reportFile)), cleanupFile=path.join(runRoot,'realm-cleanup.json'), cleanup=JSON.parse(fs.readFileSync(cleanupFile));
const pin=file=>{const b=fs.readFileSync(file);return{file,bytes:b.length,sha256:crypto.createHash('sha256').update(b).digest('hex')}};
const lo=1792879770000, hi=1792879785000, failureStatuses=new Set(['timeout','miss','error','transportError','rejected']);
const counts={}, actions=[], failed=[], traces=[], transforms=[], coverage=[], monitors=[], controls=[], windows=[];
const hash=crypto.createHash('sha256');let rawBytes=0,rows=0;
const stream=fs.createReadStream(rawFile);stream.on('data',b=>{rawBytes+=b.length;hash.update(b)});
for await(const line of readline.createInterface({input:stream,crlfDelay:Infinity})){
  const row=JSON.parse(line), entry={line:++rows,...row};counts[row.type]=(counts[row.type]??0)+1;
  if(row.type==='action'){
    if(failureStatuses.has(row.status))failed.push(entry);
    if(row.actor===actor&&(row.at>=lo&&row.at<=hi))actions.push(entry);
  }
  if(row.type==='pendingAttackEvidence'&&(failureStatuses.has(row.outcome)||(row.actor===actor&&row.at>=lo&&row.at<=hi)))traces.push(entry);
  if(row.type==='selfTransform'&&row.actor===actor&&row.at>=lo&&row.at<=hi)transforms.push(entry);
  if(row.type==='coverage')coverage.push(entry);
  if(row.type==='monitor')monitors.push(entry);
  if(row.type==='continuousWindow')windows.push(entry);
  if(['phaseStarted','phaseEnded','continuousFailure','safetyStop','finished'].includes(row.type))controls.push(entry);
}
const rawPin={file:rawFile,bytes:rawBytes,sha256:hash.digest('hex'),rows};
const slowFile=path.join(runRoot,'slow-stage-public.jsonl'), slowBytes=fs.readFileSync(slowFile);
const phase=at=>report.phaseWindows.find(p=>at>=p.startedAtMs&&at<=p.endedAtMs)?.name??(at<report.phaseWindows[0].startedAtMs?'startup-before-driver':'after-soak-drain');
const slow=slowBytes.toString('utf8').trim().split(/\r?\n/).filter(Boolean).map((l,i)=>{const r=JSON.parse(l);return{line:i+1,...r,receiverPhase:phase(r.observedAtMs),estimatedSpanStartIfReceivedImmediatelyMs:r.observedAtMs-r.durationMs}});
const selectedSlow=slow.filter(r=>r.observedAtMs>=lo&&r.observedAtMs<=hi);
const driver=coverage.map(r=>({line:r.line,at:r.at,phase:r.phase,...r.driver}));
const driverCritical=driver.filter(r=>r.at>=lo&&r.at<=hi);
const failures=windows.filter(w=>!w.passed), firstFailure=report.continuousAcceptance.firstFailure.evidence;
if(failed.length!==2||failures.length!==1||failed.some(r=>r.actor!==actor)||firstFailure.activity.nativeCompleted!==46)throw new Error('Unexpected07 failure shape');
const sampleTime=r=>r.sample.sampledAt;
const before=monitors.findLast(r=>sampleTime(r)<=1792879777785), after=monitors.find(r=>sampleTime(r)>=1792879781508);
const delta=zoneMetricsDelta(before.sample.zoneMetrics,after.sample.zoneMetrics);
const compactHistogram=h=>({count:h.count,overBudgetCount:h.overBudgetCount,p50UpperUs:h.p50UpperUs,p95UpperUs:h.p95UpperUs,p99UpperUs:h.p99UpperUs,p999UpperUs:h.p999UpperUs,
  processLifetimeMaxBeforeUs:h.processLifetimeMaxBeforeUs,processLifetimeMaxAfterUs:h.processLifetimeMaxAfterUs,buckets:h.buckets});
const compactDelta={fromSampledAtMs:sampleTime(before),toSampledAtMs:sampleTime(after),scope:delta.scope,durationMs:delta.durationMs,
  movementAdmissionsDefinition:delta.movementAdmissionsDefinition,pendingMovementDeadlineLagDefinition:delta.pendingMovementDeadlineLagDefinition};
for(const key of ['immediateRequests','lifecycleRequests','movementAdmissions'])compactDelta[key]={successCount:delta[key].successCount,failureCount:delta[key].failureCount,duration:compactHistogram(delta[key].duration)};
for(const key of ['zoneWork','ownerTickDeadlineLag','pendingMovementDeadlineLag'])compactDelta[key]=compactHistogram(delta[key]);
const nearbyMonitors=monitors.filter(r=>sampleTime(r)>=lo&&sampleTime(r)<=hi);
const monitorResources=nearbyMonitors.map(r=>({line:r.line,sampledAt:r.sample.sampledAt,stop:r.sample.stop,memory:r.sample.memory,process:r.sample.process,host:r.sample.host}));
const slowGroups={};for(const r of slow){const key=r.receiverPhase+'|'+r.stage;(slowGroups[key]??=[]).push(r.durationMs);}
const slowCensus=Object.entries(slowGroups).map(([group,values])=>({group,count:values.length,minMs:Math.min(...values),maxMs:Math.max(...values),atLeast500:values.filter(v=>v>=500).length}));
const pauseAt=report.phaseWindows.find(p=>p.name==='soak').endedAtMs, failureEmittedAt=controls.find(r=>r.type==='continuousFailure').at;
const detailed={schemaVersion:1,runId,actor,scope:'Public bounded raw projections around the two original failed actions. No private account/store or recording payload was read.',
  inputPins:{report:pin(reportFile),raw:rawPin,slowStage:pin(slowFile),standaloneMonitor:pin(path.join(runRoot,'monitor-history.jsonl')),cleanup:pin(cleanupFile)},
  controls,failedActions:failed,actorActionsAroundFailure:actions,pendingAttackEvidence:traces,actualOwnerTransformsAroundFailure:transforms,
  originalFailedContinuousWindow:failures[0],driverCritical,selectedSlow,monitorBefore:before,monitorAfter:after,nearbyMonitorResources:monitorResources,zoneMetricsDelta:compactDelta};
const selectedFile=path.join(out,'selected-events-v1.json');fs.writeFileSync(selectedFile,JSON.stringify(detailed,null,2)+'\n',{flag:'wx'});
const summary={schemaVersion:1,runId,actor,analysisScope:'Read-only stream of17.4MB public driver raw, small report/slow-stage/monitor metadata. No tests, realm, game-source edits, private-account reads or recording payload.',
  inputPins:detailed.inputPins,selectedEvents:pin(selectedFile),rawTypeCounts:counts,
  outcome:{ok:report.ok,error:report.error,capacityAccepted:false,stageResults:report.stages.map(s=>({name:s.name,seconds:s.seconds,completed:s.completed,passed:s.passed,timeoutRatio:s.timeoutRatio})),
    fullSoakInterval:report.phaseWindows.find(p=>p.name==='soak'),continuousFailureCount:report.continuousAcceptance.failureCount,assessedWindows:report.continuousAcceptance.assessedWindows,
    firstFailingActor:actor,role:firstFailure.role,finalSegment:true,seconds:firstFailure.seconds,
    nativeOpportunities:firstFailure.activity.nativeOpportunities,nativeCompleted:firstFailure.activity.nativeCompleted,completionRatio:firstFailure.nativeActivity.completionRatio,
    launchedAttacks:firstFailure.activity.attacks,positiveHits:firstFailure.activity.hits,damage:firstFailure.activity.damage,ownerAckWaits:firstFailure.activity.ownerAckWaits,
    timeoutRatio:firstFailure.timeoutRatio,activityPassed:firstFailure.activityPassed,latencyPassed:firstFailure.latencyPassed,fanoutPassed:firstFailure.fanoutPassed,
    originalFailedContinuousVerdictRetained:true},
  failureClassification:failed.map(r=>({line:r.line,actor:r.actor,kind:r.kind,status:r.status,phase:r.phase,recordedAtMs:r.at,
    sendMonoMs:r.sentAtMs,latencyMs:r.latencyMs,scheduledMonoMs:r.scheduledAtMs,plannedLatencyMs:r.plannedLatencyMs,targetId:r.targetId,requestId:r.requestId,
    attackResult:r.attackResult??null,quarantined:r.ambiguousTargetQuarantined,
    beforeOfferedLoadPauseMs:pauseAt-r.at,beforeFailureVerdictMs:failureEmittedAt-r.at,
    classification:r.kind==='attack'?'No matching typedV2 outcome by900ms; trace has zero matching owner/target packets and is not truncated. This is an original timeout, not launched-with-no-damage miss or explicit unknown/unavailable.':
      'No accepted owner UserLocation by the original2s walk deadline. Original movement timeout; subsequent fresh owner replies are not retrospectively assigned to this pending action.'})),
  attackTrace:{requestId:traces.find(t=>t.requestId==='300')?.requestId,packets:traces.find(t=>t.requestId==='300')?.packetCount,truncated:traces.find(t=>t.requestId==='300')?.truncated,
    sentAtWallMs:traces.find(t=>t.requestId==='300')?.sentAtWallMs,endedAtWallMs:traces.find(t=>t.requestId==='300')?.endedAtWallMs},
  settlement:{offeredLoadPausedAtMs:pauseAt,continuousFailureEmittedAtMs:failureEmittedAt,verdictAfterPauseMs:failureEmittedAt-pauseAt,
    explanation:'Both timeouts were recorded36–39s before offered load ended. The final continuous segment was emitted during normal finalization; that does not create or erase the earlier failures. Original per-actor final-window2/50 timeout ratio and46/50 completion fail despite the whole-cohort last-window timeout ratio0.00256.',
    cleanupChecks:cleanup.checks,gatewayExit:cleanup.gatewayExit,monitorExit:cleanup.monitorExit},
  slowStage:{pin:pin(slowFile),rows:slow.length,groups:slowCensus,critical:selectedSlow,
    queueSpanAlignment: selectedSlow.filter(r=>r.stage==='socket_input.queue_residence'&&r.durationMs>=900).map(r=>({observedAtMs:r.observedAtMs,durationMs:r.durationMs,estimatedStartMs:r.estimatedSpanStartIfReceivedImmediatelyMs})),
    limitation:'Launcher receive times are not exact source span endpoints. Public records have no connection, actor or requestId; near-exact alignment with failed sends supports an input-queue residence correlation, not strict per-request identity or the upstream root cause. Input residence ends before that input waits for the serial gate.'},
  serverTelemetry:compactDelta,
  driverLag:{allSamples:driver.length,allMaxP99Ms:Math.max(...driver.map(r=>r.eventLoopLagP99Ms)),allMaxMs:Math.max(...driver.map(r=>r.eventLoopLagMaxMs)),
    criticalSamples:driverCritical.length,criticalMaxP99Ms:Math.max(...driverCritical.map(r=>r.eventLoopLagP99Ms)),criticalMaxMs:Math.max(...driverCritical.map(r=>r.eventLoopLagMaxMs)),
    thresholdP99Ms:report.policy.maximumDriverLagP99Ms,driverGuardStopped:report.safetyReason?.includes('driver')??false,
    limitation:'One-second event-loop samples cannot establish every microsecond scheduling interval. No observed driver guard breach is not proof that local scheduling had no effect.'},
  resources:monitorResources,
  conclusion:{proven:'Two actual pre-stop delivery deadlines failed for the combat actor. Server immediate-handler and movement-admission counters independently show multi-second requests; public input queue-residence spans align with the failed send times. The original probe remains failed.',
    notProven:['No exact connection/request attribution in slow-stage whitelist.','No root cause is established for why the serial connection loop did not receive the queued input sooner.','Low aggregate process CPU/IO and short sampled zoneWork do not prove all locks or every Zone command were fast; zoneWork excludes several waits/command/snapshot paths.','No supplied evidence identifies spectator JSON/IO, disk saturation, a physical-host limit or a full workload-capacity ceiling.'],
    nextStep:'Keep formal04 frozen. Use the separate source-only queue/serial-loop audit to choose bounded instrumentation or a concrete fix; do not relax900ms/2s/rolling gates or reclassify the timeouts.'},
  sourcePins:['apps/web/scripts/playtest-capacity-soak.mjs','apps/web/scripts/playtest-capacity-soak-core.mjs','apps/web/scripts/playtest-capacity-telemetry.mjs'].map(f=>pin(path.join(project,f)))};
const summaryFile=path.join(out,'summary-v1.json');fs.writeFileSync(summaryFile,JSON.stringify(summary,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({summary:pin(summaryFile),selected:pin(selectedFile),raw:rawPin,failures:summary.failureClassification,driverLag:summary.driverLag,
  deltaCounts:Object.fromEntries(['immediateRequests','movementAdmissions','zoneWork','ownerTickDeadlineLag'].map(k=>{const x=compactDelta[k].duration??compactDelta[k];return[k,{count:x.count,over50ms:x.overBudgetCount,p99:x.p99UpperUs,oldMax:x.processLifetimeMaxBeforeUs,newMax:x.processLifetimeMaxAfterUs}]}))}));
