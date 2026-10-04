import test from 'node:test';
import assert from 'node:assert/strict';
import { DELTAS, ownerState, qualifySeven, escapeProof, driftProof, holdStopProof, eightOccupancy, rejectionProof, deathProof, deadActionProof, castCommitProof, legalExit, prematureCastProof, applyDenseObservation, ordinaryMagicEligibility } from '../dense_observation.mjs';
import { redact } from '../dense_network_driver.mjs';
import { applyProtocolObservation } from '../vendor/protocol-observation.mjs';

const map = { mapFileName: 'D021', width: 32, height: 32, blocked: new Uint8Array(32 * 32), doors: [] };
const metadata = { transfers: [], safeZones: [], fire: false, lightning: false };
function fixture(n = 7) {
  const bats = Object.entries(DELTAS).filter(([d]) => d !== 'Right').slice(0, n).map(([direction, [dx, dy]], i) => ({ objectId: i + 200, kind: 'monster', name: 'CaveBat', ai: 0, disposition: 'hostile', x: 12 + dx, y: 12 + dy, direction, dead: false }));
  const snapshot = { mapFileName: 'D021', playerObjectId: 1000, playerHp: 100, playerMaxHp: 419, playerMp: 100, playerMaxMp: 116, playerHealthObservation: 'exact', denseVitalsProvenance: { ownerId: 1000, hp: { ownerId: 1000, absolute: true, source: 'worldSnapshot' }, mp: { ownerId: 1000, absolute: true, source: 'worldSnapshot' } }, entities: [{ objectId: 1000, kind: 'player', x: 12, y: 12, dead: false }, ...bats] };
  const events = [];
  let sequence = 0;
  for (const t of [1000, 3500]) for (const bat of bats) {
    const direction = Object.keys(DELTAS).find(d => bat.x + DELTAS[d][0] === 12 && bat.y + DELTAS[d][1] === 12);
    events.push({ sequence: ++sequence, tsMs: t, direction: 'received', packet: 'ObjectAttack', payload: { objectId: bat.objectId, location: { x: bat.x, y: bat.y }, direction }, state: { ...ownerState(snapshot, bat.objectId), hp: 200 - sequence } });
    events.push({ sequence: ++sequence, tsMs: t + 60, direction: 'received', packet: 'DamageIndicator', payload: { objectId: 1000, damage: 5, damageType: 0 }, state: { ...ownerState(snapshot), hp: 200 - sequence } });
  }
  return { snapshot, events };
}
function event(sequence, tsMs, packet, x = 13, extra = {}) {
  return { sequence, tsMs, packet, direction: 'received', payload: { x, y: 12 }, ...extra, state: { map: 'D021', ownerId: 1000, x, y: 12, hp: 100, mp: 100, exactHp: true, exactMp: true, hpProvenance: { ownerId: 1000, absolute: true, source: 'worldSnapshot' }, mpProvenance: { ownerId: 1000, absolute: true, source: 'worldSnapshot' }, dead: false, ...extra.state } };
}
function magicInput(sequence, tsMs, spell = 'FireBall', mp = 100) {
  return event(sequence, tsMs, undefined, 12, { direction: 'sent', type: 'magic', spell, objectId: 1000, targetId: 200, x: 13, y: 12, state: { mp } });
}
function mpSnapshot(sequence, tsMs, mp = 95) {
  return event(sequence, tsMs, undefined, 12, { type: 'worldSnapshot', payload: { playerMp: mp }, state: { mp } });
}
function castFixture() {
  const rows = [magicInput(30, 4000), event(31, 4100, 'MagicCast', 12, { payload: { spell: 'FireBall' } }), event(32, 4150, 'ObjectMagic', 12, { payload: { objectId: 1000, spell: 'FireBall', cast: true } }), mpSnapshot(33, 4200), event(35, 4400, 'DamageIndicator', 12, { payload: { objectId: 201, damage: 8, damageType: 0 } })];
  const args = { ownerId: 1000, castSentSequence: 30, castSentMs: 4000, spell: 'FireBall', beforeMp: 100, beforeState: event(29, 3900).state, escapeSentSequence: 34, commitEndMs: 4250, untilMs: 4500 };
  return { rows, args };
}
function recastFixture() {
  const rows = [magicInput(27, 3800), event(28, 3900, 'MagicCast', 12, { payload: { spell: 'FireBall' } }), event(29, 3950, 'ObjectMagic', 12, { payload: { objectId: 1000, spell: 'FireBall', cast: true } }), mpSnapshot(30, 3990), magicInput(31, 4050, 'FireBall', 95), mpSnapshot(32, 4300), mpSnapshot(33, 5000)];
  const args = { ownerId: 1000, spell: 'FireBall', firstSentSequence: 27, firstSentMs: 3800, beforeFirstMp: 100, beforeFirstState: event(26, 3700).state, secondSentMs: 4050, secondSentSequence: 31, beforeSecondMp: 95, beforeSecondState: event(30, 3990, undefined, 12, { state: { mp: 95 } }).state, endMs: 5050 };
  return { rows, args };
}
function deadCastFixture(spell = 'FireBall') {
  const snapshot = { ...fixture().snapshot, playerHp: 0, knownSkills: [{ spell, mpCost: 5 }], entities: [{ objectId: 1000, kind: 'player', x: 12, y: 12, dead: true }, { objectId: 200, kind: 'monster', disposition: 'hostile', x: 13, y: 12, dead: false }] };
  const dead = { hp: 0, dead: true };
  const rows = [event(31, 4100, undefined, 12, { type: 'worldSnapshot', payload: snapshot, state: dead }), ...['walk', 'run', 'attack'].map((type, i) => event(32 + i, 4150 + i * 50, undefined, 12, { direction: 'sent', type, objectId: type === 'attack' ? 200 : undefined, state: dead })), magicInput(35, 4350, spell), event(36, 9200, undefined, 12, { type: 'worldSnapshot', payload: snapshot, state: dead })];
  Object.assign(rows[4], spell === 'Healing' ? { targetId: 1000, x: 12 } : {});
  rows[4].state = { ...rows[4].state, ...dead };
  const args = { deathSequence: 30, commandsEndSequence: 35, fromMs: 4000, endMs: 9300, before: { map: 'D021', x: 12, y: 12 }, ownerId: 1000, spell };
  return { snapshot, rows, args };
}
test('Seven distinct aimed real hostile cycles and positive owner hits qualify; all acceptance flags remain false', () => {
  const f = fixture(); const q = qualifySeven(f.events, f.snapshot, map, metadata, 4000);
  assert.equal(q.status, 'qualified'); assert.equal(q.attackerIds.length, 7); assert.equal(q.positiveOwnerHitCount, 14); assert.equal(q.exit.direction, 'Right');
  assert.equal(q.fullP1Accepted, false); assert.equal(q.nativeInputAccepted, false); assert.equal(q.humanAccepted, false);
});
test('Two attackers repeated 20 times never count as seven', () => {
  const f = fixture(2); f.events = Array.from({ length: 20 }, () => f.events).flat();
  assert.equal(qualifySeven(f.events, f.snapshot, map, metadata, 4000).status, 'inconclusive');
});
test('Seven Miss/zero damages, no exact HP loss, wrong facing or neutral AI fail qualification', () => {
  for (const mode of ['miss', 'hp', 'facing', 'neutral']) {
    const f = fixture();
    for (const e of f.events) {
      if (mode === 'miss' && e.packet === 'DamageIndicator') { e.payload.damage = 0; e.payload.damageType = 4; }
      if (mode === 'hp') e.state.hp = 100;
      if (mode === 'facing') e.payload.direction = 'Right';
      if (mode === 'neutral' && e.state.actor) e.state.actor.disposition = 'neutral';
    }
    assert.equal(qualifySeven(f.events, f.snapshot, map, metadata, 4000).status, 'inconclusive', mode);
  }
});
test('A live monster or static wall in either run cell removes legal exit; no walk-through', () => {
  const f = fixture();
  f.snapshot.entities.push({ objectId: 999, kind: 'monster', x: 14, y: 12, dead: false });
  assert.equal(legalExit(f.snapshot, map, metadata, true), null);
  f.snapshot.entities.pop(); const blocked = { ...map, blocked: map.blocked.slice() }; blocked.blocked[12 * 32 + 13] = 1;
  assert.equal(legalExit(f.snapshot, blocked, metadata), null);
  f.snapshot.playerPoison = 32; assert.equal(legalExit(f.snapshot, map, metadata), null);
});
test('Delayed owner Struck is a visual receipt, not authority correction', () => {
  const f = fixture(); f.snapshot.entities[0].x = 13;
  const after = applyProtocolObservation(f.snapshot, { type: 'packet', packet: 'ObjectStruck', payload: { objectId: 1000, location: { x: 12, y: 12 } } });
  assert.equal(after.entities[0].x, 13);
});
test('ACK bound retains late failure, incoming damage and old-position corrections', () => {
  const f = fixture(); const qualification = qualifySeven(f.events, f.snapshot, map, metadata, 4000);
  const rows = [event(31, 4100, 'UserLocation', 12), event(32, 4200, 'DamageIndicator', 12, { payload: { objectId: 1000, damage: 5, damageType: 0 } }), event(33, 4600, 'UserLocation')];
  const options = { qualification, sentSequence: 30, sentMs: 4000, endMs: 6500, ownerId: 1000, before: { map: 'D021', x: 12, y: 12 }, target: { x: 13, y: 12 }, committedAckSequence: 29 };
  const passed = escapeProof(rows, options); assert.equal(passed.status, 'passed'); assert.equal(passed.latencyMs, 600); assert.equal(passed.corrections.length, 2);
  rows[2].tsMs = 6000; const late = escapeProof(rows, options); assert.equal(late.status, 'failed'); assert.equal(late.ackSequence, 33); assert.equal(late.positiveOwnerHitsDuringPending.length, 1);
  assert.equal(escapeProof(rows.slice(0, 2), options).status, 'failed');
});
test('An escape with no incoming positive owner hit during pending wait stays inconclusive', () => {
  const f = fixture();
  assert.equal(escapeProof([event(31, 4520, 'UserLocation')], { qualification: qualifySeven(f.events, f.snapshot, map, metadata, 4000), sentSequence: 30, sentMs: 4000, endMs: 5000, ownerId: 1000, before: { map: 'D021', x: 12, y: 12 }, target: { x: 13, y: 12 } }).status, 'inconclusive');
});
test('Full 5-second stop collects snapshots and fails a drift at the end, without dropping failure', () => {
  const rows = [1, 2, 3, 4, 5].map((n) => event(n + 30, 4000 + n * 1000, undefined, 13, { type: 'worldSnapshot' }));
  const args = { anchorSequence: 30, anchorMs: 4000, anchor: { map: 'D021', x: 13, y: 12 }, endMs: 9000, ownerId: 1000 };
  assert.equal(driftProof(rows, args).status, 'passed');
  rows.at(-1).state.x = 14; const failed = driftProof(rows, args); assert.equal(failed.status, 'failed'); assert.equal(failed.driftReceipts.at(-1).sequence, 35);
  assert.equal(driftProof(rows.slice(0, 2), { ...args, endMs: 6000 }).status, 'inconclusive');
});
test('Held stop allows at most one existing pending step then 5 seconds stable, rejects queued steps and late receipt', () => {
  const rows = [event(31, 4500, 'UserLocation'), ...[1, 2, 3, 4, 5, 6].map(n => event(n + 31, 4500 + n * 1000, undefined, 13, { type: 'worldSnapshot' }))];
  const args = { stopSequence: 30, stopMs: 4000, atStop: { map: 'D021', x: 12, y: 12 }, endMs: 10500, ownerId: 1000 };
  assert.equal(holdStopProof(rows, args).status, 'passed');
  rows[1].state.x = 14; assert.equal(holdStopProof(rows, args).status, 'failed');
  rows[1].state.x = 13; rows[0].tsMs = 6500; assert.equal(holdStopProof(rows, args).status, 'failed');
});
test('Seven adjacent monsters do not prove full surround; eight and ordinary rejection are separate', () => {
  const f = fixture(); assert.equal(eightOccupancy(f.snapshot, map).status, 'inconclusive');
  f.snapshot.entities.push({ objectId: 299, kind: 'monster', disposition: 'hostile', x: 13, y: 12, dead: false });
  assert.equal(eightOccupancy(f.snapshot, map).status, 'qualified');
  const args = { sentSequence: 30, before: { map: 'D021', x: 12, y: 12 }, blockerId: 299, ownerId: 1000, requiredWindowMs: 1800, sentMs: 4000, endMs: 6000 };
  assert.equal(rejectionProof([event(31, 4100, 'UserLocation', 12)], args).status, 'passed');
  assert.equal(rejectionProof([event(31, 4100, 'UserLocation')], args).status, 'failed');
});
test('Rounded zero percent or fabricated death without positive owner hit is not actual death lifecycle', () => {
  const rows = [event(31, 4100, 'ObjectHealth', 12, { payload: { objectId: 1000, percent: 0 }, state: { exactHp: false, hp: 1, dead: false } })];
  assert.equal(deathProof(rows, 1000, { fromMs: 4000, endMs: 4500 }).status, 'inconclusive');
  rows.push(event(32, 4200, 'Death', 12, { state: { exactHp: true, hp: 0, dead: true } }));
  assert.equal(deathProof(rows, 1000, { fromMs: 4000, endMs: 4500 }).status, 'inconclusive');
  rows.unshift(event(30, 4000, 'DamageIndicator', 12, { payload: { objectId: 1000, damage: 5, damageType: 0 } }));
  assert.equal(deathProof(rows, 1000, { fromMs: 4000, endMs: 4500 }).status, 'qualified');
});
test('Actual death blocks later movement/attack/eligible cast; a committed cast after dead inputs fails', () => {
  const { rows, args } = deadCastFixture();
  const proof = deadActionProof(rows, args);
  assert.equal(proof.status, 'passed'); assert.equal(proof.castBarrierVerified, true); assert.equal(proof.castBarrier.actualMagicInputSequence, 35);
  rows.push(event(37, 9250, 'ObjectMagic', 12, { payload: { objectId: 1000, spell: 'FireBall', cast: true } }));
  assert.equal(deadActionProof(rows, args).status, 'failed');
});
test('Actual committed cast requires accepted ACK plus MP spend; effects after escape retained', () => {
  const { rows, args } = castFixture();
  const proof = castCommitProof(rows, args);
  assert.equal(proof.status, 'qualified'); assert.deepEqual(proof.effectsAfterEscape, [35]); assert.equal(proof.ownerCastSequence, 32); assert.equal(proof.mpReceiptSequence, 33);
  assert.equal(castCommitProof(rows.slice(0, 1), args).status, 'inconclusive');
});
test('Premature repeated cast uses real MP/cast receipts; second accepted cast or spend fails', () => {
  const { rows, args } = recastFixture();
  const proof = prematureCastProof(rows, args);
  assert.equal(proof.status, 'passed'); assert.equal(proof.firstCast.spell, 'FireBall'); assert.equal(proof.firstMpSpendObservedInWindow, true);
  rows.push(event(34, 5020, 'MagicCast', 12, { payload: { spell: 'FireBall' } })); assert.equal(prematureCastProof(rows, args).status, 'failed');
  rows.pop(); rows.at(-1).state.mp = rows.at(-1).payload.playerMp = 90; assert.equal(prematureCastProof(rows, args).status, 'failed');
  assert.equal(prematureCastProof(rows, { ...args, secondSentMs: 5200, endMs: 6200 }).status, 'inconclusive');
});
test('Secrets are recursively redacted in nested errors and packet payloads without discarding failure', () => {
  const safe = redact({ error: 'Rejected PRIVATE-TEST-PASSWORD', payload: { password: 'PRIVATE-TEST-PASSWORD', identityToken: 'PRIVATE-TEST-TOKEN', status: 'failed', uid: 123 } }, ['PRIVATE-TEST-PASSWORD', 'PRIVATE-TEST-TOKEN']);
  assert.equal(safe.payload.status, 'failed'); assert.equal(safe.payload.uid, 123); assert.ok(!JSON.stringify(safe).includes('PRIVATE-TEST'));
});
test('Only explicit absolute receipt provenance can mark HP/MP exact; unknown and percentage zero stay unknown', () => {
  const f = fixture(); delete f.snapshot.denseVitalsProvenance;
  f.snapshot.playerHp = 0; f.snapshot.playerMp = 0;
  assert.equal(ownerState(f.snapshot).exactHp, false); assert.equal(ownerState(f.snapshot).exactMp, false);
  let observed = applyDenseObservation(null, { type: 'worldSnapshot', payload: { ...f.snapshot, playerHp: 1, playerMp: 2 } }, 1000);
  assert.equal(ownerState(observed).exactHp, true); assert.equal(ownerState(observed).hpProvenance.source, 'worldSnapshot');
  observed = applyDenseObservation(observed, { type: 'packet', packet: 'ObjectHealth', payload: { objectId: 1000, percent: 0 } }, 1100);
  assert.equal(ownerState(observed).exactHp, false); assert.equal(observed.playerHp, 1);
  observed = applyDenseObservation(observed, { type: 'packet', packet: 'ObjectMana', payload: { objectId: 1000, percent: 0 } }, 1200);
  assert.equal(ownerState(observed).exactMp, false);
  observed = applyDenseObservation(observed, { type: 'worldSnapshot', payload: { ...f.snapshot, playerHp: null, playerMp: null } }, 1300);
  assert.equal(ownerState(observed).exactHp, false); assert.equal(ownerState(observed).exactMp, false);
  const rows = [event(31, 4000, 'DamageIndicator', 12, { payload: { objectId: 1000, damage: 5, damageType: 0 } }), event(32, 4100, 'Death', 12, { state: { hp: 0, dead: true, hpProvenance: null } })];
  assert.equal(deathProof(rows, 1000, { fromMs: 4000, endMs: 4500 }).status, 'inconclusive');
  const cast = [event(31, 4100, 'MagicCast'), event(32, 4200, 'HealthChanged', 12, { state: { mp: 0, mpProvenance: null } })];
  assert.equal(castCommitProof(cast, { ownerId: 1000, castSentSequence: 30, castSentMs: 4000, beforeMp: 100, untilMs: 4500 }).status, 'inconclusive');
});
test('Unknown/percentage HP decreases cannot qualify seven real owner hits', () => {
  for (const mode of ['unknown', 'percentage', 'foreignProvenance']) {
    const f = fixture();
    for (const e of f.events) { e.state.hpProvenance = mode === 'unknown' ? null : { ownerId: mode === 'foreignProvenance' ? 999 : 1000, absolute: mode !== 'percentage', source: mode === 'percentage' ? 'ObjectHealth' : 'HealthChanged' }; }
    assert.equal(qualifySeven(f.events, f.snapshot, map, metadata, 4000).status, 'inconclusive', mode);
  }
});
test('Closed trial window and actual owner reject future/foreign death, ACK and cast receipt contamination', () => {
  const f = fixture(), qualification = qualifySeven(f.events, f.snapshot, map, metadata, 4000);
  const rows = [event(31, 4150, 'DamageIndicator', 12, { payload: { objectId: 1000, damage: 5, damageType: 0 } }), event(32, 4500, 'UserLocation'), event(33, 4800, 'Death', 12, { state: { ownerId: 2222, hp: 0, dead: true } }), event(34, 7000, 'Death', 12, { state: { hp: 0, dead: true } })];
  const args = { qualification, sentSequence: 30, sentMs: 4000, endMs: 5000, ownerId: 1000, before: { map: 'D021', x: 12, y: 12 }, target: { x: 13, y: 12 } };
  assert.equal(escapeProof(rows, args).status, 'passed');
  assert.equal(deathProof(rows, 1000, { fromMs: 4000, endMs: 5000 }).status, 'inconclusive');
  assert.equal(deathProof(rows, 1000, { fromMs: 4000, endMs: 8000 }).status, 'qualified');
  const cast = [event(31, 4200, 'HealthChanged', 12, { state: { mp: 95 } }), event(32, 4300, 'MagicCast', 12, { state: { ownerId: 2222 } }), event(33, 7000, 'MagicCast')];
  assert.equal(castCommitProof(cast, { ownerId: 1000, castSentSequence: 30, castSentMs: 4000, beforeMp: 100, untilMs: 5000 }).status, 'inconclusive');
  assert.throws(() => escapeProof(rows, { ...args, endMs: undefined }));
});

