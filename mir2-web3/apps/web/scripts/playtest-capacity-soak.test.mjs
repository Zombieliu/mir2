import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { capacityPolicy, validateCapacityPool, LoginBudget, LOGIN_WINDOW_MS, Histogram, Measurements,
  assessMeasurements, movementDiagnosticMayContinue, ContinuousAcceptance, NativePlan, nativeCadence, nativeActivityVerdict, AoiCoverage, verifiedCombatHit, BoundedEvidence, memoryTrend, TimeBucketSamples, sanitize } from './playtest-capacity-soak-core.mjs';
import { CapacityClient, FixedPlan, chooseHomes, validateNativeExplicitHomes, validateNativeCombatClusters, insideCombatCoverage, activeCombatCoverage, beginPendingAttackEvidence, recordPendingAttackPacket, finishPendingAttackEvidence, PENDING_ATTACK_PACKET_LIMIT, attackResultsMode, matchAttackResultV2, ATTACK_OUTCOME_CAPABILITY, loadStationaryNpcObstacles, spacingFourScenario, selectNativeAction, offerNativeAction, offerNativeHpRecovery, offerNativeClientWork, offerPreparationKeepAlive, ordinaryPreparationConfiguration, prepareCapacityActor, activatePreparedCohort, combatCandidates, recordCombatPursuitMovement, recordCombatScoutMovement, restoreCombatPursuit, stableFanoutRange, matchesFanoutReceipt, actionFailureStatus, awaitPendingActions, actorsReachedScenario, measure, runCapacity, parseArguments, patrol, action, navigationObstacles, navigationPlanIsClear, movementHintCells, claimNavigationHint, releaseNavigationHint, NAVIGATION_HINT_MAX_CELLS } from './playtest-capacity-soak.mjs';
import { observedPlayerId } from './playtest-multiplayer-smoke.mjs';
import { SafetyStop } from './playtest-load-smoke.mjs';
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

function hpRecoveryFixture(grid = 'inventory') {
  const fixture = huntFixture([]), { client, owner, context } = fixture, commands = [], stops = [];
  Object.assign(owner, { hp: 3, maxHp: 24, dead: false });
  Object.assign(client.snapshot, { playerHp: 3, playerMaxHp: 24, beltItems: [], inventoryItems: [] });
  client.snapshot[grid === 'belt' ? 'beltItems' : 'inventoryItems'].push({ uniqueId: 901, name: '(HP)DrugSmall', quantity: 2, container: grid });
  context.guard.stop = reason => stops.push(reason);
  client.ws = { send: data => commands.push(JSON.parse(data)) };
  const packet = (name, payload) => client.observe({ type: 'packet', packet: name, payload });
  const inventory = (quantity, complete = true) => {
    const snapshot = structuredClone(client.snapshot);
    snapshot.beltItems = []; snapshot.inventoryItems = [];
    if (quantity > 0) snapshot[grid === 'belt' ? 'beltItems' : 'inventoryItems'].push({ uniqueId: 901, name: '(HP)DrugSmall', quantity, container: grid });
    if (!complete) delete snapshot.inventoryItems;
    client.observe({ type: 'worldSnapshot', payload: snapshot });
  };
  const immediateWait = () => { client.wait = async (predicate, label, deadline) => {
    assert.equal(deadline, 2000); const result = predicate();
    if (!result) throw new Error(`timed out waiting for ${label}`); return result;
  }; };
  return { ...fixture, commands, stops, packet, inventory, immediateWait };
}

for (const grid of ['inventory', 'belt']) test(`native HP maintenance verifies ordinary ${grid} UID consumption without inventing healing or activity`, async () => {
  const f = hpRecoveryFixture(grid), { client, context, rows, commands, stops } = f;
  client.ws.send = data => { commands.push(JSON.parse(data)); f.packet('UseItem', { uniqueId: 901, grid, success: true }); f.inventory(1); };
  await offerNativeHpRecovery(client, context);
  assert.deepEqual(commands, [{ type: 'useItem', uniqueId: 901, grid }]); assert.deepEqual(stops, []);
  const proof = rows.find(row => row.type === 'nativeHpRecovery' && row.status === 'verified');
  assert.equal(proof.consumed, true); assert.equal(proof.hpIncreaseObserved, false); assert.equal(proof.hpIncrease, null);
  assert.equal(proof.consumption.quantity, 1); assert.equal(client.snapshot.playerHp, 3); assert.deepEqual(f.actions, []);
  assert.equal(client.hpRecoveryPending, null); assert.equal(client.hpRecoveryEvidence, null);
  assert.equal(offerNativeHpRecovery(client, context), null); assert.equal(commands.length, 1, 'same held stack cannot bypass the six-second cooldown');
  const restored = new CapacityClient('ws://127.0.0.1:1/ws', client.account, context);
  assert.equal(restored.journeyRestorativeUseAt, client.journeyRestorativeUseAt, 'resume shares the per-account monotonic cooldown');
  Object.assign(restored, { ready: true, inGame: true, snapshot: structuredClone(client.snapshot), ws: client.ws });
  assert.equal(offerNativeHpRecovery(restored, context), null); assert.equal(commands.length, 1);
});

test('native HP maintenance requires the matching successful ACK as well as a fresh exact HP increase', async () => {
  const f = hpRecoveryFixture(), { client, context, commands, rows } = f;
  client.ws.send = data => { commands.push(JSON.parse(data)); f.packet('HealthChanged', { hp: 8, maxHp: 24 });
    f.packet('UseItem', { uniqueId: 901, grid: 'Inventory', success: true }); };
  await offerNativeHpRecovery(client, context);
  const proof = rows.find(row => row.status === 'verified');
  assert.equal(proof.consumed, false); assert.equal(proof.hpIncreaseObserved, true); assert.equal(proof.hpIncrease.hp, 8);
  assert.equal(proof.acknowledgement.sequence > proof.hpIncrease.sequence, true, 'real effect may arrive before its ACK');
  assert.equal(commands.length, 1); assert.deepEqual(f.actions, []);
});

for (const failure of ['rejected', 'wrong-uid', 'wrong-grid', 'ack-only', 'effect-only', 'incomplete-inventory', 'late-proof']) {
  test(`native HP maintenance ${failure} cannot credit recovery and fails closed`, async () => {
    const f = hpRecoveryFixture(), { client, context, commands, rows, stops } = f; f.immediateWait();
    if (failure === 'late-proof') {
      const record = client.record.bind(client);
      client.record = (direction, message) => { const event = record(direction, message);
        if (direction === 'received') event.monotonicMs += 2001; return event; };
    }
    client.ws.send = data => {
      commands.push(JSON.parse(data));
      if (failure !== 'effect-only') f.packet('UseItem', { uniqueId: failure === 'wrong-uid' ? 902 : 901,
        grid: failure === 'wrong-grid' ? 'Belt' : 'Inventory', success: failure !== 'rejected' });
      if (failure !== 'ack-only') f.inventory(0, failure !== 'incomplete-inventory');
    };
    await offerNativeHpRecovery(client, context);
    assert.equal(commands.length, 1); assert.equal(stops.length, 1); assert.equal(rows.some(row => row.status === 'verified'), false);
    assert.equal(rows.at(-1).status, failure === 'rejected' ? 'rejected' : 'timeout'); assert.deepEqual(f.actions, []);
  });
}

test('native HP maintenance keeps unknown health, missing supplies and exhausted command budget bounded and never sends MP', () => {
  for (const reason of ['unknown-exact-health', 'no-held-hp-item', 'command-budget']) {
    const f = hpRecoveryFixture(), { client, context, rows, commands } = f;
    if (reason === 'unknown-exact-health') { delete client.snapshot.playerHp; delete f.owner.hp; }
    if (reason === 'no-held-hp-item') client.snapshot.inventoryItems = [{ uniqueId: 902, name: '(MP)DrugSmall', quantity: 1 }];
    if (reason === 'command-budget') for (let n = 0; n < 3; n++) client.budget.reserve();
    for (let n = 0; n < 100; n++) assert.equal(offerNativeHpRecovery(client, context), null);
    assert.equal(commands.length, 0); assert.equal(rows.length, 1); assert.equal(rows[0].reason, reason); assert.deepEqual(f.actions, []);
  }
  const dead = hpRecoveryFixture(); dead.owner.dead = true; dead.client.snapshot.playerHp = 0;
  assert.equal(offerNativeHpRecovery(dead.client, dead.context), null); assert.equal(dead.commands.length, 0);
});

test('native HP pending evidence retains offered gameplay and drain captures the existing dose', async () => {
  const f = hpRecoveryFixture(), { client, peer, context, commands, rows } = f;
  peer.paused = true; client.role = 'movement'; client.plan = new NativePlan(performance.now());
  client.ws.send = data => { const command = JSON.parse(data); commands.push(command);
    if (['walk', 'run'].includes(command.type)) {
      const delta = { Up: [0, -1], Right: [1, 0], Down: [0, 1], Left: [-1, 0] }[command.direction];
      assert.ok(delta); f.packet('UserLocation', { x: f.owner.x + delta[0] * (command.type === 'run' ? 2 : 1),
        y: f.owner.y + delta[1] * (command.type === 'run' ? 2 : 1), direction: command.direction });
    }
  };
  const dose = offerNativeHpRecovery(client, context); assert.ok(dose); let drained = false;
  const drain = awaitPendingActions([client]).then(() => { drained = true; });
  offerNativeAction(client, context);
  assert.equal(client.pending == null, true);
  assert.equal(f.actions.length, 1); assert.equal(f.actions[0].status, 'evidence-wait');
  assert.equal(f.actions[0].maintenanceReason, 'hp-recovery');
  assert.equal(drained, false); assert.equal(commands.length, 1); assert.equal(client.budget.sent.length, 1);
  f.packet('UseItem', { uniqueId: 901, grid: 'Inventory', success: true }); f.inventory(1);
  await dose; await drain; assert.equal(drained, true); assert.equal(rows.filter(row => row.status === 'verified').length, 1);
  const measurements = new Measurements(); for (const row of f.actions) measurements.add({ ...row, at: 1 });
  const activity = measurements.actors.get(client.label);
  assert.equal(activity.plans, 1); assert.equal(activity.nativeOpportunities, 1); assert.equal(activity.nativeCompleted, 0);
  assert.equal(activity.attacks, 0); assert.equal(activity.hits, 0);
  offerNativeAction(client, context, client.plan.nextAt); await client.pending;
  assert.ok(f.actions.some(row => ['walk', 'run'].includes(row.kind) && row.status === 'success' && row.nativeOpportunity));
  assert.equal(commands.length, 2); assert.equal(client.budget.sent.length, 2);
});

test('native engine offers HP before gameplay and auxiliary without hiding missed opportunities', async () => {
  const f = hpRecoveryFixture(), { client, context, commands, actions } = f;
  client.role = 'movement'; client.plan = new NativePlan(0); client.nextRtt = 0; client.nextRefresh = 0;
  offerNativeClientWork(client, context, 2250);
  assert.deepEqual(commands, [{ type: 'useItem', uniqueId: 901, grid: 'inventory' }]);
  assert.equal(client.pending == null, true); assert.equal(client.auxPending == null, true);
  assert.equal(actions.filter(row => row.status === 'plan-gap').length, 3);
  assert.equal(actions.at(-1).status, 'evidence-wait'); assert.equal(actions.at(-1).maintenanceReason, 'hp-recovery');
  const measurements = new Measurements(); for (const row of actions) measurements.add({ ...row, at: 1 });
  assert.equal(measurements.actors.get(client.label).nativeOpportunities, 4);
  assert.equal(measurements.actors.get(client.label).nativeCompleted, 0);
  f.packet('UseItem', { uniqueId: 901, grid: 'Inventory', success: true }); f.inventory(1);
  await client.hpRecoveryPending;
});

const preparationRoute = { quests: [{ objectives: { kill: [{ monsterName: 'Deer', spawnCandidates: [
  { mapFileName: '0', position: { x: 12, y: 12 }, count: 1 },
] }], item: [] } }] };

test('ordinary preparation is opt-in and requires distinct explicit per-actor staging instead of hostile homes', () => {
  assert.equal(ordinaryPreparationConfiguration(capacityPolicy({ profile: 'probe' }), {}), null);
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native', preparationProfile: 'ordinary-economic-v1' });
  const valid = { mapFileName: '0', preparation: { stagingPoints: [{ x: 5, y: 5 }, { x: 9, y: 5 }], route: preparationRoute } };
  assert.deepEqual(ordinaryPreparationConfiguration(policy, valid), valid.preparation);
  for (const preparation of [undefined, { stagingPoint: { x: 5, y: 5 }, route: preparationRoute },
    { stagingPoints: [{ x: 5, y: 5 }, { x: 5, y: 5 }], route: preparationRoute },
    { stagingPoints: [{ x: 5, y: 5 }, { x: 65536, y: 5 }], route: preparationRoute },
    { stagingPoints: valid.preparation.stagingPoints, route: { quests: [] } }]) {
    assert.throws(() => ordinaryPreparationConfiguration(policy, { ...valid, preparation }), /distinct explicit map-0 stagingPoints/);
  }
  assert.throws(() => ordinaryPreparationConfiguration(policy, { ...valid, mapFileName: '1' }), /map-0/);
  assert.deepEqual(parseArguments(['--target-actors', '500', '--preparation-profile', 'ordinary-economic-v1', '--preparation-hp-target', '12']),
    { targetActors: '500', preparationProfile: 'ordinary-economic-v1', preparationHpTarget: '12' });
});

function ordinaryPreparationFixture() {
  const f = hpRecoveryFixture(), { client, context, owner, commands } = f, joins = [];
  context.phase = 'preparation'; context.clients = [client];
  context.policy = capacityPolicy({ profile: 'probe', activityMode: 'native', preparationProfile: 'ordinary-economic-v1' });
  context.continuous = { join: (...args) => joins.push(args) };
  client.ready = false; owner.hp = 24; client.snapshot.playerHp = 24; client.snapshot.gold = 0;
  client.snapshot.inventoryItems = [{ uniqueId: 901, name: '(HP)DrugSmall', itemIndex: 658, quantity: 12, container: 'bag1', slot: 0 }];
  client.ws.send = data => {
    const command = JSON.parse(data); commands.push(command);
    assert.equal(command.type, 'clientVersion');
    client.observe({ type: 'worldSnapshot', payload: structuredClone(client.snapshot) });
  };
  return { ...f, joins, configuration: { stagingPoint: { x: 5, y: 5 }, route: preparationRoute } };
}

test('ordinary preparation consumes actual full snapshot receipts and joins only after a fresh cohort barrier', async () => {
  const f = ordinaryPreparationFixture(), { client, context, commands, joins, configuration } = f;
  const result = await prepareCapacityActor(client, context, configuration);
  assert.equal(result.status, 'ready'); assert.equal(result.after.hpStock, 12); assert.equal(result.actualStagingReached, true);
  assert.equal(client.ready, false); assert.equal(client.plan, undefined); assert.deepEqual(joins, []);
  assert.equal(client.events.findLast(event => event.type === 'worldSnapshot').payload.inventoryItems, undefined,
    'the normal event history remains compact; supply proof observed the actual full message');
  const preparedCursor = client.sequence;
  await activatePreparedCohort([client], context);
  assert.equal(client.ready, true); assert.ok(client.plan instanceof NativePlan);
  assert.equal(joins.length, 1); assert.equal(joins[0][0], client.label);
  assert.ok(client.sequence > preparedCursor); assert.equal(commands.length, 3); assert.equal(client.budget.sent.length, 3);
  assert.equal(context.writer != null, true); assert.deepEqual(f.actions, [], 'preparation cannot credit measured gameplay');
});

test('ordinary preparation navigator sends its actual Walk through the bounded supply facade and shared budget', async () => {
  const f = ordinaryPreparationFixture(), { client, context, commands, configuration } = f;
  configuration.stagingPoint = { x: 6, y: 5 };
  client.ws.send = data => {
    const command = JSON.parse(data); commands.push(command);
    if (command.type === 'walk') {
      assert.equal(command.direction, 'Right'); f.packet('UserLocation', { x: 6, y: 5, direction: 'Right' });
    } else {
      assert.equal(command.type, 'clientVersion'); client.observe({ type: 'worldSnapshot', payload: structuredClone(client.snapshot) });
    }
  };
  const result = await prepareCapacityActor(client, context, configuration);
  assert.equal(result.actualStagingReached, true); assert.equal(result.after.x, 6);
  assert.deepEqual(commands.map(command => command.type), ['clientVersion', 'walk', 'clientVersion']);
  assert.equal(result.commandCounts.walk, 1); assert.equal(result.movementCommands, 1);
  assert.equal(client.budget.sent.length, 3); assert.equal(client.ready, false); assert.deepEqual(f.actions, []);
});

