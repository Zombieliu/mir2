import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { inventory, verifyInventory, verifyBackend, verifyFullLifecycle, verifyOwnedRealm, verifyHistogram, verifyCapacityStatus, verifyPublicTrace,
  streamCapacitySamples, resolveArchivedFile, makeCapacityProjection, STREAM_LIMITS } from './verify-evidence.mjs';

const proof = new URL('../protocol-full-lifecycle/', import.meta.url);
const run = JSON.parse(await fs.readFile(new URL('1792854507439-f411ed33.report.json', proof), 'utf8'));
const traces = await Promise.all(['a', 'b'].map(async label => (await fs.readFile(new URL(`1792854507439-f411ed33-${label}.trace.jsonl`, proof), 'utf8')).trim().split('\n').map(JSON.parse)));
const metrics = JSON.parse(await fs.readFile(new URL('1792854504282-92969528.zone-metrics.json', proof), 'utf8'));
const realm = JSON.parse(await fs.readFile(new URL('1792854504282-92969528.realm-evidence.json', proof), 'utf8'));

test('fresh full 16 passes only with the matching raw paired death, revival, kill, drop and pickup receipts', () => {
  verifyFullLifecycle(run, traces);
  for (const mutate of [
    value => { value.checks.playerDeath.status = 'not_run'; },
    value => { value.fullLifecyclePassed = false; },
    value => { value.registrationMode = 'recorded-prior-run-accounts'; },
    value => { value.requiredChecks.pop(); },
    value => { value.checks.playerDeath.evidence.observerId++; },
    value => { value.checks.playerDeath.evidence.location.x++; },
    value => { value.checks.townRevive.evidence.revivedReceipt = value.checks.playerDeath.evidence.ownerDeathReceipt; },
    value => { value.checks.townRevive.evidence.hp = 0; },
    value => { value.checks.sharedKill.evidence.targetId++; },
    value => { value.checks.sharedDrop.evidence.drop.objectId++; },
    value => { value.checks.uniquePickup.evidence.removalReceipts[1]++; },
  ]) {
    const changed = structuredClone(run); mutate(changed); assert.throws(() => verifyFullLifecycle(changed, traces));
  }
});

test('old 14/2 cannot be promoted by flipping its summary flags', () => {
  const changed = structuredClone(run);
  changed.checks.playerDeath.status = 'not_run'; changed.checks.townRevive.status = 'not_run';
  changed.ok = true; changed.fullLifecyclePassed = true;
  assert.throws(() => verifyFullLifecycle(changed, traces), /failed or missing/);
});

test('raw traces reject omitted receipts, privileged commands and unredacted credential fields', () => {
  verifyPublicTrace(traces[0]);
  const missing = structuredClone(traces[0]); missing.splice(5, 1); assert.throws(() => verifyPublicTrace(missing), /sequence/);
  for (const command of ['qaControl', 'moveTo', 'event.spawn']) assert.throws(() => verifyPublicTrace([{ sequence: 1, direction: 'sent', type: command }]));
  assert.throws(() => verifyPublicTrace([{ sequence: 1, direction: 'received', type: 'identitySession', token: 'not-redacted' }]), /unredacted/);
});

test('owned realm cleanup proves only the recorded loopback PID and six zero connection counters', () => {
  const frozen = { releaseBuild: { sha256: realm.release.sha256 }, sourceManifestSha256: realm.release.backendSourceAggregate };
  verifyOwnedRealm(realm, run, frozen);
  for (const mutate of [value => { value.afterHealth.capacity.currentReconnectLeases = 1; },
    value => { delete value.baselineHealth.capacity.currentActiveSessions; }, value => { value.listenerOwnership[0].pid++; },
    value => { value.listenerOwnership[0].address = '0.0.0.0'; }, value => { value.processIdentity.executable = 'other.exe'; },
    value => { value.release.sha256 = '0'.repeat(64); }, value => { value.capacityAccepted = true; },
    value => { value.afterHealth.zone_metrics.complete = false; }, value => { value.playtest.statuses.playerDeath = 'not_run'; }]) {
    const changed = structuredClone(realm); mutate(changed); assert.throws(() => verifyOwnedRealm(changed, run, frozen));
  }
});

