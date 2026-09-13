import assert from 'node:assert/strict';
import test from 'node:test';

import { createWizardKitingAction } from './protocol-kiting.mjs';

function openMap(width = 15, height = 15, blockedPoints = []) {
  const blocked = new Uint8Array(width * height);
  for (const point of blockedPoints) blocked[point.y * width + point.x] = 1;
  return { mapFileName: 'test', sourcePath: 'test.map', width, height, blocked };
}

function player(overrides = {}) {
  return { objectId: 1, kind: 'selfPlayer', class: 'Wizard', x: 5, y: 5, hp: 40, dead: false, ...overrides };
}

function monster(objectId, x, y, overrides = {}) {
  return { objectId, kind: 'monster', disposition: 'hostile', name: 'TigerSnake', x, y, hp: 20, dead: false, ...overrides };
}

function clientFixture(entities, overrides = {}) {
  return {
    snapshot: {
      mapFileName: 'test',
      playerObjectId: 1,
      playerHp: 40,
      playerMp: 30,
      knownSkills: [{ spell: 'FireBall', mpCost: 4, cooldownRemainingTicks: 0 }],
      entities,
      ...overrides,
    },
  };
}

function directNavigator(calls) {
  return async (destination, desiredDistance, _stopWhen, options) => {
    calls.push({ destination: { ...destination }, desiredDistance, options: { ...options } });
    const owner = calls.client.snapshot.entities.find(entity => entity.objectId === 1);
    const successfulSteps = Math.max(
      Math.abs(Number(owner.x) - Number(destination.x)),
      Math.abs(Number(owner.y) - Number(destination.y)),
    );
    Object.assign(owner, destination);
    return { reached: true, successfulSteps };
  };
}

test('Wizard retreats on a collision-planned path before using the ranged action', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const secondThreat = monster(21, 5, 6);
  const client = clientFixture([owner, target, secondThreat]);
  const navigationCalls = [];
  navigationCalls.client = client;
  const actionCalls = [];
  const wrapped = createWizardKitingAction(
    async (_client, refreshedTarget) => {
      actionCalls.push(refreshedTarget);
      return { kind: 'magic', targetId: refreshedTarget.objectId };
    },
    directNavigator(navigationCalls),
    { loadCollisionMap: async () => openMap() },
  );

  const result = await wrapped(client, target);
  assert.equal(result.kind, 'magic');
  assert.equal(navigationCalls.length, 1);
  assert.equal(navigationCalls[0].desiredDistance, 0);
  assert.deepEqual(navigationCalls[0].options, { maxSuccessfulSteps: 3, maxAttempts: 8 });
  assert.ok(Math.max(Math.abs(owner.x - 5), Math.abs(owner.y - 5)) <= 3);
  assert.ok(Math.max(Math.abs(owner.x - target.x), Math.abs(owner.y - target.y)) <= 9);
  assert.ok(Math.min(
    Math.max(Math.abs(owner.x - target.x), Math.abs(owner.y - target.y)),
    Math.max(Math.abs(owner.x - secondThreat.x), Math.abs(owner.y - secondThreat.y)),
  ) > 1);
  assert.equal(actionCalls[0], target);
});

test('Taoist SoulFireBall uses the same bounded ranged retreat with an equipped Amulet', async () => {
  const owner = player({ class: 'Taoist' });
  const target = monster(30, 7, 5, { name: 'Zombie2' });
  const client = clientFixture([owner, target], {
    knownSkills: [{ spell: 'SoulFireBall', mpCost: 5, cooldownRemainingTicks: 0 }],
    equipmentItems: [{
      name: 'Amulet',
      slot: 'amulet',
      quantity: 80,
      tooltipSource: { info: { item_index: 712 } },
    }],
  });
  const navigationCalls = [];
  navigationCalls.client = client;
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async (_client, refreshedTarget) => {
      actions += 1;
      return { kind: 'magic', targetId: refreshedTarget.objectId };
    },
    directNavigator(navigationCalls),
    { loadCollisionMap: async () => openMap() },
  );

  const result = await wrapped(client, target);
  assert.deepEqual(result, { kind: 'magic', targetId: 30 });
  assert.equal(actions, 1);
  assert.equal(navigationCalls.length, 1);
  assert.ok(Math.max(Math.abs(owner.x - 5), Math.abs(owner.y - 5)) <= 3);
});

