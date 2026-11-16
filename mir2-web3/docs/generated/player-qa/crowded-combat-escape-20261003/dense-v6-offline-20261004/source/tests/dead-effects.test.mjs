import test from 'node:test';
import assert from 'node:assert/strict';
import { deadActionProof } from '../dense_observation.mjs';

const ownerId = 1000, targetId = 293202;
function snapshot(spell = 'FireBall') {
  return { mapFileName: 'D021', playerObjectId: ownerId, playerHp: 0, playerMp: 541,
    knownSkills: [{ spell, mpCost: 3 }], inventoryItems: [], equipmentItems: [],
    beltItems: [{ uniqueId: 80, name: 'Amulets', quantity: 10 }],
    entities: [{ objectId: ownerId, kind: 'player', x: 59, y: 56, direction: 'Down', dead: true },
      { objectId: targetId, kind: 'monster', disposition: 'hostile', x: 59, y: 55, dead: false, hp: 25 }] };
}
function event(sequence, tsMs, options = {}) {
  const s = { ownerId, map: 'D021', x: 59, y: 56, direction: 'Down', hp: 0, mp: 541,
    exactHp: true, exactMp: true, dead: true,
    hpProvenance: { ownerId, absolute: true, source: 'worldSnapshot', observedMs: tsMs },
    mpProvenance: { ownerId, absolute: true, source: 'worldSnapshot', observedMs: tsMs } };
  return { sequence, tsMs, direction: 'received', ...options, state: { ...s, ...options.state } };
}
function fixture(spell = 'FireBall') {
  const base = snapshot(spell);
  const rows = [event(100, 4000, { packet: 'Death', payload: {} }),
    event(101, 4100, { type: 'worldSnapshot', payload: structuredClone(base) }),
    ...['walk', 'run', 'attack'].map((type, i) => event(102 + i, 4150 + i * 10, { direction: 'sent', type })),
    event(105, 4200, { direction: 'sent', type: 'magic', spell, objectId: ownerId,
      targetId: spell === 'Healing' ? ownerId : targetId, x: 59, y: spell === 'Healing' ? 56 : 55 }),
    event(106, 4800, { type: 'worldSnapshot', payload: structuredClone(base) }),
    event(107, 9600, { type: 'worldSnapshot', payload: structuredClone(base) })];
  const args = { deathSequence: 100, commandsEndSequence: 105, fromMs: 4000, endMs: 9700,
    ownerId, spell, before: { map: 'D021', x: 59, y: 56, direction: 'Down' } };
  return { rows, args, base };
}
function proof(f) { return deadActionProof(f.rows, f.args); }
function modifyMp(f, mp) { for (const r of f.rows.filter(e => e.sequence > 105)) r.state.mp = r.payload.playerMp = mp; }

