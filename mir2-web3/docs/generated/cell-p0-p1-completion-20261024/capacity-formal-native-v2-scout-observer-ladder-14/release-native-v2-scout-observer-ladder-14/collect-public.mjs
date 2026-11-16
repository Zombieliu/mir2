import {assertPin} from './input-lib.mjs';
import {verifyDriverEvidenceBasis} from './driver-evidence-contract.mjs';
import {assertFormal14Configured} from './formal-template-contract.mjs';
assertFormal14Configured();
import {createPhaseWindowCounter} from './phase-window-contract.mjs';
import {CORE_CODE_EVIDENCE_COMMIT} from './driver-report-contract.mjs';
import {verifyWireDiagnosticSource,wireDiagnosticIdentity,createWireRawCounter,validateWireTerminal,WIRE_TERMINAL,CORE_CODE_EVIDENCE_COMMIT as WIRE_CORE_CODE_EVIDENCE_COMMIT} from './wire-diagnostics-contract.mjs';
import {assertMeasuredPhase} from './measured-phase-contract.mjs';
import { verifySceneContract } from './scene-contract.mjs';
import { validateSlowStagePublicRow } from './slow-stage-row-contract.mjs';
// Post-run public evidence collection only. Never starts a process or a realm.
import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';
import { ALLOWED_STAGES, MAX_LINE_BYTES, MAX_PUBLIC_BYTES, MAX_WRITE_BUFFER_BYTES } from './slow-stage-public.mjs';

const base = 'C:/mir2-build/p01-completion-20261024';
const topic = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/docs/generated/cell-p0-p1-completion-20261024';
const child = 'capacity-formal-native-v2-scout-observer-ladder-14', dest = path.join(topic, child);
const realm = 'release-native-v2-scout-observer-ladder-14';
import {DRIVER_BINDING} from './driver-binding-contract.mjs';
const backendSourceCommit = DRIVER_BINDING.backendSourceCommit;
import { EXPECTED_DRIVER_SOURCE_COMMIT as driverSourceCommit, EXPECTED_DRIVER_SOURCE_SHA256 as driverSourceSha256, EXPECTED_NATIVE_PATROL_PLANNING, EXPECTED_MEASUREMENT_CLOCK, EXPECTED_NATIVE_HP_MAINTENANCE, EXPECTED_NATIVE_ATTACK_WITNESS, EXPECTED_NATIVE_CHASE_PREFERENCE } from './driver-report-contract.mjs';
if(!/^[a-f0-9]{40}$/.test(driverSourceCommit)||!/^[a-f0-9]{64}$/.test(driverSourceSha256)||!EXPECTED_NATIVE_PATROL_PLANNING)throw Error('Collector template needs the final reviewed patrol driver identity');
const backendBinarySha256 = DRIVER_BINDING.backendBinarySha256;
if (process.argv.slice(2).join(' ') !== '--collect-after-owned-cleanup') throw Error('Explicit post-cleanup collection argument required; never run during a live capacity realm');
const realmRoot = path.join(base, realm);
const expectedPolicy = {
  "profile": "standard",
  "activityMode": "native",
  "movementIntervalMs": 750,
  "stages": [
    2,
    10,
    25,
    50,
    100
  ],
  "stageSeconds": 300,
  "soakSeconds": 7200,
  "wallSeconds": 18000,
  "layout": "dispersed",
  "combatActors": 10,
  "resumeSamples": 5,
  "saveSamples": 5,
  "commandsPerSecond": 3,
  "intentionIntervalMs": 750,
  "variableActionCadence": true,
  "maximumSendLatenessMs": 250,
  "movementP95Ms": 750,
  "chatP95Ms": 1000,
  "actionTimeoutMs": 2000,
  "timeoutExclusive": 0.01,
  "maximumMissedPlanRatio": 0.05,
  "maximumCorrectionRatio": 0.05,
  "minimumMovesPerMinute": 30,
  "minimumCombatHitsPerMinute": 12,
  "minimumCombatAttacksPerMinute": 18,
  "aoiRange": 16,
  "aoiGraceMs": 1000,
  "nativeChatIntervalMs": 30000,
  "nativeTurnIntervalMs": 60000,
  "minimumNativeRunRatio": 0.5,
  "minimumNativeCellsPerMinute": 80,
  "maximumEvidenceInFlight": 4,
  "maximumMemorySlopeMiBPerHour": 32,
  "memoryWarmupSeconds": 300,
  "maximumDriverLagP99Ms": 250,
  "fullAcceptanceEligible": true
};

