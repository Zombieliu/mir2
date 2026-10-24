import test from 'node:test';
import assert from 'node:assert/strict';
import { FULL_LIFECYCLE_CHECKS, validateLifecycleCommand, sharedKillEvidence, sharedDropEvidence,
  uniquePickupEvidence, pickupReplayEvidence, playerDeathEvidence, townReviveEvidence,
  runLifecycleChecks, isLifecycleLootTarget, prepareOrdinaryExposure } from './playtest-lifecycle.mjs';

const event = (packet, payload, sequence) => ({ direction: 'received', packet, payload, sequence });
const drop = () => ({ objectId: 2000, name: 'Gold', sourceMonster: 'Scarecrow',
  x: 10, y: 11, quantity: 7, loot: { kind: 'gold', amount: 7 } });
const combat = () => ({ targetId: 99, target: 'Scarecrow',
  lifecycleStart: { cursors: [10, 10], baselineDropIds: [], mapFileName: '0' },
  hits: [{ targetId: 99, attackerName: 'ActorA', receipts: [{ struck: 12, damage: 13 }, { struck: 12 }] },
    { targetId: 99, attackerName: 'ActorB', receipts: [{ struck: 16 }, { struck: 16, damage: 17 }] }] });
function scene() {
  return [0, 1].map(index => ({
    sequence: 21,
    events: [event('ObjectDied', { objectId: 99, location: { x: 10, y: 10 } }, 20),
      event('ObjectGold', { objectId: 2000, gold: 7, location: { x: 10, y: 11 } }, 21)],
    snapshot: { mapFileName: '0', playerObjectId: 1000, playerHp: 50, playerMaxHp: 50, gold: 100,
      inventoryItems: [], equipmentItems: [], groundDrops: [drop()],
      entities: [{ objectId: 1000, name: index ? 'ActorB' : 'ActorA', kind: 'selfPlayer',
        x: 10 + index, y: 11, hp: 50, dead: false },
      { objectId: index ? 50000 : 50001, name: index ? 'ActorA' : 'ActorB', kind: 'player',
        x: 11 - index, y: 11, hp: 50, dead: false },
      { objectId: 99, kind: 'monster', name: 'Scarecrow', x: 10, y: 10, hp: 0, dead: true }] },
  }));
}
const snapshots = clients => clients.map(client => structuredClone(client.snapshot));
const received = (client, packet, payload) => client.events.push(event(packet, payload, ++client.sequence));
const fresh = client => client.events.push({ direction: 'received', type: 'worldSnapshot',
  payload: structuredClone(client.snapshot), sequence: ++client.sequence });
const request = (client, command) => client.events.push({ direction: 'sent', ...command, sequence: ++client.sequence });
function picked(clients, gold = 7, withRequests = true) {
  if (withRequests) for (const client of clients) request(client, { type: 'pickUp', objectId: 2000 });
  clients[0].snapshot.gold += gold;
  received(clients[0], 'GainedGold', { gold });
  for (const client of clients) { received(client, 'ObjectRemove', { objectId: 2000 }); client.snapshot.groundDrops = []; }
}

test('lifecycle wire shape accepts only ordinary pickup u32 and empty town revival', () => {
  validateLifecycleCommand({ type: 'pickUp', objectId: 1 });
  validateLifecycleCommand({ type: 'pickUp', objectId: 0xffffffff });
  validateLifecycleCommand({ type: 'townRevive' });
  validateLifecycleCommand({ type: 'removeItem', grid: 'inventory', uniqueId: 1, to: 0 });
  for (const objectId of [0, -1, 1.2, 0x100000000, '2000', NaN, null]) {
    assert.throws(() => validateLifecycleCommand({ type: 'pickUp', objectId }));
  }
  for (const command of [{ type: 'pickUp', objectId: 2000, accountId: 'other' },
    { type: 'townRevive', map: '0' }, { type: 'townRevive', x: 1 }, { type: 'qaControl' },
    { type: 'transferMap', key: 'crystal:0:1:1' }, { type: 'pickUpTile' },
    { type: 'removeItem', grid: 'heroInventory', uniqueId: 1, to: 0 },
    { type: 'removeItem', grid: 'inventory', uniqueId: Number.MAX_SAFE_INTEGER + 1, to: 0 },
    { type: 'removeItem', grid: 'inventory', uniqueId: '1', to: 0 },
    { type: 'removeItem', grid: 'inventory', uniqueId: 1, to: 40 },
    { type: 'removeItem', grid: 'inventory', uniqueId: 1, to: -1 },
    { type: 'removeItem', grid: 'inventory', uniqueId: 1, to: 0, force: true }]) {
    assert.throws(() => validateLifecycleCommand(command), /Invalid ordinary lifecycle command/);
  }
});

