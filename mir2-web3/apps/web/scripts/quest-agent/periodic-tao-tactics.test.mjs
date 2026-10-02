import assert from 'node:assert/strict';
import test from 'node:test';
import { createPeriodicTaoCombatAction, ownedPeriodicBoneFamiliar } from './periodic-tao-tactics.mjs';

function material(name, uniqueId, quantity = 30, overrides = {}) {
  const index = name === 'Amulet' ? 712 : 710, shape = name === 'Amulet' ? 0 : 1;
  return { name, uniqueId, quantity, durabilityCurrent: 1000, durabilityMax: 1000,
    tooltipSource: { info: { item_index: index, item_type: 8, shape,
      required_class: 4, required_type: 0, required_amount: name === 'Amulet' ? 18 : 14 } }, ...overrides };
}
function fixture(options = {}) {
  const sent = [], receipts = [], delegated = [], timeouts = [];
  const actor = { objectId: 1, kind: 'player', name: 'ProofTao', class: 'Taoist', level: 25,
    hp: 180, maxHp: 180, dead: false, x: 20, y: 20, direction: 'Right' };
  const target = { objectId: 40, kind: 'monster', name: 'Centipede', hp: 210, dead: false,
    disposition: 'Hostile', x: 23, y: 20 };
  const snapshot = { playerObjectId: 1, playerHp: 180, playerMaxHp: 180, playerMp: 100, playerMaxMp: 184,
    mapFileName: 'D001', entities: [actor, target], inventoryItems: [material('GreenPoison', 102)],
    equipmentItems: [material('Amulet', 101, 30, { slot: 'Amulet' })], beltItems: [], maxBagSlots: 40,
    knownSkills: ['SummonSkeleton', 'Poisoning', 'SoulFireBall'].map(spell => ({ spell, mpCost: spell === 'SummonSkeleton' ? 12 : spell === 'Poisoning' ? 2 : 3,
      cooldownRemainingTicks: 0, cooldownRemainingMs: 0 })) };
  const client = { snapshot, events: [], sequence: 0, failure: null, closed: false, probes: 0 };
  const event = (packet, payload, type = 'packet') => {
    const value = { direction: 'received', type, packet, payload: structuredClone(payload), sequence: ++client.sequence };
    client.events.push(value); return value;
  };
  client.send = command => {
    sent.push(structuredClone(command));
    client.events.push({ direction: 'sent', type: command.type, ...command, sequence: ++client.sequence });
    if (command.type === 'clientVersion') {
      client.probes++;
      options.onRefresh?.(client);
      client.snapshot = structuredClone(client.snapshot);
      if (options.omitMonsterPoison) for (const entity of client.snapshot.entities) if (entity.kind === 'monster') delete entity.poison;
      event(undefined, client.snapshot, 'worldSnapshot');
    } else if (command.type === 'equipItem') {
      const found = client.snapshot.inventoryItems.find(item => item.uniqueId === command.uniqueId);
      if (options.rejectEquip?.(command) || !found) { event('EquipItem', { success: false }); return; }
      const old = client.snapshot.equipmentItems.find(item => item.slot === 'Amulet');
      client.snapshot.equipmentItems = client.snapshot.equipmentItems.filter(item => item !== old);
      client.snapshot.inventoryItems = client.snapshot.inventoryItems.filter(item => item !== found);
      if (old) { delete old.slot; client.snapshot.inventoryItems.push(old); }
      client.snapshot.equipmentItems.push({ ...found, slot: options.wrongEquipSlot ? 'BraceletRight' : 'Amulet',
        uniqueId: options.wrongEquipId ? found.uniqueId + 200 : found.uniqueId });
      if (!options.noEquipAck) event('EquipItem', { success: true });
    } else if (command.type === 'moveItem') {
      const held = client.snapshot.beltItems.find(item => item.slot === command.from);
      if (!held) { event('MoveItem', { success: false }); return; }
      client.snapshot.beltItems = client.snapshot.beltItems.filter(item => item !== held);
      delete held.slot;
      held.container = 'Bag1'; held.slot = command.to - 6;
      client.snapshot.inventoryItems.push(held);
      event('MoveItem', { success: true });
    } else if (command.type === 'magic') {
      const spell = command.spell;
      options.onMagic?.(client, command);
      if (options.disconnectOnCast) client.closed = true;
      if (options.failureOnCast) client.failure = options.failureOnCast;
      if (options.noCastAck) return;
      const accepted = options.castRejected !== true;
      event(options.objectMagicOnly ? 'ObjectMagic' : 'Magic', { objectId: options.wrongCaster ? 2 : command.objectId,
        spell: options.wrongSpell ? 'Healing' : spell, targetId: options.wrongTarget ? 999 : command.targetId, cast: accepted });
      if (!accepted) return;
      const equipped = client.snapshot.equipmentItems.find(item => item.slot === 'Amulet');
      if (equipped && !options.noDebit) {
        equipped.quantity -= options.debitCount ?? 1;
        if (equipped.quantity === 0) client.snapshot.equipmentItems = client.snapshot.equipmentItems.filter(item => item !== equipped);
        event('DeleteItem', { uniqueId: 9, count: options.debitCount ?? 1 });
      }
      client.snapshot.playerMp -= spell === 'SummonSkeleton' ? 12 : spell === 'Poisoning' ? 2 : 3;
      if (spell === 'SummonSkeleton' && !options.noSpawn) {
        const pet = { objectId: 50, kind: 'monster', name: 'BoneFamiliar', hp: 140, dead: false, x: 20, y: 20,
          ownerName: options.foreignOwner ? 'SomeoneElse' : 'pRoOfTaO' };
        client.snapshot.entities.push(pet);
        event('ObjectMonster', { objectId: 50, name: 'BoneFamiliar', dead: false,
          ...(options.noMaster ? {} : { masterObjectId: options.foreignMaster ? 2 : 1 }) });
      } else if (spell === 'Poisoning' && !options.noEffect) {
        const enemy = client.snapshot.entities.find(entity => entity.objectId === command.targetId);
        const bits = options.poisonBits ?? 1;
        if (enemy) enemy.poison = bits;
        event('ObjectPoisoned', { objectId: options.wrongEffectTarget ? 41 : command.targetId, poison: bits });
      }
    }
  };
  client.wait = async (predicate, label, timeoutMs) => {
    timeouts.push(timeoutMs);
    if (client.failure) throw client.failure;
    if (client.closed) throw new Error(`Connection closed waiting for ${label}`);
    if (options.realWait) {
      const end = Date.now() + timeoutMs;
      while (Date.now() < end) { const value = predicate(); if (value) return value; await new Promise(resolve => setTimeout(resolve, 2)); }
    } else { const value = predicate(); if (value) return value; }
    throw new Error(`Timeout waiting for ${label}`);
  };
  event(undefined, snapshot, 'worldSnapshot');
  const delegate = async (_client, enemy) => {
    delegated.push(enemy); return { kind: 'magic', spell: 'SoulFireBall', targetId: enemy.objectId };
  };
  const action = createPeriodicTaoCombatAction({ delegate, onReceipt: receipt => receipts.push(receipt),
    timeoutMs: options.timeoutMs ?? 200, ...(options.factory ?? {}) });
  return { client, action, sent, receipts, delegated, timeouts, event, target, actor };
}
function addExistingPet(f) {
  f.client.snapshot.entities.push({ objectId: 50, kind: 'monster', name: 'BoneFamiliar', hp: 140, dead: false,
    ownerName: 'ProofTao', x: 20, y: 20 });
  f.event('ObjectMonster', { objectId: 50, name: 'BoneFamiliar', dead: false, masterObjectId: 1 });
}
function magic(f) { return f.sent.filter(command => command.type === 'magic'); }
function stripSummon(f) { f.client.snapshot.knownSkills = f.client.snapshot.knownSkills.filter(skill => skill.spell !== 'SummonSkeleton'); }
function stripPoison(f) { f.client.snapshot.knownSkills = f.client.snapshot.knownSkills.filter(skill => skill.spell !== 'Poisoning'); }

