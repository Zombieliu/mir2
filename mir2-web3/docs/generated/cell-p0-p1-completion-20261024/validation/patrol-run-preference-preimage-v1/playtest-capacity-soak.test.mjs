import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { capacityPolicy, validateCapacityPool, LoginBudget, LOGIN_WINDOW_MS, Histogram, Measurements,
  assessMeasurements, movementDiagnosticMayContinue, ContinuousAcceptance, NativePlan, nativeCadence, nativeActivityVerdict, AoiCoverage, verifiedCombatHit, BoundedEvidence, memoryTrend, TimeBucketSamples, sanitize } from './playtest-capacity-soak-core.mjs';
import { CapacityClient, FixedPlan, chooseHomes, validateNativeExplicitHomes, validateNativeCombatClusters, insideCombatCoverage, activeCombatCoverage, beginPendingAttackEvidence, recordPendingAttackPacket, finishPendingAttackEvidence, PENDING_ATTACK_PACKET_LIMIT, attackResultsMode, matchAttackResultV2, ATTACK_OUTCOME_CAPABILITY, loadStationaryNpcObstacles, spacingFourScenario, selectNativeAction, offerNativeAction, combatCandidates, recordCombatPursuitMovement, recordCombatScoutMovement, restoreCombatPursuit, stableFanoutRange, matchesFanoutReceipt, actionFailureStatus, awaitPendingActions, actorsReachedScenario, runCapacity, parseArguments, action, navigationObstacles, navigationPlanIsClear, movementHintCells, claimNavigationHint, releaseNavigationHint, NAVIGATION_HINT_MAX_CELLS } from './playtest-capacity-soak.mjs';
import { observedPlayerId } from './playtest-multiplayer-smoke.mjs';
import { loadProtocolCollisionMap, protocolMapCellIsWalkable, findProtocolWalkPath } from './quest-agent/protocol-navigation.mjs';

const WebSocketServer = createRequire(import.meta.url)('next/dist/compiled/ws').Server;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const actor = index => ({ accountId: `ltabcdef${index.toString(36).padStart(2, '0')}`, name: `Labcdef${index.toString(36).padStart(2, '0')}`,
  password: 'private-pass-123456', state: 'registered', provisioning: 'test-ordinary-registration' });
const pool = (endpoint, count = 1) => ({ schemaVersion: 1, purpose: 'isolated-load-actors', realm: 'isolated', endpoint,
  accounts: Array.from({ length: count }, (_, index) => actor(index)) });
const healthy = () => ({ sampledAt: Date.now(), stop: false, health: { currentActiveSessions: 0, maxActiveSessions: 100 }, memory: { currentBytes: 1000000 } });
async function withDirectory(action) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'playtest-capacity-test-'));
  try { return await action(directory); }
  finally {
    const resolved = path.resolve(directory);
    assert.equal(path.dirname(resolved), path.resolve(os.tmpdir()));
    assert.match(path.basename(resolved), /^playtest-capacity-test-/);
    await fs.rm(resolved, { recursive: true, force: true });
  }
}

test('capacity policy has finite ceilings and never quietly relaxes fixed gates', () => {
  const p = capacityPolicy({ stages: '25,50,75,100', combatActors: 10 });
  assert.equal(p.commandsPerSecond, 3); assert.equal(p.movementP95Ms, 750); assert.equal(p.chatP95Ms, 1000);
  assert.equal(p.soakSeconds, 3600); assert.equal(p.timeoutExclusive, .01); assert.equal(Object.isFrozen(p), true);
  for (const options of [{ stages: '101' }, { stages: '50,25' }, { wallSeconds: Infinity }, { soakSeconds: 300 }, { resumeSamples: 0 },
    { stages: '2', resumeSamples: 3 }, { stages: '2', saveSamples: 3 }]) {
    assert.throws(() => capacityPolicy(options));
  }
  assert.equal(capacityPolicy({ profile: 'probe' }).fullAcceptanceEligible, false);
  assert.throws(() => parseArguments(['--password', 'secret']), /Unknown/);
  assert.throws(() => parseArguments(['--stages', '10', '--stages', '100']), /repeated/);
});

test('ten ordinary native probe actors retain every fixed gate and cannot qualify for formal acceptance', () => {
  const options = { profile: 'probe', activityMode: 'native', stages: '10', combatActors: 1,
    stageSeconds: 30, soakSeconds: 360, resumeSamples: 0, saveSamples: 0 };
  const ten = capacityPolicy(options), five = capacityPolicy({ ...options, stages: '5' });
  assert.deepEqual(ten.stages, [10]);
  assert.equal(ten.fullAcceptanceEligible, false);
  assert.equal(capacityPolicy({ ...options, stageSeconds: 300, soakSeconds: 7200 }).fullAcceptanceEligible, false,
    'even a long probe remains ineligible for formal capacity acceptance');
  for (const field of Object.keys(five).filter(field => field !== 'stages')) assert.deepEqual(ten[field], five[field], field);
  assert.equal(ten.minimumCombatAttacksPerMinute, 18);
  assert.equal(ten.minimumCombatHitsPerMinute, 12);
  assert.throws(() => capacityPolicy({ ...options, stages: '11' }), /Stages must increase/);
  assert.throws(() => capacityPolicy({ ...options, stageSeconds: 19 }), /duration bounds/);
  assert.throws(() => capacityPolicy({ ...options, soakSeconds: 19 }), /duration bounds/);
  assert.equal(capacityPolicy({ profile: 'standard', activityMode: 'native', stages: '100', combatActors: 10 }).stages[0], 100);
  assert.throws(() => capacityPolicy({ profile: 'standard', stages: '101' }), /Stages must increase/);
});

test('movement diagnostic can measure 50 active actors but never qualifies as mixed gameplay acceptance', () => {
  const options = { profile: 'movement', activityMode: 'native', stages: '10,20,30,40,50', combatActors: 0 };
  const diagnostic = capacityPolicy(options), standard = capacityPolicy({ activityMode: 'native' });
  assert.equal(diagnostic.fullAcceptanceEligible, false);
  assert.equal(diagnostic.stages.at(-1), 50);
  for (const field of ['movementP95Ms', 'chatP95Ms', 'actionTimeoutMs', 'timeoutExclusive', 'maximumCorrectionRatio',
    'minimumNativeRunRatio', 'minimumNativeCellsPerMinute', 'commandsPerSecond', 'maximumMemorySlopeMiBPerHour',
    'memoryWarmupSeconds', 'maximumDriverLagP99Ms', 'soakSeconds', 'resumeSamples', 'saveSamples']) {
    assert.equal(diagnostic[field], standard[field], field);
  }
  for (const changes of [{ combatActors: 1 }, { activityMode: 'baseline' }, { stages: '101' }, { soakSeconds: 300 },
    { stageSeconds: 60 }, { resumeSamples: 0 }, { saveSamples: 0 }]) assert.throws(() => capacityPolicy({ ...options, ...changes }));
  assert.throws(() => capacityPolicy({ ...options, profile: 'standard' }), /combat actors/);
});

test('movement diagnostic retains collision failures but never continues transport, AOI or idle-load failures', () => {
  const policy = capacityPolicy({ profile: 'movement', activityMode: 'native' });
  const verdict = { passed: false, seconds: 60, activity: { moves: 74, plans: 80, gaps: 0, offeredDurationMs: 60000, unknownPacing: 0 },
    latencyPassed: true, fanoutPassed: true, timeoutRatio: 0, correctionRatio: 4 / 78, metrics: {} };
  assert.equal(movementDiagnosticMayContinue(policy, verdict), true);
  assert.equal(verdict.passed, false); assert.equal(verdict.correctionRatio, 4 / 78);
  assert.equal(movementDiagnosticMayContinue(capacityPolicy({ activityMode: 'native' }), verdict), false);
  for (const change of [{ latencyPassed: false }, { fanoutPassed: false }, { timeoutRatio: .01 },
    { activity: { ...verdict.activity, moves: 29 } }, { activity: { ...verdict.activity, offeredDurationMs: 100 } },
    { metrics: { run: { status: { rejected: 1 } } } }, { aoi: { persistentMissing: 1, ghosts: 0, duplicates: 0 } }]) {
    assert.equal(movementDiagnosticMayContinue(policy, { ...verdict, ...change }), false);
  }
  const events = [], stopped = [], gate = new ContinuousAcceptance(policy, { emit: event => {
    events.push(event); if (event.type === 'continuousFailure' && !movementDiagnosticMayContinue(policy, event.evidence)) stopped.push(event.reason);
  }});
  gate.fail('collision window', verdict);
  gate.fail('later transport failure', { status: 'transportError' });
  assert.equal(gate.summary().passed, false); assert.equal(gate.failureCount, 2);
  assert.deepEqual(stopped, ['later transport failure']);
  assert.equal(events.length, 2);
});

test('one real peer admits at most 25 within an entire 15-minute window and reserves only 5 verification logins', () => {
  let now = 1000000; const budget = new LoginBudget([], now, () => now);
  for (let i = 0; i < 25; i++) { budget.reserve(); now++; }
  assert.ok(budget.delay() > 899000); assert.throws(() => budget.reserve(), /not available/);
  for (let i = 0; i < 5; i++) budget.reserve('verification');
  assert.throws(() => budget.reserve('verification'), /not available/);
  const recovered = new LoginBudget(JSON.parse(JSON.stringify(budget.attempts)), 0, () => now);
  assert.equal(recovered.delay(), budget.delay());
  now = 1000000 + LOGIN_WINDOW_MS - 1; assert.equal(recovered.delay(), 1);
  now++; recovered.reserve(); assert.equal(recovered.attempts.length, 30);
  assert.throws(() => recovered.reserve(), /not available/);
  now += LOGIN_WINDOW_MS; recovered.reserve(); assert.equal(recovered.attempts.length, 1);
});

test('private pool accepts 100 dedicated actors but refuses original players, duplicate identities and endpoint substitution', () => {
  const url = 'wss://test.example/playtest/ws', value = pool(url, 100);
  assert.equal(validateCapacityPool(value, url).accounts.length, 100);
  assert.throws(() => validateCapacityPool(pool(url, 101), url));
  const duplicate = pool(url, 2); duplicate.accounts[1] = duplicate.accounts[0]; assert.throws(() => validateCapacityPool(duplicate, url), /Duplicate/);
  for (const id of ['demo', 'ptac9c7c7b9', 'ptbc9c7c7b9']) {
    const invalid = pool(url); invalid.accounts[0].accountId = id; assert.throws(() => validateCapacityPool(invalid, url));
  }
  assert.throws(() => validateCapacityPool(value, 'wss://different.example/ws'));
});

test('fixed-rate plans report omitted offered load and never replay a backlog after a driver stall', () => {
  const plan = new FixedPlan(0);
  assert.deepEqual(plan.take(0), { due: 0, missed: 0, index: 1 });
  assert.deepEqual(plan.take(5450), { due: 5000, missed: 4, index: 6 });
  assert.equal(plan.take(5450), null);
  assert.deepEqual(plan.take(6000), { due: 6000, missed: 0, index: 7 });
});

test('native mode is explicit, keeps baseline defaults, and requires a declared ordinary combat share', () => {
  const baseline = capacityPolicy(); assert.equal(baseline.activityMode, 'baseline'); assert.equal(baseline.intentionIntervalMs, 1000);
  assert.equal(baseline.minimumCombatHitsPerMinute, 3); assert.equal(baseline.minimumCombatAttacksPerMinute, 12);
  const native = capacityPolicy({ activityMode: 'native', stages: '10,25,50', combatActors: 10, movementIntervalMs: 650 });
  assert.equal(native.movementIntervalMs, 650); assert.equal(native.commandsPerSecond, 3);
  assert.equal(native.movementP95Ms, baseline.movementP95Ms); assert.equal(native.chatP95Ms, baseline.chatP95Ms);
  assert.equal(native.minimumCombatHitsPerMinute, 12); assert.equal(native.minimumCombatAttacksPerMinute, 18);
  for (const options of [{ movementIntervalMs: 600 }, { movementIntervalMs: 1000 }, { combatActors: 0 }, { combatActors: 13 }]) {
    assert.throws(() => capacityPolicy({ activityMode: 'native', stages: '50', ...options }));
  }
});

const nativeSnapshot = (level = 1, speed = 0) => ({ mapFileName: '0', playerObjectId: 1000, playerCrystalStats: [{ stat: 14, value: speed }],
  entities: [{ objectId: 1000, name: 'fighter', kind: 'player', level, x: 5, y: 5, direction: 'Right' }] });

function huntFixture(monsters, peerMonsters = monsters) {
  const rows = [], actions = [], policy = capacityPolicy({ profile: 'probe', activityMode: 'native' });
  const context = { policy, map: { width: 80, height: 80, blocked: new Uint8Array(6400) }, mapName: '0',
    phase: 'stage-2', monsterNames: ['Scarecrow', 'Hen', 'Deer'], targetClaims: new Map(),
    clients: [], writer: { add: row => rows.push(row) }, add: row => actions.push(row),
    guard: { assert() {}, stop(reason) { throw new Error(reason); } } };
  const client = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), context);
  const owner = { ...nativeSnapshot().entities[0], name: client.label };
  client.snapshot = { ...nativeSnapshot(), entities: [owner, ...monsters] };
  Object.assign(client, { ready: true, inGame: true, home: { x: 5, y: 5 }, role: 'combat', combatArrival: { at: 1 },
    nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity, nativeRunPrimedUntil: 0 });
  const peer = { label: actor(1).name, inGame: true, ready: true, role: 'movement', snapshot: { mapFileName: '0', playerObjectId: 1000,
    entities: [{ objectId: 1000, kind: 'player', name: actor(1).name, x: 5, y: 9 },
      { ...owner, objectId: 50001 }, ...peerMonsters] } };
  context.clients = [client, peer];
  return { client, peer, owner, context, rows, actions };
}
const huntMonster = (objectId, name, x, y = 5) => ({ objectId, kind: 'monster', name, x, y, hp: 20 });

function navigationFixture() {
  const samples = [], sent = [], policy = capacityPolicy({ profile: 'probe', activityMode: 'native', stages: '2' });
  const context = { policy, phase: 'stage-2', mapName: '0', map: { width: 30, height: 30, blocked: new Uint8Array(900) }, clients: [], targetClaims: new Map(),
    writer: { add() {} }, add: row => samples.push(row), guard: { assert() {}, stop(reason) { throw new Error(reason); } } };
  const clients = [0, 1].map(index => {
    const client = new CapacityClient('ws://127.0.0.1:1/ws', actor(index), context);
    const owner = { ...nativeSnapshot().entities[0], name: client.label, x: 5 + 2 * index, y: 5, hp: 30, maxHp: 30 };
    Object.assign(client, { snapshot: { ...nativeSnapshot(), entities: [owner] }, ready: true, inGame: true, role: 'movement', home: { x: owner.x, y: owner.y },
      nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity, plan: new NativePlan(performance.now()) });
    client.ws = { readyState: 0, send: data => sent.push({ client, command: JSON.parse(data) }), close() { this.readyState = 3; client.closed = true; } };
    return client;
  });
  context.clients = clients;
  for (const [index, client] of clients.entries()) client.snapshot.entities.push({ ...clients[1 - index].snapshot.entities[0], objectId: 50000 + 1 - index });
  const planned = () => ({ due: performance.now(), intervalMs: 750, cadence: { intervalMs: 750, cooldownMs: 600, pacingKnown: true } });
  const ack = (client, point, kind = 'walk', direction = 'Right') => {
    client.observe({ type: 'packet', packet: 'UserLocation', payload: { ...point, direction } });
    const receipt = client.events.at(-1), objectId = 50000 + clients.indexOf(client);
    for (const peer of clients.filter(other => other !== client)) peer.observe({ type: 'packet', packet: kind === 'run' ? 'ObjectRun' : 'ObjectWalk', payload: { objectId, location: point, direction } });
    return receipt;
  };
  return { context, clients, samples, sent, planned, ack };
}
const walkPlan = (x, y, direction) => ({ command: { type: 'walk', direction }, first: { x, y }, target: { x, y } });

test('navigation coordination unions true owner receipts and bounded pending cells without replacing AOI, identities or snapshots', () => {
  const { clients: [a, b], context } = navigationFixture(), saved = JSON.stringify(b.snapshot);
  assert.equal(a.attackResults, 'legacy', 'native movement coordination also works with the original attack protocol');
  a.observe({ type: 'packet', packet: 'UserLocation', payload: { x: 5, y: 6, direction: 'Down' } });
  const occupied = navigationObstacles(b), contains = (x, y) => occupied.some(point => point.x === x && point.y === y);
  assert.equal(contains(5, 5), true, 'original stale AOI position is retained'); assert.equal(contains(5, 6), true, 'latest real owner receipt is additionally used');
  assert.equal(contains(7, 5), false, 'self exclusion uses client identity despite both owner IDs1000'); assert.equal(JSON.stringify(b.snapshot), saved);
  const hint = claimNavigationHint(a, walkPlan(6, 6, 'Right'));
  assert.ok(hint); assert.equal(navigationObstacles(b).some(point => point.x === 6 && point.y === 6), true);
  for (const inactive of ['closed', 'paused', 'draining', 'navigationClosing', 'not-ready', 'not-in-game', 'map', 'dead', 'retired']) {
    const original = { ready: a.ready, inGame: a.inGame, map: a.snapshot.mapFileName, dead: a.snapshot.entities[0].dead };
    if (inactive === 'not-ready') a.ready = false; else if (inactive === 'not-in-game') a.inGame = false;
    else if (inactive === 'map') a.snapshot.mapFileName = '1'; else if (inactive === 'dead') a.snapshot.entities[0].dead = true;
    else if (inactive === 'retired') { context.clients = [b]; context.allClients = [a, b]; } else a[inactive] = true;
    assert.equal(navigationObstacles(b).some(point => point.x === 6 && point.y === 6), false, inactive);
    Object.assign(a, { ready: original.ready, inGame: original.inGame, closed: false, paused: false, draining: false, navigationClosing: false });
    a.snapshot.mapFileName = original.map; a.snapshot.entities[0].dead = original.dead; context.clients = [a, b];
  }
  releaseNavigationHint(a, hint); assert.equal(a.navigationHint, null);
  const connectedHint = claimNavigationHint(a, walkPlan(6, 6, 'Right'));
  a.lifecycleConnection++;
  assert.equal(navigationObstacles(b).some(point => point.x === 6 && point.y === 6), false, 'another connection never inherits the prior pending path');
  releaseNavigationHint(a, connectedHint);
  context.clients = Array(101).fill(a); assert.throws(() => navigationObstacles(b), /100 actor ceiling/);
});

test('navigation coordination is native-only and preserves the baseline obstacle and send path', () => {
  const { clients: [a, b], context } = navigationFixture();
  context.policy = capacityPolicy({ profile: 'probe', activityMode: 'baseline', stages: '2' });
  a.observe({ type: 'packet', packet: 'UserLocation', payload: { x: 5, y: 6, direction: 'Down' } });
  const occupied = navigationObstacles(b);
  assert.equal(occupied.some(point => point.x === 5 && point.y === 5), true);
  assert.equal(occupied.some(point => point.x === 5 && point.y === 6), false);
  assert.equal(claimNavigationHint(a, walkPlan(6, 6, 'Right')), null);
  assert.equal(navigationPlanIsClear(b, walkPlan(5, 5, 'Left')), true, 'the original baseline selection/ACK path gains no new pre-send gate');
});

test('navigation hints cover Run intermediate and diagonal corner cells with one immutable bounded entry per active connection', () => {
  const { clients: [a, b] } = navigationFixture();
  const diagonal = { command: { type: 'run', direction: 'DownRight' }, first: { x: 6, y: 6 }, target: { x: 7, y: 7 } };
  const cells = movementHintCells({ x: 5, y: 5 }, diagonal);
  assert.equal(cells.length, NAVIGATION_HINT_MAX_CELLS); assert.deepEqual(cells.map(point => `${point.x},${point.y}`).sort(), ['5,5', '5,6', '6,5', '6,6', '6,7', '7,6', '7,7']);
  assert.equal(movementHintCells({ x: 5, y: 5 }, { ...diagonal, command: { type: 'run', direction: 'Right' } }), null);
  assert.equal(movementHintCells({ x: 5, y: 5 }, { ...diagonal, target: { x: 9, y: 9 } }), null);
  const hint = claimNavigationHint(a, diagonal); assert.ok(Object.isFrozen(hint.cells));
  assert.equal(claimNavigationHint(a, walkPlan(6, 5, 'Right')), null); assert.equal(a.navigationHint, hint);
  releaseNavigationHint(a, {}); assert.equal(a.navigationHint, hint, 'unrelated or old token cannot remove it');
  assert.equal(navigationPlanIsClear(b, walkPlan(6, 6, 'DownLeft')), false, 'peer first/intermediate is blocked');
  assert.equal(navigationPlanIsClear(b, walkPlan(6, 5, 'Left')), false, 'peer diagonal side cell is also hinted');
  releaseNavigationHint(a, hint);
  a.snapshot.entities[0].x = 6; a.snapshot.entities[0].y = 5;
  assert.equal(navigationPlanIsClear(b, { command: { type: 'run', direction: 'DownLeft' }, first: { x: 6, y: 6 }, target: { x: 5, y: 7 } }), false, 'latest true owner on a diagonal side cell blocks Run');
});

test('navigation coordination prevents two same-tick sends to one destination while preserving the blocked native opportunity', async () => {
  const { clients: [a, b], context, samples, sent, planned, ack } = navigationFixture();
  let resolve; a.wait = () => new Promise(done => { resolve = done; });
  const first = action(a, context, planned(), walkPlan(6, 5, 'Right'));
  assert.equal(sent.length, 1); assert.ok(a.navigationHint);
  await action(b, context, planned(), walkPlan(6, 5, 'Left'));
  assert.equal(sent.length, 1, 'later prepared plan cannot be sent to the already hinted destination');
  const blocked = samples.find(row => row.actor === b.label); assert.equal(blocked.status, 'navigation-blocked'); assert.equal(blocked.nativeOpportunity, true); assert.equal(blocked.plannedGameplay, true);
  const measured = new Measurements(); measured.add(blocked); assert.equal(measured.actors.get(b.label).nativeOpportunities, 1); assert.equal(measured.actors.get(b.label).nativeCompleted, 0);
  assert.equal(assessMeasurements(measured, [{ name: b.label, role: 'movement' }], 1, context.policy, { requireChat: false }).passed, false);
  const receipt = ack(a, { x: 6, y: 5 }); assert.equal(a.navigationHint, null, 'fresh real UserLocation releases the pending path before the waiter finishes');
  resolve(receipt); await first; await Promise.all(a.evidencePending);
  assert.equal(navigationPlanIsClear(b, walkPlan(6, 5, 'Left')), false, 'actual acknowledged owner position continues to block the destination after hint release');
});