test('ordinary exposure unequips the exact live armour into a real empty slot and requires receipt plus fresh state', async () => {
  const client = scene()[0];
  client.snapshot.equipmentItems = [{ slot: 'armour', uniqueId: 1, name: 'BaseDress(M)' }];
  client.snapshot.inventoryItems = [{ slot: '2', uniqueId: 2, count: 1 }];
  client.refresh = async () => client.snapshot;
  const commands = [];
  client.request = async command => {
    commands.push(command);
    client.snapshot.equipmentItems = [];
    client.snapshot.inventoryItems.push({ slot: String(command.to), uniqueId: 1, count: 1 });
    return { sequence: 50, payload: { success: true, uniqueId: 1, to: command.to } };
  };
  assert.equal((await prepareOrdinaryExposure(client, Date.now() + 5000)).armourRemoved, true);
  assert.deepEqual(commands, [{ type: 'removeItem', grid: 'inventory', uniqueId: 1, to: 0 }]);
  client.snapshot.equipmentItems = [{ slot: 'armour', uniqueId: 1 }];
  client.request = async () => ({ sequence: 51, payload: { success: false, uniqueId: 1, to: 0 } });
  await assert.rejects(prepareOrdinaryExposure(client, Date.now() + 5000), /exact successful RemoveItem receipt/);
  client.request = async command => ({ sequence: 52, payload: { success: true, uniqueId: 1, to: command.to } });
  await assert.rejects(prepareOrdinaryExposure(client, Date.now() + 5000), /Fresh snapshot did not confirm/);
});

test('pickup lifecycle targets ordinary non-harvest monsters without treating animal corpses or guards as loot', () => {
  for (const name of ['Scarecrow', 'HookingCat', 'RakingCat']) {
    assert.equal(isLifecycleLootTarget({ kind: 'monster', name, hp: 10, dead: false }), true);
  }
  for (const name of ['Deer', 'Hen', 'Guard', 'Royal_Guard']) {
    assert.equal(isLifecycleLootTarget({ kind: 'monster', name, hp: 10 }), false);
  }
  assert.equal(isLifecycleLootTarget({ kind: 'monster', name: 'Scarecrow', hp: 0 }), false);
  assert.equal(isLifecycleLootTarget({ kind: 'monster', name: 'Scarecrow', hp: 10, dead: true }), false);
});

test('shared kill requires both attackers and fresh matching same-life death receipts', () => {
  const clients = scene(), proof = combat();
  assert.equal(sharedKillEvidence(clients, proof).targetId, 99);
  for (const mutation of [
    (c, p) => { p.hits[1].attackerName = 'ActorA'; },
    (c, p) => { p.hits[1].targetId = 98; },
    (c, p) => { p.lifecycleStart.cursors = [20, 20]; },
    c => { c[1].events[0].payload.objectId = 98; },
    c => { c[1].events[0].payload.location.x = 20; },
    c => { c[0].events.push(event('ObjectRevived', { objectId: 99 }, 18)); },
    (c, p) => { p.hits[1].receipts[1].struck = 22; },
  ]) {
    const c = structuredClone(clients), p = structuredClone(proof); mutation(c, p);
    assert.equal(sharedKillEvidence(c, p), null);
  }
});

