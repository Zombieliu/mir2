// Diagnostic only. Reuses frozen ordinary clients; no common steady workload.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';

const DRIVER_SHA = '47e0a5a4f35d46a63e1ead0e0a9e6eb8a2bff6a530538e8a2bc6614bc2f56296';
const CONTRACT_SHA = 'cb61d3ff0bc593f294aa434ece038458f87f94021825750091bb1e715f6e4a47';
const WS_SHA = '7b9865c26b0b6c148b4ed0d00dfc5409aff21e6a65851a2c5352b689f7351574';
const BINARY_SHA = 'ae8c83c6395e39e79e05dfb69989381669a0130ab855d9803946d84d00a9e9b4';
const REVISION = '31e9b6a74661c31d6515dd7322b466285f975d71';
const sleep = ms => new Promise(resolve => setTimeout(resolve, Math.max(1, ms)));
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function check(value, code) { if (!value) { const e = new Error(code); e.safeCode = code; throw e; } }
export function redact(value) {
  if (Array.isArray(value)) return value.map(redact);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) =>
    [k, /password|secret|token|authorization|cookie|credential|accountId/i.test(k) ? '[redacted]' : redact(v)]));
  return value;
}
const receipt = (c, e) => e ? { observerOrdinal: c.ordinal, connection: c.connection, receiveSeq: e.receiveSeq,
  atMs: e.atMs, receiveAtUtc: e.atUtc, packet: e.packet ?? e.type, payload: e.payload ? structuredClone(e.payload) : null } : null;

export function validateRuntimeBuild(runtime, observerBuild = null) {
  check(runtime.schema === 'mir2.comparable.modern-runtime.v1' && runtime.qualificationLayer === 'game-core-local' &&
    runtime.backend === 'file' && runtime.sessionCache === 'in-memory' && runtime.sourceRevision === REVISION, 'runtime_pin_mismatch');
  if (!observerBuild) {
    check(runtime.binarySha256 === BINARY_SHA && runtime.changesKind === undefined && runtime.sourceInputManifestSha256 === undefined,
      'original_binary_pin_or_explicit_manifest_required');
    return { binarySha256: BINARY_SHA, sourceProof: 'original-source-pinned-binary', sourceRevisionIsBaseOnly: false, additionalSource: null };
  }
  const manifest = observerBuild.manifest, isSha = v => typeof v === 'string' && /^[0-9a-f]{64}$/.test(v);
  const reviewedKinds = {
    'mir2.comparable.observer-timing-build.v1': { changesKind: 'observer-timing-only', proof: 'base-plus-observer-timing-overlay', observerTimingOnly: true },
    'mir2.comparable.spectator-scene-build.v1': { changesKind: 'spectator-scene-projection', proof: 'base-plus-spectator-scene-projection', observerTimingOnly: false },
  };
  const reviewed = Object.hasOwn(reviewedKinds, manifest?.schema) ? reviewedKinds[manifest.schema] : null;
  check(reviewed && manifest.baseRevision === REVISION &&
    manifest.changesKind === reviewed.changesKind && manifest.buildActualExitCode === 0 &&
    isSha(manifest.binarySha256) && isSha(manifest.sourceInputManifestSha256) && isSha(observerBuild.sha256),
  'reviewed_candidate_build_manifest_invalid');
  check(runtime.baseRevision === manifest.baseRevision && runtime.changesKind === manifest.changesKind &&
    runtime.buildActualExitCode === manifest.buildActualExitCode && runtime.binarySha256 === manifest.binarySha256 &&
    runtime.sourceInputManifestSha256 === manifest.sourceInputManifestSha256 && runtime.buildManifestSha256 === observerBuild.sha256,
  'reviewed_candidate_runtime_manifest_binding_mismatch');
  return { binarySha256: manifest.binarySha256, sourceProof: reviewed.proof, sourceRevisionIsBaseOnly: true,
    additionalSource: { buildManifestSchema: manifest.schema, observerTimingOnly: reviewed.observerTimingOnly,
      buildManifestSha256: observerBuild.sha256, sourceInputManifestSha256: manifest.sourceInputManifestSha256,
      baseRevision: manifest.baseRevision, changesKind: manifest.changesKind, buildActualExitCode: manifest.buildActualExitCode,
      binarySha256: manifest.binarySha256, cleanBaseBuild: false, behaviorFixClaimed: false, capacityClaimAllowed: false } };
}