test('navigation ACK cleanup cannot let an old finally delete a newer hint and close/map/death release their own entry', async () => {
  const { clients: [a], context, planned, ack } = navigationFixture(); let resolve;
  a.wait = () => new Promise(done => { resolve = done; });
  const running = action(a, context, planned(), walkPlan(6, 5, 'Right')), old = a.navigationHint;
  const receipt = ack(a, { x: 6, y: 5 }), fresh = claimNavigationHint(a, walkPlan(6, 6, 'Down'));
  assert.ok(fresh); assert.notEqual(old, fresh); resolve(receipt); await running; await Promise.all(a.evidencePending);
  assert.equal(a.navigationHint, fresh, 'the old action finally only releases its own identity token');
  a.observe({ type: 'worldSnapshot', payload: { ...a.snapshot, mapFileName: '1' } }); assert.equal(a.navigationHint, null);
  a.snapshot.mapFileName = '0'; const deathHint = claimNavigationHint(a, walkPlan(6, 6, 'Down')); assert.ok(deathHint);
  a.observe({ type: 'packet', packet: 'ObjectDied', payload: { objectId: 1000 } }); assert.equal(a.navigationHint, null);
  a.snapshot.entities[0].dead = false; a.snapshot.playerHp = 30;
  const closeHint = claimNavigationHint(a, walkPlan(6, 6, 'Down')); assert.ok(closeHint); await a.close(); assert.equal(a.navigationHint, null);
});

test('navigation hints release on send errors and real owner timeouts without suppressing old failure counters', async () => {
  for (const reason of ['send', 'timeout']) {
    const { clients: [a], context, samples, planned } = navigationFixture();
    if (reason === 'send') a.ws.send = () => { throw new Error('send failed'); };
    else a.wait = async () => { throw new Error('walk authoritative acknowledgement timed out'); };
    await action(a, context, planned(), walkPlan(6, 5, 'Right'));
    assert.equal(a.navigationHint, null); assert.equal(samples[0].status, reason === 'send' ? 'error' : 'timeout');
    assert.equal(samples[0].nativeOpportunity, true);
    const measured = new Measurements(); measured.add(samples[0]);
    assert.equal(measured.actors.get(a.label).nativeCompleted, 0);
    assert.equal(assessMeasurements(measured, [{ name: a.label, role: 'movement' }], 1, context.policy, { requireChat: false }).passed, false);
  }
});

test('navigation hints release on a same-position owner receipt while the original movement correction still fails acceptance', async () => {
  const { clients: [a], context, samples, planned } = navigationFixture(); let resolve;
  a.wait = () => new Promise(done => { resolve = done; });
  const running = action(a, context, planned(), walkPlan(6, 5, 'Right'));
  assert.ok(a.navigationHint);
  a.observe({ type: 'packet', packet: 'UserLocation', payload: { x: 5, y: 5, direction: 'Right' } });
  assert.equal(a.navigationHint, null);
  resolve(a.events.at(-1)); await running;
  assert.equal(samples[0].status, 'corrected'); assert.deepEqual(samples[0].actual, { x: 5, y: 5 });
  const measured = new Measurements(); measured.add(samples[0]);
  const verdict = assessMeasurements(measured, [{ name: a.label, role: 'movement' }], 1, context.policy, { requireChat: false });
  assert.equal(verdict.correctionRatio, 1); assert.equal(verdict.passed, false);
  assert.equal(measured.actors.get(a.label).nativeCompleted, 0);
});

test('native fast patrol and normal A* both avoid controlled in-flight Run cells while leaving snapshots untouched', () => {
  const { clients: [a, b], context } = navigationFixture();
  const before = JSON.stringify(a.snapshot), hint = claimNavigationHint(b, { command: { type: 'run', direction: 'Left' }, first: { x: 6, y: 5 }, target: { x: 5, y: 5 } });
  assert.ok(hint); a.nativeRunPrimedUntil = performance.now() + 10000;
  const fast = selectNativeAction(a, context); assert.ok(fast); assert.notDeepEqual(fast.target, { x: 7, y: 5 }); assert.equal(navigationPlanIsClear(a, fast), true);
  a.nativeRunPrimedUntil = 0;
  const walk = selectNativeAction(a, context); assert.ok(walk); assert.equal(navigationPlanIsClear(a, walk), true);
  assert.equal(JSON.stringify(a.snapshot), before, 'coordination never injects entities or authoritative positions');
  releaseNavigationHint(b, hint);
});

test('attack results v2 is explicit, preserves capability flags and issues strictly increasing bounded decimal IDs without fallback', () => {
  assert.equal(attackResultsMode(), 'legacy'); assert.equal(attackResultsMode('v2'), 'v2');
  assert.throws(() => attackResultsMode('auto')); assert.equal(parseArguments(['--attack-results=v2']).attackResults, 'v2');
  assert.throws(() => parseArguments(['--attack-results=v2', '--attack-results', 'legacy']), /repeated/);
  const fixture = huntFixture([]), { context } = fixture, sent = [];
  context.attackResults = 'v2'; const client = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), context);
  client.ws = { send: data => sent.push(JSON.parse(data)) };
  assert.throws(() => client.nextAttackV2(900), /not confirmed/);
  client.control({ type: 'clientCapabilities', capabilities: ['nativeResumeV1', 'serverCatalogGzipV1', ATTACK_OUTCOME_CAPABILITY] });
  assert.deepEqual(sent[0].capabilities, ['nativeResumeV1', 'serverCatalogGzipV1', ATTACK_OUTCOME_CAPABILITY]);
  assert.throws(() => client.send({ type: 'attack', objectId: 900 }), /forbidden/);
  client.attackV2Supported = true;
  const first = client.nextAttackV2(900); assert.equal(first.requestId, '1'); client.send(first);
  assert.throws(() => client.send(first), /Invalid/);
  for (const requestId of ['0', '01', '-1', '18446744073709551616']) assert.throws(() => client.send({ ...first, requestId }), /Invalid/);
  assert.throws(() => client.send({ ...first, requestId: '2', objectId: 0 }), /Invalid/);
  client.attackV2Sequence = 18446744073709551614n;
  const last = client.nextAttackV2(900); client.send(last); assert.equal(last.requestId, '18446744073709551615');
  assert.throws(() => client.nextAttackV2(900), /exhausted/);
  assert.equal(sent.filter(row => row.type === 'attackV2').length, 2);
});

test('attackResultV2 matching rejects wrong IDs, malformed versions/results and conflicting cache replays', () => {
  const request = { requestId: '1', objectId: 900 }, good = { type: 'attackResultV2', protocolVersion: 2, ...request, result: 'launched' };
  for (const change of [{ requestId: '2' }, { requestId: '01' }, { objectId: 901 }, { protocolVersion: 1 },
    { result: 'success' }, { code: 'extra' }, { result: 'unknown', code: 'private-error' }]) assert.equal(matchAttackResultV2([{ ...good, ...change }], request), null);
  assert.equal(matchAttackResultV2([good, { ...good }], request), good);
  assert.throws(() => matchAttackResultV2([good, { ...good, result: 'notLaunched' }], request), /Conflicting/);
});

test('V2 admission measurements keep legacy counters unchanged and never dilute movement corrections or credit unlaunched damage', () => {
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native', stages: '2', combatActors: 1 });
  const legacy = { actor: 'fighter', kind: 'attack', status: 'success', damage: 3, sharedHealthVerified: true,
    nativeOpportunity: true, plannedGameplay: true, plannedIntervalMs: 1422, latencyMs: 1, plannedLatencyMs: 1 };
  const a = new Measurements(); a.add(legacy); assert.equal(a.actors.get('fighter').attacks, 1);
  assert.equal(Object.hasOwn(a.summary().attack, 'attackV2Requests'), false);
  const rows = new Measurements(), v2 = { ...legacy, attackResults: 'v2', attackRequestSent: true };
  rows.add({ ...v2, attackResult: 'notLaunched', status: 'not-launched' });
  rows.add({ ...v2, status: 'timeout' });
  rows.add({ ...v2, attackResult: 'launched', status: 'miss' });
  rows.add({ ...v2, attackResult: 'launched' });
  const activity = rows.actors.get('fighter');
  assert.equal(activity.attacks, 2); assert.equal(activity.hits, 1); assert.equal(activity.nativeCompleted, 2);
  assert.equal(activity.nativeOpportunities, 4); assert.equal(rows.summary().attack.attackV2Requests, 4);
  for (let i = 0; i < 96; i++) rows.add({ ...v2, attackResult: 'launched' });
  for (let i = 0; i < 10; i++) rows.add({ actor: 'mover', kind: 'walk', status: i ? 'success' : 'corrected',
    plannedGameplay: true, nativeOpportunity: true, plannedIntervalMs: 750, latencyMs: 1, plannedLatencyMs: 1 });
  const verdict = assessMeasurements(rows, [{ name: 'fighter', role: 'combat' }, { name: 'mover', role: 'movement' }], 60, policy, { requireChat: false });
  assert.equal(verdict.attackRejectionRatio, .01); assert.equal(verdict.movementCorrectionRatio, .1); assert.equal(verdict.correctionRatio, .1);
  const rejected = new Measurements(); for (let i = 0; i < 20; i++) rejected.add({ ...v2,
    attackResult: i < 2 ? 'notLaunched' : 'launched', status: i < 2 ? 'not-launched' : 'success' });
  const rejectedVerdict = assessMeasurements(rejected, [{ name: 'fighter', role: 'combat' }], 60, policy, { requireChat: false });
  assert.equal(rejectedVerdict.attackRejectionRatio, .1); assert.equal(rejectedVerdict.passed, false);
  for (const denied of [2, 3]) {
    const gate = new ContinuousAcceptance(policy); gate.join('fighter', 'combat', 0);
    for (let i = 0; i < 40; i++) gate.add({ ...v2, attackResult: i < denied ? 'notLaunched' : 'launched',
      status: i < denied ? 'not-launched' : 'success' }, i * 1422 + 100);
    gate.sync([{ name: 'fighter', active: true }], 60000);
    const window = gate.actors.get('fighter').lastWindow;
    assert.equal(window.attackRejectionRatio, denied / 40); assert.equal(window.correctionRatio, denied / 40);
    assert.equal(window.passed, denied === 2, 'the original inclusive5% boundary and native completion gate both apply');
  }
});

function declaredObservers(homes, observers) {
  return { observerHomes: homes, observerNames: new Set(observers.map(peer => peer.label)), patrolSpan: 2,
    observerBindings: homes.map((home, homeIndex) => ({ home, homeIndex, actorIndex: homeIndex, name: observers[homeIndex].label })) };
}

function scoutFixture() {
  const fixture = huntFixture([]), { client, peer, context } = fixture;
  const secondPeer = { label: actor(2).name, inGame: true, ready: true, role: 'movement', snapshot: { mapFileName: '0', playerObjectId: 1000,
    entities: [{ objectId: 1000, name: actor(2).name, kind: 'player', x: 20, y: 5 }] } };
  context.clients.push(secondPeer); fixture.secondPeer = secondPeer;
  client.combatCluster = declaredObservers([{ x: 5, y: 9 }, { x: 20, y: 5 }], [peer, secondPeer]);
  client.combatScoutCursor = 1;
  return fixture;
}

test('active observer coverage binds only declared living movement clients and never borrows a future home', () => {
  for (const unavailable of ['future', 'not-ready', 'not-in-game', 'closed', 'paused', 'draining', 'dead', 'exact-hp-zero', 'absent', 'map', 'role']) {
    const { client, owner, peer, secondPeer, context } = scoutFixture(); owner.x = 17;
    const target = huntMonster(900, 'Deer', 30); client.snapshot.entities.push(target);
    secondPeer.snapshot.entities[0].x = 40;
    assert.equal(activeCombatCoverage(client, context).entries.length, 2, 'a real declared observer is not limited by fighter distance');
    assert.equal(combatCandidates(client, context).some(e => e.objectId === 900), true);
    if (unavailable === 'future') context.clients.splice(context.clients.indexOf(secondPeer), 1);
    else if (unavailable === 'not-ready') secondPeer.ready = false;
    else if (unavailable === 'not-in-game') secondPeer.inGame = false;
    else if (unavailable === 'exact-hp-zero') secondPeer.snapshot.playerHp = 0;
    else if (unavailable === 'dead') secondPeer.snapshot.entities[0].dead = true;
    else if (unavailable === 'absent') secondPeer.snapshot.entities = [];
    else if (unavailable === 'map') secondPeer.snapshot.mapFileName = '1';
    else if (unavailable === 'role') secondPeer.role = 'combat';
    else secondPeer[unavailable] = true;
    const active = activeCombatCoverage(client, context);
    assert.deepEqual(active.entries.map(e => e.homeIndex), [0], unavailable);
    assert.deepEqual(active.observerHomes, [{ x: 5, y: 9 }], unavailable);
    assert.equal(insideCombatCoverage(active, target), false, unavailable);
    assert.equal(combatCandidates(client, context).some(e => e.objectId === 900), false, unavailable);
    const plan = selectNativeAction(client, context);
    assert.notEqual(plan?.scoutHomeIndex, 1, unavailable);
    if (plan) assert.ok(insideCombatCoverage(active, plan.target), unavailable);
    peer.closed = true;
    assert.deepEqual(activeCombatCoverage(client, context).observerHomes, []);
    assert.equal(selectNativeAction(client, context), null, 'an explicit cluster with zero active observers stays closed');
    assert.equal(client.combatCluster.observerHomes.length, 2, 'active filtering never mutates declared homes');
  }
});

test('scout keeps its declared index and finite ACK/online budgets through observer pause and transport restore', () => {
  const { client, owner, secondPeer, context, rows } = scoutFixture(), now = Date.now();
  const first = selectNativeAction(client, context), scout = client.combatScout;
  assert.equal(first.scoutHomeIndex, 1);
  Object.assign(scout, { startedAt: now - 2000, activeElapsedMs: 3000, confirmedMoves: 23 });
  scout.visited.set('11,5', 2); client.failedTargets.set(999, { until: Infinity }); client.combatPursuitPausedAt = now;
  const restored = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), context);
  Object.assign(restored, { snapshot: structuredClone(client.snapshot), home: client.home, role: 'combat',
    combatArrival: client.combatArrival, sequence: 100, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity,
    failedTargets: client.failedTargets });
  restoreCombatPursuit(client, restored, context, now + 40000);
  secondPeer.paused = true;
  const pausedPlan = selectNativeAction(restored, context);
  assert.notEqual(pausedPlan?.scoutHomeIndex, 1);
  assert.equal(restored.combatScout.homeIndex, 1);
  assert.equal(restored.combatScout.confirmedMoves, 23);
  assert.equal(restored.combatScout.activeElapsedMs, 5000, 'offline time is excluded, prior online waiting remains charged');
  assert.equal(restored.combatScout.visited.get('11,5'), 2);
  assert.equal(restored.failedTargets.get(999).until, Infinity);
  secondPeer.paused = false;
  const resumedPlan = selectNativeAction(restored, context), origin = { ...selfFor(restored) };
  assert.equal(resumedPlan.scoutHomeIndex, 1);
  Object.assign(selfFor(restored), resumedPlan.target);
  recordCombatScoutMovement(restored, context, resumedPlan, origin,
    { packet: 'UserLocation', sequence: 101, payload: resumedPlan.target }, 'success', now + 40001);
  assert.equal(restored.combatScout, null); assert.equal(rows.at(-1).reason, 'movement-budget');
});

test('temporary observer coverage loss and restore do not wash a live pursuit movement budget', () => {
  const { client, owner, secondPeer, context, rows } = scoutFixture(), now = Date.now(); owner.x = 17;
  const target = huntMonster(900, 'Deer', 30); client.snapshot.entities.push(target);
  const first = selectNativeAction(client, context); assert.equal(first.chaseTargetId, 900);
  client.combatPursuit.confirmedMoves = 39; client.combatPursuitPausedAt = now;
  const restored = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), context);
  Object.assign(restored, { snapshot: structuredClone(client.snapshot), home: client.home, role: 'combat', sequence: 100,
    combatArrival: client.combatArrival, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity });
  secondPeer.paused = true;
  restoreCombatPursuit(client, restored, context, now + 5000);
  assert.equal(restored.combatPursuit.confirmedMoves, 39);
  selectNativeAction(restored, context);
  assert.equal(restored.combatPursuit.confirmedMoves, 39);
  secondPeer.paused = false;
  const plan = selectNativeAction(restored, context), origin = { ...selfFor(restored) };
  assert.equal(plan.chaseTargetId, 900);
  Object.assign(selfFor(restored), plan.target);
  recordCombatPursuitMovement(restored, context, plan, origin,
    { packet: 'UserLocation', sequence: 101, payload: plan.target }, 'success', now + 5001);
  assert.equal(restored.combatPursuit, null); assert.equal(rows.at(-1).reason, 'movement-budget');
});

function selfFor(client) { return client.snapshot.entities.find(entity => entity.objectId === client.snapshot.playerObjectId); }

test('pending attack diagnostics retain same-position receipts, exclude unrelated data and cap packets without changing outcomes', () => {
  const target = { ...huntMonster(900, 'Deer', 6), dead: false }, { client, peer, owner, context, rows } = huntFixture([target]);
  beginPendingAttackEvidence(client, peer, owner, target, performance.now(), client.sequence, 0);
  client.record('received', { type: 'resumeCredential', credential: 'never-copy-this' });
  client.record('received', { type: 'packet', packet: 'ObjectChat', payload: { objectId: 900, text: 'never-copy-chat' } });
  client.record('received', { type: 'packet', packet: 'ObjectHealth', payload: { objectId: 901, percent: 1 } });
  client.record('received', { type: 'packet', packet: 'UserLocation', payload: { x: owner.x, y: owner.y, direction: 'Right', password: 'never-copy-password' } });
  recordPendingAttackPacket(peer, { type: 'packet', direction: 'received', packet: 'ObjectStruck', sequence: 1,
    monotonicMs: performance.now(), payload: { objectId: 900, attackerId: 50001 } });
  for (let i = 0; i < PENDING_ATTACK_PACKET_LIMIT + 5; i++) client.record('received', {
    type: 'packet', packet: 'ObjectWalk', payload: { objectId: 900, location: { x: 7, y: 5 }, token: 'never-copy-token' } });
  finishPendingAttackEvidence(client, context, 'timeout');
  const evidence = rows.at(-1);
  assert.equal(evidence.type, 'pendingAttackEvidence'); assert.equal(evidence.outcome, 'timeout');
  assert.deepEqual(evidence.target, { objectId: 900, x: 6, y: 5, dead: false, hp: 20 });
  assert.equal(evidence.packets.length, 64); assert.equal(evidence.packetCount, 71); assert.equal(evidence.truncated, true);
  assert.equal(evidence.packets[0].packet, 'UserLocation'); assert.equal(evidence.packets[1].receiver, 'observer');
  assert.doesNotMatch(JSON.stringify(evidence), /never-copy|credential|password|token/);
  assert.equal(Object.hasOwn(evidence.target, 'incarnation'), false); assert.equal(Object.hasOwn(evidence.target, 'generation'), false);
  assert.equal(client.pendingAttackEvidence, null); assert.equal(client.pendingAttackPacketTraces.size, 0); assert.equal(peer.pendingAttackPacketTraces.size, 0);
  for (const outcome of ['success', 'timeout', 'error', 'transportError', 'cancelled']) {
    beginPendingAttackEvidence(client, peer, owner, target, performance.now(), client.sequence, 1);
    finishPendingAttackEvidence(client, context, outcome);
    assert.equal(client.pendingAttackPacketTraces.size, 0); assert.equal(peer.pendingAttackPacketTraces.size, 0);
  }
});

test('pending attack packet subscriptions are removed after synchronous send failure and transport close', async () => {
  for (const failure of ['send', 'close']) {
    const { client, peer, context, actions, rows } = huntFixture([huntMonster(900, 'Deer', 6)]), stops = [];
    context.guard.stop = reason => stops.push(reason);
    client.plan = new NativePlan(performance.now() - 1);
    client.send = () => { if (failure === 'send') throw new Error('Fixture send failed'); client.closed = true; };
    client.wait = async () => { throw new Error('Fixture transport closed'); };
    offerNativeAction(client, context); await client.pending;
    assert.equal(actions.find(row => row.kind === 'attack').status, failure === 'send' ? 'error' : 'transportError');
    assert.equal(rows.find(row => row.type === 'pendingAttackEvidence').outcome, failure === 'send' ? 'error' : 'transportError');
    assert.equal(client.pendingAttackEvidence, null); assert.equal(client.pendingAttackPacketTraces.size, 0);
    assert.equal(peer.pendingAttackPacketTraces.size, 0); assert.equal(context.targetClaims.size, 0);
    assert.equal(stops.length, failure === 'send' ? 0 : 1);
  }
});

test('native scout uses ordinary covered adjacent goals to explore another declared observer instead of circling one home', () => {
  const { client, owner, context } = scoutFixture();
  client.snapshot.entities.push({ objectId: 3000, kind: 'remotePlayer', x: 20, y: 5 });
  const plan = selectNativeAction(client, context);
  assert.equal(plan.scouting, true); assert.equal(plan.scoutHomeIndex, 1);
  assert.ok(['walk', 'run'].includes(plan.command.type));
  assert.notDeepEqual(plan.navigation.goal, { x: 20, y: 5 }, 'an occupied observer home is never forced as the goal');
  const route = findProtocolWalkPath({ map: context.map, start: owner, target: plan.navigation.goal,
    dynamicObstacles: client.snapshot.entities.filter(e => e.objectId !== owner.objectId), maxExpanded: 4000 });
  assert.ok(route.length > 8); assert.ok(route.every(point => insideCombatCoverage(client.combatCluster, point)));
});

test('native scout goal selection also excludes a controlled in-flight hint without injecting it into the fighter snapshot', () => {
  const { client, secondPeer, context } = scoutFixture(); secondPeer.context = context;
  const first = selectNativeAction(client, context), goal = first.navigation.goal;
  assert.equal(first.scouting, true);
  const origin = selfFor(secondPeer), directions = [[-1, -1, 'UpLeft'], [0, -1, 'Up'], [1, -1, 'UpRight'], [-1, 0, 'Left'],
    [1, 0, 'Right'], [-1, 1, 'DownLeft'], [0, 1, 'Down'], [1, 1, 'DownRight']];
  const direction = directions.find(([dx, dy]) => origin.x + dx === goal.x && origin.y + dy === goal.y)?.[2];
  assert.ok(direction);
  const before = JSON.stringify(client.snapshot), hint = claimNavigationHint(secondPeer, walkPlan(goal.x, goal.y, direction));
  assert.ok(hint);
  const next = selectNativeAction(client, context);
  assert.equal(next.scouting, true); assert.notDeepEqual(next.navigation.goal, goal);
  assert.equal(JSON.stringify(client.snapshot), before);
  releaseNavigationHint(secondPeer, hint);
});

test('native scout never crosses a coverage gap, map transfer or real blocked route and performs at most four searches', () => {
  for (const block of ['coverage-gap', 'transfer', 'terrain']) {
    const { client, context } = scoutFixture();
    if (block === 'coverage-gap') {
      client.combatCluster.observerHomes[1] = { x: 50, y: 5 };
      client.combatCluster.observerBindings[1].home = client.combatCluster.observerHomes[1];
    }
    else if (block === 'transfer') client.snapshot.mapTransfers = [{ bounds: { minX: 10, maxX: 10, minY: 0, maxY: 79 } }];
    else for (let y = 0; y < context.map.height; y++) context.map.blocked[y * context.map.width + 10] = 1;
    client.navigationAttempts = []; client.navigationAttemptCount = 0;
    const plan = selectNativeAction(client, context);
    assert.notEqual(plan?.scoutHomeIndex, 1, block);
    assert.ok(client.navigationAttemptCount <= 5, 'at most four failed scout routes plus the original fallback attempt');
    assert.equal(client.chaseDeferrals.size, 0); assert.equal(client.failedTargets.size, 0);
  }
});