test('ordinary preparation failure records sanitized module status and rethrows the same error without activation or credit', async () => {
  const f = ordinaryPreparationFixture(), { client, context, configuration, rows, joins, actions } = f;
  const credential = 'private-resume-credential'; client.recentSecrets.push(credential);
  const error = new Error(`shop failed ${client.account.password} ${credential}`);
  error.stack = 'private catch stack must never be recorded';
  const status = { profile: 'ordinary-economic-v1', status: 'failed', targetHp: 12, commands: 3,
    before: { hp: 24, hpStock: 0, gold: 0 }, after: null, actualStagingReached: false,
    failure: error.message, measurementCredit: false, sustainableDurationProven: false };
  let continued = false;
  await assert.rejects(async () => {
    await prepareCapacityActor(client, context, configuration, { createSupplies: () => ({
      prepare: async () => { throw error; }, status: () => status,
    }) });
    continued = true; await activatePreparedCohort([client], context);
  }, actual => actual === error);
  const failures = rows.filter(row => row.type === 'ordinaryPreparation');
  assert.equal(failures.length, 1); assert.equal(failures[0].status, 'failed');
  assert.equal(failures[0].result.commands, 3); assert.deepEqual(failures[0].result.before, status.before);
  assert.equal(failures[0].result.failure, 'shop failed [redacted] [redacted]');
  assert.equal(status.failure, error.message, 'diagnostics do not mutate module state or the original exception');
  const publicRows = JSON.stringify(rows);
  for (const secret of [client.account.password, credential, error.stack]) assert.equal(publicRows.includes(secret), false);
  assert.equal(continued, false); assert.equal(client.ready, false); assert.equal(client.plan, undefined);
  assert.deepEqual(joins, []); assert.deepEqual(actions, []); assert.deepEqual(f.commands, []);
});

test('ordinary cohort activation fails without a fresh receipt and never partially activates a cohort', async () => {
  const f = ordinaryPreparationFixture(), { client, context, configuration, joins } = f;
  await prepareCapacityActor(client, context, configuration);
  client.refresh = async () => client.snapshot;
  await assert.rejects(activatePreparedCohort([client], context), /fresh cohort readiness failed/);
  assert.equal(client.ready, false); assert.deepEqual(joins, []);
  let validated = false;
  const first = { label: 'prepared', role: 'movement', sequence: 4, ready: false, pace: async () => {},
    refresh: async () => {}, ordinarySupplies: { assertReady: ({ afterSequence }) => { assert.equal(afterSequence, 4); validated = true; } } };
  const second = { ...first, label: 'lost-stock', ordinarySupplies: { assertReady: () => { throw new Error('stock lost'); } } };
  await assert.rejects(activatePreparedCohort([first, second], context), /stock lost/);
  assert.equal(validated, true); assert.equal(first.ready, false); assert.equal(second.ready, false); assert.deepEqual(joins, []);
});

test('ordinary preparation rejects active actors, blocked staging and transfer staging before sending', async () => {
  for (const boundary of ['active', 'blocked', 'transfer']) {
    const f = ordinaryPreparationFixture(), { client, context, configuration } = f;
    if (boundary === 'active') client.ready = true;
    if (boundary === 'blocked') context.map.blocked[5 * context.map.width + 5] = 1;
    if (boundary === 'transfer') client.snapshot.mapTransfers = [{ bounds: { minX: 5, maxX: 5, minY: 5, maxY: 5 } }];
    await assert.rejects(prepareCapacityActor(client, context, configuration));
    assert.equal(f.commands.length, 0); assert.deepEqual(f.joins, []);
  }
});

test('ordinary preparation keepAlive uses the original budget only after ready while funding and HP evidence retain control', async () => {
  const f = ordinaryPreparationFixture(), { client, context, commands } = f;
  let state = 'funding'; client.ordinarySupplies = { status: () => ({ status: state }) };
  client.nextRtt = 0; client.nextRefresh = Infinity;
  client.ws.send = data => { const command = JSON.parse(data); commands.push(command);
    f.packet('KeepAlive', { time: command.time }); };
  offerPreparationKeepAlive(client, context); assert.equal(commands.length, 0);
  state = 'ready'; client.hpRecoveryEvidence = {};
  offerPreparationKeepAlive(client, context); assert.equal(commands.length, 0);
  client.hpRecoveryEvidence = null;
  offerPreparationKeepAlive(client, context); await client.auxPending;
  assert.equal(commands.length, 1); assert.equal(commands[0].type, 'keepAlive'); assert.equal(client.budget.sent.length, 1);
  assert.equal(client.ready, false); assert.deepEqual(f.joins, []);
  assert.equal(f.actions.length, 1); assert.equal(f.actions[0].kind, 'rtt'); assert.equal(f.actions[0].nativeOpportunity, undefined);
  client.nextRtt = 0; for (let n = 0; n < 2; n++) client.budget.reserve();
  offerPreparationKeepAlive(client, context); assert.equal(commands.length, 1, 'maintenance cannot exceed the account budget');
  client.snapshot.playerHp = 0; client.snapshot.entities[0].dead = true;
  assert.throws(() => offerPreparationKeepAlive(client, context), /no automatic resurrection/);
  assert.equal(commands.length, 1, 'inactive preparation retains the original owner death stop');
});

test('500 actor opt-in reaches explicit home and navigation validation while legacy callers keep their ceiling', () => {
  const map = { width: 110, height: 90, blocked: new Uint8Array(9900) };
  const anchors = Array.from({ length: 500 }, (_, index) => ({ x: 2 + index % 25 * 4, y: 2 + Math.floor(index / 25) * 4 }));
  assert.throws(() => validateNativeExplicitHomes(map, { x: 1, y: 1 }, 500, anchors));
  const policy = capacityPolicy({ targetActors: 500 });
  const proof = validateNativeExplicitHomes(map, { x: 1, y: 1 }, 500, anchors, 2, [], [], policy);
  assert.equal(proof.homes.length, 500); assert.equal(proof.footprintCells, 4500);
  const f = huntFixture([]), { client, context } = f;
  context.clients = [client, ...Array.from({ length: 499 }, () => ({ ready: false }))];
  assert.throws(() => navigationObstacles(client), /100 actor ceiling/);
  context.policy = policy;
  assert.doesNotThrow(() => navigationObstacles(client));
  context.clients.push({ ready: false });
  assert.throws(() => navigationObstacles(client), /declared actor ceiling/);
});

test('ordinary supply commands reach the capacity wire with the original budget and no QA command admission', () => {
  for (const command of [{ type: 'harvest', objectId: 123 }, { type: 'interact', objectId: 456 },
    { type: 'selectNpcDialog', key: '@buysell' }, { type: 'sellItem', uniqueId: 7, count: 1 },
    { type: 'buyItem', itemIndex: 658, count: 12, panelType: 0 }]) {
    const f = hpRecoveryFixture(); f.client.send(command);
    assert.deepEqual(f.commands, [command]); assert.equal(f.client.budget.sent.length, 1);
  }
  for (const type of ['qa.giveItem', 'event.spawn', 'qa.openStorage', 'moveTo', 'passkeyLogin']) {
    const f = hpRecoveryFixture(); assert.throws(() => f.client.send({ type }), /Not an allowed/);
    assert.deepEqual(f.commands, []);
  }
});

function prepareHuntWitness(fixture, target) {
  const { client, peer, owner } = fixture;
  assert.equal(peer.ready, true); assert.equal(peer.inGame, true);
  assert.ok(peer.snapshot.entities.some(entity => entity.objectId === target.objectId));
  const attacker = peer.snapshot.entities.find(entity => entity.name === client.label);
  assert.ok(attacker);
  // These pursuit fixtures model a moving ordinary observer as well as the
  // escaping target, so they continue testing budgets with valid witnesses.
  Object.assign(peer.snapshot.entities[0], { x: target.x + 2, y: target.y });
  Object.assign(attacker, { x: owner.x, y: owner.y });
  peer.paused = false; peer.events ??= []; peer.sequence ??= 0;
}

async function acknowledgeSentChase(fixture, moveTarget = true, prepared = undefined) {
  const { client, peer, owner, context } = fixture;
  prepareHuntWitness(fixture, client.snapshot.entities.find(entity => entity.objectId === client.combatPursuit.objectId));
  client.plan = { sent() {} };
  const plan = prepared ?? selectNativeAction(client, context), origin = { ...owner };
  assert.ok(['walk', 'run'].includes(plan.command.type));
  client.send = command => {
    assert.equal(command.type, plan.command.type);
    if (moveTarget) {
      const target = client.snapshot.entities.find(entity => entity.objectId === plan.chaseTargetId);
      target.x += plan.target.x - origin.x; target.y += plan.target.y - origin.y;
    }
    client.observe({ type: 'packet', packet: 'UserLocation', payload: { ...plan.target, direction: command.direction } });
  };
  await action(client, context, { due: performance.now(), cadence: { cooldownMs: 600, intervalMs: 750, pacingKnown: true } }, plan);
  return plan;
}

test('native post-strike same-target chase starts once at a real send and again obeys nonprogress limits', async () => {
  const f = huntFixture([huntMonster(900, 'Deer', 10)]), { client, context, rows } = f;
  selectNativeAction(client, context); const old = client.combatPursuit;
  Object.assign(old, { attackSent: true, confirmedMoves: 39, startedAt: Date.now() - 90000 });
  await acknowledgeSentChase(f);
  const segment = client.combatPursuit;
  assert.notEqual(segment, old); assert.equal(segment.attackSent, false); assert.equal(segment.confirmedMoves, 1);
  for (let n = 0; n < 20; n++) selectNativeAction(client, context);
  assert.equal(client.combatPursuit, segment); assert.equal(segment.confirmedMoves, 1, 'replans cannot reset or consume its budget');
  for (let n = 0; n < 3; n++) await acknowledgeSentChase(f);
  assert.equal(client.combatPursuit, null); assert.equal(rows.at(-1).reason, 'no-progress');
  assert.equal(rows.at(-1).confirmedMoves, 4); assert.equal(client.chaseDeferrals.has(900), true);
});

for (const reason of ['movement-budget', 'time-budget', 'route-loop']) test(`native post-strike segment retains the original ${reason} boundary`, async () => {
  const f = huntFixture([huntMonster(900, 'Deer', 10)]), { client, context, rows } = f;
  selectNativeAction(client, context); client.combatPursuit.attackSent = true;
  await acknowledgeSentChase(f);
  const segment = client.combatPursuit, plan = selectNativeAction(client, context);
  if (reason === 'movement-budget') segment.confirmedMoves = 39;
  if (reason === 'time-budget') segment.startedAt = Date.now() - 30000;
  if (reason === 'route-loop') segment.visited.set(`${plan.target.x},${plan.target.y}`, 2);
  await acknowledgeSentChase(f, true, plan);
  assert.equal(client.combatPursuit, null); assert.equal(rows.at(-1).reason, reason);
  assert.equal(client.chaseDeferrals.size, 1); assert.equal(client.failedTargets.size, 0);
});

test('native post-strike planning, pending work, budget gaps, navigation rejection and failed sends never reset a chase', async () => {
  for (const boundary of ['standing', 'pending', 'budget', 'navigation', 'send-failed']) {
    const target = huntMonster(900, 'Deer', boundary === 'standing' ? 6 : 10), f = huntFixture([target]);
    const { client, context } = f; const plan = selectNativeAction(client, context), old = client.combatPursuit;
    old.attackSent = true; old.confirmedMoves = 37;
    if (boundary === 'pending') client.pending = Promise.resolve();
    for (let n = 0; n < 5; n++) selectNativeAction(client, context);
    assert.equal(client.combatPursuit, old);
    if (['budget', 'navigation', 'send-failed'].includes(boundary)) {
      client.plan = { sent() {} }; client.send = () => { throw new Error('synthetic send failure'); };
      if (boundary === 'budget') for (let n = 0; n < 3; n++) client.budget.reserve();
      if (boundary === 'navigation') context.map.blocked[plan.first.y * context.map.width + plan.first.x] = 1;
      await action(client, context, { due: performance.now(), cadence: { cooldownMs: 600 } }, plan);
    }
    assert.equal(client.combatPursuit, old); assert.equal(old.attackSent, true); assert.equal(old.confirmedMoves, 37);
  }
});

function attackWitnessFixture() {
  const target = huntMonster(900, 'Deer', 31, 30), f = huntFixture([target]), { client, peer, owner, context } = f;
  Object.assign(owner, { x: 30, y: 30, direction: 'Right' }); client.home = { x: 30, y: 30 };
  Object.assign(peer.snapshot.entities[0], { x: 15, y: 30 });
  Object.assign(peer.snapshot.entities[1], { x: 30, y: 30 });
  const near = { label: actor(6).name, inGame: true, ready: true, role: 'movement', sequence: 80, events: [],
    snapshot: { mapFileName: '0', playerObjectId: 1000, entities: [
      { objectId: 1000, name: actor(6).name, kind: 'player', x: 25, y: 30, hp: 24, dead: false },
      { ...owner, objectId: 50006 }, structuredClone(target) ] } };
  context.clients = [client, peer, near];
  return { ...f, near, target };
}

test('native attack witness skips the first AOI-edge peer and requires both real objects on a nearby living connection', () => {
  const f = attackWitnessFixture(); let plan = selectNativeAction(f.client, f.context);
  assert.equal(plan.command.type, 'attack'); assert.equal(plan.observer, f.near); assert.equal(plan.peerAttackerId, 50006);
  for (const missing of ['target', 'attacker', 'dead', 'wrong-map', 'not-ready']) {
    const g = attackWitnessFixture();
    if (missing === 'target') g.near.snapshot.entities = g.near.snapshot.entities.filter(entity => entity.objectId !== 900);
    if (missing === 'attacker') g.near.snapshot.entities = g.near.snapshot.entities.filter(entity => entity.name !== g.client.label);
    if (missing === 'dead') g.near.snapshot.entities[0].dead = true;
    if (missing === 'wrong-map') g.near.snapshot.mapFileName = 'other';
    if (missing === 'not-ready') g.near.ready = false;
    plan = selectNativeAction(g.client, g.context);
    assert.ok(['walk', 'run'].includes(plan.command.type), missing); assert.equal(plan.observer, undefined, missing);
  }
});

test('native attack witness uses nearest eligible peer with original order on equal separation', () => {
  const f = attackWitnessFixture(), second = structuredClone(f.near);
  second.label = actor(7).name; second.snapshot.entities[0].name = second.label; f.context.clients.push(second);
  assert.equal(selectNativeAction(f.client, f.context).observer, f.near);
  second.snapshot.entities[0].x = 28;
  assert.equal(selectNativeAction(f.client, f.context).observer, second);
});

test('native attack witness is reselected with its current namespace and cursor immediately before the real send', async () => {
  const f = attackWitnessFixture(), { client, context, near, owner, target, actions } = f;
  const first = structuredClone(near); first.label = actor(7).name; first.snapshot.entities[0].name = first.label;
  first.snapshot.entities[0].x = 28; first.snapshot.entities[1].objectId = 50007;
  context.clients.splice(1, 0, first);
  const plan = selectNativeAction(client, context); assert.equal(plan.observer, first);
  first.snapshot.entities[0].x = 15; client.plan = { sent() {} }; let actualReceiver;
  const peerPacket = (packet, payload) => near.events.push({ direction: 'received', type: 'packet', packet, payload,
    sequence: ++near.sequence, monotonicMs: performance.now() });
  client.send = command => {
    assert.equal(command.type, 'attack'); actualReceiver = client.pendingAttackEvidence.receivers.find(receiver => receiver.role === 'observer');
    for (const [packet, extra] of [['ObjectStruck', { attackerId: owner.objectId }], ['DamageIndicator', { damage: 2 }], ['ObjectHealth', { percent: 90 }]]) {
      client.observe({ type: 'packet', packet, payload: { objectId: target.objectId, ...extra } });
      peerPacket(packet, { objectId: target.objectId, ...extra, ...(packet === 'ObjectStruck' ? { attackerId: 50006 } : {}) });
    }
  };
  await action(client, context, { due: performance.now(), cadence: { cooldownMs: 600, intervalMs: 750, pacingKnown: true } }, plan);
  assert.equal(actualReceiver.client, near); assert.equal(actualReceiver.cursor, 80); assert.equal(actualReceiver.attackerId, 50006);
  assert.equal(actions.find(row => row.kind === 'attack').status, 'success'); assert.equal(actions.find(row => row.kind === 'attack').sharedHealthVerified, true);
});

test('native attack witness loss before send retains the original incomplete opportunity without a fake attack', async () => {
  const f = attackWitnessFixture(), { client, context, near, actions } = f, sent = [], metrics = new Measurements();
  const plan = selectNativeAction(client, context); assert.equal(plan.observer, near);
  near.snapshot.entities[0].x = 15; client.send = command => sent.push(command); client.plan = { sent() {} };
  await action(client, context, { due: performance.now(), intervalMs: 750,
    cadence: { cooldownMs: 600, intervalMs: 750, pacingKnown: true } }, plan);
  assert.equal(sent.length, 0); assert.equal(actions.length, 1); assert.equal(actions[0].status, 'evidence-wait');
  metrics.add({ ...actions[0], at: 1 }); const counts = metrics.actors.get(client.label);
  assert.equal(counts.nativeOpportunities, 1); assert.equal(counts.nativeCompleted, 0); assert.equal(counts.plans, 1);
  assert.equal(counts.attacks, 0); assert.equal(counts.hits, 0);
  client.plan = new NativePlan(performance.now());
  offerNativeAction(client, context);
  assert.equal(sent.length, 1); assert.ok(['walk', 'run'].includes(sent[0].type));
  await Promise.resolve(); assert.equal(actions.length, 1, 'planning and sending a fallback do not prove movement');
  const [dx, dy] = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1],
    Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] }[sent[0].direction];
  const owner = client.snapshot.entities.find(entity => entity.objectId === 1000), amount = sent[0].type === 'run' ? 2 : 1;
  client.observe({ type: 'packet', packet: 'UserLocation', payload: { x: owner.x + dx * amount, y: owner.y + dy * amount,
    direction: sent[0].direction } });
  await client.pending;
  assert.equal(actions[1].status, 'success'); assert.equal(actions[1].nativeOpportunity, true);
  metrics.add({ ...actions[1], at: 2 });
  assert.equal(counts.nativeOpportunities, 2); assert.equal(counts.nativeCompleted, 1); assert.equal(counts.attacks, 0); assert.equal(counts.hits, 0);
});

