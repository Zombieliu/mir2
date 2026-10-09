import assert from 'node:assert/strict';
import test from 'node:test';

import { createWizardKitingAction, RangedSafetyBandUnavailable } from './protocol-kiting.mjs';
import { criticalProvenAggressorOffenseGuardActive } from './protocol-combat.mjs';
import { loadProtocolCollisionMap, planProtocolNavigation } from './protocol-navigation.mjs';
import { createNavigator, NavigationStepGuarded } from './protocol-play.mjs';

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

test('q89 Wizard moves a close CursedShaman into the 7-9 projectile band outside every visible Shaman footprint', async () => {
  const owner = player({ x: 5, y: 5 });
  const target = monster(20, 5, 11, { name: 'CursedShaman' });
  const secondShaman = monster(21, 11, 5, { name: 'CursedShaman0' });
  const client = clientFixture([owner, target, secondShaman]);
  const navigationCalls = [];
  navigationCalls.client = client;
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async (_client, refreshedTarget) => {
      actions += 1;
      return { kind: 'magic', targetId: refreshedTarget.objectId };
    },
    directNavigator(navigationCalls),
    {
      loadCollisionMap: async () => openMap(20, 20),
      fightWhenBlocked: true,
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7,
        maximumTargetDistance: 9,
        unsafeShamanDistance: 6,
        maxRetreatSteps: 6,
      },
    },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'magic', targetId: 20 });
  assert.equal(actions, 1);
  assert.equal(navigationCalls.length, 1);
  assert.ok(navigationCalls[0].options.maxSuccessfulSteps <= 6);
  assert.ok(Math.max(Math.abs(owner.x - target.x), Math.abs(owner.y - target.y)) >= 7);
  assert.ok(Math.max(Math.abs(owner.x - target.x), Math.abs(owner.y - target.y)) <= 9);
  for (const shaman of [target, secondShaman]) {
    assert.ok(Math.max(Math.abs(owner.x - shaman.x), Math.abs(owner.y - shaman.y)) > 6);
  }
});

test('q89 Wizard never falls back to a close CursedShaman cast when no collision-safe ranged band exists', async () => {
  const owner = player({ x: 2, y: 2 });
  const target = monster(20, 3, 2, { name: 'CursedShaman' });
  const client = clientFixture([owner, target]);
  const blocked = [];
  for (let y = 0; y < 5; y += 1) {
    for (let x = 0; x < 5; x += 1) {
      if (x !== 2 || y !== 2) blocked.push({ x, y });
    }
  }
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async () => { actions += 1; return { kind: 'magic', targetId: target.objectId }; },
    async () => { throw new Error('ranged-band fallback must not navigate'); },
    {
      loadCollisionMap: async () => openMap(5, 5, blocked),
      maxExpanded: 25,
      fightWhenBlocked: true,
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman'],
        minimumTargetDistance: 7,
        maximumTargetDistance: 9,
        unsafeShamanDistance: 6,
      },
    },
  );

  await assert.rejects(wrapped(client, target), /no collision-safe Wizard ranged band/i);
  assert.equal(actions, 0);
});

test('q89 action-owned reposition rechecks a Shaman that moves during navigator cadence', async () => {
  const owner = player({ x: 5, y: 5 });
  const target = monster(20, 5, 11, { name: 'CursedShaman', hp: 205, maxHp: 205 });
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  const sent = [];
  const physicalDistances = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  client.sent = sent;
  client.events = [];
  client.sequence = 0;
  client.send = command => {
    sent.push(command);
    const deltas = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] };
    const [dx, dy] = deltas[command.direction];
    for (let step = 0; step < (command.type === 'run' ? 2 : 1); step += 1) {
      owner.x += dx;
      owner.y += dy;
      physicalDistances.push(Math.max(Math.abs(owner.x - target.x), Math.abs(owner.y - target.y)));
    }
  };
  client.wait = async () => true;
  let movedDuringCadence = false;
  const navigator = createNavigator(client, {
    loadCollisionMap: async () => openMap(20, 20),
    delay: async () => {
      if (!movedDuringCadence) {
        movedDuringCadence = true;
        // Planning saw the Shaman at (5,11); cadence reveals it now standing
        // on the player, so every proposed physical cell is in its footprint.
        Object.assign(target, { x: 5, y: 5 });
      }
    },
    now: () => 10_000,
  });
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async () => { actions += 1; return { kind: 'magic', targetId: target.objectId }; },
    navigator,
    {
      loadCollisionMap: async () => openMap(20, 20),
      fightWhenBlocked: true,
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7,
        maximumTargetDistance: 9,
        unsafeShamanDistance: 6,
        maxRetreatSteps: 6,
      },
    },
  );

  await assert.rejects(wrapped(client, target), /no collision-safe Wizard ranged band/i);
  assert.equal(movedDuringCadence, true);
  assert.equal(actions, 0);
  assert.ok(sent.length > 0, 'a player already in the new footprint may only move outward');
  assert.ok(physicalDistances.every((value, index) => index === 0
    ? value > 0
    : value > physicalDistances[index - 1]), 'every dispatched physical cell must increase Shaman distance');
  assert.equal(actions, 0);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'wizardKiteFallback' && entry.reason === 'rangedSafetyBandInvalidAfterNavigation'));
});