test('histograms independently recompute quantiles and reject saturation, missing buckets and forged totals', () => {
  const original = metrics.after.immediateRequests.duration; verifyHistogram(original);
  for (const mutate of [value => { value.saturated = true; }, value => { value.discardedSamples = 1; },
    value => { value.buckets.pop(); }, value => { value.count++; }, value => { value.p99UpperUs = 150000; },
    value => { value.sumUs = -1; }]) {
    const changed = structuredClone(original); mutate(changed); assert.throws(() => verifyHistogram(changed));
  }
  const overflow = structuredClone(original); overflow.buckets.forEach(value => { value.count = 0; });
  overflow.count = 1; overflow.sumUs = 31000000; overflow.maxUs = 31000000; overflow.overBudgetCount = 1;
  overflow.buckets.at(-1).count = 1; for (const key of ['p50UpperUs', 'p95UpperUs', 'p99UpperUs', 'p999UpperUs']) overflow[key] = null;
  verifyHistogram(overflow); overflow.p99UpperUs = 30000000; assert.throws(() => verifyHistogram(overflow));
});

test('raw artifact inventory rejects byte changes, missing and extra files without normalizing evidence', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-evidence-verifier-'));
  try {
    const indexFile = path.join(root, 'artifact-index.json'), proofFile = path.join(root, 'raw-proof.jsonl');
    await fs.writeFile(proofFile, 'first\r\nsecond\r\n');
    const files = await inventory(root, indexFile);
    await fs.writeFile(indexFile, JSON.stringify({ schemaVersion: 1, hashMode: 'raw-bytes', files }));
    await verifyInventory(root, indexFile);
    await fs.writeFile(proofFile, 'first\nsecond\n'); await assert.rejects(verifyInventory(root, indexFile));
    await fs.writeFile(proofFile, 'first\r\nsecond\r\n'); await fs.writeFile(path.join(root, 'extra'), 'extra');
    await assert.rejects(verifyInventory(root, indexFile)); await fs.unlink(path.join(root, 'extra'));
    await fs.unlink(proofFile); await assert.rejects(verifyInventory(root, indexFile));
  } finally {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir()));
    assert.ok(path.basename(root).startsWith('mir2-evidence-verifier-'));
    await fs.rm(root, { recursive: true, force: true });
  }
});

test('failed capacity diagnostics remain failed and forged passing summaries are rejected', () => {
  const failed = { schemaVersion: 1, policy: { profile: 'probe', fullAcceptanceEligible: false, saveSamples: 0, stages: [5] },
    stages: [{ passed: false }], cleanupErrors: [], ordinaryAccountsOnly: true, visualAccepted: false,
    safetyStopped: false, continuousAcceptance: { passed: false, failureCount: 1 }, savedVerification: [],
    ok: false, movementCapacityAccepted: false, combatCapacityAccepted: false, playableCapacityAccepted: false, acceptedControlledActors: null };
  assert.equal(verifyCapacityStatus(failed, []), false);
  const changed = structuredClone(failed); changed.ok = true; assert.throws(() => verifyCapacityStatus(changed, []), /ok flag/);
  const invented = structuredClone(failed); invented.playableCapacityAccepted = true; invented.acceptedControlledActors = 5;
  assert.throws(() => verifyCapacityStatus(invented, []), /acceptance flags/);
});

