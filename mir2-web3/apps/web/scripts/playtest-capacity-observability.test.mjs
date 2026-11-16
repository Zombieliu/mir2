import test from 'node:test';
import assert from 'node:assert/strict';
import { PublicDensity, SustainObservations, beginCapacityPhase, finishCapacityPhase } from './playtest-capacity-observability.mjs';

const client = (mapFileName, role = 'movement', kinds = []) => ({ ready: true, inGame: true, role,
  snapshot: { mapFileName, playerObjectId: 1000, groundDrops: [], entities: [
    { objectId: 1000, kind: 'selfPlayer', name: 'never-serialize-owner-name', password: 'never-serialize-password' },
    ...kinds.map((kind, i) => ({ objectId: 2000 + i, kind, name: 'never-serialize-entity-name', x: 123, y: 456 })),
  ] } });

test('density classifies actual entity kinds and counts drops separately without serializing identities or payloads', () => {
  const actor = client('0', 'combat', ['remotePlayer', 'Player', 'monster', 'npc', 'unknown', 'Monster']);
  actor.snapshot.entities.at(-1).dead = true; // A visible corpse is still an entity in this snapshot.
  actor.snapshot.groundDrops = [{ objectId: 2000 }, { secret: 'never-serialize-drop' }];
  const sample = new PublicDensity().sample([actor]);
  assert.deepEqual(sample.maps[0].maxVisible, { total: 7, players: 3, monsters: 2, npcs: 1, other: 1, groundDropsReported: 2 });
  assert.equal(sample.cohortActive, 1); assert.equal(sample.cohortCombatActive, 1);
  assert.match(sample.scope, /not added to total/);
  assert.doesNotMatch(JSON.stringify(sample), /objectId|password|secret|never-serialize|123|456/);
});

test('density separates per-map admitted active cohorts from visible counts and excludes paused, closed, unready and absent owners', () => {
  const a = client('0', 'combat', ['monster']), b = client('0'), c = client('1', 'combat', ['npc']);
  const paused = { ...client('0'), paused: true }, closed = { ...client('0'), closed: true };
  const unready = { ...client('0'), ready: false }, out = { ...client('0'), inGame: false }, absent = client('0'); absent.snapshot.entities = [];
  const sample = new PublicDensity().sample([a, b, c, paused, closed, unready, out, absent]);
  assert.equal(sample.cohortActive, 3); assert.equal(sample.cohortCombatActive, 2);
  assert.deepEqual(sample.maps.map(m => [m.mapFileName, m.cohortActive, m.cohortCombatActive, m.observerSamples]), [['0', 2, 1, 2], ['1', 1, 1, 1]]);
  assert.equal(sample.maps[0].maxVisible.players, 1, 'visible players are actual snapshot counts, not inferred from the cohort size');
});

test('density accumulates bounded sampled independent maxima without rewriting prior samples or returned summaries', () => {
  const density = new PublicDensity(), first = density.sample([client('0', 'combat', ['monster', 'npc'])]);
  const second = density.sample([client('0', 'movement', ['player', 'player', 'player'])]);
  const summary = density.summary();
  assert.equal(summary.sampleCount, 2); assert.equal(summary.samplingIntervalMs, 1000);
  assert.equal(summary.maps[0].observerSamples, 2); assert.equal(summary.maps[0].maxCohortCombatActive, 1);
  assert.deepEqual(summary.maps[0].maxVisible, { total: 4, players: 4, monsters: 1, npcs: 1, other: 0, groundDropsReported: 0 });
  assert.match(summary.scope, /different observers/);
  assert.equal(first.maps[0].maxVisible.players, 1); assert.equal(second.maps[0].maxVisible.monsters, 0);
  summary.maps[0].maxVisible.total = 999;
  assert.equal(density.summary().maps[0].maxVisible.total, 4);
  assert.throws(() => density.sample(Array.from({ length: 101 }, () => client('0'))), /at most 100/);
});

test('phase raw boundaries and report windows use one explicit clock, and completed never claims assessment success', () => {
  const rows = [], context = { phaseWindows: [], writer: { add: row => rows.push(row) } };
  const phase = beginCapacityPhase(context, 'stage-2', 'measurement', 2, 1700000000000);
  finishCapacityPhase(context, phase, true, 1700000030000);
  finishCapacityPhase(context, phase, false, 1700000031000);
  assert.equal(rows.length, 2); assert.deepEqual(rows.map(r => r.type), ['phaseStarted', 'phaseEnded']);
  assert.equal(rows[0].endedAtMs, null, 'the already-written start record cannot be mutated by closing the window');
  assert.equal(rows[1].startedAtMs, phase.startedAtMs); assert.equal(rows[1].endedAtMs, phase.endedAtMs);
  assert.equal(context.phaseWindows[0], phase); assert.equal(phase.completed, true);
  assert.equal(Object.hasOwn(phase, 'passed'), false);
});