for (const kind of ['Warrior', 'Wizard', 'unlearned', 'dead', 'low HP', 'action blocked', 'no material']) {
  test(`${kind} retains the existing combat/recovery delegate without support packets`, async () => {
    const f = fixture();
    if (['Warrior', 'Wizard'].includes(kind)) f.actor.class = kind;
    if (kind === 'unlearned') { stripSummon(f); stripPoison(f); }
    if (kind === 'dead') f.actor.dead = true;
    if (kind === 'low HP') { f.actor.hp = 110; f.client.snapshot.playerHp = 110; }
    if (kind === 'action blocked') f.actor.poison = 8;
    if (kind === 'no material') { f.client.snapshot.inventoryItems = []; f.client.snapshot.equipmentItems = []; }
    assert.equal((await f.action(f.client, f.target)).spell, 'SoulFireBall');
    assert.equal(f.delegated.length, 1); assert.deepEqual(f.sent, []); assert.deepEqual(f.receipts, []);
  });
}

test('summon uses ordinary target-zero route and proves own cast, master id, owner name, and exact material debit', async () => {
  const f = fixture({ objectMagicOnly: true });
  const result = await f.action(f.client, f.target);
  assert.deepEqual(magic(f), [{ type: 'magic', objectId: 1, spell: 'SummonSkeleton', direction: 'Right',
    targetId: 0, x: 20, y: 20, spellTargetLock: false }]);
  assert.equal(result.kind, 'wait'); assert.equal(result.targetId, 40);
  assert.equal(f.receipts[0].masterObjectId, 1); assert.equal(f.receipts[0].petObjectId, 50);
  assert.equal(f.receipts[0].quantityBefore, 30); assert.equal(f.receipts[0].quantityAfter, 29);
  assert.equal(f.receipts[0].ownCreditGranted, false); assert.equal(f.delegated.length, 0);
  assert.equal(f.client.probes, 1); assert.ok(f.timeouts.every(ms => ms > 0 && ms <= 200));
});
for (const [label, options, pattern] of [
  ['missing cast ACK', { noCastAck: true }, /accepted own cast/],
  ['wrong caster', { wrongCaster: true }, /accepted own cast/],
  ['wrong spell', { wrongSpell: true }, /accepted own cast/],
  ['wrong target', { wrongTarget: true }, /accepted own cast/],
  ['server rejected cast', { castRejected: true }, /rejected by server/],
  ['no actual spawn', { noSpawn: true }, /owned BoneFamiliar spawn/],
  ['foreign master', { foreignMaster: true }, /owned BoneFamiliar spawn/],
  ['missing master', { noMaster: true }, /owned BoneFamiliar spawn/],
  ['foreign snapshot owner', { foreignOwner: true }, /owned snapshot/],
  ['no debit', { noDebit: true }, /exact Amulet debit/],
  ['wrong debit', { debitCount: 2 }, /exact Amulet debit/],
]) {
  test(`summon fails closed for ${label}`, async () => {
    const f = fixture(options);
    await assert.rejects(f.action(f.client, f.target), pattern);
    assert.equal(f.receipts.length, 0); assert.equal(f.delegated.length, 0);
  });
}
test('old own ACK and old skeleton packet do not confirm a new summon', async () => {
  const f = fixture({ noCastAck: true });
  f.event('Magic', { spell: 'SummonSkeleton', targetId: 0, cast: true });
  f.event('ObjectMonster', { objectId: 50, name: 'BoneFamiliar', dead: false, masterObjectId: 1 });
  await assert.rejects(f.action(f.client, f.target), /accepted own cast/);
});
test('an authoritative living owned pet prevents resummon and missing monster poison after refresh does not repeat a dose', async () => {
  const f = fixture({ omitMonsterPoison: true }); addExistingPet(f);
  const result = await f.action(f.client, f.target);
  assert.equal(result.spell, 'Poisoning'); assert.equal(f.receipts[0].restoredAmuletUniqueId, 101);
  assert.deepEqual(f.sent.filter(command => command.type === 'equipItem'), [
    { type: 'equipItem', grid: 'inventory', uniqueId: 102, to: 9 },
    { type: 'equipItem', grid: 'inventory', uniqueId: 101, to: 9 },
  ]);
  assert.deepEqual(magic(f), [{ type: 'magic', objectId: 1, spell: 'Poisoning', direction: 'Right',
    targetId: 40, x: 23, y: 20, spellTargetLock: true }]);
  assert.equal(f.client.snapshot.entities.find(entity => entity.objectId === 40).poison, undefined);
  assert.equal((await f.action(f.client, f.target)).spell, 'SoulFireBall');
  assert.equal(magic(f).length, 1); assert.equal(f.delegated.length, 1);
  assert.equal(f.receipts[0].ownCreditGranted, false);
});
test('public snapshot ownership corroborates packet ownership, and foreign/dead pets never qualify', () => {
  const f = fixture(); addExistingPet(f);
  assert.equal(ownedPeriodicBoneFamiliar(f.client)?.objectId, 50);
  f.event('ObjectMonster', { objectId: 50, masterObjectId: 2 });
  assert.equal(ownedPeriodicBoneFamiliar(f.client), null);
  f.event('ObjectMonster', { objectId: 50, masterObjectId: 1 });
  f.client.snapshot.entities.at(-1).ownerName = 'Other';
  assert.equal(ownedPeriodicBoneFamiliar(f.client), null);
  f.client.snapshot.entities.at(-1).ownerName = 'ProofTao'; f.client.snapshot.entities.at(-1).dead = true;
  assert.equal(ownedPeriodicBoneFamiliar(f.client), null);
});
for (const [label, options, pattern] of [
  ['missing poison ACK', { noCastAck: true }, /accepted own cast/],
  ['no target effect', { noEffect: true }, /target effect/],
  ['red effect only', { poisonBits: 2 }, /target effect/],
  ['wrong target effect', { wrongEffectTarget: true }, /target effect/],
  ['no poison debit', { noDebit: true }, /GreenPoison debit/],
  ['rejected poison cast', { castRejected: true }, /rejected by server/],
]) {
  test(`failed ${label} restores the original Amulet and preserves the failure`, async () => {
    const f = fixture(options); stripSummon(f);
    await assert.rejects(f.action(f.client, f.target), pattern);
    assert.equal(f.client.snapshot.equipmentItems.find(item => item.slot === 'Amulet')?.uniqueId, 101);
    assert.equal(f.receipts.length, 0); assert.equal(f.delegated.length, 0);
  });
}
test('Amulet restoration rejection is reported, never treated as ready for ordinary fire', async () => {
  const f = fixture({ rejectEquip: command => command.uniqueId === 101 }); stripSummon(f);
  await assert.rejects(f.action(f.client, f.target), /Equip rejected for Amulet/);
  assert.equal(f.receipts.length, 0); assert.equal(f.delegated.length, 0);
});
test('failed poison keeps its original error when bounded Amulet restoration also fails', async () => {
  const f = fixture({ noEffect: true, rejectEquip: command => command.uniqueId === 101 }); stripSummon(f);
  await assert.rejects(f.action(f.client, f.target), error => /target effect/.test(error.message) && /Equip rejected for Amulet/.test(error.restoreError));
});