test('capacity acceptance requires matching raw stages, fixed normal policy and complete continuous telemetry', () => {
  const policy = { profile: 'standard', activityMode: 'native', fullAcceptanceEligible: true, stages: [5],
    stageSeconds: 300, soakSeconds: 7200, resumeSamples: 1, saveSamples: 1, minimumCombatAttacksPerMinute: 18,
    minimumCombatHitsPerMinute: 12, minimumMovesPerMinute: 30, minimumNativeRunRatio: .5, minimumNativeCellsPerMinute: 80,
    maximumMissedPlanRatio: .05, maximumCorrectionRatio: .05, movementP95Ms: 750, chatP95Ms: 1000,
    timeoutExclusive: .01, maximumMemorySlopeMiBPerHour: 32 };
  const stage = { name: 'soak', seconds: 7200, controlledActors: 5, startedAtMs: 300100, endedAtMs: 7500100,
    passed: true, movementPassed: true, chatPassed: true, fanoutPassed: true,
    activity: [{ passed: true, nativeActivity: { passed: true } }], timeoutRatio: 0, correctionRatio: 0,
    combatCoverage: 'shared-positive-damage', rollingWindows: [] };
  const measured = { ...stage, name: 'stage-5', seconds: 300, startedAtMs: 100, endedAtMs: 300100 };
  const phaseWindows = [{ name: 'bootstrap-5', kind: 'bootstrap', targetActors: 5, startedAtMs: 0, endedAtMs: 100, completed: true },
    ...[measured, stage].map(value => ({ name: value.name, kind: 'measurement', targetActors: 5, startedAtMs: value.startedAtMs, endedAtMs: value.endedAtMs, completed: true }))];
  const report = { schemaVersion: 1, policy, stages: [measured, stage], phaseWindows, cleanupErrors: [], ordinaryAccountsOnly: true, visualAccepted: false,
    safetyStopped: false, continuousAcceptance: { passed: true, failureCount: 0 }, savedVerification: [{ status: 'passed' }],
    nativeResume: [{ status: 'passed' }], memoryTrend: { seconds: 1800, slopeMiBPerHour: 0 }, peakControlledActors: 5, highestPassedStage: 5,
    ok: true, movementCapacityAccepted: true, combatCapacityAccepted: true, playableCapacityAccepted: true, acceptedControlledActors: 5 };
  const samples = [{ type: 'stageComplete', stage: measured }, { type: 'stageComplete', stage },
    ...phaseWindows.flatMap(phase => [{ type: 'phaseStarted', at: phase.startedAtMs, ...phase, endedAtMs: null, completed: false },
      { type: 'phaseEnded', at: phase.endedAtMs, ...phase }]),
    ...Array.from({ length: 753 }, (_, index) => ({ type: 'monitor', sample: { sampledAtMs: index * 10000,
      stop: false, gatewayTelemetryAvailable: true, process: { pid: 1, creationTimeMs: 1 },
      zoneMetrics: { ...structuredClone(index === 752 ? metrics.after : metrics.baseline), sinceInitializationMs: index * 10000 } } })),
    { type: 'finished', ok: true, playableCapacityAccepted: true }];
  assert.equal(verifyCapacityStatus(report, samples), true);
  for (const mutate of [value => { value[0].stage.passed = false; }, value => { value.find(row => row.type === 'finished').ok = false; },
    value => { value.find(row => row.type === 'monitor').sample.zoneMetrics.complete = false; }, value => { value.at(-2).sample.zoneMetrics.immediateRequests.duration.saturated = true; },
    value => { value.at(-2).sample.process.pid++; }, value => { value.push({ type: 'continuousFailure' }); },
    value => { value.find(row => row.type === 'phaseEnded').endedAtMs++; }]) {
    const changed = structuredClone(samples); mutate(changed); assert.throws(() => verifyCapacityStatus(report, changed));
  }
  const weakened = structuredClone(report); weakened.policy.minimumCombatHitsPerMinute = 0;
  assert.throws(() => verifyCapacityStatus(weakened, samples), /weakens/);
  const shorterProfile = structuredClone(report); shorterProfile.policy.stageSeconds = 299;
  assert.equal(verifyCapacityStatus(shorterProfile, samples), false, 'authentic shorter Node profile is not accepted for this round');
  const shorterSoak = structuredClone(report), shorterSamples = structuredClone(samples);
  shorterSoak.policy.soakSeconds = 3600;
  const shortStage = shorterSoak.stages.find(value => value.name === 'soak'); shortStage.seconds = 3600; shortStage.endedAtMs = 3900100;
  shorterSoak.phaseWindows.find(value => value.name === 'soak').endedAtMs = shortStage.endedAtMs;
  shorterSamples.find(value => value.type === 'stageComplete' && value.stage.name === 'soak').stage = structuredClone(shortStage);
  const shortEnd = shorterSamples.find(value => value.type === 'phaseEnded' && value.name === 'soak'); shortEnd.at = shortEnd.endedAtMs = shortStage.endedAtMs;
  assert.equal(verifyCapacityStatus(shorterSoak, shorterSamples), false, 'one-hour source-valid soak remains diagnostic for the two-hour round');
});