test('an aborted bootstrap closes its real interval as incomplete and invalid clocks cannot manufacture duration', () => {
  const rows = [], context = { writer: { add: row => rows.push(row) } };
  const phase = beginCapacityPhase(context, 'bootstrap-100', 'bootstrap', 100, 1700000000000);
  try { throw new Error('actual admission failed'); }
  catch { /* The original admission error remains the caller's result. */ }
  finally { finishCapacityPhase(context, phase, false, 1700000002500); }
  assert.equal(phase.completed, false); assert.equal(phase.endedAtMs - phase.startedAtMs, 2500);
  assert.equal(rows.at(-1).completed, false);
  const invalid = beginCapacityPhase(context, 'soak', 'measurement', 100, 1700000005000);
  assert.throws(() => finishCapacityPhase(context, invalid, true, 1700000004999), /clock/);
  assert.equal(invalid.endedAtMs, null);
});


test('500-actor density requires explicit target metadata and retains aggregate-only sampling and map-history bounds', () => {
  const cohort = Array.from({ length: 500 }, (_, i) => client('0', i < 50 ? 'combat' : 'movement'));
  assert.throws(() => new PublicDensity().sample(cohort), /at most 100/);
  assert.throws(() => new PublicDensity({ targetActors: 250 }), /explicit capacity target/);
  const density = new PublicDensity({ targetActors: 500 }), sample = density.sample(cohort);
  assert.equal(sample.capacityTargetActors, 500);
  assert.equal(sample.maximumControlledActors, 500);
  assert.equal(sample.cohortActive, 500);
  assert.equal(sample.cohortCombatActive, 50);
  assert.equal(sample.maps[0].maxVisible.players, 1, 'target metadata cannot fabricate visible players');
  assert.equal(density.summary().capacityTargetActors, 500);
  assert.equal(density.summary().maps[0].maxCohortActive, 500);
  assert.doesNotMatch(JSON.stringify(sample), /objectId|password|never-serialize/);
  assert.throws(() => density.sample([...cohort, client('0')]), /at most 500/);
  const boundedMaps = new PublicDensity({ targetActors: 500 });
  boundedMaps.sample(Array.from({ length: 100 }, (_, i) => client(String(i))));
  assert.throws(() => boundedMaps.sample([client('101')]), /map history/);
  assert.equal(Object.hasOwn(new PublicDensity().summary(), 'capacityTargetActors'), false);
});

test('500-actor phase windows require the context opt-in while retaining real counts, clocks and incomplete preparation', () => {
  const rows = [], context = { policy: { targetActors: 500 }, writer: { add: row => rows.push(row) } };
  const phase = beginCapacityPhase(context, 'stage-500', 'measurement', 500, 1000);
  assert.equal(phase.targetActors, 500);
  assert.equal(phase.capacityTargetActors, 500);
  assert.equal(phase.maximumControlledActors, 500);
  finishCapacityPhase(context, phase, false, 2500);
  assert.equal(rows.at(-1).completed, false);
  assert.equal(rows.at(-1).endedAtMs - rows[0].startedAtMs, 1500);
  const prep = beginCapacityPhase(context, 'preparation-25', 'bootstrap', 25, 2500);
  assert.equal(prep.targetActors, 25, 'actual phase cohort stays separate from declared final target');
  assert.equal(prep.capacityTargetActors, 500);
  finishCapacityPhase(context, prep, false, 2600);
  for (const [target, kind] of [[501, 'measurement'], [500, 'preparation']]) {
    assert.throws(() => beginCapacityPhase(context, 'invalid', kind, target, 3000), /Invalid bounded capacity phase/);
  }
  const legacy = { writer: { add() {} } };
  assert.throws(() => beginCapacityPhase(legacy, 'stage-500', 'measurement', 500, 1000), /Invalid bounded capacity phase/);
  const original = beginCapacityPhase(legacy, 'stage-100', 'measurement', 100, 1000);
  assert.equal(Object.hasOwn(original, 'capacityTargetActors'), false);
});