test('authoritative partial retreat progress continues the bounded ranged fight', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let acted = 0;
  const wrapped = createWizardKitingAction(
    async () => { acted += 1; return { kind: 'magic', targetId: target.objectId }; },
    async destination => {
      Object.assign(owner, { x: destination.x, y: owner.y });
      return { reached: false, successfulSteps: Math.abs(destination.x - 5) || 1 };
    },
    { loadCollisionMap: async () => openMap() },
  );

  const result = await wrapped(client, target);
  assert.deepEqual(result, { kind: 'magic', targetId: target.objectId });
  assert.equal(acted, 1);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'wizardKiteFallback' && entry.reason === 'partialRetreatProgress'));
});

test('authoritative displacement wins when the navigator reports stale extra steps', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let acted = 0;
  const wrapped = createWizardKitingAction(
    async () => { acted += 1; return { kind: 'magic', targetId: target.objectId }; },
    async destination => {
      Object.assign(owner, destination);
      return { reached: true, successfulSteps: 4 };
    },
    { loadCollisionMap: async () => openMap() },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'magic', targetId: target.objectId });
  assert.equal(acted, 1);
  assert.ok(Math.max(Math.abs(owner.x - 5), Math.abs(owner.y - 5)) <= 3);
  assert.equal(diagnostics.some(entry => entry.reason === 'retreatDisplacementOutOfBounds'), false);
});

test('journey mode attacks when an authoritative retreat makes no progress', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let acted = 0;
  const wrapped = createWizardKitingAction(
    async () => { acted += 1; return { kind: 'magic', targetId: target.objectId }; },
    async () => ({ reached: false, successfulSteps: 0 }),
    { loadCollisionMap: async () => openMap(), fightWhenBlocked: true },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'magic', targetId: target.objectId });
  assert.equal(acted, 1);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'wizardKiteFallback' && entry.reason === 'retreatNavigationStalled'));
});

test('journey mode continues after the navigator spends its exact retreat step budget', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let acted = 0;
  const wrapped = createWizardKitingAction(
    async () => { acted += 1; return { kind: 'magic', targetId: target.objectId }; },
    async () => {
      Object.assign(owner, { x: 2, y: 5 });
      throw new Error('Navigation successful step budget exceeded (3)');
    },
    { loadCollisionMap: async () => openMap(), fightWhenBlocked: true },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'magic', targetId: target.objectId });
  assert.equal(acted, 1);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'wizardKiteFallback' && entry.reason === 'partialRetreatProgress'));
});

test('journey mode attacks when a moving threat closes the planned retreat path', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let acted = 0;
  const wrapped = createWizardKitingAction(
    async (_client, refreshedTarget) => {
      acted += 1;
      return { kind: 'magic', targetId: refreshedTarget.objectId };
    },
    async () => { throw new Error('No walk path on test from 5,5 to 4,6'); },
    { loadCollisionMap: async () => openMap(), fightWhenBlocked: true },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'magic', targetId: target.objectId });
  assert.equal(acted, 1);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'wizardKiteFallback' && entry.reason === 'retreatNavigationNoPath'));
});

test('Wizard without affordable ranged magic and passive or NPC entities never retreat', async () => {
  const owner = player();
  const passive = monster(20, 6, 5, { disposition: 'friendly' });
  const npc = { objectId: 21, kind: 'npc', disposition: 'hostile', x: 5, y: 6, hp: 10, dead: false };
  const client = clientFixture([owner, passive, npc], {
    playerMp: 0,
    knownSkills: [{ spell: 'FireBall', mpCost: 4, cooldownRemainingTicks: 0 }],
  });
  let navigated = false;
  let acted = false;
  const wrapped = createWizardKitingAction(
    async () => { acted = true; return { kind: 'attack' }; },
    async () => { navigated = true; },
    { loadCollisionMap: async () => openMap() },
  );

  assert.deepEqual(await wrapped(client, passive), { kind: 'attack' });
  assert.equal(acted, true);
  assert.equal(navigated, false);
});