test('D2031 entry has a collision-valid nine-tile CursedShaman firing position', async () => {
  const map = await loadProtocolCollisionMap('D2031');
  const shaman = { x: 263, y: 273 };
  const plan = planProtocolNavigation({
    map,
    start: { x: 278, y: 284 },
    target: shaman,
    desiredDistance: 9,
    dynamicObstacles: [{ x: 275, y: 270 }],
  });

  assert.ok(plan);
  assert.deepEqual(plan.path.at(-1), { x: 272, y: 282 });
  assert.equal(plan.steps.length, 6);
  assert.equal(Math.max(Math.abs(272 - shaman.x), Math.abs(282 - shaman.y)), 9);
  assert.equal(Math.max(Math.abs(272 - 275), Math.abs(282 - 270)) > 6, true);
});

test('q89 Wizard wrapper reaches the live D2031 entry band before casting', async () => {
  const owner = player({ x: 278, y: 284 });
  const target = monster(341100, 263, 273, { name: 'CursedShaman', hp: 205 });
  const secondShaman = monster(341105, 275, 270, { name: 'CursedShaman0', hp: 205 });
  const client = clientFixture([owner, target, secondShaman], { mapFileName: 'D2031' });
  const navigationCalls = [];
  navigationCalls.client = client;
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async (_client, refreshedTarget) => {
      actions += 1;
      return { kind: 'magic', targetId: refreshedTarget.objectId };
    },
    directNavigator(navigationCalls),
    {
      loadCollisionMap: loadProtocolCollisionMap,
      fightWhenBlocked: true,
      maxExpanded: 256,
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7,
        maximumTargetDistance: 9,
        unsafeShamanDistance: 6,
        maxRetreatSteps: 6,
      },
    },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'magic', targetId: 341100 });
  assert.equal(actions, 1);
  assert.equal(navigationCalls.length, 1);
  assert.ok(navigationCalls[0].options.maxSuccessfulSteps <= 6);
  assert.equal(Math.max(Math.abs(owner.x - target.x), Math.abs(owner.y - target.y)), 9);
  assert.equal(Math.max(Math.abs(owner.x - secondShaman.x), Math.abs(owner.y - secondShaman.y)) > 6, true);
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
  assert.equal(navigationCalls[0].options.beforeMovement, undefined,
    'classes without the q89 Wizard policy retain their ordinary kiting navigator');
  assert.ok(Math.max(Math.abs(owner.x - 5), Math.abs(owner.y - 5)) <= 3);
});

test('ranged retreat never selects or crosses a live map-transfer cell', async () => {
  const owner = player();
  const target = monster(30, 5, 7);
  const secondThreat = monster(31, 6, 5);
  const client = clientFixture([owner, target, secondThreat], {
    mapTransfers: [{
      key: 'cave-exit',
      mapFileName: 'test',
      bounds: { minX: 2, maxX: 4, minY: 2, maxY: 4 },
    }],
  });
  const navigationCalls = [];
  navigationCalls.client = client;
  const wrapped = createWizardKitingAction(
    async () => ({ kind: 'magic', targetId: target.objectId }),
    directNavigator(navigationCalls),
    { loadCollisionMap: async () => openMap() },
  );

  await wrapped(client, target);
  assert.equal(navigationCalls.length, 1);
  const { destination, options } = navigationCalls[0];
  assert.equal(
    destination.x >= 2 && destination.x <= 4 && destination.y >= 2 && destination.y <= 4,
    false,
  );
  assert.deepEqual(options.forbiddenPoints, [
    { x: 2, y: 2 }, { x: 3, y: 2 }, { x: 4, y: 2 },
    { x: 2, y: 3 }, { x: 3, y: 3 }, { x: 4, y: 3 },
    { x: 2, y: 4 }, { x: 3, y: 4 }, { x: 4, y: 4 },
  ]);
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

test('target leaving AOI after a successful retreat refreshes the encounter', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let acted = 0;
  const wrapped = createWizardKitingAction(
    async () => { acted += 1; return { kind: 'magic' }; },
    async destination => {
      Object.assign(owner, destination);
      client.snapshot.entities = [owner];
      return { reached: true, successfulSteps: 1 };
    },
    { loadCollisionMap: async () => openMap(), maxRetreatSteps: 1 },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'retreated', targetId: 20 });
  assert.equal(acted, 0);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'wizardKiteFallback' && entry.reason === 'targetLeftSnapshotAfterRetreat'));
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