test('native scout counts only fresh real successful movement and preserves pursuit, deferral and ambiguous-attack budgets', () => {
  const { client, owner, context } = scoutFixture(), plan = selectNativeAction(client, context), origin = { ...owner }, now = Date.now();
  const pursuit = { objectId: 900, confirmedMoves: 39, visited: new Map([['1,1', 2]]) };
  client.combatPursuit = pursuit; client.failedTargets.set(901, { until: Infinity }); client.chaseDeferrals.set(902, { until: now + 30000 });
  const receipt = { packet: 'UserLocation', sequence: 1, payload: { ...plan.target } };
  recordCombatScoutMovement(client, context, plan, origin, receipt, 'corrected', now);
  recordCombatScoutMovement(client, context, plan, origin, { ...receipt, payload: origin }, 'success', now);
  assert.equal(client.combatScout.confirmedMoves, 0);
  Object.assign(owner, plan.target); recordCombatScoutMovement(client, context, plan, origin, receipt, 'success', now);
  recordCombatScoutMovement(client, context, plan, origin, receipt, 'success', now + 1000);
  assert.equal(client.combatScout.confirmedMoves, 1);
  assert.equal(client.combatPursuit, pursuit); assert.equal(pursuit.confirmedMoves, 39);
  assert.equal(client.failedTargets.get(901).until, Infinity); assert.equal(client.chaseDeferrals.has(902), true);
});

test('native scout advances its finite waypoint on real position loops, movement budget and time budget', () => {
  for (const reason of ['route-loop', 'movement-budget', 'time-budget']) {
    const { client, owner, context, rows } = scoutFixture(), now = Date.now();
    const original = selectNativeAction(client, context), scout = client.combatScout;
    if (reason === 'route-loop') scout.visited.set(`${original.target.x},${original.target.y}`, 2);
    if (reason === 'movement-budget') scout.confirmedMoves = 23;
    if (reason === 'time-budget') scout.startedAt = now - 20000;
    const origin = { ...owner }; Object.assign(owner, original.target);
    recordCombatScoutMovement(client, context, original, origin,
      { packet: 'UserLocation', sequence: 1, payload: { ...owner } }, 'success', now);
    assert.equal(client.combatScout, null); assert.equal(client.combatScoutCursor, 0); assert.equal(rows.at(-1).reason, reason);
    assert.equal(client.failedTargets.size, 0); assert.equal(client.chaseDeferrals.size, 0);
  }
});

test('native scout yields immediately to a real witnessed combat candidate without changing native pacing or thresholds', () => {
  const { client, peer, owner, context } = scoutFixture(); assert.equal(selectNativeAction(client, context).scouting, true);
  const target = huntMonster(900, 'Scarecrow', 6); client.snapshot.entities.push(target); peer.snapshot.entities.push(target); owner.direction = 'Right';
  const plan = selectNativeAction(client, context);
  assert.equal(plan.command.type, 'attack'); assert.equal(plan.command.objectId, 900); assert.equal(plan.scouting, undefined);
  assert.equal(context.policy.minimumCombatAttacksPerMinute, 18); assert.equal(context.policy.minimumCombatHitsPerMinute, 12);
  assert.equal(nativeCadence(client.snapshot, 'attack', context.policy).intervalMs, 1436);
});

test('native hunt prioritizes a legal adjacent target visible to both peers over an unwitnessed nearer candidate', () => {
  const hidden = huntMonster(900, 'Deer', 6), adjacent = huntMonster(901, 'Scarecrow', 6, 6);
  const { client, owner, context } = huntFixture([hidden, adjacent], [adjacent]); owner.direction = 'DownRight';
  const planned = selectNativeAction(client, context);
  assert.equal(planned.command.type, 'attack'); assert.equal(planned.command.objectId, 901);
  assert.equal(planned.peerAttackerId, 50001);
});

test('native hunt keeps its selected reachable target when another nonadjacent monster becomes nearer', () => {
  const selected = huntMonster(900, 'Deer', 10), alternate = huntMonster(901, 'Scarecrow', 12);
  const { client, context } = huntFixture([selected, alternate]);
  assert.equal(selectNativeAction(client, context).hunting, 'Deer');
  Object.assign(alternate, { x: 8, y: 7 });
  assert.equal(selectNativeAction(client, context).hunting, 'Deer');
});

function acknowledgeEscapingTarget(fixture, target, at, sequence) {
  const { client, owner, context } = fixture;
  const plan = selectNativeAction(client, context), origin = { ...owner };
  assert.ok(['walk', 'run'].includes(plan.command.type)); assert.equal(plan.chaseTargetId, target.objectId);
  const dx = plan.target.x - owner.x, dy = plan.target.y - owner.y;
  Object.assign(owner, plan.target); target.x += dx; target.y += dy;
  const receipt = { packet: 'UserLocation', sequence, payload: { ...plan.target, direction: plan.command.direction } };
  recordCombatPursuitMovement(client, context, plan, origin, receipt, 'success', at);
  return { plan, origin, receipt };
}

test('native hunt follows confirmed A* detour progress even while Chebyshev distance initially increases', () => {
  const target = huntMonster(900, 'Scarecrow', 1), fixture = huntFixture([target]);
  const { client, owner, context } = fixture;
  for (let y = 0; y <= 10; y++) context.map.blocked[y * context.map.width + 4] = 1;
  const now = Date.now(), initialDistance = Math.max(Math.abs(owner.x - target.x), Math.abs(owner.y - target.y));
  let movedAway = false, attacked = false;
  for (let sequence = 1; sequence <= 30; sequence++) {
    const plan = selectNativeAction(client, context);
    if (plan.command.type === 'turn') { owner.direction = plan.command.direction; continue; }
    if (plan.command.type === 'attack') { attacked = true; break; }
    assert.equal(plan.chaseTargetId, target.objectId);
    assert.ok(Number.isInteger(plan.navigation.remainingSteps));
    const origin = { ...owner }; Object.assign(owner, plan.target);
    movedAway ||= Math.max(Math.abs(owner.x - target.x), Math.abs(owner.y - target.y)) > initialDistance;
    recordCombatPursuitMovement(client, context, plan, origin,
      { packet: 'UserLocation', sequence, payload: { ...owner } }, 'success', now + sequence * 650);
    assert.ok(client.combatPursuit, 'a decreasing routed distance must survive a necessary detour');
    assert.equal(client.chaseDeferrals.size, 0);
  }
  assert.equal(movedAway, true); assert.equal(attacked, true);
});

test('native hunt does not credit a newly planned route when the target moves during each authoritative step', () => {
  const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]), now = Date.now();
  for (let sequence = 1; sequence <= 4; sequence++) {
    const { plan } = acknowledgeEscapingTarget(fixture, target, now + sequence * 650, sequence);
    assert.ok(plan.navigation.remainingSteps > 0, 'fixture uses an actual A* route');
  }
  assert.equal(fixture.client.combatPursuit, null);
  assert.equal(fixture.rows.at(-1).reason, 'no-progress');
});

test('native hunt cannot turn real blocking corrections into detour progress or successful movement evidence', () => {
  const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]);
  const { client, owner, context } = fixture, now = Date.now(), plan = selectNativeAction(client, context);
  for (let sequence = 1; sequence <= 8; sequence++) {
    recordCombatPursuitMovement(client, context, plan, { ...owner },
      { packet: 'UserLocation', sequence, payload: { ...owner } }, 'corrected', now + sequence * 1000);
  }
  assert.equal(client.combatPursuit.confirmedMoves, 0); assert.equal(client.combatPursuit.routeProgress, undefined);
  for (let y = 0; y < context.map.height; y++) context.map.blocked[y * context.map.width + 7] = 1;
  const blocked = selectNativeAction(client, context);
  assert.equal(blocked?.chaseTargetId, undefined); assert.equal(client.combatPursuit, null);
  assert.equal(client.chaseDeferrals.size, 0, 'a blocked plan provides no authoritative movement to defer from');
  assert.equal(context.policy.minimumCombatHitsPerMinute, 12);
});

test('native hunt defers an authoritative position loop even when replanned routes claim progress', () => {
  const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]);
  const { client, owner, context, rows } = fixture; selectNativeAction(client, context); const now = Date.now();
  for (let sequence = 1; sequence <= 5; sequence++) {
    const origin = { ...owner }, actual = { x: sequence % 2 ? 6 : 5, y: 5 };
    const plan = { command: { type: 'walk', direction: sequence % 2 ? 'Right' : 'Left' },
      first: actual, target: actual, chaseTargetId: 900, chaseTargetPoint: { ...target },
      navigation: { goal: { x: 9, y: 5 }, remainingSteps: 20 - sequence } };
    Object.assign(owner, actual);
    recordCombatPursuitMovement(client, context, plan, origin,
      { packet: 'UserLocation', sequence, payload: actual }, 'success', now + sequence * 100);
  }
  assert.equal(client.combatPursuit, null); assert.equal(rows.at(-1).reason, 'route-loop');
  assert.equal(rows.at(-1).confirmedMoves, 5); assert.equal(client.chaseDeferrals.size, 1);
});

test('native hunt retains confirmed loop evidence across genuinely blocked replans and ordinary fallback patrol ACKs', () => {
  const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]);
  const { client, owner, context, rows } = fixture; const now = Date.now();
  let sequence = 0, chaseLanding;
  for (let cycle = 0; cycle < 3; cycle++) {
    const plan = selectNativeAction(client, context), origin = { ...owner };
    assert.equal(plan.chaseTargetId, 900); Object.assign(owner, plan.target);
    chaseLanding ??= { ...owner }; assert.equal(owner.x, chaseLanding.x); assert.equal(owner.y, chaseLanding.y);
    recordCombatPursuitMovement(client, context, plan, origin,
      { packet: 'UserLocation', sequence: ++sequence, payload: { ...owner } }, 'success', now + cycle * 2000);
    if (cycle === 2) break;
    const pursuit = client.combatPursuit, moves = pursuit.confirmedMoves;
    for (let y = 4; y <= 6; y++) for (let x = 9; x <= 11; x++) context.map.blocked[y * context.map.width + x] = 1;
    const fallback = selectNativeAction(client, context), fallbackOrigin = { ...owner };
    assert.equal(fallback.chaseTargetId, undefined); assert.equal(client.combatPursuit, pursuit);
    // Restore the previous real owner origin using the same ordinary planner;
    // this is an actual acknowledged fallback step, not an invented chase ACK.
    client.targetCursor = 3;
    const homePlan = selectNativeAction(client, context);
    assert.ok(['walk', 'run'].includes(homePlan.command.type));
    assert.deepEqual(homePlan.target, { x: origin.x, y: origin.y });
    Object.assign(owner, homePlan.target);
    recordCombatPursuitMovement(client, context, homePlan, fallbackOrigin,
      { packet: 'UserLocation', sequence: ++sequence, payload: { ...owner } }, 'success', now + cycle * 2000 + 750);
    assert.equal(pursuit.confirmedMoves, moves, 'fallback patrol cannot manufacture chase progress');
    for (let y = 4; y <= 6; y++) for (let x = 9; x <= 11; x++) context.map.blocked[y * context.map.width + x] = 0;
  }
  assert.equal(client.combatPursuit, null); assert.equal(rows.at(-1).reason, 'route-loop');
  assert.equal(rows.at(-1).confirmedMoves, 3); assert.equal(client.chaseDeferrals.size, 1);
});

test('native hunt keeps finite movement and time budgets despite continually decreasing replanned route lengths', () => {
  for (const budget of ['movement-budget', 'time-budget']) {
    const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]);
    const { client, owner, context, rows } = fixture; selectNativeAction(client, context); const now = Date.now();
    client.combatPursuit.confirmedMoves = budget === 'movement-budget' ? 39 : 0;
    client.combatPursuit.startedAt = budget === 'time-budget' ? now - 30000 : now;
    const plan = selectNativeAction(client, context), origin = { ...owner }; Object.assign(owner, plan.target);
    recordCombatPursuitMovement(client, context, plan, origin,
      { packet: 'UserLocation', sequence: 1, payload: { ...owner } }, 'success', now);
    assert.equal(client.combatPursuit, null); assert.equal(rows.at(-1).reason, budget);
    assert.equal(rows.at(-1).attacksSent, 0);
  }
});

test('native hunt resume preserves consumed pursuit budgets without charging offline time or sharing mutable route history', () => {
  const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]);
  const { client, context } = fixture; selectNativeAction(client, context); const now = Date.now();
  Object.assign(client.combatPursuit, { startedAt: now, lastMovementAt: now + 2000, activeElapsedMs: 20000,
    confirmedMoves: 39, routeInitialized: true, routeProgress: { key: 'old-route', bestRemaining: 2 } });
  client.combatPursuitPausedAt = now + 10000;
  client.combatPursuit.visited.set('6,5', 1);
  const restored = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), context);
  restored.snapshot = structuredClone(client.snapshot); restored.sequence = 100;
  restoreCombatPursuit(client, restored, context, now + 40000);
  assert.equal(restored.combatPursuit.activeElapsedMs, 30000, 'online waiting after the last ACK remains charged');
  assert.equal(restored.combatPursuit.startedAt, now + 40000);
  assert.equal(restored.combatPursuit.confirmedMoves, 39);
  assert.equal(restored.combatPursuit.routeProgress, null);
  assert.notEqual(restored.combatPursuit.visited, client.combatPursuit.visited);
  assert.equal(restored.combatPursuit.visited.get('6,5'), 1);
});

test('native hunt defers a repeatedly escaping unattacked target for30s then selects another without touching attack quarantine', () => {
  const fleeing = huntMonster(900, 'Deer', 10), alternate = huntMonster(901, 'Scarecrow', 18);
  const fixture = huntFixture([fleeing, alternate]), { client, context, rows } = fixture;
  selectNativeAction(client, context); const now = Date.now();
  client.failedTargets.set(999, { until: Infinity, ambiguous: true, failures: 1 });
  for (let index = 1; index <= 4; index++) acknowledgeEscapingTarget(fixture, fleeing, now + index * 650, index);
  assert.equal(client.combatPursuit, null); assert.equal(rows.filter(row => row.type === 'combatChaseDeferred').length, 1);
  const deferred = client.chaseDeferrals.get(900); assert.equal(deferred.until, now + 2600 + 30000);
  assert.equal(client.failedTargets.has(900), false, 'a futile chase is not a failed or ambiguous attack');
  assert.equal(client.failedTargets.get(999).until, Infinity);
  assert.equal(combatCandidates(client, context, deferred.until - 1).some(target => target.objectId === 900), false);
  assert.equal(combatCandidates(client, context, deferred.until).some(target => target.objectId === 900), true);
  assert.equal(selectNativeAction(client, context).hunting, 'Scarecrow');
  assert.equal(rows[0].attacksSent, 0); assert.equal(rows[0].nonImprovingMoves, 4);
});

test('native hunt time threshold requires repeated actual movement and never counts plans, duplicate receipts or corrected steps', () => {
  const fleeing = huntMonster(900, 'Deer', 10), fixture = huntFixture([fleeing]);
  const { client, context, owner } = fixture;
  let plan = selectNativeAction(client, context); const now = Date.now(); client.combatPursuit.lastProgressAt = now - 60000;
  for (let index = 0; index < 20; index++) selectNativeAction(client, context);
  assert.equal(client.combatPursuit.nonImprovingMoves, 0); assert.equal(client.chaseDeferrals.size, 0);
  const correction = { packet: 'UserLocation', sequence: 1, payload: { x: owner.x, y: owner.y } };
  recordCombatPursuitMovement(client, context, plan, { ...owner }, correction, 'corrected', now);
  recordCombatPursuitMovement(client, context, plan, { ...owner }, correction, 'success', now);
  assert.equal(client.combatPursuit.nonImprovingMoves, 0);
  const first = acknowledgeEscapingTarget(fixture, fleeing, now, 2);
  assert.equal(client.combatPursuit.nonImprovingMoves, 1); assert.equal(client.chaseDeferrals.size, 0);
  recordCombatPursuitMovement(client, context, first.plan, first.origin, first.receipt, 'success', now + 7000);
  assert.equal(client.combatPursuit.nonImprovingMoves, 1, 're-observing one ACK cannot become a second move');
  acknowledgeEscapingTarget(fixture, fleeing, now + 1000, 3);
  assert.equal(client.chaseDeferrals.size, 1); assert.equal(client.combatPursuit, null);
});

test('native hunt distance improvement resets futile-chase evidence and already attacked pursuits never enter the temporary deferral map', () => {
  const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]);
  const { client, owner, context } = fixture; selectNativeAction(client, context); const now = Date.now();
  for (let index = 1; index <= 3; index++) acknowledgeEscapingTarget(fixture, target, now + index * 100, index);
  assert.equal(client.combatPursuit.nonImprovingMoves, 3);
  const plan = selectNativeAction(client, context), origin = { ...owner }; Object.assign(owner, plan.target);
  recordCombatPursuitMovement(client, context, plan, origin, { packet: 'UserLocation', sequence: 4, payload: { ...plan.target } }, 'success', now + 400);
  assert.equal(client.combatPursuit.nonImprovingMoves, 0); assert.equal(client.combatPursuit.bestDistance, 4);
  client.combatPursuit.attackSent = true;
  for (let index = 5; index <= 10; index++) acknowledgeEscapingTarget(fixture, target, now + 10000 + index * 100, index);
  assert.equal(client.chaseDeferrals.size, 0); assert.equal(client.combatPursuit.attackSent, true);
  client.failedTargets.set(900, { until: Infinity, ambiguous: true, failures: 1 });
  selectNativeAction(client, context);
  assert.equal(client.combatPursuit, null);
  assert.equal(combatCandidates(client, context, now + 1e12).some(entity => entity.objectId === 900), false);
});

test('native hunt movement hook keeps all corrected and successful actions in the original denominator', async () => {
  const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]), { client, peer, owner, context, actions, rows } = fixture;
  peer.paused = true; let corrected = true, planned;
  client.send = command => {
    if (!corrected) {
      const dx = planned.target.x - owner.x, dy = planned.target.y - owner.y;
      Object.assign(owner, planned.target); target.x += dx; target.y += dy;
    }
    client.events.push({ direction: 'received', type: 'packet', packet: 'UserLocation', sequence: ++client.sequence,
      monotonicMs: performance.now(), payload: { x: owner.x, y: owner.y, direction: command.direction } });
  };
  const offer = async () => {
    client.nativeRunPrimedUntil = 0; planned = selectNativeAction(client, context);
    client.plan = new NativePlan(performance.now()); offerNativeAction(client, context); await client.pending;
  };
  await offer(); assert.equal(actions[0].status, 'corrected'); assert.equal(client.combatPursuit.nonImprovingMoves, 0);
  corrected = false;
  for (let index = 0; index < 4; index++) await offer();
  assert.equal(actions.length, 5); assert.equal(actions.filter(row => row.status === 'success').length, 4);
  assert.ok(actions.every(row => row.plannedGameplay && row.nativeOpportunity && row.chaseTargetId === 900));
  assert.equal(rows.filter(row => row.type === 'combatChaseDeferred').length, 1);
  assert.equal(client.chaseDeferrals.size, 1); assert.equal(client.failedTargets.size, 0);
  assert.equal(context.policy.minimumCombatHitsPerMinute, 12);
});

test('native hunt drops dead, missing, friendly, claimed and unreachable targets without laundering failed attack identities', () => {
  for (const invalidate of [
    ({ target }) => { target.dead = true; },
    ({ target }) => { target.hp = 0; },
    ({ target }) => { target.disposition = 'friendly'; },
    ({ target }) => { target.ownerName = 'ordinary-owner'; },
    ({ client, owner }) => { client.snapshot.entities = [owner]; },
    ({ context }) => { context.targetClaims.set(900, 'another-fighter'); },
    ({ context, target }) => { for (let x = target.x - 1; x <= target.x + 1; x++) for (let y = target.y - 1; y <= target.y + 1; y++) context.map.blocked[y * context.map.width + x] = 1; },
  ]) {
    const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]);
    selectNativeAction(fixture.client, fixture.context); assert.equal(fixture.client.combatPursuit.objectId, 900);
    invalidate({ ...fixture, target }); const next = selectNativeAction(fixture.client, fixture.context);
    assert.equal(fixture.client.combatPursuit, null); assert.equal(next?.hunting, undefined);
    assert.equal(fixture.client.failedTargets.size, 0); assert.equal(fixture.client.chaseDeferrals.size, 0);
  }
});

test('native hunt deferrals stay bounded and resume rebases only pursuit timing while retaining quarantine and map resets', () => {
  const target = huntMonster(900, 'Deer', 10), fixture = huntFixture([target]), { client, context } = fixture;
  selectNativeAction(client, context); const now = Date.now();
  for (let index = 0; index < 16; index++) client.chaseDeferrals.set(2000 + index, { mapFileName: '0', until: now + 90000 });
  for (let index = 1; index <= 4; index++) acknowledgeEscapingTarget(fixture, target, now + index * 100, index);
  assert.equal(client.chaseDeferrals.size, 16); assert.equal(client.chaseDeferrals.has(2000), false);
  assert.equal(client.chaseDeferrals.has(900), true);
  client.chaseDeferrals.delete(900); selectNativeAction(client, context);
  client.combatPursuit.attackSent = true; client.combatPursuit.nonImprovingMoves = 3;
  client.failedTargets.set(999, { until: Infinity, ambiguous: true, failures: 1 });
  const restored = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), context);
  restored.snapshot = structuredClone(client.snapshot); restored.sequence = 400; restored.failedTargets = client.failedTargets;
  client.combatCluster = declaredObservers([{ x: 5, y: 5 }], [fixture.peer]);
  restoreCombatPursuit(client, restored, context, now + 5000);
  assert.equal(restored.combatPursuit.objectId, 900); assert.equal(restored.combatPursuit.attackSent, true);
  assert.equal(restored.combatPursuit.nonImprovingMoves, 0); assert.equal(restored.combatPursuit.lastReceiptSequence, 400);
  assert.equal(restored.combatPursuit.lastProgressAt, now + 5000); assert.equal(restored.failedTargets.get(999).until, Infinity);
  assert.notEqual(restored.chaseDeferrals, client.chaseDeferrals); assert.equal(restored.chaseDeferrals.size, 15);
  assert.equal(restored.combatCluster, client.combatCluster, 'resume keeps the exact declared observer footprint');
  restored.snapshot.mapFileName = 'other'; restoreCombatPursuit(client, restored, context, now + 6000);
  assert.equal(restored.combatPursuit, null); assert.equal(restored.chaseDeferrals.size, 0);
  client.observe({ type: 'packet', packet: 'MapInformation', payload: { fileName: '0', title: 'same-map reset' } });
  assert.equal(client.combatPursuit, null); assert.equal(client.chaseDeferrals.size, 0);
  assert.equal(client.failedTargets.get(999).until, Infinity);
});

test('native timing follows authoritative level, attack speed and slow while preserving the 550/600ms difference', () => {
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native', movementIntervalMs: 650 });
  assert.equal(nativeCadence(nativeSnapshot(1, 0), 'attack', policy).intervalMs, 1436);
  assert.equal(nativeCadence(nativeSnapshot(7, 2), 'attack', policy).intervalMs, 1232);
  const fast = nativeCadence(nativeSnapshot(28, 8), 'attack', policy);
  assert.equal(fast.nativeAttackMs, 550); assert.equal(fast.zoneAttackFloorMs, 600); assert.equal(fast.intervalMs, 650);
  assert.equal(nativeCadence(nativeSnapshot(1, -2), 'attack', policy).intervalMs, 1556);
  const slowed = nativeSnapshot(); slowed.entities[0].poison = 4;
  assert.equal(nativeCadence(slowed, 'run', policy).intervalMs, 1250);
  assert.equal(nativeCadence({}, 'attack', policy).pacingKnown, false);
  assert.equal(nativeCadence({}, 'attack', policy).intervalMs, 1450);
});

