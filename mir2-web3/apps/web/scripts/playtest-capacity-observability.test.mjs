import test from 'node:test';
import assert from 'node:assert/strict';
import { PublicDensity, beginCapacityPhase, finishCapacityPhase } from './playtest-capacity-observability.mjs';

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
