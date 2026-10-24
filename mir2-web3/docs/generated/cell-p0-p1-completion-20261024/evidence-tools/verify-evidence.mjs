// Read-only verification by default. Evidence is always hashed as raw bytes.
// This verifies the recorded P0/P1 scope; it grants no frontend or Cell acceptance.
import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const FULL_CHECKS = ['authGuards', 'twoAccounts', 'mutualVisibility', 'movement', 'turn', 'chat', 'chatLatency',
  'sharedCombat', 'mapRoundTrip', 'logout', 'reloginPersistence', 'sharedKill', 'sharedDrop', 'uniquePickup', 'playerDeath', 'townRevive'];
const COUNTERS = ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases', 'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight'];
const ORDINARY = new Set(['clientVersion', 'newAccount', 'login', 'newCharacter', 'startGame', 'keepAlive', 'walk', 'run', 'turn', 'chat', 'attack', 'equipItem', 'useItem', 'logOut', 'pickUp', 'townRevive', 'removeItem']);
const BOUNDS = [100, 250, 500, 1000, 2000, 5000, 10000, 20000, 35000, 50000, 75000, 100000, 150000, 250000, 500000, 1000000, 5000000, 30000000, null];
const CLASS = ['movementAdmissions', 'immediateRequests', 'lifecycleRequests'];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fail = message => { throw new Error(message); };
const check = (condition, message) => { if (!condition) fail(message); };
const int = value => Number.isSafeInteger(value) && value >= 0;
const same = (a, b) => assert.deepEqual(a, b);
const normalizedCode = bytes => Buffer.from(bytes.toString('utf8').replace(/\r\n/g, '\n'));
const git = (workspace, args) => execFileSync('git', ['-c', 'gc.auto=0', '-c', 'maintenance.auto=false', ...args],
  { cwd: workspace, windowsHide: true, maxBuffer: 16 * 1024 * 1024, env: { ...process.env, GIT_NO_LAZY_FETCH: '1' } });
const readJson = async file => JSON.parse((await fs.readFile(file, 'utf8')).replace(/^\uFEFF/, ''));
const safeRelative = name => typeof name === 'string' && name.length > 0 && !name.includes('\\') && !name.includes(':') &&
  !name.startsWith('/') && name.split('/').every(part => part && part !== '.' && part !== '..');
const SELECTED_RAW = new Set(['stageComplete', 'phaseStarted', 'phaseEnded', 'continuousFailure', 'safetyStop', 'finished']);
export const STREAM_LIMITS = Object.freeze({ lineBytes: 64 * 1024 * 1024, selectedBytes: 128 * 1024 * 1024, selectedRows: 50000 });

async function rawDigest(file) {
  const digest = crypto.createHash('sha256'); let bytes = 0;
  for await (const chunk of createReadStream(file)) { digest.update(chunk); bytes += chunk.length; check(int(bytes), 'Raw evidence exceeds safe byte counting'); }
  return { bytes, sha256: digest.digest('hex') };
}

function verifyRedactedFields(value) {
  if (!value || typeof value !== 'object') return;
  for (const [key, nested] of Object.entries(value)) {
    if (/^(?:password|secret|token|credential|authorization|cookie|assertion|privateKey)$/i.test(key) && nested != null) {
      check(nested === '[redacted]', 'Public raw evidence contains an unredacted credential field');
    } else verifyRedactedFields(nested);
  }
}

