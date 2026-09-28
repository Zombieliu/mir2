import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { capacityPolicy, validateCapacityPool, LoginBudget, LOGIN_WINDOW_MS, Histogram, Measurements,
  assessMeasurements, ContinuousAcceptance, NativePlan, nativeCadence, nativeActivityVerdict, AoiCoverage, verifiedCombatHit, BoundedEvidence, memoryTrend, TimeBucketSamples, sanitize } from './playtest-capacity-soak-core.mjs';
import { CapacityClient, FixedPlan, chooseHomes, loadStationaryNpcObstacles, spacingFourScenario, selectNativeAction, offerNativeAction, combatCandidates, stableFanoutRange, matchesFanoutReceipt, actionFailureStatus, awaitPendingActions, actorsReachedScenario, runCapacity, parseArguments } from './playtest-capacity-soak.mjs';
import { observedPlayerId } from './playtest-multiplayer-smoke.mjs';
import { loadProtocolCollisionMap, protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';

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
  await assert.rejects(fs.stat(path.join(directory, 'private')), /ENOENT/);
}));

test('slow real socket acknowledgements expose offered-load gaps instead of silently lowering the requested activity', async () => withDirectory(async directory => {
  const realm = await fakeRealm(directory, { delayedWalk: true });
  try {
    const report = await runCapacity(runOptions(realm, directory));
    assert.equal(report.ok, false); assert.match(report.error, /Stage 1 did not satisfy/);
    assert.equal(report.stages.length, 1); assert.equal(report.stages[0].passed, false);
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
    for (const item of realm.types) assert.ok(realm.types.filter(other => other.at > item.at - 1000 && other.at <= item.at).length <= 3, 'at most 3 commands per actor in any rolling second');
  } finally { await realm.close(); }
}));

for (const movementIntervalMs of [650, 750]) test(`native ${movementIntervalMs}ms real socket probe produces sustained two-cell Runs and honest offered load`, async () => withDirectory(async directory => {
  const realm = await fakeRealm(directory);
  try {
    const report = await runCapacity({ ...runOptions(realm, directory), activityMode: 'native', movementIntervalMs });
    assert.equal(report.error, undefined); assert.equal(report.ok, true);
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

for (const { lateFirstHit, sparseStats } of [{ lateFirstHit: false, sparseStats: false }, { lateFirstHit: true, sparseStats: false }, { lateFirstHit: false, sparseStats: true }]) test(lateFirstHit
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
      const input = JSON.parse(data); commands.push({ index, type: input.type, at: performance.now() });
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
      if (input.type === 'attack') {
        const owner = active.get(index);
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
        if (!lateFirstHit) deliver();
        else if (commands.filter(command => command.type === 'attack').length === 1) setTimeout(deliver, 4500);
      }
      if (input.type === 'logOut') { active.delete(index); send(socket, 'LogOutSuccess', { characters: [] }); }
    });
  });
  const policy = capacityPolicy({ profile: 'probe', activityMode: 'native', movementIntervalMs: 650, stages: '2', combatActors: 1 });
  const context = { endpoint, policy, runId: 'native-wire-fixture', phase: 'native-wire-fixture', clients: [],
    map: { width: 32, height: 32, blocked: new Uint8Array(1024) }, mapName: '0', monsterNames: ['Hen'], targetClaims: new Map(),
    guard: { assert() {}, stop: error => errors.push(error) }, writer: { add: row => evidence.push(row) }, add: row => rows.push(row) };
  const clients = [0, 1].map(index => new CapacityClient(endpoint, actor(index), context)); context.clients = clients;
  try {
    for (const [index, client] of clients.entries()) {
      await client.connect(); await client.request({ type: 'login', accountId: client.account.accountId, password: client.account.password }, 'LoginSuccess');
      await client.request({ type: 'startGame', characterIndex: index }, 'StartGame');
      client.role = index ? 'combat' : 'movement'; client.home = { x: index ? 12 : 8, y: index ? 10 : 8 };
      client.nextNativeChatAt = Infinity; client.nextNativeTurnAt = Infinity;
    }
    const start = performance.now() + 1200; for (const client of clients) client.plan = new NativePlan(start);
    if (lateFirstHit) {
      // Timers can wake a fraction early. Reach the real monotonic deadline
      // before offering the single attack; never advance the production clock.
      for (let attempts = 0; attempts < 100 && performance.now() < start; attempts++) {
        await sleep(Math.max(1, Math.min(25, start - performance.now())));
      }
      assert.ok(performance.now() >= start, 'bounded fixture wait reaches the actual legal deadline');
      offerNativeAction(clients[1], context); assert.ok(clients[1].pending); await clients[1].pending;
      assert.equal(rows.filter(row => row.kind === 'attack').length, 1);
      assert.equal(rows.find(row => row.kind === 'attack').ambiguousTargetQuarantined, true);
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