test('blocked Wizard retreat fails closed without calling navigation or combat', async () => {
  const owner = player({ x: 2, y: 2 });
  const target = monster(20, 3, 2);
  const client = clientFixture([owner, target]);
  const blocked = [];
  for (let y = 0; y < 5; y += 1) {
    for (let x = 0; x < 5; x += 1) {
      if (x !== 2 || y !== 2) blocked.push({ x, y });
    }
  }
  let navigated = false;
  let acted = false;
  const wrapped = createWizardKitingAction(
    async () => { acted = true; },
    async () => { navigated = true; },
    { loadCollisionMap: async () => openMap(5, 5, blocked), maxExpanded: 25 },
  );

  await assert.rejects(wrapped(client, target), /No collision-safe Wizard retreat/);
  assert.equal(navigated, false);
  assert.equal(acted, false);
});

test('journey mode fights the selected threat when every collision-safe retreat is blocked', async () => {
  const owner = player({ x: 2, y: 2 });
  const target = monster(20, 3, 2);
  const client = clientFixture([owner, target]);
  const blocked = [];
  for (let y = 0; y < 5; y += 1) {
    for (let x = 0; x < 5; x += 1) {
      if (x !== 2 || y !== 2) blocked.push({ x, y });
    }
  }
  let acted = 0;
  const wrapped = createWizardKitingAction(
    async () => { acted += 1; return { kind: 'magic', targetId: target.objectId }; },
    async () => { throw new Error('blocked fallback must not navigate'); },
    { loadCollisionMap: async () => openMap(5, 5, blocked), maxExpanded: 25, fightWhenBlocked: true },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'magic', targetId: target.objectId });
  assert.equal(acted, 1);
});

test('per-target retreat cell budget stops repeated kiting without another move', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const navigationCalls = [];
  navigationCalls.client = client;
  const wrapped = createWizardKitingAction(
    async () => ({ kind: 'magic' }),
    directNavigator(navigationCalls),
    {
      loadCollisionMap: async () => openMap(),
      maxRetreatSteps: 1,
      maxRetreatCellsPerTarget: 1,
    },
  );

  await wrapped(client, target);
  Object.assign(owner, { x: 5, y: 5 });
  await assert.rejects(wrapped(client, target), /Wizard retreat cell budget exceeded/);
  assert.equal(navigationCalls.length, 1);
});

test('retreat budget is scoped to the consecutive target engagement', async () => {
  const owner = player();
  const targetA = monster(20, 7, 5);
  const targetB = monster(21, 5, 7);
  const client = clientFixture([owner, targetA, targetB]);
  const navigationCalls = [];
  navigationCalls.client = client;
  const wrapped = createWizardKitingAction(
    async () => ({ kind: 'magic' }),
    directNavigator(navigationCalls),
    {
      loadCollisionMap: async () => openMap(),
      maxRetreatSteps: 1,
      maxRetreatCellsPerTarget: 1,
    },
  );

  await wrapped(client, targetA);
  Object.assign(owner, { x: 5, y: 5 });
  await wrapped(client, targetB);
  Object.assign(owner, { x: 5, y: 5 });
  await wrapped(client, targetA);

  assert.equal(navigationCalls.length, 3);
});

test('map change during retreat fails closed without issuing the combat action', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  let acted = false;
  const wrapped = createWizardKitingAction(
    async () => { acted = true; },
    async destination => {
      Object.assign(owner, destination);
      client.snapshot.mapFileName = 'other-map';
      return { reached: true, successfulSteps: 1 };
    },
    { loadCollisionMap: async () => openMap(), maxRetreatSteps: 1 },
  );

  await assert.rejects(wrapped(client, target), /Wizard retreat changed map/);
  assert.equal(acted, false);
});

test('player death during retreat fails closed without issuing the combat action', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  let acted = false;
  const wrapped = createWizardKitingAction(
    async () => { acted = true; },
    async destination => {
      Object.assign(owner, destination, { hp: 0, dead: true });
      client.snapshot.playerHp = 0;
      return { reached: true, successfulSteps: 1 };
    },
    { loadCollisionMap: async () => openMap(), maxRetreatSteps: 1 },
  );

  await assert.rejects(wrapped(client, target), /Player died during Wizard retreat/);
  assert.equal(acted, false);
});