test('known spell without ACK still fails on explicit absolute owner MP spend, including exact R17 Wizard/Taoist pattern', () => {
  for (const spell of ['FireBall', 'Healing']) {
    const f = fixture(spell); modifyMp(f, 538);
    const p = proof(f);
    assert.equal(p.status, 'failed'); assert.equal(p.castBarrier.status, 'failed');
    assert.equal(p.castBarrierVerified, false); assert.equal(p.absoluteMpSpendReceipts[0].sequence, 106);
  }
});
test('victim-only target damage without independent owner evidence stays inconclusive and never gives a passed caster barrier', () => {
  const f = fixture();
  f.rows.push(event(108, 9650, { packet: 'DamageIndicator', payload: { objectId: targetId, damage: 8, damageType: 0 } }));
  const p = proof(f); assert.equal(p.status, 'inconclusive'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.unattributedTargetDamageReceipts[0].sequence, 108);
});
test('material consumption keeps exact cross-grid UID totals and fails without cast ACK', () => {
  const f = fixture(); f.rows[6].payload.beltItems[0].quantity = 9; f.rows[7].payload.beltItems[0].quantity = 9;
  const p = proof(f); assert.equal(p.status, 'failed'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.materialConsumptionReceipts[0].sequence, 106);
  const moved = fixture();
  for (const e of moved.rows.filter(e => e.sequence > 105)) { e.payload.inventoryItems = e.payload.beltItems; e.payload.beltItems = []; }
  assert.equal(proof(moved).status, 'passed');
});
test('HP rise with Dead=true stays inconclusive; actual owner-life/facing mutation fails without cast ACK', () => {
  for (const mode of ['hp', 'alive', 'direction']) {
    const f = fixture('Healing');
    for (const e of f.rows.filter(e => e.sequence > 105)) {
      if (mode === 'hp') e.state.hp = e.payload.playerHp = 8;
      if (mode === 'alive') { e.state.dead = false; e.payload.entities[0].dead = false; }
      if (mode === 'direction') e.state.direction = e.payload.entities[0].direction = 'Up';
    }
    const p = proof(f); assert.equal(p.status, mode === 'hp' ? 'inconclusive' : 'failed', mode); assert.equal(p.castBarrierVerified, false, mode);
    if (mode === 'hp') { assert.equal(p.ownerLifeMutationReceipts.length, 0); assert.ok(p.ownerHpIncreaseReceipts.length > 0); }
    else assert.ok(p.ownerLifeMutationReceipts.length > 0, mode);
  }
});
test('unknown or percentage-only post-input MP cannot produce a passed cast-death proof', () => {
  for (const mode of ['unknown', 'percent', 'missingPayload', 'foreignProvenance']) {
    const f = fixture();
    for (const e of f.rows.filter(e => e.sequence > 105)) {
      if (mode === 'unknown') { e.state.exactMp = false; e.state.mpProvenance = null; }
      if (mode === 'percent') e.state.mpProvenance = { ownerId, absolute: false, source: 'ObjectMana' };
      if (mode === 'missingPayload') delete e.payload.playerMp;
      if (mode === 'foreignProvenance') e.state.mpProvenance.ownerId = 2222;
    }
    const p = proof(f); assert.equal(p.status, 'inconclusive', mode); assert.equal(p.castBarrierVerified, false, mode);
  }
});
test('missing/invalid input, learned cost or target cannot borrow effects or create a passed barrier', () => {
  for (const mode of ['missingInput', 'wrongInputSpell', 'unlearned', 'costUnknown', 'targetInvalid', 'wrongInputOwner']) {
    const f = fixture();
    if (mode === 'missingInput') f.rows = f.rows.filter(e => e.sequence !== 105);
    if (mode === 'wrongInputSpell') f.rows[5].spell = 'Healing';
    if (mode === 'unlearned') f.rows[1].payload.knownSkills = [];
    if (mode === 'costUnknown') delete f.rows[1].payload.knownSkills[0].mpCost;
    if (mode === 'targetInvalid') f.rows[1].payload.entities[1].dead = true;
    if (mode === 'wrongInputOwner') f.rows[5].objectId = 2222;
    const p = proof(f); assert.equal(p.status, 'inconclusive', mode); assert.equal(p.castBarrierVerified, false, mode);
  }
});
test('closed trial owner gate excludes foreign, future, before-input and wrong-target effects', () => {
  const f = fixture();
  f.rows.push(event(108, 9650, { type: 'worldSnapshot', payload: { ...snapshot(), playerMp: 100 }, state: { ownerId: 2222, mp: 100 } }),
    event(109, 9900, { type: 'worldSnapshot', payload: { ...snapshot(), playerMp: 538 }, state: { mp: 538 } }),
    event(90, 3900, { packet: 'DamageIndicator', payload: { objectId: targetId, damage: 8, damageType: 0 } }),
    event(110, 9660, { packet: 'DamageIndicator', payload: { objectId: 9999, damage: 8, damageType: 0 } }));
  const p = proof(f); assert.equal(p.status, 'passed'); assert.equal(p.absoluteMpSpendReceipts.length, 0);
  assert.equal(p.targetDamageReceipts.length, 0);
});
test('delayed previously committed projectile damage does not relabel a new dead Magic as executed', () => {
  const f = fixture();
  f.rows.unshift(event(90, 3700, { direction: 'sent', type: 'magic', spell: 'FireBall', objectId: ownerId,
    targetId, x: 59, y: 55, state: { hp: 20, dead: false } }),
    event(91, 3750, { packet: 'MagicCast', payload: { spell: 'FireBall' }, state: { hp: 20, dead: false } }),
    event(92, 3800, { packet: 'ObjectMagic', payload: { objectId: ownerId, spell: 'FireBall', cast: true }, state: { hp: 20, dead: false } }));
  f.rows.push(event(108, 9650, { packet: 'DamageIndicator', payload: { objectId: targetId, damage: 8, damageType: 0 } }));
  const p = proof(f); assert.notEqual(p.status, 'failed'); assert.equal(p.targetDamageReceipts.length, 0);
  assert.equal(p.priorProjectileEffectCandidates[0].sequence, 108);
});
test('wrong-spell or cast=false receipts are excluded from committed cast proof, with no false passed barrier from wrong spell', () => {
  const f = fixture();
  f.rows.push(event(108, 9650, { packet: 'ObjectMagic', payload: { objectId: ownerId, spell: 'Healing', cast: true } }));
  const p = proof(f); assert.notEqual(p.castBarrier.status, 'passed'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.excludedCastReceipts[0].sequence, 108);
  const prep = fixture(); prep.rows.push(event(108, 9650, { packet: 'ObjectMagic', payload: { objectId: ownerId, spell: 'FireBall', cast: false } }));
  assert.notEqual(proof(prep).status, 'failed'); assert.equal(proof(prep).preparatoryMagicReceipts[0].sequence, 108);
});
test('invalid or partial material observations cannot be called unchanged or manufacture consumption', () => {
  for (const mode of ['partial', 'duplicate', 'unknownQuantity']) {
    const f = fixture();
    for (const e of f.rows.filter(e => e.sequence > 105)) {
      if (mode === 'partial') delete e.payload.beltItems;
      if (mode === 'duplicate') e.payload.inventoryItems = [{ ...e.payload.beltItems[0] }];
      if (mode === 'unknownQuantity') e.payload.beltItems[0].quantity = null;
    }
    const p = proof(f); assert.equal(p.status, 'inconclusive', mode);
    assert.equal(p.castBarrierVerified, false, mode); assert.equal(p.materialConsumptionReceipts.length, 0, mode);
  }
});