test('journey mode refreshes after an authoritative correction burst exceeds the local retreat budget', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let acted = 0;
  const wrapped = createWizardKitingAction(
    async () => { acted += 1; return { kind: 'magic', targetId: target.objectId }; },
    async () => {
      Object.assign(owner, { x: 3, y: 1 });
      return { reached: false, successfulSteps: 0 };
    },
    { loadCollisionMap: async () => openMap(), fightWhenBlocked: true },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'retreated', targetId: target.objectId });
  assert.equal(acted, 0);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'wizardKiteFallback' && entry.reason === 'retreatDisplacementOutOfBounds' &&
    entry.moved === 4 && entry.stepBudget === 3));
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

test('an opt-in pre-base-action guard blocks the captured cast after kiting navigation receives fresh critical strikes', async () => {
  const owner = player({ x: 5, y: 5, hp: 80, maxHp: 180 });
  const target = monster(20, 7, 5);
  const nearby = monster(21, 5, 6);
  const client = clientFixture([owner, target, nearby], { playerHp: 80, playerMaxHp: 180 });
  client.events = [];
  let casts = 0;
  let guardCalls = 0;
  const wrapped = createWizardKitingAction(
    async () => { casts += 1; return { kind: 'magic', targetId: target.objectId }; },
    async destination => {
      Object.assign(owner, destination, { hp: 7, maxHp: 180 });
      Object.assign(client.snapshot, { playerHp: 7, playerMaxHp: 180 });
      Object.assign(target, { x: owner.x + 1, y: owner.y });
      Object.assign(nearby, { x: owner.x, y: owner.y + 1 });
      client.events.push(
        { direction: 'received', packet: 'ObjectStruck', payload: { objectId: 1, attackerId: 20 } },
        { direction: 'received', packet: 'ObjectStruck', payload: { objectId: 1, attackerId: 21 } },
      );
      return { reached: true, successfulSteps: 1 };
    },
    {
      loadCollisionMap: async () => openMap(),
      beforeBaseAction: current => {
        guardCalls += 1;
        assert.equal(current.snapshot.playerHp, 7);
        assert.equal(current.events.filter(event => event.packet === 'ObjectStruck').length, 2);
        return criticalProvenAggressorOffenseGuardActive(current, {
          criticalProvenAggressorOffenseGuard: { hpRatio: 0.35, minimumProvenAggressors: 2 },
        }) ? { kind: 'wait', delayMs: 1 } : null;
      },
    },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'wait', delayMs: 1 });
  assert.equal(guardCalls, 1);
  assert.equal(casts, 0);
});