test('Wrong spell receipts or mismatched actual Magic input cannot qualify the requested cast', () => {
  for (const mode of ['wrongAck', 'wrongOwnerCast', 'wrongInput', 'missingInput']) {
    const { rows, args } = castFixture();
    if (mode === 'wrongAck') rows[1].payload.spell = 'Healing';
    if (mode === 'wrongOwnerCast') rows[2].payload.spell = 'Healing';
    if (mode === 'wrongInput') rows[0].spell = 'Healing';
    if (mode === 'missingInput') rows.shift();
    assert.equal(castCommitProof(rows, args).status, 'inconclusive', mode);
  }
});

test('ObjectMagic cast=false or absent cast is preparation, not a committed requested spell pair', () => {
  for (const cast of [false, undefined]) {
    const { rows, args } = castFixture(); rows[2].payload.cast = cast;
    const proof = castCommitProof(rows, args);
    assert.equal(proof.status, 'inconclusive'); assert.equal(proof.ownerCastSequence, null);
    assert.deepEqual(proof.excludedCastReceipts.map(e => e.sequence), [32]);
  }
});

test('First paired cast and actual absolute MP spend must share the closed commitment window', () => {
  for (const mode of ['pairAfterClose', 'mpAfterClose', 'mpBeforeInput', 'cachedMpOnAck', 'unknownBaseline', 'percentMp']) {
    const { rows, args } = castFixture();
    if (mode === 'pairAfterClose') rows[2].tsMs = 4300;
    if (mode === 'mpAfterClose') rows[3].tsMs = 4300;
    if (mode === 'mpBeforeInput') rows[3].tsMs = 3900;
    if (mode === 'cachedMpOnAck') { rows[3].type = 'packet'; rows[3].packet = 'MagicDelay'; rows[3].payload = { spell: 'FireBall', delay: 1800 }; }
    if (mode === 'unknownBaseline') args.beforeState.mpProvenance = null;
    if (mode === 'percentMp') rows[3].state.mpProvenance = { ownerId: 1000, absolute: false, source: 'ObjectMana' };
    assert.equal(castCommitProof(rows, args).status, 'inconclusive', mode);
  }
});