test('native attack pacing uses the real sparse WorldSnapshot zero-stat contract without guessing absent or malformed blocks', () => {
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native' });
  // The actual probe self entity exposed level=3; its sparse stat projection
  // had no stat 14. WorldSnapshot/stats.rs define missing ids as zero only
  // when the containing playerCrystalStats array is present.
  const snapshot = { ...nativeSnapshot(3, 0), playerCrystalStats: [{ stat: 0, value: 1 }, { stat: 1, value: 5 }, { stat: 8, value: 5 }] };
  for (const stats of [snapshot.playerCrystalStats, [], [{ stat: 14, value: 0 }]]) {
    const cadence = nativeCadence({ ...snapshot, playerCrystalStats: stats }, 'attack', policy);
    assert.equal(cadence.pacingKnown, true); assert.equal(cadence.attackSpeed, 0);
    assert.equal(cadence.nativeAttackMs, 1358); assert.equal(cadence.intervalMs, 1408);
    assert.equal(cadence.attackSpeedSource, stats.some(stat => stat.stat === 14) ? 'authoritative-stat-14' : 'authoritative-sparse-zero');
  }
  for (const stats of [undefined, null, {}, [{ stat: 14, value: '0' }], [{ stat: 14, value: NaN }],
    [{ stat: 14, value: 0 }, { stat: 14, value: 2 }], [{ stat: 400, value: 1 }]]) {
    assert.equal(nativeCadence({ ...snapshot, playerCrystalStats: stats }, 'attack', policy).pacingKnown, false);
  }
  const missingLevel = { ...snapshot, entities: [{ ...snapshot.entities[0], level: undefined }] };
  assert.equal(nativeCadence(missingLevel, 'attack', policy).pacingKnown, false);
  const measurements = new Measurements();
  for (let i = 0; i < 22; i++) measurements.add({ actor: 'fighter', kind: 'attack', status: 'success', plannedGameplay: true,
    nativeOpportunity: true, plannedIntervalMs: 1408, pacingKnown: nativeCadence(snapshot, 'attack', policy).pacingKnown,
    damage: 3, sharedHealthVerified: true, latencyMs: 300, plannedLatencyMs: 310 });
  assert.equal(measurements.actors.get('fighter').unknownPacing, 0);
  assert.equal(nativeActivityVerdict(measurements.actors.get('fighter'), 'combat', 30, policy).passed, true);
});

test('native calendar never catches up by violating elapsed cooldown after jitter or a stall', () => {
  const plan = new NativePlan(0);
  assert.equal(plan.take(200, 650).due, 0); plan.sent(200, 600);
  assert.equal(plan.take(650, 650), null);
  assert.equal(plan.take(800, 650).due, 800); plan.sent(800, 600);
  const afterStall = plan.take(4100, 650);
  assert.equal(afterStall.missed, 4); assert.equal(afterStall.due, 4050);
  plan.sent(4100, 600); assert.equal(plan.take(4100, 650), null);
});

test('native evidence waiting cannot be hidden as reduced offered load or mislabeled busy', () => {
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native' }), m = new Measurements();
  for (let i = 0; i < 40; i++) m.add({ actor: 'runner', kind: 'run', status: i < 5 ? 'evidence-wait' : 'success', movedCells: 2,
    nativeOpportunity: true, plannedGameplay: true, plannedIntervalMs: 750, latencyMs: 100, plannedLatencyMs: 120 });
  const sample = m.actors.get('runner'), verdict = nativeActivityVerdict(sample, 'movement', 30, policy);
  assert.equal(sample.gaps, 0); assert.equal(sample.evidenceWaits, 5); assert.equal(sample.nativeCompleted, 35);
  assert.equal(verdict.completionRatio, .875); assert.equal(verdict.passed, false);
  assert.equal(nativeActivityVerdict({ ...sample, nativeCompleted: 40, nativeOpportunities: 40 }, 'movement', 30, policy).passed, true);
  assert.equal(nativeActivityVerdict({ ...sample, nativeCompleted: 40, runs: 0 }, 'movement', 30, policy).passed, false);
});

test('native login-window waiting cannot degrade to all Walk and recover its capacity verdict later', () => {
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native', movementIntervalMs: 750 });
  const gate = new ContinuousAcceptance(policy), windows = [];
  gate.emit = event => { if (event.type === 'continuousWindow') windows.push(event); };
  gate.join('runner', 'movement', 0);
  for (let i = 0; i < 80; i++) gate.add({ actor: 'runner', phase: 'auth-window-wait', kind: 'walk', status: 'success',
    plannedGameplay: true, nativeOpportunity: true, plannedIntervalMs: 750, movedCells: 1, latencyMs: 100, plannedLatencyMs: 120 }, i * 750 + 100);
  gate.sync([{ name: 'runner', active: true }], 60000);
  assert.equal(gate.failed, true); assert.equal(windows[0].nativeActivity.runRatio, 0);
  for (let i = 0; i < 80; i++) gate.add({ actor: 'runner', phase: 'stage-50', kind: 'run', status: 'success',
    plannedGameplay: true, nativeOpportunity: true, plannedIntervalMs: 750, movedCells: 2, latencyMs: 100, plannedLatencyMs: 120 }, 60100 + i * 750);
  gate.sync([{ name: 'runner', active: true }], 120000);
  assert.deepEqual(windows.map(window => window.passed), [false, true]); assert.equal(gate.summary().passed, false);
});

test('native continuous running starts with Walk, validates both Run cells, and resets after a long pause', () => {
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native' }), map = { width: 20, height: 20, blocked: new Uint8Array(400) };
  const context = { policy, map, runId: 'fixture', monsterNames: [], targetClaims: new Map(), clients: [] };
  const client = { snapshot: nativeSnapshot(), context, home: { x: 5, y: 5 }, role: 'movement', targetCursor: 0, movementCount: 0,
    nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity, plan: new NativePlan(0), nativeRunPrimedUntil: 0 };
  assert.equal(selectNativeAction(client, context, 0).command.type, 'walk');
  client.nativeRunPrimedUntil = Infinity;
  const run = selectNativeAction(client, context, 0); assert.equal(run.command.type, 'run'); assert.deepEqual(run.first, { x: 6, y: 5 });
  map.blocked[5 * 20 + 6] = 1;
  assert.notDeepEqual(selectNativeAction(client, context, 0)?.first, { x: 6, y: 5 });
  map.blocked[5 * 20 + 6] = 0; map.blocked[5 * 20 + 7] = 1;
  assert.notDeepEqual(selectNativeAction(client, context, 0)?.target, { x: 7, y: 5 });
  map.blocked[5 * 20 + 7] = 0; client.nativeRunPrimedUntil = 0;
  assert.equal(selectNativeAction(client, context, 0).command.type, 'walk');
});

test('explicit native anchors prove full-map reachability without widening automatic or baseline placement', () => {
  const map = { width: 300, height: 30, blocked: new Uint8Array(9000) }, origin = { x: 5, y: 5 };
  const anchors = [{ x: 210, y: 5 }, { x: 240, y: 5 }];
  assert.throws(() => chooseHomes(map, origin, 2, 'hotspot', [], anchors, 3), /reachable patrol anchors/);
  const proof = validateNativeExplicitHomes(map, origin, 2, anchors, 4);
  assert.deepEqual(proof.homes, anchors); assert.notEqual(proof.homes[0], anchors[0]);
  assert.equal(proof.footprintCells, 50); assert.ok(proof.visitedCells <= 9000);
  assert.deepEqual(chooseHomes(map, origin, 1, 'hotspot'), [origin], 'automatic selection keeps its old scope');
  for (const bad of [anchors.slice(0, 1), [{ x: 210.5, y: 5 }, anchors[1]], [{ x: 298, y: 5 }, anchors[1]], [anchors[0], { x: 214, y: 5 }]]) {
    assert.throws(() => validateNativeExplicitHomes(map, origin, 2, bad, 4));
  }
  assert.throws(() => validateNativeExplicitHomes(map, origin, 2, anchors, 3), /patrolSpan/);
  assert.throws(() => validateNativeExplicitHomes(map, origin, 2, anchors, 4, [], [{ x: 212, y: 7 }]), /footprint/);
  assert.throws(() => validateNativeExplicitHomes(map, origin, 2, anchors, 4, [{ bounds: { minX: 212, maxX: 212, minY: 7, maxY: 7 } }]), /footprint/);
  map.blocked[7 * map.width + 212] = 1;
  assert.throws(() => validateNativeExplicitHomes(map, origin, 2, anchors, 4), /footprint/);
  map.blocked[7 * map.width + 212] = 0;
  for (let y = 0; y < map.height; y++) map.blocked[y * map.width + 100] = 1;
  assert.throws(() => validateNativeExplicitHomes(map, origin, 2, anchors, 4), /unreachable/);
  map.blocked.fill(0);
  assert.throws(() => validateNativeExplicitHomes(map, origin, 2, anchors, 4, [{ bounds: { minX: 100, maxX: 100, minY: 0, maxY: 29 } }]), /unreachable/);
});

test('explicit wide native patrol uses two lawful steps toward a four-cell corner and preserves all collision checks', () => {
  const map = { width: 30, height: 30, blocked: new Uint8Array(900) }, policy = capacityPolicy({ profile: 'probe', activityMode: 'native' });
  const context = { map, policy, patrolSpan: 4 }, owner = { ...nativeSnapshot().entities[0], x: 5, y: 5 };
  const client = { snapshot: { ...nativeSnapshot(), entities: [owner] }, context, home: { x: 5, y: 5 }, role: 'movement',
    targetCursor: 0, movementCount: 0, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity, plan: new NativePlan(0), nativeRunPrimedUntil: 0 };
  assert.equal(selectNativeAction(client, context, 0).command.type, 'walk');
  client.nativeRunPrimedUntil = Infinity;
  const first = selectNativeAction(client, context, 0);
  assert.deepEqual(first.first, { x: 6, y: 5 }); assert.deepEqual(first.target, { x: 7, y: 5 });
  Object.assign(owner, first.target);
  const second = selectNativeAction(client, context, 0);
  assert.deepEqual(second.target, { x: 9, y: 5 }); assert.equal(second.command.type, 'run');
  const corners = new Set(); Object.assign(owner, { x: 5, y: 5 }); client.targetCursor = 0;
  for (let i = 0; i < 24; i++) {
    const step = selectNativeAction(client, context, 0); assert.equal(step.command.type, 'run');
    assert.equal(Math.max(Math.abs(step.target.x - owner.x), Math.abs(step.target.y - owner.y)), 2);
    for (const cell of [step.first, step.target]) assert.ok(cell.x >= 5 && cell.x <= 9 && cell.y >= 5 && cell.y <= 9);
    Object.assign(owner, step.target); corners.add(`${owner.x},${owner.y}`);
  }
  for (const point of ['5,5', '9,5', '9,9', '5,9']) assert.ok(corners.has(point));
  for (const block of [{ x: 6, y: 5 }, { x: 7, y: 5 }]) {
    Object.assign(owner, { x: 5, y: 5 }); client.targetCursor = 0;
    client.snapshot.entities = [owner, { objectId: 900, kind: 'monster', ...block }];
    const step = selectNativeAction(client, context, 0); assert.ok(step);
    for (const cell of [step.first, step.target]) assert.notDeepEqual(cell, block);
  }
  client.snapshot.entities = [owner]; client.targetCursor = 0;
  client.snapshot.mapTransfers = [{ bounds: { minX: 6, maxX: 6, minY: 5, maxY: 5 } }];
  assert.notDeepEqual(selectNativeAction(client, context, 0).first, { x: 6, y: 5 });
  client.snapshot.mapTransfers = []; map.blocked[5 * map.width + 6] = 1; client.targetCursor = 0;
  assert.notDeepEqual(selectNativeAction(client, context, 0).first, { x: 6, y: 5 });
  map.blocked.fill(1); map.blocked[5 * map.width + 5] = 0;
  assert.equal(selectNativeAction(client, context, 0), null, 'all blocked remains a counted navigation failure');
});

test('native combat clusters cover the exact cohort and contain hunting inside declared observer footprints', () => {
  const homes = [{ x: 5, y: 5 }, { x: 10, y: 10 }, { x: 21, y: 5 }], indices = new Set([1]);
  const definitions = [{ combatActorIndices: [1], movementActorIndices: [0, 2] }];
  const [cluster] = validateNativeCombatClusters(definitions, homes, indices, 4);
  assert.equal(insideCombatCoverage(cluster, { x: 35, y: 10 }), true);
  assert.equal(insideCombatCoverage(cluster, { x: 36, y: 10 }), false);
  for (const bad of [[], [{ combatActorIndices: [0], movementActorIndices: [1, 2] }], [{ combatActorIndices: [1], movementActorIndices: [0, 0, 2] }],
    [{ combatActorIndices: [1], movementActorIndices: [0] }], [{ combatActorIndices: [1], movementActorIndices: [] }]]) {
    assert.throws(() => validateNativeCombatClusters(bad, homes, indices, 4));
  }
  const map = { width: 70, height: 40, blocked: new Uint8Array(2800) }, policy = capacityPolicy({ profile: 'probe', activityMode: 'native', stages: '3', combatActors: 1 });
  const owner = { objectId: 1000, kind: 'player', x: 21, y: 10, direction: 'Right' };
  const hidden = { objectId: 900, kind: 'monster', name: 'Deer', x: 22, y: 10, hp: 25 };
  const visible = { objectId: 901, kind: 'monster', name: 'Deer', x: 24, y: 10, hp: 25 };
  const outside = { objectId: 902, kind: 'monster', name: 'Deer', x: 36, y: 10, hp: 25 };
  const peer = { label: 'observer', inGame: true, ready: true, role: 'movement', snapshot: { mapFileName: '0', playerObjectId: 50001,
    entities: [{ objectId: 50001, kind: 'player', x: 21, y: 5 }, visible, outside] } };
  cluster.observerNames.add(peer.label);
  cluster.observerBindings[1].name = peer.label;
  const context = { policy, map, patrolSpan: 4, clients: [peer], monsterNames: ['Deer'], targetClaims: new Map() };
  const client = { snapshot: { mapFileName: '0', playerObjectId: 1000, entities: [owner, hidden, visible, outside] }, context, home: homes[1],
    combatCluster: cluster, combatArrival: { at: 1 }, role: 'combat', failedTargets: new Map(), targetCursor: 0, movementCount: 0,
    nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity, nativeRunPrimedUntil: Infinity };
  assert.deepEqual(combatCandidates(client, context).map(entity => entity.objectId), [901, 900], 'prefer a target with a real observer and exclude the outside target');
  let step = selectNativeAction(client, context, 0); assert.ok(step); assert.equal(step.hunting, 'Deer');
  assert.ok(insideCombatCoverage(cluster, step.first)); assert.ok(insideCombatCoverage(cluster, step.target));
  client.snapshot.entities = [owner, outside];
  step = selectNativeAction(client, context, 0); assert.ok(step); assert.equal(step.hunting, undefined);
  assert.ok(insideCombatCoverage(cluster, step.first)); assert.ok(insideCombatCoverage(cluster, step.target));
  Object.assign(owner, { x: 50, y: 10 });
  step = selectNativeAction(client, context, 0); assert.ok(step); assert.equal(step.command.type, 'run'); assert.equal(step.target.x, 48);
  assert.equal(step.hunting, undefined, 'an externally pushed actor returns ordinarily instead of chasing outside coverage');
});

test('native patrol returns from the captured detour to a reachable declared corner when home is occupied', () => {
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native' }), map = { width: 300, height: 570, blocked: new Uint8Array(300 * 570) };
  const context = { policy, map, runId: 'captured-detour' }, owner = { ...nativeSnapshot().entities[0], x: 292, y: 560 };
  const obstacle = { objectId: 900, kind: 'monster', name: 'HookingCat', x: 289, y: 560 };
  const client = { snapshot: { ...nativeSnapshot(), entities: [owner, obstacle] }, context, home: { x: 289, y: 560 }, role: 'movement',
    targetCursor: 3, movementCount: 0, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity,
    plan: new NativePlan(0), nativeRunPrimedUntil: Infinity };
  // The old return-home-only branch is still the baseline comparator. The
  // live probe did not log its monster positions; this fixture proves the
  // captured position plus an occupied home, not that the live cause is known.
  const baseline = { ...context, policy: capacityPolicy({ profile: 'probe' }) };
  assert.equal(selectNativeAction(client, baseline, 0), null);
  const returned = selectNativeAction(client, context, 0);
  assert.deepEqual(returned.target, { x: 291, y: 560 }); assert.equal(client.targetCursor, 0);
  assert.equal(client.navigationAttempts[0].targetOccupied, true);
  assert.deepEqual(client.home, { x: 289, y: 560 }, 'recovery must not silently relocate the declared scene');
  Object.assign(owner, returned.target);
  for (let step = 0; step < 12; step++) {
    const plan = selectNativeAction(client, context, 0); assert.ok(plan);
    for (const cell of [plan.first, plan.target]) {
      assert.ok(cell.x >= 289 && cell.x <= 291 && cell.y >= 560 && cell.y <= 562);
      assert.equal(protocolMapCellIsWalkable(map, cell, [obstacle]), true);
    }
    Object.assign(owner, plan.target);
  }
  Object.assign(owner, { x: 292, y: 560 }); client.targetCursor = 3;
  map.blocked[560 * map.width + 291] = 1;
  const aroundWall = selectNativeAction(client, context, 0); assert.ok(aroundWall); assert.equal(client.targetCursor, 1);
  for (const cell of [aroundWall.first, aroundWall.target]) assert.equal(protocolMapCellIsWalkable(map, cell, [obstacle]), true);
  assert.equal(client.navigationAttempts.some(attempt => !attempt.staticWalkable), true);
  map.blocked[560 * map.width + 291] = 0; client.targetCursor = 3;
  client.snapshot.mapTransfers = [{ bounds: { minX: 291, maxX: 291, minY: 560, maxY: 560 } }];
  const aroundPortal = selectNativeAction(client, context, 0); assert.ok(aroundPortal); assert.equal(client.targetCursor, 1);
  assert.ok([aroundPortal.first, aroundPortal.target].every(cell => !(cell.x === 291 && cell.y === 560)));
  assert.equal(client.navigationAttempts.some(attempt => attempt.reason === 'next-step-transfer'), true);
});

test('primed native patrol chooses a lawful clear two-cell corner before an avoidable edge detour', () => {
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native' }), map = { width: 20, height: 20, blocked: new Uint8Array(400) };
  const context = { policy, map }, owner = { ...nativeSnapshot().entities[0], x: 7, y: 7 };
  const obstacle = { objectId: 900, kind: 'monster', x: 6, y: 7 };
  const client = { snapshot: { ...nativeSnapshot(), entities: [owner, obstacle] }, context, home: { x: 5, y: 5 }, role: 'movement',
    targetCursor: 2, movementCount: 0, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity,
    plan: new NativePlan(0), nativeRunPrimedUntil: Infinity };
  const baseline = { ...context, policy: capacityPolicy({ profile: 'probe' }) };
  assert.equal(selectNativeAction(client, baseline, 0).command.type, 'walk');
  client.targetCursor = 2;
  const direct = selectNativeAction(client, context, 0);
  assert.equal(direct.command.type, 'run'); assert.deepEqual(direct.target, { x: 7, y: 5 });
  assert.deepEqual(findProtocolWalkPath({ map, start: owner, target: direct.target, dynamicObstacles: [obstacle] }),
    [{ x: 7, y: 7 }, direct.first, direct.target], 'fast preference still describes the ordinary collision-aware path');
  client.nativeRunPrimedUntil = 0; client.targetCursor = 2;
  assert.equal(selectNativeAction(client, context, 0).command.type, 'walk', 'standing still does not gain a forced Run');

  client.nativeRunPrimedUntil = Infinity;
  for (const cell of [{ x: 6, y: 7 }, { x: 7, y: 6 }, { x: 5, y: 6 }, { x: 6, y: 5 }, { x: 6, y: 6 }, { x: 5, y: 5 }]) {
    Object.assign(obstacle, cell); client.targetCursor = 3;
    const plan = selectNativeAction(client, context, 0); assert.ok(plan);
    assert.notDeepEqual(plan.target, { x: 5, y: 5 }, 'neither diagonal side pair, the intermediate cell nor the destination can be bypassed');
    for (const step of [plan.first, plan.target]) assert.equal(protocolMapCellIsWalkable(map, step, [obstacle]), true);
  }
  client.snapshot.entities = [owner]; client.targetCursor = 3;
  map.blocked[6 * map.width + 6] = 1;
  assert.notDeepEqual(selectNativeAction(client, context, 0).target, { x: 5, y: 5 });
  map.blocked[6 * map.width + 6] = 0; client.targetCursor = 3;
  client.snapshot.mapTransfers = [{ bounds: { minX: 6, maxX: 6, minY: 6, maxY: 6 } }];
  assert.notDeepEqual(selectNativeAction(client, context, 0).target, { x: 5, y: 5 }, 'intermediate transfer must not be entered');
  client.snapshot.mapTransfers = [{ bounds: { minX: 5, maxX: 5, minY: 5, maxY: 5 } }]; client.targetCursor = 3;
  assert.notDeepEqual(selectNativeAction(client, context, 0).target, { x: 5, y: 5 }, 'destination transfer must not be entered');
});