export class MonitorProjection {
  constructor(phases = [], required = false) {
    check(Array.isArray(phases) && phases.length <= 128, 'Too many monitor phase boundaries');
    this.required = required; this.count = 0; this.validCount = 0; this.invalidCount = 0; this.stoppedCount = 0;
    this.maxGapMs = 0; this.maxRawLineBytes = 0;
    this.phaseBrackets = phases.map(phase => ({ name: phase.name, startedAtMs: phase.startedAtMs, endedAtMs: phase.endedAtMs,
      beforeStart: null, afterStart: null, beforeEnd: null, afterEnd: null }));
  }
  add(sample, rawLineBytes = 0) {
    this.count++; this.maxRawLineBytes = Math.max(this.maxRawLineBytes, rawLineBytes);
    if (sample?.stop === true) this.stoppedCount++;
    let value;
    try {
      const at = sample?.sampledAtMs ?? sample?.sampledAt;
      check(int(at) && sample.stop === false && sample.gatewayTelemetryAvailable === true && int(sample.process?.pid) && sample.process.pid > 0 &&
        Number.isFinite(sample.process.creationTimeMs) && sample.process.creationTimeMs > 0, 'Missing or stopped raw monitor observation');
      if (sample.sampledAt != null) check(at === (typeof sample.sampledAt === 'number' ? sample.sampledAt : Date.parse(sample.sampledAt)), 'Raw monitor source and normalized clocks disagree');
      verifyMetrics(sample.zoneMetrics);
      value = { sampledAtMs: at, process: { pid: sample.process.pid, creationTimeMs: sample.process.creationTimeMs }, zoneMetrics: sample.zoneMetrics };
      const previous = this.last;
      if (previous) {
        check(value.sampledAtMs >= previous.sampledAtMs && value.process.pid === previous.process.pid &&
          value.process.creationTimeMs === previous.process.creationTimeMs && value.zoneMetrics.sinceInitializationMs >= previous.zoneMetrics.sinceInitializationMs,
        'Capacity monitor process identity, wall clock or lifetime changed');
        this.maxGapMs = Math.max(this.maxGapMs, value.sampledAtMs - previous.sampledAtMs);
        for (const key of [...CLASS, 'zoneWork', 'ownerTickDeadlineLag', 'pendingMovementDeadlineLag']) {
          const before = previous.zoneMetrics[key].duration ?? previous.zoneMetrics[key], after = value.zoneMetrics[key].duration ?? value.zoneMetrics[key];
          check(['count', 'sumUs', 'maxUs', 'overBudgetCount'].every(field => after[field] >= before[field]) &&
            after.buckets.every((bucket, index) => bucket.count >= before.buckets[index].count), 'Capacity cumulative telemetry reset');
          if (CLASS.includes(key)) check(value.zoneMetrics[key].successCount >= previous.zoneMetrics[key].successCount &&
            value.zoneMetrics[key].failureCount >= previous.zoneMetrics[key].failureCount, 'Capacity request counters reset');
        }
      }
    } catch (error) { this.invalidCount++; if (this.required) throw error; return; }
    this.validCount++; this.first ??= value; this.last = value;
    for (const phase of this.phaseBrackets) {
      if (value.sampledAtMs <= phase.startedAtMs) phase.beforeStart = value;
      if (value.sampledAtMs >= phase.startedAtMs && !phase.afterStart) phase.afterStart = value;
      if (value.sampledAtMs <= phase.endedAtMs) phase.beforeEnd = value;
      if (value.sampledAtMs >= phase.endedAtMs && !phase.afterEnd) phase.afterEnd = value;
    }
  }
  snapshot() {
    return { count: this.count, validCount: this.validCount, invalidCount: this.invalidCount, stoppedCount: this.stoppedCount,
      complete: this.count > 0 && this.invalidCount === 0, firstSampledAtMs: this.first?.sampledAtMs ?? null,
      lastSampledAtMs: this.last?.sampledAtMs ?? null, maxGapMs: this.maxGapMs, maxRawLineBytes: this.maxRawLineBytes,
      first: this.first ?? null, last: this.last ?? null, phaseBrackets: this.phaseBrackets.map(phase => ({ ...phase,
        startBeforeGapMs: phase.beforeStart ? phase.startedAtMs - phase.beforeStart.sampledAtMs : null,
        startAfterGapMs: phase.afterStart ? phase.afterStart.sampledAtMs - phase.startedAtMs : null,
        endBeforeGapMs: phase.beforeEnd ? phase.endedAtMs - phase.beforeEnd.sampledAtMs : null,
        endAfterGapMs: phase.afterEnd ? phase.afterEnd.sampledAtMs - phase.endedAtMs : null })) };
  }
}

export async function streamCapacitySamples(file, expected, limits = STREAM_LIMITS, report) {
  const digest = crypto.createHash('sha256'), decoder = new TextDecoder('utf-8', { fatal: true }), selected = [], fragments = [];
  const monitors = new MonitorProjection(report?.phaseWindows ?? [], report?.playableCapacityAccepted === true);
  let bytes = 0, rows = 0, lineBytes = 0, selectedBytes = 0;
  const consume = () => {
    let line = Buffer.concat(fragments, lineBytes); fragments.length = 0; lineBytes = 0;
    if (line.at(-1) === 13) line = line.subarray(0, -1);
    let value;
    try { value = JSON.parse(decoder.decode(line)); }
    catch { fail('Invalid archived JSONL record'); }
    check(value && typeof value === 'object' && !Array.isArray(value) && typeof value.type === 'string', 'Archived JSONL record has no explicit type');
    verifyRedactedFields(value);
    rows++;
    if (value.type === 'monitor') monitors.add(value.sample, line.length);
    else if (SELECTED_RAW.has(value.type)) {
      selectedBytes += line.length;
      check(selectedBytes <= limits.selectedBytes && selected.length < limits.selectedRows, 'Archived acceptance projection exceeds fixed memory bounds');
      selected.push(value);
    }
  };
  for await (const chunk of createReadStream(file, { highWaterMark: 65536 })) {
    digest.update(chunk); bytes += chunk.length; check(int(bytes), 'Raw archive exceeds safe byte counting');
    let offset = 0;
    while (offset < chunk.length) {
      const newline = chunk.indexOf(10, offset), end = newline < 0 ? chunk.length : newline;
      const fragment = chunk.subarray(offset, end); fragments.push(fragment); lineBytes += fragment.length;
      check(lineBytes <= limits.lineBytes, 'Archived JSONL line exceeds fixed byte limit');
      if (newline >= 0) consume();
      offset = newline < 0 ? chunk.length : newline + 1;
    }
  }
  if (lineBytes) consume();
  const actual = { bytes, sha256: digest.digest('hex') };
  check(expected && actual.bytes === expected.bytes && actual.sha256 === expected.sha256, 'Raw archive size/SHA mismatch');
  return { selected, rows, bytes, sha256: actual.sha256, monitorSummary: monitors.snapshot() };
}

export function makeCapacityProjection(runId, streamed) {
  return { schemaVersion: 1, runId, rawBytes: streamed.bytes, rawSha256: streamed.sha256, rawRows: streamed.rows,
    monitorSummary: streamed.monitorSummary,
    phaseEvents: streamed.selected.filter(row => ['phaseStarted', 'phaseEnded'].includes(row.type)),
    failureEvents: streamed.selected.filter(row => ['continuousFailure', 'safetyStop'].includes(row.type)),
    finishedEvents: streamed.selected.filter(row => row.type === 'finished') };
}