test('explicit historical Git blobs preserve the 32 frozen code inputs after workspace changes, using only declared code LF normalization', async () => {
  const workspace = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-evidence-backend-'));
  const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
  const git = args => execFileSync('git', ['-c', 'core.autocrlf=false', '-c', 'commit.gpgsign=false', '-c',
    `core.hooksPath=${path.join(workspace, 'empty-hooks')}`, ...args], { cwd: workspace, windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'] });
  try {
    const root = path.join(workspace, 'evidence'); await fs.mkdir(path.join(root, 'evidence-tools'), { recursive: true });
    git(['init', '--quiet']);
    git(['-c', 'user.name=Verifier Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--allow-empty', '--quiet', '-m', 'Immutable base fixture']);
    const base = git(['rev-parse', 'HEAD']).toString().trim();
    await fs.writeFile(path.join(workspace, '.gitattributes'), '* text eol=lf\n');
    const files = [], normalized = [];
    for (let index = 0; index < 32; index++) {
      const relative = `mir2-web3/apps/simulation/src/pin_${index}.rs`, absolute = path.join(workspace, relative);
      const raw = Buffer.from(`// frozen ${index}\r\nfn main() {}\r\n`), lf = Buffer.from(raw.toString().replace(/\r\n/g, '\n'));
      await fs.mkdir(path.dirname(absolute), { recursive: true }); await fs.writeFile(absolute, raw);
      files.push({ path: relative, bytes: raw.length, sha256: digest(raw) });
      normalized.push({ path: relative, rawBytes: raw.length, rawSha256: digest(raw), lfBytes: lf.length, lfSha256: digest(lf) });
    }
    const frozen = { schemaVersion: 1, files, sourceManifestSha256: digest(Buffer.from(JSON.stringify(files))), gitBase: base };
    const pin = { schemaVersion: 1, files: normalized, frozenSourceManifestSha256: frozen.sourceManifestSha256,
      frozenGitBase: frozen.gitBase, normalizedManifestSha256: digest(Buffer.from(JSON.stringify(normalized))),
      releaseBuild: { profile: 'release', sha256: 'a'.repeat(64), sourceManifestSha256: frozen.sourceManifestSha256 } };
    const pinFile = path.join(root, 'evidence-tools/normalized-code-pin.json');
    await fs.writeFile(path.join(root, 'backend-source-frozen.json'), JSON.stringify(frozen)); await fs.writeFile(pinFile, JSON.stringify(pin));
    git(['add', '--all']); git(['-c', 'user.name=Verifier Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--quiet', '-m', 'Frozen code fixture']);
    const commit = git(['rev-parse', 'HEAD']).toString().trim();
    await verifyBackend(root, workspace);
    const extra = 'mir2-web3/apps/gateway/src/new_recording_writer.rs';
    await fs.mkdir(path.dirname(path.join(workspace, extra)), { recursive: true });
    await fs.writeFile(path.join(workspace, extra), '// unlisted backend input\n');
    await assert.rejects(verifyBackend(root, workspace), /outside the frozen manifest/);
    await verifyBackend(root, workspace, commit);
    git(['add', '--', extra]);
    git(['-c', 'user.name=Verifier Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--quiet', '-m', 'Additional backend input fixture']);
    const laterCommit = git(['rev-parse', 'HEAD']).toString().trim();
    await assert.rejects(verifyBackend(root, workspace, laterCommit), /outside the frozen manifest/);
    await fs.unlink(path.join(workspace, extra));
    await fs.writeFile(path.join(workspace, files[0].path), '// later P2 edit\n');
    await assert.rejects(verifyBackend(root, workspace), /differs from release freeze/);
    await verifyBackend(root, workspace, commit);
    await assert.rejects(verifyBackend(root, workspace, 'HEAD'), /full 40-character/);
    pin.files[0].lfSha256 = '0'.repeat(64); pin.normalizedManifestSha256 = digest(Buffer.from(JSON.stringify(pin.files)));
    await fs.writeFile(pinFile, JSON.stringify(pin)); await assert.rejects(verifyBackend(root, workspace, commit), /differs from explicit LF pin/);
  } finally {
    assert.equal(path.dirname(path.resolve(workspace)), path.resolve(os.tmpdir()));
    assert.ok(path.basename(workspace).startsWith('mir2-evidence-backend-'));
    await fs.rm(workspace, { recursive: true, force: true });
  }
});

test('archive streaming hashes raw bytes and parses cross-chunk Unicode/escaped large lines without retaining gameplay payloads', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-evidence-stream-'));
  try {
    const file = path.join(root, 'raw.jsonl'), large = '😀\\quoted\n'.repeat(140000);
    const bytes = Buffer.from([JSON.stringify({ type: 'action', payload: large }), JSON.stringify({ type: 'stageComplete', stage: { note: '跨chunk\\边界' } }),
      JSON.stringify({ type: 'finished', ok: false })].join('\r\n') + '\r\n');
    await fs.writeFile(file, bytes);
    const expected = { bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
    const result = await streamCapacitySamples(file, expected);
    assert.equal(result.rows, 3); assert.equal(result.bytes, bytes.length); assert.equal(result.selected.length, 2);
    assert.deepEqual(result.selected.map(row => row.type), ['stageComplete', 'finished']);
    assert.equal(result.selected[0].stage.note, '跨chunk\\边界');
    await assert.rejects(streamCapacitySamples(file, { ...expected, sha256: '0'.repeat(64) }), /size\/SHA/);
    await assert.rejects(streamCapacitySamples(file, expected, { ...STREAM_LIMITS, lineBytes: 1024 }), /line exceeds/);
    await assert.rejects(streamCapacitySamples(file, expected, { ...STREAM_LIMITS, selectedBytes: 1 }), /fixed memory/);
    await fs.writeFile(file, bytes.toString().replace(/\r\n/g, '\n'));
    await assert.rejects(streamCapacitySamples(file, expected), /size\/SHA/);
  } finally {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir())); assert.ok(path.basename(root).startsWith('mir2-evidence-stream-'));
    await fs.rm(root, { recursive: true, force: true });
  }
});