function sustainFixture() {
  const rows = [], observer = new SustainObservations({}, row => rows.push(row));
  const actor = { label: 'ordinary-owner', lifecycleConnection: 1, role: 'combat' };
  const snapshot = { mapFileName: '0', playerObjectId: 1000, playerHp: 18, playerMaxHp: 24, gold: 65,
    entities: [{ objectId: 1000, name: actor.label, hp: 75, maxHp: 100, secret: 'never-retain-entity' }],
    inventoryItems: [{ uniqueId: 1, itemIndex: 658, name: '(HP)DrugSmall', quantity: 10, secret: 'never-retain-item' },
      { uniqueId: 2, itemIndex: 659, name: '(HP)DrugSmall', quantity: 99 }],
    beltItems: [{ uniqueId: 3, itemIndex: 658, quantity: 2 }], secret: 'never-retain-snapshot' };
  const observe = (value = snapshot, sequence = 1, monotonicMs = 900, source = actor) => observer.observe(source,
    { type: 'worldSnapshot', payload: value }, { direction: 'received', sequence, monotonicMs }, 1700000000000 + monotonicMs);
  return { rows, observer, actor, snapshot, observe };
}

test('sustain keeps only actual scalar identity, exact world HP, gold and complete HP658 bag plus belt stock', () => {
  const f = sustainFixture(); f.observe();
  assert.deepEqual(f.rows, [], 'packet observation never writes per-packet rows');
  f.observer.activate(f.actor, 'stage-2', 1000, 1700000001000);
  const row = f.rows[0];
  assert.equal(row.boundary, 'activation'); assert.equal(row.completeMinute, false);
  assert.deepEqual(row.latest.owner, { name: f.actor.label, mapFileName: '0', localId: 1000 });
  assert.deepEqual([row.latest.hp, row.latest.maximumHp, row.latest.gold, row.latest.hpStock], [18, 24, 65, 12]);
  assert.deepEqual(row.latest.complete, { identity: true, hp: true, maximumHp: true, hpStock: true, gold: true });
  assert.deepEqual([row.latest.sequence, row.latest.monotonicMs, row.latest.receivedAtMs, row.latest.ageMs], [1, 900, 1700000000900, 100]);
  assert.equal(row.observations.observedNetHpStockChange, null, 'one endpoint does not prove unchanged stock');
  f.snapshot.inventoryItems[0].quantity = 999; f.snapshot.entities[0].name = 'changed-raw';
  row.latest.owner.name = 'changed-result'; row.latest.complete.hpStock = false;
  const state = f.observer.actors.get(f.actor.label);
  assert.equal(state.latest.hpStock, 12); assert.equal(state.latest.owner.name, f.actor.label);
  assert.equal(state.latest.complete.hpStock, true);
  assert.doesNotMatch(JSON.stringify(state), /inventoryItems|beltItems|entities|never-retain/);
  assert.match(row.scope, /not proof of consumption/); assert.match(row.scope, /none proves potion healing/);
});

test('sustain incomplete or malformed actual inventories are unknown instead of zero and missing exact HP never uses owner percent', () => {
  const mutations = [snapshot => delete snapshot.inventoryItems, snapshot => delete snapshot.beltItems,
    snapshot => delete snapshot.inventoryItems[0].itemIndex, snapshot => delete snapshot.inventoryItems[0].quantity,
    snapshot => snapshot.inventoryItems[0].quantity = '10', snapshot => snapshot.beltItems.push({ ...snapshot.inventoryItems[0] }),
    snapshot => snapshot.beltItems[0].uniqueId = Number.MAX_SAFE_INTEGER + 1];
  for (const mutate of mutations) {
    const f = sustainFixture(); mutate(f.snapshot); delete f.snapshot.playerHp; delete f.snapshot.playerMaxHp; delete f.snapshot.gold;
    f.observe(); f.observer.activate(f.actor, 'stage-2', 1000, 1000);
    const value = f.rows[0].latest;
    assert.deepEqual([value.hpStock, value.hp, value.maximumHp, value.gold], [null, null, null, null]);
    assert.equal(value.complete.identity, true); assert.equal(value.complete.hpStock, false); assert.equal(value.complete.hp, false);
  }
  const f = sustainFixture(); f.snapshot.inventoryItems = []; f.snapshot.beltItems = [];
  f.observe(); f.observer.activate(f.actor, 'stage-2', 1000, 1000);
  assert.equal(f.rows[0].latest.hpStock, 0, 'two actual complete empty arrays prove zero stock');
});

