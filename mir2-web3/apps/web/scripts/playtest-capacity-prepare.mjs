// Ordinary preparation only. This module never starts the capacity engine,
// activates a cohort, seeds accounts, edits the account store, or measures CCU.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';
import { CapacityClient, ordinaryLogin, prepareCapacityActor, drain, awaitPendingActions,
  ordinaryPreparationConfiguration, attackResultsMode, recoverPreparationHeldHp } from './playtest-capacity-soak.mjs';
import { capacityPreparationPolicy, validateCapacityPool, LoginBudget, LOGIN_WINDOW_MS,
  BoundedEvidence, CapacityMonitor, sanitize } from './playtest-capacity-soak-core.mjs';
import { outsideRepository, validateEndpoint } from './playtest-multiplayer-smoke.mjs';
import { loadProtocolCollisionMap, protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';
import { equipStarterGear } from './quest-agent/protocol-play.mjs';

const assert = (ok, message) => { if (!ok) throw new Error(message); };
const defaultServices = Object.freeze({
  createClient: (endpoint, account, context) => new CapacityClient(endpoint, account, context),
  createMonitor: (file, writer) => new CapacityMonitor(file, writer),
  createWriter: file => new BoundedEvidence(file),
  login: ordinaryLogin, recover: recoverPreparationHeldHp, prepare: prepareCapacityActor, drain, awaitPending: awaitPendingActions,
  loadMap: loadProtocolCollisionMap, isWalkable: protocolMapCellIsWalkable, equip: equipStarterGear, now: Date.now, monotonicNow: () => performance.now(),
});

async function regularJson(file) {
  assert(typeof file === 'string' && path.isAbsolute(file), 'An absolute JSON input path is required');
  const stat = await fs.lstat(file);
  assert(stat.isFile() && !stat.isSymbolicLink() && path.resolve(await fs.realpath(file)).toLowerCase() === path.resolve(file).toLowerCase(), 'JSON inputs must be canonical regular files');
  return JSON.parse(await fs.readFile(file, 'utf8'));
}
async function securePoolFile(file) {
  assert(typeof file === 'string' && path.isAbsolute(file), 'An absolute private poolFile is required');
  const absolute = outsideRepository(file), directory = path.dirname(absolute);
  assert(path.basename(directory) === 'private' || path.basename(directory).startsWith('load-private-'), 'Pool must be in an outside-repository private directory');
  const stat = await fs.lstat(directory);
  assert(stat.isDirectory() && !stat.isSymbolicLink() && path.resolve(await fs.realpath(directory)).toLowerCase() === path.resolve(directory).toLowerCase(), 'Private pool directory must be canonical');
  if (process.platform === 'win32') {
    const owner = execFileSync('whoami.exe', [], { encoding: 'utf8', windowsHide: true }).trim();
    execFileSync('icacls.exe', [directory, '/inheritance:r', '/grant:r', owner + ':(OI)(CI)F'], { stdio: 'pipe', windowsHide: true });
  } else await fs.chmod(directory, 0o700);
  return absolute;
}
function integer(value, minimum, maximum, label) {
  assert(Number.isSafeInteger(value) && value >= minimum && value <= maximum, label + ' must be an integer from ' + minimum + ' to ' + maximum);
  return value;
}
function scalar(row) {
  if (!row) return null;
  return Object.fromEntries(['sequence', 'monotonicMs', 'ownerId', 'ownerName', 'map', 'x', 'y', 'hp', 'maxHp',
    'hpStock', 'gold', 'venison', 'inSafeZone', 'nearbyHostileCount', 'currentNearbyHostileCount', 'playerPoison',
    'ownerPoison', 'dazed', 'actualStagingReached', 'stagingProtection'].filter(key => row[key] !== undefined).map(key => [key, row[key]]));
}
function actorEvidence(client, result, readiness) {
  return { name: client.label, characterIndex: client.account.characterIndex,
    before: scalar(result?.before), after: scalar(result?.after), freshReadiness: scalar(readiness),
    targetHp: result?.targetHp ?? null, purchased: result?.purchased ?? null,
    soldVenison: result?.soldVenison ?? null, goldEarned: result?.goldEarned ?? null, goldSpent: result?.goldSpent ?? null,
    funding: result?.funding ? { method: result.funding.method, collected: result.funding.collected, hunts: result.funding.hunts } : null,
    commands: result?.commands ?? null, movementCommands: result?.movementCommands ?? null,
    elapsedMs: result?.elapsedMs ?? null, actualStagingReached: result?.actualStagingReached === true,
    reusedInventoryWithoutEconomicWork: result?.before?.hpStock >= result?.targetHp && result?.purchased === 0 &&
      result?.soldVenison === 0 && result?.funding === null,
    normalLogout: false, durableReloginReadbackPending: true };
}

// This is a planning filter over a frozen ordinary staging catalogue. It never
// moves an entity, changes occupancy or treats offline saved points as Zone state.
export function selectPreparationStagingPoint({ snapshot, receipt, defaultPoint, candidates, savedPoints }) {
  const owner = snapshot?.entities?.find(e => e.objectId === snapshot.playerObjectId);
  assert(owner && receipt?.ownerId === snapshot.playerObjectId && receipt.ownerName === owner.name &&
    receipt.map === '0' && String(snapshot.mapFileName) === '0' && Number.isSafeInteger(owner.x) && Number.isSafeInteger(owner.y) &&
    receipt.x === owner.x && receipt.y === owner.y && Number.isSafeInteger(receipt.sequence) && receipt.sequence > 0 &&
    Number.isFinite(receipt.monotonicMs), 'Fresh same-owner map0 staging snapshot required');
  const occupied = new Set();
  for (const entity of snapshot.entities ?? []) {
    if (entity.objectId === snapshot.playerObjectId || entity.dead === true) continue;
    if (['player', 'selfplayer', 'remoteplayer', 'monster', 'npc'].includes(String(entity.kind).toLowerCase()) &&
        Number.isSafeInteger(entity.x) && Number.isSafeInteger(entity.y)) occupied.add(entity.x + ',' + entity.y);
  }
  const declared = new Set(candidates.map(p => p.x + ',' + p.y));
  const order = [defaultPoint, ...candidates];
  for (const point of order) {
    const key = point.x + ',' + point.y;
    if (!declared.has(key) || savedPoints.has(key) || occupied.has(key) ||
        Math.max(Math.abs(owner.x - point.x), Math.abs(owner.y - point.y)) > 16 ||
        (snapshot.mapTransfers ?? []).some(t => t.bounds && point.x >= t.bounds.minX && point.x <= t.bounds.maxX &&
          point.y >= t.bounds.minY && point.y <= t.bounds.maxY)) continue;
    return { x: point.x, y: point.y };
  }
  throw new Error('No fresh visible unoccupied distinct declared staging candidate');
}


// Public history proves earlier normal saves only. It is not a current Store,
// resource, fresh-login or durable-readback attestation.
const continuationMode = 'historical-prepared-index-v1';
const cleanupCheckNames = ['sixCountersZero', 'monitorStoppedBeforeGateway', 'monitorExitZero',
  'monitorTerminalPreserved', 'monitorSamplerAbsent', 'normalOwnedShutdown', 'gatewayProcessAbsent',
  'driverProcessAbsent', 'monitorProcessAbsent', 'portsFree', 'postSourceUnchanged'];
const sameJson = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const normalizedPath = (file, platform = process.platform) => {
  const resolved = path.resolve(file).replaceAll('\\', '/');
  return platform === 'win32' ? resolved.toLowerCase() : resolved;
};
// Path identity only; this does not authorize reading an evidence file.
export function sameContinuationEvidencePath(left, right, platform = process.platform) {
  return normalizedPath(left, platform) === normalizedPath(right, platform);
}
function evidenceReference(value) {
  assert(value && typeof value.file === 'string' && path.isAbsolute(value.file) &&
    Number.isSafeInteger(value.bytes) && value.bytes > 0 && value.bytes <= 4 * 1024 * 1024 &&
    typeof value.sha256 === 'string' && /^[0-9a-f]{64}$/.test(value.sha256), 'Invalid continuation evidence pin');
  return Object.freeze({ file: path.resolve(value.file), bytes: value.bytes, sha256: value.sha256 });
}
function sameEvidence(a, b, allowArchiveCopy = false) {
  const left = evidenceReference(a), right = evidenceReference(b);
  return left.bytes === right.bytes && left.sha256 === right.sha256 &&
    (normalizedPath(left.file) === normalizedPath(right.file) ||
      (allowArchiveCopy && path.basename(left.file).toLowerCase() === path.basename(right.file).toLowerCase()));
}
async function pinnedHistoryJson(pin) {
  let cursor = path.resolve(pin.file), stat;
  for (;;) {
    const current = await fs.lstat(cursor);
    assert(!current.isSymbolicLink() && sameContinuationEvidencePath(await fs.realpath(cursor), cursor),
      'Continuation evidence must be canonical regular files without symlink ancestors');
    if (cursor === path.resolve(pin.file)) stat = current;
    const parent = path.dirname(cursor); if (parent === cursor) break; cursor = parent;
  }
  assert(stat.isFile(), 'Continuation evidence must be regular files');
  assert(stat.size === pin.bytes, 'Continuation evidence byte count differs');
  const bytes = await fs.readFile(pin.file);
  assert(bytes.length === pin.bytes && crypto.createHash('sha256').update(bytes).digest('hex') === pin.sha256,
    'Continuation evidence raw SHA differs');
  return JSON.parse(bytes.toString('utf8'));
}
function deepFreeze(value) {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const item of Object.values(value)) deepFreeze(item);
    Object.freeze(value);
  }
  return value;
}
/** Read-only preflight: no client, auth ledger, Store mutation or login occurs. */
export async function validatePreparationContinuation(continuation, {
  pool, endpoint, maxPrepared, targetHp, stagingPoints, stagingCandidates, map,
  isWalkable = protocolMapCellIsWalkable,
}) {
  assert(continuation?.mode === continuationMode && Array.isArray(continuation.evidence) &&
    continuation.evidence.length > 0 && continuation.evidence.length <= 100,
    'Explicit historical-prepared-index-v1 evidence groups are required');
  assert(Object.keys(continuation).every(k => ['mode', 'evidence'].includes(k)), 'Unknown continuation input');
  const accounts = validateCapacityPool(pool, endpoint).accounts;
  assert(maxPrepared === 100 && accounts.length === 100, 'Continuation requires the complete original100 pool and maxPrepared100');
  capacityPreparationPolicy({ preparationProfile: 'ordinary-economic-v1', preparationHpTarget: targetHp });
  assert(Array.isArray(stagingPoints) && stagingPoints.length === 100 && Array.isArray(stagingCandidates) &&
    stagingCandidates.length === 335 && typeof isWalkable === 'function' && map,
    'Continuation requires original100 staging indices and frozen335 walkable candidates');
  const pointKey = p => p.x + ',' + p.y;
  const validPoint = p => Number.isSafeInteger(p?.x) && Number.isSafeInteger(p?.y) && p.x >= 0 && p.y >= 0 &&
    p.x <= 65535 && p.y <= 65535 && (p.mapFileName == null || String(p.mapFileName) === '0') && isWalkable(map, p);
  assert(stagingCandidates.every(validPoint) && new Set(stagingCandidates.map(pointKey)).size === 335 &&
    stagingPoints.every(p => validPoint(p) && stagingCandidates.some(c => pointKey(c) === pointKey(p))) &&
    new Set(stagingPoints.map(pointKey)).size === 100, 'Continuation staging catalogue differs');
  const groups = continuation.evidence.map(group => {
    assert(group && Object.keys(group).length === 4 && ['report', 'preparedIndex', 'cleanup', 'receipt'].every(k => Object.hasOwn(group, k)),
      'Continuation groups require exactly report/index/cleanup/receipt pins');
    return Object.fromEntries(Object.entries(group).map(([key, pin]) => [key, evidenceReference(pin)]));
  });
  assert(groups.reduce((n, group) => n + Object.values(group).reduce((m, pin) => m + pin.bytes, 0), 0) <= 32 * 1024 * 1024,
    'Continuation public evidence exceeds the bounded32MiB input');
  const historicalPreparedIndex = [], sourceOriginalVerdicts = [], names = new Set(), characters = new Set(), points = new Set();
  for (const group of groups) {
    const [report, index, cleanup, receipt] = await Promise.all(['report', 'preparedIndex', 'cleanup', 'receipt'].map(k => pinnedHistoryJson(group[k])));
    assert(receipt.schemaVersion === 1 && index.schemaVersion === 1 && cleanup.schemaVersion === 1 &&
      typeof receipt.packageId === 'string' && receipt.packageId === index.packageId && receipt.packageId === cleanup.packageId,
      'Continuation source package identities differ');
    assert(sameEvidence(receipt.rawOriginalReport, group.report, true) && sameEvidence(receipt.preparedIndexRecord, group.preparedIndex, true) &&
      sameEvidence(receipt.cleanupRecord, group.cleanup, true) && sameEvidence(index.sourceOriginalReport, receipt.rawOriginalReport) &&
      normalizedPath(report.reportPath) === normalizedPath(receipt.rawOriginalReport.file), 'Continuation evidence cross-binding differs');
    assert(report.schemaVersion === 1 && ['ordinary-economic-preparation-only', 'ordinary-economic-continuation-only'].includes(report.mode) &&
      typeof report.ok === 'boolean' && typeof report.ordinaryPreparationEvidenceEligible === 'boolean' && report.preparationOnly === true && report.testExecution === false && report.attackResults === 'v2' &&
      report.workerConcurrency === 1 && report.maxPrepared === 100 && report.completePoolAccountCount === 100 && report.completePoolPreserved === true &&
      report.measurementPerformed === false && report.cohortActivated === false && report.capacityAccepted === false && report.p0Accepted === false &&
      Array.isArray(report.cleanupErrors) && report.cleanupErrors.length === 0,
      'Continuation history must be actual ordinary preparation-only evidence, not measurement or a test seam');
    assert(receipt.preparationOnly === true && receipt.workerCount === 1 && receipt.maxPrepared === 100 && receipt.cleanupComplete === true &&
      receipt.measurementPerformed === false && receipt.gameplayEngineActivated === false && receipt.capacityAccepted === false && receipt.p0Accepted === false,
      'Continuation source receipt scope differs');
    const verdict = receipt.rawOriginalVerdict;
    assert(verdict && verdict.ok === report.ok && (verdict.error ?? null) === (report.error ?? null) &&
      verdict.preparedCount === report.preparedCount && verdict.testExecution === false && verdict.attackResults === 'v2' &&
      verdict.ordinaryPreparationEvidenceEligible === report.ordinaryPreparationEvidenceEligible,
      'Continuation original verdict must remain unchanged');
    assert(cleanupCheckNames.every(k => cleanup.checks?.[k] === true) && Object.values(cleanup.checks).every(v => v === true) &&
      ['gatewayExit', 'monitorExit'].every(k => cleanup[k]?.exitCode === 0 && cleanup[k].signal === null && cleanup[k].error === null) &&
      [0, 1].includes(cleanup.driverExit?.exitCode) && cleanup.driverExit.signal === null && cleanup.driverExit.error === null &&
      ['remainingListeners', 'remainingMonitorChildren', 'logErrors'].every(k => Array.isArray(cleanup[k]) && cleanup[k].length === 0),
      'Continuation source normal cleanup is incomplete');
    assert(Number.isSafeInteger(report.preparedCount) && report.preparedCount > 0 && report.preparedCount <= 100 &&
      Array.isArray(report.actorResults) && Array.isArray(report.preparedIndex) && Array.isArray(index.records) &&
      report.actorResults.length === report.preparedCount && report.preparedIndex.length === report.preparedCount &&
      sameJson(report.preparedIndex, index.records), 'Continuation successful index/count differs');
    if (report.failedActor) {
      assert(typeof report.failedActor.name === 'string' && !report.actorResults.some(a => a.name === report.failedActor.name),
        'A failed actor cannot be historical prepared');
    }
    for (let i = 0; i < index.records.length; i++) {
      const row = index.records[i], actor = report.actorResults[i], originalPoolIndex = historicalPreparedIndex.length;
      const account = accounts[originalPoolIndex];
      assert(account && row.name === account.name && actor.name === account.name && account.state === 'ready' &&
        Number.isSafeInteger(row.characterIndex) && row.characterIndex > 0 && row.characterIndex === account.characterIndex && actor.characterIndex === row.characterIndex &&
        !names.has(row.name) && !characters.has(row.characterIndex), 'Continuation must be the disjoint original pool prefix in order');
      assert(row.map === '0' && validPoint(row) && stagingCandidates.some(c => pointKey(c) === pointKey(row)) && !points.has(pointKey(row)) &&
        row.normalLogout === true && row.inSafeZone === true && actor.normalLogout === true && actor.finalNormalDrainPositionMatched === true &&
        actor.actualStagingReached === true && Number.isSafeInteger(actor.targetHp) && actor.targetHp >= targetHp,
        'Continuation historical normal save, stock target or staging reservation differs');
      for (const proof of [actor.after, actor.freshReadiness]) {
        assert(proof && proof.ownerName === row.name && proof.map === row.map && proof.x === row.x && proof.y === row.y &&
          Number.isSafeInteger(proof.ownerId) && proof.ownerId > 0 && Number.isSafeInteger(proof.sequence) && proof.sequence > 0 &&
          Number.isFinite(proof.monotonicMs) && proof.monotonicMs >= 0 && Number.isSafeInteger(proof.hpStock) && proof.hpStock >= targetHp &&
          Number.isFinite(proof.hp) && Number.isFinite(proof.maxHp) && proof.maxHp > 0 && proof.hp > 0 && proof.hp / proof.maxHp >= .7 &&
          proof.inSafeZone === true && proof.actualStagingReached === true, 'Continuation historical fresh readiness differs');
      }
      assert(actor.after.ownerId === actor.freshReadiness.ownerId && actor.freshReadiness.sequence > actor.after.sequence &&
        actor.freshReadiness.monotonicMs >= actor.after.monotonicMs && row.sequence === actor.freshReadiness.sequence &&
        row.monotonicMs === actor.freshReadiness.monotonicMs, 'Continuation historical receipt ordering differs');
      names.add(row.name); characters.add(row.characterIndex); points.add(pointKey(row));
      historicalPreparedIndex.push({ name: row.name, characterIndex: row.characterIndex, map: row.map, x: row.x, y: row.y,
        sequence: row.sequence, monotonicMs: row.monotonicMs, inSafeZone: true, normalLogout: true,
        originalPoolIndex, origin: 'historical', freshThisRun: false, sourceReportPin: group.report,
        durableReloginReadbackPending: true });
    }
    sourceOriginalVerdicts.push({ sourceReportPin: group.report, ok: report.ok, error: report.error ?? null, preparedCount: report.preparedCount });
  }
  assert(historicalPreparedIndex.length > 0 && historicalPreparedIndex.length < accounts.length, 'Historical failed actors, duplicate groups or an empty complement are not permitted');
  return deepFreeze({ mode: continuationMode, scope: 'public-history-not-fresh; no current Store or auth provenance attested',
    historicalPreparedCount: historicalPreparedIndex.length, expectedThisRun: accounts.length - historicalPreparedIndex.length,
    historicalNotFresh: true, historicalPreparedIndex, evidence: groups, sourceOriginalVerdicts,
    remainingPoolIndices: accounts.map((_, index) => index).slice(historicalPreparedIndex.length) });
}