const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const privatePart = /^(?:private|secrets?|credentials?)$/i;
function safeRelative(value, archive = false) {
  if (typeof value !== 'string' || !value || path.isAbsolute(value) || path.win32.isAbsolute(value) || path.posix.isAbsolute(value)) throw Error('Expected a safe relative public path');
  const parts = value.split(/[\\/]/);
  if (parts.some(part => !part || part === '.' || part === '..' || /[\x00-\x1f:]/.test(part) || /[. ]$/.test(part) || privatePart.test(part))) throw Error('Unsafe relative public path component');
  if (!archive && (parts.some(part => /^recordings$/i.test(part)) || /\.(?:exe|samples\.jsonl)$/i.test(parts.at(-1)))) throw Error('Payload is not a public-copy input');
  return parts.join('/');
}
const samePath = (a, b) => path.resolve(a).toLowerCase() === path.resolve(b).toLowerCase();
async function realDirectory(root) {
  const actual = await fs.realpath(root), stat = await fs.lstat(root);
  if (!samePath(actual, root) || !stat.isDirectory() || stat.isSymbolicLink()) throw Error('Redirected public directory');
  return actual;
}
function lexicalPublicFile(file, archive = false) {
  if (typeof file !== 'string' || !path.isAbsolute(file) || file.split(/[\\/]/).some(part => part === '.' || part === '..' || privatePart.test(part))) throw Error('Unsafe absolute public source path');
  const requested = path.resolve(file);
  for (const root of [base, topic]) {
    try { return { root, requested, relative: safeRelative(path.relative(root, requested), archive) }; } catch {}
  }
  throw Error('Public path escaped the explicit archive/topic roots');
}
async function realPublicFile(file, archive = false) {
  const { root, requested } = lexicalPublicFile(file, archive), rootReal = await realDirectory(root), actual = await fs.realpath(requested);
  safeRelative(path.relative(rootReal, actual), archive);
  const stat = await fs.lstat(actual);
  if (!samePath(actual, requested) || !stat.isFile() || stat.isSymbolicLink()) throw Error('Redirected or non-regular public source');
  return actual;
}
async function publicOutputFile(file) {
  const { root, requested } = lexicalPublicFile(file), rootReal = await realDirectory(root), parent = path.dirname(requested);
  const parentReal = await realDirectory(parent);
  if (!samePath(parentReal, rootReal)) safeRelative(path.relative(rootReal, parentReal));
  return requested;
}
const read = async file => JSON.parse((await fs.readFile(await realPublicFile(file), 'utf8')).replace(/^\uFEFF/, ''));
const writeNew = async (file, value) => fs.writeFile(await publicOutputFile(file), JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
const counters = ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases', 'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight'];

function redactCheck(value) {
  if (!value || typeof value !== 'object') return;
  for (const [key, nested] of Object.entries(value)) {
    if (/^(?:password|secret|token|credential|authorization|cookie|assertion|privateKey)$/i.test(key) && nested != null && nested !== '[redacted]') throw Error('Unredacted public JSON credential field');
    redactCheck(nested);
  }
}
async function rawDigest(file) {
  file = await realPublicFile(file);
  const before = await fs.lstat(file);
  if (!before.isFile() || before.isSymbolicLink()) throw Error('Only regular stable archive files are allowed');
  const digest = crypto.createHash('sha256'); let bytes = 0;
  for await (const chunk of createReadStream(file)) { bytes += chunk.length; digest.update(chunk); }
  const after = await fs.stat(file);
  if (bytes !== before.size || after.size !== before.size || after.mtimeMs !== before.mtimeMs) throw Error('Source changed during streaming digest');
  return { bytes, sha256: digest.digest('hex') };
}
async function streamRows(file, observe, lineLimit = 16 * 1024 * 1024) {
  file = await realPublicFile(file, true);
  const before = await fs.lstat(file);
  if (!before.isFile() || before.isSymbolicLink()) throw Error('Only regular stable JSONL archives are allowed');
  const digest = crypto.createHash('sha256'), source = createReadStream(file); let bytes = 0, rows = 0;
  source.on('data', chunk => { bytes += chunk.length; digest.update(chunk); });
  for await (const line of createInterface({ input: source, crlfDelay: Infinity })) {
    if (!line || Buffer.byteLength(line) > lineLimit) throw Error('Invalid or oversized JSONL row');
    const value = JSON.parse(line); rows++; await observe(value,line);
  }
  const after = await fs.stat(file);
  if (bytes !== before.size || after.size !== before.size || after.mtimeMs !== before.mtimeMs) throw Error('JSONL archive changed while read');
  return { bytes, sha256: digest.digest('hex'), rows };
}

await realDirectory(base); await realDirectory(topic); safeRelative(child);
const wireSource=await verifyWireDiagnosticSource(),wireIdentity=wireDiagnosticIdentity(wireSource);
let wireTerminal=null,wireTerminalPin=null;try{const file=await realPublicFile(path.join(realmRoot,WIRE_TERMINAL));wireTerminal=await read(file);wireTerminalPin={file,...await rawDigest(file)};}catch(error){if(error.code!=='ENOENT')throw error;}
try { await fs.access(dest); throw Error('Refusing an existing public collection'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
const cleanup = await read(path.join(realmRoot, 'realm-cleanup.json'));
if (!Number.isSafeInteger(cleanup.pid) || cleanup.pid < 1 || cleanup.backendSourceCommit !== backendSourceCommit || cleanup.driverSourceCommit !== driverSourceCommit ||
    cleanup.checks?.processAbsent !== true || cleanup.checks?.portsFree !== true || cleanup.remainingOwnedPortListeners?.length ||
    !terminalExit(cleanup.gatewayExit) || !terminalExit(cleanup.monitorExit)) throw Error('Collect only after both owned processes and Gateway listeners are confirmed stopped');
function terminalExit(value) { return value && (Number.isInteger(value.code) || (typeof value.signal === 'string' && value.signal.length > 0) || value.spawnError === true); }
const cleanupKeys = ['sixCountersZero', 'normalOwnedShutdown', 'processAbsent', 'portsFree', 'monitorStoppedBeforeGateway', 'monitorExitZero', 'monitorTerminalPreserved'];
const normalProcessCleanup = cleanupKeys.every(key => cleanup.checks?.[key] === true) && counters.every(key => cleanup.preStopCapacity?.[key] === 0) &&
  cleanup.gatewayExit?.code === 0 && cleanup.monitorExit?.code === 0 && !cleanup.remainingOwnedPortListeners?.length && !cleanup.inputsChangedWhileRunning && !cleanup.error;

const slowStatus = await read(path.join(realmRoot, 'slow-stage-public-status.json'));
const slowFile = path.join(realmRoot, 'slow-stage-public.jsonl'), slowStat = await fs.stat(await realPublicFile(slowFile));
if (slowStat.size > MAX_PUBLIC_BYTES) throw Error('Slow-stage metadata exceeds fixed32MiB cap');
const stageLabels = new Set(ALLOWED_STAGES);
const slowMeta=await streamRows(slowFile,(row,line)=>validateSlowStagePublicRow(row,line),MAX_LINE_BYTES);
const allowedReasons = new Set(['request-stop-error','noncanonical-connection','unsafe-connection','noncanonical-phase-elapsed','unsafe-phase-elapsed','noncanonical-duration','unsafe-duration','invalid-receive-clock','public-byte-cap','write-buffer-cap','write-error','write-backpressure','non-buffer-stderr','line-byte-cap','partial-line-at-eof','stderr-read-error','stderr-closed-before-end','metadata-write-error','metadata-closed-before-finish','gateway-not-exited','stderr-end-timeout','metadata-end-error','metadata-finish-timeout']);
if (slowStatus.schemaVersion !== 1 || slowStatus.thresholdMs !== 50 || !samePath(slowStatus.dataFile ?? '', slowFile) ||
    slowStatus.lineLimitBytes !== MAX_LINE_BYTES || slowStatus.totalLimitBytes !== MAX_PUBLIC_BYTES || slowStatus.writeLimitBytes !== MAX_WRITE_BUFFER_BYTES ||
    slowStatus.writeHighWaterMarkBytes !== MAX_WRITE_BUFFER_BYTES || !Number.isSafeInteger(slowStatus.maxBufferedLineBytes) || slowStatus.maxBufferedLineBytes < 0 || slowStatus.maxBufferedLineBytes > MAX_LINE_BYTES ||
    !Number.isSafeInteger(slowStatus.maxQueuedWriteBytes) || slowStatus.maxQueuedWriteBytes < 0 || slowStatus.maxQueuedWriteBytes > MAX_WRITE_BUFFER_BYTES ||
    !Array.isArray(slowStatus.reasons) || slowStatus.reasons.some(reason => !allowedReasons.has(reason))) throw Error('Invalid slow-stage capture status');
const { thresholdMs, dataFile, ...runtimeSlowStatus } = slowStatus;
if (JSON.stringify(runtimeSlowStatus) !== JSON.stringify(cleanup.slowStageDiagnostics)) throw Error('Slow-stage public status differs from the original cleanup');
const diagnosticsComplete = slowStatus.complete === true && slowStatus.stderrEnded === true && slowStatus.writerFinished === true &&
  slowStatus.stopRequested === false && slowStatus.reasons.length === 0 && slowStatus.rowsQueued === slowMeta.rows && slowStatus.bytesQueued === slowMeta.bytes;
if (slowStatus.complete === true && !diagnosticsComplete) throw Error('Claimed complete slow-stage stream contradicts actual rows or bytes');
const normalOwnedCleanup = normalProcessCleanup && diagnosticsComplete;

const reportFile = cleanup.report?.reportPath;
if (typeof reportFile !== 'string' || !samePath(path.dirname(reportFile), path.join(realmRoot, 'reports')) || !reportFile.endsWith('.report.json')) throw Error('Cleanup must name an explicit owned report');
await realPublicFile(reportFile);
const runId = path.basename(reportFile, '.report.json');
if (!/^[0-9]+-[a-f0-9]+$/.test(runId) || cleanup.report?.runId !== runId) throw Error('Unexpected owned formal02 runId');
const reportRelative = `${realm}/reports/${runId}.report.json`, rawRelative = `${realm}/reports/${runId}.samples.jsonl`;
const report = await read(path.join(base, reportRelative)), release = await read(path.join(base, 'release-account-store-wait-diagnostics-v1.json'));
const manifestBytes = await fs.readFile(await realPublicFile(path.join(base, 'backend-source-account-store-wait-diagnostics-v1.json'))), manifest = JSON.parse(manifestBytes);
const build = await read(path.join(realmRoot, 'gateway-build-pin.json')), processRecord = await read(path.join(realmRoot, 'realm-process.json'));
if(JSON.stringify(processRecord.explicitAdmissionCaps)!=='[128,100,100,4,1,2]'||processRecord.tokioWorkers!==4||processRecord.slowStageThresholdMs!==50||processRecord.content!=='platinum_176'||processRecord.routing!=='single'||processRecord.accountBackend!=='file')throw Error('Formal04 must retain the actual original formal launcher caps/runtime profile');
const preparedSources = await read(path.join(realmRoot, 'source-pins.json'));
const admittedSources = await read(path.join(realmRoot, 'source-pins-admitted-v2.json'));
const provenance = await read(path.join(realmRoot, 'report-provenance.json'));
const driverFreezeFile = path.join(base, 'capacity-attack-deadline-driver-frozen-v1.json'), driverFreeze = await read(driverFreezeFile);
const reportIdentity = { file: reportFile, ...await rawDigest(reportFile) };
if (report.runId !== runId || typeof report.ok !== 'boolean' || typeof report.playableCapacityAccepted !== 'boolean' ||
    report.acceptedControlledActors !== (report.playableCapacityAccepted ? 100 : null) || JSON.stringify(report.policy) !== JSON.stringify(expectedPolicy) ||
    report.attackResults?.mode !== 'v2' || report.attackResults.capability !== 'nativeAttackOutcomeV1' || report.ordinaryAccountsOnly !== true ||
    !Number.isInteger(report.peakControlledActors) || report.peakControlledActors < 0 || report.peakControlledActors > 100 ||
    release.sourceCommit !== backendSourceCommit || release.binary.sha256 !== backendBinarySha256 || manifest.files?.length !== 50 ||
    build.sha256 !== release.binary.sha256 || build.bytes !== release.binary.bytes || build.sourceCommit !== backendSourceCommit || cleanup.sourceCommit !== backendSourceCommit ||
    processRecord.sourceCommit !== backendSourceCommit || processRecord.sha256 !== backendBinarySha256 || build.sourceManifest?.sha256 !== sha(manifestBytes) ||
    release.sourceManifestFileSha256 !== sha(manifestBytes) || manifest.sourceCommit !== backendSourceCommit ||
    manifest.sourceManifestSha256 !== release.sourceManifestSha256 || sha(JSON.stringify(manifest.files)) !== release.sourceManifestSha256 ||
    !samePath(release.backendSourceManifest, path.join(base, 'backend-source-account-store-wait-diagnostics-v1.json'))) throw Error('Original coordinated V2 formal/source/binary identity differs');
for (const value of [build, processRecord, preparedSources, admittedSources, provenance, cleanup.report]) {
  if (value.backendSourceCommit !== backendSourceCommit || value.driverSourceCommit !== driverSourceCommit) throw Error('Backend and driver commit provenance differs');
}
if (driverFreeze.schemaVersion !== 1 || driverFreeze.files?.length !== 9 || sha(JSON.stringify(driverFreeze.files)) !== driverFreeze.sourceSha256 ||
    driverFreeze.sourceSha256 !== driverSourceSha256 ||
    preparedSources.driverSourceSha256 !== driverFreeze.sourceSha256 || provenance.driverSourceSha256 !== driverFreeze.sourceSha256 ||
    !samePath(preparedSources.driverFrozenManifest ?? '', driverFreezeFile) || !samePath(provenance.driverFrozenManifest ?? '', driverFreezeFile) ||
    preparedSources.socketDiagnosticsReleaseUsed !== true || provenance.socketDiagnosticsReleaseUsed !== true ||
    provenance.backendBinarySha256 !== backendBinarySha256 || JSON.stringify(provenance.report) !== JSON.stringify(reportIdentity) ||
    provenance.runId !== runId || provenance.capacityAcceptanceAdjudicated !== false ||
    !Array.isArray(preparedSources.files) || sha(JSON.stringify(preparedSources.files)) !== preparedSources.sourceConfigSha256 ||
    !Array.isArray(admittedSources.files) || sha(JSON.stringify(admittedSources.files)) !== admittedSources.sourceConfigSha256) throw Error('Original report bytes/new frozen9 binding differs');
for (const value of [preparedSources, admittedSources]) {
  for (const entry of driverFreeze.files) {
    const file = path.join(path.dirname(path.dirname(path.dirname(topic))), entry.file);
    const matched = value.files.find(row => samePath(row.file, file));
    if (!matched || matched.bytes !== entry.bytes || matched.sha256 !== entry.sha256) throw Error('Recorded source closure does not contain the exact coordinated nine inputs');
  }
}
const scene=await verifySceneContract(realmRoot,'formal');
if(processRecord.sceneProfile!==scene.profile||processRecord.sceneSourceSha256!==scene.sourceSha256||processRecord.scene10Sha256!==scene.probeScene.sha256||processRecord.scene100Sha256!==scene.formalScene.sha256||processRecord.sceneAnalysisSha256!==scene.analysis.sha256||processRecord.patrolSpan!==2||preparedSources.sceneSourceSha256!==scene.sourceSha256||provenance.sceneSourceSha256!==scene.sourceSha256)throw Error('Original halo scene/static identity differs from the frozen source closure');
const strict = { minimumCombatAttacksPerMinute: 18, minimumCombatHitsPerMinute: 12, minimumNativeRunRatio: .5, minimumNativeCellsPerMinute: 80,
  maximumCorrectionRatio: .05, maximumMissedPlanRatio: .05, timeoutExclusive: .01, movementP95Ms: 750, maximumMemorySlopeMiBPerHour: 32 };
if (Object.entries(strict).some(([key, value]) => report.policy[key] !== value) || report.navigationCoordination?.maximumHintCellsPerConnection !== 7 ||
    report.navigationCoordination.scope !== 'Native tool-local conservative union of original AOI obstacles, active controlled owner receipts and one in-flight movement hint per connection; never authoritative occupancy, snapshots or AOI.' ||
    report.navigationCoordination.blockedOpportunityRule !== 'A conflicting prepared move is still an offered navigation-blocked/native opportunity; original cadence, completion and correction gates remain.') throw Error('Original native thresholds/coordination scope differs');
if(JSON.stringify(report.nativePatrolPlanning)!==JSON.stringify(EXPECTED_NATIVE_PATROL_PLANNING)||JSON.stringify(provenance.nativePatrolPlanning)!==JSON.stringify(report.nativePatrolPlanning))throw Error('Original report patrol planning declaration differs from exact pinned source');
for(const [key,value]of [['nativeHpMaintenance',EXPECTED_NATIVE_HP_MAINTENANCE],['nativeAttackWitness',EXPECTED_NATIVE_ATTACK_WITNESS],['nativeChasePreference',EXPECTED_NATIVE_CHASE_PREFERENCE]])if(JSON.stringify(report[key])!==JSON.stringify(value)||JSON.stringify(provenance[key])!==JSON.stringify(value))throw Error('Original HP/witness metadata differs from exact final source');
if(JSON.stringify(report.measurementClock)!==JSON.stringify(EXPECTED_MEASUREMENT_CLOCK)||JSON.stringify(provenance.measurementClock)!==JSON.stringify(report.measurementClock))throw Error('Original measurement-clock declaration differs from the exact final driver');
for(const value of [processRecord,preparedSources,admittedSources,provenance])if(value.coreCodeEvidenceCommit!==CORE_CODE_EVIDENCE_COMMIT||JSON.stringify(value.wireDiagnostics)!==JSON.stringify(wireIdentity))throw Error('External diagnostics/core evidence identity differs');
if(JSON.stringify(cleanup.wireDiagnostics?.identity)!==JSON.stringify(wireIdentity)||JSON.stringify(cleanup.wireDiagnostics?.terminal??null)!==JSON.stringify(wireTerminalPin)||JSON.stringify(provenance.wireTerminal??null)!==JSON.stringify(wireTerminalPin))throw Error('Independent diagnostic terminal raw pin differs');
if(wireTerminal&&wireTerminal.runId!==runId)throw Error('Diagnostic terminal is not bound to this original run');
const soak = report.phaseWindows.find(value => value.name === 'soak');
const failed = report.continuousAcceptance?.firstFailure?.evidence ?? null;
for (const stage of report.stages) {
  assertMeasuredPhase(stage,stage.seconds);
  const cohort = /^stage-(2|10|25|50|100)$/.exec(stage.name);
  if (stage.name === 'soak') { if (stage.seconds !== 7200 || stage.controlledActors !== 100) throw Error('Original7200s soak scope differs'); }
  else if (stage.name === 'post-resume-observation') { if (stage.seconds !== 60 || stage.controlledActors !== 100) throw Error('Original post-resume60s/100 scope differs'); }
  else if (!cohort || stage.seconds !== 300 || stage.controlledActors !== Number(cohort[1])) throw Error('Original five-stage300s scope differs');
}
if (soak && (!Number.isSafeInteger(soak.startedAtMs) || !Number.isSafeInteger(soak.endedAtMs))) throw Error('Original formal soak clock is invalid');

const rawActions = {}, windows = [], typeCounts = {}, windowsByPhase = {}, wireRaw=createWireRawCounter(),minuteWindows=report.ok===true?createPhaseWindowCounter(report,'formal'):null;
const rawMeta = await streamRows(path.join(base, rawRelative), row => {
  wireRaw.observe(row);minuteWindows?.observe(row);
  typeCounts[row.type] = (typeCounts[row.type] ?? 0) + 1;
  if (row.type === 'window') { const phase = windowsByPhase[row.phase] ??= { complete: 0, passed: 0 }; phase.complete++; if (row.assessment?.passed === true) phase.passed++; }
  if (row.type === 'window' && row.phase === 'soak') windows.push({ passed: row.assessment?.passed === true, seconds: row.assessment?.seconds, at: row.at });
  if (row.type === 'action') {
    const key = `${row.phase}:${row.kind}:${row.status}`; rawActions[key] = (rawActions[key] ?? 0) + 1;
  }
});
const wireCapture=validateWireTerminal(wireTerminal,wireSource,wireRaw.summary());
const apiFile = path.join(topic, 'evidence-tools/verify-evidence.mjs');
const api = await import(pathToFileURL(await realPublicFile(apiFile)));
const entry = { kind: 'capacity-samples-jsonl', runId, report: `${child}/${reportRelative}`, path: rawRelative,
  bytes: rawMeta.bytes, sha256: rawMeta.sha256, projection: `${child}/${realm}/reports/${runId}.raw-projection.json` };
safeRelative(entry.report); safeRelative(entry.projection); safeRelative(entry.path, true);
const expectedRaw = await realPublicFile(path.join(base, entry.path), true);
const resolved = await api.resolveArchivedFile(base, entry);
if (!samePath(expectedRaw, resolved)) throw Error('Archive resolver changed the declared raw source');
const streamed = await api.streamCapacitySamples(resolved, entry, api.STREAM_LIMITS, report);
const rawVerifierAccepted = api.verifyCapacityStatus(report, streamed.selected, streamed.monitorSummary);
if (rawVerifierAccepted !== report.playableCapacityAccepted) throw Error('Original report formal acceptance disagrees with the strict frozen verifier');
if (rawVerifierAccepted) {
  api.verifyCapacityPhases(report, streamed.selected);minuteWindows.assertComplete();
  const expectedNames = ['stage-2', 'stage-10', 'stage-25', 'stage-50', 'stage-100', 'soak', 'post-resume-observation'];
  if (JSON.stringify(report.stages.map(value => value.name)) !== JSON.stringify(expectedNames) ||
      report.stages.slice(0, 5).some((stage, index) => stage.seconds !== 300 || stage.controlledActors !== [2, 10, 25, 50, 100][index] || !stage.completed || !stage.passed || stage.endedAtMs - stage.startedAtMs < 300000) ||
      report.stages[5].seconds !== 7200 || !report.stages[5].completed || !report.stages[5].passed || report.stages[5].controlledActors !== 100 ||
      soak?.completed !== true || soak.endedAtMs - soak.startedAtMs < 7200000 ||
      report.stages[6].seconds !== 60 || report.stages[6].controlledActors !== 100 || !report.stages[6].completed || !report.stages[6].passed ||
      report.stages[6].endedAtMs - report.stages[6].startedAtMs < 60000) throw Error('Formal acceptance requires the original five300s stages, complete7200s soak and post-resume60s/100 observation');
}
const capacityAccepted = rawVerifierAccepted && normalOwnedCleanup;

const monitorRelative = `${realm}/monitor-history.jsonl`; let firstMonitor, lastMonitor, terminalMonitor, monitorRows = 0, monitorStoppedRows = 0;
function monitorProjection(sample) {
  return { sampledAt: sample.sampledAt, stop: sample.stop, reason: sample.reason ?? null,
    gatewayTelemetryAvailable: sample.gatewayTelemetryAvailable,
    process: sample.process ? { pid: sample.process.pid, creationTimeMs: sample.process.creationTimeMs,
      rawCounters: sample.process.rawCounters, rates: sample.process.rates } : null,
    memory: sample.memory, health: sample.health,
    zoneMetrics: sample.zoneMetrics ? { version: sample.zoneMetrics.version, complete: sample.zoneMetrics.complete,
      sinceInitializationMs: sample.zoneMetrics.sinceInitializationMs } : null };
}
const monitorMeta = await streamRows(path.join(base, monitorRelative), row => {
  if (row.type === 'monitor') { const projected = monitorProjection(row.sample); firstMonitor ??= projected; lastMonitor = projected; monitorRows++; }
  if (row.type === 'monitorStopped') { terminalMonitor = monitorProjection(row.sample); monitorStoppedRows++; }
});
const monitorTerminalMatchesCleanup = monitorStoppedRows === 1 && terminalMonitor?.stop === true && terminalMonitor.reason === 'Operator requested load drain' &&
  cleanup.monitorTerminal?.sampledAt === terminalMonitor.sampledAt;
if (cleanup.checks?.monitorTerminalPreserved === true && !monitorTerminalMatchesCleanup) throw Error('Claimed normal monitor terminal contradicts the original history');

const recordingDirectory = path.join(realmRoot, 'recordings'), recordingFiles = [];
safeRelative(path.relative(base, recordingDirectory), true);
if (!samePath(await fs.realpath(recordingDirectory), recordingDirectory)) throw Error('Recording directory is redirected');
for (const item of await fs.readdir(recordingDirectory, { withFileTypes: true })) {
  if (!item.isFile() || !item.name.endsWith('.jsonl')) throw Error('Unexpected recording archive entry');
  let minCapturedAtMs, maxCapturedAtMs, firstSequence, lastSequence; const maps = new Set(), schemas = new Set();
  const fileMeta = await streamRows(path.join(recordingDirectory, item.name), frame => {
    if (!Number.isSafeInteger(frame.capturedAtMs) || !Number.isSafeInteger(frame.sequence) || typeof frame.mapFileName !== 'string' || typeof frame.schema !== 'string') throw Error('Invalid recording metadata');
    minCapturedAtMs = Math.min(minCapturedAtMs ?? frame.capturedAtMs, frame.capturedAtMs);
    maxCapturedAtMs = Math.max(maxCapturedAtMs ?? frame.capturedAtMs, frame.capturedAtMs);
    firstSequence ??= frame.sequence; lastSequence = frame.sequence; maps.add(frame.mapFileName); schemas.add(frame.schema);
  });
  recordingFiles.push({ file: item.name, bytes: fileMeta.bytes, sha256: fileMeta.sha256, frames: fileMeta.rows,
    minCapturedAtMs, maxCapturedAtMs, firstSequence, lastSequence, mapFileNames: [...maps], schemas: [...schemas] });
}
let spectatorAfter;
try { spectatorAfter = await read(path.join(realmRoot, 'spectator-health-after.json')); } catch (error) { if (error.code !== 'ENOENT') throw error; }
const recordingSummary = { schemaVersion: 1, createdAt: new Date().toISOString(), runId, sourceCommit: release.sourceCommit, backendSourceCommit, driverSourceCommit,
  scope: 'Post-stop streaming size/SHA/count and frame-header metadata only. No replay/entity payload copied.',
  directory: recordingDirectory, fileCount: recordingFiles.length, totalFrames: recordingFiles.reduce((sum, file) => sum + file.frames, 0),
  totalBytes: recordingFiles.reduce((sum, file) => sum + file.bytes, 0), healthPersistedFramesTotal: spectatorAfter?.metrics?.persistedFramesTotal ?? null,
  recordingErrorsTotal: spectatorAfter?.metrics?.recordingErrorsTotal ?? null, files: recordingFiles, rawVerifierAccepted, normalOwnedCleanup, capacityAccepted };
recordingSummary.persistedCounterMatchesFiles = recordingSummary.healthPersistedFramesTotal === null ? null : recordingSummary.totalFrames === recordingSummary.healthPersistedFramesTotal;
const recordingSummaryFile = path.join(base, 'collect-formal-native-v2-scout-observer-ladder-14-recordings-metadata.json');
await writeNew(recordingSummaryFile, recordingSummary);
const monitorEndpointsFile = path.join(base, 'collect-formal-native-v2-scout-observer-ladder-14-monitor-endpoints.json');
await writeNew(monitorEndpointsFile, { schemaVersion: 1, runId, scope: 'Whitelisted first/last public resource samples and original stop terminal when present. Full monitor raw remains external; normal cleanup is adjudicated separately.',
  source: { path: monitorRelative, ...monitorMeta }, monitorRows, monitorStoppedRows, firstMonitor, lastMonitor, terminalMonitor, monitorTerminalMatchesCleanup, normalOwnedCleanup, capacityAccepted });

const realmFiles = ['gateway-build-pin.json', 'source-pins.json', 'source-pins-admitted-v2.json', 'realm-process.json', 'realm-cleanup.json',
  'slow-stage-public.jsonl', 'slow-stage-public-status.json', 'slow-stage-diagnostics-config.json', 'slow-stage-public.mjs', 'slow-stage-schema-contract.json', 'candidate-release-binding.json', 'full16-gate.json', 'slow-stage-row-contract.mjs', 'verify-full16-gate.mjs', 'projection-build-source-lib.mjs',
  'driver-evidence-contract.mjs', 'scene-binding.json', 'scene-contract.mjs', 'backend-driver-source-guard.mjs', 'measured-phase-contract.mjs', 'resource-profile.json', 'run-options.json', 'run-options-admitted-v2.json', 'scenario.json', 'provisioning.json',
  'spectator-health-before.json', 'monitor-options.json', 'monitor-process.json',
  'launcher-source-frozen-v1.json', 'launcher-source-prepared-v1.json', 'driver-report-contract.mjs', 'preparation-plan.json', 'final-inputs-approved.json', 'report-provenance.json', 'feasibility-gate.json', 'static-preparation-verification.json', 'source-guard-readonly-verification.json', 'scenario-prefix-readiness.json',
  'initialize-private.ps1', 'input-lib.mjs', 'prepare-inputs.mjs', 'run-owned.mjs', 'verify-feasibility-gate.mjs', 'monitor-owned.mjs', 'request-stop.mjs', 'recover-drained-owned.ps1',
  `reports/${runId}.report.json`];
for (const name of ['launcher-syntax-check.log', 'run-command.log', 'spectator-health-after.json']) { try { await fs.access(path.join(realmRoot, name)); realmFiles.push(name); } catch (error) { if (error.code !== 'ENOENT') throw error; } }
realmFiles.push('phase-window-contract.mjs','hp-maintenance-status-schema.mjs','formal-template-contract.mjs','pending-formal14-binding-v1.json','wire-diagnostics-contract.mjs','wire-diagnostics-adapter.mjs','verify-probe18-collected-evidence.mjs','driver-binding-contract.mjs','current-driver-binding.json','native-chase-report-contract.mjs','backend-library-binding.json','scene-driver-compatibility.mjs','scene-driver-compatibility-proof-v1.json');if(wireTerminalPin)realmFiles.push(WIRE_TERMINAL);
const sourcePaths = [`${realm}/collect-public.mjs`, `${realm}/template-derivation-v1.json`, ...wireSource.files.map(p=>path.relative(base,p.file).split(path.sep).join('/')),...realmFiles.map(file => `${realm}/${file}`), 'release-account-store-wait-diagnostics-v1.json', 'backend-source-account-store-wait-diagnostics-v1.json', 'account-store-wait-diagnostics-v1-build-receipt.json',
  'capacity-scout-observer-preference-driver-frozen-v1.json',
  ...scene.publicSourceRelatives,
  'collect-formal-native-v2-scout-observer-ladder-14-recordings-metadata.json', 'collect-formal-native-v2-scout-observer-ladder-14-monitor-endpoints.json',
  'release-native-v2-scout-observer-ladder-14/collect-public.mjs', 'release-native-v2-scout-observer-ladder-14/verify-public.mjs'];
const evidenceBasis=await verifyDriverEvidenceBasis(driverFreeze,assertPin);sourcePaths.push(...evidenceBasis.pins.map(p=>path.relative(base,p.file).split(path.sep).join('/')));
const runtimeLauncher=await read(path.join(realmRoot,'launcher-source-frozen-v1.json'));if(!Array.isArray(runtimeLauncher.files)||runtimeLauncher.files.length>128)throw Error('Runtime launcher source closure missing or oversized');for(const p of runtimeLauncher.files){const relative=safeRelative(path.relative(base,p.file).split(path.sep).join('/'));const actual=await rawDigest(p.file);if(actual.bytes!==p.bytes||actual.sha256!==p.sha256)throw Error('Runtime sealed source changed before collection');sourcePaths.push(relative);}
// No optional arbitrary analysis path: only the explicit public allowlist is copied.

const originals = [];
for (const relative of [...new Set(sourcePaths)]) {
  const normalized = safeRelative(relative), absolute = path.join(base, ...normalized.split('/'));
  if (relative.endsWith('.json')) redactCheck(await read(absolute));
  originals.push({ path: relative, ...await rawDigest(absolute), classification: 'explicit-public-original' });
}
const catalog = { schemaVersion: 1, createdAt: new Date().toISOString(), archiveRoot: base,
  scope: 'Independent explicit public allowlist for authentic formal standard native compact-wire span2-halo ladder14. Original partial/failure/pass report bytes are retained; strict frozen verifier plus authentic normal cleanup adjudicates formal acceptance. No private store, secret, EXE or recording payload copied.',
  files: originals, externalArchives: [{ path: rawRelative, bytes: rawMeta.bytes, sha256: rawMeta.sha256, kind: 'capacity-samples-jsonl', copyToRepository: false },
    { path: monitorRelative, bytes: monitorMeta.bytes, sha256: monitorMeta.sha256, rows: monitorMeta.rows, kind: 'resource-monitor-history-jsonl', copyToRepository: false },
    ...recordingFiles.map(file => ({ path: `${realm}/recordings/${file.file}`, bytes: file.bytes, sha256: file.sha256, frames: file.frames, kind: 'spectator-recording-jsonl', copyToRepository: false }))],
  run: { runId, wireDiagnostics:{identity:wireIdentity,terminal:wireTerminalPin,capture:wireCapture}, coreCodeEvidenceCommit:CORE_CODE_EVIDENCE_COMMIT, sourceCommit: release.sourceCommit, backendSourceCommit, executionSourceCommit:driverSourceCommit, driverSourceCommit, driverSourceSha256: driverFreeze.sourceSha256, socketDiagnosticsReleaseUsed: true, sceneProfile: scene.profile, sceneSourceSha256: scene.sourceSha256, scene10Sha256: scene.probeScene.sha256, scene100Sha256: scene.formalScene.sha256, sceneStaticAnalysis: scene.analysis, reportOk: report.ok, driverPlayableCapacityAccepted: report.playableCapacityAccepted, rawVerifierAccepted, normalProcessCleanup, diagnosticsComplete, slowStageMetadata: { rows: slowMeta.rows, bytes: slowMeta.bytes, sha256: slowMeta.sha256, status: slowStatus }, normalOwnedCleanup, navigationCoordination: report.navigationCoordination, nativePatrolPlanning:report.nativePatrolPlanning, measurementClock:report.measurementClock, nativeHpMaintenance:report.nativeHpMaintenance, nativeAttackWitness:report.nativeAttackWitness, nativeChasePreference:report.nativeChasePreference, gatewaySha256: release.binary.sha256, backendInputCount: manifest.files.length,
    realGameplayAttempt: true, report: reportRelative, raw: rawRelative, peakControlledActors: report.peakControlledActors, highestPassedStage: report.highestPassedStage, phaseWindows: report.phaseWindows, stages: report.stages.map(stage => ({ name: stage.name, controlledActors: stage.controlledActors, seconds: stage.seconds, measuredSeconds: stage.measuredSeconds, completed: stage.completed, passed: stage.passed, startedAtMs: stage.startedAtMs, endedAtMs: stage.endedAtMs, minuteWindows: stage.rollingWindows?.length ?? 0 })),
    soakIntervalSeconds: soak ? (soak.endedAtMs - soak.startedAtMs) / 1000 : null, soakCompleted: soak?.completed === true, completed60SecondSoakWindows: windows.length,
    passed60SecondSoakWindows: windows.filter(window => window.passed).length, windowsByPhase, capacityAccepted, formalRunStarted: true,
    failureWindow: failed ? { actor: failed.actor, phases: failed.phases, seconds: failed.seconds, role: failed.role,
      successfulMoves: failed.activity?.moves, correctedMoves: (failed.metrics?.walk?.status.corrected ?? 0) + (failed.metrics?.run?.status.corrected ?? 0),
      offeredMoves: (failed.metrics?.walk?.count ?? 0) + (failed.metrics?.run?.count ?? 0), movementCorrectionRatio: failed.movementCorrectionRatio,
      attackRejectionRatio: failed.attackRejectionRatio, timeoutRatio: failed.timeoutRatio,
      launchedAttacks: failed.activity?.attacks, positiveHits: failed.activity?.hits, damage: failed.activity?.damage,
      v2Requests: failed.metrics?.attack?.attackV2Requests, notLaunched: failed.metrics?.attack?.attackV2NotLaunched } : null,
    allSixCleanupCountersZero: counters.every(key => cleanup.preStopCapacity?.[key] === 0), allOwnedProcessesStopped: true, normalGatewayExit: cleanup.gatewayExit?.code ?? null, normalMonitorExit: cleanup.monitorExit?.code ?? null },
  limitations: ['The new compact span2 halo scene is independently bound; historical scene passes are not inherited and static geometry does not prove live navigation or supply.',
    'Failure-window counters, if any, belong only to the recorded first failing continuous window, not the entire run.',
    'Formal policy remains five300s measured cohorts2/10/25/50/100 plus7200s soak, original five resumes, post-resume60s/100 observation, five saves and all original latency/error/activity gates. Partial runs and shorter windows never substitute for full scope.',
    'Typed Launched proves only this command launched a swing. The original driver still requires paired positive damage/health evidence for a hit.',
    'The first/last monitor projection is a summary. Full raw monitor bytes and spectator payloads remain external and independently SHA-bound.',
    'This File/loopback single-host formal profile does not certify production storage/RPO, TLS/NIC bandwidth, frontend acceptance or Cell scheduling.'] };
const catalogName = 'collect-formal-native-v2-scout-observer-ladder-14-public-artifact-catalog.json';
await writeNew(path.join(base, catalogName), catalog);
originals.push({ path: catalogName, ...await rawDigest(path.join(base, catalogName)), classification: 'derived-public-catalog' });
await realDirectory(topic); safeRelative(child); await fs.mkdir(dest); await realDirectory(dest);
const copied = [];
for (const original of originals) {
  const normalized = safeRelative(original.path), bytes = await fs.readFile(await realPublicFile(path.join(base, ...normalized.split('/'))));
  if (bytes.length !== original.bytes || sha(bytes) !== original.sha256) throw Error('Original public source changed before copy');
  const target = path.join(dest, ...normalized.split('/')); await fs.mkdir(path.dirname(target), { recursive: true });
  await fs.writeFile(await publicOutputFile(target), bytes, { flag: 'wx' });
  const actual = await rawDigest(target);
  if (actual.bytes !== original.bytes || actual.sha256 !== original.sha256) throw Error('Public copy differs');
  copied.push({ path: `${child}/${original.path}`, source: original.path, ...actual });
}
await writeNew(path.join(topic, entry.projection), api.makeCapacityProjection(runId, streamed));
await writeNew(path.join(dest, 'raw-artifact-manifest.json'), { schemaVersion: 1, files: [entry] });
await writeNew(path.join(dest, 'public-copy-provenance.json'), { schemaVersion: 1, createdAt: new Date().toISOString(), hashMode: 'raw-bytes',
  scope: 'Exact explicit public allowlist only. Full capacity raw, monitor raw and recording payload remain external. No private/store/credentials/EXE payload copied.',
  files: copied, allOriginalCopiesByteEqual: true, structurallyUnredactedSensitiveJsonFields: 0,
  privateSecretValuesReadOrClaimedChecked: false });
const summary = { schemaVersion: 1, createdAt: new Date().toISOString(), runId,
  method: 'Posthoc file-only stable streaming SHA/metadata, explicit allowlist, structural JSON redaction, exact-copy and existing frozen raw verifier API. No build, suite, game module import, service or load.',
  originalCopies: copied.length, originalCopyBytes: copied.reduce((sum, file) => sum + file.bytes, 0),
  raw: { bytes: streamed.bytes, sha256: streamed.sha256, rows: streamed.rows, monitorCount: streamed.monitorSummary.count,
    validMonitorCount: streamed.monitorSummary.validCount, invalidMonitorCount: streamed.monitorSummary.invalidCount,
    stoppedMonitorCount: streamed.monitorSummary.stoppedCount, maxGapMs: streamed.monitorSummary.maxGapMs, phaseBrackets: streamed.monitorSummary.phaseBrackets },
  wireDiagnostics:{identity:wireIdentity,terminal:wireTerminalPin,capture:wireCapture}, rawTypeCounts: typeCounts, actionCountsByPhaseKindStatus: rawActions, run: catalog.run,
  monitor: { bytes: monitorMeta.bytes, sha256: monitorMeta.sha256, rows: monitorMeta.rows, monitorRows, normalTerminalRows: monitorStoppedRows },
  spectator: { files: recordingSummary.fileCount, frames: recordingSummary.totalFrames, bytes: recordingSummary.totalBytes,
    errors: recordingSummary.recordingErrorsTotal, persistedCounterMatchesFiles: recordingSummary.persistedCounterMatchesFiles },
  apiVerifier: { file: path.relative(topic, apiFile).split(path.sep).join('/'), ...await rawDigest(apiFile), accepted: rawVerifierAccepted },
  capacityAccepted, rawVerifierAccepted, normalProcessCleanup, diagnosticsComplete, normalOwnedCleanup, backendSourceCommit, driverSourceCommit, originalVerdictPreserved: true, rootIndexUpdated: false, existingTopicsModified: false };
redactCheck(summary);
await writeNew(path.join(dest, 'collection-verification.json'), summary);
await writeNew(path.join(base, 'collect-formal-native-v2-scout-observer-ladder-14-collection-result.json'), { destination: dest, summary, manifest: entry });
console.log(JSON.stringify({ destination: dest, originalCopies: copied.length, raw: summary.raw, spectator: summary.spectator,
  failureWindow: catalog.run.failureWindow, reportOk: report.ok, rawVerifierAccepted, normalOwnedCleanup, capacityAccepted, rootIndexUpdated: false }));