export async function resolveArchivedFile(archiveRoot, entry) {
  check(archiveRoot, 'External raw archive requires explicit --archive-root');
  check(entry.kind === 'capacity-samples-jsonl' && /^\d{13}-[a-f0-9]{8}$/.test(entry.runId) && safeRelative(entry.path) &&
    safeRelative(entry.report) && path.posix.basename(entry.path) === `${entry.runId}.samples.jsonl` &&
    path.posix.basename(entry.report) === `${entry.runId}.report.json` && int(entry.bytes) && /^[a-f0-9]{64}$/.test(entry.sha256), 'Invalid relative raw archive manifest entry');
  const root = await fs.realpath(archiveRoot); check((await fs.stat(root)).isDirectory(), 'Archive root must be a directory');
  let candidate = root;
  for (const component of entry.path.split('/')) {
    check(!/^(?:private|secrets?|credentials?)$/i.test(component), 'Raw archive path enters a private namespace');
    candidate = path.join(candidate, component); check(!(await fs.lstat(candidate)).isSymbolicLink(), 'Raw archive links are not permitted');
  }
  const resolved = await fs.realpath(candidate), relative = path.relative(root, resolved);
  check(safeRelative(relative.split(path.sep).join('/')) && (await fs.stat(resolved)).isFile(), 'Raw archive escapes its explicit root');
  return resolved;
}

export async function inventory(root, excluded) {
  const rows = [];
  async function walk(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      const absolute = path.join(directory, entry.name);
      check(!entry.isSymbolicLink(), 'Evidence must not contain symbolic links');
      if (entry.isDirectory()) await walk(absolute);
      else {
        check(entry.isFile(), 'Unsupported evidence filesystem entry');
        if (absolute === excluded) continue;
        rows.push({ path: path.relative(root, absolute).split(path.sep).join('/'), ...await rawDigest(absolute) });
      }
    }
  }
  await walk(root); return rows.sort((a, b) => a.path.localeCompare(b.path, 'en'));
}

export async function verifyInventory(root, indexFile) {
  const index = await readJson(indexFile);
  check(index.schemaVersion === 1 && index.hashMode === 'raw-bytes' && Array.isArray(index.files), 'Invalid raw artifact index schema');
  const names = new Set();
  for (const row of index.files) {
    check(safeRelative(row.path) && !names.has(row.path) && int(row.bytes) && /^[a-f0-9]{64}$/.test(row.sha256), 'Invalid or duplicate artifact index entry');
    names.add(row.path);
  }
  const actual = await inventory(root, indexFile);
  check(actual.length === index.files.length && actual.every(row => names.has(row.path)), 'Artifact membership differs from the index');
  const expected = new Map(index.files.map(row => [row.path, row]));
  for (const row of actual) check(row.bytes === expected.get(row.path).bytes && row.sha256 === expected.get(row.path).sha256, `Raw artifact mismatch: ${row.path}`);
  return actual;
}

export function verifyHistogram(histogram) {
  check(histogram && histogram.saturated === false && histogram.discardedSamples === 0 &&
    ['count', 'sumUs', 'maxUs', 'overBudgetCount'].every(key => int(histogram[key])), 'Missing, saturated or unsafe histogram');
  check(histogram.overBudgetCount <= histogram.count && histogram.maxUs <= histogram.sumUs, 'Impossible histogram totals');
  check(Array.isArray(histogram.buckets) && histogram.buckets.length === BOUNDS.length, 'Missing fixed histogram buckets');
  let sum = 0;
  histogram.buckets.forEach((bucket, index) => {
    check(bucket.upperBoundUs === BOUNDS[index] && int(bucket.count), 'Invalid fixed histogram bucket'); sum += bucket.count;
  });
  check(sum === histogram.count, 'Histogram buckets differ from count');
  check(histogram.buckets.slice(10).reduce((total, bucket) => total + bucket.count, 0) === histogram.overBudgetCount, 'Histogram over-budget numerator differs from raw buckets');
  if (histogram.count) {
    const maximumBucket = BOUNDS.findIndex(bound => bound === null || histogram.maxUs <= bound);
    check(histogram.buckets[maximumBucket].count > 0 && histogram.buckets.slice(maximumBucket + 1).every(bucket => bucket.count === 0), 'Histogram maximum contradicts occupied buckets');
  }
  for (const [key, numerator, denominator] of [['p50UpperUs', 1, 2], ['p95UpperUs', 95, 100], ['p99UpperUs', 99, 100], ['p999UpperUs', 999, 1000]]) {
    const rank = (BigInt(histogram.count) * BigInt(numerator) + BigInt(denominator - 1)) / BigInt(denominator);
    let total = 0n, expected = null;
    if (rank) for (const bucket of histogram.buckets) { total += BigInt(bucket.count); if (total >= rank) { expected = bucket.upperBoundUs; break; } }
    check(histogram[key] === expected, 'Forged histogram quantile upper bound');
  }
  if (!histogram.count) check(histogram.sumUs === 0 && histogram.maxUs === 0 && histogram.overBudgetCount === 0, 'Nonempty totals on empty histogram');
}

function verifyMetrics(value) {
  check(value?.version === 2 && value.complete === true && value.budgetUs === 50000 &&
    value.scope === 'gateway shared-owner transaction, may contain multiple maps', 'Missing or incomplete v2 metrics scope');
  for (const key of CLASS) {
    const request = value[key]; verifyHistogram(request?.duration);
    check(int(request.successCount) && int(request.failureCount) && request.successCount + request.failureCount === request.duration.count, 'Request class counts disagree');
  }
  for (const key of ['zoneWork', 'ownerTickDeadlineLag', 'pendingMovementDeadlineLag']) verifyHistogram(value[key]);
}

