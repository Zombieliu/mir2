import { verifyProbe12CollectedEvidence, REQUIRED_PROBE12_RUN_ID } from './verify-probe12-collected-evidence.mjs';
import {assertMeasuredPhase} from './measured-phase-contract.mjs';
import { verifySceneContract } from './scene-contract.mjs';
import { validateSlowStagePublicRow } from './slow-stage-row-contract.mjs';
import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { root, base, project, currentKeys, driverFrozenFile, read, pin, sha, assertPin, samePath, collectDriverInputs, git } from './input-lib.mjs';
import { verifyDriverReportContract } from './driver-report-contract.mjs';
import { ALLOWED_STAGES, MAX_LINE_BYTES, MAX_PUBLIC_BYTES, MAX_WRITE_BUFFER_BYTES } from './slow-stage-public.mjs';
import { createInterface } from 'node:readline';

export async function verifyFeasibilityGate(binding, preparedDrivers) {
  const { release, backendSourceCommit, driverSourceCommit } = binding;
  const probeRoot = path.join(base, 'release-native-10-v2-compact-halo-feasibility-12');
  if (!samePath(await fs.realpath(probeRoot), probeRoot)) throw new Error('Probe12 root must be a real owned directory');
  const owned = async name => { const file = path.join(probeRoot, name); if (!samePath(await fs.realpath(file), file)) throw new Error('Redirected probe12 evidence is forbidden'); return file; };
  const cleanupFile = await owned('realm-cleanup.json'), buildFile = await owned('gateway-build-pin.json');
  const cleanup = await read(cleanupFile), build = await read(buildFile);
  const processFile=await owned('realm-process.json'), probeProcess=await read(processFile);
  if(JSON.stringify(probeProcess.explicitAdmissionCaps)!=='[24,10,10,4,1,2]'||probeProcess.tokioWorkers!==4||probeProcess.content!=='platinum_176'||probeProcess.routing!=='single'||probeProcess.accountBackend!=='file'||probeProcess.slowStageThresholdMs!==50||probeProcess.slowStagePublicSchema!=='socket-context-v1'||probeProcess.slowStageHelperSha256!=='c2a137084f2166cbae603f721c6dde7322790c8dc2e05c2b8d59b79d7a58619f')throw Error('Probe12 original06 launcher caps or baseline runtime configuration differs');
  if (build.sourceCommit !== backendSourceCommit || build.backendSourceCommit !== backendSourceCommit || build.driverSourceCommit !== driverSourceCommit || !/^[a-f0-9]{40}$/.test(build.driverSourceCommit ?? '') ||
      build.sha256 !== release.binary.sha256 || build.bytes !== release.binary.bytes || build.sourceManifest?.sha256 !== release.sourceManifestFileSha256 ||
      JSON.stringify(build.releaseRecord) !== JSON.stringify(binding.releasePin) || build.sourceManifest?.inputs !== 41) throw new Error('Formal admission requires the fresh same468/a430/41 backend from real probe12');
  if (cleanup.sourceCommit !== backendSourceCommit || cleanup.backendSourceCommit !== backendSourceCommit || cleanup.driverSourceCommit !== build.driverSourceCommit ||
      cleanup.sha256 !== build.sha256 || cleanup.formalStarted !== false || cleanup.capacityAccepted !== false) throw new Error('Probe12 cleanup provenance differs');
  const sourcePinFile = await owned('source-pins-admitted-v2.json'), sources = await read(sourcePinFile);
  const preparedSourceFile = await owned('source-pins.json'), preparedSources = await read(preparedSourceFile);
  const drivers = preparedDrivers ?? await collectDriverInputs(), driver = drivers.frozen;
  if (drivers.driverSourceCommit !== driverSourceCommit || sources.sourceCommit !== backendSourceCommit || sources.backendSourceCommit !== backendSourceCommit ||
      sources.driverSourceCommit !== build.driverSourceCommit || preparedSources.backendSourceCommit !== backendSourceCommit ||
      preparedSources.driverSourceCommit !== build.driverSourceCommit || !samePath(preparedSources.driverFrozenManifest ?? '', driverFrozenFile) ||
      preparedSources.driverSourceSha256 !== driver.sourceSha256 || preparedSources.socketDiagnosticsReleaseUsed !== true ||
      !Array.isArray(sources.files) || sha(JSON.stringify(sources.files)) !== sources.sourceConfigSha256) throw new Error('Probe12 exact coordinated-driver input binding is incomplete');
  const preparedSourcePin = await pin(preparedSourceFile), boundPrepared = sources.files.find(row => samePath(row.file, preparedSourceFile));
  if (!boundPrepared || boundPrepared.bytes !== preparedSourcePin.bytes || boundPrepared.sha256 !== preparedSourcePin.sha256 ||
      !Array.isArray(preparedSources.files) || sha(JSON.stringify(preparedSources.files)) !== preparedSources.sourceConfigSha256) throw new Error('Probe12 prepared source closure is not bound into admitted evidence');
  for (const input of sources.files) await assertPin(input);
  for (const input of drivers.pins) {
    const matched = sources.files.find(row => samePath(row.file, input.file));
    if (!matched || matched.bytes !== input.bytes || matched.sha256 !== input.sha256) throw new Error('Probe12 driver/import/data/transport differs from the current exact frozen closure');
  }
  for (const input of driver.files) {
    const bytes = await fs.readFile(path.join(project, input.file));
    if (sha(Buffer.from(bytes.toString('utf8').replace(/\r\n/g, '\n'))) !== sha(git(['show', build.driverSourceCommit + ':mir2-web3/' + input.file]))) throw new Error('Probe12 driver commit does not contain the exact same frozen nine inputs');
  }
  const coordinationSource = await fs.readFile(path.join(project, 'apps/web/scripts/playtest-capacity-soak.mjs'), 'utf8');
  const declaration = coordinationSource.match(/Object\.defineProperty\(report, 'navigationCoordination', \{ enumerable: true,\s*value: Object\.freeze\(\{ scope: '([^'\r\n]+)',\s*maximumHintCellsPerConnection: NAVIGATION_HINT_MAX_CELLS, blockedOpportunityRule: '([^'\r\n]+)' \}\) \}\)/);
  const maximumCells = coordinationSource.match(/export const NAVIGATION_HINT_MAX_CELLS = (\d+);/);
  if (!declaration || maximumCells?.[1] !== '7') throw new Error('Current pinned native navigation declaration is missing or incompatible');
  const expectedCoordination = { scope: declaration[1], maximumHintCellsPerConnection: 7, blockedOpportunityRule: declaration[2] };
  const { capacityPolicy } = await import(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-capacity-soak-core.mjs')));
  if (currentKeys.some(key => cleanup.preStopCapacity?.[key] !== 0) || cleanup.checks?.sixCountersZero !== true ||
      cleanup.checks?.normalOwnedShutdown !== true || cleanup.checks?.processAbsent !== true || cleanup.checks?.portsFree !== true ||
      cleanup.checks?.monitorStoppedBeforeGateway !== true || cleanup.checks?.monitorExitZero !== true || cleanup.checks?.monitorTerminalPreserved !== true ||
      cleanup.gatewayExit?.code !== 0 || cleanup.monitorExit?.code !== 0 || cleanup.inputsChangedWhileRunning || cleanup.error) throw new Error('Probe12 owned cleanup did not pass');
  const reportFile = cleanup.report?.reportPath;
  if (typeof reportFile !== 'string' || !samePath(path.dirname(reportFile), path.join(probeRoot, 'reports')) || !reportFile.endsWith('.report.json') ||
      !samePath(await fs.realpath(reportFile), reportFile)) throw new Error('Probe12 report must be an explicit owned real file');
  const provenanceFile = await owned('report-provenance.json'), provenance = await read(provenanceFile), reportPin = await pin(reportFile);
  if (provenance.backendSourceCommit !== backendSourceCommit || provenance.driverSourceCommit !== build.driverSourceCommit ||
      provenance.backendBinarySha256 !== build.sha256 || provenance.driverSourceSha256 !== driver.sourceSha256 ||
      !samePath(provenance.driverFrozenManifest ?? '', driverFrozenFile) || provenance.socketDiagnosticsReleaseUsed !== true || provenance.capacityAccepted !== false ||
      JSON.stringify(provenance.report) !== JSON.stringify(reportPin)) throw new Error('Probe12 original report bytes are not bound to the backend/driver identity sidecar');
  const report = await read(reportFile), p = report.policy;
  if(report.runId!==REQUIRED_PROBE12_RUN_ID)throw Error('Formal09 is bound only to compact12 run1792908575146-40a66fff; later or historical probes cannot substitute');
  await verifyDriverReportContract(report);
  for(const key of ['nativeHpMaintenance','nativeAttackWitness'])if(JSON.stringify(provenance[key])!==JSON.stringify(report[key]))throw Error('Probe12 immutable HP/witness metadata is not preserved');
  if (JSON.stringify(provenance.nativePatrolPlanning) !== JSON.stringify(report.nativePatrolPlanning)) throw Error('Probe12 native patrol planning declaration is not preserved by the original result sidecar');
  const scene=await verifySceneContract(root,'formal'),probeScene=await verifySceneContract(probeRoot,'probe');
  if(JSON.stringify(provenance.measurementClock)!==JSON.stringify(report.measurementClock))throw Error('Probe12 measurement-clock declaration is not preserved in its result sidecar');
  const probeScenarioFile=await owned('scenario.json');
  if(scene.sourceSha256!==probeScene.sourceSha256||probeProcess.sceneProfile!==scene.profile||probeProcess.sceneSourceSha256!==scene.sourceSha256||probeProcess.scene10Sha256!==scene.probeScene.sha256||probeProcess.scene100Sha256!==scene.formalScene.sha256||probeProcess.sceneAnalysisSha256!==scene.analysis.sha256||probeProcess.patrolSpan!==2||preparedSources.sceneSourceSha256!==scene.sourceSha256||provenance.sceneSourceSha256!==scene.sourceSha256)throw Error('Fresh probe12 exact halo scene/static source identity differs from formal first10');
  const slowStatusFile=await owned('slow-stage-public-status.json'), slowFile=await owned('slow-stage-public.jsonl'), slowStatus=await read(slowStatusFile), slowBefore=await fs.stat(slowFile);
  if(slowBefore.size>MAX_PUBLIC_BYTES) throw Error('Probe12 public diagnostic total exceeds fixed32MiB');
  const slowDigest=crypto.createHash('sha256'); let slowBytes=0,slowRows=0;
  for await(const chunk of createReadStream(slowFile)){slowBytes+=chunk.length;slowDigest.update(chunk);}
  for await(const line of createInterface({input:createReadStream(slowFile),crlfDelay:Infinity})) {
    if(Buffer.byteLength(line)>MAX_LINE_BYTES || !line) throw Error('Probe12 public diagnostic line is invalid');
    validateSlowStagePublicRow(JSON.parse(line),line); slowRows++;
  }
  const slowAfter=await fs.stat(slowFile), {thresholdMs,dataFile,...runtimeSlowStatus}=slowStatus;
  if(slowBefore.size!==slowBytes||slowAfter.size!==slowBytes||slowBefore.mtimeMs!==slowAfter.mtimeMs||slowStatus.schemaVersion!==1||thresholdMs!==50||!samePath(dataFile??'',slowFile)||
    slowStatus.complete!==true||slowStatus.stderrEnded!==true||slowStatus.writerFinished!==true||slowStatus.stopRequested!==false||!Array.isArray(slowStatus.reasons)||slowStatus.reasons.length!==0||
    slowStatus.rowsQueued!==slowRows||slowStatus.bytesQueued!==slowBytes||slowStatus.lineLimitBytes!==MAX_LINE_BYTES||slowStatus.totalLimitBytes!==MAX_PUBLIC_BYTES||
    slowStatus.writeLimitBytes!==MAX_WRITE_BUFFER_BYTES||slowStatus.writeHighWaterMarkBytes!==MAX_WRITE_BUFFER_BYTES||!Number.isSafeInteger(slowStatus.maxBufferedLineBytes)||slowStatus.maxBufferedLineBytes<0||slowStatus.maxBufferedLineBytes>MAX_LINE_BYTES||
    !Number.isSafeInteger(slowStatus.maxQueuedWriteBytes)||slowStatus.maxQueuedWriteBytes<0||slowStatus.maxQueuedWriteBytes>MAX_WRITE_BUFFER_BYTES||
    JSON.stringify(runtimeSlowStatus)!==JSON.stringify(cleanup.slowStageDiagnostics)) throw Error('Probe12 diagnostic completeness/limits/cleanup binding differs');
  const slowPin={file:slowFile,bytes:slowBytes,sha256:slowDigest.digest('hex')};
  if (provenance.runId !== report.runId || cleanup.report?.runId !== report.runId || cleanup.report?.backendSourceCommit !== backendSourceCommit ||
      cleanup.report?.driverSourceCommit !== build.driverSourceCommit || JSON.stringify(report.navigationCoordination) !== JSON.stringify(expectedCoordination)) throw new Error('Probe12 report navigation declaration/provenance differs from exact pinned source');
  const expectedPolicy = capacityPolicy({ profile: 'probe', activityMode: 'native', stages: '2,10', stageSeconds: 300, soakSeconds: 360,
    combatActors: 1, resumeSamples: 0, saveSamples: 0, wallSeconds: 3600, movementIntervalMs: 750 });
  if (JSON.stringify(p) !== JSON.stringify(expectedPolicy)) throw new Error('Probe12 must retain the entire original probe policy');
  if (report.ok !== true || report.ordinaryAccountsOnly !== true || report.attackResults?.mode !== 'v2' || report.attackResults.capability !== 'nativeAttackOutcomeV1' || p?.profile !== 'probe' || p.activityMode !== 'native' ||
      JSON.stringify(p.stages) !== '[2,10]' || p.stageSeconds !== 300 || p.soakSeconds !== 360 || p.combatActors !== 1 || p.resumeSamples !== 0 || p.saveSamples !== 0 ||
      p.fullAcceptanceEligible !== false || report.playableCapacityAccepted !== false || report.acceptedControlledActors !== null ||
      report.peakControlledActors !== 10 || report.highestPassedStage !== 10 || report.safetyStopped !== false || report.error || report.cleanupErrors?.length ||
      report.continuousAcceptance?.passed !== true || report.continuousAcceptance.failureCount !== 0) throw new Error('Real compact probe12 run1792908575146-40a66fff has not passed; a probe remains ineligible for capacity acceptance');
  const strict = { minimumCombatAttacksPerMinute: 18, minimumCombatHitsPerMinute: 12, minimumNativeRunRatio: .5, minimumNativeCellsPerMinute: 80,
    maximumCorrectionRatio: .05, maximumMissedPlanRatio: .05, timeoutExclusive: .01, movementP95Ms: 750, maximumMemorySlopeMiBPerHour: 32 };
  if (Object.entries(strict).some(([key, value]) => p[key] !== value)) throw new Error('Probe12 policy differs from fixed ordinary thresholds');
  const expectedPhases=[['stage-2',2,300,5],['stage-10',10,300,5],['soak',10,360,6]];
  if(JSON.stringify(report.stages?.map(p=>p.name))!==JSON.stringify(expectedPhases.map(p=>p[0]))) throw Error('Probe12 requires exactly the original measured2/10/soak order');
  const phases=expectedPhases.map(([name,count,seconds,windows])=>{
    const phase=report.stages.find(p=>p.name===name);assertMeasuredPhase(phase,seconds,true);
    if(!phase?.completed||!phase.passed||phase.controlledActors!==count||phase.seconds!==seconds||!Number.isSafeInteger(phase.startedAtMs)||!Number.isSafeInteger(phase.endedAtMs)||phase.endedAtMs-phase.startedAtMs<seconds*1000||
      phase.rollingWindows?.length!==windows||phase.rollingWindows.some((w,i)=>!w.passed||!Number.isFinite(w.seconds)||w.seconds<60||!Number.isFinite(w.elapsedSeconds)||w.elapsedSeconds<(i+1)*60)||
      (count===10&&phase.combatCoverage!=='shared-positive-damage')) throw Error('Probe12 requires real300/300/360 clock and complete passing5/5/6 windows'); return phase;
  });
  const rawFile = reportFile.replace(/\.report\.json$/, '.samples.jsonl');
  if (!samePath(await fs.realpath(rawFile), rawFile)) throw new Error('Redirected raw probe12 evidence is forbidden');
  const before = await fs.stat(rawFile), digest = crypto.createHash('sha256'); let bytes = 0;
  for await (const chunk of createReadStream(rawFile)) { bytes += chunk.length; digest.update(chunk); }
  const rawPin = { file: rawFile, bytes, sha256: digest.digest('hex') };
  const verifierFile = path.join(project, 'docs/generated/cell-p0-p1-completion-20261024/evidence-tools/verify-evidence.mjs');
  const { streamCapacitySamples, STREAM_LIMITS, verifyCapacityStatus, verifyCapacityPhases } = await import(pathToFileURL(verifierFile));
  const streamed = await streamCapacitySamples(rawFile, rawPin, STREAM_LIMITS, report);
  verifyCapacityPhases(report, streamed.selected);
  if (verifyCapacityStatus(report, streamed.selected, streamed.monitorSummary) !== false) throw new Error('Probe12 was incorrectly promoted to formal capacity acceptance');
  const after = await fs.stat(rawFile), monitor = streamed.monitorSummary;
  if (before.size !== bytes || after.size !== bytes || before.mtimeMs !== after.mtimeMs || !monitor.complete || monitor.count < 2 || monitor.invalidCount || monitor.stoppedCount || monitor.maxGapMs > 15000 ||
      streamed.selected.some(row => ['continuousFailure', 'safetyStop'].includes(row.type))) throw new Error('Probe12 raw/monitor continuity is incomplete');
  for(const [name,, ,windows] of expectedPhases) {
    const rows=streamed.selected.filter(row=>row.type==='window'&&row.phase===name);
    if(rows.length!==windows||rows.some(row=>row.assessment?.passed!==true||row.assessment.seconds<60)) throw Error('Probe12 raw measured windows do not match exact5/5/6 complete counts');
  }
  for (const phase of phases) {
    const bracket = monitor.phaseBrackets.find(value => value.name === phase.name);
    if (!bracket?.beforeStart || !bracket.afterStart || !bracket.beforeEnd || [bracket.startBeforeGapMs, bracket.startAfterGapMs, bracket.endBeforeGapMs].some(gap => gap < 0 || gap > 15000)) throw new Error('Probe12 raw monitor does not cover its true phase interval');
  }
  const collectedEvidence = await verifyProbe12CollectedEvidence({report,reportPin,rawPin,cleanup,provenance,sources,scene,binding,monitor});
  const files = await Promise.all([cleanupFile, buildFile, processFile, reportFile, sourcePinFile, preparedSourceFile, provenanceFile, await owned('launcher-source-frozen-v1.json'), slowStatusFile, probeScenarioFile, await owned('driver-report-contract.mjs'), verifierFile].map(pin));
  files.push(rawPin,slowPin,...scene.files,...probeScene.files,...collectedEvidence.files,await pin(path.join(root,'verify-probe12-collected-evidence.mjs')));

  return { schemaVersion: 1, verifiedAt: new Date().toISOString(), sourceCommit: backendSourceCommit, backendSourceCommit, driverSourceCommit,
    feasibilityDriverSourceCommit: build.driverSourceCommit, exactSameFrozenDriverInputs: true, driverFrozenManifest: driverFrozenFile,
    driverSourceSha256: driver.sourceSha256, releaseSha256: release.binary.sha256, runId: report.runId, navigationCoordination: expectedCoordination, nativePatrolPlanning: report.nativePatrolPlanning, measurementClock: report.measurementClock, nativeHpMaintenance:report.nativeHpMaintenance,nativeAttackWitness:report.nativeAttackWitness, diagnosticsComplete:true,
    feasibilityPassed: true, collectedEvidence, sceneProfile: scene.profile, sceneSourceSha256: scene.sourceSha256, scene10Sha256: scene.probeScene.sha256, scene100Sha256: scene.formalScene.sha256, capacityAccepted: false, profile: 'probe', measuredCohorts:[2,10],configuredSeconds:[300,300],measuredSeconds:phases.slice(0,2).map(p=>p.measuredSeconds),measuredPassingWindows:[5,5],native10SoakSeconds: 360,native10SoakMeasuredSeconds:phases[2].measuredSeconds, actualPassedSoakWindows: 6, rawRows: streamed.rows,
    rawMonitor: { count: monitor.count, complete: monitor.complete, maxGapMs: monitor.maxGapMs }, files };
}