test('sustain accepts numeric or canonical string UID zero and still rejects duplicate zero across bag and belt', () => {
  for (const id of [0, '0']) {
    const f = sustainFixture(); f.snapshot.inventoryItems[0].uniqueId = id;
    f.observe(); f.observer.activate(f.actor, 'stage-2', 1000, 1000);
    assert.equal(f.rows[0].latest.hpStock, 12); assert.equal(f.rows[0].latest.complete.hpStock, true);
    const duplicate = structuredClone(f.snapshot); duplicate.beltItems[0].uniqueId = id === 0 ? '0' : 0;
    f.observe(duplicate, 2, 2000); f.observer.drain(f.actor, 'drain', 2100, 2100);
    assert.equal(f.rows.at(-1).latest.hpStock, null); assert.equal(f.rows.at(-1).latest.complete.hpStock, false);
  }
});

test('sustain safe-zone membership is only the current raw boolean on the same actual scalar receipt', () => {
  const f = sustainFixture(); f.snapshot.inSafeZone = true; f.observe();
  f.observer.activate(f.actor, 'stage-2', 1000, 1000);
  assert.equal(f.rows[0].latest.inSafeZone, true);
  const next = structuredClone(f.snapshot); next.inSafeZone = false; f.observe(next, 2, 2000);
  f.observer.tick('stage-2', 61000, 61000); assert.equal(f.rows.at(-1).latest.inSafeZone, false);
  delete next.inSafeZone; f.observe(next, 3, 62000); f.observer.tick('soak', 121000, 121000);
  assert.equal(f.rows.at(-1).latest.inSafeZone, null, 'missing current membership never reuses the older true or false');
  next.inSafeZone = 'true'; f.observe(next, 4, 122000); f.observer.drain(f.actor, 'drain', 123000, 123000);
  assert.equal(f.rows.at(-1).latest.inSafeZone, null);
});

test('sustain replaces an older complete scalar with a newer incomplete actual snapshot without reusing its known stock or HP', () => {
  const f = sustainFixture(); f.observe(); f.observer.activate(f.actor, 'stage-2', 1000, 1000);
  const newer = structuredClone(f.snapshot); delete newer.inventoryItems; delete newer.playerHp; delete newer.gold;
  f.observe(newer, 2, 2000); f.observer.drain(f.actor, 'drain', 2500, 2500);
  const row = f.rows.at(-1);
  assert.equal(row.latest.sequence, 2); assert.equal(row.latest.ageMs, 500);
  assert.equal(row.latest.hpStock, null); assert.equal(row.latest.hp, null); assert.equal(row.latest.gold, null);
  assert.equal(row.observations.unknownStockSnapshots, 1); assert.equal(row.observations.observedNetHpStockChange, null);
});

test('sustain actual minute windows separate ACKs, proven quantities and numeric HP increases without replaying stalled minutes', () => {
  const f = sustainFixture(); f.observe(); f.observer.activate(f.actor, 'stage-2', 1000, 1000);
  const next = structuredClone(f.snapshot); next.inventoryItems[0].quantity = 9; next.playerHp = 15;
  f.observe(next, 2, 60000);
  f.observer.recordHpRecovery(f.actor, 'ack'); f.observer.recordHpRecovery(f.actor, 'consumption', 1);
  f.observer.recordHpRecovery(f.actor, 'hp-increase');
  f.observer.tick('stage-2', 60999, 60999); assert.equal(f.rows.length, 1);
  f.observer.tick('stage-2', 61000, 61000);
  const minute = f.rows.at(-1);
  assert.equal(minute.boundary, 'minute'); assert.equal(minute.elapsedMs, 60000); assert.equal(minute.completeMinute, true);
  assert.equal(minute.latest.ageMs, 1000); assert.equal(minute.latest.observedDuringInterval, true);
  assert.equal(minute.observations.observedNetHpStockChange, -1);
  assert.equal(minute.observations.hpRecoveryAcknowledgements, 1);
  assert.equal(minute.observations.hpRecoveryConsumptionProofs, 1);
  assert.equal(minute.observations.hpRecoveryProvenConsumedQuantity, 1);
  assert.equal(minute.observations.hpRecoveryHpIncreaseProofs, 1);
  assert.equal(minute.observations.hpRecoveryProvenConsumedPerMinute, 1);
  assert.deepEqual([minute.observations.minimumObservedHp, minute.observations.maximumObservedHp], [15, 15]);
  f.observer.tick('soak', 241000, 241000);
  assert.equal(f.rows.length, 3, 'a three-minute stall closes one real interval');
  assert.equal(f.rows.at(-1).elapsedMs, 180000); assert.equal(f.rows.at(-1).observations.hpRecoveryAcknowledgements, 0);
  f.observer.tick('soak', 241001, 241001); assert.equal(f.rows.length, 3);
  assert.equal(minute.observations.hpRecoveryAcknowledgements, 1, 'a reset cannot mutate emitted rows');
});