/**
 * Prepare a prefix of one complete ordinary account pool, with one live worker.
 * options: realm, endpoint, poolFile, monitor, output, scenarioFile, mapRoot,
 * maxPrepared, finiteWallSeconds, targetHp (default12), optional routeFile,
 * loginNotBefore (only for an absent ledger), signal and onProgress.
 * The optional second argument is a unit-test seam. Its results are explicitly
 * ineligible as ordinary runtime evidence; production wrappers omit it.
 */
export async function runPreparationBank(options, testServices) {
  assert(options?.realm === 'isolated', 'Preparation requires realm isolated');
  const continuation = options.continuation;
  const endpoint = validateEndpoint(options.endpoint);
  const attackResults = attackResultsMode(options.attackResults ?? 'v2');
  assert(attackResults === 'v2', 'Ordinary preparation requires attackResults v2; legacy is not permitted');
  const maxPrepared = integer(Number(options.maxPrepared), 1, 100, 'maxPrepared');
  const finiteWallSeconds = integer(Number(options.finiteWallSeconds), 1, 43200, 'finiteWallSeconds');
  const preparation = capacityPreparationPolicy({ preparationProfile: 'ordinary-economic-v1', preparationHpTarget: options.targetHp ?? 12 });
  assert(typeof options.monitor === 'string' && path.isAbsolute(options.monitor), 'An absolute real monitor file is required');
  assert(typeof options.output === 'string' && path.isAbsolute(options.output), 'An absolute outside-repository output is required');
  assert(typeof options.mapRoot === 'string' && path.isAbsolute(options.mapRoot), 'An absolute mapRoot is required');
  assert(options.onProgress == null || typeof options.onProgress === 'function', 'onProgress must be a function');
  assert(options.signal == null || (typeof options.signal.addEventListener === 'function' && typeof options.signal.removeEventListener === 'function' &&
    typeof options.signal.aborted === 'boolean'), 'signal must be an AbortSignal');
  const services = { ...defaultServices, ...testServices }, testExecution = testServices !== undefined;
  const output = outsideRepository(options.output), accountFile = await securePoolFile(options.poolFile);
  await fs.mkdir(output, { recursive: true });
  const runId = Date.now() + '-' + crypto.randomBytes(4).toString('hex');
  const samplesPath = path.join(output, runId + '.preparation.jsonl'), reportPath = path.join(output, runId + '.preparation-report.json');
  const writer = services.createWriter(samplesPath), guard = services.createMonitor(options.monitor, writer);
  const policy = Object.freeze({ activityMode: 'native', actionTimeoutMs: 2000, ...preparation });
  const report = { schemaVersion: 1, preparationOnly: true, mode: 'ordinary-economic-preparation-only', attackResults, runId, reportPath, samplesPath,
    startedAt: new Date().toISOString(), maxPrepared, targetHp: preparation.preparationHpTarget, finiteWallSeconds,
    workerConcurrency: 1, preparedCount: 0, actorResults: [], preparedIndex: [], checkpoints: [], cleanupErrors: [],
    stockDefinition: 'Helper fresh complete-inventory plus belt HP658 quantity; HP guard scalars are not independent healing proof.',
    testExecution, ordinaryPreparationEvidenceEligible: false, measurementPerformed: false, cohortActivated: false,
    coldBankCopyPerformed: false, sustainableTwoHourSupplyProven: false, capacityAccepted: false, p0Accepted: false,
    durableReloginVerificationPerformed: false };
  if (continuation !== undefined) Object.assign(report, { mode: 'ordinary-economic-continuation-only',
    continuationMode, historicalPreparedCount: null, historicalNotFresh: true, historicalPreparedIndex: [],
    historicalEvidence: [], sourceOriginalVerdicts: [], expectedThisRun: null, newPreparedCount: 0, combinedSavedCount: null,
    all100FreshPreparedThisRun: false, fullDurableReadinessVerified: false, continuationPreparationEvidenceEligible: false,
    first4FreshReusePerformedThisRun: false });
  const context = { endpoint, policy, attackResults, writer, guard, accountFile, ledgerFile: path.join(path.dirname(accountFile), 'capacity-auth-budget.json'),
    phase: 'preparation', clients: [], allClients: [], cleanupErrors: report.cleanupErrors,
    add(sample) { writer.add({ type: 'preparationAction', phase: 'preparation', at: Date.now(), ...sanitize(sample, this.pool?.accounts.map(a => a.password) ?? []) }); } };
  const savedPoints = new Set();
  let lock, timer, actorTimer, current = null, poolIdentity, unpreparedTail, initialCount = 0, continuationPlan, historicalPoolBefore;
  const deadline = services.now() + finiteWallSeconds * 1000;
  const stop = reason => guard.stop(reason), onSignal = () => stop('Operator requested normal preparation drain');
  const onAbort = () => stop('Preparation AbortSignal requested normal drain');
  const check = () => { if (services.now() >= deadline) stop('Finite preparation wall-clock budget exhausted'); guard.assert(); };
  const errorText = error => sanitize(String(error?.message ?? error), [...(context.pool?.accounts.map(a => a.password) ?? []), ...(current?.secrets ?? []), ...(current?.recentSecrets ?? [])]);
  const progress = value => { writer.add({ type: 'preparationProgress', at: Date.now(), ...value }); options.onProgress?.(value); };
  async function closeCurrent() {
    if (!current) return null;
    const client = current, errorsBefore = context.cleanupErrors.length;
    client.paused = true; client.preparationCommandBudget = null;
    await services.awaitPending([client]);
    client.ordinarySupplies?.dispose();
    const saved = await services.drain(client, context);
    if (client.closed && !client.inGame) {
      context.clients = context.clients.filter(item => item !== client);
      context.allClients = context.allClients.filter(item => item !== client);
      current = null;
    }
    assert(context.cleanupErrors.length === errorsBefore, 'Normal preparation drain reported an error');
    assert(saved && client.closed && !client.inGame, 'Normal preparation logout or transport close was not proved');
    return saved;
  }
  process.on('SIGINT', onSignal); process.on('SIGTERM', onSignal);
  options.signal?.addEventListener('abort', onAbort, { once: true });
  try {
    if (options.signal?.aborted) onAbort();
    timer = setTimeout(() => stop('Finite preparation wall-clock budget exhausted'), finiteWallSeconds * 1000);
    await guard.start(); check();
    lock = await fs.open(path.join(path.dirname(accountFile), 'capacity-run.lock'), 'wx', 0o600);
    await lock.writeFile(JSON.stringify({ runId, pid: process.pid, endpoint, mode: report.mode }));
    context.pool = validateCapacityPool(await regularJson(accountFile), endpoint);
    initialCount = context.pool.accounts.length;
    assert(maxPrepared <= initialCount, 'maxPrepared exceeds the complete ordinary pool');
    poolIdentity = context.pool.accounts.map(a => ({ accountId: a.accountId, name: a.name }));
    unpreparedTail = JSON.stringify(context.pool.accounts.slice(maxPrepared));
    report.completePoolAccountCount = initialCount;
    report.initialReadyCharacters = context.pool.accounts.filter(a => a.state === 'ready').length;
    let ledger;
    try { ledger = await regularJson(context.ledgerFile); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    if (continuation !== undefined) assert(ledger, 'Continuation requires the existing original auth ledger');
    if (ledger) {
      assert(ledger.schemaVersion === 1 && ledger.purpose === 'capacity-auth-budget' && ledger.endpoint === endpoint, 'Existing auth ledger belongs to another endpoint or schema');
      assert(options.loginNotBefore === undefined, 'Existing auth ledger must not be overridden by loginNotBefore');
    }
    const notBefore = ledger ? 0 : options.loginNotBefore === undefined ? services.now() + LOGIN_WINDOW_MS : Date.parse(options.loginNotBefore);
    assert(Number.isFinite(notBefore), 'loginNotBefore must be an explicit valid ISO time');
    context.loginBudget = new LoginBudget(ledger?.attempts ?? [], notBefore);
    report.authBudget = { scope: 'One unchanged persistent ledger shared across this complete pool; admission25/verification5, total30 per901s client budget; server peer/account limits remain independent.',
      inherited: Boolean(ledger), initialAttempts: context.loginBudget.attempts.length, initialNotBefore: new Date(notBefore).toISOString(), explicitFreshOrigin: !ledger && options.loginNotBefore !== undefined };
    let scenario = await regularJson(options.scenarioFile);
    if (!scenario.preparation?.route) {
      assert(options.routeFile, 'An authoritative routeFile or inline preparation.route is required');
      scenario = { ...scenario, preparation: { ...scenario.preparation, route: await regularJson(options.routeFile) } };
    }
    const configuration = ordinaryPreparationConfiguration({ ...policy, stages: [initialCount] }, scenario);
    assert(String(scenario.mapFileName ?? '0') === '0', 'Ordinary preparation cannot change maps');
    context.mapName = '0';
    context.map = await services.loadMap('0', { mapRoot: options.mapRoot, packagedMapRoot: options.mapRoot });
    let candidates = null;
    if (scenario.preparation.stagingCandidates !== undefined) {
      candidates = scenario.preparation.stagingCandidates;
      assert(scenario.preparation.candidatePolicy === 'fresh-unoccupied-distinct-saved-v1' && Array.isArray(candidates) && candidates.length === 335,
        'Explicit frozen335 staging candidates and policy are required');
      assert(candidates.every(p => Number.isSafeInteger(p?.x) && Number.isSafeInteger(p?.y) && p.x >= 0 && p.y >= 0 && p.x <= 65535 && p.y <= 65535 &&
        (p.mapFileName == null || String(p.mapFileName) === '0') && services.isWalkable(context.map, p)), 'Staging candidates must be ordinary walkable map0 cells');
      candidates = candidates.map(({ x, y }) => Object.freeze({ x, y }));
      const declared = new Set(candidates.map(p => p.x + ',' + p.y));
      assert(declared.size === 335 && configuration.stagingPoints.every(p => declared.has(p.x + ',' + p.y)), 'Preferred points must be contained in the complete distinct frozen catalogue');
      report.stagingSelection = { policy: scenario.preparation.candidatePolicy, candidateCount: 335,
        scope: 'One late synchronous choice; current same-map visible unoccupied cells and prior successfully saved assignments in this run only. No Zone occupancy or saved-position edits.' };
    }

    if (continuation !== undefined) {
      continuationPlan = await validatePreparationContinuation(continuation, { pool: context.pool, endpoint, maxPrepared,
        targetHp: preparation.preparationHpTarget, stagingPoints: configuration.stagingPoints,
        stagingCandidates: candidates, map: context.map, isWalkable: services.isWalkable });
      historicalPoolBefore = JSON.stringify(context.pool.accounts.slice(0, continuationPlan.historicalPreparedCount));
      Object.assign(report, { historicalPreparedCount: continuationPlan.historicalPreparedCount,
        historicalPreparedIndex: continuationPlan.historicalPreparedIndex, historicalEvidence: continuationPlan.evidence,
        sourceOriginalVerdicts: continuationPlan.sourceOriginalVerdicts, expectedThisRun: continuationPlan.expectedThisRun,
        combinedSavedCount: continuationPlan.historicalPreparedCount });
      for (const point of continuationPlan.historicalPreparedIndex) savedPoints.add(point.x + ',' + point.y);
      report.stagingSelection.scope = 'Historical verified normal-save reservations plus current-run saved assignments; historical actors are not fresh or live Zone occupancy.';
    }
    const selectedIndices = continuationPlan?.remainingPoolIndices ?? Array.from({ length: maxPrepared }, (_, index) => index);
    for (const index of selectedIndices) {
      check();
      current = services.createClient(endpoint, context.pool.accounts[index], context);
      current.ready = false; context.clients.push(current); context.allClients.push(current);
      await services.login(current, context, 'admission'); check();
      assert(String(current.snapshot?.mapFileName) === '0', 'Ordinary character is not on preparation map0');
      const actorStartedAtMs = services.monotonicNow(), actorDeadlineMs = actorStartedAtMs + 600000;
      current.preparationCommandBudget = { maximum: 2048, sent: 0 };
      actorTimer = setTimeout(() => stop('Preparation actor600s budget exhausted'), 600000);
      const recovery = await services.recover(current, context, { actorDeadlineMs }); check();
      assert(services.monotonicNow() < actorDeadlineMs, 'Preparation actor600s budget exhausted');
      assert(['verified', 'not-needed'].includes(recovery?.status) && recovery.preparationOnly === true && recovery.measurementCredit === false, 'Ordinary held HP recovery evidence is incomplete');
      await services.equip(current); check();
      const remainingActorMs = Math.floor(actorDeadlineMs - services.monotonicNow());
      assert(remainingActorMs > 0, 'Preparation actor600s budget exhausted');
      const preHelperCommands = current.preparationCommandBudget.sent, helperCommandBudget = 2048 - preHelperCommands - 1;
      assert(helperCommandBudget > 0, 'Preparation shared2048 command budget exhausted');
      const result = await services.prepare(current, context, { route: configuration.route, stagingPoint: configuration.stagingPoints[index], timeoutMs: remainingActorMs, maxCommands: helperCommandBudget,
        ...(candidates ? { resolveStagingPoint: input => selectPreparationStagingPoint({ ...input, candidates, savedPoints }) } : {}) });
      check();
      const cursor = current.sequence;
      await current.refresh(); check();
      const readiness = current.ordinarySupplies.assertReady({ afterSequence: cursor });
      assert(result.status === 'ready' && current.ready === false && readiness.ownerName === current.label && readiness.map === '0' &&
        readiness.hpStock >= preparation.preparationHpTarget && result.after?.ownerName === current.label && (!candidates || (readiness.inSafeZone === true && result.resolvedStagingPoint?.x === readiness.x && result.resolvedStagingPoint?.y === readiness.y)), 'Fresh ordinary preparation evidence is incomplete');
      const evidence = actorEvidence(current, result, readiness);
      evidence.preparationRecovery = recovery; evidence.actorBudgetMs = 600000; evidence.helperBudgetMs = remainingActorMs;
      evidence.preHelperCommands = preHelperCommands; evidence.helperCommandBudget = helperCommandBudget;
      evidence.totalPreparationCommandsBeforeDrain = current.preparationCommandBudget.sent;
      assert(services.monotonicNow() < actorDeadlineMs, 'Preparation actor600s budget exhausted');
      clearTimeout(actorTimer); actorTimer = null;
      const saved = await closeCurrent();
      assert(saved.account.name === evidence.name && saved.account.characterIndex === evidence.characterIndex, 'Normal saved identity changed');
      assert(saved.state.name === evidence.name && saved.state.name === readiness.ownerName && saved.state.mapFileName === '0' &&
        saved.state.mapFileName === readiness.map && Number.isSafeInteger(saved.state.x) && Number.isSafeInteger(saved.state.y) &&
        saved.state.x === readiness.x && saved.state.y === readiness.y, 'Final normal saved position or identity differs from fresh staging readiness');
      evidence.normalLogout = true;
      evidence.finalNormalDrainPositionMatched = true;
      const assigned = { name: evidence.name, characterIndex: evidence.characterIndex, map: '0', x: saved.state.x, y: saved.state.y,
        sequence: readiness.sequence, monotonicMs: readiness.monotonicMs, inSafeZone: readiness.inSafeZone, normalLogout: true, durableReloginReadbackPending: true };
      const pointKey = assigned.x + ',' + assigned.y;
      assert(!savedPoints.has(pointKey), 'Prepared saved staging assignments must remain distinct');
      if (continuationPlan) Object.assign(assigned, { originalPoolIndex: index, origin: 'current-run', freshThisRun: true });
      savedPoints.add(pointKey); report.preparedIndex.push(assigned);
      report.actorResults.push(evidence); report.preparedCount++;
      if (continuationPlan) { report.newPreparedCount = report.preparedCount; report.combinedSavedCount = report.historicalPreparedCount + report.preparedCount; }
      writer.add({ type: 'ordinaryPreparedAndNormallySaved', at: Date.now(), ...evidence });
      progress({ stage: 'prepared', preparedCount: report.preparedCount, target: continuationPlan?.expectedThisRun ?? maxPrepared,
        ...(continuationPlan ? { newPreparedCount: report.preparedCount, historicalPreparedCount: report.historicalPreparedCount,
          expectedThisRun: report.expectedThisRun, combinedSavedCount: report.combinedSavedCount, historicalNotFresh: true } : {}),
        name: evidence.name, characterIndex: evidence.characterIndex });
      if ([25, 50, 100].includes(report.preparedCount)) {
        const checkpoint = { preparedCount: report.preparedCount, at: Date.now(), normalLogouts: report.preparedCount,
          completePoolAccountCount: initialCount, coldCopyPerformed: false, durableReloginVerificationPending: true, capacityAccepted: false };
        report.checkpoints.push(checkpoint); progress({ stage: 'checkpoint', ...checkpoint });
      }
    }
  } catch (error) {
    report.error = errorText(error); stop('Ordinary preparation failed: ' + report.error);
    if (current) {
      let status; try { status = current.ordinarySupplies?.status(); } catch {}
      report.failedActor = { ...actorEvidence(current, status, null), preparationRecovery: current.preparationHpRecoveryResult ?? null, error: report.error };
    }
  } finally {
    clearTimeout(timer); clearTimeout(actorTimer);
    context.phase = 'drain';
    if (current) {
      try { await closeCurrent(); } catch (error) { context.cleanupErrors.push({ step: 'normalActorDrain', error: errorText(error) }); }
    }
    if (context.pool) {
      try {
        const persisted = validateCapacityPool(await regularJson(accountFile), endpoint);
        const identity = persisted.accounts.map(a => ({ accountId: a.accountId, name: a.name }));
        report.completePoolPreserved = persisted.accounts.length === initialCount && JSON.stringify(identity) === JSON.stringify(poolIdentity) &&
          JSON.stringify(persisted.accounts.slice(maxPrepared)) === unpreparedTail && JSON.stringify(persisted) === JSON.stringify(context.pool) &&
          (!continuationPlan || JSON.stringify(persisted.accounts.slice(0, continuationPlan.historicalPreparedCount)) === historicalPoolBefore);
        if (!report.completePoolPreserved) throw new Error('Complete pool identity, persisted values or unprepared tail changed');
      } catch (error) {
        report.completePoolPreserved = false; context.cleanupErrors.push({ step: 'completePoolPreservation', error: errorText(error) });
      }
    }
    try { guard.close(); } catch (error) { context.cleanupErrors.push({ step: 'monitorPollClose', error: errorText(error) }); }
    if (lock) {
      try { await lock.close(); await fs.unlink(path.join(path.dirname(accountFile), 'capacity-run.lock')); }
      catch (error) { context.cleanupErrors.push({ step: 'runLockRelease', error: errorText(error) }); }
    }
    process.off('SIGINT', onSignal); process.off('SIGTERM', onSignal); options.signal?.removeEventListener('abort', onAbort);
    try { await writer.close(); } catch (error) { context.cleanupErrors.push({ step: 'evidenceClose', error: errorText(error) }); }
  }
  report.finishedAt = new Date().toISOString();
  report.safetyStopped = guard.stopped; report.safetyReason = guard.reason ?? null;
  report.ok = !report.error && !report.safetyStopped && report.preparedCount === (continuationPlan?.expectedThisRun ?? maxPrepared) && report.completePoolPreserved === true && !report.cleanupErrors.length;
  report.ordinaryPreparationEvidenceEligible = continuation === undefined && !testExecution && report.ok;
  if (continuation !== undefined) report.continuationPreparationEvidenceEligible = !testExecution && report.ok && Boolean(continuationPlan);
  await fs.writeFile(reportPath, JSON.stringify(sanitize(report, context.pool?.accounts.map(a => a.password) ?? []), null, 2) + '\n', { flag: 'wx' });
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  throw new Error('Import runPreparationBank from an owned wrapper; there is no implicit live CLI');
}