test('archive reads require an explicit root and reject missing, absolute, traversal, private and linked paths', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-evidence-archive-'));
  try {
    const archive = path.join(root, 'archive'), outside = path.join(root, 'outside'), runId = '1792855739500-1962792b';
    await fs.mkdir(path.join(archive, 'reports'), { recursive: true }); await fs.mkdir(outside);
    const file = path.join(archive, 'reports', runId + '.samples.jsonl'); await fs.writeFile(file, '{}\n');
    await fs.writeFile(path.join(outside, runId + '.samples.jsonl'), '{}\n');
    const entry = { kind: 'capacity-samples-jsonl', runId, report: `capacity/${runId}.report.json`, path: `reports/${runId}.samples.jsonl`, bytes: 3, sha256: 'a'.repeat(64) };
    assert.equal(await resolveArchivedFile(archive, entry), await fs.realpath(file));
    await assert.rejects(resolveArchivedFile(undefined, entry), /explicit --archive-root/);
    for (const invalid of [{ path: `../outside/${runId}.samples.jsonl` }, { path: file }, { report: '../outside.report.json' },
      { path: `private/${runId}.samples.jsonl` }, { path: `reports/not-the-run.samples.jsonl` }]) {
      await assert.rejects(resolveArchivedFile(archive, { ...entry, ...invalid }));
    }
    await assert.rejects(resolveArchivedFile(archive, { ...entry, path: `missing/${runId}.samples.jsonl` }));
    await fs.symlink(outside, path.join(archive, 'escape'), process.platform === 'win32' ? 'junction' : 'dir');
    await assert.rejects(resolveArchivedFile(archive, { ...entry, path: `escape/${runId}.samples.jsonl` }), /links|escapes/);
  } finally {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir())); assert.ok(path.basename(root).startsWith('mir2-evidence-archive-'));
    await fs.rm(root, { recursive: true, force: true });
  }
});