test('existing belt poison moves through normal MoveItem and verified EquipItem before the cast', async () => {
  const f = fixture(); stripSummon(f);
  const green = f.client.snapshot.inventoryItems.pop(); green.slot = 4; green.container = 'Belt';
  f.client.snapshot.beltItems.push(green);
  await f.action(f.client, f.target);
  assert.deepEqual(f.sent.filter(command => command.type === 'moveItem'), [{ type: 'moveItem', grid: 'belt', from: 4, to: 6 }]);
  assert.equal(f.client.snapshot.equipmentItems[0].uniqueId, 101);
});
for (const [label, mutate] of [
  ['wrong template', item => item.tooltipSource.info.item_index = 711],
  ['wrong shape', item => item.tooltipSource.info.shape = 2],
  ['zero quantity', item => item.quantity = 0],
  ['missing exact quantity', item => delete item.quantity],
  ['broken', item => item.durabilityCurrent = 0],
  ['unknown durability', item => delete item.durabilityCurrent],
  ['expired', item => item.expired = true],
  ['expiry cannot be proved', item => item.tooltipSource.user_item = { expire_info: { expiry_binary_datetime: 1 } }],
  ['wrong class requirement', item => item.tooltipSource.info.required_class = 1],
  ['wrong level', item => item.tooltipSource.info.required_amount = 26],
]) {
  test(`invalid ${label} poison delegates without equipping or casting support`, async () => {
    const f = fixture(); stripSummon(f); mutate(f.client.snapshot.inventoryItems[0]);
    assert.equal((await f.action(f.client, f.target)).spell, 'SoulFireBall');
    assert.deepEqual(f.sent, []); assert.equal(f.receipts.length, 0);
  });
}
for (const options of [{ noEquipAck: true }, { wrongEquipId: true }, { wrongEquipSlot: true }, { rejectEquip: () => true }]) {
  test(`material equip must have a fresh successful ACK and exact slot/identity/quantity (${JSON.stringify(options)})`, async () => {
    const f = fixture(options); stripSummon(f);
    await assert.rejects(f.action(f.client, f.target), /Equip|equipped GreenPoison/);
    assert.deepEqual(magic(f), []); assert.equal(f.receipts.length, 0);
  });
}