test('Early recast cannot pass from another first spell, preparation-only pair, or later MP spend', () => {
  for (const mode of ['wrongFirstSpell', 'firstPreparation', 'missingFirstMp', 'firstMpAfterSecond', 'wrongSecondInput']) {
    const { rows, args } = recastFixture();
    if (mode === 'wrongFirstSpell') rows[1].payload.spell = rows[2].payload.spell = 'Healing';
    if (mode === 'firstPreparation') rows[2].payload.cast = false;
    if (mode === 'missingFirstMp') rows.splice(3, 1);
    if (mode === 'firstMpAfterSecond') rows[3].tsMs = 4080;
    if (mode === 'wrongSecondInput') rows.find(e => e.sequence === 31).spell = 'Healing';
    assert.equal(prematureCastProof(rows, args).status, 'inconclusive', mode);
  }
});

test('Early recast matches actual second spell and separates cast=false from an accepted cast', () => {
  const { rows, args } = recastFixture();
  rows.push(event(34, 5020, 'ObjectMagic', 12, { payload: { objectId: 1000, spell: 'FireBall', cast: false } }));
  let proof = prematureCastProof(rows, args);
  assert.equal(proof.status, 'passed'); assert.equal(proof.acceptedSecondCastReceipts.length, 0); assert.equal(proof.excludedSecondCastReceipts.length, 1);
  rows.at(-1).payload.cast = true;
  assert.equal(prematureCastProof(rows, args).status, 'failed');
  rows.at(-1).payload.spell = 'Healing';
  proof = prematureCastProof(rows, args);
  assert.equal(proof.acceptedSecondCastReceipts.length, 0); assert.equal(proof.excludedSecondCastReceipts[0].payload.spell, 'Healing');
});