test('a shared drop needs fresh matching public spawns and both current source-name snapshots', () => {
  const clients = scene(), proof = combat(), kill = sharedKillEvidence(clients, proof);
  assert.equal(sharedDropEvidence(clients, proof, kill).drop.objectId, 2000);
  assert.match(sharedDropEvidence(clients, proof, kill).sourceScope, /do not expose a unique/);
  for (const mutation of [
    (c, p) => { p.lifecycleStart.baselineDropIds = [2000]; },
    c => { c[1].snapshot.mapFileName = '1'; },
    c => { c[1].snapshot.groundDrops[0].loot.amount = 8; },
    c => { c[0].snapshot.groundDrops[0].sourceMonster = 'OtherMonster'; },
    c => { c[1].events[1].sequence = 19; },
    c => { c[1].events[1].payload.objectId = 999; },
    c => { c[1].snapshot.groundDrops = []; },
  ]) {
    const c = structuredClone(clients), p = structuredClone(proof); mutation(c, p);
    assert.equal(sharedDropEvidence(c, p, kill), null);
  }
});

test('contested gold pickup requires one award, both removals and exact two-player conservation', () => {
  const clients = scene(), before = snapshots(clients), cursors = clients.map(client => client.sequence);
  picked(clients);
  assert.equal(uniquePickupEvidence(clients, cursors, before, drop()).winner, 0);
  for (const mutation of [
    c => { received(c[1], 'GainedGold', { gold: 7 }); c[1].snapshot.gold += 7; },
    c => { c[0].snapshot.gold += 1; },
    c => { c[1].snapshot.gold += 1; },
    c => { c[1].snapshot.groundDrops = [drop()]; },
    c => { c[1].events = c[1].events.filter(value => value.packet !== 'ObjectRemove'); },
    c => { c[0].events = c[0].events.filter(value => value.packet !== 'GainedGold'); },
    c => { c[0].snapshot.inventoryItems.push({ uniqueId: 7, itemIndex: 1, count: 1 }); },
    c => { c[0].snapshot.mapFileName = '1'; },
    c => { c[1].events = c[1].events.filter(value => value.direction !== 'sent'); },
    c => { c[0].events.find(value => value.direction === 'sent').sequence = 999; },
  ]) {
    const c = structuredClone(clients); mutation(c);
    assert.equal(uniquePickupEvidence(c, cursors, before, drop()), null);
  }
  assert.equal(uniquePickupEvidence(clients, clients.map(client => client.sequence), before, drop()), null);
});

test('item pickup preserves one safe exact UID and rejects merges, invented identity and unrelated inventory changes', () => {
  const clients = scene(), before = snapshots(clients), cursors = [21, 21];
  const itemDrop = { ...drop(), name: 'Sword', quantity: 1, loot: { kind: 'inventoryItem',
    exactItem: { uidAssigned: true, item: { unique_id: '18446744073709551615', item_index: 5, count: 1 } } } };
  for (const client of clients) request(client, { type: 'pickUp', objectId: 2000 });
  for (const client of clients) { client.snapshot.groundDrops = []; received(client, 'ObjectRemove', { objectId: 2000 }); }
  received(clients[0], 'GainedItem', { item: structuredClone(itemDrop.loot.exactItem.item) });
  clients[0].snapshot.inventoryItems.push({ uniqueId: '18446744073709551615', itemIndex: 5, count: 1 });
  assert.equal(uniquePickupEvidence(clients, cursors, before, itemDrop).gainPacket, 'GainedItem');
  for (const mutation of [
    c => { c[0].snapshot.inventoryItems[0].uniqueId = '7'; },
    c => { c[0].events.find(value => value.packet === 'GainedItem').payload.item.unique_id = Number.MAX_SAFE_INTEGER + 1; },
    c => { c[0].snapshot.inventoryItems.push({ uniqueId: 8, itemIndex: 6, count: 1 }); },
    c => { c[0].snapshot.inventoryItems[0].count = 2; },
  ]) {
    const c = structuredClone(clients); mutation(c);
    assert.equal(uniquePickupEvidence(c, cursors, before, itemDrop), null);
  }
});

