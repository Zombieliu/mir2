import test from 'node:test';
import assert from 'node:assert/strict';
import { deadActionProof } from '../dense_observation.mjs';

const ownerId = 1000, targetId = 293202;
const provenance = (source, tsMs, actor = ownerId) => ({ ownerId: actor, absolute: true, source, observedMs: tsMs });
function snapshot(spell = 'FireBall') {
  return { mapFileName: 'D021', playerObjectId: ownerId, playerHp: 0, playerMaxHp: 239,
    playerMp: 541, playerMaxMp: 550, knownSkills: [{ spell, mpCost: 3 }],
    inventoryItems: [], equipmentItems: [{ uniqueId: 0, name: 'Candle', quantity: 1 }],
    beltItems: [{ uniqueId: 80, name: 'Amulets', quantity: 10 }],
    entities: [{ objectId: ownerId, kind: 'player', x: 59, y: 56, direction: 'Down', dead: true, level: 30 },
      { objectId: targetId, kind: 'monster', disposition: 'hostile', x: 59, y: 55, dead: false, hp: 25 }] };
}
function event(sequence, tsMs, extra = {}) {
  return { sequence, tsMs, direction: 'received', ...extra,
    state: { ownerId, map: 'D021', x: 59, y: 56, direction: 'Down', hp: 0, mp: 541,
      exactHp: true, exactMp: true, dead: true,
      hpProvenance: provenance('worldSnapshot', tsMs), mpProvenance: provenance('worldSnapshot', tsMs), ...extra.state } };
}
function health(sequence, tsMs, payload, state = {}) {
  return event(sequence, tsMs, { packet: 'HealthChanged', payload,
    state: { hp: payload.hp, mp: payload.mp, hpProvenance: provenance('HealthChanged', tsMs), mpProvenance: provenance('HealthChanged', tsMs), ...state } });
}
function fixture(spell = 'FireBall') {
  const base = snapshot(spell);
  const rows = [event(100, 4000, { packet: 'Death', payload: {} }),
    event(101, 4100, { type: 'worldSnapshot', payload: structuredClone(base) }),
    event(102, 4150, { direction: 'sent', type: 'walk' }), event(103, 4160, { direction: 'sent', type: 'run' }),
    event(104, 4170, { direction: 'sent', type: 'attack', objectId: targetId }),
    event(105, 4200, { direction: 'sent', type: 'magic', spell, objectId: ownerId,
      targetId: spell === 'Healing' ? ownerId : targetId, x: 59, y: spell === 'Healing' ? 56 : 55 }),
    event(110, 4800, { type: 'worldSnapshot', payload: structuredClone(base) }),
    event(120, 9600, { type: 'worldSnapshot', payload: structuredClone(base) })];
  return { rows, args: { deathSequence: 100, commandsEndSequence: 105, fromMs: 4000, endMs: 9700,
    ownerId, spell, before: { map: 'D021', x: 59, y: 56, direction: 'Down' } } };
}
const proof = f => deadActionProof([...f.rows].sort((a, b) => a.sequence - b.sequence), f.args);
const postSnapshots = f => f.rows.filter(e => e.type === 'worldSnapshot' && e.sequence > 105);
function hit(f, victim = targetId, damage = 8) {
  f.rows.push(event(121, 9650, { packet: 'DamageIndicator', payload: { objectId: victim, damage, damageType: 0 } }));
}
function expectUnattributed(p) {
  assert.equal(p.status, 'inconclusive'); assert.equal(p.castBarrier.status, 'inconclusive'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.targetDamageReceipts.length, 0); assert.ok(p.unattributedTargetDamageReceipts.length > 0);
  assert.equal(p.absoluteMpSpendReceipts.length, 0); assert.equal(p.materialConsumptionReceipts.length, 0);
}
function refill(f, reverse = false) {
  for (const e of postSnapshots(f)) {
    e.state.hp = e.payload.playerHp = 250; e.payload.playerMaxHp = 250;
    e.state.mp = e.payload.playerMp = 550; e.payload.entities[0].level = 31;
  }
  if (reverse) f.rows.push(event(106, 4300, { packet: 'LevelChanged', payload: { level: 31, experience: 0, maxExperience: 9000 } }),
    health(107, 4320, { hp: 250, mp: 550 }));
  else f.rows.push(health(106, 4300, { hp: 250, mp: 541 }), health(107, 4320, { hp: 250, mp: 550 }),
    event(108, 4340, { packet: 'LevelChanged', payload: { level: 31, experience: 0, maxExperience: 9000 } }));
}