test('Owner Death or exact zero HP inside early-recast waiting is failure, never successful cooldown denial', () => {
  for (const mode of ['Death', 'exactHpZero']) {
    const { rows, args } = recastFixture();
    rows.push(event(34, 5020, mode === 'Death' ? 'Death' : undefined, 12, { type: mode === 'Death' ? 'packet' : 'worldSnapshot', payload: { playerHp: 0, playerMp: 95 }, state: { hp: 0, mp: 95, dead: mode === 'Death' } }));
    const proof = prematureCastProof(rows, args);
    assert.equal(proof.status, 'failed', mode); assert.equal(proof.deathReceiptSequence, 34); assert.match(proof.reason, /not cooldown rejection/);
  }
});

test('Unknown/percent zero and future/foreign death do not fabricate an early-recast failure', () => {
  const { rows, args } = recastFixture();
  rows.push(event(34, 5020, 'ObjectHealth', 12, { payload: { objectId: 1000, percent: 0 }, state: { hp: 0, hpProvenance: { ownerId: 1000, absolute: false, source: 'ObjectHealth' } } }), event(35, 5030, 'Death', 12, { state: { ownerId: 2222, hp: 0, dead: true } }), event(36, 7000, 'Death', 12, { state: { hp: 0, dead: true } }));
  const proof = prematureCastProof(rows, args);
  assert.equal(proof.status, 'passed'); assert.equal(proof.deathReceiptSequence, null);
  args.beforeSecondState.mpProvenance = null;
  assert.equal(prematureCastProof(rows, args).status, 'inconclusive');
});