test('archive streaming rejects malformed UTF-8/JSON and excessive selected rows rather than dropping evidence', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-evidence-stream-invalid-'));
  try {
    const file = path.join(root, 'raw.jsonl');
    for (const bytes of [Buffer.from('{broken\n'), Buffer.from([0xff, 10]), Buffer.from('\n')]) {
      await fs.writeFile(file, bytes);
      await assert.rejects(streamCapacitySamples(file, { bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') }), /Invalid archived/);
    }
    const bytes = Buffer.from(Array.from({ length: 3 }, () => JSON.stringify({ type: 'stageComplete' })).join('\n') + '\n');
    await fs.writeFile(file, bytes);
    await assert.rejects(streamCapacitySamples(file, { bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') },
      { ...STREAM_LIMITS, selectedRows: 2 }), /fixed memory/);
  } finally {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir())); assert.ok(path.basename(root).startsWith('mir2-evidence-stream-invalid-'));
    await fs.rm(root, { recursive: true, force: true });
  }
});

test('10001 real-shape monitor rows stream beyond the former 64MiB limit with bounded phase snapshots', async t => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-evidence-stream-10k-'));
  try {
    const fixture = JSON.parse(await fs.readFile(new URL('./monitor-shape.fixture.json', import.meta.url), 'utf8'));
    const base = fixture.record.sample.sampledAt, initialLifetime = fixture.record.sample.zoneMetrics.sinceInitializationMs;
    const file = path.join(root, 'raw.jsonl'), handle = await fs.open(file, 'wx'), digest = crypto.createHash('sha256');
    let bytes = 0;
    try {
      for (let index = 0; index <= 10000; index++) {
        fixture.record.sample.sampledAt = base + index * 1000;
        fixture.record.sample.sampledAtMs = fixture.record.sample.sampledAt;
        fixture.record.sample.zoneMetrics.sinceInitializationMs = initialLifetime + index * 1000;
        const line = JSON.stringify(fixture.record) + '\n', encoded = Buffer.from(line);
        assert.equal((await handle.write(encoded)).bytesWritten, encoded.length); digest.update(encoded); bytes += encoded.length;
      }
      const final = Buffer.from(JSON.stringify({ type: 'finished', ok: false }) + '\n');
      assert.equal((await handle.write(final)).bytesWritten, final.length); digest.update(final); bytes += final.length;
    } finally { await handle.close(); }
    const phaseWindows = Array.from({ length: 32 }, (_, index) => ({ name: `phase-${index}`, startedAtMs: base + 100000 + index * 200000,
      endedAtMs: base + 250000 + index * 200000 }));
    const result = await streamCapacitySamples(file, { bytes, sha256: digest.digest('hex') }, STREAM_LIMITS,
      { playableCapacityAccepted: true, phaseWindows });
    assert.ok(bytes > 80 * 1048576); assert.equal(result.rows, 10002); assert.equal(result.selected.length, 1);
    const summary = result.monitorSummary;
    assert.equal(summary.count, 10001); assert.equal(summary.validCount, 10001); assert.equal(summary.invalidCount, 0);
    assert.equal(summary.maxGapMs, 1000); assert.equal(summary.firstSampledAtMs, base); assert.equal(summary.lastSampledAtMs, base + 10000000);
    assert.ok(summary.phaseBrackets.every(phase => phase.beforeStart && phase.afterStart && phase.beforeEnd && phase.afterEnd));
    const retained = new Set([summary.first, summary.last, ...summary.phaseBrackets.flatMap(value => [value.beforeStart, value.afterStart, value.beforeEnd, value.afterEnd])]);
    assert.ok(retained.size <= 130); assert.ok(Buffer.byteLength(JSON.stringify(summary)) < 2 * 1048576);
    assert.ok([...retained].every(value => !('host' in value) && !('network' in value) && !('memory' in value)));
    const projection = makeCapacityProjection('fixture-10001', result), projectionBytes = Buffer.byteLength(JSON.stringify(projection));
    assert.ok(projectionBytes < 2 * 1048576);
    t.diagnostic(JSON.stringify({ rawBytes: bytes, rawRows: result.rows, monitorRows: summary.count,
      retainedUniqueSnapshots: retained.size, phaseWindows: summary.phaseBrackets.length, retainedNonMonitorRows: result.selected.length,
      projectionBytes, processPeakRssKiB: process.resourceUsage().maxRSS }));
  } finally {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir())); assert.ok(path.basename(root).startsWith('mir2-evidence-stream-10k-'));
    await fs.rm(root, { recursive: true, force: true });
  }
});