test('native initial no-witness selection can use another genuinely witnessed adjacent target', () => {
  const f = attackWitnessFixture(), alternate = huntMonster(901, 'Scarecrow', 30, 31);
  f.client.snapshot.entities.push(alternate); f.owner.direction = 'Down';
  f.near.snapshot.entities = f.near.snapshot.entities.filter(entity => entity.objectId !== 900);
  f.near.snapshot.entities.push(structuredClone(alternate));
  const plan = selectNativeAction(f.client, f.context);
  assert.equal(plan.command.type, 'attack'); assert.equal(plan.command.objectId, 901); assert.equal(plan.observer, f.near);
});

test('native initial no-witness target with no approach route retains the original bounded scout fallback', () => {
  const f = attackWitnessFixture(), { client, near, owner, context } = f;
  near.snapshot.entities = near.snapshot.entities.filter(entity => entity.objectId !== 900);
  client.combatCluster = { observerHomes: [{ x: 25, y: 30 }], patrolSpan: 2, observerNames: new Set([near.label]),
    observerBindings: [{ actorIndex: 6, homeIndex: 0, home: { x: 25, y: 30 }, name: near.label }] };
  context.clients = [client, near];
  for (let y = 29; y <= 31; y++) for (let x = 30; x <= 32; x++) {
    if (x !== owner.x || y !== owner.y) context.map.blocked[y * context.map.width + x] = 1;
  }
  const plan = selectNativeAction(client, context);
  assert.equal(plan.scouting, true); assert.ok(['walk', 'run'].includes(plan.command.type)); assert.equal(plan.chaseTargetId, undefined);
  assert.equal(context.policy.minimumCombatAttacksPerMinute, 18); assert.equal(context.policy.minimumCombatHitsPerMinute, 12);
});

function approachWitnessFixture(ownerPoint, targetPoint, peerPoints, targetId = 900) {
  const target = huntMonster(targetId, 'Deer', targetPoint.x, targetPoint.y), f = huntFixture([target]);
  Object.assign(f.owner, ownerPoint); f.client.home = { ...ownerPoint };
  f.context.map = { width: 320, height: 700, blocked: new Uint8Array(320 * 700) };
  const peers = peerPoints.map(([x, y], index) => ({ label: actor(index + 10).name, ready: true, inGame: true, role: 'movement',
    snapshot: { mapFileName: '0', playerObjectId: 1000, entities: [
      { objectId: 1000, name: actor(index + 10).name, kind: 'player', x, y, hp: 30, dead: false },
      { ...f.owner, objectId: 50010 + index }, structuredClone(target) ] } }));
  f.context.clients = [f.client, ...peers];
  f.client.combatCluster = { observerHomes: peerPoints.map(([x, y]) => ({ x, y })), patrolSpan: 4,
    observerNames: new Set(peers.map(peer => peer.label)), observerBindings: peers.map((peer, index) => ({
      actorIndex: index + 10, homeIndex: index, home: { x: peerPoints[index][0], y: peerPoints[index][1] }, name: peer.label })) };
  return { ...f, target, peers };
}

// Exact ordinary owner/target/peer positions from the four probe10 deferrals.
// The open map models geometry only; it does not replace the original terrain
// or assert the per-decision AOI content that was not retained by that run.
for (const geometry of [
  { targetId: 206513, owner: { x: 274, y: 661 }, target: { x: 274, y: 662 }, peers:
    [[265, 654], [273, 672], [277, 672], [261, 650], [259, 665], [256, 669], [255, 663], [255, 651], [254, 669]] },
  { targetId: 206704, owner: { x: 282, y: 663 }, target: { x: 282, y: 662 }, peers:
    [[273, 670], [276, 671], [267, 654], [259, 653], [257, 665], [256, 672], [255, 666], [253, 673], [253, 655]] },
  { targetId: 206507, owner: { x: 283, y: 665 }, target: { x: 283, y: 666 }, peers:
    [[278, 673], [273, 670], [267, 654], [259, 653], [257, 668], [256, 663], [255, 671], [255, 666], [255, 653]] },
  { targetId: 206715, owner: { x: 285, y: 665 }, target: { x: 284, y: 665 }, peers:
    [[276, 671], [273, 672], [265, 654], [260, 665], [259, 668], [259, 651], [255, 664], [254, 655], [251, 669]] },
]) test(`native witnessed approach avoids the probe10 unobservable ${geometry.targetId} neighbour loop`, () => {
  const f = approachWitnessFixture(geometry.owner, geometry.target, geometry.peers, geometry.targetId);
  const plan = selectNativeAction(f.client, f.context);
  assert.ok(['walk', 'run'].includes(plan.command.type)); assert.equal(plan.scouting, true);
  assert.equal(plan.chaseTargetId, undefined); assert.equal(f.client.combatPursuit, null);
  assert.equal(f.client.chaseDeferrals.size, 0); assert.equal(f.client.failedTargets.size, 0);
  assert.equal(f.context.policy.minimumCombatAttacksPerMinute, 18); assert.equal(f.context.policy.minimumCombatHitsPerMinute, 12);
});

test('native witnessed approach skips cheap ineligible targets before the original four path-target limit', () => {
  const monsters = [6, 7, 8, 9].map((x, i) => huntMonster(900 + i, 'Deer', x));
  const alternate = huntMonster(905, 'Scarecrow', 12), f = huntFixture([...monsters, alternate]);
  Object.assign(f.peer.snapshot.entities[0], { x: 18, y: 5 });
  const plan = selectNativeAction(f.client, f.context);
  assert.ok(['walk', 'run'].includes(plan.command.type)); assert.equal(plan.chaseTargetId, 905);
  const peer = f.peer.snapshot.entities[0], goal = plan.navigation.goal;
  assert.ok(Math.max(Math.abs(peer.x - f.owner.x), Math.abs(peer.y - f.owner.y)) > 6,
    'a future attack tile may be witnessed before the current owner enters the six-cell range');
  assert.ok(Math.max(Math.abs(peer.x - goal.x), Math.abs(peer.y - goal.y)) <= 6);
  assert.ok(Math.max(Math.abs(peer.x - alternate.x), Math.abs(peer.y - alternate.y)) <= 6);
  assert.equal(plan.command.type, 'walk'); assert.equal(f.client.combatPursuit.objectId, 905);
});

for (const unavailable of ['not-ready', 'dead', 'wrong-map', 'missing-attacker', 'missing-target']) {
  test(`native witnessed approach rejects a ${unavailable} observer without replacing real membership with distance`, () => {
    const f = huntFixture([huntMonster(900, 'Deer', 10)]);
    if (unavailable === 'not-ready') f.peer.ready = false;
    if (unavailable === 'dead') f.peer.snapshot.entities[0].dead = true;
    if (unavailable === 'wrong-map') f.peer.snapshot.mapFileName = 'other';
    if (unavailable === 'missing-attacker') f.peer.snapshot.entities = f.peer.snapshot.entities.filter(e => e.name !== f.client.label);
    if (unavailable === 'missing-target') f.peer.snapshot.entities = f.peer.snapshot.entities.filter(e => e.objectId !== 900);
    const plan = selectNativeAction(f.client, f.context);
    assert.ok(['walk', 'run'].includes(plan.command.type)); assert.equal(plan.chaseTargetId, undefined);
    assert.equal(f.client.combatPursuit, null); assert.equal(f.actions.length, 0);
  });
}

test('native witnessed approach retains the original four path targets and eight adjacent destinations', () => {
  const monsters = [2, 4, 6, 8].map((y, i) => huntMonster(900 + i, 'Deer', 10, y));
  const alternate = huntMonster(905, 'Scarecrow', 11), f = huntFixture([...monsters, alternate]);
  Object.assign(f.peer.snapshot.entities[0], { x: 9, y: 5 });
  for (const monster of monsters) for (let y = monster.y - 1; y <= monster.y + 1; y++) for (let x = 9; x <= 11; x++) {
    f.context.map.blocked[y * f.context.map.width + x] = 1;
  }
  const plan = selectNativeAction(f.client, f.context);
  assert.equal(plan.chaseTargetId, undefined, 'the fifth eligible target does not expand the original search budget');
  assert.ok(f.client.navigationAttemptCount <= 36, 'at most32 target paths plus the original four-corner fallback');
  assert.equal(f.client.navigationAttempts.some(attempt => attempt.target.x === 12), false);
});

test('native witnessed approach metadata is immutable and does not start gameplay without its original monitor', async () => withDirectory(async directory => {
  const report = await runCapacity({ realm: 'isolated', endpoint: 'ws://127.0.0.1:1/ws',
    accountsFile: path.join(directory, 'private', 'absent.json'), monitor: path.join(directory, 'absent-monitor.json'),
    output: path.join(directory, 'reports'), profile: 'probe', activityMode: 'native' });
  assert.equal(report.ok, false); assert.equal(Object.isFrozen(report.nativeAttackWitness), true);
  assert.match(report.nativeAttackWitness.approachRule, /actual snapshot contains both attacker and target/);
  assert.match(report.nativeAttackWitness.approachRule, /prospective attack tile/);
  assert.equal(Object.getOwnPropertyDescriptor(report, 'nativeAttackWitness').writable, false);
  assert.throws(() => { report.nativeAttackWitness.approachRule = 'ignore membership'; }, TypeError);
  assert.deepEqual(report.bytes, { sent: 0, received: 0 });
}));

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

function patrolFallbackFixture(options = {}) {
  const fixture = navigationFixture(), { clients: [client], context } = fixture;
  const home = options.home ?? { x: 5, y: 5 }, origin = options.origin ?? { x: 7, y: 4 };
  const blocked = options.obstacles ?? [{ x: 6, y: 5 }, { x: 6, y: 6 }, { x: 5, y: 6 }];
  const owner = { ...client.snapshot.entities[0], ...origin };
  client.snapshot.entities = [owner, ...blocked.map((point, index) => ({ objectId: 900 + index, kind: 'monster', name: 'Scarecrow', hp: 20, ...point }))];
  Object.assign(client, { home, targetCursor: options.cursor ?? 0, nativeRunPrimedUntil: Infinity });
  Object.assign(context, { map: options.map ?? context.map, patrolSpan: 2, clients: [client] });
  return { ...fixture, client, owner };
}

test('native primed fallback chooses a real Run from an existing corner under two observed approximate Scarecrow arrangements', async () => {
  const map = await loadProtocolCollisionMap('0');
  // These nearby public positions approximate two formal03 frames. They are
  // portable obstacle fixtures, not a reconstruction of its send-time AOI.
  for (const setup of [
    { origin: { x: 254, y: 670 }, cursor: 0, obstacles: [{ x: 253, y: 671 }, { x: 253, y: 672 }, { x: 252, y: 672 }],
      oldGoal: { x: 254, y: 671 }, goal: { x: 254, y: 673 }, first: { x: 254, y: 671 }, target: { x: 254, y: 672 }, cursorAfter: 1 },
    { origin: { x: 252, y: 674 }, cursor: 2, obstacles: [{ x: 252, y: 671 }, { x: 254, y: 672 }, { x: 253, y: 673 }],
      oldGoal: { x: 252, y: 673 }, goal: { x: 254, y: 671 }, first: { x: 252, y: 673 }, target: { x: 252, y: 672 }, cursorAfter: 0 },
  ]) {
    const { client, context, owner } = patrolFallbackFixture({ ...setup, map, home: { x: 252, y: 671 } });
    const oldFirstRoute = findProtocolWalkPath({ map, start: owner, target: setup.oldGoal,
      dynamicObstacles: client.snapshot.entities.slice(1), maxExpanded: 4000 });
    assert.equal(oldFirstRoute.length, 2, 'the original first feasible corner really offers only one Walk step');
    const before = JSON.stringify(client.snapshot), plan = patrol(client, context);
    assert.equal(plan.command.type, 'run'); assert.deepEqual(plan.first, setup.first); assert.deepEqual(plan.target, setup.target);
    assert.deepEqual(plan.navigation.goal, setup.goal); assert.equal(client.targetCursor, setup.cursorAfter);
    assert.equal(navigationPlanIsClear(client, plan), true); assert.equal(JSON.stringify(client.snapshot), before);
  }
});

test('native primed fallback keeps the original first Walk when no original corner route has a legal Run', () => {
  const { client, context, owner } = patrolFallbackFixture({ origin: { x: 5, y: 5 } });
  const goals = [{ x: 7, y: 5 }, { x: 7, y: 7 }, { x: 5, y: 7 }, { x: 5, y: 5 }];
  for (const target of goals.filter(point => point.x !== owner.x || point.y !== owner.y)) {
    const route = findProtocolWalkPath({ map: context.map, start: owner, target, dynamicObstacles: client.snapshot.entities.slice(1), maxExpanded: 4000 });
    assert.ok(route?.length >= 2);
    assert.ok(route.length < 3 || route[2].x - route[1].x !== route[1].x - owner.x || route[2].y - route[1].y !== route[1].y - owner.y);
  }
  const plan = patrol(client, context);
  assert.equal(plan.command.type, 'walk'); assert.deepEqual(plan.target, { x: 5, y: 4 }); assert.equal(client.targetCursor, 0);
});

test('native fallback does not prefer later Run routes when unprimed and leaves baseline first-path selection unchanged', () => {
  for (const mode of ['native', 'baseline']) {
    const { client, context } = patrolFallbackFixture();
    if (mode === 'native') client.nativeRunPrimedUntil = 0;
    else { context.policy = capacityPolicy({ profile: 'probe', activityMode: 'baseline', stages: '2' }); client.movementCount = 9; }
    const plan = patrol(client, context);
    assert.equal(plan.command.type, 'walk', mode); assert.deepEqual(plan.target, { x: 7, y: 5 }, mode); assert.equal(client.targetCursor, 0);
  }
});

test('native fallback Run preference retains controlled-owner and pending-path obstacles without changing snapshots', () => {
  for (const mode of ['owner', 'hint']) {
    const { client, context, clients: [, peer] } = patrolFallbackFixture();
    peer.snapshot.entities = [{ ...peer.snapshot.entities[0], x: 7, y: mode === 'owner' ? 6 : 8 }];
    context.clients = [client, peer];
    const hint = mode === 'hint' ? claimNavigationHint(peer, { command: { type: 'run', direction: 'Up' }, first: { x: 7, y: 7 }, target: { x: 7, y: 6 } }) : null;
    if (mode === 'hint') assert.ok(hint);
    const before = JSON.stringify(client.snapshot), plan = patrol(client, context);
    assert.ok(plan); assert.notDeepEqual(plan.target, { x: 7, y: 6 }); assert.equal(navigationPlanIsClear(client, plan), true);
    assert.equal(JSON.stringify(client.snapshot), before); releaseNavigationHint(peer, hint);
  }
});

test('native fallback Run preference retains diagonal side-cell collision checks', () => {
  const { client, context } = patrolFallbackFixture({ origin: { x: 3, y: 3 }, cursor: 1, obstacles: [] });
  const direct = patrol(client, context); assert.equal(direct.command.type, 'run'); assert.deepEqual(direct.target, { x: 5, y: 5 });
  client.targetCursor = 1; context.map.blocked[3 * context.map.width + 4] = 1;
  const blockedSide = patrol(client, context); assert.ok(blockedSide); assert.notDeepEqual(blockedSide.target, direct.target);
  assert.equal(navigationPlanIsClear(client, blockedSide), true);
  assert.ok(movementHintCells({ x: 3, y: 3 }, blockedSide).every(point => protocolMapCellIsWalkable(context.map, point)));
});

test('native fallback Run preference cannot enter a transfer in its first or second step', () => {
  for (const y of [5, 6]) {
    const { client, context } = patrolFallbackFixture();
    client.snapshot.mapTransfers = [{ bounds: { minX: 7, maxX: 7, minY: y, maxY: y } }];
    const plan = patrol(client, context); assert.ok(plan);
    assert.ok([plan.first, plan.target].every(point => point.x !== 7 || point.y !== y));
    assert.equal(navigationPlanIsClear(client, plan), true);
  }
});