test('Dead caster cannot claim a barrier from absent, dead, distant or wrong-kind targets or wrong coordinates', () => {
  for (const mode of ['absent', 'dead', 'distant', 'npc', 'self', 'wrongCoordinates']) {
    const { rows, args } = deadCastFixture();
    const input = rows.find(e => e.type === 'magic'), snapshot = rows[0].payload;
    if (mode === 'absent') snapshot.entities.pop();
    if (mode === 'dead') snapshot.entities[1].dead = true;
    if (mode === 'distant') snapshot.entities[1].x = 25;
    if (mode === 'npc') snapshot.entities[1].kind = 'npc';
    if (mode === 'self') input.targetId = 1000;
    if (mode === 'wrongCoordinates') input.x = 14;
    const proof = deadActionProof(rows, args);
    assert.equal(proof.status, 'inconclusive', mode); assert.equal(proof.castBarrierVerified, false); assert.equal(proof.castBarrier.status, 'not-applicable');
  }
});

test('Dead caster needs actual known MP cost, sufficient absolute MP, learned spell and matching Magic input', () => {
  for (const mode of ['lowMp', 'unknownMp', 'unknownCost', 'unlearned', 'missingMagic', 'wrongMagic', 'wrongOwner']) {
    const { rows, args } = deadCastFixture();
    const snapshot = rows[0].payload, input = rows.find(e => e.type === 'magic');
    if (mode === 'lowMp') { snapshot.playerMp = 4; rows[0].state.mp = 4; }
    if (mode === 'unknownMp') rows[0].state.mpProvenance = null;
    if (mode === 'unknownCost') snapshot.knownSkills[0].mpCost = null;
    if (mode === 'unlearned') snapshot.knownSkills = [];
    if (mode === 'missingMagic') rows.splice(rows.indexOf(input), 1);
    if (mode === 'wrongMagic') input.spell = 'Healing';
    if (mode === 'wrongOwner') input.objectId = 2222;
    const proof = deadActionProof(rows, args);
    assert.equal(proof.status, 'inconclusive', mode); assert.equal(proof.castBarrierVerified, false); assert.equal(proof.castBarrier.status, 'not-applicable');
  }
});

test('Dead Healing has an otherwise-valid self target; Warrior cast is explicitly not applicable', () => {
  const healing = deadCastFixture('Healing');
  assert.equal(ordinaryMagicEligibility(healing.snapshot, 'Healing', 1000).status, 'eligible');
  assert.equal(deadActionProof(healing.rows, healing.args).castBarrierVerified, true);
  const warrior = deadCastFixture(); warrior.args.spell = null; warrior.rows = warrior.rows.filter(e => e.type !== 'magic');
  const proof = deadActionProof(warrior.rows, warrior.args);
  assert.equal(proof.status, 'passed'); assert.equal(proof.castBarrier.status, 'not-applicable'); assert.equal(proof.castBarrierVerified, false);
});

test('Dead cast eligibility cannot be borrowed from a future or another owner snapshot', () => {
  for (const mode of ['future', 'foreign']) {
    const { rows, args } = deadCastFixture();
    if (mode === 'future') rows[0].sequence = 37;
    if (mode === 'foreign') rows[0].state.ownerId = 2222;
    const proof = deadActionProof(rows, args);
    assert.equal(proof.status, 'inconclusive', mode); assert.equal(proof.castBarrierVerified, false);
  }
});