test('pickup replay cannot pass without fresh snapshots or after any repeated gain', () => {
  const clients = scene(); picked(clients);
  const before = snapshots(clients), cursors = clients.map(client => client.sequence);
  assert.equal(pickupReplayEvidence(clients, cursors, before, drop()), null);
  for (const client of clients) request(client, { type: 'pickUp', objectId: 2000 });
  for (const client of clients) fresh(client);
  assert.ok(pickupReplayEvidence(clients, cursors, before, drop()));
  received(clients[1], 'GainedItem', { item: { unique_id: 7 } });
  assert.equal(pickupReplayEvidence(clients, cursors, before, drop()), null);
});

function deadScene() {
  const clients = scene(), context = { initialAlive: true, names: ['ActorA', 'ActorB'],
    observerId: 50000, mapFileName: '0', cursors: [21, 21] };
  clients[0].snapshot.playerHp = 0; clients[0].snapshot.entities[0].dead = true;
  received(clients[0], 'Death', { location: { x: 10, y: 11 } });
  received(clients[1], 'ObjectDied', { objectId: 50000, location: { x: 10, y: 11 } });
  return { clients, context };
}

test('player death needs exact owner death and the same observer actor, not rounded zero health', () => {
  const { clients, context } = deadScene();
  assert.equal(playerDeathEvidence(clients, context).actor, 'ActorA');
  for (const mutation of [
    c => { c[0].events = []; },
    c => { c[1].events = []; },
    c => { c[1].events.at(-1).payload.objectId = 1000; },
    c => { c[0].snapshot.entities[0].dead = false; c[0].snapshot.playerHp = 1; c[0].snapshot.playerHealthPercent = 0; },
    c => { c[0].snapshot.entities[0].name = 'DifferentOwner'; },
    c => { c[0].snapshot.mapFileName = '1'; },
    c => { received(c[0], 'Death', { location: { x: 10, y: 11 } }); },
  ]) {
    const c = structuredClone(clients); mutation(c);
    assert.equal(playerDeathEvidence(c, context), null);
  }
});

test('town revival needs a new alive snapshot plus source-map observer revival or departure', () => {
  const { clients, context } = deadScene(), cursors = clients.map(client => client.sequence);
  request(clients[0], { type: 'townRevive' });
  received(clients[0], 'Revived', {});
  clients[0].snapshot.entities[0].dead = false; clients[0].snapshot.playerHp = 50;
  received(clients[1], 'ObjectRevived', { objectId: 50000 });
  assert.equal(townReviveEvidence(clients, context, cursors), null, 'old snapshot is not an alive barrier');
  fresh(clients[0]);
  assert.equal(townReviveEvidence(clients, context, cursors).toMap, '0');
  clients[0].snapshot.mapFileName = '1';
  assert.equal(townReviveEvidence(clients, context, cursors), null);
  received(clients[1], 'ObjectRemove', { objectId: 50000 });
  assert.equal(townReviveEvidence(clients, context, cursors), null, 'map transition packet still required');
  received(clients[0], 'MapInformation', { fileName: '1' }); fresh(clients[0]);
  assert.equal(townReviveEvidence(clients, context, cursors).observerPacket, 'ObjectRemove');
  clients[0].snapshot.playerHp = 0;
  assert.equal(townReviveEvidence(clients, context, cursors), null);
});

