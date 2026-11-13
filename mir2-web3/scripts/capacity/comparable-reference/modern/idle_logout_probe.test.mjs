import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import { EventEmitter } from 'node:events';
import * as driver from './driver.mjs';
import * as contract from './contract.mjs';
import { runIdleLogoutProbe, redact, validateRuntimeBuild } from './idle_logout_probe.mjs';

const ruleBytes = await fs.readFile(new URL('../workload.v2.json', import.meta.url)), rules = JSON.parse(ruleBytes);
function plan25() {
  const p = { schema: 'mir2.comparable.plan.v1', qualificationLayer: 'game-core-local', scenario: 'dense-movement-chat', purpose: 'capacity',
    population: 25, durationMs: 300000, planId: '0123456789abcdefabcd', definitionSha256: contract.hash(ruleBytes), generatorSourceSha256: 'a'.repeat(64),
    map: rules.map, actorTemplate: rules.actorTemplate, resourceContract: rules.resourceContract, thresholds: rules.thresholds, causality: rules.causality,
    scheduling: { version: 2, chatPhaseRule: 'same-primary-phase-with-midpoint-offset', primaryIntervalMs: 2000, minimumSameActorOfferGapMs: 1000, chatOffsetWithinPrimaryIntervalMs: 1000 },
    assessment: { windowMs: 60000, perActor: true, allCompleteWindowsRequired: true, failedActionCanBecomeCompleted: false,
      notSentIsTimeout: false, notSentIsCompleted: false, safetyOrEvidenceFailureIsHardFailure: true },
    equipmentActorOrdinals: [0, 1, 2], equipmentRoundsPerActor: 2, actors: [], actions: [] };
  for (let ordinal = 0; ordinal < 25; ordinal++) {
    const x = 10 + (ordinal % 5) * 2, y = 10 + Math.floor(ordinal / 5), actor = { ordinal, group: 0, peerOrdinals: [(ordinal + 1) % 25],
      start: { x, y }, lane: [{ x, y }, { x: x + 1, y }] }; p.actors.push(actor);
    const actions = [], phase = Math.floor(ordinal * 2000 / 25);
    for (let dueMs = phase, step = 0; dueMs < 300000; dueMs += 2000, step++) actions.push({ actorOrdinal: ordinal, kind: 'walk', dueMs,
      deadlineMs: dueMs + 2000, direction: step % 2 ? 'Left' : 'Right', target: actor.lane[step % 2 ? 0 : 1] });
    for (let dueMs = 15000 + phase; dueMs < 300000; dueMs += 30000) actions.push({ actorOrdinal: ordinal, kind: 'chat', dueMs, deadlineMs: dueMs + 2500 });
    actions.sort((a, b) => a.dueMs - b.dueMs || a.kind.localeCompare(b.kind));
    actions.forEach((a, i) => { a.ticketSequence = i + 1; if (a.kind === 'chat') { a.nonce = `m2cmp:${p.planId}:${ordinal}:${i + 1}`; a.nonceSha256 = contract.hash(a.nonce); } });
    p.actions.push(...actions);
  }
  p.actions.sort((a, b) => a.dueMs - b.dueMs || a.actorOrdinal - b.actorOrdinal); return p;
}
function owned(ordinal, p) {
  const gear = [['weapon', 221, 4000], ['armour', 317, 5000], ['braceletLeft', 482, 7000], ['braceletRight', 482, 7000]];
  return { playerObjectId: 1000, playerHp: 674, playerMp: 151, mapFileName: 'DogYoHyun', gold: 0, inventoryCapacity: 46, maxBagSlots: 40,
    entities: [{ objectId: 1000, kind: 'player', name: `Diag${ordinal}`, level: 40, class: 'Warrior', gender: 'Male', dead: false, ...p.actors[ordinal].start }],
    inventoryItems: Array.from({ length: 9 }, (_, slot) => ({ slot, uniqueId: 20000 + ordinal * 100 + slot, key: 'crystal-item-662', quantity: 20 })), beltItems: [],
    equipmentItems: gear.map(([slot, index, dura], i) => ({ slot, key: `crystal-item-${index}`, quantity: 1, uniqueId: 10000 + ordinal * 100 + i,
      durabilityCurrent: dura, durabilityMax: dura, tooltipSource: { userItem: { item_index: index } } })) };
}
// Virtual time only verifies orchestration. No socket or performance is tested.
async function scenario(logoutDelay, observerBuild = null) {
  const p = plan25(), rows = [], sends = [], persisted = [], sockets = [], jobs = []; let clock = 0;
  const actors = p.actors.map(a => ({ ordinal: a.ordinal, name: `Diag${a.ordinal}`, accountId: `private-account-${a.ordinal}`,
    password: `never-export-this-password-${a.ordinal}`, characterIndex: 0 }));
  const enqueue = (ms, fn) => { jobs.push({ at: clock + ms, fn }); jobs.sort((a, b) => a.at - b.at); };
  const advance = async ms => { const end = clock + ms; while (jobs[0]?.at <= end) { const job = jobs.shift(); clock = job.at; job.fn(); }
    clock = end; await Promise.resolve(); };
  const packet = (s, name, payload) => s.emit('message', Buffer.from(JSON.stringify({ type: 'packet', packet: name, payload })), false);
  class Socket extends EventEmitter {
    constructor(url, options) { super(); this.readyState = 0; this._closeFrameReceived = false; sockets.push(this);
      assert.equal(options.perMessageDeflate, false); assert.equal(persisted.length, sockets.length);
      queueMicrotask(() => { this.readyState = 1; }); }
    send(raw) {
      const c = JSON.parse(raw); sends.push({ actorOrdinal: this.ordinal, type: c.type, atMs: clock });
      if (c.type === 'clientVersion') packet(this, 'ClientVersion', { result: 1 });
      if (c.type === 'login') { this.ordinal = actors.findIndex(a => a.accountId === c.accountId); packet(this, 'LoginSuccess', { characters: [{ index: 0, level: 40, name: actors[this.ordinal].name }] }); }
      if (c.type === 'startGame') { const s = owned(this.ordinal, p); packet(this, 'StartGame', { result: 4 });
        packet(this, 'UserInformation', { objectId: 1000, hp: 674, name: actors[this.ordinal].name, level: 40, location: p.actors[this.ordinal].start });
        packet(this, 'MapInformation', { fileName: 'DogYoHyun' }); this.emit('message', Buffer.from(JSON.stringify({ type: 'worldSnapshot', payload: s })), false); }
      if (c.type === 'removeItem') { assert.equal(c.to, 9); assert.equal(c.uniqueId, 10000); enqueue(184, () => packet(this, 'RemoveItem', { uniqueId: 10000, grid: 'Inventory', to: 9, success: true })); }
      if (c.type === 'logOut') enqueue(this.ordinal === 0 ? logoutDelay : 20, () => { packet(this, 'LogOutSuccess', {});
        this.emit('message', Buffer.from(JSON.stringify({ type: 'worldSnapshot', payload: { ...owned(this.ordinal, p), playerObjectId: null, playerHp: null, entities: [] } })), false); });
    }
    close() { this.readyState = 3; this._closeFrameReceived = true; this.emit('close', 1000); }
    terminate() { this.readyState = 3; this.emit('close', 1006); }
  }
  class VirtualClient extends driver.ModernClient {
    async wait(predicate, deadline, label, { ignoreFailure = false } = {}) {
      for (;;) { const result = predicate(); if (result) { if (Number.isFinite(result.atMs) && result.atMs > deadline) throw new Error('late'); return result; }
        if ((!ignoreFailure && this.failure) || this.closed || clock >= deadline) throw new Error('unavailable or deadline'); await advance(Math.min(10, deadline - clock)); }
    }
  }
  const history = Array.from({ length: 50 }, (_, i) => ({ atMs: 100000, purpose: `prior-${i}` }));
  const ledger = { schema: 'mir2.comparable.local-auth-ledger.v1', purpose: 'comparable-local-only', endpoint: 'ws://127.0.0.1:19010/ws', attempts: structuredClone(history) };
  const runtime = { schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory', endpoint: ledger.endpoint,
    sourceRevision: '31e9b6a74661c31d6515dd7322b466285f975d71', binarySha256: 'ae8c83c6395e39e79e05dfb69989381669a0130ab855d9803946d84d00a9e9b4' };
  if (observerBuild) Object.assign(runtime, observerBuild.manifest, { schema: 'mir2.comparable.modern-runtime.v1', buildManifestSha256: observerBuild.sha256 });
  const report = await runIdleLogoutProbe({ driver: { ...driver, ModernClient: VirtualClient }, contract, plan: p,
    cohort: { schema: 'mir2.comparable.modern-cohort.v1', qualificationLayer: 'game-core-local', mapSha256: contract.MAP_SHA, actors },
    runtime, observerBuild,
    ledger, WebSocket: Socket, emit: r => rows.push(JSON.parse(JSON.stringify(r))), saveLedger: async l => persisted.push(structuredClone(l)),
    now: () => clock, wallNow: () => 1000000 + clock, wait: advance });
  return { report, rows, sends, ledger, history, persisted, sockets };
}
test('diagnostic25 preserves50 history, sends no steady actions, keeps late5s FAIL and closes without duplicate Logout or secret output', async () => {
  const f = await scenario(6574);
  assert.equal(f.report.ok, false); assert.equal(f.report.runCompleted, true); assert.equal(f.report.admitted, 25); assert.equal(f.report.idle.elapsedMs, 60000);
  assert.equal(f.report.selectedLogout.status, 'loggedOutLate'); assert.equal(f.report.selectedLogout.ackElapsedMs, 6574);
  assert.equal(f.report.selectedLogout.passed, false); assert.equal(f.report.selectedLogout.originalTimeoutKept, true);
  assert.equal(f.report.cleanup[0].logout.status, 'loggedOutLate'); assert.equal(f.report.cleanup.filter(r => r.normalLogout).length, 24);
  assert.ok(f.report.cleanup.every(r => r.transportClose)); assert.equal(f.sends.filter(s => s.type === 'logOut' && s.actorOrdinal === 0).length, 1);
  assert.equal(f.sends.filter(s => s.type === 'login').length, 25); assert.equal(f.sends.filter(s => s.type === 'removeItem').length, 1);
  assert.equal(f.sends.filter(s => ['walk', 'chat', 'equipItem', 'attackDirection'].includes(s.type)).length, 0);
  assert.equal(f.persisted.length, 25); assert.deepEqual(f.ledger.attempts.slice(0, 50), f.history); assert.equal(f.ledger.attempts.length, 75);
  assert.ok(f.rows.some(r => r.event === 'authWindowWait')); assert.equal(f.rows.filter(r => r.event === 'authReserved').length, 25);
  assert.ok(f.rows.some(r => r.event === 'lateLifecycleClosure' && r.passed === false));
  const text = JSON.stringify({ rows: f.rows, report: f.report }); assert.ok(!text.includes('never-export-this-password')); assert.ok(!text.includes('private-account-'));
  assert.equal(f.report.capacityClaimAllowed, false); assert.equal(f.report.commonSteadyActionsExecuted, false);
  assert.deepEqual(redact({ nested: [{ password: 'p', authToken: 't', accountId: 'a', safe: 1 }] }), { nested: [{ password: '[redacted]', authToken: '[redacted]', accountId: '[redacted]', safe: 1 }] });
  const g = await scenario(20); assert.equal(g.report.ok, true); assert.ok(g.report.cleanup.every(r => r.normalLogout && r.transportClose));
  assert.equal(g.report.capacityQualified, false); assert.equal(g.report.diagnosticOnly, true);
});
test('observer-only manifest binds exact runtime/hash; wrong pins and implicit candidate opt-in fail closed', async () => {
  const manifest = { schema: 'mir2.comparable.observer-timing-build.v1', baseRevision: '31e9b6a74661c31d6515dd7322b466285f975d71',
    changesKind: 'observer-timing-only', buildActualExitCode: 0, binarySha256: 'b'.repeat(64), sourceInputManifestSha256: 'c'.repeat(64) };
  const observerBuild = { manifest, sha256: contract.hash(Buffer.from(JSON.stringify(manifest))) };
  const runtime = { ...manifest, schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory',
    sourceRevision: manifest.baseRevision, buildManifestSha256: observerBuild.sha256 };
  assert.equal(validateRuntimeBuild(runtime, observerBuild).sourceRevisionIsBaseOnly, true);
  assert.throws(() => validateRuntimeBuild(runtime), /explicit_manifest_required/);
  for (const mutate of [m => { m.schema = 'other'; }, m => { m.baseRevision = 'd'.repeat(40); }, m => { m.changesKind = 'behavior-fix'; },
    m => { m.buildActualExitCode = 1; }, m => { m.binarySha256 = 'short'; }, m => { m.sourceInputManifestSha256 = 'bad'; }]) {
    const wrong = structuredClone(observerBuild); mutate(wrong.manifest); assert.throws(() => validateRuntimeBuild(runtime, wrong));
  }
  for (const key of ['baseRevision', 'changesKind', 'buildActualExitCode', 'binarySha256', 'sourceInputManifestSha256', 'buildManifestSha256']) {
    const wrong = { ...runtime, [key]: key === 'buildActualExitCode' ? 1 : 'wrong' };
    assert.throws(() => validateRuntimeBuild(wrong, observerBuild));
  }
  assert.throws(() => validateRuntimeBuild(runtime, { ...observerBuild, sha256: 'bad' }));
  const actual = await scenario(20, observerBuild);
  assert.equal(actual.report.ok, true); assert.equal(actual.report.sourceProof, 'base-plus-observer-timing-overlay');
  assert.equal(actual.report.binarySha256, manifest.binarySha256); assert.equal(actual.report.additionalSource.cleanBaseBuild, false);
  assert.equal(actual.report.additionalSource.behaviorFixClaimed, false); assert.equal(actual.report.additionalSource.buildManifestSha256, observerBuild.sha256);
  assert.equal(actual.report.capacityClaimAllowed, false); assert.equal(actual.report.idle.requiredMs, 60000);
});
test('spectator-scene build is a distinct reviewed branch; schema/kind cross-labelling and arbitrary builds are rejected', async () => {
  const manifest = { schema: 'mir2.comparable.spectator-scene-build.v1', baseRevision: '31e9b6a74661c31d6515dd7322b466285f975d71',
    changesKind: 'spectator-scene-projection', buildActualExitCode: 0, binarySha256: 'e'.repeat(64), sourceInputManifestSha256: 'f'.repeat(64) };
  const build = { manifest, sha256: contract.hash(Buffer.from(JSON.stringify(manifest))) };
  const runtime = { ...manifest, schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory',
    sourceRevision: manifest.baseRevision, buildManifestSha256: build.sha256 };
  const proof = validateRuntimeBuild(runtime, build);
  assert.equal(proof.sourceProof, 'base-plus-spectator-scene-projection');
  assert.equal(proof.additionalSource.observerTimingOnly, false); assert.equal(proof.additionalSource.buildManifestSchema, manifest.schema);
  assert.equal(proof.additionalSource.cleanBaseBuild, false); assert.equal(proof.additionalSource.behaviorFixClaimed, false);
  for (const change of [{ changesKind: 'observer-timing-only' }, { schema: 'mir2.comparable.observer-timing-build.v1' },
    { schema: 'arbitrary-build' }, { changesKind: 'another-fix' }, { buildActualExitCode: 1 }, { baseRevision: 'a'.repeat(40) }]) {
    assert.throws(() => validateRuntimeBuild(runtime, { ...build, manifest: { ...manifest, ...change } }));
  }
  assert.throws(() => validateRuntimeBuild({ ...runtime, binarySha256: 'a'.repeat(64) }, build));
  assert.throws(() => validateRuntimeBuild({ ...runtime, sourceInputManifestSha256: 'a'.repeat(64) }, build));
  assert.throws(() => validateRuntimeBuild({ ...runtime, buildManifestSha256: 'a'.repeat(64) }, build));
  assert.throws(() => validateRuntimeBuild({ ...runtime, changesKind: undefined },
    { ...build, manifest: { ...manifest, schema: 'toString', changesKind: undefined } }));
  assert.throws(() => validateRuntimeBuild(runtime));
  const actual = await scenario(20, build);
  assert.equal(actual.report.ok, true); assert.equal(actual.report.sourceProof, 'base-plus-spectator-scene-projection');
  assert.equal(actual.report.additionalSource.observerTimingOnly, false); assert.equal(actual.report.capacityClaimAllowed, false);
  assert.equal(actual.report.idle.requiredMs, 60000); assert.equal(actual.report.removeItem.budgetMs, 15000);
  assert.equal(actual.report.selectedLogout.originalBudgetMs, 5000); assert.equal(actual.report.cleanup.length, 25);
});