test('native fallback Run preference retains routeAllowed on the complete route to each original goal', () => {
  const { client, context, owner } = patrolFallbackFixture(), allowed = point => point.y <= 6;
  const rejectedGoal = findProtocolWalkPath({ map: context.map, start: owner, target: { x: 7, y: 7 },
    dynamicObstacles: client.snapshot.entities.slice(1), maxExpanded: 4000 });
  assert.ok(rejectedGoal.slice(0, 3).every(allowed)); assert.equal(rejectedGoal.every(allowed), false);
  const plan = patrol(client, context, client.home, allowed); assert.ok(plan); assert.notDeepEqual(plan.navigation.goal, { x: 7, y: 7 });
  const accepted = findProtocolWalkPath({ map: context.map, start: owner, target: plan.navigation.goal,
    dynamicObstacles: client.snapshot.entities.slice(1), maxExpanded: 4000 });
  assert.ok(accepted.every(allowed)); assert.equal(navigationPlanIsClear(client, plan), true);
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
  secondPeer.snapshot.entities[0].x = 25;
  secondPeer.snapshot.entities.push({ ...owner, objectId: 50002 }, target);
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
  prepareHuntWitness(fixture, target);
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
  prepareHuntWitness(fixture, target); let corrected = true, planned;
  client.send = command => {
    if (!corrected) {
      const dx = planned.target.x - owner.x, dy = planned.target.y - owner.y;
      Object.assign(owner, planned.target); target.x += dx; target.y += dy;
    }
    client.events.push({ direction: 'received', type: 'packet', packet: 'UserLocation', sequence: ++client.sequence,
      monotonicMs: performance.now(), payload: { x: owner.x, y: owner.y, direction: command.direction } });
  };
  const offer = async () => {
    prepareHuntWitness(fixture, target);
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
  const peer = { label: 'peer', inGame: true, ready: true, role: 'movement', home: { x: 279, y: 557 }, snapshot: { mapFileName: '0', playerObjectId: 1000, entities: [
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
  peer.ready = true;
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

function measurementClockFixture({ advance = ({ waitMs }) => waitMs, sample = () => ({}), emit = true,
  stopAfterSleep = () => null, continuous = false, policy = capacityPolicy({ profile: 'probe', stages: '1' }), actorCount = 1, actionIntervalMs = 1000 } = {}) {
  const started = 1000000, rows = [], samples = [];
  let at = started, tick = 0, nextAction = started + actionIntervalMs, stopped;
  const context = { policy, clients: Array.from({ length: actorCount }, (_, i) => ({ label: i ? 'peer' : 'mover', role: 'movement', ready: true })), aoi: new AoiCoverage(), phaseWindows: [],
    writer: { add: row => rows.push(row) }, guard: {
      stop(reason) { stopped ??= reason; }, assert() { if (stopped) throw new Error(stopped); },
    } };
  if (continuous) { context.continuous = new ContinuousAcceptance(policy); context.continuous.join('mover', 'movement', 0); }
  const clock = { now: () => at, async sleep(waitMs) {
    at += advance({ tick: ++tick, elapsedMs: at - started, waitMs });
    context.aoi.stats.samples++;
    if (emit) while (nextAction <= at) {
      for (const client of context.clients) {
      const row = { actor: client.label, phase: context.phase, at: nextAction, kind: 'walk', status: 'success',
        latencyMs: 100, plannedLatencyMs: 120, plannedGameplay: true, ...sample((nextAction - started) / 1000) };
      samples.push(row); context.current?.add(row); context.window?.add(row); context.continuous?.add(row, nextAction - started);
      }
      nextAction += actionIntervalMs;
    }
    const reason = stopAfterSleep({ tick, elapsedMs: at - started }); if (reason) context.guard.stop(reason);
  } };
  return { context, clock, rows, samples, elapsed: () => at - started, stopped: () => stopped };
}

test('measurement phases retain five and six complete actual minutes after late then early timer jitter', async () => {
  for (const seconds of [300, 360]) {
    const fixture = measurementClockFixture({ advance: ({ tick, waitMs }) => tick === 600 ? 180 : tick === 1200 ? 20 : waitMs });
    const stage = await measure(fixture.context, 'soak', seconds, fixture.clock);
    assert.equal(stage.seconds, seconds); assert.equal(stage.measuredSeconds, fixture.elapsed() / 1000);
    assert.ok(stage.measuredSeconds > seconds, 'real jitter is retained rather than labelled as the configured duration');
    assert.equal(stage.rollingWindows.length, seconds / 60); assert.equal(stage.passed, true);
    assert.equal(stage.rollingWindows[0].seconds, 60.08);
    let priorEnd = stage.startedAtMs;
    for (const window of stage.rollingWindows) {
      assert.equal(window.startedAtMs, priorEnd); assert.ok(window.seconds >= 60);
      assert.equal(window.seconds, (window.endedAtMs - window.startedAtMs) / 1000); priorEnd = window.endedAtMs;
    }
    assert.equal(priorEnd, stage.endedAtMs);
    const raw = fixture.rows.filter(row => row.type === 'window');
    assert.deepEqual(raw.map(row => row.assessment.seconds), stage.rollingWindows.map(window => window.seconds));
    assert.deepEqual(fixture.rows.filter(row => row.type === 'phaseEnded').map(row => [row.startedAtMs, row.endedAtMs, row.completed]),
      [[stage.startedAtMs, stage.endedAtMs, true]]);
  }
});

test('measurement phases use actual elapsed duration for whole-phase rates while seconds keeps the profile contract', async () => {
  const fixture = measurementClockFixture({ advance: () => 60100, sample: second => ({ kind: second % 2 ? 'walk' : 'turn' }) });
  const stage = await measure(fixture.context, 'stage-1', 60, fixture.clock);
  const nominal = new Measurements(); for (const row of fixture.samples) nominal.add(row);
  assert.equal(assessMeasurements(nominal, [{ name: 'mover', role: 'movement' }], 60, fixture.context.policy, { requireChat: false }).passed, true,
    'the same thirty moves would pass if the additional real time were hidden in a nominal denominator');
  assert.equal(stage.seconds, 60); assert.equal(stage.measuredSeconds, 60.1); assert.equal(stage.passed, false);
  assert.equal(stage.rollingWindows[0].seconds, 60.1); assert.equal(stage.rollingWindows[0].passed, false);
});

test('measurement phases preserve short probes and non-minute tails without promoting a partial window', async () => {
  for (const seconds of [30, 95]) {
    const fixture = measurementClockFixture();
    const stage = await measure(fixture.context, 'stage-1', seconds, fixture.clock);
    assert.equal(stage.seconds, seconds); assert.equal(stage.measuredSeconds, seconds); assert.equal(stage.passed, true);
    assert.equal(stage.rollingWindows.length, Math.floor(seconds / 60));
    assert.equal(stage.metrics.walk.count, seconds, 'every tail action remains in whole-phase assessment');
    assert.equal(fixture.context.phaseWindows[0].endedAtMs - fixture.context.phaseWindows[0].startedAtMs, seconds * 1000);
  }
});

test('measurement phases retain the configured multi-actor native chat boundary despite positive clock jitter', async () => {
  for (const seconds of [30, 31]) {
    const policy = capacityPolicy({ profile: 'probe', activityMode: 'native', stages: '2', combatActors: 0 });
    const fixture = measurementClockFixture({ policy, actorCount: 2, actionIntervalMs: 750, advance: () => seconds * 1000 + 1,
      sample: () => ({ kind: 'run', movedCells: 2, nativeOpportunity: true, plannedIntervalMs: 750, pacingKnown: true }) });
    const stage = await measure(fixture.context, 'stage-2', seconds, fixture.clock);
    assert.equal(stage.controlledActors, 2); assert.equal(stage.seconds, seconds); assert.equal(stage.measuredSeconds, seconds + .001);
    assert.ok(stage.activity.every(actor => actor.passed)); assert.equal(stage.movementPassed, true);
    assert.equal(stage.metrics.peerChat, undefined); assert.equal(stage.chatPassed, seconds === 30);
    assert.equal(stage.passed, seconds === 30, 'jitter never introduces a new chat gate at30; configured31 still requires real peerChat');
    assert.equal(stage.rollingWindows.length, 0);
  }
});

test('measurement phases retain failed partial-minute actions in whole-phase and continuous verdicts', async () => {
  const fixture = measurementClockFixture({ continuous: true, sample: second => second > 90 ? { status: 'timeout' } : {} });
  const stage = await measure(fixture.context, 'soak', 95, fixture.clock);
  assert.equal(stage.rollingWindows.length, 1); assert.equal(stage.rollingWindows[0].passed, true);
  assert.equal(stage.metrics.walk.status.timeout, 5); assert.equal(stage.passed, false);
  fixture.context.continuous.finish(fixture.elapsed());
  const continuous = fixture.context.continuous.summary();
  assert.equal(continuous.passed, false); assert.equal(continuous.firstFailure.evidence.final, true);
  assert.equal(continuous.firstFailure.evidence.metrics.walk.status.timeout, 5);
});

test('measurement phases fail closed on backward or invalid clocks without completing or inventing a window', async () => {
  for (const badStep of [-50, NaN]) {
    const fixture = measurementClockFixture({ advance: ({ tick }) => tick === 1 ? 100 : badStep });
    await assert.rejects(measure(fixture.context, 'soak', 60, fixture.clock), /clock is invalid or moved backward/);
    assert.match(fixture.stopped(), /clock/); assert.equal(fixture.context.phaseWindows[0].completed, false);
    assert.equal(fixture.context.phaseWindows[0].endedAtMs, 1000100);
    assert.equal(fixture.rows.filter(row => row.type === 'window' || row.type === 'stageComplete').length, 0);
  }
});

test('measurement phases preserve existing stall stops and never split one stalled interval into several minute windows', async () => {
  const stopped = measurementClockFixture({ emit: false, advance: () => 180000,
    stopAfterSleep: () => 'Native driver stalled over 60 legal action opportunities' });
  await assert.rejects(measure(stopped.context, 'soak', 180, stopped.clock), /stalled over 60 legal action opportunities/);
  assert.equal(stopped.rows.filter(row => row.type === 'window' || row.type === 'stageComplete').length, 0);
  assert.equal(stopped.context.phaseWindows[0].completed, false);
  assert.equal(stopped.context.phaseWindows[0].endedAtMs, 1180000, 'a latched stop retains the current valid clock before any assessment');
  const unlatched = measurementClockFixture({ emit: false, advance: ({ tick, waitMs }) => tick === 1 ? 180000 : waitMs });
  const stage = await measure(unlatched.context, 'soak', 180, unlatched.clock);
  assert.equal(stage.seconds, 180); assert.equal(stage.measuredSeconds, 300);
  assert.deepEqual(stage.rollingWindows.map(window => window.seconds), [180, 60, 60]);
  assert.equal(stage.passed, false); assert.ok(stage.rollingWindows.every(window => !window.passed));
});
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
    assert.equal(Object.isFrozen(report.nativePatrolPlanning), true);
    assert.equal(report.nativePatrolPlanning.maximumFallbackCornerTargets, 4); assert.equal(report.nativePatrolPlanning.pathExpansionLimit, 4000);
    assert.equal(Object.getOwnPropertyDescriptor(report, 'nativePatrolPlanning').writable, false);
    assert.throws(() => { report.nativePatrolPlanning.maximumFallbackCornerTargets = 5; }, TypeError);
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


// Actual formal25 showed two ordinary fighters spending interleaved chase ACKs
// on the same target. This fixture preserves each connection's owner1000 while
// using independent actor identities and actual matching UserLocation receipts.
function chasePreferenceFixture(monsters = [huntMonster(900, 'Deer', 10), huntMonster(901, 'Scarecrow', 10, 7)]) {
  const f = huntFixture(monsters), second = new CapacityClient('ws://127.0.0.1:1/ws', actor(15), f.context);
  const secondOwner = { ...nativeSnapshot().entities[0], name: second.label, x: 5, y: 7 };
  second.snapshot = { ...nativeSnapshot(), entities: [secondOwner, { ...f.owner, objectId: 50001 }, ...structuredClone(monsters)] };
  Object.assign(second, { ready: true, inGame: true, role: 'combat', home: { x: 5, y: 7 }, combatArrival: { at: 1 },
    nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity, nativeRunPrimedUntil: 0 });
  f.client.snapshot.entities.push({ ...secondOwner, objectId: 50002 });
  Object.assign(f.peer.snapshot.entities[0], { x: 9, y: 6 });
  f.peer.snapshot.entities.push({ ...secondOwner, objectId: 50002 });
  f.peer.events = []; f.peer.sequence = 0;
  f.context.clients = [f.client, second, f.peer];
  return { ...f, second, secondOwner };
}
function chasePreferenceAck(f, hunter, mono, sequence = 1, prepared) {
  const owner = hunter.snapshot.entities.find(e => e.objectId === hunter.snapshot.playerObjectId);
  const plan = prepared ?? selectNativeAction(hunter, f.context, mono), origin = { x: owner.x, y: owner.y };
  assert.ok(['walk', 'run'].includes(plan.command.type)); assert.equal(plan.chaseTargetId, 900);
  Object.assign(owner, plan.target);
  for (const other of f.context.clients) if (other !== hunter) {
    const observed = other.snapshot.entities.find(e => e.name === hunter.label);
    if (observed) Object.assign(observed, plan.target);
  }
  const receipt = { packet: 'UserLocation', sequence, monotonicMs: mono, payload: { ...plan.target, direction: plan.command.direction } };
  recordCombatPursuitMovement(hunter, f.context, plan, origin, receipt, 'success', Date.now());
  return { plan, origin, receipt };
}

test('native chase preference avoids the formal25 duplicated approach after a real peer ACK without sharing owner1000 identity', () => {
  const f = chasePreferenceFixture(); assert.equal(f.client.snapshot.playerObjectId, 1000); assert.equal(f.second.snapshot.playerObjectId, 1000);
  chasePreferenceAck(f, f.second, 1000);
  const plan = selectNativeAction(f.client, f.context, 1500);
  assert.equal(plan.chaseTargetId, 901);
  assert.equal(f.second.combatPursuit.objectId, 900); assert.equal(f.context.targetClaims.size, 0);
  assert.equal(f.client.failedTargets.size, 0); assert.equal(f.client.chaseDeferrals.size, 0);
});

test('native chase preference keeps first4 claimed fallback when three preferred alternatives have no path', () => {
  const monsters = [huntMonster(900, 'Deer', 10), huntMonster(901, 'Scarecrow', 10, 7),
    huntMonster(902, 'Deer', 10, 3), huntMonster(903, 'Scarecrow', 10, 9), huntMonster(904, 'Deer', 13)];
  const f = chasePreferenceFixture(monsters); chasePreferenceAck(f, f.second, 1000);
  for (const monster of monsters.slice(1, 4)) for (let y = monster.y - 1; y <= monster.y + 1; y++) for (let x = 9; x <= 11; x++) f.context.map.blocked[y * 80 + x] = 1;
  f.client.navigationAttempts = []; f.client.navigationAttemptCount = 0;
  const plan = selectNativeAction(f.client, f.context, 1500);
  assert.equal(plan.chaseTargetId, 900, 'a preference never removes the original reachable fourth candidate');
  assert.ok(f.client.navigationAttemptCount <= 32);
  assert.equal(f.client.navigationAttempts.some(a => a.target.x >= 12), false, 'a fifth target cannot expand the original path budget');
});

test('native chase preference retains a sole reachable approach and gives an actual adjacent witnessed attack precedence', () => {
  const only = chasePreferenceFixture([huntMonster(900, 'Deer', 10)]); chasePreferenceAck(only, only.second, 1000);
  assert.equal(selectNativeAction(only.client, only.context, 1500).chaseTargetId, 900);
  const adjacent = chasePreferenceFixture([huntMonster(900, 'Deer', 6), huntMonster(901, 'Scarecrow', 10, 7)]);
  chasePreferenceAck(adjacent, adjacent.second, 1000);
  const plan = selectNativeAction(adjacent.client, adjacent.context, 1500);
  assert.equal(plan.command.type, 'attack'); assert.equal(plan.command.objectId, 900);
});

test('native chase preference uses a symmetric first-receipt label tiebreak regardless of client iteration order', () => {
  for (const reverse of [false, true]) {
    const f = chasePreferenceFixture(), a = selectNativeAction(f.client, f.context, 1000), b = selectNativeAction(f.second, f.context, 1000);
    chasePreferenceAck(f, f.client, 1000, 1, a); chasePreferenceAck(f, f.second, 1000, 1, b);
    if (reverse) f.context.clients.reverse();
    assert.ok(f.client.label < f.second.label);
    assert.equal(selectNativeAction(f.client, f.context, 1500).chaseTargetId, 900);
    assert.equal(selectNativeAction(f.second, f.context, 1500).chaseTargetId, 901);
  }
});

test('native chase preference honors an earlier actual receipt before lexical actor order', () => {
  const f = chasePreferenceFixture(), a = selectNativeAction(f.client, f.context, 900), b = selectNativeAction(f.second, f.context, 900);
  chasePreferenceAck(f, f.second, 900, 1, b); chasePreferenceAck(f, f.client, 1000, 1, a);
  assert.equal(selectNativeAction(f.second, f.context, 1500).chaseTargetId, 900);
  assert.equal(selectNativeAction(f.client, f.context, 1500).chaseTargetId, 901);
});

test('native chase preference expires at the original receipt age and reacquires only from a new real ACK', () => {
  const f = chasePreferenceFixture(); chasePreferenceAck(f, f.second, 1000);
  assert.equal(selectNativeAction(f.client, f.context, 1500).chaseTargetId, 901);
  // This actor still has no movement receipt on either target: restoring the
  // original selected candidate here isolates expiry from its own preference.
  f.client.combatPursuit = null;
  assert.equal(selectNativeAction(f.client, f.context, 4001).chaseTargetId, 900);
  chasePreferenceAck(f, f.second, 4100, 2);
  assert.equal(f.second.combatPursuit.coordinationFirstReceiptMonoMs, 4100);
  assert.equal(f.second.combatPursuit.coordinationLastReceiptMonoMs, 4100);
  assert.equal(f.second.combatPursuit.confirmedMoves, 2, 'expiry never replenishes an old movement budget');
  assert.equal(selectNativeAction(f.client, f.context, 4200).chaseTargetId, 901);
});

test('native chase preference records the fractional actual receipt clock and ignores duplicate ACK processing', () => {
  const f = chasePreferenceFixture(), first = chasePreferenceAck(f, f.second, 1000.125);
  const pursuit = f.second.combatPursuit;
  assert.equal(pursuit.coordinationFirstReceiptMonoMs, 1000.125); assert.equal(pursuit.coordinationLastReceiptMonoMs, 1000.125);
  recordCombatPursuitMovement(f.second, f.context, first.plan, first.origin, { ...first.receipt, monotonicMs: 2500 }, 'success', Date.now());
  assert.equal(pursuit.coordinationLastReceiptMonoMs, 1000.125); assert.equal(pursuit.confirmedMoves, 1);
  assert.equal(selectNativeAction(f.client, f.context, 4001).chaseTargetId, 900, 'processing a receipt again cannot refresh its TTL');
});

test('native chase preference rejects invalid, zero, corrected and unmatched movement evidence', () => {
  for (const mutation of ['wrong-packet', 'stale-sequence', 'zero-step', 'wrong-location', 'wrong-target', 'corrected', 'missing-mono']) {
    const f = chasePreferenceFixture(), plan = selectNativeAction(f.second, f.context, 1000), origin = { x: f.secondOwner.x, y: f.secondOwner.y };
    const receipt = { packet: 'UserLocation', sequence: 1, monotonicMs: 1000, payload: { ...plan.target } };
    if (mutation === 'wrong-packet') receipt.packet = 'ObjectWalk';
    if (mutation === 'stale-sequence') receipt.sequence = f.second.combatPursuit.lastReceiptSequence;
    if (mutation === 'zero-step') receipt.payload = { ...origin };
    if (mutation === 'wrong-location') receipt.payload = { x: 70, y: 70 };
    if (mutation === 'wrong-target') plan.chaseTargetId = 901;
    if (mutation === 'missing-mono') delete receipt.monotonicMs;
    Object.assign(f.secondOwner, receipt.payload);
    recordCombatPursuitMovement(f.second, f.context, plan, origin, receipt, mutation === 'corrected' ? 'corrected' : 'success', Date.now());
    assert.equal(f.second.combatPursuit.coordinationFirstReceiptMonoMs, null, mutation);
    assert.equal(f.second.combatPursuit.coordinationLastReceiptMonoMs, null, mutation);
    assert.equal(selectNativeAction(f.client, f.context, 1500).chaseTargetId, 900, mutation);
  }
});

test('native chase preference clears a stale or malformed new receipt clock without inventing coordination progress', () => {
  for (const mono of [NaN, Infinity, -1, 999]) {
    const f = chasePreferenceFixture(); chasePreferenceAck(f, f.second, 1000);
    const plan = selectNativeAction(f.second, f.context, 1200), origin = { x: f.secondOwner.x, y: f.secondOwner.y };
    Object.assign(f.secondOwner, plan.target);
    recordCombatPursuitMovement(f.second, f.context, plan, origin, { packet: 'UserLocation', sequence: 2, monotonicMs: mono, payload: { ...plan.target } }, 'success', Date.now());
    assert.equal(f.second.combatPursuit.coordinationFirstReceiptMonoMs, null);
    assert.equal(f.second.combatPursuit.coordinationLastReceiptMonoMs, null);
    assert.equal(selectNativeAction(f.client, f.context, 1300).chaseTargetId, 900);
  }
});

test('native chase preference does not arise from pure plans or a throwing ordinary movement send', async () => {
  const f = chasePreferenceFixture(); for (let i = 0; i < 20; i++) assert.equal(selectNativeAction(f.second, f.context, 1000).chaseTargetId, 900);
  assert.equal(f.second.combatPursuit.coordinationFirstReceiptMonoMs, null);
  assert.equal(selectNativeAction(f.client, f.context, 1500).chaseTargetId, 900);
  f.second.plan = { sent() {} }; f.second.send = () => { throw new Error('ordinary movement send failed'); };
  const plan = selectNativeAction(f.second, f.context, 1600);
  await action(f.second, f.context, { due: performance.now(), cadence: { cooldownMs: 600, intervalMs: 750, pacingKnown: true } }, plan);
  assert.equal(f.second.combatPursuit.coordinationFirstReceiptMonoMs, null); assert.equal(f.second.combatPursuit.coordinationLastReceiptMonoMs, null);
  assert.equal(f.actions.some(row => row.status === 'error'), true);
  assert.equal(selectNativeAction(f.client, f.context, 1700).chaseTargetId, 900);
});

test('native chase preference ignores inactive, wrong-map, noncombat, invalid-target and already-attacking peers', () => {
  for (const invalidate of [f => { f.second.ready = false; }, f => { f.second.inGame = false; }, f => { f.second.closed = true; },
    f => { f.second.paused = true; }, f => { f.second.draining = true; }, f => { f.second.navigationClosing = true; },
    f => { f.second.role = 'movement'; }, f => { f.second.snapshot.mapFileName = 'other'; }, f => { f.secondOwner.dead = true; },
    f => { f.second.combatPursuit.attackSent = true; },
    f => { f.second.snapshot.entities = f.second.snapshot.entities.filter(e => e.objectId !== 900); },
    f => { f.second.snapshot.entities.find(e => e.objectId === 900).dead = true; },
    f => { f.second.combatPursuit.name = 'other-monster'; },
    f => { f.second.combatPursuit.coordinationLastReceiptMonoMs = 2000; }]) {
    const f = chasePreferenceFixture(); chasePreferenceAck(f, f.second, 1000); invalidate(f);
    assert.equal(selectNativeAction(f.client, f.context, 1500).chaseTargetId, 900);
  }
});

test('native chase preference resume drops socket receipt preference while retaining spent chase and quarantine budgets', () => {
  const f = chasePreferenceFixture(); chasePreferenceAck(f, f.client, 1000); const now = Date.now(), pursuit = f.client.combatPursuit;
  Object.assign(pursuit, { startedAt: now, lastMovementAt: now + 1000, activeElapsedMs: 2000, confirmedMoves: 39 });
  pursuit.visited.set('6,5', 2); f.client.failedTargets.set(999, { until: Infinity, ambiguous: true }); f.client.combatPursuitPausedAt = now + 2000;
  const restored = new CapacityClient('ws://127.0.0.1:1/ws', actor(0), f.context); restored.snapshot = structuredClone(f.client.snapshot); restored.sequence = 99;
  restoreCombatPursuit(f.client, restored, f.context, now + 40000);
  assert.equal(restored.combatPursuit.coordinationFirstReceiptMonoMs, null); assert.equal(restored.combatPursuit.coordinationLastReceiptMonoMs, null);
  assert.equal(restored.combatPursuit.confirmedMoves, 39); assert.equal(restored.combatPursuit.activeElapsedMs, 4000);
  assert.equal(restored.combatPursuit.visited.get('6,5'), 2); assert.notEqual(restored.combatPursuit.visited, pursuit.visited);
  assert.equal(f.client.failedTargets.get(999).until, Infinity, 'rebasing a pursuit cannot erase the existing attack quarantine');
});

test('native chase preference leaves baseline candidate choice and every declared gameplay gate unchanged', () => {
  const f = chasePreferenceFixture(); chasePreferenceAck(f, f.second, 1000);
  const original = f.context.policy, nativeBefore = structuredClone(original);
  f.context.policy = capacityPolicy({ profile: 'probe' }); const baselineBefore = structuredClone(f.context.policy);
  assert.equal(selectNativeAction(f.client, f.context, 1500).chaseTargetId, undefined);
  assert.equal(selectNativeAction(f.client, f.context, 1500).hunting, 'Deer');
  assert.deepEqual(original, nativeBefore); assert.deepEqual(f.context.policy, baselineBefore);
  assert.equal(original.minimumCombatAttacksPerMinute, 18); assert.equal(original.minimumCombatHitsPerMinute, 12);
  assert.equal(f.context.policy.minimumCombatAttacksPerMinute, 12); assert.equal(f.context.policy.minimumCombatHitsPerMinute, 3);
  for (const field of ['minimumNativeRunRatio', 'maximumMissedPlanRatio', 'maximumCorrectionRatio', 'movementP95Ms',
    'maximumDriverLagP99Ms']) assert.equal(original[field], f.context.policy[field]);
});

for (const [boundary, ownerAt, observerAt, typedAt, requestId, expected] of [
  ['already recorded before the final poll', 888, 478, 888, '1', 'success'],
  ['the inclusive 900ms boundary', 900, 478, 900, '1', 'success'],
  ['a typed result after 900ms', 888, 478, 901, '1', 'timeout'],
  ['owner damage after 900ms', 901, 478, 888, '1', 'miss'],
  ['observer damage after 900ms', 888, 901, 888, '1', 'miss'],
  ['a different request id before 900ms', 888, 478, 888, '2', 'timeout'],
]) test(`V2 deadline evidence handles ${boundary} without extending the receipt window`, async t => {
  const base = 1700000000000;
  t.mock.timers.enable({ apis: ['Date', 'setTimeout'], now: base });
  t.mock.method(performance, 'now', () => Date.now() - base);
  const target = huntMonster(900, 'Deer', 6), f = huntFixture([target]);
  const { client, peer, owner, context, actions, rows } = f;
  Object.assign(client, { attackResults: 'v2', attackV2Supported: true, plan: { sent() {} } });
  Object.assign(peer, { events: [], sequence: 0 });
  const packetGroup = (receiver, attackerId) => {
    for (const [packet, extra] of [['ObjectStruck', { attackerId }], ['DamageIndicator', { damage: 3 }], ['ObjectHealth', { percent: 85 }]]) {
      const message = { type: 'packet', packet, payload: { objectId: target.objectId, ...extra } };
      if (receiver === client) client.observe(message);
      else {
        const event = { ...message, direction: 'received', sequence: ++receiver.sequence, monotonicMs: performance.now() };
        receiver.events.push(event); recordPendingAttackPacket(receiver, event);
      }
    }
  };
  let sent = 0;
  client.send = command => {
    sent++; assert.equal(command.type, 'attackV2'); assert.equal(command.requestId, '1');
    setTimeout(() => client.observe({ type: 'attackResultV2', protocolVersion: 2,
      requestId, objectId: target.objectId, result: 'launched' }), typedAt);
    setTimeout(() => packetGroup(client, owner.objectId), ownerAt);
    setTimeout(() => packetGroup(peer, 50001), observerAt);
  };
  const plan = selectNativeAction(client, context); assert.equal(plan.command.type, 'attack');
  const pending = action(client, context, { due: 0, cadence: { cooldownMs: 600, intervalMs: 750, pacingKnown: true } }, plan);
  // The previous poll wakes at 880ms. Evidence can arrive at 888/900/901ms,
  // but the next poll resumes at 903ms: no extra observation wait is allowed.
  t.mock.timers.tick(478); t.mock.timers.tick(402); await Promise.resolve();
  t.mock.timers.tick(8); t.mock.timers.tick(12); t.mock.timers.tick(1); t.mock.timers.tick(2);
  await pending;
  const actual = actions.find(row => row.kind === 'attack'), trace = rows.find(row => row.type === 'pendingAttackEvidence');
  assert.equal(sent, 1); assert.equal(actual.status, expected); assert.equal(trace.outcome, expected);
  assert.equal(trace.truncated, false); assert.equal(trace.requestId, '1');
  assert.equal(client.pendingAttackPacketTraces.size, 0); assert.equal(peer.pendingAttackPacketTraces.size, 0);
  assert.equal(context.targetClaims.size, 0);
  if (expected === 'success') {
    assert.equal(actual.latencyMs, Math.max(ownerAt, observerAt)); assert.ok(actual.latencyMs <= 900);
    assert.equal(actual.sharedHealthVerified, true); assert.equal(actual.damage, 3);
    assert.equal(client.failedTargets.size, 0);
  } else {
    assert.equal(actual.latencyMs, 903); assert.equal(actual.sharedHealthVerified, undefined);
    assert.equal(client.failedTargets.get(target.objectId).until, Infinity);
    assert.equal(actual.ambiguousTargetQuarantined, true);
  }
});

test('capacity wait marks only its real elapsed deadline with a typed label and the original message', async () => {
  const client = { label: 'deadline-actor', context: { guard: { assert() {} } } };
  const started = Date.now(), label = 'funding harvest', timeoutMs = 5;
  let reads = 0;
  await assert.rejects(CapacityClient.prototype.wait.call(client, () => { reads++; return null; }, label, timeoutMs), error => {
    assert.equal(error.code, 'CAPACITY_WAIT_TIMEOUT'); assert.equal(error.waitLabel, label);
    assert.equal(error.message, 'deadline-actor timed out waiting for funding harvest');
    assert.ok(Date.now() >= started + timeoutMs); assert.ok(reads >= 1);
    return true;
  });
});

for (const boundary of ['guard', 'predicate', 'transport', 'deadline-predicate', 'closed'])
  test(`capacity wait preserves ${boundary} failure without a timeout marker`, async () => {
    const error = boundary === 'guard' ? new SafetyStop('monitor requested stop') : new Error(`${boundary} failure`);
    const client = { label: 'failure-actor', context: { guard: { assert() { if (boundary === 'guard') throw error; } } } };
    if (boundary === 'transport') client.failure = error;
    if (boundary === 'closed') client.closed = true;
    const predicate = () => { if (boundary === 'predicate') throw error; return null; };
    const finalPredicate = boundary === 'deadline-predicate' ? () => { throw error; } : null;
    await assert.rejects(CapacityClient.prototype.wait.call(client, predicate, 'bounded receipt',
      finalPredicate ? 0 : 100, finalPredicate), actual => {
      if (boundary === 'closed') assert.equal(actual.message, 'failure-actor transport closed during bounded receipt');
      else assert.equal(actual, error);
      assert.equal(Object.hasOwn(actual, 'code'), false); assert.equal(Object.hasOwn(actual, 'waitLabel'), false);
      return true;
    });
  });

for (const boundary of ['ordinary wait', 'guard', 'failure', 'closed'])
  test(`V2 final evidence read preserves ${boundary} failure semantics`, async t => {
    const base = 1700000000000;
    t.mock.timers.enable({ apis: ['Date', 'setTimeout'], now: base });
    let available = false, finalReads = 0, stopped = false;
    const client = { context: { guard: { assert() { if (stopped) throw new Error('guard stopped'); } } } };
    const receipt = { monotonicMs: 888 }, predicate = () => available ? receipt : null;
    const finalPredicate = () => { finalReads++; return predicate(); };
    const pending = CapacityClient.prototype.wait.call(client, predicate, 'bounded receipt', 900,
      boundary === 'ordinary wait' ? undefined : finalPredicate).then(value => ({ value }), error => ({ error }));
    setTimeout(() => {
      available = true;
      if (boundary === 'guard') stopped = true;
      if (boundary === 'failure') client.failure = new Error('socket failed');
      if (boundary === 'closed') client.closed = true;
    }, 888);
    t.mock.timers.tick(880); await Promise.resolve(); t.mock.timers.tick(8); t.mock.timers.tick(15);
    const actual = await pending;
    assert.ok(actual.error); assert.equal(finalReads, 0);
    assert.match(actual.error.message, boundary === 'ordinary wait' ? /timed out/ :
      boundary === 'guard' ? /guard stopped/ : boundary === 'failure' ? /socket failed/ : /transport closed/);
  });
function observerHintScoutFixture() {
  const fixture = scoutFixture();
  fixture.client.combatScoutCursor = 0;
  fixture.hintTarget = huntMonster(9100, 'Deer', 20, 6);
  fixture.secondPeer.snapshot.entities.push(fixture.hintTarget);
  return fixture;
}

test('observer scout preference keeps the original cyclic no-hint plan and emits no selection credit', () => {
  const { client, context, rows, actions } = scoutFixture();
  const plan = selectNativeAction(client, context);
  assert.equal(plan.scoutHomeIndex, 1); assert.deepEqual(plan.navigation.goal, { x: 19, y: 6 });
  assert.equal(client.navigationAttemptCount, 0); assert.equal(client.combatScout.confirmedMoves, 0);
  assert.equal(rows.length, 0); assert.equal(actions.length, 0);
  assert.equal(context.policy.minimumCombatAttacksPerMinute, 18); assert.equal(context.policy.minimumCombatHitsPerMinute, 12);
  assert.equal(nativeCadence(client.snapshot, 'attack', context.policy).intervalMs, 1436);
});

test('observer scout preference uses peer-only target geometry for ordinary movement without inventing owner visibility', () => {
  const { client, secondPeer, owner, context, rows, actions, hintTarget } = observerHintScoutFixture();
  const before = JSON.stringify(client.snapshot);
  assert.ok(Math.max(Math.abs(selfFor(secondPeer).x - owner.x), Math.abs(selfFor(secondPeer).y - owner.y)) > 6);
  assert.equal(secondPeer.snapshot.entities.some(entity => entity.name === client.label), false);
  const plan = selectNativeAction(client, context);
  assert.equal(plan.scouting, true); assert.equal(plan.scoutHomeIndex, 1); assert.ok(['walk', 'run'].includes(plan.command.type));
  assert.equal(plan.command.objectId, undefined); assert.equal(plan.targetEntity, undefined); assert.equal(plan.observer, undefined);
  assert.equal(JSON.stringify(client.snapshot), before); assert.equal(client.combatPursuit, null);
  assert.equal(client.combatScout.confirmedMoves, 0); assert.equal(actions.length, 0); assert.equal(context.targetClaims.size, 0);
  const selection = rows.find(row => row.type === 'combatScoutSelection');
  assert.equal(selection.reason, 'observer-target-waypoint'); assert.equal(selection.fallbackHomeIndex, 0);
  assert.equal(selection.hintTargetId, hintTarget.objectId); assert.equal(selection.pathSearches, 2); assert.equal(selection.hintPathSearches, 1);
  for (const point of [selection.hintTarget, selection.potentialAttackTile]) {
    assert.ok(Math.max(Math.abs(point.x - selfFor(secondPeer).x), Math.abs(point.y - selfFor(secondPeer).y)) <= 6);
    assert.ok(insideCombatCoverage(activeCombatCoverage(client, context), point));
  }
  const next = selectNativeAction(client, context);
  assert.equal(next.scouting, true); assert.notEqual(next.command.type, 'attack'); assert.equal(rows.length, 1);
});

test('observer scout preference preserves the exact cyclic fallback when its hint fails within the four-search budget', () => {
  const baseline = observerHintScoutFixture(), hinted = observerHintScoutFixture();
  baseline.secondPeer.snapshot.entities.pop();
  for (const fixture of [baseline, hinted]) for (let y = 0; y < fixture.context.map.height; y++) {
    fixture.context.map.blocked[y * fixture.context.map.width + 10] = 1;
  }
  const original = selectNativeAction(baseline.client, baseline.context), selected = selectNativeAction(hinted.client, hinted.context);
  assert.deepEqual(selected, original); assert.equal(selected.scoutHomeIndex, 0);
  assert.deepEqual(hinted.client.combatScout.goal, baseline.client.combatScout.goal);
  assert.equal(hinted.client.navigationAttemptCount, 1); assert.equal(hinted.client.navigationAttempts.length, 1);
  const selection = hinted.rows.find(row => row.type === 'combatScoutSelection');
  assert.equal(selection.reason, 'cyclic-fallback'); assert.equal(selection.pathSearches, 2); assert.equal(selection.hintPathSearches, 1);
  assert.equal(hinted.client.combatScout.confirmedMoves, 0); assert.equal(hinted.actions.length, 0);
});

test('observer scout preference excludes ineligible targets, unavailable peers and geometry without changing fallback', () => {
  const cases = [
    ['dead target', f => { f.hintTarget.dead = true; }],
    ['zero HP', f => { f.hintTarget.hp = 0; }],
    ['friendly', f => { f.hintTarget.disposition = ' friendly '; }],
    ['owned', f => { f.hintTarget.ownerName = 'another-player'; }],
    ['not monster', f => { f.hintTarget.kind = 'player'; }],
    ['not starter', f => { f.hintTarget.name = 'Oma'; }],
    ['not declared starter', f => { f.hintTarget.name = 'HookingCat'; }],
    ['zero id', f => { f.hintTarget.objectId = 0; }],
    ['negative id', f => { f.hintTarget.objectId = -1; }],
    ['fractional id', f => { f.hintTarget.objectId = 1.5; }],
    ['string id', f => { f.hintTarget.objectId = '9100'; }],
    ['out of wire id range', f => { f.hintTarget.objectId = 0x100000000; }],
    ['infinite id', f => { f.hintTarget.objectId = Infinity; }],
    ['nonfinite point', f => { f.hintTarget.x = NaN; }],
    ['fractional point', f => { f.hintTarget.x = 20.5; }],
    ['outside coverage', f => { f.hintTarget.x = 60; }],
    ['outside actual peer range', f => { f.hintTarget.y = 12; }],
    ['no walkable potential strike tile', f => {
      for (let y = 5; y <= 7; y++) for (let x = 19; x <= 21; x++) if (x !== 20 || y !== 6) f.context.map.blocked[y * 80 + x] = 1;
    }],
    ['transfer target', f => { f.client.snapshot.mapTransfers = [{ bounds: { minX: 20, maxX: 20, minY: 6, maxY: 6 } }]; }],
    ['failed target', f => { f.client.failedTargets.set(9100, { until: Date.now() + 30000 }); }],
    ['quarantined target', f => { f.client.failedTargets.set(9100, { until: Infinity }); }],
    ['deferred target', f => { f.client.chaseDeferrals.set(9100, { mapFileName: '0', until: Date.now() + 30000 }); }],
    ['claimed target', f => { f.context.targetClaims.set(9100, 'other-fighter'); }],
    ['not ready', f => { f.secondPeer.ready = false; }],
    ['not in game', f => { f.secondPeer.inGame = false; }],
    ['paused', f => { f.secondPeer.paused = true; }],
    ['closed', f => { f.secondPeer.closed = true; }],
    ['draining', f => { f.secondPeer.draining = true; }],
    ['navigation closing', f => { f.secondPeer.navigationClosing = true; }],
    ['wrong map', f => { f.secondPeer.snapshot.mapFileName = '1'; }],
    ['dead observer', f => { selfFor(f.secondPeer).dead = true; }],
    ['invalid observer point', f => { selfFor(f.secondPeer).x = Infinity; }],
    ['not movement role', f => { f.secondPeer.role = 'combat'; }],
    ['not declared observer', f => { f.secondPeer.label = 'not-a-declared-observer'; }],
  ];
  for (const [name, invalidate] of cases) {
    const fixture = observerHintScoutFixture(); invalidate(fixture);
    const plan = selectNativeAction(fixture.client, fixture.context);
    assert.equal(plan.scoutHomeIndex, 0, name); assert.deepEqual(plan.navigation.goal, { x: 5, y: 8 }, name);
    assert.equal(fixture.client.navigationAttemptCount, 0, name);
    assert.equal(fixture.rows.some(row => row.type === 'combatScoutSelection'), false, name);
    assert.equal(fixture.actions.length, 0, name);
  }
});

test('observer scout preference allows expired exclusions and own claims using wall-clock eligibility', () => {
  for (const allowed of [
    f => f.client.failedTargets.set(9100, { until: Date.now() - 1 }),
    f => f.client.chaseDeferrals.set(9100, { mapFileName: '0', until: Date.now() - 1 }),
    f => f.client.chaseDeferrals.set(9100, { mapFileName: '1', until: Infinity }),
    f => f.context.targetClaims.set(9100, f.client.label),
  ]) {
    const fixture = observerHintScoutFixture(); allowed(fixture);
    const plan = selectNativeAction(fixture.client, fixture.context, 1);
    assert.equal(plan.scoutHomeIndex, 1); assert.equal(plan.command.objectId, undefined);
    assert.equal(fixture.rows.find(row => row.type === 'combatScoutSelection').hintTargetId, 9100);
  }
});

test('observer scout preference never resets an established scout clock, ACKs, visits or other finite pursuit budgets', () => {
  const fixture = observerHintScoutFixture(), { client, context, rows } = fixture;
  fixture.secondPeer.snapshot.entities.pop(); const original = selectNativeAction(client, context), scout = client.combatScout;
  const now = Date.now(); scout.startedAt = now - 19000; scout.activeElapsedMs = 100; scout.confirmedMoves = 23;
  scout.lastReceiptSequence = 7; scout.visited.set('2,2', 2);
  const visited = scout.visited, pursuit = { objectId: 9199, name: 'Deer', mapFileName: '0', confirmedMoves: 39, startedAt: now - 29000 };
  client.snapshot.entities.push(huntMonster(9199, 'Deer', 8));
  client.combatPursuit = pursuit; client.failedTargets.set(9101, { until: Infinity });
  client.chaseDeferrals.set(9102, { mapFileName: '0', until: now + 30000 });
  fixture.secondPeer.snapshot.entities.push(fixture.hintTarget);
  const plan = selectNativeAction(client, context);
  assert.deepEqual(plan, original); assert.equal(client.combatScout, scout); assert.equal(scout.startedAt, now - 19000);
  assert.equal(scout.activeElapsedMs, 100); assert.equal(scout.confirmedMoves, 23); assert.equal(scout.lastReceiptSequence, 7);
  assert.equal(scout.visited, visited); assert.equal(visited.get('2,2'), 2); assert.equal(client.combatPursuit, pursuit);
  assert.equal(client.failedTargets.get(9101).until, Infinity); assert.equal(client.chaseDeferrals.get(9102).until, now + 30000);
  assert.equal(rows.some(row => row.type === 'combatScoutSelection'), false);
  const origin = { ...fixture.owner }, receipt = { packet: 'UserLocation', sequence: 8, payload: { ...plan.target } };
  recordCombatScoutMovement(client, context, plan, origin, receipt, 'corrected', now); assert.equal(scout.confirmedMoves, 23);
  Object.assign(fixture.owner, plan.target); recordCombatScoutMovement(client, context, plan, origin, receipt, 'success', now);
  assert.equal(client.combatScout, null); assert.equal(rows.at(-1).reason, 'movement-budget'); assert.equal(client.combatScoutCursor, 1);
});

test('observer scout preference keeps Run-first ACK separate from waypoint arrival and ignores duplicate or corrected receipts', () => {
  const { client, owner, context, rows } = observerHintScoutFixture(); client.nativeRunPrimedUntil = Infinity;
  const plan = selectNativeAction(client, context), scout = client.combatScout, origin = { ...owner };
  assert.equal(plan.command.type, 'run'); assert.notDeepEqual(plan.first, scout.goal); assert.notDeepEqual(plan.target, scout.goal);
  Object.assign(owner, plan.first);
  const first = { packet: 'UserLocation', sequence: 1, payload: { ...owner } };
  recordCombatScoutMovement(client, context, plan, origin, first, 'success');
  recordCombatScoutMovement(client, context, plan, origin, first, 'success');
  recordCombatScoutMovement(client, context, plan, origin, { ...first, sequence: 2, payload: { ...plan.target } }, 'corrected');
  assert.equal(client.combatScout, scout); assert.equal(scout.confirmedMoves, 1); assert.equal(scout.lastReceiptSequence, 1);
  Object.assign(owner, { x: scout.goal.x - 1, y: scout.goal.y });
  const lastOrigin = { ...owner }, last = selectNativeAction(client, context);
  assert.deepEqual(last.target, scout.goal); Object.assign(owner, last.target);
  recordCombatScoutMovement(client, context, last, lastOrigin, { packet: 'UserLocation', sequence: 2, payload: { ...owner } }, 'success');
  assert.equal(client.combatScout, null); assert.equal(rows.at(-1).reason, 'waypoint-reached');
});

test('observer scout preference bounds snapshot hints and per-phase selection diagnostics without gameplay credit', () => {
  const { client, secondPeer, context, rows, actions, hintTarget } = observerHintScoutFixture();
  const peerOwner = secondPeer.snapshot.entities[0];
  secondPeer.snapshot.entities = [peerOwner, ...Array.from({ length: 2047 }, () => ({ objectId: 0 })), hintTarget];
  assert.equal(selectNativeAction(client, context).scoutHomeIndex, 0);
  client.combatScout = null; client.combatScoutCursor = 0; secondPeer.snapshot.entities[2047] = hintTarget;
  for (let index = 0; index < 36; index++) {
    client.combatScout = null; client.combatScoutCursor = 0;
    assert.equal(selectNativeAction(client, context).scoutHomeIndex, 1);
  }
  const selections = rows.filter(row => row.type === 'combatScoutSelection');
  assert.equal(selections.length, 32); assert.equal(selections.at(-1).truncated, true); assert.equal(selections.at(-1).rowLimit, 32);
  assert.ok(selections.slice(0, -1).every(row => row.hintScanTruncated && row.pathSearches === 2));
  assert.equal(JSON.stringify(selections).includes('private-pass'), false);
  assert.equal(JSON.stringify(selections).includes('entities'), false); assert.equal(actions.length, 0); assert.equal(context.targetClaims.size, 0);
  context.phase = 'stage-10'; client.combatScout = null; client.combatScoutCursor = 0;
  selectNativeAction(client, context); assert.equal(rows.filter(row => row.type === 'combatScoutSelection').length, 33);
});
test('observer scout preference discarded paths reuse read-only obstacles without an extra rejected-cell cleanup', t => {
  let clock = 1000; t.mock.method(performance, 'now', () => clock);
  const { client, hintTarget, context, rows } = observerHintScoutFixture();
  for (let y = 0; y < context.map.height; y++) context.map.blocked[y * context.map.width + 10] = 1;
  const retained = Object.freeze({ x: 1, y: 1, mapFileName: '0', expiresAt: 4000 });
  client.rejectedWalkCells = new Map([['1,1', retained], ['2,1', { x: 2, y: 1, mapFileName: '0', expiresAt: 0 }]]);
  Object.defineProperty(hintTarget, 'hp', { get() { clock = 5000; return 20; } });
  const snapshot = JSON.stringify(client.snapshot), selected = selectNativeAction(client, context);
  assert.equal(selected.scoutHomeIndex, 0); assert.equal(client.navigationAttemptCount, 1);
  assert.deepEqual([...client.rejectedWalkCells], [['1,1', retained]], 'only the original fallback cleanup removes its already-expired cell');
  assert.equal(client.rejectedWalkCells.get('1,1'), retained); assert.equal(JSON.stringify(client.snapshot), snapshot);
  assert.equal(rows.find(row => row.type === 'combatScoutSelection').pathSearches, 2);
  assert.equal(context.targetClaims.size, 0); assert.equal(client.combatScout.confirmedMoves, 0);
});

function antiConvergenceScoutFixture() {
  const f = observerHintScoutFixture(), { client, context, peer, secondPeer } = f;
  const thirdPeer = { label: actor(4).name, inGame: true, ready: true, role: 'movement', snapshot: { mapFileName: '0', playerObjectId: 1000,
    entities: [{ objectId: 1000, name: actor(4).name, kind: 'player', x: 20, y: 15 }, huntMonster(9200, 'Deer', 20, 16)] } };
  context.clients.push(thirdPeer);
  client.combatCluster = declaredObservers([{ x: 5, y: 9 }, { x: 20, y: 5 }, { x: 20, y: 15 }], [peer, secondPeer, thirdPeer]);
  const other = new CapacityClient('ws://127.0.0.1:1/ws', actor(3), context);
  const otherOwner = { ...nativeSnapshot().entities[0], name: other.label, x: 5, y: 6 };
  Object.assign(other, { snapshot: { ...nativeSnapshot(), entities: [otherOwner] }, ready: true, inGame: true, role: 'combat', home: { x: 5, y: 6 },
    combatArrival: { at: 1 }, combatCluster: client.combatCluster, combatScoutCursor: 0, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity });
  context.clients.push(other);
  return { ...f, thirdPeer, other, otherOwner };
}

test('scout anti-convergence prefers a distinct reachable hint after another fighter starts scouting', () => {
  // Prove each alternative through the public planner before the contested case.
  for (const expected of [1, 2]) {
    const f = antiConvergenceScoutFixture();
    (expected === 1 ? f.thirdPeer : f.secondPeer).snapshot.entities.pop();
    const plan = selectNativeAction(f.other, f.context);
    assert.equal(plan.scouting, true); assert.equal(plan.scoutHomeIndex, expected);
    assert.ok(['walk', 'run'].includes(plan.command.type)); assert.equal(plan.command.objectId, undefined);
    assert.equal(f.rows.at(-1).reason, 'observer-target-waypoint'); assert.equal(f.rows.at(-1).pathSearches, 2);
    assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
  }
  const f = antiConvergenceScoutFixture(), first = selectNativeAction(f.client, f.context);
  assert.equal(first.scoutHomeIndex, 1); assert.equal(f.context.targetClaims.size, 0);
  const firstScout = f.client.combatScout, ownerSnapshot = JSON.stringify(f.other.snapshot);
  const second = selectNativeAction(f.other, f.context);
  assert.equal(second.scouting, true); assert.equal(second.scoutHomeIndex, 2, 'a reachable unused waypoint must precede the occupied hint');
  assert.equal(f.client.combatScout, firstScout); assert.equal(firstScout.confirmedMoves, 0);
  assert.equal(f.other.combatScout.confirmedMoves, 0); assert.equal(JSON.stringify(f.other.snapshot), ownerSnapshot);
  assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
  assert.ok(['walk', 'run'].includes(second.command.type)); assert.equal(second.command.objectId, undefined);
  assert.equal(f.rows.at(-1).pathSearches, 2); assert.equal(f.rows.at(-1).hintPathSearches, 1);
});


test('scout anti-convergence ignores unavailable and expired other scouts at exact native budget boundaries', t => {
  t.mock.timers.enable({ apis: ['Date'], now: 1700000000000 });
  const variants = [
    ['not ready', f => { f.client.ready = false; }], ['not in game', f => { f.client.inGame = false; }],
    ['closed', f => { f.client.closed = true; }], ['paused', f => { f.client.paused = true; }],
    ['draining', f => { f.client.draining = true; }], ['navigation closing', f => { f.client.navigationClosing = true; }],
    ['wrong role', f => { f.client.role = 'movement'; }], ['dead', f => { f.owner.dead = true; }],
    ['zero hp', f => { f.client.snapshot.playerHp = 0; }], ['other map', f => { f.client.snapshot.mapFileName = '1'; }],
    ['scout map mismatch', f => { f.client.combatScout.mapFileName = '1'; }],
    ['20 seconds', f => { f.client.combatScout.startedAt = Date.now() - 20000; }],
    ['accumulated 20 seconds', f => { f.client.combatScout.activeElapsedMs = 1000; f.client.combatScout.startedAt = Date.now() - 19000; }],
    ['24 ACKs', f => { f.client.combatScout.confirmedMoves = 24; }],
    ['future clock', f => { f.client.combatScout.startedAt = Date.now() + 1; }],
  ];
  for (const [name, invalidate] of variants) {
    const f = antiConvergenceScoutFixture(); selectNativeAction(f.client, f.context); invalidate(f);
    const scout = f.client.combatScout, startedAt = scout.startedAt, confirmedMoves = scout.confirmedMoves;
    const plan = selectNativeAction(f.other, f.context);
    assert.equal(plan.scoutHomeIndex, 1, name); assert.equal(f.rows.at(-1).hintOrderChangedByActiveScoutPreference, false, name);
    assert.equal(f.client.combatScout, scout, name); assert.equal(scout.startedAt, startedAt, name);
    assert.equal(scout.confirmedMoves, confirmedMoves, name); assert.equal(f.context.targetClaims.size, 0, name);
  }
  const f = antiConvergenceScoutFixture(); selectNativeAction(f.client, f.context);
  f.client.combatScout.startedAt = Date.now() - 19999; f.client.combatScout.confirmedMoves = 23;
  assert.equal(selectNativeAction(f.other, f.context).scoutHomeIndex, 2, '19.999 seconds and23 ACKs still prefer another waypoint');
});

test('scout anti-convergence compares declared coordinates across clusters rather than local home indices', () => {
  for (const sameCoordinate of [false, true]) {
    const f = antiConvergenceScoutFixture();
    f.client.combatCluster = declaredObservers([{ x: 5, y: 9 }, { x: 20, y: 15 }, { x: 20, y: 5 }],
      [f.peer, f.thirdPeer, f.secondPeer]);
    if (sameCoordinate) f.client.failedTargets.set(9200, { until: Infinity });
    const first = selectNativeAction(f.client, f.context);
    assert.equal(first.scoutHomeIndex, sameCoordinate ? 2 : 1);
    const occupied = f.client.combatCluster.observerHomes[first.scoutHomeIndex];
    assert.deepEqual(occupied, sameCoordinate ? { x: 20, y: 5 } : { x: 20, y: 15 });
    const second = selectNativeAction(f.other, f.context);
    assert.equal(second.scoutHomeIndex, sameCoordinate ? 2 : 1,
      sameCoordinate ? 'different indices identifying the same waypoint conflict' : 'the same index at a different waypoint is not a conflict');
    assert.equal(f.context.targetClaims.size, 0);
  }
});

test('scout anti-convergence retains original hint order when all hints are shared or only one remains', () => {
  const single = antiConvergenceScoutFixture(); single.thirdPeer.snapshot.entities.pop();
  assert.equal(selectNativeAction(single.client, single.context).scoutHomeIndex, 1);
  assert.equal(selectNativeAction(single.other, single.context).scoutHomeIndex, 1, 'a planning preference is not an exclusive claim');
  assert.equal(single.rows.at(-1).hintOrderChangedByActiveScoutPreference, false);
  const f = antiConvergenceScoutFixture();
  assert.equal(selectNativeAction(f.client, f.context).scoutHomeIndex, 1);
  assert.equal(selectNativeAction(f.other, f.context).scoutHomeIndex, 2);
  const third = new CapacityClient('ws://127.0.0.1:1/ws', actor(5), f.context);
  Object.assign(third, { snapshot: { ...nativeSnapshot(), entities: [{ ...nativeSnapshot().entities[0], name: third.label, x: 5, y: 7 }] },
    ready: true, inGame: true, role: 'combat', home: { x: 5, y: 7 }, combatArrival: { at: 1 }, combatCluster: f.client.combatCluster,
    combatScoutCursor: 0, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity });
  f.context.clients.push(third);
  assert.equal(selectNativeAction(third, f.context).scoutHomeIndex, 1, 'all shared hints retain the original cycle order');
  assert.equal(f.rows.at(-1).hintOrderChangedByActiveScoutPreference, false);
  assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
});

test('scout anti-convergence never resets current or other scout and pursuit state or creates gameplay credit', t => {
  t.mock.timers.enable({ apis: ['Date'], now: 1700000000000 });
  const f = antiConvergenceScoutFixture(); selectNativeAction(f.client, f.context);
  const first = f.client.combatScout;
  Object.assign(first, { startedAt: Date.now() - 19000, activeElapsedMs: 100, confirmedMoves: 23, lastReceiptSequence: 7 });
  first.visited.set('2,2', 2); const firstVisited = first.visited;
  const pursuit = { objectId: 9199, mapFileName: '0', confirmedMoves: 39, startedAt: Date.now() - 29000 };
  f.client.combatPursuit = pursuit; f.client.chaseDeferrals.set(9300, { mapFileName: '0', until: Date.now() + 30000 });
  assert.equal(selectNativeAction(f.other, f.context).scoutHomeIndex, 2);
  assert.equal(f.client.combatScout, first); assert.equal(first.startedAt, Date.now() - 19000);
  assert.equal(first.activeElapsedMs, 100); assert.equal(first.confirmedMoves, 23); assert.equal(first.lastReceiptSequence, 7);
  assert.equal(first.visited, firstVisited); assert.equal(first.visited.get('2,2'), 2); assert.equal(f.client.combatPursuit, pursuit);
  assert.equal(pursuit.confirmedMoves, 39); assert.equal(f.client.chaseDeferrals.get(9300).until, Date.now() + 30000);
  const current = f.other.combatScout, currentVisited = current.visited;
  Object.assign(current, { startedAt: Date.now() - 19000, activeElapsedMs: 100, confirmedMoves: 23, lastReceiptSequence: 9 });
  current.visited.set('3,3', 2); f.client.closed = true;
  const rowsBefore = f.rows.length, plan = selectNativeAction(f.other, f.context);
  assert.equal(plan.scoutHomeIndex, 2); assert.equal(f.other.combatScout, current); assert.equal(current.startedAt, Date.now() - 19000);
  assert.equal(current.activeElapsedMs, 100); assert.equal(current.confirmedMoves, 23); assert.equal(current.lastReceiptSequence, 9);
  assert.equal(current.visited, currentVisited); assert.equal(current.visited.get('3,3'), 2); assert.equal(f.rows.length, rowsBefore);
  assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
  assert.equal(f.context.policy.minimumCombatAttacksPerMinute, 18); assert.equal(f.context.policy.minimumCombatHitsPerMinute, 12);
});

test('scout anti-convergence failed preferred paths keep the exact cyclic fallback within four total searches', () => {
  const baseline = antiConvergenceScoutFixture(), ranked = antiConvergenceScoutFixture();
  for (const f of [baseline, ranked]) {
    selectNativeAction(f.client, f.context);
    for (let y = 0; y < f.context.map.height; y++) f.context.map.blocked[y * f.context.map.width + 10] = 1;
  }
  baseline.secondPeer.snapshot.entities.pop(); baseline.thirdPeer.snapshot.entities.pop();
  const original = selectNativeAction(baseline.other, baseline.context), firstScout = ranked.client.combatScout;
  const selected = selectNativeAction(ranked.other, ranked.context);
  assert.deepEqual(selected, original); assert.equal(selected.scoutHomeIndex, 0);
  assert.deepEqual(ranked.other.combatScout.goal, baseline.other.combatScout.goal);
  assert.equal(ranked.rows.at(-1).reason, 'cyclic-fallback'); assert.equal(ranked.rows.at(-1).pathSearches, 3);
  assert.equal(ranked.rows.at(-1).hintPathSearches, 2); assert.equal(ranked.other.navigationAttemptCount, 2);
  assert.equal(ranked.rows.at(-1).hintOrderChangedByActiveScoutPreference, true);
  assert.equal(ranked.client.combatScout, firstScout); assert.equal(firstScout.confirmedMoves, 0);
  assert.equal(ranked.context.targetClaims.size, 0); assert.equal(ranked.actions.length, 0);
});

test('scout anti-convergence does not introduce distance sorting when no other scout shares a hint', () => {
  const f = antiConvergenceScoutFixture();
  Object.assign(f.secondPeer.snapshot.entities[0], { x: 35, y: 5 }); Object.assign(f.secondPeer.snapshot.entities[1], { x: 35, y: 6 });
  Object.assign(f.thirdPeer.snapshot.entities[0], { x: 12, y: 5 }); Object.assign(f.thirdPeer.snapshot.entities[1], { x: 12, y: 6 });
  const cluster = declaredObservers([{ x: 5, y: 9 }, { x: 35, y: 5 }, { x: 12, y: 5 }], [f.peer, f.secondPeer, f.thirdPeer]);
  f.client.combatCluster = cluster; f.other.combatCluster = cluster;
  const plan = selectNativeAction(f.other, f.context);
  assert.equal(plan.scoutHomeIndex, 1, 'the original cyclic far hint still precedes the closer later hint');
  assert.equal(f.rows.at(-1).hintOrderChangedByActiveScoutPreference, false); assert.equal(f.rows.at(-1).pathSearches, 2);
  assert.equal(f.context.targetClaims.size, 0);
});


function scoutSelectionDiagnosticCounts(row) {
  return Object.fromEntries(['hintCount', 'activeOtherWaypointCount', 'sharedHintCount', 'unsharedHintCount', 'hintPreferenceReason']
    .map(key => [key, row[key]]));
}

test('scout selection diagnostics leave low-hint peer state unobserved and preserve single-hint and no-hint routes', () => {
  const f = antiConvergenceScoutFixture(); f.thirdPeer.snapshot.entities.pop();
  let peerScoutReads = 0;
  Object.defineProperty(f.client, 'combatScout', { configurable: true, get() { peerScoutReads++; throw new Error('low-hint peer scout must not be scanned'); } });
  const snapshot = JSON.stringify(f.other.snapshot), selected = selectNativeAction(f.other, f.context);
  assert.equal(selected.scoutHomeIndex, 1); assert.ok(['walk', 'run'].includes(selected.command.type));
  assert.equal(selected.command.objectId, undefined); assert.equal(peerScoutReads, 0);
  assert.deepEqual(scoutSelectionDiagnosticCounts(f.rows.at(-1)), {
    hintCount: 1, activeOtherWaypointCount: null, sharedHintCount: null, unsharedHintCount: null, hintPreferenceReason: 'insufficient-hints',
  });
  assert.equal(f.rows.at(-1).pathSearches, 2); assert.equal(f.rows.at(-1).hintOrderChangedByActiveScoutPreference, false);
  assert.equal(JSON.stringify(f.other.snapshot), snapshot); assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
  const empty = antiConvergenceScoutFixture(); empty.secondPeer.snapshot.entities.pop(); empty.thirdPeer.snapshot.entities.pop();
  Object.defineProperty(empty.client, 'combatScout', { get() { throw new Error('zero-hint peer scout must not be scanned'); } });
  assert.equal(selectNativeAction(empty.other, empty.context).scoutHomeIndex, 0);
  assert.equal(empty.rows.some(row => row.type === 'combatScoutSelection'), false, 'zero hints still produce no diagnostic row');
});

test('scout selection diagnostics distinguish no active other waypoint without changing the first reachable hint', () => {
  const f = antiConvergenceScoutFixture(), snapshot = JSON.stringify(f.other.snapshot);
  const plan = selectNativeAction(f.other, f.context);
  assert.equal(plan.scoutHomeIndex, 1); assert.equal(plan.scouting, true); assert.equal(plan.command.objectId, undefined);
  assert.deepEqual(scoutSelectionDiagnosticCounts(f.rows.at(-1)), {
    hintCount: 2, activeOtherWaypointCount: 0, sharedHintCount: 0, unsharedHintCount: 2, hintPreferenceReason: 'no-active-other-waypoints',
  });
  assert.equal(f.rows.at(-1).pathSearches, 2); assert.equal(f.rows.at(-1).hintPathSearches, 1);
  assert.equal(f.rows.at(-1).hintOrderChangedByActiveScoutPreference, false);
  assert.equal(JSON.stringify(f.other.snapshot), snapshot); assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
});

test('scout selection diagnostics count mixed alternatives and report the existing stable reordering', () => {
  const f = antiConvergenceScoutFixture(); assert.equal(selectNativeAction(f.client, f.context).scoutHomeIndex, 1);
  const priorScout = f.client.combatScout, priorGoal = { ...priorScout.goal }, selected = selectNativeAction(f.other, f.context);
  assert.equal(selected.scoutHomeIndex, 2); assert.ok(['walk', 'run'].includes(selected.command.type));
  assert.deepEqual(scoutSelectionDiagnosticCounts(f.rows.at(-1)), {
    hintCount: 2, activeOtherWaypointCount: 1, sharedHintCount: 1, unsharedHintCount: 1, hintPreferenceReason: 'reordered',
  });
  assert.equal(f.rows.at(-1).hintOrderChangedByActiveScoutPreference, true); assert.equal(f.rows.at(-1).pathSearches, 2);
  assert.equal(f.client.combatScout, priorScout); assert.deepEqual(priorScout.goal, priorGoal); assert.equal(priorScout.confirmedMoves, 0);
  assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
});

test('scout selection diagnostics distinguish all-unshared alternatives while preserving their cyclic order', () => {
  const f = antiConvergenceScoutFixture(), a = f.secondPeer.snapshot.entities.pop(), b = f.thirdPeer.snapshot.entities.pop();
  assert.equal(selectNativeAction(f.client, f.context).scoutHomeIndex, 0);
  f.secondPeer.snapshot.entities.push(a); f.thirdPeer.snapshot.entities.push(b);
  const prior = f.client.combatScout, plan = selectNativeAction(f.other, f.context);
  assert.equal(plan.scoutHomeIndex, 1, 'all-unshared hints retain original order rather than distance sorting');
  assert.deepEqual(scoutSelectionDiagnosticCounts(f.rows.at(-1)), {
    hintCount: 2, activeOtherWaypointCount: 1, sharedHintCount: 0, unsharedHintCount: 2, hintPreferenceReason: 'all-hints-unshared',
  });
  assert.equal(f.rows.at(-1).hintOrderChangedByActiveScoutPreference, false); assert.equal(f.rows.at(-1).pathSearches, 2);
  assert.equal(f.client.combatScout, prior); assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
});

test('scout selection diagnostics distinguish all-shared alternatives without creating an exclusive claim', () => {
  const f = antiConvergenceScoutFixture(); assert.equal(selectNativeAction(f.client, f.context).scoutHomeIndex, 1);
  assert.equal(selectNativeAction(f.other, f.context).scoutHomeIndex, 2);
  const third = new CapacityClient('ws://127.0.0.1:1/ws', actor(5), f.context);
  Object.assign(third, { snapshot: { ...nativeSnapshot(), entities: [{ ...nativeSnapshot().entities[0], name: third.label, x: 5, y: 7 }] },
    ready: true, inGame: true, role: 'combat', home: { x: 5, y: 7 }, combatArrival: { at: 1 }, combatCluster: f.client.combatCluster,
    combatScoutCursor: 0, nextNativeChatAt: Infinity, nextNativeTurnAt: Infinity });
  f.context.clients.push(third);
  const plan = selectNativeAction(third, f.context);
  assert.equal(plan.scoutHomeIndex, 1); assert.equal(plan.command.objectId, undefined);
  assert.deepEqual(scoutSelectionDiagnosticCounts(f.rows.at(-1)), {
    hintCount: 2, activeOtherWaypointCount: 2, sharedHintCount: 2, unsharedHintCount: 0, hintPreferenceReason: 'all-hints-shared',
  });
  assert.equal(f.rows.at(-1).hintOrderChangedByActiveScoutPreference, false); assert.equal(f.rows.at(-1).pathSearches, 2);
  assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
});

test('scout selection diagnostics keep the original per-phase row cap and omit counts from its truncation sentinel', () => {
  const f = antiConvergenceScoutFixture(), snapshot = JSON.stringify(f.other.snapshot);
  for (let i = 0; i < 36; i++) {
    f.other.combatScout = null; f.other.combatScoutCursor = 0;
    const plan = selectNativeAction(f.other, f.context); assert.equal(plan.scoutHomeIndex, 1); assert.equal(plan.command.objectId, undefined);
  }
  const rows = f.rows.filter(row => row.type === 'combatScoutSelection'); assert.equal(rows.length, 32);
  assert.ok(rows.slice(0, -1).every(row => row.hintCount === 2 && row.activeOtherWaypointCount === 0 &&
    row.sharedHintCount === 0 && row.unsharedHintCount === 2 && row.hintPreferenceReason === 'no-active-other-waypoints' &&
    row.pathSearches === 2 && row.hintPathSearches === 1 && row.truncated === false));
  const sentinel = rows.at(-1); assert.equal(sentinel.truncated, true); assert.equal(sentinel.rowLimit, 32);
  for (const key of ['hintCount', 'activeOtherWaypointCount', 'sharedHintCount', 'unsharedHintCount', 'hintPreferenceReason'])
    assert.equal(Object.hasOwn(sentinel, key), false, 'the original cap sentinel shape is retained');
  assert.equal(JSON.stringify(f.other.snapshot), snapshot); assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
  assert.equal(JSON.stringify(rows).includes('entities'), false); assert.equal(JSON.stringify(rows).includes('private-pass'), false);
  f.context.phase = 'stage-10'; f.other.combatScout = null; f.other.combatScoutCursor = 0;
  assert.equal(selectNativeAction(f.other, f.context).scoutHomeIndex, 1);
  assert.equal(f.rows.filter(row => row.type === 'combatScoutSelection').length, 33);
});


function targetWaypointScoutFixture() {
  const f = observerHintScoutFixture();
  Object.assign(f.hintTarget, { x: 25, y: 10 });
  f.client.nativeRunPrimedUntil = Infinity;
  return f;
}

function applyTargetWaypointAck(f, plan, sequence, now = Date.now()) {
  const origin = { ...f.owner };
  Object.assign(f.owner, plan.target);
  recordCombatScoutMovement(f.client, f.context, plan, origin,
    { packet: 'UserLocation', sequence, payload: { ...f.owner } }, 'success', now);
}

test('scout target waypoint follows the validated attack tile across real movement ACK ticks without visibility or credit', t => {
  t.mock.timers.enable({ apis: ['Date'], now: 100000 });
  const f = targetWaypointScoutFixture(), beforePeer = JSON.stringify(f.secondPeer.snapshot);
  let plan = selectNativeAction(f.client, f.context);
  const selected = f.rows.find(row => row.type === 'combatScoutSelection'), goal = selected.potentialAttackTile;
  const home = f.client.combatCluster.observerHomes[selected.selectedHomeIndex];
  assert.ok(Math.max(Math.abs(goal.x - home.x), Math.abs(goal.y - home.y)) >= 5);
  assert.deepEqual(plan.navigation.goal, goal);
  assert.deepEqual(selected.selectedWaypoint, goal); assert.equal(selected.selectedWaypointSource, 'hint-attack-tile');
  const scout = f.client.combatScout, visited = scout.visited, startedAt = scout.startedAt;
  for (let tick = 0; tick < 3; tick++) {
    assert.deepEqual(plan.navigation.goal, goal, `tick ${tick} must keep the target waypoint`);
    assert.equal(plan.scouting, true); assert.equal(plan.scoutHomeIndex, selected.selectedHomeIndex);
    assert.equal(plan.command.objectId, undefined); assert.equal(plan.targetEntity, undefined); assert.equal(plan.observer, undefined);
    applyTargetWaypointAck(f, plan, tick + 1);
    assert.equal(f.client.combatScout, scout); assert.equal(scout.startedAt, startedAt);
    assert.equal(scout.visited, visited); assert.equal(scout.confirmedMoves, tick + 1);
    t.mock.timers.tick(750);
    plan = selectNativeAction(f.client, f.context);
  }
  assert.deepEqual(plan.navigation.goal, goal);
  assert.equal(f.client.snapshot.entities.length, 1, 'peer-only target never becomes an owner AOI entity');
  assert.equal(JSON.stringify(f.secondPeer.snapshot), beforePeer);
  assert.equal(f.rows.filter(row => row.type === 'combatScoutSelection').length, 1);
  assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
  assert.equal(f.context.policy.minimumCombatAttacksPerMinute, 18); assert.equal(f.context.policy.minimumCombatHitsPerMinute, 12);
});

test('scout target waypoint invalidation safely falls back without rebuilding the finite scout', () => {
  for (const invalid of ['blocked', 'no-route', 'occupied', 'transfer', 'coverage']) {
    const f = targetWaypointScoutFixture();
    const first = selectNativeAction(f.client, f.context), goal = f.rows.at(-1).potentialAttackTile;
    assert.deepEqual(first.navigation.goal, goal);
    applyTargetWaypointAck(f, first, 1);
    const scout = f.client.combatScout, visited = scout.visited, startedAt = scout.startedAt;
    if (invalid === 'blocked') f.context.map.blocked[goal.y * f.context.map.width + goal.x] = 1;
    else if (invalid === 'no-route') {
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) if (dx || dy)
        f.context.map.blocked[(goal.y + dy) * f.context.map.width + goal.x + dx] = 1;
    } else if (invalid === 'occupied') f.client.snapshot.entities.push({ objectId: 9900, kind: 'player', name: 'occupied', x: goal.x, y: goal.y });
    else if (invalid === 'transfer') f.client.snapshot.mapTransfers = [{ bounds: { minX: goal.x, maxX: goal.x, minY: goal.y, maxY: goal.y } }];
    else f.secondPeer.closed = true;
    const plan = selectNativeAction(f.client, f.context);
    if (plan) {
      assert.notDeepEqual(plan.navigation?.goal, goal, invalid);
      assert.ok(insideCombatCoverage(activeCombatCoverage(f.client, f.context), plan.target), invalid);
      assert.notEqual(plan.command.type, 'attack', invalid);
    }
    assert.equal(f.client.combatScout, scout, invalid); assert.equal(scout.startedAt, startedAt, invalid);
    assert.equal(scout.visited, visited, invalid); assert.equal(scout.confirmedMoves, 1, invalid);
    assert.ok(f.client.navigationAttemptCount <= 4, invalid);
    assert.equal(f.context.targetClaims.size, 0, invalid); assert.equal(f.actions.length, 0, invalid);
  }
});

test('scout target waypoint unreachable new hint retains the exact original cyclic fallback within four searches', () => {
  const baseline = targetWaypointScoutFixture(), hinted = targetWaypointScoutFixture();
  baseline.secondPeer.snapshot.entities.pop();
  for (const f of [baseline, hinted]) for (let y = 0; y < f.context.map.height; y++) f.context.map.blocked[y * f.context.map.width + 10] = 1;
  const original = selectNativeAction(baseline.client, baseline.context), selected = selectNativeAction(hinted.client, hinted.context);
  assert.deepEqual(selected, original); assert.deepEqual(hinted.client.combatScout.goal, baseline.client.combatScout.goal);
  const row = hinted.rows.find(row => row.type === 'combatScoutSelection');
  assert.equal(row.reason, 'cyclic-fallback'); assert.ok(row.pathSearches <= 4); assert.ok(row.hintPathSearches >= 1);
  assert.deepEqual(row.selectedWaypoint, selected.navigation.goal); assert.equal(row.selectedWaypointSource, 'declared-home');
  assert.equal(hinted.client.combatScout.confirmedMoves, 0); assert.equal(hinted.context.targetClaims.size, 0); assert.equal(hinted.actions.length, 0);
});

test('scout target waypoint does not redirect an ordinary established scout or reset its saved state', () => {
  const f = targetWaypointScoutFixture(); f.secondPeer.snapshot.entities.pop();
  const original = selectNativeAction(f.client, f.context), scout = f.client.combatScout, goal = { ...scout.goal }, visited = scout.visited;
  scout.startedAt = Date.now() - 19000; scout.activeElapsedMs = 100; scout.confirmedMoves = 23; scout.lastReceiptSequence = 7;
  visited.set('2,2', 2); f.secondPeer.snapshot.entities.push(f.hintTarget);
  const plan = selectNativeAction(f.client, f.context);
  assert.deepEqual(plan, original); assert.deepEqual(scout.goal, goal); assert.equal(f.client.combatScout, scout);
  assert.equal(scout.startedAt <= Date.now() - 19000, true); assert.equal(scout.activeElapsedMs, 100);
  assert.equal(scout.confirmedMoves, 23); assert.equal(scout.lastReceiptSequence, 7); assert.equal(scout.visited, visited); assert.equal(visited.get('2,2'), 2);
  assert.equal(f.rows.filter(row => row.type === 'combatScoutSelection').length, 0);
});

test('scout target waypoint keeps corrected ACK exclusion and the existing time move and visit limits', t => {
  t.mock.timers.enable({ apis: ['Date'], now: 200000 });
  for (const reason of ['movement-budget', 'route-loop', 'time-budget']) {
    const f = targetWaypointScoutFixture(), plan = selectNativeAction(f.client, f.context), goal = f.rows.at(-1).potentialAttackTile;
    assert.deepEqual(plan.navigation.goal, goal);
    const scout = f.client.combatScout, origin = { ...f.owner }, visited = scout.visited;
    if (reason === 'movement-budget') scout.confirmedMoves = 23;
    else if (reason === 'route-loop') scout.visited.set(`${plan.target.x},${plan.target.y}`, 2);
    const count = scout.confirmedMoves, receipt = { packet: 'UserLocation', sequence: 1, payload: { ...plan.target } };
    recordCombatScoutMovement(f.client, f.context, plan, origin, receipt, 'corrected', Date.now());
    assert.equal(f.client.combatScout, scout); assert.equal(scout.confirmedMoves, count); assert.equal(scout.visited, visited);
    Object.assign(f.owner, plan.target);
    recordCombatScoutMovement(f.client, f.context, plan, origin, receipt, 'success', Date.now() + (reason === 'time-budget' ? 20000 : 0));
    assert.equal(f.client.combatScout, null); assert.equal(f.rows.at(-1).reason, reason);
    assert.equal(f.context.targetClaims.size, 0); assert.equal(f.actions.length, 0);
  }
});

test('AOI received state clears a ghost clock during the actual in-range interval between polls', () => {
  const a = observer('a', 280, [entity('b', 297, 50002)]), b = observer('b', 297, [entity('a', 280, 50001)]);
  const clients = [a, b], coverage = new AoiCoverage();
  assert.equal(coverage.sample(clients, 0).issues.length, 0); assert.equal(coverage.extraSince.get('a:b'), 0);
  b.snapshot.entities[0].x = 295; // Real owner ACK at t288: distance15, no observer-side prediction.
  coverage.reconcileReceived?.(clients, 'b'); assert.equal(coverage.extraSince.has('a:b'), false);
  b.snapshot.entities[0].x = 297; // Real owner ACK at t956: distance17 again.
  coverage.reconcileReceived?.(clients, 'b'); assert.equal(coverage.extraSince.has('a:b'), false);
  assert.equal(coverage.sample(clients, 1080).issues.length, 0); assert.equal(coverage.stats.ghosts, 0);
  assert.equal(coverage.extraSince.get('a:b'), 1080);
  a.snapshot.entities[0].x = 281; // Real observer ACK at t1235: distance16.
  coverage.reconcileReceived?.(clients, 'a'); assert.equal(coverage.extraSince.has('a:b'), false);
  assert.equal(coverage.sample(clients, 2236).issues.length, 0); assert.equal(coverage.stats.ghosts, 0);
});

test('AOI received state retains strict continuous ghost grace and ignores unrelated actor notifications', () => {
  const a = observer('a', 0, [entity('b', 17, 50002)]), b = observer('b', 17), c = observer('c', 100);
  const clients = [a, b, c], coverage = new AoiCoverage(); coverage.sample(clients, 0);
  coverage.reconcileReceived?.(clients, 'c'); assert.equal(coverage.extraSince.get('a:b'), 0);
  assert.equal(coverage.sample(clients, 1000).issues.some(issue => issue.reason === 'ghost'), false);
  assert.equal(coverage.sample(clients, 1001).issues.some(issue => issue.observer === 'a' && issue.owner === 'b' && issue.reason === 'ghost'), true);
  const empty = new AoiCoverage(), untouched = new Proxy([], { get() { throw new Error('No cohort scan is needed without pending clocks'); } });
  assert.equal(typeof empty.reconcileReceived, 'function'); empty.reconcileReceived(untouched, 'a');
});

test('AOI received state clears missing only for exactly one entry or no longer required visibility', () => {
  const a = observer('a', 0), b = observer('b', 10, [entity('a', 0, 50001)]), clients = [a, b], coverage = new AoiCoverage();
  coverage.sample(clients, 0); assert.equal(coverage.missingSince.get('a:b'), 0);
  a.snapshot.entities.push(entity('b', 10, 50002), entity('b', 10, 59999));
  coverage.reconcileReceived?.(clients, 'a'); assert.equal(coverage.missingSince.get('a:b'), 0);
  assert.equal(coverage.sample(clients, 500).issues.some(issue => issue.reason === 'duplicate'), true);
  a.snapshot.entities.pop(); coverage.reconcileReceived?.(clients, 'a'); assert.equal(coverage.missingSince.has('a:b'), false);
  a.snapshot.entities.pop(); coverage.reconcileReceived?.(clients, 'a'); assert.equal(coverage.missingSince.has('a:b'), false); // The hook never starts a new timer.
  coverage.sample(clients, 700); assert.equal(coverage.missingSince.get('a:b'), 700);
  const priorStats = { ...coverage.stats };
  b.snapshot.entities[0].x = 17; coverage.reconcileReceived?.(clients, 'b'); assert.equal(coverage.missingSince.has('a:b'), false);
  assert.deepEqual(coverage.stats, priorStats); // Reconciliation gives no visibility credit or sample.
});

test('AOI received state clears changed receiver remove snapshot and map replacement without clearing another pair', () => {
  const a = observer('a', 0, [entity('b', 17, 50002)]), b = observer('b', 17), c = observer('c', 100, [entity('d', 117, 50004)]), d = observer('d', 117);
  const clients = [a, b, c, d], coverage = new AoiCoverage(); coverage.sample(clients, 0);
  a.snapshot.entities.pop(); coverage.reconcileReceived?.(clients, 'a'); assert.equal(coverage.extraSince.has('a:b'), false);
  assert.equal(coverage.extraSince.get('c:d'), 0);
  b.snapshot.entities[0].x = 10; coverage.sample(clients, 100); assert.equal(coverage.missingSince.get('a:b'), 100);
  a.snapshot = { ...a.snapshot, mapFileName: '1' }; coverage.reconcileReceived?.(clients, 'a'); assert.equal(coverage.missingSince.has('a:b'), false);
  assert.equal(coverage.extraSince.get('c:d'), 0); assert.equal(coverage.stats.samples, 2);
});

test('AOI received state hook follows real decoded owner peer Join Leave snapshot and map state', () => {
  const rows = [], { client, context, owner, peer, snapshot, packet } = lifecycleFixture({ add: row => rows.push(row) });
  const other = observer(peer.name, 27, [entity(owner.name, 10, 50010)]); other.snapshot.entities[0].y = 10;
  context.clients = [client, other]; context.aoi = new AoiCoverage();
  client.observe(snapshot([{ ...owner, x: 10 }, { ...peer, x: 27 }]));
  context.aoi.sample(context.clients, 0); const key = `${client.label}:${other.label}`; assert.equal(context.aoi.extraSince.get(key), 0);
  client.observe(packet('UserLocation', { x: 12, y: 10, direction: 'Right' }));
  assert.equal(client.snapshot.entities.find(item => item.objectId === owner.objectId).x, 12); assert.equal(context.aoi.extraSince.has(key), false);
  client.observe(packet('ObjectRemove', { objectId: peer.objectId })); context.aoi.sample(context.clients, 100); assert.equal(context.aoi.missingSince.get(key), 100);
  client.observe(packet('ObjectPlayer', { ...peer, x: 27 })); assert.equal(context.aoi.missingSince.has(key), false);
  other.snapshot.entities[0].x = 40; context.aoi.sample(context.clients, 200); assert.equal(context.aoi.extraSince.get(key), 200);
  client.observe(packet('ObjectWalk', { objectId: peer.objectId, location: { x: 39, y: 10 }, direction: 'Left' })); assert.equal(context.aoi.extraSince.get(key), 200);
  client.observe(packet('ObjectRemove', { objectId: peer.objectId })); assert.equal(context.aoi.extraSince.has(key), false);
  other.snapshot.entities[0].x = 20; context.aoi.sample(context.clients, 300); assert.equal(context.aoi.missingSince.get(key), 300);
  client.observe(snapshot([{ ...owner, x: 12 }, { ...peer, x: 20 }])); assert.equal(context.aoi.missingSince.has(key), false);
  client.observe(packet('ObjectRemove', { objectId: peer.objectId })); context.aoi.sample(context.clients, 400); assert.equal(context.aoi.missingSince.get(key), 400);
  client.observe({ type: 'worldSnapshot', payload: { mapFileName: '1', playerObjectId: owner.objectId, entities: [{ ...owner, x: 12 }] } });
  assert.equal(context.aoi.missingSince.has(key), false); assert.equal(context.aoi.stats.samples, 5);
});