function runnableScene({ exposeDeath = false } = {}) {
  const clients = scene(), sent = [];
  let available = true;
  for (const [index, client] of clients.entries()) {
    if (exposeDeath) client.snapshot.entities.push({ objectId: 101, kind: 'monster',
      name: 'HookingCat', x: 10, y: 10, hp: 100, dead: false });
    client.failure = null; client.closed = false;
    client.refresh = async () => { fresh(client); return client.snapshot; };
    client.record = () => {};
    client.send = command => {
      if (command.type !== 'attack') validateLifecycleCommand(command);
      else assert.equal(command.objectId, 101);
      sent.push({ index, ...command });
      request(client, command);
      if (command.type === 'pickUp' && available) { available = false; picked(clients, 7, false); }
    };
    client.wait = async (predicate, label) => {
      if (label.includes('owner Death') && exposeDeath) {
        clients[0].snapshot.entities[0].dead = true; clients[0].snapshot.playerHp = 0;
        received(clients[0], 'Death', { location: { x: 10, y: 11 } });
        received(clients[1], 'ObjectDied', { objectId: 50000, location: { x: 10, y: 11 } });
      }
      const value = predicate();
      if (!value) throw new Error('Fixture has no required packet: ' + label);
      return value;
    };
    client.request = async (command, packet) => {
      validateLifecycleCommand(command); sent.push({ index, ...command });
      request(client, command);
      assert.equal(command.type, 'townRevive'); assert.equal(packet, 'Revived');
      clients[0].snapshot.entities[0].dead = false; clients[0].snapshot.playerHp = 50;
      received(clients[0], 'Revived', {}); received(clients[1], 'ObjectRevived', { objectId: 50000 });
      return clients[0].events.at(-1);
    };
  }
  return { clients, sent };
}

test('runner records real-shaped lifecycle receipts and never sends revival without witnessed death', async () => {
  const { clients, sent } = runnableScene();
  const results = await runLifecycleChecks({ clients, combat: combat(), loadMap: async () => ({}),
    deadline: Date.now() + 5000 });
  assert.deepEqual(FULL_LIFECYCLE_CHECKS.map(key => results[key].status),
    ['passed', 'passed', 'passed', 'not_run', 'not_run']);
  assert.equal(sent.filter(value => value.type === 'pickUp').length, 4, 'two contenders plus both replays');
  assert.equal(sent.filter(value => value.type === 'townRevive').length, 0);
});

test('runner full packet fixture joins pickup, owner death and normal town revival without privileged actions', async () => {
  const { clients, sent } = runnableScene({ exposeDeath: true }), recorded = [];
  const results = await runLifecycleChecks({ clients, combat: combat(), loadMap: async () => ({}),
    deadline: Date.now() + 5000, onCheck: (key, value) => recorded.push([key, value.status]) });
  assert.deepEqual(FULL_LIFECYCLE_CHECKS.map(key => results[key].status), FULL_LIFECYCLE_CHECKS.map(() => 'passed'));
  assert.deepEqual(recorded.map(([key]) => key), FULL_LIFECYCLE_CHECKS);
  assert.equal(sent.filter(value => value.type === 'townRevive').length, 1);
  assert.ok(sent.every(command => ['pickUp', 'townRevive', 'attack'].includes(command.type)));
});

test('missing combat prerequisites produce explicit not_run and no action or pass', async () => {
  const results = await runLifecycleChecks({ clients: [], combat: null, loadMap: async () => { throw new Error('unexpected navigation'); },
    deadline: Date.now() + 1000 });
  assert.deepEqual(Object.keys(results), FULL_LIFECYCLE_CHECKS);
  assert.ok(Object.values(results).every(result => result.status === 'not_run'));
});

test('bounded ordinary loot recovery retains the failed attempt and a failed next-target search', async () => {
  const { clients, sent } = runnableScene(), attempts = [];
  for (const client of clients) client.snapshot.groundDrops = [];
  let searches = 0;
  const results = await runLifecycleChecks({ clients, combat: combat(), loadMap: async () => ({}),
    deadline: Date.now() + 5000, onAttempt: value => attempts.push(value),
    nextCombat: async () => { searches++; throw new Error('No remaining ordinary non-harvest target'); } });
  assert.equal(searches, 1);
  assert.equal(attempts[0].sharedKill.status, 'passed');
  assert.equal(attempts[0].sharedDrop.status, 'failed');
  assert.equal(attempts[1].search.status, 'failed');
  assert.equal(results.uniquePickup.status, 'not_run');
  assert.equal(sent.length, 0);
});