export async function runIdleLogoutProbe({ driver, contract, plan, cohort, runtime, ledger, WebSocket, emit, saveLedger,
  now = () => performance.now(), wallNow = Date.now, wait = sleep, observerBuild = null }) {
  contract.validatePlan(plan);
  const url = driver.endpoint(runtime.endpoint);
  check(plan.population === 25 && plan.scenario === 'dense-movement-chat', 'diagnostic_requires_dense25_fixture');
  check(cohort.schema === 'mir2.comparable.modern-cohort.v1' && cohort.qualificationLayer === 'game-core-local' &&
    cohort.mapSha256 === plan.map.rawSha256 && cohort.actors?.length === 25 &&
    cohort.actors.every((a, i) => a.ordinal === i && Number.isInteger(a.characterIndex) && typeof a.name === 'string' &&
      typeof a.accountId === 'string' && !a.accountId.startsWith('sui:') && typeof a.password === 'string') &&
    new Set(cohort.actors.map(a => a.accountId)).size === 25 && new Set(cohort.actors.map(a => a.name)).size === 25,
  'isolated_ordinary_cohort_invalid');
  const buildProof = validateRuntimeBuild(runtime, observerBuild);
  check(ledger.schema === 'mir2.comparable.local-auth-ledger.v1' && ledger.purpose === 'comparable-local-only' &&
    ledger.endpoint === url && Array.isArray(ledger.attempts), 'inherited_local_auth_ledger_required');
  const stamp = () => ({ atMs: now(), atUtc: new Date(wallNow()).toISOString() });
  const report = { schema: 'mir2.comparable.idle-logout-diagnostic.v1', processId: process.pid,
    diagnosticOnly: true, commonSteadyActionsExecuted: false, capacityQualified: false, capacityClaimAllowed: false,
    fullCombatQualified: false, physicalTriClientQualified: false, productionStackQualified: false,
    qualificationLayer: 'game-core-local', sourceRevision: REVISION, ...buildProof,
    populationRequested: 25, admitted: 0, admissions: [], idle: null, removeItem: null, selectedLogout: null,
    cleanup: [], phaseFailures: [], startedAtUtc: new Date(wallNow()).toISOString(), runCompleted: false, ok: false };
  const clients = [], admitted = new Set(), logoutResults = new Map(); let failed = false;
  const record = row => {
    if (row.event === 'sent') {
      const target = row.command?.type === 'removeItem' ? report.removeItem : row.command?.type === 'logOut' ? logoutResults.get(row.actorOrdinal) : null;
      if (target) { target.sentAtMs = row.atMs; target.sentAtUtc = row.atUtc; }
    }
    emit(redact(row));
  };
  const phase = (name, detail = {}) => record({ event: 'diagnosticPhase', phase: name, ...stamp(), ...detail });
  const noteFailure = (name, ordinal, reason) => { failed = true; const row = { phase: name, actorOrdinal: ordinal, reason, ...stamp() };
    report.phaseFailures.push(row); record({ event: 'diagnosticFailure', ...row, passed: false }); };
  const reserve = async ordinal => {
    for (;;) {
      const wall = wallNow(), delay = contract.localLoginDelay(ledger.attempts, wall);
      if (!delay) {
        ledger.attempts.push({ atMs: wall, purpose: 'admission' }); await saveLedger(ledger);
        record({ event: 'authReserved', actorOrdinal: ordinal, purpose: 'admission', attemptNumber: ledger.attempts.length, ...stamp() }); return;
      }
      record({ event: 'authWindowWait', actorOrdinal: ordinal, purpose: 'admission', admitted: report.admitted, waitMs: delay, ...stamp() });
      await wait(Math.min(1000, delay));
    }
  };
  const observeLate = async (c, floor, matcher, label, maximumMs) => {
    const began = now(), end = began + maximumMs;
    phase('lateObservationStarted', { actorOrdinal: c.ordinal, label, originalTimeoutKept: true, maximumMs, observationDeadlineAtMs: end });
    let ack;
    for (;;) {
      ack = c.events.find(e => e.receiveSeq > floor && matcher(e));
      if (ack || c.closed || now() >= end) break;
      await wait(Math.min(10, end - now()));
    }
    // Only the original client's exact pending matcher can retire its fence.
    c.reconcileLate();
    phase('lateObservationEnded', { actorOrdinal: c.ordinal, label, originalTimeoutKept: true,
      elapsedMs: now() - began, acknowledgement: receipt(c, ack), fenceRetired: !c.pending && !c.gate.pending });
    return ack;
  };
  const logout = async (c, observeLateMs = 0) => {
    if (logoutResults.has(c.ordinal)) return logoutResults.get(c.ordinal);
    const row = { actorOrdinal: c.ordinal, connection: c.connection, attempted: false, passed: false,
      acknowledgementWithinBudget: false, actualLogoutSuccessObserved: false, timedOut: false, originalBudgetMs: 5000 };
    logoutResults.set(c.ordinal, row);
    if (!admitted.has(c.ordinal)) { row.status = 'notAdmitted'; return row; }
    if (c.pending || c.gate.pending) {
      row.status = 'unresolvedPreviousAction'; noteFailure('normalLogout', c.ordinal, row.status); return row;
    }
    if (!c.inGame) {
      row.status = 'alreadyOutsideWorldWithoutThisLogoutReceipt'; noteFailure('normalLogout', c.ordinal, row.status); return row;
    }
    const began = now(), floor = c.sequence, deadline = began + 5000;
    row.attempted = true; row.requestBegan = stamp(); row.deadlineAtMs = deadline; row.receiveSequenceFloor = floor;
    phase('originalNormalLogoutStarted', { actorOrdinal: c.ordinal, connection: c.connection, deadlineAtMs: deadline, receiveSequenceFloor: floor });
    const matcher = e => ['LogOutSuccess', 'LogOutFailed'].includes(e.packet);
    let ack;
    try { ack = await c.request({ type: 'logOut' }, matcher, deadline, 'originalNormalLogout', { ignoreFailure: true }); }
    catch { row.timedOut = now() >= deadline;
      noteFailure('originalNormalLogout', c.ordinal, row.timedOut ? 'original_5000ms_deadline_exceeded' : 'ordinary_logout_request_failed');
      if (observeLateMs > 0) ack = await observeLate(c, floor, matcher, 'originalNormalLogout', observeLateMs);
      else ack = c.events.find(e => e.receiveSeq > floor && matcher(e));
    }
    if (ack) {
      row.acknowledgement = receipt(c, ack); row.ackElapsedMs = ack.atMs - (row.sentAtMs ?? began); row.requestElapsedMs = ack.atMs - began;
      row.actualLogoutSuccessObserved = ack.packet === 'LogOutSuccess';
      row.acknowledgementWithinBudget = ack.atMs <= deadline && !row.timedOut;
      row.passed = row.actualLogoutSuccessObserved && row.acknowledgementWithinBudget;
      row.status = row.actualLogoutSuccessObserved ? row.passed ? 'loggedOutWithinBudget' : 'loggedOutLate' : 'explicitLogoutFailed';
      if (row.status === 'explicitLogoutFailed') noteFailure('originalNormalLogout', c.ordinal, row.status);
    } else row.status = 'logoutAcknowledgementMissing';
    row.originalTimeoutKept = row.timedOut || Boolean(ack && ack.atMs > deadline);
    phase('originalNormalLogoutEnded', { ...row });
    if (!row.passed) failed = true;
    return row;
  };
  try {
    phase('admissionStarted');
    for (const actor of cohort.actors) {
      await reserve(actor.ordinal);
      const c = new driver.ModernClient({ url, actor, connection: actor.ordinal + 1, WebSocket, emit: record, now }); clients.push(c);
      const proof = await c.login();
      driver.verifyInitialFixture(c.snapshot, plan.actors[actor.ordinal].start);
      const initial = c.ownedSnapshotEvidence('diagnosticInitialOwnedState');
      admitted.add(actor.ordinal); report.admitted++;
      report.admissions.push({ actorOrdinal: actor.ordinal, connection: c.connection, bootstrap: proof, sourceReceipt: initial.sourceReceipt, fingerprint: initial.fingerprint });
    }
    const began = now(), idleEnd = began + 60000;
    report.idle = { began: stamp(), requiredMs: 60000, plannedEndAtMs: idleEnd, completed: false };
    phase('idleStarted', { admitted: report.admitted, requiredMs: 60000, plannedEndAtMs: idleEnd });
    while (now() < idleEnd) {
      check(clients.every(c => !c.failure && !c.closed && c.inGame), 'admitted_client_unavailable_during_idle');
      await wait(Math.min(100, idleEnd - now()));
    }
    report.idle.ended = stamp(); report.idle.elapsedMs = now() - began; report.idle.completed = true;
    phase('idleEnded', { elapsedMs: report.idle.elapsedMs });
    const c = clients[0], snapshot = c.snapshot, weapon = snapshot.equipmentItems.find(i => i.slot === 'weapon' && i.key === 'crystal-item-221');
    check(weapon && Number.isSafeInteger(Number(weapon.uniqueId)) && Number(weapon.uniqueId) > 0 &&
      !snapshot.inventoryItems.some(i => Number(i.slot) === 9), 'fresh_weapon221_or_empty_bag9_missing');
    const source = c.ownedSnapshotEvidence('diagnosticBeforeRemove'), requestBeganAtMs = now(), floor = c.sequence, deadline = requestBeganAtMs + 15000;
    report.removeItem = { actorOrdinal: 0, connection: c.connection, uniqueId: weapon.uniqueId, grid: 'inventory', to: 9,
      requestBegan: stamp(), receiveSequenceFloor: floor, deadlineAtMs: deadline, budgetMs: 15000,
      sourceReceipt: source.sourceReceipt, originalFingerprint: source.fingerprint, passed: false, timedOut: false };
    phase('removeWeaponStarted', { ...report.removeItem });
    const matcher = e => e.packet === 'RemoveItem' && Number(e.payload?.uniqueId) === Number(weapon.uniqueId) &&
      e.payload?.to === 9 && String(e.payload?.grid).toLowerCase() === 'inventory';
    let ack;
    try { ack = await c.request({ type: 'removeItem', uniqueId: weapon.uniqueId, grid: 'inventory', to: 9 }, matcher, deadline, 'diagnosticRemoveItem'); }
    catch { report.removeItem.timedOut = now() >= deadline; noteFailure('removeWeapon', 0, report.removeItem.timedOut ? 'original_15000ms_deadline_exceeded' : 'ordinary_remove_request_failed');
      await observeLate(c, floor, matcher, 'diagnosticRemoveItem', 15000); throw Object.assign(new Error(), { safeCode: 'remove_request_failed' }); }
    report.removeItem.acknowledgement = receipt(c, ack); report.removeItem.ackElapsedMs = ack.atMs - (report.removeItem.sentAtMs ?? requestBeganAtMs);
    report.removeItem.passed = ack.payload.success === true && ack.atMs <= deadline;
    phase('removeWeaponAcknowledged', { ...report.removeItem }); check(report.removeItem.passed, 'ordinary_remove_rejected');
    report.selectedLogout = await logout(c, 15000);
  } catch (error) { failed = true; report.failure = error.safeCode ?? 'diagnostic_phase_failed'; phase('probePhaseFailed', { reason: report.failure }); }
  finally {
    phase('cleanupStarted');
    for (const c of clients) {
      const row = { actorOrdinal: c.ordinal, connection: c.connection, admitted: admitted.has(c.ordinal), normalLogout: false, transportClose: false };
      try { row.logout = await logout(c); row.normalLogout = row.logout.passed; }
      catch { noteFailure('cleanupLogout', c.ordinal, 'cleanup_logout_failed'); }
      try { row.close = await c.close(); row.transportClose = row.close.passed; }
      catch { noteFailure('independentClose', c.ordinal, 'original_5000ms_close_failed'); }
      clearInterval(c.timer); if (!row.normalLogout || !row.transportClose) failed = true; report.cleanup.push(row);
    }
    phase('cleanupEnded'); report.runCompleted = true; report.completedAtUtc = new Date(wallNow()).toISOString();
    report.ok = !failed && report.admitted === 25 && report.idle?.completed === true && report.removeItem?.passed === true &&
      report.selectedLogout?.passed === true && report.cleanup.length === 25 && report.cleanup.every(r => r.normalLogout && r.transportClose);
  }
  return redact(report);
}