test('native navigation diagnostics retain the first blocked streak with bounded occupants while every failed opportunity still fails load gates', async () => {
  const rows = [], actions = [], measurements = new Measurements();
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native' }), map = { width: 20, height: 20, blocked: new Uint8Array(400) };
  const context = { policy, map, mapName: '0', runId: 'blocked-fixture', phase: 'stage-2', pool: pool('ws://127.0.0.1:1/ws', 2),
    clients: [], writer: { add: row => rows.push(row) }, add: row => { actions.push(row); measurements.add(row); },
    guard: { assert() {}, stop(reason) { throw new Error(reason); } } };
  const client = new CapacityClient(context.pool.endpoint, actor(0), context);
  const owner = { ...nativeSnapshot().entities[0], name: client.label, x: 8, y: 5 };
  const blockers = [[7, 5], [7, 7], [5, 7], [5, 5]].map(([x, y], index) => ({ objectId: 900 + index, x, y,
    kind: index === 3 ? 'player' : 'monster', name: index === 3 ? 'private-non-cohort-player' : 'Hen',
    password: 'do-not-copy-password', credential: 'do-not-copy-ticket', inventory: ['private-inventory'] }));
  const nearby = Array.from({ length: 80 }, (_, index) => ({ objectId: 10000 + index, kind: 'monster', name: 'Deer', x: 9, y: 6 }));
  client.snapshot = { ...nativeSnapshot(), entities: [owner, ...blockers, ...nearby,
    { objectId: 88888, kind: 'monster', name: 'ignored-dead', x: 8, y: 6, dead: true }] };
  Object.assign(client, { ready: true, inGame: true, home: { x: 5, y: 5 }, role: 'movement', nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity });
  const offer = async () => {
    client.plan = new NativePlan(performance.now()); offerNativeAction(client, context, performance.now()); await client.pending;
  };
  for (let index = 0; index < 20; index++) await offer();
  assert.equal(actions.length, 20); assert.ok(actions.every(row => row.status === 'navigation-blocked' && row.nativeOpportunity));
  assert.equal(nativeActivityVerdict(measurements.actors.get(client.label), 'movement', 30, policy).passed, false);
  assert.equal(rows.length, 1); assert.equal(client.navigationBlockedRows, 1);
  const diagnostic = rows[0]; assert.equal(diagnostic.type, 'navigationBlocked');
  assert.deepEqual(diagnostic.self, { x: 8, y: 5 }); assert.deepEqual(diagnostic.home, { x: 5, y: 5 });
  assert.equal(diagnostic.maxExpanded, 4000); assert.equal(diagnostic.attemptCount, 4);
  assert.ok(diagnostic.attempts.every(attempt => attempt.reason === 'no-path-or-expansion-limit' && attempt.targetOccupied));
  assert.equal(diagnostic.nearbyOccupantCount, 84); assert.equal(diagnostic.nearbyOccupants.length, 32);
  assert.equal(diagnostic.occupantsTruncated, true); assert.equal(diagnostic.attemptsTruncated, false);
  assert.doesNotMatch(JSON.stringify(diagnostic), /private-non-cohort|do-not-copy|private-inventory|ignored-dead/);

  // A turn or chat while stationary must not restart the diagnostic streak.
  let planned;
  client.send = command => {
    if (['walk', 'run'].includes(command.type)) Object.assign(owner, planned.target);
    owner.direction = command.direction;
    client.events.push({ direction: 'received', type: 'packet', packet: 'UserLocation', sequence: ++client.sequence,
      monotonicMs: performance.now(), payload: { x: owner.x, y: owner.y, direction: owner.direction } });
  };
  client.nextNativeTurnAt = 0; await offer();
  assert.equal(actions.at(-1).kind, 'turn'); assert.equal(actions.at(-1).status, 'success'); assert.equal(client.navigationBlocked, true);
  await offer(); assert.equal(actions.at(-1).status, 'navigation-blocked'); assert.equal(rows.length, 1);

  client.snapshot.entities = [owner]; planned = selectNativeAction(client, context, performance.now());
  await offer(); assert.equal(actions.at(-1).status, 'success'); assert.equal(client.navigationBlocked, false);
  Object.assign(owner, { x: 8, y: 5 }); client.snapshot.entities = [owner, ...blockers];
  await offer(); assert.equal(actions.at(-1).status, 'navigation-blocked'); assert.equal(rows.length, 2);
  assert.equal(client.navigationBlockedRows, 2);
});

test('native corrected Walk detours around the refused first cell until expiry without hiding the correction', async () => {
  const rows = [], actions = [], measurements = new Measurements();
  const context = { policy: capacityPolicy({ profile: 'probe', activityMode: 'native' }),
    map: { width: 80, height: 30, blocked: new Uint8Array(2400) }, mapName: '0', patrolSpan: 4,
    phase: 'stage-2', clients: [], writer: { add: row => rows.push(row) },
    add: row => { actions.push(row); measurements.add(row); }, guard: { assert() {}, stop(reason) { throw new Error(reason); } } };
  const client = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), context);
  const owner = { ...nativeSnapshot().entities[0], name: client.label, x: 8, y: 5 };
  client.snapshot = { ...nativeSnapshot(), entities: [owner] };
  Object.assign(client, { ready: true, inGame: true, home: { x: 5, y: 5 }, role: 'movement', targetCursor: 3, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity });
  client.send = command => client.events.push({ direction: 'received', type: 'packet', packet: 'UserLocation',
    sequence: ++client.sequence, monotonicMs: performance.now(), payload: { x: owner.x, y: owner.y, direction: command.direction } });
  const offer = async () => { client.plan = new NativePlan(performance.now()); offerNativeAction(client, context); await client.pending; };
  assert.deepEqual(selectNativeAction(client, context).first, { x: 7, y: 5 });
  await offer();
  assert.equal(actions.length, 1); assert.equal(actions[0].status, 'corrected');
  assert.equal(measurements.actors.get(client.label).nativeCompleted, 0);
  const detour = selectNativeAction(client, context);
  assert.notDeepEqual(detour.first, { x: 7, y: 5 });
  assert.ok(detour.target.x >= 5 && detour.target.x <= 9 && detour.target.y >= 5 && detour.target.y <= 9);
  assert.equal(client.rejectedWalkCells.size, 1);
  const rejected = [...client.rejectedWalkCells.values()][0];
  assert.ok(rejected.expiresAt > performance.now() && rejected.expiresAt <= performance.now() + 3000);
  client.targetCursor = 3; rejected.expiresAt = 0;
  assert.deepEqual(selectNativeAction(client, context).first, { x: 7, y: 5 }, 'a transient refused cell is retried after expiry');
  assert.equal(client.rejectedWalkCells.size, 0);
  assert.equal(rows.filter(row => row.type === 'movementCorrection').length, 1);
});

test('native correction avoidance never guesses a Run blocker and keeps a bounded private-safe diagnostic', async () => {
  const rows = [], actions = [];
  const context = { policy: capacityPolicy({ profile: 'probe', activityMode: 'native' }),
    map: { width: 80, height: 30, blocked: new Uint8Array(2400) }, mapName: '0', patrolSpan: 4,
    phase: 'stage-2', clients: [], writer: { add: row => rows.push(row) }, add: row => actions.push(row),
    guard: { assert() {}, stop(reason) { throw new Error(reason); } } };
  const client = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), context);
  const owner = { ...nativeSnapshot().entities[0], name: client.label, x: 8, y: 5 };
  client.snapshot = { ...nativeSnapshot(), entities: [owner, ...Array.from({ length: 80 }, (_, index) => ({
    objectId: 900 + index, kind: 'monster', name: 'never-copy-name', x: 10, y: 7, dead: Boolean(index % 2),
    password: 'private-password', inventory: ['private-inventory'] }))] };
  Object.assign(client, { ready: true, inGame: true, home: { x: 5, y: 5 }, role: 'movement', targetCursor: 3, nativeRunPrimedUntil: Infinity,
    nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity });
  client.send = command => client.events.push({ direction: 'received', type: 'packet', packet: 'UserLocation',
    sequence: ++client.sequence, monotonicMs: performance.now(), payload: { x: owner.x, y: owner.y, direction: command.direction } });
  const offer = async () => { client.plan = new NativePlan(performance.now()); offerNativeAction(client, context); await client.pending; };
  await offer(); assert.equal(actions[0].kind, 'run'); assert.equal(actions[0].status, 'corrected');
  assert.equal(client.rejectedWalkCells.size, 0, 'either Run cell could be occupied; do not invent a blocker');
  const diagnostic = rows.find(row => row.type === 'movementCorrection');
  assert.equal(diagnostic.nearbyOccupants.length, 32); assert.equal(diagnostic.nearbyOccupantCount, 80);
  assert.ok(diagnostic.nearbyOccupants.some(entity => entity.dead));
  assert.doesNotMatch(JSON.stringify(diagnostic), /never-copy-name|private-password|private-inventory/);
  client.snapshot.entities = [owner]; client.nativeRunPrimedUntil = 0;
  for (let index = 0; index < 20; index++) {
    owner.x = 8 + index; client.home.x = owner.x - 3; client.targetCursor = 3;
    await offer(); assert.ok(client.rejectedWalkCells.size <= 16);
  }
  assert.equal(client.rejectedWalkCells.size, 16);
  assert.equal(rows.filter(row => row.type === 'movementCorrection').length, 1, 'one diagnostic per uninterrupted correction streak');
  assert.equal(actions.filter(row => row.status === 'corrected').length, 21, 'every correction remains in the original action denominator');
});

test('real Bichon collision map and BorderVillage NPCs produce a full four-corner native Run patrol', async () => {
  const map = await loadProtocolCollisionMap('0'), npcs = await loadStationaryNpcObstacles('0');
  const origin = { x: 285, y: 617 }, board = npcs.find(npc => npc.name === 'BorderVillage_Board');
  assert.deepEqual(board, { x: 284, y: 615, name: 'BorderVillage_Board' });
  assert.equal(protocolMapCellIsWalkable(map, board), true, 'a walkable map tile does not exclude an occupying authored NPC');
  const oldHomes = chooseHomes(map, origin, 2, 'dispersed', [], [], 3);
  assert.deepEqual(oldHomes[0], { x: 282, y: 614 }, 'reproduce the actual failed public probe anchor');
  const homes = chooseHomes(map, origin, 2, 'dispersed', [], [], 3, npcs);
  for (const home of homes) assert.equal(npcs.some(npc => npc.x >= home.x && npc.x <= home.x + 2 && npc.y >= home.y && npc.y <= home.y + 2), false);
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native' }), context = { policy, map, runId: 'real-map-fixture' };
  const owner = { objectId: 1000, kind: 'player', name: 'runner', ...origin, direction: 'Up', level: 1 };
  const client = { snapshot: { mapFileName: '0', playerObjectId: 1000, entities: [owner, ...npcs.map((npc, i) => ({ ...npc, kind: 'npc', objectId: 20000 + i }))] },
    context, home: homes[0], role: 'movement', targetCursor: 0, movementCount: 0, nextNativeChatAt: Infinity,
    nextNativeTurnAt: Infinity, plan: new NativePlan(0), nativeRunPrimedUntil: 0 };
  const commands = [], corners = new Set();
  for (let i = 0; i < 40; i++) {
    const plan = selectNativeAction(client, context, 0); assert.ok(plan);
    for (const cell of [plan.first, plan.target]) {
      assert.equal(protocolMapCellIsWalkable(map, cell), true);
      assert.equal(npcs.some(npc => npc.x === cell.x && npc.y === cell.y), false);
    }
    commands.push(plan.command.type); Object.assign(owner, plan.target); client.movementCount++; client.nativeRunPrimedUntil = performance.now() + 1200;
    if (i > 5) corners.add(`${owner.x},${owner.y}`);
  }
  assert.equal(commands[0], 'walk'); assert.ok(commands.slice(5).every(kind => kind === 'run'));
  assert.deepEqual(corners, new Set([[0, 0], [2, 0], [2, 2], [0, 2]].map(([dx, dy]) => `${homes[0].x + dx},${homes[0].y + dy}`)));
});

test('native combat walks the real village-to-forest route before hunting, without requiring an observer to arrive', async () => {
  const map = await loadProtocolCollisionMap('0'), npcs = await loadStationaryNpcObstacles('0'), evidence = [];
  const manifest = JSON.parse(await fs.readFile(new URL('../lib/generated/crystal_respawn_manifest.json', import.meta.url), 'utf8'));
  const transfers = manifest.maps.find(entry => entry.map_file_name === '0').movements.map(move => ({ bounds: {
    minX: move.source.x, maxX: move.source.x, minY: move.source.y, maxY: move.source.y } }));
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native', stages: '2', combatActors: 1 });
  const context = { policy, map, runId: 'real-combat-transit', phase: 'settle', monsterNames: ['Hen', 'Deer'], targetClaims: new Map(),
    clients: [], writer: { add: row => evidence.push(row) } };
  const owner = { objectId: 1000, kind: 'player', name: 'fighter', x: 276, y: 612, direction: 'Right', level: 1 };
  const hen = { objectId: 900, kind: 'monster', name: 'Hen', x: 277, y: 612, hp: 5 };
  const client = { label: 'fighter', snapshot: { mapFileName: '0', playerObjectId: 1000, mapTransfers: transfers,
    entities: [owner, hen, ...npcs.map((npc, index) => ({ ...npc, kind: 'npc', objectId: 20000 + index }))] },
    context, home: { x: 275, y: 556 }, role: 'combat', ready: true, inGame: true, failedTargets: new Map(),
    targetCursor: 0, movementCount: 0, sequence: 1, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity,
    plan: new NativePlan(0), nativeRunPrimedUntil: 0 };
  context.clients.push(client);
  assert.equal(actorsReachedScenario(context), false, 'a combat-only cohort cannot settle at its village login location');
  const commands = [];
  for (let step = 0; step < 100 && !client.combatArrival; step++) {
    const plan = selectNativeAction(client, context, 0);
    if (client.combatArrival) break;
    assert.ok(plan); assert.equal(plan.initialCombatTransit, true); assert.ok(['walk', 'run'].includes(plan.command.type));
    for (const cell of [plan.first, plan.target]) {
      assert.equal(protocolMapCellIsWalkable(map, cell), true);
      assert.equal(npcs.some(npc => npc.x === cell.x && npc.y === cell.y), false);
      assert.equal(transfers.some(transfer => transfer.bounds.minX === cell.x && transfer.bounds.minY === cell.y), false);
    }
    commands.push(plan.command.type); Object.assign(owner, plan.target); client.sequence++;
    client.movementCount++; client.nativeRunPrimedUntil = performance.now() + 1200;
  }
  assert.deepEqual(client.combatArrival?.actual, client.home);
  assert.equal(actorsReachedScenario(context), true); assert.equal(commands[0], 'walk'); assert.ok(commands.includes('run'));
  assert.ok(commands.length > 20, 'the declared forest requires actual travel, not nearby village combat');
  assert.equal(evidence.filter(row => row.type === 'combatArrival').length, 1);

  // Arrival is a one-time requirement. After a chase, a reachable adjacent
  // monster with a real observer must not force the fighter back home again.
  Object.assign(owner, { x: 280, y: 557, direction: 'Right' }); Object.assign(hen, { x: 281, y: 557 });
  const peer = { label: 'peer', inGame: true, snapshot: { mapFileName: '0', playerObjectId: 1000, entities: [
    { objectId: 1000, kind: 'player', name: 'peer', x: 279, y: 557 }, { ...owner, objectId: 50001 }, { ...hen }] } };
  context.clients.push(peer);
  assert.equal(selectNativeAction(client, context, 0).command.type, 'attack');
  assert.equal(evidence.filter(row => row.type === 'combatArrival').length, 1); assert.equal(actorsReachedScenario(context), true);

  const baseline = { ...context, policy: capacityPolicy({ profile: 'probe', stages: '2', combatActors: 1 }) };
  client.combatArrival = undefined; client.home = { x: 275, y: 556 };
  assert.equal(selectNativeAction(client, baseline, 0).command.type, 'attack', 'baseline hunting remains unchanged');
  assert.equal(client.combatArrival, undefined); assert.equal(actorsReachedScenario(baseline), true);
});

test('native pending evidence is recorded separately and never sends another attack into an ambiguous hit window', () => {
  const rows = [], policy = capacityPolicy({ profile: 'probe', activityMode: 'native', stages: '2', combatActors: 1 });
  const context = { policy, map: { width: 20, height: 20, blocked: new Uint8Array(400) }, runId: 'fixture',
    clients: [], monsterNames: ['Hen'], targetClaims: new Map(), add: row => rows.push(row), guard: { stop() {} } };
  const client = { label: 'fighter', snapshot: nativeSnapshot(), context, role: 'combat', home: { x: 5, y: 5 }, failedTargets: new Map(),
    nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity, plan: new NativePlan(0), pending: Promise.resolve(), waitReason: 'evidence-wait', evidencePending: new Set() };
  client.snapshot.entities.push({ objectId: 900, kind: 'monster', name: 'Hen', x: 6, y: 5, hp: 5 });
  const peer = observer('peer', 5, [entity('fighter', 5, 50001), { objectId: 900, name: 'Hen', kind: 'monster', x: 6, y: 5, hp: 5 }]);
  context.clients = [client, peer]; client.inGame = true;
  offerNativeAction(client, context, 0);
  assert.equal(rows.length, 1); assert.equal(rows[0].status, 'evidence-wait'); assert.equal(rows[0].kind, 'attack');
  assert.equal(rows[0].plannedIntervalMs, 1436); assert.equal(rows[0].nativeOpportunity, true);
});

test('histograms preserve a real latency tail without retaining per-action samples', () => {
  const histogram = new Histogram(), bytes = histogram.buckets.byteLength;
  for (let i = 0; i < 99000; i++) histogram.add(12.25);
  for (let i = 0; i < 1001; i++) histogram.add(900.1);
  assert.equal(histogram.quantile(.95), 13); assert.equal(histogram.quantile(.99), 901);
  assert.equal(histogram.buckets.byteLength, bytes); assert.equal(histogram.count, 100001);
  histogram.add(80000); assert.equal(histogram.quantile(1), 80000);
});

function ordinaryMovement(measurements, name, count = 60) {
  for (let i = 0; i < count; i++) measurements.add({ actor: name, kind: 'walk', status: 'success', latencyMs: 100,
    plannedLatencyMs: 120, plannedGameplay: true, at: i * 1000 });
}
test('idle, correction-only, omitted planned load and swing-only actors cannot pass as sustained players', () => {
  const p = capacityPolicy({ profile: 'probe', stages: '2' }), actors = [{ name: 'mover', role: 'movement' }];
  const good = new Measurements(); ordinaryMovement(good, 'mover');
  assert.equal(assessMeasurements(good, actors, 60, p, { requireChat: false }).passed, true);
  const idle = new Measurements(); for (let i = 0; i < 60; i++) idle.add({ actor: 'mover', kind: 'rtt', status: 'success', latencyMs: 5 });
  assert.equal(assessMeasurements(idle, actors, 60, p, { requireChat: false }).passed, false);
  const corrections = new Measurements(); for (let i = 0; i < 60; i++) corrections.add({ actor: 'mover', kind: 'walk', status: 'corrected', latencyMs: 5, plannedLatencyMs: 5, plannedGameplay: true });
  assert.equal(assessMeasurements(corrections, actors, 60, p, { requireChat: false }).passed, false);
  const omitted = new Measurements(); ordinaryMovement(omitted, 'mover', 50);
  for (let i = 0; i < 10; i++) omitted.add({ actor: 'mover', kind: 'walk', status: 'plan-gap', plannedGameplay: true });
  assert.equal(assessMeasurements(omitted, actors, 60, p, { requireChat: false }).passed, false);
  const swingOnly = new Measurements(); ordinaryMovement(swingOnly, 'mover');
  for (let i = 0; i < 60; i++) swingOnly.add({ actor: 'fighter', kind: 'attack', status: 'miss', plannedGameplay: true, latencyMs: 2, plannedLatencyMs: 2 });
  const verdict = assessMeasurements(swingOnly, [...actors, { name: 'fighter', role: 'combat' }], 60, p, { requireChat: false });
  assert.equal(verdict.passed, false); assert.equal(verdict.combatCoverage, 'scenario-insufficient');
});

test('each declared combat actor needs positive damage with shared health and a sustained attack rate', () => {
  const m = new Measurements(), p = capacityPolicy({ profile: 'probe', stages: '2', combatActors: 1 });
  ordinaryMovement(m, 'mover');
  for (let i = 0; i < 60; i++) m.add({ actor: 'fighter', kind: 'attack', status: 'success', damage: 8,
    sharedHealthVerified: false, plannedGameplay: true, latencyMs: 100, plannedLatencyMs: 120 });
  const actors = [{ name: 'mover', role: 'movement' }, { name: 'fighter', role: 'combat' }];
  assert.equal(assessMeasurements(m, actors, 60, p, { requireChat: false }).combatCoverage, 'scenario-insufficient');
  for (let i = 0; i < 3; i++) m.add({ actor: 'fighter', kind: 'attack', status: 'success', damage: 8,
    sharedHealthVerified: true, plannedGameplay: true, latencyMs: 100, plannedLatencyMs: 120 });
  assert.equal(assessMeasurements(m, actors, 60, p, { requireChat: false }).combatCoverage, 'shared-positive-damage');
});

const movementSample = (name, phase = 'ramp', status = 'success') => ({ actor: name, kind: 'walk', phase, status,
  plannedGameplay: true, latencyMs: 100, plannedLatencyMs: 120, at: 0 });

test('ramp and auth-window activity failures remain latched after later measured stages recover', () => {
  const windows = [], gate = new ContinuousAcceptance(capacityPolicy({ profile: 'probe' }), { emit: event => windows.push(event) });
  gate.join('mover', 'movement', 0);
  // A whole minute of missing ACKs while waiting for the next auth window.
  for (let i = 0; i < 60; i++) gate.add(movementSample('mover', i < 30 ? 'ramp' : 'auth-window-wait', 'timeout'), i * 1000 + 100);
  gate.sync([{ name: 'mover', active: true }], 60000);
  assert.equal(gate.failed, true); assert.equal(gate.summary().firstFailure.evidence.timeoutRatio, 1);
  for (let i = 0; i < 60; i++) gate.add(movementSample('mover', 'stage-50'), 60100 + i * 1000);
  gate.sync([{ name: 'mover', active: true }], 120000);
  const assessed = windows.filter(event => event.type === 'continuousWindow');
  assert.deepEqual(assessed.map(event => event.passed), [false, true]);
  assert.equal(gate.summary().passed, false);
  assert.deepEqual(assessed[0].phases, ['ramp', 'auth-window-wait']);
});

test('native resume pauses only its selected owner while peers retain activity obligations', () => {
  const p = capacityPolicy({ profile: 'probe', stages: '2' }), gate = new ContinuousAcceptance(p);
  for (const name of ['resuming', 'peer']) gate.join(name, 'movement', 0);
  for (let i = 0; i < 30; i++) for (const name of ['resuming', 'peer']) gate.add(movementSample(name), i * 1000 + 100);
  gate.pause('resuming', 30000);
  for (let i = 0; i < 60; i++) gate.add(movementSample('peer', 'native-resume-under-load'), 30100 + i * 1000);
  gate.sync([{ name: 'resuming', active: true }, { name: 'peer', active: true }], 90000);
  for (let i = 0; i < 30; i++) for (const name of ['resuming', 'peer']) gate.add(movementSample(name, 'post-resume-observation'), 90100 + i * 1000);
  // Stop all offered load at one common time, then simulate a long batched drain.
  for (const name of ['resuming', 'peer']) gate.pause(name, 120000);
  gate.finish(240000);
  const summary = gate.summary(); assert.equal(summary.passed, true);
  assert.equal(summary.actors.find(actor => actor.name === 'resuming').activeSeconds, 60);
  assert.equal(summary.actors.find(actor => actor.name === 'peer').activeSeconds, 120);

  const stalled = new ContinuousAcceptance(p);
  stalled.join('resuming', 'movement', 0); stalled.join('peer', 'movement', 0); stalled.pause('resuming', 0);
  stalled.sync([{ name: 'resuming', active: false }, { name: 'peer', active: true }], 60000);
  assert.equal(stalled.summary().passed, false);
  assert.equal(stalled.summary().firstFailure.evidence.actor, 'peer');
  assert.equal(stalled.summary().firstFailure.evidence.activityPassed, false);
});

test('planned disconnect waits existing observer obligations while newly offered peer actions keep running', async () => {
  let finishOld, finishNew, drained = false;
  const peer = { pending: new Promise(resolve => { finishOld = resolve; }) };
  const barrier = awaitPendingActions([peer]).then(() => { drained = true; });
  peer.pending = new Promise(resolve => { finishNew = resolve; });
  await Promise.resolve(); assert.equal(drained, false);
  finishOld(); await barrier; assert.equal(drained, true);
  // The newly offered action is still pending: the cohort was not stopped.
  let newFinished = false; peer.pending.then(() => { newFinished = true; });
  await Promise.resolve(); assert.equal(newFinished, false);
  finishNew(); await peer.pending;
});

test('drain includes observer evidence created by the captured owner acknowledgement', async () => {
  let acknowledge, observe, drained = false;
  const client = { evidencePending: new Set() };
  client.pending = new Promise(resolve => { acknowledge = resolve; }).then(() => {
    const evidence = new Promise(resolve => { observe = resolve; }); client.evidencePending.add(evidence);
    return undefined;
  });
  const barrier = awaitPendingActions([client]).then(() => { drained = true; });
  acknowledge(); await client.pending; await Promise.resolve(); assert.equal(drained, false);
  observe(); await barrier; assert.equal(drained, true);
});

