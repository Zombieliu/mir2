import {assertMeasuredPhase} from './measured-phase-contract.mjs';
import { verifySceneContract } from './scene-contract.mjs';
import { validateSlowStagePublicRow } from './slow-stage-row-contract.mjs';
// Independent post-copy verification. No final topic index is created.
import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { createInterface } from 'node:readline';
import { ALLOWED_STAGES, MAX_LINE_BYTES, MAX_PUBLIC_BYTES, MAX_WRITE_BUFFER_BYTES } from './slow-stage-public.mjs';
import { EXPECTED_DRIVER_SOURCE_COMMIT, EXPECTED_DRIVER_SOURCE_SHA256, EXPECTED_NATIVE_PATROL_PLANNING, EXPECTED_MEASUREMENT_CLOCK } from './driver-report-contract.mjs';
if(!/^[a-f0-9]{40}$/.test(EXPECTED_DRIVER_SOURCE_COMMIT)||!/^[a-f0-9]{64}$/.test(EXPECTED_DRIVER_SOURCE_SHA256)||!EXPECTED_NATIVE_PATROL_PLANNING)throw Error('Verifier template needs the final reviewed patrol driver identity');
const base = 'C:/mir2-build/p01-completion-20261024';
const topic = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/docs/generated/cell-p0-p1-completion-20261024';
if (process.argv.slice(2).join(' ') !== '--verify-collected-public') throw Error('Explicit post-cleanup public verification argument required');
const realm = 'release-native-10-v2-halo-feasibility-09';
const expectedPolicy = {
  "profile": "probe",
  "activityMode": "native",
  "movementIntervalMs": 750,
  "stages": [
    2,
    10
  ],
  "stageSeconds": 300,
  "soakSeconds": 360,
  "wallSeconds": 3600,
  "layout": "dispersed",
  "combatActors": 1,
  "resumeSamples": 0,
  "saveSamples": 0,
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
  "fullAcceptanceEligible": false
};
const counters = ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases', 'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight'];
const child = 'capacity-native10-v2-halo-diagnostics', directory = path.join(topic, child), sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const samePath = (a, b) => path.resolve(a).toLowerCase() === path.resolve(b).toLowerCase();
const privatePart = /^(?:private|secrets?|credentials?)$/i;
function safeRelative(value, archive = false) {
  assert.ok(typeof value === 'string' && value.length > 0 && !path.isAbsolute(value) && !path.win32.isAbsolute(value) && !path.posix.isAbsolute(value));
  const parts = value.split(/[\\/]/);
  assert.ok(parts.every(part => part && part !== '.' && part !== '..' && !/[\x00-\x1f:]/.test(part) && !/[. ]$/.test(part) && !privatePart.test(part)));
  if (!archive) assert.ok(!parts.some(part => /^recordings$/i.test(part)) && !/\.(?:exe|samples\.jsonl)$/i.test(parts.at(-1)));
  return parts.join('/');
}
async function realDirectory(root) {
  const actual = await fs.realpath(root), stat = await fs.lstat(root);
  assert.ok(samePath(actual, root) && stat.isDirectory() && !stat.isSymbolicLink(), 'Redirected evidence root or directory');
  return actual;
}
async function fileAt(root, relative, archive = false) {
  const normalized = safeRelative(relative, archive), rootReal = await realDirectory(root);
  const requested = path.resolve(root, ...normalized.split('/')), actual = await fs.realpath(requested);
  const resolvedRelative = safeRelative(path.relative(rootReal, actual), archive);
  assert.ok(samePath(actual, requested) && samePath(path.resolve(rootReal, ...resolvedRelative.split('/')), requested), 'Evidence escaped or redirected its declared root');
  const stat = await fs.lstat(actual); assert.ok(stat.isFile() && !stat.isSymbolicLink());
  return actual;
}
const readAt = async (root, relative) => JSON.parse((await fs.readFile(await fileAt(root, relative), 'utf8')).replace(/^\uFEFF/, ''));
const api = await import(pathToFileURL(await fileAt(topic, 'evidence-tools/verify-evidence.mjs')));
async function digestAt(root, relative, archive = false) {
  const file = await fileAt(root, relative, archive);
  const before = await fs.lstat(file); assert.ok(before.isFile() && !before.isSymbolicLink());
  let bytes = 0; const hash = crypto.createHash('sha256');
  for await (const chunk of createReadStream(file)) { bytes += chunk.length; hash.update(chunk); }
  const after = await fs.stat(file); assert.equal(bytes, before.size); assert.equal(after.size, before.size); assert.equal(after.mtimeMs, before.mtimeMs);
  return { bytes, sha256: hash.digest('hex') };
}
async function safeInventory(root) {
  await realDirectory(root); const rows = [];
  async function walk(relative = '') {
    const here = relative ? path.join(root, ...safeRelative(relative).split('/')) : root;
    await realDirectory(here);
    for (const entry of await fs.readdir(here, { withFileTypes: true })) {
      const next = safeRelative(relative ? relative + '/' + entry.name : entry.name);
      assert.ok(!entry.isSymbolicLink(), 'Symbolic evidence entry');
      if (entry.isDirectory()) await walk(next);
      else { assert.ok(entry.isFile()); rows.push({ path: next, ...await digestAt(root, next) }); }
    }
  }
  await walk(); return rows.sort((a, b) => a.path.localeCompare(b.path, 'en'));
}
async function writeNewAt(root, relative, value) {
  const normalized = safeRelative(relative), rootReal = await realDirectory(root), target = path.resolve(root, ...normalized.split('/'));
  assert.ok(samePath(await fs.realpath(path.dirname(target)), path.dirname(target)) && !path.relative(rootReal, target).startsWith('..'));
  await fs.writeFile(target, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
}
function redactCheck(value) {
  if (!value || typeof value !== 'object') return;
  for (const [key, item] of Object.entries(value)) {
    if (/^(?:password|secret|token|credential|authorization|cookie|assertion|privateKey)$/i.test(key) && item != null) assert.equal(item, '[redacted]');
    redactCheck(item);
  }
}
const inventory = await safeInventory(directory), provenance = await readAt(directory, 'public-copy-provenance.json');
assert.ok(provenance.files.length > 0); let jsonFiles = 0;
for (const file of inventory) {
  safeRelative(file.path);
  if (file.path.endsWith('.json')) { redactCheck(await readAt(directory, file.path)); jsonFiles++; }
}
for (const file of provenance.files) {
  const normalized = safeRelative(file.path); assert.ok(normalized.startsWith(child + '/'));
  const relative = safeRelative(normalized.slice(child.length + 1)), source = safeRelative(file.source);
  const local = await digestAt(directory, relative), original = await digestAt(base, source);
  assert.deepEqual(local, { bytes: file.bytes, sha256: file.sha256 }); assert.deepEqual(original, local);
}
const catalog = await readAt(directory, 'collect-release-native-10-v2-halo-feasibility-09-public-artifact-catalog.json');
const cleanup = await readAt(directory, realm + '/realm-cleanup.json');
const processRecord = await readAt(directory, realm + '/realm-process.json');
const scene=await verifySceneContract(path.join(directory,realm),'probe');
assert.equal(processRecord.sceneProfile,scene.profile);assert.equal(processRecord.sceneSourceSha256,scene.sourceSha256);assert.equal(processRecord.scene10Sha256,scene.probeScene.sha256);assert.equal(processRecord.scene100Sha256,scene.formalScene.sha256);assert.equal(processRecord.sceneAnalysisSha256,scene.analysis.sha256);assert.equal(processRecord.patrolSpan,4);assert.equal(catalog.run.sceneSourceSha256,scene.sourceSha256);
assert.deepEqual(processRecord.explicitAdmissionCaps,[24,10,10,4,1,2]);assert.equal(processRecord.tokioWorkers,4);assert.equal(processRecord.slowStageThresholdMs,50);
assert.equal(processRecord.content,'platinum_176');assert.equal(processRecord.routing,'single');assert.equal(processRecord.accountBackend,'file');
assert.ok(cleanup.checks?.processAbsent === true && cleanup.checks?.portsFree === true && !cleanup.remainingOwnedPortListeners?.length && terminalExit(cleanup.gatewayExit) && terminalExit(cleanup.monitorExit));
assert.equal(provenance.files.length, catalog.files.length + 1, 'exact public allowlist plus its catalog; no hardcoded copy count');
for (const file of catalog.externalArchives) {
  const relative = safeRelative(file.path, true);
  assert.ok(['capacity-samples-jsonl', 'resource-monitor-history-jsonl', 'spectator-recording-jsonl'].includes(file.kind) && relative.endsWith('.jsonl'));
  assert.deepEqual(await digestAt(base, relative, true), { bytes: file.bytes, sha256: file.sha256 });
}
const manifest = await readAt(directory, 'raw-artifact-manifest.json'); assert.equal(manifest.files.length, 1);
const entry = manifest.files[0], reportPath = safeRelative(entry.report), projectionPath = safeRelative(entry.projection), rawPath = safeRelative(entry.path, true);
assert.ok(reportPath.startsWith(child + '/') && projectionPath.startsWith(child + '/'));
const report = await readAt(directory, reportPath.slice(child.length + 1));
for(const stage of report.stages)assertMeasuredPhase(stage,stage.seconds);
const expectedRaw = await fileAt(base, rawPath, true), raw = await api.resolveArchivedFile(base, { ...entry, path: rawPath });
assert.ok(samePath(raw, expectedRaw));
const streamed = await api.streamCapacitySamples(expectedRaw, entry, api.STREAM_LIMITS, report);
const rawVerifierAccepted = api.verifyCapacityStatus(report, streamed.selected, streamed.monitorSummary);
function terminalExit(value) { return value && (Number.isInteger(value.code) || (typeof value.signal === 'string' && value.signal.length > 0) || value.spawnError === true); }
const cleanupKeys = ['sixCountersZero', 'normalOwnedShutdown', 'processAbsent', 'portsFree', 'monitorStoppedBeforeGateway', 'monitorExitZero', 'monitorTerminalPreserved'];
const normalProcessCleanup = cleanupKeys.every(key => cleanup.checks?.[key] === true) && counters.every(key => cleanup.preStopCapacity?.[key] === 0) &&
  cleanup.gatewayExit?.code === 0 && cleanup.monitorExit?.code === 0 && !cleanup.remainingOwnedPortListeners?.length && !cleanup.inputsChangedWhileRunning && !cleanup.error;
const slowStatus = await readAt(directory, realm + '/slow-stage-public-status.json');
assert.equal(slowStatus.schemaVersion, 1); assert.ok(Array.isArray(slowStatus.reasons));
const slowFile = await fileAt(directory, realm + '/slow-stage-public.jsonl'), slowStat = await fs.stat(slowFile);
assert.ok(slowStat.size <= MAX_PUBLIC_BYTES); let slowRows = 0;
for await (const line of createInterface({ input: createReadStream(slowFile), crlfDelay: Infinity })) {
  assert.ok(Buffer.byteLength(line) <= MAX_LINE_BYTES); const row = JSON.parse(line); slowRows++;
  validateSlowStagePublicRow(row,line);
}
assert.equal(slowStatus.lineLimitBytes, MAX_LINE_BYTES); assert.equal(slowStatus.totalLimitBytes, MAX_PUBLIC_BYTES);
assert.equal(slowStatus.writeLimitBytes, MAX_WRITE_BUFFER_BYTES); assert.equal(slowStatus.writeHighWaterMarkBytes, MAX_WRITE_BUFFER_BYTES); assert.equal(slowStatus.thresholdMs, 50);
assert.ok(Number.isSafeInteger(slowStatus.maxBufferedLineBytes) && slowStatus.maxBufferedLineBytes >= 0 && slowStatus.maxBufferedLineBytes <= MAX_LINE_BYTES &&
  Number.isSafeInteger(slowStatus.maxQueuedWriteBytes) && slowStatus.maxQueuedWriteBytes >= 0 && slowStatus.maxQueuedWriteBytes <= MAX_WRITE_BUFFER_BYTES);
const { thresholdMs, dataFile, ...runtimeSlowStatus } = slowStatus;
assert.deepEqual(runtimeSlowStatus, cleanup.slowStageDiagnostics);
const diagnosticsComplete = slowStatus.complete === true && slowStatus.stderrEnded === true && slowStatus.writerFinished === true &&
  slowStatus.stopRequested === false && slowStatus.reasons.length === 0 && slowStatus.rowsQueued === slowRows && slowStatus.bytesQueued === slowStat.size;
if (slowStatus.complete === true) assert.ok(diagnosticsComplete);
const normalOwnedCleanup = normalProcessCleanup && diagnosticsComplete;
assert.equal(catalog.run.normalProcessCleanup, normalProcessCleanup); assert.equal(catalog.run.diagnosticsComplete, diagnosticsComplete);

const capacityAccepted = false; assert.equal(rawVerifierAccepted,false);
assert.equal(rawVerifierAccepted, report.playableCapacityAccepted);
assert.equal(catalog.run.rawVerifierAccepted, rawVerifierAccepted); assert.equal(catalog.run.normalOwnedCleanup, normalOwnedCleanup); assert.equal(catalog.run.capacityAccepted, capacityAccepted);
assert.deepEqual(report.policy, expectedPolicy);
if(report.ok===true){
 api.verifyCapacityPhases(report,streamed.selected);
 const expected=[['stage-2',2,300,5],['stage-10',10,300,5],['soak',10,360,6]];
 assert.deepEqual(report.stages.map(s=>s.name),expected.map(s=>s[0]));
 for(const[name,count,seconds,required]of expected){const stage=report.stages.find(s=>s.name===name),rawWindows=streamed.selected.filter(r=>r.type==='window'&&r.phase===name);
  assert.ok(stage.completed&&stage.passed&&stage.controlledActors===count&&stage.seconds===seconds&&stage.endedAtMs-stage.startedAtMs>=seconds*1000);
  assert.equal(stage.rollingWindows.length,required);assert.equal(rawWindows.length,required);
  assert.ok(stage.rollingWindows.every((w,j)=>w.passed&&w.seconds>=60&&w.elapsedSeconds>=(j+1)*60)&&rawWindows.every(w=>w.assessment?.passed===true&&w.assessment.seconds>=60));
 }
 assert.ok(streamed.monitorSummary.complete&&!streamed.monitorSummary.invalidCount&&!streamed.monitorSummary.stoppedCount&&streamed.monitorSummary.maxGapMs<=15000);
}
assert.deepEqual(await readAt(directory, projectionPath.slice(child.length + 1)), api.makeCapacityProjection(entry.runId, streamed));
assert.deepEqual(report.nativePatrolPlanning,EXPECTED_NATIVE_PATROL_PLANNING);assert.deepEqual(catalog.run.nativePatrolPlanning,EXPECTED_NATIVE_PATROL_PLANNING);
assert.equal(report.ok, catalog.run.reportOk); assert.equal(report.attackResults.mode, 'v2');
assert.equal(catalog.run.backendSourceCommit, '46880bd1ef415131c4cf85aeb4dd2fa4f6fd4f9b');
assert.equal(catalog.run.driverSourceCommit, EXPECTED_DRIVER_SOURCE_COMMIT);
assert.equal(catalog.run.driverSourceSha256, EXPECTED_DRIVER_SOURCE_SHA256);
assert.equal(report.navigationCoordination.maximumHintCellsPerConnection, 7); assert.equal(report.policy.fullAcceptanceEligible, false);
const result = { schemaVersion: 1, verifiedAt: new Date().toISOString(), runId: entry.runId,
  method: 'Independent read-only public inventory, explicit-copy raw SHA, external raw/monitor/recording SHA, structural credential-field scan and frozen verifier status/projection round trip.',
  originalCopiesVerified: provenance.files.length, publicFilesVerifiedBeforeThisReceipt: inventory.length, publicJsonFilesStructurallyChecked: jsonFiles,
  externalArchivesVerified: catalog.externalArchives.length, rawBytes: streamed.bytes, rawSha256: streamed.sha256, rawRows: streamed.rows,
  invalidRawMonitorRows: streamed.monitorSummary.invalidCount, originalVerdictPreserved: true, reportOk: report.ok, backendSourceCommit: catalog.run.backendSourceCommit, driverSourceCommit: catalog.run.driverSourceCommit, rawVerifierAccepted, normalProcessCleanup, diagnosticsComplete, normalOwnedCleanup, capacityAccepted,
  privateSecretValuesReadOrClaimedChecked: false, finalIndexCreated: false, existingTopicsModified: false };
assert.deepEqual(report.measurementClock,EXPECTED_MEASUREMENT_CLOCK);assert.deepEqual(catalog.run.measurementClock,EXPECTED_MEASUREMENT_CLOCK);
const sidecar = await readAt(directory, realm + '/report-provenance.json'), originalReport = await digestAt(directory, reportPath.slice(child.length + 1));
assert.equal(sidecar.report.bytes, originalReport.bytes); assert.equal(sidecar.report.sha256, originalReport.sha256);
assert.deepEqual(sidecar.measurementClock,EXPECTED_MEASUREMENT_CLOCK);assert.equal(sidecar.backendSourceCommit, catalog.run.backendSourceCommit); assert.equal(sidecar.driverSourceCommit, catalog.run.driverSourceCommit);
assert.equal(sidecar.driverSourceSha256, catalog.run.driverSourceSha256); assert.equal(sidecar.capacityAcceptanceAdjudicated, false);
assert.deepEqual(sidecar.nativePatrolPlanning, EXPECTED_NATIVE_PATROL_PLANNING);
await writeNewAt(directory, 'public-verification.json', result);
await writeNewAt(base, 'collect-release-native-10-v2-halo-feasibility-09-public-verification-result.json', result);
console.log(JSON.stringify(result));