export function verifyPublicTrace(events) {
  check(Array.isArray(events) && events.length > 0, 'Missing raw trace');
  let previous = 0;
  for (const event of events) {
    check(int(event.sequence) && event.sequence === previous + 1, 'Trace sequence is missing, duplicated or reordered'); previous = event.sequence;
    check(['received', 'sent', 'diagnostic'].includes(event.direction), 'Unknown trace direction');
    if (event.direction === 'sent') check(ORDINARY.has(event.type), 'Nonordinary command in lifecycle trace');
    verifyRedactedFields(event);
  }
}

const receipt = (events, sequence, packet, direction = 'received') => {
  check(int(sequence) && sequence > 0, 'Missing receipt sequence');
  const value = events.find(event => event.sequence === sequence);
  check(value && value.direction === direction && (direction === 'sent' ? value.type : value.packet ?? value.type) === packet, 'Report receipt does not match raw trace');
  return value;
};
const location = value => ({ x: value?.location?.x ?? value?.x, y: value?.location?.y ?? value?.y });

export function verifyFullLifecycle(run, traces) {
  check(run.schemaVersion === 1 && run.profile === 'full-lifecycle' && run.registrationMode === 'two-fresh-ordinary-registrations' &&
    !run.resumedFrom && run.ok === true && run.status === 'passed' && run.fullLifecyclePassed === true &&
    !run.error && !run.cleanupErrors?.length && run.visualAccepted === false && run.loadAccepted === false && run.twoPhysicalComputersTested === false, 'Full lifecycle is incomplete, resumed or overclaimed');
  same([...run.requiredChecks].sort(), [...FULL_CHECKS].sort());
  check(FULL_CHECKS.every(key => run.checks?.[key]?.status === 'passed'), 'Full lifecycle has a failed or missing required check');
  check(run.accounts?.length === 2 && new Set(run.accounts.map(account => account.accountId)).size === 2 &&
    new Set(run.accounts.map(account => account.name)).size === 2 && run.accounts.every(account => account.accountId !== 'demo'), 'Invalid fresh ordinary cohort');
  same(run.checks.twoAccounts.evidence.map(actor => actor.name), run.accounts.map(actor => actor.name));
  check(traces.length === 2, 'Both owner traces are required'); traces.forEach(verifyPublicTrace);
  traces.forEach(events => check(events.filter(event => event.direction === 'sent' && event.type === 'newAccount').length === 1, 'Fresh ordinary registration missing'));
  const death = run.checks.playerDeath.evidence, revive = run.checks.townRevive.evidence;
  check(death.actor === run.accounts[0].name && revive.actor === death.actor && int(death.observerId) && death.observerId > 0, 'Death actor identity mismatch');
  const ownerDeath = receipt(traces[0], death.ownerDeathReceipt, 'Death');
  const observerDeath = receipt(traces[1], death.observerDeathReceipt, 'ObjectDied');
  check(observerDeath.payload?.objectId === death.observerId, 'Death observer ID mismatch');
  same(location(ownerDeath.payload), death.location); same(location(observerDeath.payload), death.location);
  check(revive.requestReceipt > death.ownerDeathReceipt && revive.revivedReceipt > revive.requestReceipt &&
    revive.aliveSnapshotReceipt > revive.revivedReceipt && revive.observerReceipt > death.observerDeathReceipt &&
    Number.isFinite(revive.hp) && revive.hp > 0 && revive.fromMap === death.mapFileName, 'Revival is not fresh after the witnessed death');
  receipt(traces[0], revive.requestReceipt, 'townRevive', 'sent'); receipt(traces[0], revive.revivedReceipt, 'Revived');
  const alive = receipt(traces[0], revive.aliveSnapshotReceipt, 'worldSnapshot');
  check(String(alive.payload?.mapFileName) === revive.toMap, 'Alive snapshot map mismatch'); same(location(alive.payload), revive.location);
  const observed = receipt(traces[1], revive.observerReceipt, revive.observerPacket);
  check(observed.payload?.objectId === death.observerId && (revive.toMap === revive.fromMap
    ? ['ObjectRevived', 'ObjectPlayer'].includes(revive.observerPacket) : revive.observerPacket === 'ObjectRemove'), 'Revival observer namespace mismatch');
  const kill = run.checks.sharedKill.evidence, drop = run.checks.sharedDrop.evidence, pickup = run.checks.uniquePickup.evidence;
  traces.forEach((events, index) => {
    const died = receipt(events, kill.deathReceipts[index], 'ObjectDied'); check(died.payload?.objectId === kill.targetId, 'Shared kill ID mismatch'); same(location(died.payload), kill.location);
    const spawned = receipt(events, drop.spawnReceipts[index], drop.drop.loot.kind === 'gold' ? 'ObjectGold' : 'ObjectItem');
    check(spawned.sequence > died.sequence && spawned.payload?.objectId === drop.drop.objectId, 'Shared drop receipt mismatch'); same(location(spawned.payload), location(drop.drop));
    const request = receipt(events, pickup.requestReceipts[index], 'pickUp', 'sent'); check(request.objectId === drop.drop.objectId, 'Pickup request ID mismatch');
    check(receipt(events, pickup.removalReceipts[index], 'ObjectRemove').payload?.objectId === drop.drop.objectId, 'Pickup removal mismatch');
  });
  check(pickup.objectId === drop.drop.objectId && [0, 1].includes(pickup.winner), 'Unique pickup identity mismatch');
  receipt(traces[pickup.winner], pickup.gainReceipt, pickup.gainPacket);
}