test('short final participation still retains real rejection, correction and latency failures', () => {
  for (const sample of [movementSample('short', 'ramp', 'rejected'), movementSample('short', 'ramp', 'corrected'),
    { ...movementSample('short', 'native-resume-under-load'), latencyMs: 2000, plannedLatencyMs: 2020 }]) {
    const gate = new ContinuousAcceptance(capacityPolicy({ profile: 'probe' })); gate.join('short', 'movement', 0);
    gate.add(sample, 1000); gate.pause('short', 2000); gate.finish(50000);
    assert.equal(gate.summary().passed, false); assert.equal(gate.summary().actors[0].activeSeconds, 2);
  }
});

const entity = (name, x, id) => ({ name, x, y: 5, objectId: id, kind: 'player' });
function observer(name, x, entries = []) { return { label: name, inGame: true, snapshot: { mapFileName: '0', playerObjectId: 1000, entities: [entity(name, x, 1000), ...entries] } }; }
test('AOI denominator includes a player whose ObjectPlayer was never delivered, using actual 16-cell geometry', () => {
  const a = observer('a', 0), b = observer('b', 16, [entity('a', 0, 50001), entity('c', 17, 50003)]), c = observer('c', 17, [entity('b', 16, 50002)]);
  const coverage = new AoiCoverage(), first = coverage.sample([a, b, c], 100);
  assert.equal(first.expectedPairs, 4); assert.equal(first.observedPairs, 3);
  coverage.sample([a, b, c], 1200); assert.equal(coverage.stats.persistentMissing, 1);
  a.snapshot.entities.push(entity('b', 16, 50002)); coverage.sample([a, b, c], 1300);
  c.snapshot.entities.push(entity('a', 0, 50001)); coverage.sample([a, b, c], 1400); coverage.sample([a, b, c], 2501);
  assert.equal(coverage.stats.ghosts, 1);
  c.snapshot.entities.push(entity('b', 16, 59999)); coverage.sample([a, b, c], 2600); assert.equal(coverage.stats.duplicates, 1);
});

test('native-resume AOI grace restarts for planned absence but duplicates and later ghosts stay failed', () => {
  const a = observer('a', 0), b = observer('b', 10, [entity('a', 0, 50001)]), coverage = new AoiCoverage();
  coverage.sample([a, b], 0); b.paused = true; coverage.sample([a, b], 500);
  b.paused = false; assert.equal(coverage.sample([a, b], 2000).issues.length, 0);
  a.snapshot.entities.push(entity('b', 10, 50002)); assert.equal(coverage.sample([a, b], 2500).issues.length, 0);
  b.paused = true; a.snapshot.entities.push(entity('b', 10, 59999));
  const duplicate = coverage.sample([a, b], 3000);
  assert.equal(duplicate.issues[0].reason, 'duplicate');
  const gate = new ContinuousAcceptance(capacityPolicy({ profile: 'probe' }));
  gate.observeAoi(duplicate, 'native-resume-under-load');
  coverage.reset(); a.snapshot.entities = [entity('a', 0, 1000)];
  gate.observeAoi(coverage.sample([a, b], 3500), 'post-resume-observation');
  assert.equal(gate.summary().passed, false); assert.equal(gate.summary().firstFailure.evidence.phase, 'native-resume-under-load');

  const ghostGate = new ContinuousAcceptance(capacityPolicy({ profile: 'probe' }));
  b.paused = false; b.snapshot.entities[0].x = 20; a.snapshot.entities.push(entity('b', 20, 50002));
  coverage.sample([a, b], 4000); ghostGate.observeAoi(coverage.sample([a, b], 5101), 'native-resume-under-load');
  assert.equal(ghostGate.summary().passed, false);
  assert.ok(ghostGate.summary().firstFailure.evidence.issues.some(issue => issue.reason === 'ghost'));
});

test('fanout latency geometry accounts for simultaneous runs without shrinking the complete AOI denominator', () => {
  assert.equal(stableFanoutRange('run'), 12); assert.equal(stableFanoutRange('walk'), 12); assert.equal(stableFanoutRange('chat'), 14);
  const a = observer('a', 0, [entity('b', 14, 50002)]), b = observer('b', 14, [entity('a', 0, 50001)]);
  assert.equal(new AoiCoverage().sample([a, b], 0).expectedPairs, 2);
  assert.equal(14 <= stableFanoutRange('run'), false); assert.equal(12 + 2 + 2 <= 16, true);
  assert.equal(14 + 2 <= 16, true); assert.equal(16 <= stableFanoutRange('chat'), false);
});

function hit(attacker, damage = 5, health = 75, order = 'damage-first') {
  const a = { packet: 'DamageIndicator', payload: { objectId: 900, damage }, monotonicMs: 100, sequence: 11 };
  const b = { packet: 'ObjectStruck', payload: { objectId: 900, attackerId: attacker }, monotonicMs: 105, sequence: 12 };
  if (order !== 'damage-first') { a.sequence = 12; b.sequence = 11; }
  return [a, b, { packet: 'ObjectHealth', payload: { objectId: 900, percent: health }, monotonicMs: 110, sequence: 13 }];
}
test('real hit correlation accepts either packet order, rejects another attacker, zero damage and divergent shared HP', () => {
  assert.equal(verifiedCombatHit(hit(1000), hit(50001, 5, 75, 'strike-first'), 900, 1000, 50001).damage, 5);
  assert.equal(verifiedCombatHit(hit(1000, 0), hit(50001, 0), 900, 1000, 50001), null);
  assert.equal(verifiedCombatHit(hit(1000), hit(50001, 5, 70), 900, 1000, 50001), null);
  assert.equal(verifiedCombatHit([...hit(1000), { packet: 'ObjectStruck', payload: { objectId: 900, attackerId: 888 } }], hit(50001), 900, 1000, 50001), null);
  assert.equal(verifiedCombatHit([{ packet: 'ObjectAttack', payload: { objectId: 1000 } }], [], 900, 1000, 50001), null);
});

test('shared combat requires this strike\'s finite post-damage HP, never old AOI health or unrelated death', () => {
  const oldHealth = { packet: 'ObjectHealth', payload: { objectId: 900, percent: 100 }, sequence: 1, monotonicMs: 1 };
  const stale = attacker => [oldHealth, ...hit(attacker).slice(0, 2)];
  assert.equal(verifiedCombatHit(stale(1000), stale(50001), 900, 1000, 50001), null);
  const death = { packet: 'ObjectDied', payload: { objectId: 900 }, sequence: 14, monotonicMs: 120 };
  assert.equal(verifiedCombatHit([...stale(1000), death], [...stale(50001), death], 900, 1000, 50001), null);
  for (const health of [NaN, Infinity, -1, 101, undefined]) {
    const own = hit(1000), peer = hit(50001); own[2].payload.percent = health; peer[2].payload.percent = health;
    assert.equal(verifiedCombatHit(own, peer, 900, 1000, 50001), null);
  }
  for (const patch of [{ sequence: 30 }, { monotonicMs: 300 }, { sequence: 10 }, { monotonicMs: 50 }]) {
    const own = hit(1000), peer = hit(50001); Object.assign(own[2], patch); Object.assign(peer[2], patch);
    assert.equal(verifiedCombatHit(own, peer, 900, 1000, 50001), null);
  }
  assert.equal(verifiedCombatHit([oldHealth, ...hit(1000, 5, 0, 'strike-first'), death], [oldHealth, ...hit(50001, 5, 0, 'strike-first'), death], 900, 1000, 50001).healthPercent, 0);
});

test('a real rejection is not downgraded to a miss merely because a swing packet was also delivered', () => {
  const rejected = new Error('Public gameplay command rejected'); rejected.code = 'PUBLIC_COMMAND_REJECTED';
  assert.equal(actionFailureStatus(rejected, true, false), 'rejected');
  assert.equal(actionFailureStatus(new Error('timed out'), true, false), 'miss');
  assert.equal(actionFailureStatus(new Error('timed out'), false, false), 'timeout');
});

function lifecycleFixture(writer) {
  const endpoint = 'ws://127.0.0.1:7111/ws';
  const context = { writer, pool: pool(endpoint, 2), phase: 'ramp', guard: { assert() {} }, add() {} };
  const client = new CapacityClient(endpoint, actor(0), context);
  const owner = { objectId: 1000, kind: 'player', name: actor(0).name, x: 10, y: 10 };
  const peer = { objectId: 50001, kind: 'player', name: actor(1).name, x: 11, y: 10 };
  const snapshot = entities => ({ type: 'worldSnapshot', payload: { mapFileName: '0', playerObjectId: 1000,
    entities, inventoryItems: [{ password: actor(0).password }], credential: 'do-not-persist-snapshot-ticket' } });
  const packet = (name, payload) => ({ type: 'packet', packet: name, payload });
  return { client, context, owner, peer, snapshot, packet };
}

test('player lifecycle diagnostics preserve remove and reentry wire order with projection before and after', () => {
  const rows = [], { client, owner, peer, snapshot, packet } = lifecycleFixture({ add: row => rows.push(row) });
  client.observe(snapshot([owner]));
  client.observe(packet('ObjectPlayer', { ...peer, password: actor(0).password, arbitraryPrivateField: 'omit-this-field' }));
  client.observe(packet('ObjectRemove', { objectId: peer.objectId }));
  client.observe(packet('ObjectPlayer', peer));
  client.observe(packet('ObjectRemove', { objectId: peer.objectId })); // an older direct remove delivered AFTER reentry
  client.observe(packet('ObjectRun', { objectId: peer.objectId, location: { x: 12, y: 10 } }));
  const lifecycle = rows.filter(row => row.source !== 'worldSnapshot');
  assert.deepEqual(lifecycle.map(row => row.source), ['ObjectPlayer', 'ObjectRemove', 'ObjectPlayer', 'ObjectRemove', 'ObjectRun']);
  assert.deepEqual(lifecycle.map(row => [row.beforePresent, row.afterPresent]), [[false, true], [true, false], [false, true], [true, false], [false, false]]);
  assert.ok(lifecycle.every(row => row.peer.name === actor(1).name && row.peer.objectId === 50001));
  assert.equal(lifecycle.at(-1).orphanMotion, true); assert.equal(lifecycle.at(-1).identitySource, 'bounded-identity-cache');
  assert.deepEqual(lifecycle.at(-1).wire, { objectId: 50001, x: 12, y: 10 });
  for (let i = 1; i < rows.length; i++) { assert.ok(rows[i].sequence > rows[i - 1].sequence); assert.ok(rows[i].monotonicMs >= rows[i - 1].monotonicMs); }
  assert.ok(rows.every(row => row.connection === client.lifecycleConnection && (row.before.self?.name === actor(0).name || row.before.self === null)));
  assert.equal(client.snapshot.entities.some(entity => entity.objectId === peer.objectId), false);
  const text = JSON.stringify(rows);
  for (const privateValue of [actor(0).password, 'do-not-persist-snapshot-ticket', 'omit-this-field', 'inventoryItems', 'arbitraryPrivateField']) assert.equal(text.includes(privateValue), false);
});

test('player lifecycle diagnostics retain same-membership owner coordinate replacements but omit ordinary peer movement', () => {
  const rows = [], { client, context, owner, peer, snapshot, packet } = lifecycleFixture({ add: row => rows.push(row) });
  client.observe(snapshot([owner, peer]));
  const count = rows.length;
  for (let i = 0; i < 1000; i++) client.observe(packet('ObjectRun', { objectId: peer.objectId, location: { x: 11 + i, y: 10 } }));
  assert.equal(rows.length, count, 'ordinary peer motion is still omitted');
  client.observe(snapshot([{ ...owner, x: 15 }, { ...peer, x: 20 }]));
  assert.equal(rows.length, count + 1, 'an authoritative snapshot overwriting owner coordinates is causal evidence');
  assert.equal(rows.at(-1).before.self.x, 10); assert.equal(rows.at(-1).after.self.x, 15);
  client.observe(snapshot([{ ...owner, x: 15 }, { ...peer, x: 21 }]));
  assert.equal(rows.length, count + 1, 'only peer coordinates changing does not copy another snapshot');
  client.observe(snapshot([{ ...owner, x: 15 }]));
  assert.equal(rows.at(-1).source, 'worldSnapshot');
  assert.deepEqual(rows.at(-1).before.players.map(player => player.name), [actor(0).name, actor(1).name]);
  assert.deepEqual(rows.at(-1).after.players.map(player => player.name), [actor(0).name]);
  for (let i = 0; i < 1000; i++) client.observe(packet('ObjectRun', { objectId: peer.objectId, location: { x: 11 + i, y: 10 } }));
  assert.equal(rows.filter(row => row.orphanMotion).length, 1, 'only the first orphan movement per absence is retained');
  client.observe(snapshot([owner, { ...peer, objectId: 50002 }]));
  assert.equal(rows.at(-1).after.players[1].objectId, 50002, 'replacement identity is visible even for the same character name');
  const beforeNonCohort = rows.length;
  client.observe(packet('ObjectPlayer', { ...peer, objectId: 90000, name: 'UnrelatedPerson' }));
  client.observe(packet('ObjectRemove', { objectId: 90000 }));
  assert.equal(rows.length, beforeNonCohort); assert.equal(JSON.stringify(rows).includes('UnrelatedPerson'), false);
  const other = new CapacityClient(client.url, actor(0), context);
  assert.notEqual(other.lifecycleConnection, client.lifecycleConnection, 'resume/relogin connection sequence resets cannot be confused');
  assert.ok(client.events.length <= 2048);
});

test('self transform diagnostics preserve unsolicited public position changes with fixed fields and no peer or unchanged-coordinate traffic', () => {
  const rows = [], { client, owner, peer, snapshot, packet } = lifecycleFixture({ add: row => rows.push(row) });
  client.observe(snapshot([owner, peer]));
  client.observe(packet('UserLocation', { x: 12, y: 10, direction: 'Right', password: 'never-log-password' }));
  client.observe(packet('UserLocation', { x: 12, y: 10, direction: 'Up' }));
  client.observe(packet('KeepAlive', { time: 99, x: 800, y: 800 }));
  client.observe(packet('ObjectRun', { objectId: peer.objectId, location: { x: 15, y: 10 } }));
  client.observe(packet('Pushed', { x: 11, y: 10, direction: 'Left', inventoryItems: ['private-item'] }));
  client.observe(packet('ObjectStruck', { objectId: 1000, location: { x: 13, y: 11 }, attackerObjectId: 9999, token: 'never-log-token' }));
  assert.deepEqual({ x: client.snapshot.entities[0].x, y: client.snapshot.entities[0].y }, { x: 11, y: 10 });
  client.observe(packet('UserLocation', { x: 13, y: 11 }));
  client.observe(packet('UserDash', { location: { x: 13, y: 13 }, direction: 'Down' }));
  client.observe(packet('ObjectBackStep', { objectId: 1000, location: { x: 13, y: 12 }, direction: 'Up' }));
  const changes = rows.filter(row => row.type === 'selfTransform');
  assert.deepEqual(changes.map(row => row.source), ['UserLocation', 'Pushed', 'UserLocation', 'UserDash', 'ObjectBackStep']);
  assert.deepEqual(changes.map(row => row.before.position), [{ x: 10, y: 10 }, { x: 12, y: 10 }, { x: 11, y: 10 }, { x: 13, y: 11 }, { x: 13, y: 13 }]);
  assert.deepEqual(changes[2].wire, { x: 13, y: 11 });
  assert.ok(changes.every(row => row.connection === client.lifecycleConnection && row.after.playerObjectId === 1000));
  for (let index = 1; index < changes.length; index++) assert.ok(changes[index].sequence > changes[index - 1].sequence);
  assert.doesNotMatch(JSON.stringify(changes), /never-log|private-item|inventoryItems|attackerObjectId|players|entities/);
  client.observe(snapshot([owner, peer]));
  assert.equal(rows.at(-1).type, 'playerLifecycle'); assert.deepEqual(rows.at(-1).before.self.x, 13); assert.deepEqual(rows.at(-1).after.self.x, 10);

  let count = 0, maximumBytes = 0;
  client.context.writer = { add: row => { count++; maximumBytes = Math.max(maximumBytes, Buffer.byteLength(JSON.stringify(row))); } };
  for (let index = 0; index < 3000; index++) client.observe(packet('UserLocation', { x: 100 + index, y: 10, direction: 'Right' }));
  assert.equal(count, 3000); assert.equal(client.selfTransformRows, 3005); assert.ok(maximumBytes < 700);
  assert.ok(client.events.length <= 2048); assert.equal(client.lifecycleIdentities.size, 2, 'no per-move aggregate or growing identity table');
});

test('captured ACK then old self Struck leaves the next native Walk planned from the actual owner tile', () => {
  for (const [ack, old] of [
    [{ x: 289, y: 563, direction: 'Down' }, { x: 289, y: 562, direction: 'Right' }],
    [{ x: 291, y: 560, direction: 'Right' }, { x: 289, y: 560, direction: 'Right' }],
  ]) {
    const rows = [], { client, context, owner, snapshot, packet } = lifecycleFixture({ add: row => rows.push(row) });
    Object.assign(context, { policy: capacityPolicy({ profile: 'probe', activityMode: 'native' }),
      map: { width: 600, height: 700, blocked: new Uint8Array(600 * 700) } });
    client.observe(snapshot([{ ...owner, ...old }]));
    client.observe(packet('UserLocation', ack));
    client.observe(packet('ObjectStruck', { objectId: 1000, location: { x: old.x, y: old.y }, direction: old.direction, attackerId: 900 }));
    Object.assign(client, { home: { x: ack.x, y: ack.y }, role: 'movement', targetCursor: 0, movementCount: 0,
      nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity, plan: new NativePlan(0), nativeRunPrimedUntil: 0 });
    const plan = selectNativeAction(client, context, 0);
    assert.deepEqual(plan.command, { type: 'walk', direction: 'Right' });
    assert.deepEqual(plan.target, { x: ack.x + 1, y: ack.y });
    assert.ok(client.events.some(event => event.packet === 'ObjectStruck'), 'hit evidence remains observable');
    assert.equal(rows.filter(row => row.type === 'selfTransform').length, 1, 'only the real movement ACK changes the owner position');
    client.observe(packet('UserLocation', { ...plan.target, direction: plan.command.direction }));
    assert.deepEqual({ x: client.snapshot.entities[0].x, y: client.snapshot.entities[0].y }, plan.target);
  }
});

test('player lifecycle diagnostics keep a bounded remembered identity and projection footprint', () => {
  const { client, owner, peer, snapshot, packet } = lifecycleFixture({ add() {} });
  client.observe(snapshot([owner]));
  for (let i = 0; i < 700; i++) {
    client.observe(packet('ObjectPlayer', { ...peer, objectId: 50000 + i }));
    client.observe(packet('ObjectRemove', { objectId: 50000 + i }));
  }
  assert.equal(client.lifecycleIdentities.size, 256); assert.ok(client.lifecycleIdentityEvictions > 0);
  assert.ok(client.lifecycleMissing.size <= 256); assert.ok(client.lifecycleOrphans.size <= 256);
  const rows = []; client.context.writer = { add: row => rows.push(row) };
  client.observe(snapshot([owner, ...Array.from({ length: 500 }, (_, i) => ({ ...peer, objectId: 60000 + i }))]));
  assert.equal(rows.at(-1).after.players.length, 256); assert.equal(rows.at(-1).after.count, 501); assert.equal(rows.at(-1).after.truncated, true);
  client.observe(snapshot([owner]));
  assert.ok(client.lifecycleIdentities.size <= 256); assert.ok(client.lifecycleMissing.size <= 256);
});

test('player lifecycle diagnostics persist real socket receive order through the existing bounded writer without secrets', async () => withDirectory(async directory => {
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 }); await new Promise(resolve => server.once('listening', resolve));
  const endpoint = `ws://127.0.0.1:${server.address().port}/ws`, evidenceFile = path.join(directory, 'lifecycle.jsonl');
  const writer = new BoundedEvidence(evidenceFile), fixture = lifecycleFixture(writer);
  const context = { ...fixture.context, pool: pool(endpoint, 2) }, client = new CapacityClient(endpoint, actor(0), context);
  server.on('connection', socket => {
    const send = value => socket.send(JSON.stringify(value));
    send(fixture.packet('Connected', {}));
    socket.on('message', data => {
      const command = JSON.parse(data);
      if (command.type === 'clientVersion') { send(fixture.packet('ClientVersion', {})); send(fixture.snapshot([fixture.owner])); }
      if (command.type === 'clientCapabilities') {
        send(fixture.packet('ObjectPlayer', { ...fixture.peer, password: actor(0).password }));
        send(fixture.packet('UserLocation', { x: 12, y: 10, direction: 'Right', credential: 'never-log-owner-ticket' }));
        send(fixture.packet('ObjectRemove', { objectId: fixture.peer.objectId }));
        send(fixture.packet('ObjectPlayer', fixture.peer));
        send(fixture.packet('ObjectRemove', { objectId: fixture.peer.objectId }));
        send(fixture.packet('ObjectRun', { objectId: fixture.peer.objectId, location: { x: 12, y: 10 } }));
        send(fixture.snapshot([fixture.owner, fixture.peer]));
      }
      if (command.type === 'logOut') send(fixture.packet('LogOutSuccess', {}));
    });
  });
  try {
    await client.connect(); await client.wait(() => client.lifecycleRows >= 7, 'real lifecycle diagnostic rows');
    assert.equal(client.failure, undefined); await client.close(); await writer.close();
    const text = await fs.readFile(evidenceFile, 'utf8'), rows = text.trim().split('\n').map(JSON.parse);
    assert.deepEqual(rows.map(row => row.source), ['worldSnapshot', 'ObjectPlayer', 'UserLocation', 'ObjectRemove', 'ObjectPlayer', 'ObjectRemove', 'ObjectRun', 'worldSnapshot']);
    assert.equal(rows[2].type, 'selfTransform'); assert.deepEqual(rows[2].after.position, { x: 12, y: 10 });
    assert.equal(rows.at(-1).before.self.x, 12); assert.equal(rows.at(-1).after.self.x, 10);
    assert.equal(rows.at(-1).after.players.length, 2); assert.equal(rows.at(-2).afterPresent, false);
    assert.ok(rows.every(row => Number.isInteger(row.sequence) && Number.isFinite(row.monotonicMs) && Number.isFinite(row.at)));
    for (const secret of [actor(0).password, 'do-not-persist-snapshot-ticket', 'inventoryItems', 'credential']) assert.equal(text.includes(secret), false);
  } finally {
    client.draining = true; await client.close().catch(() => {});
    for (const socket of server.clients) socket.terminate(); await new Promise(resolve => server.close(resolve));
    if (!writer.stream.writableEnded) await writer.close();
  }
}));

