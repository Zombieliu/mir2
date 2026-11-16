// Independent post-copy verification. No final topic index is created.
import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
const base = 'C:/mir2-build/p01-completion-20261024';
const topic = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/docs/generated/cell-p0-p1-completion-20261024';
if (process.argv.slice(2).join(' ') !== '--verify-collected-public') throw Error('Explicit post-cleanup public verification argument required');
const realm = 'formal-native-v2-ladder-02';
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
const counters = ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases', 'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight'];
const child = 'capacity-formal-native-v2-ladder-02', directory = path.join(topic, child), sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
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
const catalog = await readAt(directory, 'collect-formal-native-v2-ladder-02-public-artifact-catalog.json');
const cleanup = await readAt(directory, realm + '/realm-cleanup.json');
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
const expectedRaw = await fileAt(base, rawPath, true), raw = await api.resolveArchivedFile(base, { ...entry, path: rawPath });
assert.ok(samePath(raw, expectedRaw));
const streamed = await api.streamCapacitySamples(expectedRaw, entry, api.STREAM_LIMITS, report);
const rawVerifierAccepted = api.verifyCapacityStatus(report, streamed.selected, streamed.monitorSummary);
function terminalExit(value) { return value && (Number.isInteger(value.code) || (typeof value.signal === 'string' && value.signal.length > 0) || value.spawnError === true); }
const cleanupKeys = ['sixCountersZero', 'normalOwnedShutdown', 'processAbsent', 'portsFree', 'monitorStoppedBeforeGateway', 'monitorExitZero', 'monitorTerminalPreserved'];
const normalOwnedCleanup = cleanupKeys.every(key => cleanup.checks?.[key] === true) && counters.every(key => cleanup.preStopCapacity?.[key] === 0) &&
  cleanup.gatewayExit?.code === 0 && cleanup.monitorExit?.code === 0 && !cleanup.remainingOwnedPortListeners?.length && !cleanup.inputsChangedWhileRunning && !cleanup.error;
const capacityAccepted = rawVerifierAccepted && normalOwnedCleanup;
assert.equal(rawVerifierAccepted, report.playableCapacityAccepted);
assert.equal(catalog.run.rawVerifierAccepted, rawVerifierAccepted); assert.equal(catalog.run.normalOwnedCleanup, normalOwnedCleanup); assert.equal(catalog.run.capacityAccepted, capacityAccepted);
assert.deepEqual(report.policy, expectedPolicy);
if (rawVerifierAccepted) {
  api.verifyCapacityPhases(report, streamed.selected);
  assert.deepEqual(report.stages.map(stage => stage.name), ['stage-2', 'stage-10', 'stage-25', 'stage-50', 'stage-100', 'soak', 'post-resume-observation']);
  report.stages.slice(0, 5).forEach((stage, index) => assert.ok(stage.completed && stage.passed && stage.seconds === 300 && stage.controlledActors === [2, 10, 25, 50, 100][index] && stage.endedAtMs - stage.startedAtMs >= 300000));
  const soak = report.stages[5]; assert.ok(soak.completed && soak.passed && soak.seconds === 7200 && soak.controlledActors === 100 && soak.endedAtMs - soak.startedAtMs >= 7200000);
  const post = report.stages[6]; assert.ok(post.completed && post.passed && post.seconds === 60 && post.controlledActors === 100 && post.endedAtMs - post.startedAtMs >= 60000);
}
assert.deepEqual(await readAt(directory, projectionPath.slice(child.length + 1)), api.makeCapacityProjection(entry.runId, streamed));
assert.equal(report.ok, catalog.run.reportOk); assert.equal(report.attackResults.mode, 'v2');
assert.equal(catalog.run.backendSourceCommit, '461f1d85eed6309a55a88bd79a0022f7b97febe4');
assert.equal(catalog.run.driverSourceCommit, 'c42f860e0544f0687a5ccf5ddaccad4daecc8acc');
assert.equal(catalog.run.driverSourceSha256, '35868583cbacd7215bf3eadee0e8d6781ed1abe4f21bc40c0c0255d24797faae');
assert.equal(report.navigationCoordination.maximumHintCellsPerConnection, 7); assert.equal(report.policy.fullAcceptanceEligible, true);
const result = { schemaVersion: 1, verifiedAt: new Date().toISOString(), runId: entry.runId,
  method: 'Independent read-only public inventory, explicit-copy raw SHA, external raw/monitor/recording SHA, structural credential-field scan and frozen verifier status/projection round trip.',
  originalCopiesVerified: provenance.files.length, publicFilesVerifiedBeforeThisReceipt: inventory.length, publicJsonFilesStructurallyChecked: jsonFiles,
  externalArchivesVerified: catalog.externalArchives.length, rawBytes: streamed.bytes, rawSha256: streamed.sha256, rawRows: streamed.rows,
  invalidRawMonitorRows: streamed.monitorSummary.invalidCount, originalVerdictPreserved: true, reportOk: report.ok, backendSourceCommit: catalog.run.backendSourceCommit, driverSourceCommit: catalog.run.driverSourceCommit, rawVerifierAccepted, normalOwnedCleanup, capacityAccepted,
  privateSecretValuesReadOrClaimedChecked: false, finalIndexCreated: false, existingTopicsModified: false };
const sidecar = await readAt(directory, realm + '/report-provenance.json'), originalReport = await digestAt(directory, reportPath.slice(child.length + 1));
assert.equal(sidecar.report.bytes, originalReport.bytes); assert.equal(sidecar.report.sha256, originalReport.sha256);
assert.equal(sidecar.backendSourceCommit, catalog.run.backendSourceCommit); assert.equal(sidecar.driverSourceCommit, catalog.run.driverSourceCommit);
assert.equal(sidecar.driverSourceSha256, catalog.run.driverSourceSha256); assert.equal(sidecar.capacityAcceptanceAdjudicated, false);
await writeNewAt(directory, 'public-verification.json', result);
await writeNewAt(base, 'collect-formal-native-v2-ladder-02-public-verification-result.json', result);
console.log(JSON.stringify(result));