export function verifyOwnedRealm(realm, run, frozen) {
  check(realm.ok === true && realm.shutdown?.code === 0 && realm.shutdown.signal === null && realm.remainingOwnedPortListeners?.length === 0 &&
    ['freshRealm', 'capacityCleanup', 'normalOwnedShutdown', 'portsAfterStop'].every(key => realm.checks?.[key] === 'passed'), 'Owned process/connection cleanup is incomplete');
  check(['finalAccepted', 'visualAccepted', 'twoComputerAccepted', 'capacityAccepted'].every(key => realm[key] === false), 'Owned functional realm overclaims acceptance');
  check(realm.release?.profile === 'release' && realm.release.sha256 === frozen.releaseBuild.sha256 && realm.release.backendSourceAggregate === frozen.sourceManifestSha256, 'Release/source pin mismatch');
  check(int(realm.pid) && realm.pid > 0 && realm.processIdentity?.pid === realm.pid &&
    realm.processIdentity.executable.replace(/\\/g, '/').toLowerCase() === realm.release.executable.replace(/\\/g, '/').toLowerCase(), 'Owned PID executable mismatch');
  check(realm.listenerOwnership?.length === 2 && realm.listenerOwnership.every(row => row.pid === realm.pid && row.address === '127.0.0.1') &&
    new Set(realm.listenerOwnership.map(row => row.port)).size === 2, 'Listener ownership was not exclusively loopback');
  check(realm.playtest?.runId === run.runId && realm.playtest.ok === true && realm.playtest.registrationMode === run.registrationMode, 'Realm summary belongs to a different functional run');
  same(realm.playtest.requiredChecks, run.requiredChecks);
  same(realm.playtest.statuses, Object.fromEntries(Object.entries(run.checks).map(([key, value]) => [key, value.status])));
  for (const state of [realm.baselineHealth, realm.afterHealth]) {
    check(state?.ok === true && state.session_cache?.healthy === true, 'Owned health unavailable');
    COUNTERS.forEach(key => check(state.capacity?.[key] === 0, 'Six connection counters were not zero')); verifyMetrics(state.zone_metrics);
  }
  CLASS.forEach(key => check(realm.afterHealth.zone_metrics[key].duration.count > realm.baselineHealth.zone_metrics[key].duration.count, 'Request class has no measured observations'));
}

export function verifyCapacityPhases(report, samples) {
  check(Array.isArray(report.phaseWindows) && report.phaseWindows.length > 0 && report.phaseWindows.length <= 128, 'Accepted capacity lacks bounded explicit phase windows');
  const active = new Map(), ended = [];
  for (const event of samples) if (event.type === 'phaseStarted' || event.type === 'phaseEnded') {
    check(typeof event.name === 'string' && event.name.length > 0 && event.name.length <= 80 && ['bootstrap', 'measurement'].includes(event.kind) &&
      int(event.targetActors) && event.targetActors > 0 && event.targetActors <= 100 && int(event.startedAtMs), 'Invalid raw capacity phase');
    const projection = { name: event.name, kind: event.kind, targetActors: event.targetActors,
      startedAtMs: event.startedAtMs, endedAtMs: event.endedAtMs, completed: event.completed };
    if (event.type === 'phaseStarted') {
      check(active.size === 0 && !ended.some(value => value.name === event.name) && event.startedAtMs >= (ended.at(-1)?.endedAtMs ?? 0) && event.at === event.startedAtMs &&
        event.endedAtMs === null && event.completed === false, 'Duplicate or forged phase start'); active.set(event.name, projection);
    } else {
      const start = active.get(event.name);
      check(start && start.kind === event.kind && start.targetActors === event.targetActors && start.startedAtMs === event.startedAtMs &&
        int(event.endedAtMs) && event.endedAtMs >= event.startedAtMs && event.at === event.endedAtMs && event.completed === true, 'Missing, incomplete or forged phase end');
      active.delete(event.name); ended.push(projection);
    }
  }
  check(active.size === 0, 'Capacity phase did not end'); same(ended, report.phaseWindows);
  for (const count of report.policy.stages) {
    check(ended.some(phase => phase.name === `bootstrap-${count}` && phase.kind === 'bootstrap' && phase.targetActors === count), 'Missing bootstrap phase scope');
    check(report.stages.some(stage => stage.name === `stage-${count}` && stage.controlledActors === count), 'Missing declared measured stage');
  }
  for (const stage of report.stages) {
    const phase = ended.find(value => value.name === stage.name && value.kind === 'measurement');
    check(phase && phase.targetActors === stage.controlledActors && phase.startedAtMs === stage.startedAtMs && phase.endedAtMs === stage.endedAtMs &&
      stage.endedAtMs - stage.startedAtMs >= stage.seconds * 1000, 'Stage timing contradicts its explicit raw phase');
  }
}