test('an opt-in pre-base-action guard covers both fast-path and blocked-retreat fallbacks', async () => {
  let casts = 0;
  const block = () => ({ kind: 'wait', delayMs: 1 });
  const fastOwner = player();
  const fastTarget = monster(20, 12, 5);
  const fastClient = clientFixture([fastOwner, fastTarget]);
  const fast = createWizardKitingAction(
    async () => { casts += 1; return { kind: 'magic' }; },
    async () => { throw new Error('fast path must not navigate'); },
    { loadCollisionMap: async () => openMap(), beforeBaseAction: block },
  );
  assert.deepEqual(await fast(fastClient, fastTarget), { kind: 'wait', delayMs: 1 });

  const blocked = [];
  for (let y = 0; y < 5; y += 1) {
    for (let x = 0; x < 5; x += 1) {
      if (x !== 2 || y !== 2) blocked.push({ x, y });
    }
  }
  const blockedOwner = player({ x: 2, y: 2 });
  const blockedTarget = monster(21, 3, 2);
  const blockedClient = clientFixture([blockedOwner, blockedTarget]);
  const fallback = createWizardKitingAction(
    async () => { casts += 1; return { kind: 'magic' }; },
    async () => { throw new Error('blocked fallback must not navigate'); },
    {
      loadCollisionMap: async () => openMap(5, 5, blocked),
      maxExpanded: 25,
      fightWhenBlocked: true,
      beforeBaseAction: block,
    },
  );
  assert.deepEqual(await fallback(blockedClient, blockedTarget), { kind: 'wait', delayMs: 1 });
  assert.equal(casts, 0);
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

test('q89 exhausted retreat budget never falls through to a close Shaman cast', async () => {
  const owner = player();
  const target = monster(20, 12, 5, { name: 'CursedShaman' });
  const closeZombie = monster(21, 6, 5, { name: 'CursedZombie' });
  const client = clientFixture([owner, target, closeZombie]);
  const navigationCalls = [];
  navigationCalls.client = client;
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async (_client, refreshedTarget) => {
      actions += 1;
      // The authoritative target can step inward between cadence actions.
      // Preserve the object id so this is the same exhausted engagement.
      if (actions === 1) Object.assign(refreshedTarget, { x: owner.x + 6, y: owner.y });
      return { kind: 'magic', targetId: refreshedTarget.objectId };
    },
    directNavigator(navigationCalls),
    {
      loadCollisionMap: async () => openMap(),
      maxRetreatSteps: 1,
      maxRetreatCellsPerTarget: 1,
      fightWhenBlocked: true,
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7,
        maximumTargetDistance: 9,
        unsafeShamanDistance: 6,
      },
    },
  );

  await wrapped(client, target);
  await assert.rejects(
    wrapped(client, target),
    error => error instanceof RangedSafetyBandUnavailable,
  );
  assert.equal(actions, 1);
  assert.equal(navigationCalls.length, 1);
});

test('generic exhausted retreat budget retains its journey combat fallback', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const navigationCalls = [];
  navigationCalls.client = client;
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async () => ({ kind: 'magic', call: ++actions }),
    directNavigator(navigationCalls),
    {
      loadCollisionMap: async () => openMap(),
      maxRetreatSteps: 1,
      maxRetreatCellsPerTarget: 1,
      fightWhenBlocked: true,
    },
  );

  await wrapped(client, target);
  Object.assign(owner, { x: 5, y: 5 });
  assert.deepEqual(await wrapped(client, target), { kind: 'magic', call: 2 });
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

test('authoritative map change during retreat refreshes without issuing combat', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
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

  assert.deepEqual(await wrapped(client, target), { kind: 'retreated', targetId: 20 });
  assert.equal(acted, false);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'wizardKiteFallback' && entry.reason === 'retreatMapChanged' &&
    entry.fromMapFileName === 'test' && entry.toMapFileName === 'other-map'));
});

test('known blocked retreat with an authoritative map change refreshes safely', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let acted = false;
  const wrapped = createWizardKitingAction(
    async () => { acted = true; },
    async () => {
      client.snapshot.mapFileName = '0';
      throw new Error('No walk path after emergency teleport');
    },
    { loadCollisionMap: async () => openMap(), maxRetreatSteps: 1, fightWhenBlocked: true },
  );

  assert.deepEqual(await wrapped(client, target), { kind: 'retreated', targetId: 20 });
  assert.equal(acted, false);
  assert.ok(diagnostics.some(entry => entry.reason === 'retreatMapChanged'));
});

test('map change plus death remains fatal during retreat', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const wrapped = createWizardKitingAction(
    async () => { throw new Error('combat must not run'); },
    async destination => {
      Object.assign(owner, destination, { hp: 0, dead: true });
      client.snapshot.playerHp = 0;
      client.snapshot.mapFileName = '0';
      return { reached: true, successfulSteps: 1 };
    },
    { loadCollisionMap: async () => openMap(), maxRetreatSteps: 1 },
  );

  await assert.rejects(wrapped(client, target), /Player died during Wizard retreat/);
});

test('unknown retreat navigation errors remain fatal', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target]);
  const wrapped = createWizardKitingAction(
    async () => { throw new Error('combat must not run'); },
    async () => { throw new Error('socket closed unexpectedly'); },
    { loadCollisionMap: async () => openMap(), maxRetreatSteps: 1, fightWhenBlocked: true },
  );

  await assert.rejects(wrapped(client, target), /socket closed unexpectedly/);
});