test('another actor committed before this dead input makes victim-only target damage unattributed, not this owner spell proof', () => {
  const f = fixture();
  f.rows.unshift(event(98, 3980, { packet: 'ObjectMagic', payload: { objectId: 2222, spell: 'FireBall', cast: true, targetIds: [targetId] } }));
  f.rows.push(event(108, 9650, { packet: 'DamageIndicator', payload: { objectId: targetId, damage: 8, damageType: 0 } }));
  const p = proof(f); assert.notEqual(p.status, 'failed'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.targetDamageReceipts.length, 0); assert.equal(p.unattributedTargetDamageReceipts[0].sequence, 108);
  const future = fixture();
  future.rows.push(event(109, 9900, { packet: 'ObjectMagic', payload: { objectId: 2222, spell: 'FireBall', cast: true, targetIds: [targetId] } }));
  assert.equal(proof(future).status, 'passed');
});

test('cached owner state cannot borrow another payload owner MP or positive HP/facing as authority', () => {
  const f = fixture();
  for (const e of f.rows.filter(e => e.sequence > 105)) {
    e.payload.playerObjectId = 2222; e.payload.playerMp = e.state.mp = 538;
    e.payload.playerHp = e.state.hp = 8; e.payload.entities[0].direction = 'Up';
  }
  const p = proof(f); assert.equal(p.status, 'inconclusive'); assert.equal(p.castBarrierVerified, false);
  assert.equal(p.absoluteMpSpendReceipts.length, 0); assert.equal(p.ownerLifeMutationReceipts.length, 0);
});

test('terminal cached or percentage zero without an absolute owner HP payload cannot pass death rejection', () => {
  for (const mode of ['unknown', 'percent', 'missingPayload', 'foreignProvenance']) {
    const f = fixture();
    for (const e of f.rows.filter(e => e.sequence > 105)) {
      if (mode === 'unknown') { e.state.exactHp = false; e.state.hpProvenance = null; }
      if (mode === 'percent') e.state.hpProvenance = { ownerId, absolute: false, source: 'ObjectHealth' };
      if (mode === 'missingPayload') delete e.payload.playerHp;
      if (mode === 'foreignProvenance') e.state.hpProvenance.ownerId = 2222;
    }
    const p = proof(f); assert.equal(p.status, 'inconclusive', mode); assert.equal(p.castBarrierVerified, false, mode);
    assert.equal(p.exactDeadSnapshotsAfterCommands, 0, mode);
  }
});

test('a later actual MP debit after an observed MP increase still fails, while future debit stays outside this case', () => {
  const f = fixture(); f.rows[6].payload.playerMp = f.rows[6].state.mp = 545;
  f.rows[7].payload.playerMp = f.rows[7].state.mp = 542;
  const p = proof(f); assert.equal(p.status, 'failed'); assert.equal(p.absoluteMpSpendReceipts[0].sequence, 107);
  const future = fixture();
  future.rows.push(event(108, 9900, { type: 'worldSnapshot', payload: { ...snapshot(), playerMp: 538 }, state: { mp: 538 } }));
  assert.equal(proof(future).status, 'passed');
});

test('material unknown/unsafe identities cannot fabricate a debit or rejection pass; explicit identity zero is kept', () => {
  for (const mode of ['unsafeUid', 'stringUid', 'negativeQuantity', 'booleanQuantity']) {
    const f = fixture();
    for (const e of f.rows.filter(e => e.sequence > 105)) {
      const item = e.payload.beltItems[0];
      if (mode === 'unsafeUid') item.uniqueId = Number.MAX_SAFE_INTEGER + 1;
      if (mode === 'stringUid') item.uniqueId = '80';
      if (mode === 'negativeQuantity') item.quantity = -1;
      if (mode === 'booleanQuantity') item.quantity = true;
    }
    const p = proof(f); assert.equal(p.status, 'inconclusive', mode); assert.equal(p.materialConsumptionReceipts.length, 0, mode);
  }
  const zero = fixture();
  for (const e of zero.rows.filter(e => e.type === 'worldSnapshot')) e.payload.beltItems[0].uniqueId = 0;
  zero.rows[6].payload.beltItems[0].quantity = 9;
  const p = proof(zero); assert.equal(p.status, 'failed'); assert.equal(p.materialConsumptionReceipts[0].consumedItems[0].uniqueId, 0);
});

test('committed owner actions before requested Magic fail generic death, without borrowing them as that spell commitment', () => {
  const f = fixture();
  f.rows.push(event(104, 4180, { packet: 'ObjectMagic', payload: { objectId: ownerId, spell: 'FireBall', cast: true } }));
  const p = proof(f); assert.equal(p.status, 'failed'); assert.equal(p.matchingCastReceipts.length, 0);
  assert.equal(p.castBarrier.status, 'inconclusive'); assert.equal(p.castBarrierVerified, false);
  assert.ok(p.illegalLivingReceipts.some(e => e.packet === 'ObjectMagic'));
});