test('discarded intermediate monitors still detect stop, process switches and cumulative resets', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-evidence-stream-middle-'));
  try {
    const fixture = JSON.parse(await fs.readFile(new URL('./monitor-shape.fixture.json', import.meta.url), 'utf8'));
    for (const failure of ['stop', 'pid', 'buckets']) {
      const rows = Array.from({ length: 3 }, (_, index) => {
        const value = structuredClone(fixture.record); value.sample.sampledAt += index * 1000; value.sample.sampledAtMs = value.sample.sampledAt;
        value.sample.zoneMetrics = { ...structuredClone(metrics.after), sinceInitializationMs: index * 1000 };
        return value;
      });
      if (failure === 'stop') rows[1].sample.stop = true;
      if (failure === 'pid') rows[1].sample.process.pid++;
      if (failure === 'buckets') rows[1].sample.zoneMetrics = { ...structuredClone(metrics.baseline), sinceInitializationMs: 1000 };
      const bytes = Buffer.from(rows.map(JSON.stringify).join('\n') + '\n'), file = path.join(root, failure + '.jsonl');
      await fs.writeFile(file, bytes); const expected = { bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
      await assert.rejects(streamCapacitySamples(file, expected, STREAM_LIMITS, { playableCapacityAccepted: true }));
      const failedProof = await streamCapacitySamples(file, expected, STREAM_LIMITS, { playableCapacityAccepted: false });
      assert.equal(failedProof.rows, 3); assert.equal(failedProof.selected.length, 0);
      assert.equal(failedProof.monitorSummary.complete, false); assert.equal(failedProof.monitorSummary.invalidCount, 1);
      assert.equal(failedProof.monitorSummary.stoppedCount, failure === 'stop' ? 1 : 0);
    }
  } finally {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir())); assert.ok(path.basename(root).startsWith('mir2-evidence-stream-middle-'));
    await fs.rm(root, { recursive: true, force: true });
  }
});