async function main() {
  const required = ['--driver-module', '--ws-module', '--plan', '--cohort', '--runtime-receipt', '--ledger', '--output'];
  const allowed = [...required, '--build-manifest'], options = {};
  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    const equal = args[i].indexOf('='), key = equal >= 0 ? args[i].slice(0, equal) : args[i], value = equal >= 0 ? args[i].slice(equal + 1) : args[++i];
    check(allowed.includes(key) && !options[key] && value, 'unknown_or_duplicate_option'); options[key] = value;
  }
  for (const key of required) check(options[key] && path.isAbsolute(options[key]) && /^c:[\\/]/i.test(options[key]), 'all_paths_must_be_explicit_absolute_C');
  if (options['--build-manifest']) check(path.isAbsolute(options['--build-manifest']) && /^c:[\\/]/i.test(options['--build-manifest']), 'build_manifest_must_be_absolute_C');
  const driverPath = options['--driver-module'], contractPath = path.join(path.dirname(driverPath), 'contract.mjs');
  const driverBytes = await fs.readFile(driverPath), contractBytes = await fs.readFile(contractPath);
  check(sha(driverBytes) === DRIVER_SHA && sha(contractBytes) === CONTRACT_SHA, 'frozen_v3_driver_pin_mismatch');
  check(sha(await fs.readFile(options['--ws-module'])) === WS_SHA, 'frozen_ws_module_pin_mismatch');
  const driver = await import(pathToFileURL(driverPath).href), contract = await import(pathToFileURL(contractPath).href);
  const readJson = async key => { try { return JSON.parse(await fs.readFile(options[key], 'utf8')); } catch { throw Object.assign(new Error(), { safeCode: 'input_json_unavailable_or_invalid' }); } };
  const planBytes = await fs.readFile(options['--plan']), plan = JSON.parse(planBytes), planSha256 = sha(planBytes);
  const cohort = await readJson('--cohort'), runtime = await readJson('--runtime-receipt'), ledger = await readJson('--ledger');
  let observerBuild = null;
  if (options['--build-manifest']) {
    const bytes = await fs.readFile(options['--build-manifest']);
    observerBuild = { manifest: JSON.parse(bytes), sha256: sha(bytes) };
  }
  validateRuntimeBuild(runtime, observerBuild);
  check(cohort.planSha256 === planSha256 && runtime.planSha256 === planSha256 && ledger.planSha256 === planSha256, 'fixture_plan_pin_mismatch');
  check(path.resolve(cohort.accountStore) === path.resolve(runtime.accountStore) &&
    path.dirname(options['--ledger']) === path.dirname(options['--cohort']), 'isolated_store_or_ledger_path_mismatch');
  const output = options['--output'];
  for (const suffix of ['.events.jsonl', '.report.json']) { try { await fs.stat(output + suffix); throw Object.assign(new Error(), { safeCode: 'output_already_exists' }); }
    catch (e) { check(e.code === 'ENOENT', e.safeCode ?? 'output_path_unavailable'); } }
  const evidence = await fs.open(output + '.events.jsonl', 'wx'); let queue = Promise.resolve(), writeFailure;
  const emit = row => { check(!writeFailure, 'evidence_write_failed'); const line = JSON.stringify(redact(row)) + '\n';
    queue = queue.then(() => evidence.write(line)).catch(e => { writeFailure = e; }); };
  const saveLedger = async value => { const next = options['--ledger'] + '.next';
    await fs.writeFile(next, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); await fs.rename(next, options['--ledger']); };
  const WebSocket = createRequire(import.meta.url)(options['--ws-module']); let report;
  try { report = await runIdleLogoutProbe({ driver, contract, plan, cohort, runtime, ledger, WebSocket, emit, saveLedger, observerBuild }); }
  finally { await queue; await evidence.close(); }
  check(!writeFailure, 'evidence_write_failed'); report.planSha256 = planSha256; report.eventsSha256 = sha(await fs.readFile(output + '.events.jsonl'));
  report.sourceHashes = { probe: sha(await fs.readFile(new URL(import.meta.url))), driver: DRIVER_SHA, contract: CONTRACT_SHA,
    wsModule: sha(await fs.readFile(options['--ws-module'])) };
  await fs.writeFile(output + '.report.json', JSON.stringify(redact(report), null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ ok: report.ok, diagnosticOnly: true, runCompleted: report.runCompleted, admitted: report.admitted,
    capacityClaimAllowed: false, report: output + '.report.json' })); process.exitCode = report.ok ? 0 : 1;
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main().catch(error => {
  console.error(JSON.stringify({ ok: false, diagnosticOnly: true, errorCode: error.safeCode ?? 'diagnostic_setup_failed' })); process.exitCode = 1;
});