export function verifyCapacityStatus(report, samples, streamedMonitors) {
  const policy = report.policy, final = report.stages?.find(stage => stage.name === 'soak') ?? report.stages?.at(-1);
  check(report.schemaVersion === 1 && policy && Array.isArray(report.stages) && Array.isArray(report.cleanupErrors) &&
    report.ordinaryAccountsOnly === true && report.visualAccepted === false, 'Incomplete capacity report schema');
  const expectedOk = policy.profile !== 'movement' && !report.error && report.safetyStopped === false && !report.cleanupErrors.length &&
    report.continuousAcceptance?.passed === true && report.continuousAcceptance.failureCount === 0 &&
    report.stages.every(stage => stage.passed === true) && final?.passed === true &&
    report.savedVerification?.length === policy.saveSamples && report.savedVerification.every(value => value.status === 'passed') && !report.evidenceError;
  check(report.ok === Boolean(expectedOk), 'Capacity ok flag contradicts its gates');
  const movement = report.ok && policy.fullAcceptanceEligible === true;
  check(report.movementCapacityAccepted === movement && report.combatCapacityAccepted === (movement && final.combatCoverage === 'shared-positive-damage') &&
    report.playableCapacityAccepted === (report.movementCapacityAccepted && report.combatCapacityAccepted), 'Capacity acceptance flags contradict recorded scope');
  check(report.acceptedControlledActors === (report.playableCapacityAccepted ? policy.stages.at(-1) : null), 'Forged accepted cohort');
  if (report.ok || report.playableCapacityAccepted) {
    check(samples?.length > 0, 'Passing capacity report requires its raw samples');
    same(samples.filter(row => row.type === 'stageComplete').map(row => row.stage), report.stages);
    const finished = samples.filter(row => row.type === 'finished');
    check(finished.length === 1 && finished[0].ok === report.ok && finished[0].playableCapacityAccepted === report.playableCapacityAccepted, 'Missing or forged raw finished status');
    check(!samples.some(row => row.type === 'continuousFailure') && !samples.some(row => row.type === 'monitor' && row.sample?.stop === true), 'Raw capacity samples contain a failure');
    for (const stage of report.stages) for (const assessment of [stage, ...(stage.rollingWindows ?? [])]) {
      check(assessment.passed === true && assessment.movementPassed === true && assessment.chatPassed === true &&
        assessment.activity?.length > 0 && assessment.activity.every(actor => actor.passed && actor.nativeActivity?.passed !== false) &&
        assessment.timeoutRatio < policy.timeoutExclusive && assessment.correctionRatio <= policy.maximumCorrectionRatio, 'Passing stage contains a failed assessment');
      if (assessment.aoi) check(assessment.aoi.samples > 0 && ['persistentMissing', 'ghosts', 'duplicates'].every(key => assessment.aoi[key] === 0), 'AOI acceptance contradicts counters');
    }
    check(report.stages.every(stage => stage.fanoutPassed === true), 'Passing stage contains a failed fanout gate');
  }
  if (report.playableCapacityAccepted) {
    verifyCapacityPhases(report, samples);
    check(policy.profile === 'standard' && policy.activityMode === 'native' && policy.soakSeconds >= 3600 && policy.stageSeconds >= 120 &&
      policy.resumeSamples >= 1 && policy.saveSamples >= 1 && final.name === 'soak' && final.seconds === policy.soakSeconds &&
      report.peakControlledActors >= policy.stages.at(-1) && report.highestPassedStage === policy.stages.at(-1) &&
      report.memoryTrend?.seconds >= 1800 && Number.isFinite(report.memoryTrend.slopeMiBPerHour) && report.memoryTrend.slopeMiBPerHour <= policy.maximumMemorySlopeMiBPerHour &&
      report.nativeResume?.length === policy.resumeSamples && report.nativeResume.every(value => value.status === 'passed'), 'Capacity acceptance lacks full duration, memory or resume scope');
    const strict = { minimumCombatAttacksPerMinute: 18, minimumCombatHitsPerMinute: 12, minimumMovesPerMinute: 30,
      minimumNativeRunRatio: .5, minimumNativeCellsPerMinute: 80, maximumMissedPlanRatio: .05, maximumCorrectionRatio: .05,
      movementP95Ms: 750, chatP95Ms: 1000, timeoutExclusive: .01, maximumMemorySlopeMiBPerHour: 32 };
    check(Object.entries(strict).every(([key, expected]) => policy[key] === expected), 'Accepted capacity policy weakens the recorded normal gates');
    let monitorSummary = streamedMonitors;
    if (!monitorSummary) {
      const projection = new MonitorProjection(report.phaseWindows, true);
      samples.filter(row => row.type === 'monitor').forEach(row => projection.add(row.sample)); monitorSummary = projection.snapshot();
    }
    check(monitorSummary.complete && monitorSummary.count >= 2 && monitorSummary.stoppedCount === 0 && monitorSummary.maxGapMs <= 15000 &&
      !samples.some(row => row.type === 'safetyStop'), 'Accepted capacity has no complete continuous raw monitor interval');
    for (const stage of report.stages) {
      const phase = monitorSummary.phaseBrackets.find(value => value.name === stage.name);
      check(phase && phase.startedAtMs === stage.startedAtMs && phase.endedAtMs === stage.endedAtMs &&
        phase.beforeStart && phase.afterStart && phase.beforeEnd && phase.afterStart.sampledAtMs <= phase.beforeEnd.sampledAtMs &&
        [phase.startBeforeGapMs, phase.startAfterGapMs, phase.endBeforeGapMs].every(gap => gap >= 0 && gap <= 15000) &&
        (phase.endAfterGapMs === null || phase.endAfterGapMs <= 15000), 'Raw monitor observations do not cover a measured phase within the original freshness bound');
    }
  }
  // A shorter authentic Node run remains a diagnostic for this round's profile.
  return report.playableCapacityAccepted === true && policy.stageSeconds >= 300 && policy.soakSeconds >= 7200;
}