for (const [label, mutate] of [
  ['unknown mana', f => delete f.client.snapshot.playerMp],
  ['insufficient mana', f => f.client.snapshot.playerMp = 1],
  ['unknown cost', f => delete f.client.snapshot.knownSkills.find(skill => skill.spell === 'Poisoning').mpCost],
  ['zero noncanonical cost', f => f.client.snapshot.knownSkills.find(skill => skill.spell === 'Poisoning').mpCost = 0],
  ['missing cooldown', f => { const skill = f.client.snapshot.knownSkills.find(skill => skill.spell === 'Poisoning'); delete skill.cooldownRemainingMs; delete skill.cooldownRemainingTicks; }],
  ['invalid cooldown', f => f.client.snapshot.knownSkills.find(skill => skill.spell === 'Poisoning').cooldownRemainingMs = 'unknown'],
]) {
  test(`${label} never manufactures support readiness`, async () => {
    const f = fixture(); stripSummon(f); mutate(f);
    assert.equal((await f.action(f.client, f.target)).spell, 'SoulFireBall'); assert.deepEqual(magic(f), []);
  });
}
test('fresh owner movement requires authoritative readiness; an elapsed local clock never releases a positive cooldown', async () => {
  let probes = 0;
  const f = fixture({ onRefresh: client => {
    probes++;
    for (const skill of client.snapshot.knownSkills) { skill.cooldownRemainingMs = probes === 1 ? 600 : 0; skill.cooldownRemainingTicks = probes === 1 ? 1 : 0; }
  } });
  f.event('UserLocation', { x: 20, y: 20 });
  assert.equal((await f.action(f.client, f.target)).kind, 'wait'); assert.deepEqual(magic(f), []);
  assert.equal(f.client.probes, 1);
  const next = await f.action(f.client, f.target);
  assert.equal(next.spell, 'SummonSkeleton'); assert.equal(f.client.probes, 3); assert.equal(magic(f).length, 1);
});
test('post-equipment target movement/loss prevents a stale Poisoning cast and restores Amulet', async () => {
  const f = fixture(); stripSummon(f);
  const originalSend = f.client.send;
  f.client.send = command => { originalSend(command); if (command.type === 'equipItem' && command.uniqueId === 102) f.target.x = 50; };
  await f.action(f.client, f.target);
  assert.deepEqual(magic(f), []); assert.equal(f.client.snapshot.equipmentItems[0].uniqueId, 101);
});
test('unknown actor coordinates never generate a summon with fabricated coordinates', async () => {
  const f = fixture(); delete f.actor.x;
  await f.action(f.client, f.target);
  assert.deepEqual(f.sent, []); assert.equal(f.delegated.length, 1);
});
test('a last real Amulet is confirmed consumed only by a fresh snapshot without that instance', async () => {
  const f = fixture(); f.client.snapshot.equipmentItems[0].quantity = 1;
  const result = await f.action(f.client, f.target);
  assert.equal(result.spell, 'SummonSkeleton'); assert.equal(f.receipts[0].quantityAfter, 0);
  assert.ok(!f.client.snapshot.equipmentItems.some(item => item.uniqueId === 101));
});