test('sustain no-receipt minute reports scalar age and unknown net change without manufacturing freshness', () => {
  const f = sustainFixture(); f.observe(); f.observer.activate(f.actor, 'stage-2', 1000, 1000);
  f.observer.tick('stage-2', 61000, 61000);
  const row = f.rows.at(-1);
  assert.equal(row.latest.sequence, 1); assert.equal(row.latest.ageMs, 60100); assert.equal(row.latest.observedDuringInterval, false);
  assert.equal(row.observations.worldSnapshots, 0); assert.equal(row.observations.observedNetHpStockChange, null);
  f.observer.observe(f.actor, { type: 'worldSnapshot', payload: f.snapshot }, { direction: 'sent', sequence: 2, monotonicMs: 62000 }, 62000);
  f.observe(f.snapshot, 1, 63000);
  assert.equal(f.observer.actors.get(f.actor.label).latest.sequence, 1, 'sent and duplicate receipts do not refresh observation age');
});

test('sustain moving an HP stack from bag to belt preserves net stock and cannot prove consumption or healing', () => {
  const f = sustainFixture(); f.observe(); f.observer.activate(f.actor, 'stage-2', 1000, 1000);
  const moved = structuredClone(f.snapshot); moved.beltItems.push(moved.inventoryItems.shift());
  f.observe(moved, 2, 2000); f.observer.drain(f.actor, 'drain', 2100, 2100);
  const row = f.rows.at(-1);
  assert.equal(row.latest.hpStock, 12); assert.equal(row.observations.observedNetHpStockChange, 0);
  assert.equal(row.observations.hpRecoveryProvenConsumedQuantity, 0); assert.equal(row.observations.hpRecoveryHpIncreaseProofs, 0);
});

test('sustain identity and connection changes never splice stock deltas across owners, maps or resumed local IDs', () => {
  for (const change of ['owner', 'map', 'local-id', 'connection']) {
    const f = sustainFixture(); f.observe(); f.observer.activate(f.actor, 'stage-2', 1000, 1000);
    const next = structuredClone(f.snapshot); next.inventoryItems[0].quantity--;
    const source = { ...f.actor };
    if (change === 'owner') next.entities[0].name = 'another-owner';
    if (change === 'map') next.mapFileName = '1';
    if (change === 'local-id') { next.playerObjectId = 1001; next.entities[0].objectId = 1001; }
    if (change === 'connection') source.lifecycleConnection = 2;
    f.observe(next, 2, 2000, source);
    if (change === 'connection') f.observer.bindResumedConnection(source);
    f.observer.drain(source, 'drain', 2100, 2100);
    const row = f.rows.at(-1);
    assert.equal(row.boundary, 'drain'); assert.equal(row.observations.observedNetHpStockChange, null, change);
    assert.ok(row.observations.identityChanges > 0, change);
    if (change === 'owner') assert.equal(row.latest.complete.identity, false);
  }
});

test('sustain stops minute publication at the existing offered-load end and drains with the actual final receipt', () => {
  const f = sustainFixture(); f.observe(); f.observer.activate(f.actor, 'stage-2', 1000, 1000);
  f.observer.stop(59000); f.observer.tick('drain', 121000, 121000); assert.equal(f.rows.length, 1);
  const final = structuredClone(f.snapshot); final.gold = 80; final.inventoryItems[0].quantity = 8;
  f.observe(final, 2, 121000); f.observer.drain(f.actor, 'drain', 121500, 121500);
  const row = f.rows.at(-1);
  assert.equal(row.boundary, 'drain'); assert.equal(row.completeMinute, false); assert.equal(row.elapsedMs, 120500);
  assert.equal(row.offeredEndedAtMonotonicMs, 59000); assert.equal(row.latest.gold, 80); assert.equal(row.latest.ageMs, 500);
  f.observer.drain(f.actor, 'drain', 122000, 122000); assert.equal(f.rows.length, 2);
});

test('sustain actor storage stays bounded by the explicit cohort policy and retains no raw snapshot history', () => {
  for (const [policy, maximum] of [[{}, 100], [{ targetActors: 500 }, 500]]) {
    const observer = new SustainObservations(policy);
    for (let i = 0; i <= maximum; i++) {
      const actor = { label: `owner-${i}`, lifecycleConnection: i + 1 };
      observer.observe(actor, { type: 'worldSnapshot', payload: {} }, { direction: 'received', sequence: 1, monotonicMs: i }, 1000 + i);
    }
    assert.equal(observer.actors.size, maximum);
    assert.doesNotMatch(JSON.stringify([...observer.actors.values()]), /inventoryItems|beltItems|entities|password/);
  }
});