test('no observed other skill is not positive owner attribution for a victim-only target hit', () => {
  const f = fixture(); hit(f); const p = proof(f); expectUnattributed(p);
  assert.equal(p.illegalLivingReceipts.length, 0); assert.equal(p.unattributedTargetDamageReceipts[0].sequence, 121);
});
test('a different player physical attack needs no ObjectMagic and cannot be turned into this owner Magic', () => {
  const f = fixture();
  f.rows.push(event(106, 4300, { packet: 'ObjectAttack', payload: { objectId: 2222, direction: 'Right', location: { x: 58, y: 55 } },
    state: { actor: { objectId: 2222, kind: 'player', x: 58, y: 55, direction: 'Right', dead: false } } }));
  hit(f); expectUnattributed(proof(f));
});
test('periodic poison, environment and unobserved projectile sources are observationally indistinguishable in victim-only packets', () => {
  // These labels are test scenarios, not invented packet cause/source fields.
  for (const [scenario, damage] of [['poison', 3], ['environment', 5], ['unobservedProjectile', 8]]) {
    const f = fixture(); hit(f, targetId, damage); const p = proof(f); expectUnattributed(p);
    assert.equal(p.unattributedTargetDamageReceipts[0].payload.damage, damage, scenario);
  }
});
test('actual v4-style dead Attack then dead Magic at one target cannot attribute its single hit to either input', () => {
  const f = fixture(); hit(f); const p = proof(f); expectUnattributed(p);
  assert.ok(p.attemptedCommands.some(e => e.type === 'attack' && e.targetId === targetId));
  assert.equal(p.castBarrier.actualMagicInputSequence, 105);
});
test('observed prior projectile and foreign magic contexts remain raw attribution candidates, never rejection proof', () => {
  for (const mode of ['prior', 'foreign']) {
    const f = fixture();
    if (mode === 'prior') f.rows.push(event(90, 3700, { direction: 'sent', type: 'magic', spell: 'FireBall', objectId: ownerId,
      targetId, x: 59, y: 55, state: { hp: 20, dead: false } }),
      event(91, 3750, { packet: 'MagicCast', payload: { spell: 'FireBall' }, state: { hp: 20, dead: false } }),
      event(92, 3800, { packet: 'ObjectMagic', payload: { objectId: ownerId, spell: 'FireBall', cast: true }, state: { hp: 20, dead: false } }));
    else f.rows.push(event(106, 4300, { packet: 'ObjectMagic', payload: { objectId: 2222, spell: 'FireBall', cast: true, targetIds: [targetId] } }));
    hit(f); const p = proof(f); expectUnattributed(p);
    if (mode === 'prior') assert.equal(p.priorProjectileEffectCandidates[0].sequence, 121);
  }
});
test('incoming damage to the Healing self-target is not proof of forbidden self Healing', () => {
  const f = fixture('Healing'); hit(f, ownerId, 5); expectUnattributed(proof(f));
});
test('actual absolute owner MP debit still fails despite target attribution uncertainty or foreign cast context', () => {
  for (const foreign of [false, true]) {
    const f = fixture();
    for (const e of postSnapshots(f)) e.payload.playerMp = e.state.mp = 538;
    if (foreign) f.rows.push(event(106, 4300, { packet: 'ObjectMagic', payload: { objectId: 2222, spell: 'FireBall', cast: true, targetIds: [targetId] } }));
    hit(f); const p = proof(f); assert.equal(p.status, 'failed'); assert.equal(p.castBarrier.status, 'failed');
    assert.equal(p.castBarrierVerified, false); assert.equal(p.absoluteMpSpendReceipts[0].sequence, 110);
    assert.equal(p.unattributedTargetDamageReceipts[0].sequence, 121);
  }
});
test('independent material debit or a real matching true cast still fails with victim-only target effects', () => {
  for (const mode of ['material', 'commit']) {
    const f = fixture();
    if (mode === 'material') for (const e of postSnapshots(f)) e.payload.beltItems[0].quantity = 9;
    else f.rows.push(event(106, 4300, { packet: 'MagicCast', payload: { spell: 'FireBall' } }),
      event(107, 4350, { packet: 'ObjectMagic', payload: { objectId: ownerId, spell: 'FireBall', cast: true } }));
    hit(f); const p = proof(f); assert.equal(p.status, 'failed'); assert.equal(p.castBarrier.status, 'failed');
    assert.equal(p.castBarrierVerified, false); assert.equal(p.unattributedTargetDamageReceipts.length, 1);
  }
});
test('source LevelUp HealthChanged before LevelChanged with Dead=true is a possible refill, not alive or a rejection pass', () => {
  const f = fixture('Healing'); refill(f); const p = proof(f);
  assert.equal(p.status, 'inconclusive'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.ownerLifeMutationReceipts.length, 0); assert.ok(p.ownerHpIncreaseReceipts.some(e => e.sequence === 106));
  assert.equal(p.sourceLevelUpContext.levelChangedReceipts[0].sequence, 108);
  assert.equal(p.sourceLevelUpContext.provesHpCauseOrRejection, false);
  assert.equal(p.absoluteMpSpendReceipts.length, 0);
});
test('LevelChanged before refill has the same conservative conclusion without an invented packet-order gate', () => {
  const f = fixture('Healing'); refill(f, true); const p = proof(f);
  assert.equal(p.status, 'inconclusive'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.ownerLifeMutationReceipts.length, 0); assert.equal(p.sourceLevelUpContext.levelChangedReceipts[0].sequence, 106);
});
test('HP rise alone with Dead=true or unknown life state cannot identify Healing or revival', () => {
  for (const mode of ['dead', 'unknown', 'healthOnly']) {
    const f = fixture('Healing');
    if (mode === 'healthOnly') f.rows.push(health(106, 4300, { hp: 8, mp: 541 }));
    else for (const e of postSnapshots(f)) {
      e.payload.playerHp = e.state.hp = 8;
      if (mode === 'unknown') { delete e.payload.entities[0].dead; e.state.dead = false; }
    }
    const p = proof(f); assert.equal(p.status, 'inconclusive', mode); assert.equal(p.castBarrierVerified, false, mode);
    assert.equal(p.ownerLifeMutationReceipts.length, 0, mode); assert.ok(p.ownerHpIncreaseReceipts.length > 0, mode);
  }
});
test('future, stale and foreign LevelChanged cannot supply an owner refill exception or acceptance', () => {
  const f = fixture('Healing');
  for (const e of postSnapshots(f)) e.payload.playerHp = e.state.hp = 8;
  f.rows.push(event(99, 3900, { packet: 'LevelChanged', payload: { level: 31 } }),
    event(125, 9900, { packet: 'LevelChanged', payload: { level: 31 } }),
    event(106, 4300, { packet: 'LevelChanged', payload: { objectId: 2222, level: 31 } }),
    event(107, 4320, { packet: 'LevelChanged', payload: { level: 31 }, state: { ownerId: 2222 } }));
  const p = proof(f); assert.equal(p.status, 'inconclusive'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.sourceLevelUpContext.levelChangedReceipts.length, 0);
});
test('level refill cannot erase a later real absolute MP debit after an observed MP increase', () => {
  const f = fixture('Healing'); refill(f); const last = postSnapshots(f).at(-1);
  last.payload.playerMp = last.state.mp = 547;
  const p = proof(f); assert.equal(p.status, 'failed'); assert.equal(p.castBarrier.status, 'failed');
  assert.equal(p.absoluteMpSpendReceipts[0].sequence, 120); assert.equal(p.castBarrierVerified, false);
});
test('LevelChanged does not excuse actual Dead=false, Revived or authoritative movement/facing', () => {
  for (const mode of ['alive', 'revived', 'move', 'facing']) {
    const f = fixture('Healing'); refill(f);
    if (mode === 'revived') f.rows.push(event(109, 4500, { packet: 'Revived', payload: {}, state: { dead: false } }));
    else for (const e of postSnapshots(f)) {
      if (mode === 'alive') { e.payload.entities[0].dead = false; e.state.dead = false; }
      if (mode === 'move') { e.payload.entities[0].x = e.state.x = 60; }
      if (mode === 'facing') e.payload.entities[0].direction = e.state.direction = 'Up';
    }
    const p = proof(f); assert.equal(p.status, 'failed', mode); assert.equal(p.castBarrierVerified, false, mode);
  }
});
test('foreign HealthChanged objectId must not borrow an owner cached absolute MP provenance', () => {
  const f = fixture(); f.rows.push(health(106, 4300, { objectId: 2222, hp: 0, mp: 538 }));
  const p = proof(f); assert.equal(p.status, 'passed'); assert.equal(p.castBarrierVerified, true);
  assert.equal(p.absoluteMpSpendReceipts.length, 0); assert.equal(p.excludedAbsoluteMpReceipts[0].sequence, 106);
});
test('foreign HealthChanged cannot supply missing owner MP coverage or invent a mana failure', () => {
  const f = fixture(); f.rows.push(health(106, 4300, { objectId: 2222, hp: 0, mp: 538 }));
  for (const e of postSnapshots(f)) delete e.payload.playerMp;
  const p = proof(f); assert.equal(p.status, 'inconclusive'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.absoluteMpSpendReceipts.length, 0); assert.equal(p.excludedAbsoluteMpReceipts[0].sequence, 106);
});
test('normal owned HealthChanged with no objectId or exact current owner still proves a real forbidden debit', () => {
  for (const object of [{}, { objectId: ownerId }]) {
    const f = fixture(); f.rows.push(health(106, 4300, { ...object, hp: 0, mp: 538 }));
    const p = proof(f); assert.equal(p.status, 'failed'); assert.equal(p.castBarrier.status, 'failed');
    assert.equal(p.absoluteMpSpendReceipts[0].sequence, 106); assert.equal(p.castBarrierVerified, false);
  }
});
test('missing or invalid actual learned Magic prerequisites cannot turn ambiguous HP/damage into a caster result', () => {
  for (const mode of ['missingInput', 'unlearned', 'unknownCost', 'invalidTarget']) {
    const f = fixture(); hit(f);
    if (mode === 'missingInput') f.rows = f.rows.filter(e => e.sequence !== 105);
    if (mode === 'unlearned') f.rows[1].payload.knownSkills = [];
    if (mode === 'unknownCost') delete f.rows[1].payload.knownSkills[0].mpCost;
    if (mode === 'invalidTarget') f.rows[1].payload.entities[1].dead = true;
    const p = proof(f); assert.equal(p.status, 'inconclusive', mode); assert.equal(p.castBarrierVerified, false, mode);
    assert.equal(p.castBarrier.status, 'not-applicable', mode);
  }
});
test('uncertain attribution, source refill or independent failures never change global acceptance flags', () => {
  const variants = [fixture(), fixture('Healing'), fixture()]; hit(variants[0]); refill(variants[1]);
  for (const e of postSnapshots(variants[2])) e.payload.playerMp = e.state.mp = 538;
  for (const f of variants) for (const flag of ['fullP1Accepted', 'nativeInputAccepted', 'nativeVisualAccepted', 'humanAccepted', 'ordinaryProgressionAccepted', 'loadAccepted']) assert.equal(proof(f)[flag], false);
});