for (const terminal of ['clear', 'death-respawn', 'map', 'player']) {
  test(`poison lifetime resets on ${terminal} while ordinary snapshot omission retains it`, async () => {
    const f = fixture({ omitMonsterPoison: true }); stripSummon(f);
    await f.action(f.client, f.target);
    if (terminal === 'clear') f.event('ObjectPoisoned', { objectId: 40, poison: 0 });
    if (terminal === 'death-respawn') { f.event('ObjectDied', { objectId: 40 }); f.event('ObjectMonster', { objectId: 40, dead: false, name: 'Centipede' }); }
    if (terminal === 'map') f.client.snapshot.mapFileName = 'D002';
    if (terminal === 'player') f.client.snapshot.entities[0].name = 'NewProofTao';
    await f.action(f.client, f.target);
    assert.deepEqual(magic(f).map(command => command.spell), ['Poisoning', 'Poisoning']);
  });
}
test('pet damage remains independent evidence and cannot forge quest or experience credit', async () => {
  const f = fixture(); addExistingPet(f); stripPoison(f);
  const quests = [{ questId: 92011, current: 0 }]; f.client.snapshot.questLog = quests;
  f.event('ObjectStruck', { objectId: 40, attackerId: 50 }); f.event('DamageIndicator', { objectId: 40, damage: 23 });
  await f.action(f.client, f.target);
  assert.deepEqual(f.client.snapshot.questLog, quests); assert.equal(f.client.snapshot.playerExperience, undefined);
  assert.equal(f.delegated.length, 1); assert.equal(f.receipts.length, 0);
});
test('bounded real timeout never confirms an unacknowledged summon or retries it', async () => {
  const f = fixture({ noCastAck: true, realWait: true, timeoutMs: 30 });
  const start = Date.now(); await assert.rejects(f.action(f.client, f.target), /Timeout|deadline/);
  assert.ok(Date.now() - start < 500); assert.equal(magic(f).length, 1); assert.equal(f.receipts.length, 0);
});
for (const options of [{ disconnectOnCast: true }, { failureOnCast: new Error('Gateway rejected ordinary magic') }]) {
  test(`connection failure propagates without further support or restoration packets (${options.disconnectOnCast ? 'closed' : 'gateway error'})`, async () => {
    const f = fixture(options); stripSummon(f);
    await assert.rejects(f.action(f.client, f.target), /Connection closed|Gateway rejected/);
    assert.equal(magic(f).length, 1); assert.equal(f.receipts.length, 0);
    assert.equal(f.sent.at(-1).type, 'magic');
  });
}
test('a map change during a cast cannot borrow spawn or inventory evidence from another map', async () => {
  const f = fixture({ onMagic: client => client.snapshot.mapFileName = 'D002' });
  await assert.rejects(f.action(f.client, f.target), /player\/map changed/);
  assert.equal(f.receipts.length, 0);
});
test('the inherited journey deadline is checked inside every support wait', async () => {
  let expired = false;
  const f = fixture({ onMagic: () => { expired = true; }, factory: { checkDeadline: () => { if (expired) throw new Error('original periodic deadline'); } } });
  await assert.rejects(f.action(f.client, f.target), /original periodic deadline/); assert.equal(f.receipts.length, 0);
});
test('normal default combat resumes with actual SoulFireBall after owned pet and existing poison', async () => {
  const f = fixture(); addExistingPet(f); f.target.poison = 1;
  const action = createPeriodicTaoCombatAction();
  const result = await action(f.client, f.target);
  assert.equal(result.kind, 'magic'); assert.equal(result.spell, 'SoulFireBall');
  assert.equal(magic(f)[0].spell, 'SoulFireBall'); assert.equal(magic(f)[0].targetId, 40);
});
