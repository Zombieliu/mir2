// Ordinary preparation only. This module never starts the capacity engine,
// activates a cohort, seeds accounts, edits the account store, or measures CCU.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { CapacityClient, ordinaryLogin, prepareCapacityActor, drain, awaitPendingActions,
  ordinaryPreparationConfiguration, attackResultsMode } from './playtest-capacity-soak.mjs';
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
  login: ordinaryLogin, prepare: prepareCapacityActor, drain, awaitPending: awaitPendingActions,
  loadMap: loadProtocolCollisionMap, isWalkable: protocolMapCellIsWalkable, equip: equipStarterGear, now: Date.now,
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
  const context = { endpoint, policy, attackResults, writer, guard, accountFile, ledgerFile: path.join(path.dirname(accountFile), 'capacity-auth-budget.json'),
    phase: 'preparation', clients: [], allClients: [], cleanupErrors: report.cleanupErrors,
    add(sample) { writer.add({ type: 'preparationAction', phase: 'preparation', at: Date.now(), ...sanitize(sample, this.pool?.accounts.map(a => a.password) ?? []) }); } };
  const savedPoints = new Set();
  let lock, timer, current = null, poolIdentity, unpreparedTail, initialCount = 0;
  const deadline = services.now() + finiteWallSeconds * 1000;
  const stop = reason => guard.stop(reason), onSignal = () => stop('Operator requested normal preparation drain');
  const onAbort = () => stop('Preparation AbortSignal requested normal drain');
  const check = () => { if (services.now() >= deadline) stop('Finite preparation wall-clock budget exhausted'); guard.assert(); };
  const errorText = error => sanitize(String(error?.message ?? error), [...(context.pool?.accounts.map(a => a.password) ?? []), ...(current?.secrets ?? []), ...(current?.recentSecrets ?? [])]);
  const progress = value => { writer.add({ type: 'preparationProgress', at: Date.now(), ...value }); options.onProgress?.(value); };
  async function closeCurrent() {
    if (!current) return null;
    const client = current, errorsBefore = context.cleanupErrors.length;
    client.paused = true;
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

    for (let index = 0; index < maxPrepared; index++) {
      check();
      current = services.createClient(endpoint, context.pool.accounts[index], context);
      current.ready = false; context.clients.push(current); context.allClients.push(current);
      await services.login(current, context, 'admission'); check();
      assert(String(current.snapshot?.mapFileName) === '0', 'Ordinary character is not on preparation map0');
      await services.equip(current); check();
      const result = await services.prepare(current, context, { route: configuration.route, stagingPoint: configuration.stagingPoints[index],
        ...(candidates ? { resolveStagingPoint: input => selectPreparationStagingPoint({ ...input, candidates, savedPoints }) } : {}) });
      check();
      const cursor = current.sequence;
      await current.refresh(); check();
      const readiness = current.ordinarySupplies.assertReady({ afterSequence: cursor });
      assert(result.status === 'ready' && current.ready === false && readiness.ownerName === current.label && readiness.map === '0' &&
        readiness.hpStock >= preparation.preparationHpTarget && result.after?.ownerName === current.label && (!candidates || (readiness.inSafeZone === true && result.resolvedStagingPoint?.x === readiness.x && result.resolvedStagingPoint?.y === readiness.y)), 'Fresh ordinary preparation evidence is incomplete');
      const evidence = actorEvidence(current, result, readiness);
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
      savedPoints.add(pointKey); report.preparedIndex.push(assigned);
      report.actorResults.push(evidence); report.preparedCount++;
      writer.add({ type: 'ordinaryPreparedAndNormallySaved', at: Date.now(), ...evidence });
      progress({ stage: 'prepared', preparedCount: report.preparedCount, target: maxPrepared, name: evidence.name, characterIndex: evidence.characterIndex });
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
      report.failedActor = { ...actorEvidence(current, status, null), error: report.error };
    }
  } finally {
    clearTimeout(timer);
    context.phase = 'drain';
    if (current) {
      try { await closeCurrent(); } catch (error) { context.cleanupErrors.push({ step: 'normalActorDrain', error: errorText(error) }); }
    }
    if (context.pool) {
      try {
        const persisted = validateCapacityPool(await regularJson(accountFile), endpoint);
        const identity = persisted.accounts.map(a => ({ accountId: a.accountId, name: a.name }));
        report.completePoolPreserved = persisted.accounts.length === initialCount && JSON.stringify(identity) === JSON.stringify(poolIdentity) &&
          JSON.stringify(persisted.accounts.slice(maxPrepared)) === unpreparedTail && JSON.stringify(persisted) === JSON.stringify(context.pool);
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
  report.ok = !report.error && !report.safetyStopped && report.preparedCount === maxPrepared && report.completePoolPreserved === true && !report.cleanupErrors.length;
  report.ordinaryPreparationEvidenceEligible = !testExecution && report.ok;
  await fs.writeFile(reportPath, JSON.stringify(sanitize(report, context.pool?.accounts.map(a => a.password) ?? []), null, 2) + '\n', { flag: 'wx' });
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  throw new Error('Import runPreparationBank from an owned wrapper; there is no implicit live CLI');
}
