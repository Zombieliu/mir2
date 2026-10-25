import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const root = "C:\\mir2-build\\p01-completion-20261024\\full-lifecycle-socket-diagnostics-v2\\public";
const taskRoot = "C:\\mir2-build\\p01-completion-20261024\\full-lifecycle-socket-diagnostics-v2";
const repo = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const { loadPreparedInputs } = await import('./verify-inputs.mjs');
const { profile, releaseInput, backendBytes, backendManifest: backend, backendInputCount, releaseRecordBytes } = await loadPreparedInputs(repo, taskRoot);
const { verifyFullLifecycle, verifyPublicTrace } = await import('./frozen-audit-support.mjs');
const privateRoot = path.join(taskRoot, 'private');
const stamp = process.argv[2];
assert.ok(/^\d{13}-[a-f0-9]{8}$/.test(stamp), 'A generated realm stamp is required');
const realm = JSON.parse(await fs.readFile(path.join(root, stamp + '.realm-evidence.json'), 'utf8'));
const runId = realm.playtest.runId;
assert.ok(/^\d{13}-[a-f0-9]{8}$/.test(runId));
const run = JSON.parse(await fs.readFile(path.join(root, runId + '.report.json'), 'utf8'));
assert.equal(run.ok, true); assert.equal(run.fullLifecyclePassed, true); assert.equal(run.requiredChecks.length, 16);
assert.ok(run.requiredChecks.every(key => run.checks[key]?.status === 'passed'));
assert.equal(run.registrationMode, 'two-fresh-ordinary-registrations'); assert.equal(run.visualAccepted, false);
assert.equal(realm.ok, true); assert.equal(realm.shutdown.code, 0); assert.deepEqual(realm.remainingOwnedPortListeners, []);
assert.equal(realm.release.sha256, releaseInput.sha256);
assert.equal(realm.release.sourceCommit, releaseInput.sourceCommit);
assert.equal(realm.release.backendInputCount, backendInputCount);
assert.equal(realm.backendManifestVerifiedInputs, backendInputCount);
assert.equal(realm.privateAcl.ownerOnlyVerified, true);
assert.equal(realm.configuration.spectatorRecording, 'enabled; fresh independent private directory');
assert.equal(realm.configuration.slowStageThresholdMs, 50);
assert.equal(profile.socketDiagnosticsEnabled, true); assert.equal(profile.slowStageThresholdMs, 50);
for (const pin of profile.driverPins) {
 const bytes = await fs.readFile(path.join(repo, pin.file));
 assert.equal(bytes.length, pin.bytes);
 assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'), pin.sha256);
 assert.equal(realm.sourcePins[pin.file], pin.sha256);
}
assert.equal(realm.release.releaseRecordFileSha256, releaseInput.releaseRecordFileSha256);
assert.equal(crypto.createHash('sha256').update(await fs.readFile(path.join(root, stamp + '.release-input.json'))).digest('hex'), releaseInput.releaseRecordFileSha256);
assert.equal(crypto.createHash('sha256').update(await fs.readFile(path.join(root, stamp + '.backend-source.json'))).digest('hex'), releaseInput.backendSourceManifestFileSha256);
const fields = ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases',
  'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight'];
for (const state of [realm.baselineHealth, realm.afterHealth]) for (const field of fields) assert.equal(state.capacity[field], 0);
const baseline = realm.baselineHealth.zone_metrics, after = realm.afterHealth.zone_metrics;
for (const metric of [baseline, after]) { assert.equal(metric.version, 2); assert.equal(metric.complete, true); }
const classes = ['movementAdmissions', 'immediateRequests', 'lifecycleRequests'];
const deltas = Object.fromEntries(classes.map(key => {
  const before = baseline[key], value = after[key];
  assert.ok(value.duration.count > before.duration.count);
  assert.equal(value.duration.count, value.successCount + value.failureCount);
  assert.equal(value.duration.saturated, false); assert.equal(value.duration.discardedSamples, 0);
  assert.equal(value.duration.buckets.reduce((sum, bucket) => sum + bucket.count, 0), value.duration.count);
  return [key, { count: value.duration.count - before.duration.count, successCount: value.successCount - before.successCount,
    failureCount: value.failureCount - before.failureCount, sumUs: value.duration.sumUs - before.duration.sumUs,
    overBudgetCount: value.duration.overBudgetCount - before.duration.overBudgetCount }];
}));
await fs.writeFile(path.join(root, stamp + '.zone-metrics.json'), JSON.stringify({ schemaVersion: 1, sourceRealmEvidence: stamp + '.realm-evidence.json',
  capturedHealthBeforeShutdown: true, baseline, after, classDeltas: deltas,
  scope: 'functional two-socket release profile; cumulative bucket snapshots. MovementAdmissions excludes pending authoritative outbound completion. No capacity or latency SLO acceptance is inferred.' }, null, 2) + '\n', { flag: 'wx' });