test('missing authoritative map remains fatal before retreat', async () => {
  const owner = player();
  const target = monster(20, 7, 5);
  const client = clientFixture([owner, target], { mapFileName: '' });
  const wrapped = createWizardKitingAction(
    async () => { throw new Error('combat must not run'); },
    async () => { throw new Error('navigation must not run'); },
    { loadCollisionMap: async () => openMap(), maxRetreatSteps: 1 },
  );

  await assert.rejects(wrapped(client, target), /Cannot kite without an authoritative mapFileName/);
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


test('q89 kiting installs the physical halo guard before a Shaman exists in the planning AOI', async () => {
  const owner = player({ x: 5, y: 5 });
  const priest = monster(20, 9, 5, { name: 'CursedPriest' });
  const closeZombie = monster(21, 5, 6, { name: 'HungryZombie' });
  const client = clientFixture([owner, priest, closeZombie]);
  let actions = 0;
  let navigations = 0;
  const wrapped = createWizardKitingAction(
    async () => { actions += 1; return { kind: 'magic', targetId: priest.objectId }; },
    async (_destination, _distance, _stopWhen, options) => {
      navigations += 1;
      assert.equal(typeof options.beforeMovement, 'function', 'the policy must not depend on initial AOI names');
      // Simulate a Shaman revealed after collision planning, at navigator
      // cadence immediately before a two-cell Run would be sent.
      const freshShaman = monster(22, 6, 5, { name: 'CursedShaman' });
      client.snapshot.entities.push(freshShaman);
      const hazard = options.beforeMovement({
        mapId: 'test', from: { x: 5, y: 5 },
        physicalCells: [{ x: 6, y: 5 }, { x: 7, y: 5 }], movementType: 'run',
      });
      assert.ok(hazard);
      throw new NavigationStepGuarded(hazard);
    },
    {
      loadCollisionMap: async () => openMap(),
      fightWhenBlocked: true,
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7, maximumTargetDistance: 9, unsafeShamanDistance: 6,
      },
    },
  );

  await assert.rejects(() => wrapped(client, priest), RangedSafetyBandUnavailable);
  assert.equal(navigations, 1);
  assert.equal(actions, 0, 'no physical Run or ordinary-target cast may follow the fresh halo');
});

test('q89 named Shaman guard blocks a fresh footprint during non-Shaman kiting without casting', async () => {
  const owner = player({ x: 5, y: 5 });
  const priest = monster(20, 9, 5, { name: 'CursedPriest' });
  const closeZombie = monster(21, 5, 6, { name: 'HungryZombie' });
  const movingShaman = monster(22, 20, 5, { name: 'CursedShaman' });
  const client = clientFixture([owner, priest, closeZombie, movingShaman]);
  let actions = 0;
  let navigations = 0;
  const wrapped = createWizardKitingAction(
    async () => { actions += 1; return { kind: 'magic', targetId: priest.objectId }; },
    async (_destination, _distance, _stopWhen, options) => {
      navigations += 1;
      assert.equal(typeof options.beforeMovement, 'function');
      // This AOI change occurs after the retreat was planned and before the
      // movement packet; both physical Run cells must be rejected.
      Object.assign(movingShaman, { x: 6, y: 5 });
      const hazard = options.beforeMovement({
        mapId: 'test', from: { x: 5, y: 5 },
        physicalCells: [{ x: 6, y: 5 }, { x: 7, y: 5 }], movementType: 'run',
      });
      assert.ok(hazard);
      throw new NavigationStepGuarded(hazard);
    },
    {
      loadCollisionMap: async () => openMap(),
      fightWhenBlocked: true,
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7, maximumTargetDistance: 9, unsafeShamanDistance: 6,
      },
    },
  );

  await assert.rejects(() => wrapped(client, priest), RangedSafetyBandUnavailable);
  assert.equal(navigations, 1);
  assert.equal(actions, 0, 'the guarded fresh physical cell must prevent the ordinary Priest cast');
});