test('guards, dead or disappeared targets and temporarily failed targets are never reused as ordinary combat load', () => {
  const client = observer('fighter', 10); client.failedTargets = new Map([[4, { until: 2000 }]]);
  client.snapshot.entities.push(...[
    { objectId: 1, kind: 'monster', name: 'Guard', x: 11, y: 5, hp: 9999 },
    { objectId: 2, kind: 'monster', name: 'Hen', x: 11, y: 5, hp: 5 },
    { objectId: 3, kind: 'monster', name: 'Deer', x: 11, y: 5, hp: 0, dead: true },
    { objectId: 4, kind: 'monster', name: 'Scarecrow', x: 11, y: 5, hp: 20 },
    { objectId: 5, kind: 'monster', name: 'Hen', x: 12, y: 5, hp: 5 },
    { objectId: 6, kind: 'monster', name: 'Hen', x: 11, y: 5, hp: 5, disposition: 'friendly' },
    { objectId: 7, kind: 'monster', name: 'Hen', x: 11, y: 5, hp: 5, ownerName: 'SomeOtherPlayer' },
  ]);
  const context = { monsterNames: ['Guard', 'Hen', 'Deer', 'Scarecrow'], targetClaims: new Map([[5, 'other-fighter']]) };
  assert.deepEqual(combatCandidates(client, context, 1000).map(target => target.objectId), [2]);
  client.snapshot.entities = client.snapshot.entities.filter(item => item.objectId !== 2);
  assert.deepEqual(combatCandidates(client, context, 1000), []);
});

test('patrol anchors exclude disconnected regions, blocked cells and legal doorway hazards', () => {
  const map = { width: 30, height: 30, blocked: new Uint8Array(900) };
  for (let y = 0; y < 30; y++) map.blocked[y * 30 + 15] = 1;
  const homes = chooseHomes(map, { x: 8, y: 15 }, 30, 'dispersed', [{ bounds: { minX: 9, maxX: 11, minY: 8, maxY: 10 } }]);
  assert.equal(new Set(homes.map(point => `${point.x},${point.y}`)).size, 30);
  assert.ok(homes.every(point => point.x < 15 && !map.blocked[point.y * 30 + point.x] && !(point.x >= 9 && point.x <= 11 && point.y >= 8 && point.y <= 10)));
});

test('spacing-four scenario generation uses reachable cells and labels static visibility as unaccepted planning', () => {
  const map = { width: 80, height: 80, blocked: new Uint8Array(6400) };
  for (let y = 0; y < 80; y++) map.blocked[y * 80 + 60] = 1;
  const transfers = [{ bounds: { minX: 22, maxX: 22, minY: 22, maxY: 22 } }];
  const scenario = spacingFourScenario(map, { x: 30, y: 30 }, { x: 30, y: 30 }, 50, 'fixture', transfers);
  assert.equal(scenario.anchors.length, 50); assert.equal(new Set(scenario.anchors.map(p => `${p.x},${p.y}`)).size, 50);
  assert.ok(scenario.anchors.every(point => point.x < 60 && !(point.x === 22 && point.y === 22)));
  const footprint = new Set();
  for (const anchor of scenario.anchors) for (let dy = 0; dy < 3; dy++) for (let dx = 0; dx < 3; dx++) {
    const x = anchor.x + dx, y = anchor.y + dy, key = `${x},${y}`;
    assert.equal(footprint.has(key), false, 'native patrols cannot share intermediate or corner cells'); footprint.add(key);
    assert.equal(map.blocked[y * 80 + x], 0); assert.equal(x === 22 && y === 22, false);
  }
  assert.equal(footprint.size, 450);
  assert.equal(scenario.generated.staticOnly, true); assert.ok(scenario.generated.meanVisiblePeers > 20);
  assert.throws(() => spacingFourScenario(map, { x: 30, y: 30 }, { x: 70, y: 30 }, 5, 'fixture'), /lacks enough reachable/);
});

test('long-term growth is distinct from a filled cache plateau', () => {
  const plateau = Array.from({ length: 121 }, (_, i) => ({ sampledAtMs: i * 30000, memory: { currentBytes: 100 * 1048576 } }));
  assert.equal(memoryTrend(plateau).slopeMiBPerHour, 0);
  const leak = plateau.map((point, i) => ({ ...point, memory: { currentBytes: point.memory.currentBytes + i * 1048576 } }));
  assert.ok(Math.abs(memoryTrend(leak).slopeMiBPerHour - 120) < .0001);
  assert.equal(memoryTrend([plateau[0]]).slopeMiBPerHour, null);
});

test('one-hour monitor history retains stable time span for both 1Hz and 2Hz producers in bounded memory', () => {
  for (const step of [1000, 500]) {
    const history = new TimeBucketSamples();
    for (let at = 0; at <= 3600000; at += step) history.add({ sampledAtMs: at, memory: { currentBytes: 100 * 1048576 + at / 3600000 * 20 * 1048576 } });
    assert.equal(history.rawSamples, 3600000 / step + 1); assert.ok(history.samples.length <= 1201);
    const trend = memoryTrend(history.samples.filter(sample => sample.sampledAtMs >= 300000));
    assert.ok(trend.seconds >= 3297); assert.ok(Math.abs(trend.slopeMiBPerHour - 20) < .0001);
  }
});

test('public evidence redacts tickets and fails closed if its bounded write queue overflows', async () => withDirectory(async directory => {
  const file = path.join(directory, 'evidence.jsonl'), writer = new BoundedEvidence(file, 256);
  writer.add({ token: 'example-token', password: 'private-pass', nested: { credential: 'native-ticket' } });
  await writer.close();
  const text = await fs.readFile(file, 'utf8');
  for (const secret of ['example-token', 'private-pass', 'native-ticket']) assert.equal(text.includes(secret), false);
  const overflow = new BoundedEvidence(path.join(directory, 'overflow.jsonl'), 32);
  overflow.add({ value: 'x'.repeat(100) }); await assert.rejects(overflow.close(), /overflow/);
  assert.deepEqual(sanitize({ credential: 'secret' }), { credential: '[redacted]' });
}));

test('long native-ticket rotation keeps bounded history and never retains bulky snapshots or raw secrets in receipts', () => {
  const context = { guard: { assert() {} }, writer: { add() {} } }, client = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), context);
  for (let i = 0; i < 10000; i++) client.record('received', { type: 'packet', packet: 'ObjectTurn', payload: { objectId: 5, direction: 'Up' } });
  for (let i = 0; i < 100; i++) client.observe({ type: 'resumeCredential', protocol: 'nativeResumeV1', generation: i,
    credential: Buffer.alloc(32, i).toString('base64url'), expiresAtMs: Date.now() + 30000 });
  assert.ok(client.events.length <= 2048); assert.equal(client.recentSecrets.length, 3);
  assert.equal(client.secrets.length, 1); assert.equal(client.events.findLast(event => event.type === 'resumeCredential').credential, '[redacted]');
});

async function fakeRealm(directory, { rateLimit = false, stopOnWalk = false, delayedWalk = false, rejectWalkDuringRamp = false } = {}) {
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 }); await new Promise(resolve => server.once('listening', resolve));
  const endpoint = `ws://127.0.0.1:${server.address().port}/ws`, types = [], monitor = path.join(directory, 'monitor.json');
  const accountFile = path.join(directory, 'private', 'accounts.json'), maps = path.join(directory, 'maps');
  await fs.mkdir(path.dirname(accountFile)); await fs.mkdir(maps);
  await fs.writeFile(accountFile, JSON.stringify(pool(endpoint)));
  const bytes = Buffer.alloc(52 + 32 * 32 * 12); bytes.writeInt16LE(32, 0); bytes.writeInt16LE(32, 2);
  await fs.writeFile(path.join(maps, 'capacityfixture.map'), bytes);
  let stopping = false, monitorWrite = Promise.resolve(), monitorFailure, monitorHoldUntil = 0;
  let created = false, x = 16, y = 16, direction = 'Down', currentTicket, generation = 0;
  const writeMonitor = () => {
    monitorWrite = monitorWrite.then(async () => {
      if (Date.now() < monitorHoldUntil) return;
      await fs.writeFile(`${monitor}.next`, JSON.stringify({ ...healthy(), stop: stopping, reason: stopping ? 'fixture memory threshold' : undefined }));
      for (let attempt = 0; ; attempt++) {
        try { await fs.rename(`${monitor}.next`, monitor); break; }
        catch (error) { if (!['EPERM', 'EACCES'].includes(error.code) || attempt >= 20) throw error; await sleep(10); }
      }
    });
    return monitorWrite;
  };
  await writeMonitor(); const timer = setInterval(() => void writeMonitor().catch(error => { monitorFailure = error; }), 100);
  server.on('connection', (socket, request) => {
    assert.equal(request.headers.origin, `http://127.0.0.1:${server.address().port}`);
    let inGame = false, capabilities = false;
    const send = (packet, payload) => { if (socket.readyState === 1) socket.send(JSON.stringify({ type: 'packet', packet, payload })); };
    const snapshot = () => { if (socket.readyState === 1) socket.send(JSON.stringify({ type: 'worldSnapshot', payload: {
      mapFileName: 'capacityfixture', playerObjectId: inGame ? 1000 : null, playerHp: 100, playerMaxHp: 100,
      playerCrystalStats: [{ stat: 14, value: 0 }],
      entities: inGame ? [{ name: actor(0).name, objectId: 1000, kind: 'player', level: 1, x, y, direction, hp: 100, maxHp: 100, dead: false }] : [],
      inventoryItems: [], equipmentItems: [], mapTransfers: [], gold: 0 } })); };
    send('Connected', {});
    const renew = () => {
      if (!inGame || !capabilities || socket.readyState !== 1) return;
      currentTicket = { credential: Buffer.alloc(32, ++generation).toString('base64url'), generation, expiresAtMs: Date.now() + 30000 };
      socket.send(JSON.stringify({ type: 'resumeCredential', protocol: 'nativeResumeV1', ...currentTicket }));
    };
    const renewal = setInterval(renew, 10000); socket.on('close', () => clearInterval(renewal));
    socket.on('message', data => {
      const input = JSON.parse(data); types.push({ type: input.type, at: Date.now() });
      if (input.type === 'clientVersion') { send('ClientVersion', {}); snapshot(); }
      if (input.type === 'clientCapabilities') capabilities = true;
      if (input.type === 'resumeSession') {
        if (input.credential === currentTicket?.credential && currentTicket.expiresAtMs > Date.now()) {
          inGame = true;
          socket.send(JSON.stringify({ type: 'sessionResumed', protocol: 'nativeResumeV1', characterIndex: 17, generation: generation + 1 }));
          snapshot(); renew();
        } else socket.send(JSON.stringify({ type: 'resumeRejected', code: 'unavailable' }));
      }
      if (input.type === 'login') {
        if (rateLimit) socket.send(JSON.stringify({ type: 'error', code: 'rateLimited', message: 'too many authentication attempts; retry after 900 seconds' }));
        else send('LoginSuccess', { characters: created ? [{ name: actor(0).name, index: 17 }] : [] });
      }
      if (input.type === 'newCharacter') { created = true; send('NewCharacterSuccess', { character: { name: input.name, index: 17 } }); }
      if (input.type === 'startGame') {
        inGame = true;
        // Hold an otherwise healthy monitor briefly so afterAdmission keeps
        // this test in ramp, while its already admitted actor really moves.
        if (rejectWalkDuringRamp) monitorHoldUntil = Date.now() + 2500;
        send('StartGame', { result: 4 }); snapshot(); renew();
      }
      if (input.type === 'walk' || input.type === 'run') {
        if (rejectWalkDuringRamp) { socket.send(JSON.stringify({ type: 'error', code: 'ordinaryCommandRejected', message: 'fixture movement denied' })); return; }
        const delta = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] }[input.direction];
        x += delta[0] * (input.type === 'run' ? 2 : 1); y += delta[1] * (input.type === 'run' ? 2 : 1); direction = input.direction;
        if (delayedWalk) setTimeout(() => send('UserLocation', { x, y, direction }), 1800);
        else send('UserLocation', { x, y, direction });
        if (stopOnWalk) { stopping = true; void writeMonitor().catch(error => { monitorFailure = error; }); }
      }
      if (input.type === 'turn') { direction = input.direction; send('UserLocation', { x, y, direction }); }
      if (input.type === 'chat') send('ObjectChat', { objectId: 1000, text: input.message });
      if (input.type === 'keepAlive') send('KeepAlive', { time: input.time });
      if (input.type === 'logOut') { inGame = false; send('LogOutSuccess', { characters: [] }); }
    });
  });
  return { endpoint, types, monitor, accountFile, maps, stop: () => { stopping = true; return writeMonitor(); },
    async close() {
      clearInterval(timer); await monitorWrite.catch(error => { monitorFailure = error; });
      for (const socket of server.clients) socket.terminate(); await new Promise(resolve => server.close(resolve));
      if (monitorFailure) throw monitorFailure;
    } };
}
function runOptions(realm, directory) {
  return { realm: 'isolated', endpoint: realm.endpoint, accountsFile: realm.accountFile, monitor: realm.monitor,
    output: path.join(directory, 'reports'), profile: 'probe', stages: '1', stageSeconds: 20, soakSeconds: 20,
    wallSeconds: 120, loginNotBefore: new Date().toISOString(), mapRoot: realm.maps };
}

test('native combat arrival uses real socket movement without observers and keeps the entire transit in continuous activity', async () => withDirectory(async directory => {
  const realm = await fakeRealm(directory), rows = [], evidence = [], errors = [];
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native', movementIntervalMs: 650, stages: '2', combatActors: 1 });
  const continuous = new ContinuousAcceptance(policy);
  const context = { endpoint: realm.endpoint, policy, runId: 'combat-arrival-wire', phase: 'settle', clients: [],
    map: await loadProtocolCollisionMap('capacityfixture', { mapRoot: realm.maps, packagedMapRoot: realm.maps }),
    mapName: 'capacityfixture', monsterNames: ['Hen'], targetClaims: new Map(),
    guard: { assert() {}, stop: message => errors.push(message) }, writer: { add: row => evidence.push(row) },
    add: row => { rows.push(row); continuous.add(row); } };
  const client = new CapacityClient(realm.endpoint, actor(0), context); context.clients.push(client);
  try {
    await client.connect(); await client.request({ type: 'login', accountId: client.account.accountId, password: client.account.password }, 'LoginSuccess');
    await client.request({ type: 'newCharacter', name: client.label, class: 'Warrior', gender: 'Male' }, 'NewCharacterSuccess');
    await client.request({ type: 'startGame', characterIndex: 17 }, 'StartGame');
    client.role = 'combat'; client.ready = true; client.home = { x: 16, y: 8 };
    client.nextNativeChatAt = Infinity; client.nextNativeTurnAt = Infinity;
    client.activityStartedAtMs = performance.now(); continuous.join(client.label, client.role, client.activityStartedAtMs);
    client.plan = new NativePlan(performance.now() + 1200);
    const deadline = performance.now() + 8000;
    while (!client.combatArrival && performance.now() < deadline) { offerNativeAction(client, context); await sleep(20); }
    await awaitPendingActions([client]); continuous.pause(client.label); continuous.finish();
    assert.deepEqual(errors, []); assert.deepEqual(client.combatArrival?.actual, { x: 16, y: 8 });
    assert.equal(actorsReachedScenario(context), true); assert.ok(client.combatArrival.activeElapsedMs >= 3000);
    assert.equal(evidence.filter(row => row.type === 'combatArrival').length, 1);
    const moves = rows.filter(row => ['walk', 'run'].includes(row.kind));
    assert.ok(moves.length >= 5); assert.ok(moves.every(row => row.status === 'success' && row.initialCombatTransit));
    assert.equal(moves.reduce((sum, row) => sum + row.movedCells, 0), 8);
    assert.ok(continuous.summary().actors[0].activeSeconds >= 3, 'ordinary transit is never paused out of load coverage');
    assert.equal(realm.types.some(row => ['attack', 'moveTo', 'stage5Command'].includes(row.type)), false);
  } finally { await awaitPendingActions([client]); client.draining = true; await client.close(); await realm.close(); }
}));

test('real socket protocol stops after auth rejection without retry, StartGame or rewriting account progress', async () => withDirectory(async directory => {
  const realm = await fakeRealm(directory, { rateLimit: true });
  try {
    const report = await runCapacity({ ...runOptions(realm, directory), resumeSamples: 1, saveSamples: 1 });
    assert.equal(report.ok, false); assert.match(report.error, /too many authentication/);
    assert.equal(realm.types.filter(item => item.type === 'login').length, 1);
    assert.equal(realm.types.some(item => item.type === 'startGame'), false);
    const stored = JSON.parse(await fs.readFile(realm.accountFile, 'utf8')); assert.equal(stored.accounts[0].state, 'registered');
    const ledger = JSON.parse(await fs.readFile(path.join(directory, 'private', 'capacity-auth-budget.json'), 'utf8'));
    assert.equal(ledger.attempts.length, 1);
  } finally { await realm.close(); }
}));

test('a stop from the parent monitor drains a moving ordinary actor, preserves failure and never retries login', async () => withDirectory(async directory => {
  const realm = await fakeRealm(directory, { stopOnWalk: true });
  try {
    const report = await runCapacity({ ...runOptions(realm, directory), saveSamples: 1 });
    assert.equal(report.ok, false); assert.equal(report.safetyStopped, true); assert.equal(report.playableCapacityAccepted, false);
    assert.match(report.safetyReason, /fixture memory/); assert.deepEqual(report.cleanupErrors, []);
    assert.equal(realm.types.filter(item => item.type === 'login').length, 1);
    assert.equal(realm.types.filter(item => item.type === 'newCharacter').length, 1);
    assert.equal(realm.types.filter(item => item.type === 'logOut').length, 1);
    assert.equal(realm.types.some(item => ['newAccount', 'moveTo', 'stage5'].includes(item.type)), false);
    assert.equal(report.savedVerification.length, 0);
    const stored = JSON.parse(await fs.readFile(realm.accountFile, 'utf8')); assert.equal(stored.accounts[0].characterIndex, 17);
    for (const file of await fs.readdir(path.join(directory, 'reports'))) assert.equal((await fs.readFile(path.join(directory, 'reports', file), 'utf8')).includes(actor(0).password), false);
  } finally { await realm.close(); }
}));

test('a real socket gameplay rejection during unmeasured ramp latches failure and drains normally', async () => withDirectory(async directory => {
  const realm = await fakeRealm(directory, { rejectWalkDuringRamp: true });
  try {
    const report = await runCapacity(runOptions(realm, directory));
    assert.equal(report.ok, false); assert.equal(report.stages.length, 0);
    assert.equal(report.continuousAcceptance.passed, false);
    assert.equal(report.continuousAcceptance.firstFailure.evidence.phase, 'ramp');
    assert.equal(report.continuousAcceptance.firstFailure.evidence.status, 'rejected');
    assert.equal(report.metrics.walk.status.rejected, 1); assert.deepEqual(report.cleanupErrors, []);
    assert.equal(realm.types.filter(item => item.type === 'logOut').length, 1);
    assert.equal(realm.types.filter(item => item.type === 'login').length, 1);
  } finally { await realm.close(); }
}));

test('missing monitor prevents private-account reads, login and all gameplay', async () => withDirectory(async directory => {
  const report = await runCapacity({ realm: 'isolated', endpoint: 'ws://127.0.0.1:1/ws', accountsFile: path.join(directory, 'private', 'absent.json'),
    monitor: path.join(directory, 'absent-monitor.json'), output: path.join(directory, 'reports'), profile: 'probe' });
  assert.equal(report.ok, false); assert.equal(report.safetyStopped, true); assert.deepEqual(report.bytes, { sent: 0, received: 0 });
  assert.equal(Object.hasOwn(report, 'attackResults'), false, 'default legacy report shape stays unchanged');
  const v2 = await runCapacity({ realm: 'isolated', endpoint: 'ws://127.0.0.1:1/ws', accountsFile: path.join(directory, 'private', 'absent.json'),
    monitor: path.join(directory, 'absent-monitor.json'), output: path.join(directory, 'v2-reports'), profile: 'probe', attackResults: 'v2' });
  assert.equal(v2.attackResults.mode, 'v2'); assert.equal(Object.isFrozen(v2.attackResults), true);
  assert.equal(Object.getOwnPropertyDescriptor(v2, 'attackResults').writable, false);
  assert.deepEqual(v2.bytes, { sent: 0, received: 0 });
  await assert.rejects(fs.stat(path.join(directory, 'private')), /ENOENT/);
}));

test('slow real socket acknowledgements expose offered-load gaps instead of silently lowering the requested activity', async () => withDirectory(async directory => {
  const realm = await fakeRealm(directory, { delayedWalk: true });
  try {
    const report = await runCapacity(runOptions(realm, directory));
    assert.equal(report.ok, false); assert.match(report.error, /Stage 1 did not satisfy/);
    assert.equal(report.stages.length, 1); assert.equal(report.stages[0].passed, false);
    assert.equal(report.phaseWindows.at(-1).completed, true, 'a completed measured interval may still fail its quality gates');
    assert.equal(report.phaseWindows.at(-1).endedAtMs, report.stages[0].endedAtMs);
    assert.ok(report.metrics.walk.status.busy > 0); assert.ok(report.metrics.walk.fromSend.p95Ms >= 1750);
    assert.deepEqual(report.cleanupErrors, []); assert.equal(realm.types.filter(item => item.type === 'logOut').length, 1);
  } finally { await realm.close(); }
}));

test('two real sockets both have SelfPlayer 1000 while movement and removal use each receiver\'s distinct Zone identity', async () => {
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 }); await new Promise(resolve => server.once('listening', resolve));
  const endpoint = `ws://127.0.0.1:${server.address().port}/ws`, active = new Map();
  server.on('connection', socket => {
    let index;
    const send = (packet, payload) => socket.send(JSON.stringify({ type: 'packet', packet, payload }));
    const snapshot = () => socket.send(JSON.stringify({ type: 'worldSnapshot', payload: { mapFileName: '0', playerObjectId: active.has(index) ? 1000 : null,
      entities: [...active].map(([id, state]) => ({ ...state.player, objectId: id === index ? 1000 : state.player.objectId })), inventoryItems: [], equipmentItems: [] } }));
    send('Connected', {});
    socket.on('message', data => {
      const command = JSON.parse(data);
      if (command.type === 'clientVersion') { send('ClientVersion', {}); snapshot(); }
      if (command.type === 'login') { index = command.accountId === actor(0).accountId ? 0 : 1; send('LoginSuccess', { characters: [{ index, name: actor(index).name }] }); }
      if (command.type === 'startGame') {
        const player = { objectId: 50001 + index, kind: 'player', name: actor(index).name, x: 10 + index * 2, y: 10, direction: 'Right', hp: 100, maxHp: 100 };
        active.set(index, { player, socket }); send('StartGame', { result: 4 }); snapshot();
        for (const [id, other] of active) if (id !== index) other.socket.send(JSON.stringify({ type: 'packet', packet: 'ObjectPlayer', payload: player }));
      }
      if (command.type === 'walk') {
        const state = active.get(index); state.player.x++;
        send('UserLocation', { x: state.player.x, y: state.player.y, direction: 'Right' });
        for (const [id, other] of active) if (id !== index) other.socket.send(JSON.stringify({ type: 'packet', packet: 'ObjectWalk', payload: {
          objectId: state.player.objectId, location: { x: state.player.x, y: state.player.y }, direction: 'Right' } }));
      }
      if (command.type === 'logOut') {
        const state = active.get(index); active.delete(index); send('LogOutSuccess', { characters: [] });
        for (const other of active.values()) other.socket.send(JSON.stringify({ type: 'packet', packet: 'ObjectRemove', payload: { objectId: state.player.objectId } }));
      }
    });
  });
  const context = { guard: { assert() {} }, writer: { add() {} }, add() {} }, clients = [0, 1].map(index => new CapacityClient(endpoint, actor(index), context));
  try {
    for (const [index, client] of clients.entries()) {
      await client.connect(); await client.request({ type: 'login', accountId: client.account.accountId, password: client.account.password }, 'LoginSuccess');
      await client.request({ type: 'startGame', characterIndex: index }, 'StartGame');
    }
    await clients[0].wait(() => observedPlayerId(clients[0], actor(1).name) === 50002, 'real other-player projection');
    assert.equal(clients[0].snapshot.playerObjectId, 1000); assert.equal(clients[1].snapshot.playerObjectId, 1000);
    assert.equal(observedPlayerId(clients[1], actor(0).name), 50001);
    const coverage = new AoiCoverage().sample(clients); assert.equal(coverage.expectedPairs, 2); assert.equal(coverage.observedPairs, 2);
    await clients[0].request({ type: 'walk', direction: 'Right' }, 'UserLocation');
    const event = await clients[1].wait(() => clients[1].events.find(event => event.packet === 'ObjectWalk'), 'real observer movement');
    assert.equal(matchesFanoutReceipt(event, 'walk', {}, observedPlayerId(clients[1], actor(0).name), { x: 11, y: 10 }), true);
    assert.equal(matchesFanoutReceipt(event, 'walk', {}, 1000, { x: 11, y: 10 }), false);
    await clients[0].logout();
    await clients[1].wait(() => clients[1].events.some(event => event.packet === 'ObjectRemove' && event.payload.objectId === 50001), 'mapped removal');
    assert.equal(observedPlayerId(clients[1], actor(0).name), null);
  } finally {
    for (const client of clients) { client.draining = true; await client.close(); }
    for (const socket of server.clients) socket.terminate(); await new Promise(resolve => server.close(resolve));
  }
});