const traces = await Promise.all(['a', 'b'].map(async label => (await fs.readFile(path.join(root, `${runId}-${label}.trace.jsonl`), 'utf8')).trim().split('\n').map(line => JSON.parse(line))));
verifyFullLifecycle(run, traces);
const death = run.checks.playerDeath.evidence;
const deathPackets = traces.map((events, index) => events.find(event => event.sequence === (index ? death.observerDeathReceipt : death.ownerDeathReceipt)));
assert.equal(deathPackets[0].packet, 'Death'); assert.equal(deathPackets[1].packet, 'ObjectDied');
assert.equal(deathPackets[1].payload.objectId, death.observerId);
for (const event of deathPackets) assert.deepEqual(event.payload.location, death.location);
const ordinary = new Set(['clientVersion', 'newAccount', 'login', 'newCharacter', 'startGame', 'keepAlive', 'walk', 'run', 'turn', 'chat', 'attack', 'equipItem', 'useItem', 'logOut', 'pickUp', 'townRevive', 'removeItem']);
let sentCount = 0;
for (const file of (await fs.readdir(root)).filter(name => name.startsWith(runId) && name.endsWith('.trace.jsonl'))) {
  const events = (await fs.readFile(path.join(root, file), 'utf8')).trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  verifyPublicTrace(events);
  for (const event of events.filter(value => value.direction === 'sent')) { assert.ok(ordinary.has(event.type), 'Nonordinary sent command'); sentCount++; }
}
const credentials = JSON.parse(await fs.readFile(run.privateCredentialsPath, 'utf8'));
assert.equal(credentials.length, 2);
const values = [...credentials.map(value => value.password), ...await Promise.all(['recovery-mac-key.txt', 'identity-session-secret.txt', 'identity-recovery-pepper.txt', 'passkey-auth-secret.txt'].map(file => fs.readFile(path.join(privateRoot, file), 'utf8')))];
const outputNames = (await fs.readdir(root)).filter(name => name.startsWith(stamp) || name.startsWith(runId));
for (const name of outputNames) {
  const text = await fs.readFile(path.join(root, name), 'utf8');
  assert.ok(values.every(value => value && !text.includes(value)), 'Configured private credential value found in public evidence');
}
const binarySha256 = crypto.createHash('sha256').update(await fs.readFile(realm.release.executable)).digest('hex');
assert.equal(binarySha256, realm.release.sha256);
const result = { schemaVersion: 1, auditedAt: new Date().toISOString(), runId, realmStamp: stamp, all16Passed: true,
  pairedDeathReceiptsMatchRawTrace: true, pairedReviveAndKillDropPickupReceiptsMatchRawTrace: true, onlyOrdinarySentCommands: true, sentCommandCount: sentCount,
  backendInputCount, backendSourceCommit: releaseInput.sourceCommit, backendSourceAggregate: releaseInput.backendSourceAggregate, frozenDriverRawPinsMatched: 6,
  checkedPublicFileCount: outputNames.length, checkedPrivateValueCount: values.length, credentialValuesAbsent: true,
  rawZoneMetricsVersion: 2, rawZoneMetricsComplete: true, nonzeroClassDeltas: deltas,
  binarySha256, sixCountersZeroBeforeAndAfter: true, normalExitCode: realm.shutdown.code,
  firstNoLootAttemptPreserved: run.lifecycleDropAttempts?.[0]?.sharedDrop?.status === 'failed',
  scope: 'P0 socket-diagnostics server Release normal public protocol lifecycle; neutral receipt checks from pinned verifier snapshot only. Independent dynamically counted candidate-source proof; historical builds remain separate. No frontend/native UI, two-computer, capacity, Cell or global ghost census acceptance' };
await fs.writeFile(path.join(root, stamp + '.independent-audit.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
await fs.writeFile(path.join(root, stamp + '.source-manifest-verification.json'), JSON.stringify({ schemaVersion: 1, sourceCommit: releaseInput.sourceCommit, backendInputCount, sourceManifestSha256: releaseInput.backendSourceAggregate, sourceManifestFileSha256: releaseInput.backendSourceManifestFileSha256, matchedRawInputs: backendInputCount, matchedRawDriverPins: 6, releaseSha256: releaseInput.sha256, scope: 'This fresh41-input socket-diagnostics Release only; historical source/binary proofs remain unchanged' }, null, 2) + '\n', { flag: 'wx' });
await fs.copyFile(import.meta.filename, path.join(root, stamp + '.audit-source.mjs'), 1);
console.log(JSON.stringify(result));