export async function verifyBackend(root, workspace, commit) {
  const frozen = await readJson(path.join(root, 'backend-source-frozen.json'));
  const pin = await readJson(path.join(root, 'evidence-tools/normalized-code-pin.json'));
  check(frozen.schemaVersion === 1 && frozen.files?.length === 32 && sha(Buffer.from(JSON.stringify(frozen.files))) === frozen.sourceManifestSha256, 'Invalid frozen backend manifest');
  check(pin.schemaVersion === 1 && pin.files?.length === 32 && pin.frozenSourceManifestSha256 === frozen.sourceManifestSha256 &&
    pin.frozenGitBase === frozen.gitBase && sha(Buffer.from(JSON.stringify(pin.files))) === pin.normalizedManifestSha256, 'Invalid normalized code pin binding');
  if (commit) check(/^[a-f0-9]{40}$/.test(commit), 'Historical backend commit must be a full 40-character SHA');
  for (const [index, row] of frozen.files.entries()) {
    const normal = pin.files[index];
    check(safeRelative(row.path) && /^mir2-web3\/(?:apps\/(?:gateway|simulation)\/.*\.rs|(?:.*\/)?Cargo\.(?:toml|lock)|\.cargo\/.*)$/.test(row.path) &&
      normal.path === row.path && normal.rawBytes === row.bytes && normal.rawSha256 === row.sha256, 'Code normalization attempted outside frozen code inputs');
    const bytes = commit ? git(workspace, ['cat-file', 'blob', `${commit}:${row.path}`]) : await fs.readFile(path.join(workspace, row.path));
    if (!commit) check(bytes.length === row.bytes && sha(bytes) === row.sha256, 'Current backend differs from release freeze; supply the immutable historical commit after P2');
    const normalized = normalizedCode(bytes);
    check(normalized.length === normal.lfBytes && sha(normalized) === normal.lfSha256, 'Historical/current backend code differs from explicit LF pin');
  }
  check(pin.releaseBuild?.profile === 'release' && /^[a-f0-9]{64}$/.test(pin.releaseBuild.sha256) &&
    pin.releaseBuild.sourceManifestSha256 === frozen.sourceManifestSha256, 'Missing immutable release/source binding');
  return { ...frozen, releaseBuild: pin.releaseBuild };
}