test('a complete bounded socket probe produces real movement samples but cannot claim 50-player or combat acceptance', async () => withDirectory(async directory => {
  const realm = await fakeRealm(directory);
  try {
    const report = await runCapacity({ ...runOptions(realm, directory), resumeSamples: 1, saveSamples: 1 });
    assert.equal(report.error, undefined); assert.equal(report.ok, true);
    assert.equal(report.stages.length, 3); assert.ok(report.metrics.walk.status.success >= 20);
    assert.equal(report.stages[2].name, 'post-resume-observation'); assert.equal(report.continuousAcceptance.passed, true);
    assert.equal(report.movementCapacityAccepted, false); assert.equal(report.combatCapacityAccepted, false); assert.equal(report.playableCapacityAccepted, false);
    assert.equal(report.acceptedControlledActors, null); assert.equal(report.capacityUpperBound, null);
    assert.equal(report.nativeResume.length, 1); assert.equal(report.nativeResume[0].replayRejected, true);
    assert.equal(report.nativeResume[0].loginFallback, false); assert.equal(report.savedVerification[0].status, 'passed');
    const raw = (await fs.readFile(report.reportPath.replace(/\.report\.json$/, '.samples.jsonl'), 'utf8')).trim().split('\n').map(line => JSON.parse(line));
    assert.equal(report.phaseWindows.length, 4); assert.ok(report.phaseWindows.every(phase => phase.completed && phase.endedAtMs >= phase.startedAtMs));
    assert.deepEqual(raw.filter(row => row.type === 'phaseEnded').map(({ type, at, ...phase }) => phase), report.phaseWindows);
    for (const stage of report.stages) {
      const phase = report.phaseWindows.find(window => window.name === stage.name);
      assert.equal(stage.startedAtMs, phase.startedAtMs); assert.equal(stage.endedAtMs, phase.endedAtMs);
    }
    assert.ok(report.density.sampleCount > 0); assert.ok(raw.filter(row => row.type === 'coverage').every(row => row.density?.samplingIntervalMs === 1000));
    for (const item of realm.types) assert.ok(realm.types.filter(other => other.at > item.at - 1000 && other.at <= item.at).length <= 3, 'at most 3 commands per actor in any rolling second');
  } finally { await realm.close(); }
}));

for (const { movementIntervalMs, patrolSpan } of [{ movementIntervalMs: 650 }, { movementIntervalMs: 750 }, { movementIntervalMs: 750, patrolSpan: 4 }]) test(`native ${movementIntervalMs}ms real socket probe produces sustained two-cell Runs and honest offered load${patrolSpan ? ' with explicit wide patrol' : ''}`, async () => withDirectory(async directory => {
  const realm = await fakeRealm(directory);
  try {
    const scenarioFile = patrolSpan ? path.join(directory, 'wide-patrol.json') : undefined;
    if (scenarioFile) await fs.writeFile(scenarioFile, JSON.stringify({ mapFileName: 'capacityfixture', anchors: [{ x: 16, y: 16 }], patrolSpan }));
    const report = await runCapacity({ ...runOptions(realm, directory), activityMode: 'native', movementIntervalMs, scenarioFile });
    assert.equal(report.error, undefined); assert.equal(report.ok, true);
    if (patrolSpan) { assert.equal(report.scenario.patrolSpan, 4); assert.equal(report.scenario.explicitNativeHomes.footprintCells, 25); }
    const moves = realm.types.filter(item => ['walk', 'run'].includes(item.type));
    assert.equal(moves[0].type, 'walk'); assert.ok(moves.filter(item => item.type === 'run').length >= 30);
    for (let i = 1; i < moves.length; i++) assert.ok(moves[i].at - moves[i - 1].at >= 599, 'never overrun the authoritative 600ms movement floor');
    for (const stage of report.stages) {
      assert.equal(stage.activity[0].nativeActivity.passed, true); assert.ok(stage.activity[0].nativeActivity.runRatio >= .5);
      assert.ok(stage.activity[0].nativeActivity.cellsPerMinute >= 80); assert.ok(stage.activity[0].nativeActivity.completionRatio >= .95);
    }
    const chats = realm.types.filter(item => item.type === 'chat');
    for (let i = 1; i < chats.length; i++) assert.ok(chats[i].at - chats[i - 1].at >= 30000);
    assert.equal(report.nativeLoadScope.singleUnacknowledgedMovement, true); assert.equal(report.playableCapacityAccepted, false);
    assert.equal(report.continuousAcceptance.passed, true); assert.deepEqual(report.cleanupErrors, []);
  } finally { await realm.close(); }
}));

test('attack-results v2 fails closed against a legacy server after the original15s capability deadline', async () => {
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 }); await new Promise(resolve => server.once('listening', resolve));
  const commands = [], { context } = huntFixture([]); context.attackResults = 'v2';
  server.on('connection', socket => { socket.send(JSON.stringify({ type: 'packet', packet: 'Connected', payload: {} }));
    socket.on('message', data => { const input = JSON.parse(data); commands.push(input);
      if (input.type === 'clientVersion') socket.send(JSON.stringify({ type: 'packet', packet: 'ClientVersion', payload: {} })); }); });
  const client = new CapacityClient(`ws://127.0.0.1:${server.address().port}/ws`, actor(0), context), started = performance.now();
  try {
    await assert.rejects(client.connect(), /attackV2 capability; no legacy fallback/);
    assert.ok(performance.now() - started >= 14900); assert.equal(client.attackV2Supported, false);
    assert.equal(commands.filter(command => ['attack', 'attackV2', 'login'].includes(command.type)).length, 0);
    assert.deepEqual(commands.find(command => command.type === 'clientCapabilities').capabilities,
      ['nativeResumeV1', 'serverCatalogGzipV1', ATTACK_OUTCOME_CAPABILITY]);
  } finally { client.draining = true; await client.close(); for (const socket of server.clients) socket.terminate(); await new Promise(resolve => server.close(resolve)); }
});

for (const { lateFirstHit, sparseStats, noSwingFirstHit, v2Outcome } of [{ lateFirstHit: false, sparseStats: false }, { lateFirstHit: true, sparseStats: false }, { lateFirstHit: false, sparseStats: true }, { noSwingFirstHit: true },
  ...['launched', 'notLaunched', 'missing', 'wrong-id', 'late', 'launched-no-damage', 'unavailable', 'unknown'].map(v2Outcome => ({ v2Outcome }))]) test(v2Outcome
  ? `attack-results v2 real socket ${v2Outcome} preserves explicit admission, damage and failure accounting`
  : noSwingFirstHit
  ? 'native real sockets keep same-position correction and target movement diagnostic while the actual900ms timeout still fails the1percent gate'
  : lateFirstHit
  ? 'native real sockets never reclaim an unresolved attack target when old damage arrives after the prior retry window'
  : sparseStats ? 'native real sockets read sparse authoritative zero AttackSpeed without inventing pacing'
  : 'native real sockets keep running while old observer evidence is pending and attacks respect current stats', async () => {
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 }); await new Promise(resolve => server.once('listening', resolve));
  const endpoint = `ws://127.0.0.1:${server.address().port}/ws`, active = new Map(), commands = [], rows = [], errors = [], evidence = [];
  const monster = { objectId: 900, name: 'Hen', kind: 'monster', x: 13, y: 10, hp: 100, maxHp: 100, dead: false };
  server.on('connection', socket => {
    let index;
    const send = (target, packet, payload) => { if (target.readyState === 1) target.send(JSON.stringify({ type: 'packet', packet, payload })); };
    const snapshot = () => socket.send(JSON.stringify({ type: 'worldSnapshot', payload: { mapFileName: '0', playerObjectId: active.has(index) ? 1000 : null,
      playerHp: 100, playerMaxHp: 100, playerCrystalStats: sparseStats ? [{ stat: 0, value: 1 }, { stat: 1, value: 5 }] : [{ stat: 14, value: 2 }],
      entities: [...active].map(([id, state]) => ({ ...state.player, objectId: id === index ? 1000 : state.player.objectId })).concat(monster),
      inventoryItems: [], equipmentItems: [] } }));
    send(socket, 'Connected', {});
    socket.on('message', data => {
      const input = JSON.parse(data); commands.push({ index, type: input.type, at: performance.now(),
        ...(input.type === 'attackV2' ? { requestId: input.requestId } : {}) });
      if (input.type === 'clientCapabilities' && input.capabilities.includes(ATTACK_OUTCOME_CAPABILITY)) socket.send(JSON.stringify({
        type: 'attackCapabilitiesV2', protocolVersion: 2, capability: ATTACK_OUTCOME_CAPABILITY, supported: true, scope: 'connection' }));
      if (input.type === 'clientVersion') { send(socket, 'ClientVersion', {}); snapshot(); }
      if (input.type === 'login') { index = input.accountId === actor(0).accountId ? 0 : 1; send(socket, 'LoginSuccess', { characters: [{ index, name: actor(index).name }] }); }
      if (input.type === 'startGame') {
        const player = { objectId: 50001 + index, kind: 'player', name: actor(index).name, x: index ? 12 : 8, y: index ? 10 : 8, direction: 'Right', level: sparseStats ? 3 : 7, hp: 100, maxHp: 100 };
        active.set(index, { player, socket }); send(socket, 'StartGame', { result: 4 }); snapshot();
        for (const [id, peer] of active) if (id !== index) send(peer.socket, 'ObjectPlayer', player);
      }
      if (['walk', 'run'].includes(input.type)) {
        const owner = active.get(index), step = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] }[input.direction];
        owner.player.x += step[0] * (input.type === 'run' ? 2 : 1); owner.player.y += step[1] * (input.type === 'run' ? 2 : 1);
        owner.player.direction = input.direction; const payload = { objectId: owner.player.objectId, location: { x: owner.player.x, y: owner.player.y }, direction: input.direction };
        send(socket, 'UserLocation', { ...payload.location, direction: input.direction });
        for (const [id, peer] of active) if (id !== index) setTimeout(() => send(peer.socket, input.type === 'run' ? 'ObjectRun' : 'ObjectWalk', payload), 900);
      }
      if (['attack', 'attackV2'].includes(input.type)) {
        const owner = active.get(index);
        if (noSwingFirstHit) {
          send(socket, 'UserLocation', { x: owner.player.x, y: owner.player.y, direction: owner.player.direction });
          monster.x++; monster.y++;
          for (const peer of active.values()) send(peer.socket, 'ObjectWalk', { objectId: 900,
            location: { x: monster.x, y: monster.y }, direction: 'DownRight' });
          return;
        }
        // Only the FIRST attack ever produces damage in the delayed case.
        // A second request must not claim that old 4.5s-late packet group.
        for (const [id, peer] of active) {
          const attackerId = id === index ? 1000 : owner.player.objectId;
          send(peer.socket, 'ObjectAttack', { objectId: attackerId, direction: owner.player.direction });
        }
        const deliver = () => {
          monster.hp -= 3;
          for (const [id, peer] of active) {
            const attackerId = id === index ? 1000 : owner.player.objectId;
            send(peer.socket, 'ObjectStruck', { objectId: 900, attackerId });
            send(peer.socket, 'DamageIndicator', { objectId: 900, damage: 3 });
            send(peer.socket, 'ObjectHealth', { objectId: 900, percent: monster.hp });
          }
        };
        if (v2Outcome) {
          if (v2Outcome !== 'launched-no-damage') deliver();
          const result = ['unavailable', 'unknown', 'notLaunched'].includes(v2Outcome) ? v2Outcome : 'launched';
          const response = { type: 'attackResultV2', protocolVersion: 2,
            requestId: v2Outcome === 'wrong-id' ? (BigInt(input.requestId) + 1n).toString() : input.requestId,
            objectId: input.objectId, result, ...(result === 'unavailable' ? { code: 'executionUnavailable' } : result === 'unknown' ? { code: 'outcomeUnknown' } : {}) };
          if (v2Outcome === 'late') setTimeout(() => { if (socket.readyState === 1) socket.send(JSON.stringify(response)); }, 1100);
          else if (v2Outcome !== 'missing') socket.send(JSON.stringify(response));
        } else if (!lateFirstHit) deliver();
        else if (commands.filter(command => command.type === 'attack').length === 1) setTimeout(deliver, 4500);
      }
      if (input.type === 'logOut') { active.delete(index); send(socket, 'LogOutSuccess', { characters: [] }); }
    });
  });
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native', movementIntervalMs: 650, stages: '2', combatActors: 1 });
  const context = { endpoint, policy, ...(v2Outcome ? { attackResults: 'v2' } : {}), runId: 'native-wire-fixture', phase: 'native-wire-fixture', clients: [],
    map: { width: 32, height: 32, blocked: new Uint8Array(1024) }, mapName: '0', monsterNames: ['Hen'], targetClaims: new Map(),
    guard: { assert() {}, stop: error => errors.push(error) }, writer: { add: row => evidence.push(row) }, add: row => rows.push(row) };
  const clients = [0, 1].map(index => new CapacityClient(endpoint, actor(index), context)); context.clients = clients;
  try {
    for (const [index, client] of clients.entries()) {
      await client.connect(); await client.request({ type: 'login', accountId: client.account.accountId, password: client.account.password }, 'LoginSuccess');
      await client.request({ type: 'startGame', characterIndex: index }, 'StartGame');
      client.role = index ? 'combat' : 'movement'; client.ready = true; client.home = { x: index ? 12 : 8, y: index ? 10 : 8 };
      client.nextNativeChatAt = Infinity; client.nextNativeTurnAt = Infinity;
    }
    if (!lateFirstHit && !sparseStats) {
      const [cluster] = validateNativeCombatClusters([{ combatActorIndices: [1], movementActorIndices: [0] }], clients.map(client => client.home), new Set([1]), 2);
      cluster.observerNames.add(clients[0].label); cluster.observerBindings[0].name = clients[0].label; clients[1].combatCluster = cluster;
    }
    const start = performance.now() + 1200; for (const client of clients) client.plan = new NativePlan(start);
    if (lateFirstHit || noSwingFirstHit || v2Outcome) {
      // Timers can wake a fraction early. Reach the real monotonic deadline
      // before offering the single attack; never advance the production clock.
      for (let attempts = 0; attempts < 100 && performance.now() < start; attempts++) {
        await sleep(Math.max(1, Math.min(25, start - performance.now())));
      }
      assert.ok(performance.now() >= start, 'bounded fixture wait reaches the actual legal deadline');
      offerNativeAction(clients[1], context); assert.ok(clients[1].pending); await clients[1].pending;
      assert.equal(rows.filter(row => row.kind === 'attack').length, 1);
      if (v2Outcome) {
        const actual = rows.find(row => row.kind === 'attack'), measured = new Measurements(); measured.add(actual);
        const trace = evidence.find(row => row.type === 'pendingAttackEvidence'), activity = measured.actors.get(clients[1].label);
        const status = v2Outcome === 'launched' ? 'success' : v2Outcome === 'notLaunched' ? 'not-launched' :
          v2Outcome === 'launched-no-damage' ? 'miss' : ['unavailable', 'unknown'].includes(v2Outcome) ? 'error' : 'timeout';
        assert.equal(actual.status, status); assert.equal(trace.outcome, status); assert.equal(trace.requestId, '1');
        assert.equal(actual.attackRequestSent, true); assert.equal(measured.summary().attack.attackV2Requests, 1);
        assert.equal(activity.attacks, ['launched', 'launched-no-damage'].includes(v2Outcome) ? 1 : 0);
        assert.equal(activity.hits, v2Outcome === 'launched' ? 1 : 0);
        assert.equal(activity.nativeCompleted, ['launched', 'launched-no-damage'].includes(v2Outcome) ? 1 : 0);
        assert.equal(commands.filter(command => command.type === 'attack').length, 0);
        if (['missing', 'wrong-id', 'late', 'launched-no-damage'].includes(v2Outcome)) {
          assert.ok(actual.latencyMs >= 890 && actual.latencyMs < 1800);
          assert.equal(clients[1].failedTargets.get(900).until, Infinity);
          assert.equal(actual.ambiguousTargetQuarantined, true);
        }
        if (v2Outcome === 'late') { await sleep(350); assert.equal(rows.find(row => row.kind === 'attack').status, 'timeout'); }
        if (v2Outcome === 'launched') {
          await sleep(Math.max(1, clients[1].plan.nextAt - performance.now() + 30));
          offerNativeAction(clients[1], context); await clients[1].pending;
          assert.deepEqual(commands.filter(command => command.type === 'attackV2').map(command => command.requestId), ['1', '2']);
          assert.equal(rows.filter(row => row.kind === 'attack' && row.status === 'success').length, 2);
        } else assert.equal(commands.filter(command => command.type === 'attackV2').length, 1, 'unknown/denied results are never retried by this opportunity');
        assert.equal(errors.length, ['unavailable', 'unknown'].includes(v2Outcome) ? 1 : 0);
        for (const client of clients) assert.equal(client.pendingAttackPacketTraces.size, 0);
        return;
      }
      assert.equal(rows.find(row => row.kind === 'attack').ambiguousTargetQuarantined, true);
      if (noSwingFirstHit) {
        const actual = rows.find(row => row.kind === 'attack'), trace = evidence.find(row => row.type === 'pendingAttackEvidence');
        assert.equal(actual.status, 'timeout'); assert.ok(actual.latencyMs >= 890 && actual.latencyMs < 1800);
        assert.equal(trace.outcome, 'timeout'); assert.equal(trace.truncated, false);
        assert.ok(trace.packets.some(row => row.packet === 'UserLocation' && row.payload.x === 12 && row.payload.y === 10));
        assert.ok(trace.packets.some(row => row.packet === 'ObjectWalk' && row.payload.objectId === 900));
        assert.equal(trace.packets.some(row => row.packet === 'ObjectAttack'), false);
        for (const client of clients) assert.equal(client.pendingAttackPacketTraces.size, 0);
        const measured = new Measurements(); measured.add(actual);
        for (let i = 0; i < 54; i++) measured.add({ ...actual, kind: i % 2 ? 'turn' : 'attack', status: 'success',
          damage: 3, sharedHealthVerified: true, latencyMs: 1, plannedLatencyMs: 1 });
        const verdict = assessMeasurements(measured, [{ name: clients[1].label, role: 'combat' }], 60, policy, { requireChat: false });
        assert.equal(verdict.timeoutRatio, 1 / 55); assert.equal(verdict.passed, false);
        assert.equal(clients[1].failedTargets.get(900).until, Infinity);
        assert.deepEqual(errors, []); return;
      }
      await sleep(3150); // Beyond the old 3s retry quarantine, before old damage arrives.
      assert.equal(combatCandidates(clients[1], context).some(target => target.objectId === 900), false);
      clients[1].plan = new NativePlan(performance.now()); offerNativeAction(clients[1], context); await clients[1].pending;
      for (const client of clients) await client.wait(() => client.events.some(event => event.packet === 'DamageIndicator' && event.payload.objectId === 900), 'real late old strike', 2500);
      await awaitPendingActions(clients);
      assert.equal(commands.filter(command => command.type === 'attack').length, 1);
      assert.equal(rows.some(row => row.kind === 'attack' && row.sharedHealthVerified), false);
      assert.equal(clients[1].failedTargets.get(900).until, Infinity);
      assert.equal(clients[1].combatHunting.attempts, 1); assert.equal(clients[1].combatHunting.positiveHits, 0);
      assert.deepEqual(clients[1].combatArrival.actual, clients[1].home);
      assert.deepEqual(errors, []); return;
    }
    while (performance.now() < start + 5500) {
      for (const client of clients) offerNativeAction(client, context);
      await sleep(20);
    }
    await awaitPendingActions(clients);
    assert.deepEqual(errors, []);
    const walkTimes = commands.filter(command => command.index === 0 && ['walk', 'run'].includes(command.type)).map(command => command.at);
    assert.ok(walkTimes.length >= 7); assert.ok(walkTimes.slice(1).some((time, i) => time - walkTimes[i] < 850), '900ms observer evidence does not stall legal 650ms movement');
    assert.equal(rows.some(row => row.status === 'busy'), false); assert.equal(rows.some(row => row.status === 'evidence-wait'), false);
    assert.ok(rows.some(row => row.kind === 'fanout-run' && row.status === 'success' && row.latencyMs >= 850));
    const attackTimes = commands.filter(command => command.index === 1 && command.type === 'attack').map(command => command.at);
    assert.ok(attackTimes.length >= 4);
    for (let i = 1; i < attackTimes.length; i++) assert.ok(attackTimes[i] - attackTimes[i - 1] >= (sparseStats ? 1358 : 1182));
    assert.ok(rows.filter(row => row.kind === 'attack' && row.sharedHealthVerified && row.damage === 3).length >= 4);
    assert.equal(evidence.filter(row => row.type === 'combatArrival').length, 1);
    assert.deepEqual(clients[1].combatArrival.actual, { x: 12, y: 10 });
    assert.equal(clients[1].combatHunting.attempts, attackTimes.length);
    assert.equal(clients[1].combatHunting.positiveHits, attackTimes.length);
    assert.deepEqual(clients[1].combatHunting.targetBounds, { minX: 13, maxX: 13, minY: 10, maxY: 10 });
    assert.equal(clients[1].combatHunting.maximumTargetDistanceFromHome, 1);
    const attackEvidence = evidence.filter(row => row.type === 'pendingAttackEvidence');
    assert.equal(attackEvidence.length, attackTimes.length); assert.ok(attackEvidence.every(row => row.outcome === 'success'));
    for (const client of clients) assert.equal(client.pendingAttackPacketTraces.size, 0);
    if (sparseStats) for (const row of rows.filter(row => row.kind === 'attack')) {
      assert.equal(row.pacingKnown, true); assert.equal(row.cadence.attackSpeedSource, 'authoritative-sparse-zero');
      assert.equal(row.cadence.level, 3); assert.equal(row.cadence.attackSpeed, 0); assert.equal(row.plannedIntervalMs, 1408);
    }
  } finally {
    await awaitPendingActions(clients);
    for (const client of clients) { client.draining = true; await client.close(); }
    for (const socket of server.clients) socket.terminate(); await new Promise(resolve => server.close(resolve));
  }
});