test('q89 named Shaman guard permits only an outward escape from a pre-existing halo', async () => {
  const owner = player({ x: 7, y: 5 });
  const shaman = monster(20, 13, 5, { name: 'CursedShaman' });
  const client = clientFixture([owner, shaman]);
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async () => { actions += 1; return { kind: 'magic', targetId: shaman.objectId }; },
    async (_destination, _distance, _stopWhen, options) => {
      assert.equal(typeof options.beforeMovement, 'function');
      assert.equal(options.beforeMovement({
        mapId: 'test', from: { x: 7, y: 5 },
        physicalCells: [{ x: 6, y: 5 }, { x: 5, y: 5 }, { x: 4, y: 5 }], movementType: 'run',
      }), null, 'distance six to seven to eight to nine is a strictly outward recovery');
      Object.assign(owner, { x: 4, y: 5 });
      return { reached: true, successfulSteps: 3 };
    },
    {
      loadCollisionMap: async () => openMap(40, 40),
      fightWhenBlocked: true,
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7, maximumTargetDistance: 9, unsafeShamanDistance: 6,
      },
    },
  );

  assert.deepEqual(await wrapped(client, shaman), { kind: 'magic', targetId: shaman.objectId });
  assert.equal(actions, 1);
});

test('q89 no-motion base action rechecks a newly adjacent Shaman before casting', async () => {
  const owner = player({ x: 5, y: 5 });
  const priest = monster(20, 10, 5, { name: 'CursedPriest' });
  const shaman = monster(21, 20, 5, { name: 'CursedShaman' });
  const client = clientFixture([owner, priest, shaman]);
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async () => { actions += 1; return { kind: 'magic', targetId: priest.objectId }; },
    async () => { throw new Error('no close threat must not navigate'); },
    {
      beforeBaseAction: async () => { Object.assign(shaman, { x: 9, y: 5 }); },
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7, maximumTargetDistance: 9, unsafeShamanDistance: 6,
      },
    },
  );

  await assert.rejects(() => wrapped(client, priest), RangedSafetyBandUnavailable);
  assert.equal(actions, 0);
});


test('q89 policy action uses the target object refreshed after its asynchronous pre-action callback', async () => {
  const owner = player({ x: 5, y: 5 });
  const stalePriest = monster(20, 10, 5, { name: 'CursedPriest' });
  const client = clientFixture([owner, stalePriest]);
  const replacement = monster(20, 13, 7, { name: 'CursedPriest', hp: 19 });
  let received = null;
  const wrapped = createWizardKitingAction(
    async (_client, target) => { received = target; return { kind: 'magic', targetId: target.objectId }; },
    async () => { throw new Error('no close threat must not navigate'); },
    {
      beforeBaseAction: async () => {
        client.snapshot.entities = [owner, replacement];
      },
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7, maximumTargetDistance: 9, unsafeShamanDistance: 6,
      },
    },
  );

  assert.deepEqual(await wrapped(client, stalePriest), { kind: 'magic', targetId: 20 });
  assert.equal(received, replacement, 'the cast must receive the current same-id target object');
  assert.deepEqual({ x: received.x, y: received.y, hp: received.hp }, { x: 13, y: 7, hp: 19 });
});

test('q89 policy action returns safely without casting when its target leaves after the pre-action callback', async () => {
  const owner = player({ x: 5, y: 5 });
  const priest = monster(20, 10, 5, { name: 'CursedPriest' });
  const client = clientFixture([owner, priest]);
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async () => { actions += 1; return { kind: 'magic', targetId: priest.objectId }; },
    async () => { throw new Error('no close threat must not navigate'); },
    {
      beforeBaseAction: async () => { client.snapshot.entities = [owner]; },
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7, maximumTargetDistance: 9, unsafeShamanDistance: 6,
      },
    },
  );

  assert.deepEqual(await wrapped(client, priest), { kind: 'retreated', targetId: 20 });
  assert.equal(actions, 0);
});

test('q89 policy action does not cast a target confirmed dead after the pre-action callback', async () => {
  const owner = player({ x: 5, y: 5 });
  const priest = monster(20, 10, 5, { name: 'CursedPriest' });
  const client = clientFixture([owner, priest]);
  let actions = 0;
  const wrapped = createWizardKitingAction(
    async () => { actions += 1; return { kind: 'magic', targetId: priest.objectId }; },
    async () => { throw new Error('no close threat must not navigate'); },
    {
      beforeBaseAction: async () => { Object.assign(priest, { dead: true, hp: 0 }); },
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7, maximumTargetDistance: 9, unsafeShamanDistance: 6,
      },
    },
  );

  assert.deepEqual(await wrapped(client, priest), { kind: 'retreated', targetId: 20 });
  assert.equal(actions, 0);
});