export async function verifyTopic({ root, indexFile, workspace, backendCommit, requireCapacity = false, archiveRoot }) {
  const files = await verifyInventory(root, indexFile), names = new Set(files.map(row => row.path));
  const relativeRoot = path.relative(workspace, root).split(path.sep).join('/'); check(safeRelative(relativeRoot), 'Topic must be inside its Git workspace');
  const tracked = [...git(workspace, ['ls-files', '-z', '--', relativeRoot]).toString('utf8').split('\0'),
    ...git(workspace, ['ls-tree', '-r', '-z', '--name-only', 'HEAD', '--', relativeRoot]).toString('utf8').split('\0')].filter(Boolean);
  for (const file of tracked) if (path.resolve(workspace, file) !== indexFile) check(names.has(path.posix.relative(relativeRoot, file)), 'Committed/staged evidence is missing from the index');
  const frozen = await verifyBackend(root, workspace, backendCommit);
  const read = name => { check(names.has(name), 'Required evidence is missing from the index'); return readJson(path.join(root, name)); };
  const lines = async name => {
    check(names.has(name), 'Required raw trace/samples missing from index');
    return (await fs.readFile(path.join(root, name), 'utf8')).trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  };
  const archived = new Map(), archivedIds = new Set(), archivedPaths = new Set(); let archivedFiles = 0, archivedRows = 0, archivedMonitorRows = 0, invalidMonitorRows = 0;
  for (const name of names) if (path.posix.basename(name) === 'raw-artifact-manifest.json') {
    const manifest = await read(name);
    check(manifest.schemaVersion === 1 && Array.isArray(manifest.files) && manifest.files.length > 0 && manifest.files.length <= 256, 'Invalid raw artifact manifest');
    for (const entry of manifest.files) {
      check(names.has(entry.report) && !archived.has(entry.report) && !archivedIds.has(entry.runId), 'Missing or duplicate archive/report binding');
      if (entry.projection != null) check(safeRelative(entry.projection) && names.has(entry.projection), 'Raw archive projection is outside the indexed topic');
      const file = await resolveArchivedFile(archiveRoot, entry);
      check(!archivedPaths.has(file), 'Raw archive file was reused for another report'); archivedIds.add(entry.runId); archivedPaths.add(file);
      archived.set(entry.report, { entry, file });
    }
  }
  let fullRuns = 0, failedReportsPreserved = 0, capacityReports = 0, capacityAccepted = 0;
  for (const name of names) {
    if (!name.endsWith('.report.json')) continue;
    const run = await read(name), directory = path.posix.dirname(name);
    if (run.profile === 'full-lifecycle') {
      if (run.ok !== true) { check(run.fullLifecyclePassed !== true && run.status !== 'passed', 'Failed lifecycle report was retroactively promoted'); failedReportsPreserved++; continue; }
      const traces = await Promise.all(['a', 'b'].map(label => lines(`${directory}/${run.runId}-${label}.trace.jsonl`)));
      verifyFullLifecycle(run, traces);
      for (const traceName of names) if (traceName.startsWith(`${directory}/${run.runId}-`) && traceName.endsWith('.trace.jsonl')) verifyPublicTrace(await lines(traceName));
      const realms = [];
      for (const candidate of names) if (candidate.endsWith('.realm-evidence.json')) { const value = await read(candidate); if (value.playtest?.runId === run.runId) realms.push({ name: candidate, value }); }
      check(realms.length === 1, 'Full lifecycle needs exactly one matching owned realm record');
      const { name: realmName, value: realm } = realms[0];
      verifyOwnedRealm(realm, run, frozen);
      const metrics = await read(realmName.replace(/\.realm-evidence\.json$/, '.zone-metrics.json'));
      same(metrics.baseline, realm.baselineHealth.zone_metrics); same(metrics.after, realm.afterHealth.zone_metrics);
      for (const key of CLASS) {
        const before = metrics.baseline[key], after = metrics.after[key], delta = metrics.classDeltas?.[key];
        same(delta, { count: after.duration.count - before.duration.count, successCount: after.successCount - before.successCount,
          failureCount: after.failureCount - before.failureCount, sumUs: after.duration.sumUs - before.duration.sumUs,
          overBudgetCount: after.duration.overBudgetCount - before.duration.overBudgetCount });
      }
      for (const [file, digest] of Object.entries(realm.sourcePins ?? {})) {
        check(safeRelative(file) && /^[a-f0-9]{64}$/.test(digest), 'Invalid lifecycle driver source pin');
        // --backend-commit selects historical Rust/Cargo inputs only. The
        // lifecycle driver must still match its recorded raw source pin.
        check(sha(await fs.readFile(path.join(workspace, 'mir2-web3', file))) === digest, 'Lifecycle driver changed after its recorded run');
      }
      const audits = [...names].filter(candidate => candidate === realmName.replace(/\.realm-evidence\.json$/, '.independent-audit.json'));
      check(audits.length === 1 && (await read(audits[0])).binarySha256 === realm.release.sha256, 'Independent release digest mismatch');
      fullRuns++;
    } else if (run.policy && Array.isArray(run.stages)) {
      capacityReports++;
      const sampleName = `${directory}/${run.runId}.samples.jsonl`, external = archived.get(name);
      check(!(names.has(sampleName) && external), 'Capacity raw samples have ambiguous local/archive sources');
      if (external) check(external.entry.runId === run.runId, 'Raw archive run ID differs from report');
      const streamed = external ? await streamCapacitySamples(external.file, external.entry, STREAM_LIMITS, run) : names.has(sampleName)
        ? await streamCapacitySamples(path.join(root, sampleName), files.find(row => row.path === sampleName), STREAM_LIMITS, run) : null;
      if (external) { archivedFiles++; archivedRows += streamed.rows; archivedMonitorRows += streamed.monitorSummary.count; invalidMonitorRows += streamed.monitorSummary.invalidCount; }
      if (external?.entry.projection) same(await read(external.entry.projection), makeCapacityProjection(run.runId, streamed));
      const samples = streamed?.selected ?? [];
      if (verifyCapacityStatus(run, samples, streamed?.monitorSummary)) capacityAccepted++;
    }
  }
  check(archivedFiles === archived.size, 'Raw archive manifest points to a non-capacity report');
  check(fullRuns > 0, 'No fresh complete 16-check lifecycle run');
  if (requireCapacity) check(capacityAccepted > 0, 'No accepted standard/native capacity report; P0 capacity remains pending');
  return { files: files.length, frozenBackendFiles: 32, historicalBackendPins: backendCommit ? 32 : 0, fullRuns,
    failedReportsPreserved, capacityReports, capacityAccepted, archivedFiles, archivedRows, archivedMonitorRows, invalidMonitorRows };
}

async function main() {
  const options = {}, flags = new Set(['--write-index', '--require-capacity']);
  for (let index = 2; index < process.argv.length; index++) {
    const key = process.argv[index];
    check(['--root', '--index', '--backend-commit', '--archive-root', ...flags].includes(key), 'Unknown verifier option');
    options[key] = flags.has(key) ? true : process.argv[++index]; check(options[key], 'Missing verifier argument');
  }
  const root = path.resolve(options['--root'] ?? path.join(path.dirname(fileURLToPath(import.meta.url)), '..'));
  const indexFile = path.resolve(options['--index'] ?? path.join(root, 'artifact-index.json'));
  check(path.dirname(indexFile) === root, 'Index must be directly inside the evidence topic');
  if (options['--write-index']) {
    const files = await inventory(root, indexFile);
    await fs.writeFile(indexFile, JSON.stringify({ schemaVersion: 1, hashMode: 'raw-bytes', createdAt: new Date().toISOString(), files }, null, 2) + '\n', { flag: 'wx' });
    console.log(JSON.stringify({ indexedFiles: files.length })); return;
  }
  const workspace = git(root, ['rev-parse', '--show-toplevel']).toString('utf8').trim();
  const result = await verifyTopic({ root, indexFile, workspace, backendCommit: options['--backend-commit'],
    requireCapacity: options['--require-capacity'] === true, archiveRoot: options['--archive-root'] });
  console.log(JSON.stringify(result));
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main().catch(error => {
  // No source, trace payload, configured value or private path is printed.
  console.error(JSON.stringify({ verified: false, error: error.code ?? (error instanceof assert.AssertionError ? 'evidence mismatch' : error.message) })); process.exitCode = 1;
});
