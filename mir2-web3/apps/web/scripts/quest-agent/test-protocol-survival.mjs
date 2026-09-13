import assert from 'node:assert/strict';
import test from 'node:test';

import {
  canFinishNearCompleteKillQuestWithoutHpStock,
  canFinishNearCompleteKillQuestWithReducedHpStock,
  canResumeStockedCombatExpedition,
  createPostEngagementSupplyGate,
  evasiveRecoveryDangerDistanceForQuest,
  evasiveRecoveryTimeoutMsForQuest,
  hpDrugCount,
  mpDrugCount,
  hpRestockTargetForActiveQuests,
  journeyExpeditionDepartureFloorForQuest,
  journeyResumeDisposition,
  journeyMpRestockTargetForQuest,
  questCombatMpUseThresholdForQuest,
  minimumJourneyMpStockForQuest,
  questRetreatBiasPosition,
  questRetreatProfile,
  questNeedsPostRetreatRecovery,
  shouldPreferObjectiveMapOverCurrent,
  recoverHealthWhileEvading,
  waitForPassiveHealthRecovery,
} from './protocol-survival.mjs';

test('level-15 Taoist snake hunting does not require MP-only town trips', () => {
  assert.equal(minimumJourneyMpStockForQuest(42, 'Taoist'), 0);
  assert.equal(minimumJourneyMpStockForQuest(49, 'Taoist'), 4);
  assert.equal(minimumJourneyMpStockForQuest(42, 'Wizard'), 4);
  assert.equal(minimumJourneyMpStockForQuest(42, 'Warrior'), 0);
  assert.equal(minimumJourneyMpStockForQuest(54, 'Wizard'), 12);
  assert.equal(minimumJourneyMpStockForQuest(54, 'Taoist'), 12);
});

test('caster expedition restock targets stay separate from their field triggers', () => {
  assert.equal(journeyMpRestockTargetForQuest(54, 'Wizard'), 80);
  assert.equal(journeyMpRestockTargetForQuest(54, 'Taoist'), 80);
  assert.equal(journeyMpRestockTargetForQuest(42, 'Wizard'), 32);
  assert.equal(journeyMpRestockTargetForQuest(49, 'Taoist'), 12);
  assert.equal(journeyMpRestockTargetForQuest(54, 'Warrior'), 0);
  assert.equal(journeyMpRestockTargetForQuest(49, 'Wizard', { fallback: 18 }), 18);
  assert.equal(questCombatMpUseThresholdForQuest(54, 'Wizard'), 0.75);
  assert.equal(questCombatMpUseThresholdForQuest(54, 'Taoist'), 0.75);
  assert.equal(questCombatMpUseThresholdForQuest(54, 'Warrior'), 0.3);
  assert.equal(questCombatMpUseThresholdForQuest(49, 'Wizard', { fallback: 0.4 }), 0.4);
});

test('long expeditions accept a partial but still conservative funded departure stock', () => {
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(54, 'Wizard'), { hp: 64, mp: 64 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(54, 'Taoist'), { hp: 64, mp: 64 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(54, 'Warrior'), { hp: 64, mp: 0 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(62, 'Warrior'), { hp: 64, mp: 0 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(49, 'Wizard'), { hp: 0, mp: 0 });
});

test('a stocked Warrior resumes q54 combat instead of attempting passive field recovery', () => {
  const snapshot = {
    playerObjectId: 1,
    playerHp: 137,
    playerMaxHp: 192,
    questLog: [{ questId: 54, stage: 'inProgress' }],
    inventoryItems: [{ name: '(HP)DrugSmall', quantity: 51 }],
    beltItems: [{ name: '(HP)DrugSmall', quantity: 20 }],
    entities: [{ objectId: 1, kind: 'selfPlayer', class: 'Warrior', hp: 137, maxHp: 192 }],
  };
  assert.equal(canResumeStockedCombatExpedition(snapshot), true);
  snapshot.playerHp = 80;
  assert.equal(canResumeStockedCombatExpedition(snapshot), false);
  snapshot.playerHp = 137;
  snapshot.inventoryItems[0].quantity = 0;
  assert.equal(canResumeStockedCombatExpedition(snapshot), false);
});

test('a stocked q54 Taoist reaches the class-specific retreat loop without waiting in the old field', () => {
  const state = {
    playerObjectId: 1,
    playerHp: 52,
    playerMaxHp: 113,
    questLog: [{ questId: 54, stage: 'inProgress' }],
    inventoryItems: [
      { name: '(HP)DrugSmall', quantity: 40 },
      { name: '(MP)DrugSmall', quantity: 8 },
    ],
    entities: [{ objectId: 1, kind: 'selfPlayer', class: 'Taoist', hp: 52, maxHp: 113 }],
  };
  assert.equal(canResumeStockedCombatExpedition(state), true);
  state.inventoryItems[1].quantity = 3;
  assert.equal(canResumeStockedCombatExpedition(state), false);
});

test('resumed field pressure evades while recoverable and waits for death only below one-hit survival', () => {
  const actor = { objectId: 1, kind: 'selfPlayer', x: 20, y: 20, hp: 30, maxHp: 68, dead: false };
  const pressured = {
    playerObjectId: 1,
    playerHp: 30,
    playerMaxHp: 68,
    entities: [
      actor,
      { objectId: 2, kind: 'monster', disposition: 'hostile', x: 20, y: 19, hp: 20 },
      { objectId: 3, kind: 'monster', disposition: 'hostile', x: 21, y: 20, hp: 20 },
    ],
  };

  assert.equal(journeyResumeDisposition(pressured).status, 'evade');
  pressured.playerHp = 2;
  actor.hp = 2;
  assert.equal(journeyResumeDisposition(pressured).status, 'awaitDeath');
  pressured.playerHp = 60;
  actor.hp = 60;
  assert.equal(journeyResumeDisposition(pressured).status, 'ready');
  pressured.playerHp = 30;
  actor.hp = 30;
  pressured.entities.splice(1);
  assert.equal(journeyResumeDisposition(pressured).status, 'ready');
});

test('active expedition quests raise HP stock without affecting completed quests', () => {
  const snapshot = {
    questLog: [
      { questId: 30, stage: 'completed' },
      { questId: 54, stage: 'inProgress' },
    ],
  };
  assert.equal(hpRestockTargetForActiveQuests(snapshot, {
    fallback: 24,
    targets: { 54: 80 },
  }), 80);
  snapshot.questLog[1].stage = 'completed';
  assert.equal(hpRestockTargetForActiveQuests(snapshot, {
    fallback: 24,
    targets: { 54: 80 },
  }), 24);
});

test('q62 can request a full insect-cave departure stock independently of the field trigger', () => {
  const state = {
    questLog: [{ questId: 62, stage: 'inProgress' }],
  };
  assert.equal(hpRestockTargetForActiveQuests(state, {
    fallback: 24,
    targets: { 62: 80 },
  }), 80);
  state.questLog[0].stage = 'completed';
  assert.equal(hpRestockTargetForActiveQuests(state, {
    fallback: 24,
    targets: { 62: 80 },
  }), 24);
});

function snapshot(mapFileName, hp = 0, gold = 500) {
  return {
    mapFileName,
    gold,
    questLog: [{ questId: 33, stage: 'inProgress' }],
    inventoryItems: [],
    beltItems: Array.from({ length: hp }, (_, index) => ({ name: '(HP)DrugSmall', uniqueId: index })),
  };
}

function clientAt(mapFileName, hp = 0) {
  return {
    snapshot: snapshot(mapFileName, hp),
    sequence: 4,
    events: [],
    sent: [],
    send(command) {
      this.sent.push(command);
      if (command.type === 'clientVersion') {
        this.sequence += 1;
        const payload = structuredClone(this.snapshot);
        this.events.push({ sequence: this.sequence, direction: 'received', type: 'worldSnapshot', payload });
      }
    },
    async wait(predicate, label) {
      const value = predicate();
      if (!value) throw new Error(`unsatisfied ${label}`);
      return value;
    },
  };
}

test('post-engagement gate leaves a stocked player in place', async () => {
  const client = clientAt('3', 2);
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    travel: async map => calls.push(['travel', map]),
    navigateNear: async () => calls.push(['navigate']),
    restock: async () => calls.push(['restock']),
  });
  assert.deepEqual(await gate(client), { status: 'sufficient', hp: 2, mp: 0, restockCount: 0 });
  assert.deepEqual(calls, []);
  assert.deepEqual(client.sent, []);
});

test('post-engagement gate can force a town service pass for missing combat gear', async () => {
  const client = clientAt('D001', 6);
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    minimumHpStock: 4,
    travel: async map => {
      calls.push(['travel', map]);
      client.snapshot = { ...client.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async owner => {
      calls.push(['restock', owner.snapshot.mapFileName]);
      return { status: 'restocked' };
    },
  });

  const result = await gate(client, { forceRestock: true });
  assert.equal(result.status, 'restocked');
  assert.deepEqual(calls, [['travel', '0'], ['restock', '0'], ['travel', 'D001']]);
});

test('journey stock threshold proactively restocks before the last potion is consumed', async () => {
  const client = clientAt('2', 6);
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    minimumHpStock: 4,
    travel: async map => {
      calls.push(['travel', map]);
      client.snapshot = { ...client.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async (owner, _navigateNear, options) => {
      calls.push(['restock', owner.snapshot.mapFileName, options]);
      owner.snapshot = snapshot('0', 24, 900);
      return { status: 'restocked' };
    },
  });

  const result = await gate(client, { minimumHpStock: 8 });
  assert.equal(result.hp, 24);
  assert.deepEqual(calls, [
    ['travel', '0'],
    ['restock', '0', { targetHp: 8, lowStockHp: 8 }],
    ['travel', '2'],
  ]);
});

test('an expedition refill can require a full departure stock without raising its field trigger', async () => {
  const client = clientAt('D401', 20);
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    minimumHpStock: 4,
    travel: async map => {
      calls.push(['travel', map]);
      client.snapshot = { ...client.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async (owner, _navigateNear, options) => {
      calls.push(['restock', options]);
      owner.snapshot = snapshot('0', 40, 0);
      return { status: 'restocked' };
    },
  });

  await assert.rejects(() => gate(client, {
    minimumHpStock: 24,
    requiredAfterRestockHpStock: 80,
    returnMapFileName: 'D406',
  }), /needsFunds: unable to restock supplies to HP 80/);
  assert.deepEqual(calls, [
    ['travel', '0'],
    ['restock', { targetHp: 80, lowStockHp: 80 }],
  ]);
  assert.equal(client.snapshot.mapFileName, '0');
});

test('empty HP stock travels to village, proves restock, returns and refreshes quest state', async () => {
  const client = clientAt('3', 0);
  const calls = [];
  const travel = async map => {
    calls.push(['travel', map]);
    client.snapshot = { ...client.snapshot, mapFileName: map };
  };
  const restock = async owner => {
    calls.push(['restock', owner.snapshot.mapFileName]);
    owner.snapshot = snapshot('0', 6, 260);
    return { status: 'restocked', after: { hp: 6, gold: 260 } };
  };
  const gate = createPostEngagementSupplyGate({ travel, navigateNear: async () => {}, restock });
  const result = await gate(client);
  assert.deepEqual(calls, [['travel', '0'], ['restock', '0'], ['travel', '3']]);
  assert.equal(result.status, 'restocked');
  assert.equal(result.hp, 6);
  assert.equal(result.returnMap, '3');
  assert.equal(result.restockCount, 1);
  assert.deepEqual(client.sent, [{ type: 'clientVersion' }]);
  assert.equal(client.snapshot.questLog[0].questId, 33);
});

test('expedition restock may return directly to a safer objective map', async () => {
  const client = clientAt('D421', 0);
  const traveled = [];
  const gate = createPostEngagementSupplyGate({
    travel: async map => {
      traveled.push(map);
      client.snapshot = { ...client.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async owner => {
      owner.snapshot = snapshot('0', 24, 900);
      return { status: 'restocked' };
    },
  });

  const result = await gate(client, { returnMapFileName: 'D406' });
  assert.deepEqual(traveled, ['0', 'D406']);
  assert.equal(result.returnMap, 'D406');
  assert.equal(client.snapshot.mapFileName, 'D406');
});

test('an explicit empty return leaves a dangerous expedition in town for its quest-aware traveler', async () => {
  const client = clientAt('D401', 0);
  const traveled = [];
  const gate = createPostEngagementSupplyGate({
    travel: async map => {
      traveled.push(map);
      client.snapshot = { ...client.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async owner => {
      owner.snapshot = snapshot('0', 80, 900);
      return { status: 'restocked' };
    },
  });

  const result = await gate(client, {
    requiredAfterRestockHpStock: 80,
    returnMapFileName: '',
  });
  assert.deepEqual(traveled, ['0']);
  assert.equal(result.returnMap, undefined);
  assert.equal(client.snapshot.mapFileName, '0');
});

test('needsFunds fails explicitly before returning to danger', async () => {
  const client = clientAt('2', 0);
  const traveled = [];
  const gate = createPostEngagementSupplyGate({
    travel: async map => {
      traveled.push(map);
      client.snapshot = { ...client.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async owner => {
      owner.snapshot = snapshot('0', 0, 100);
      return { status: 'needsFunds', gold: 100 };
    },
  });
  await assert.rejects(() => gate(client), /needsFunds/);
  assert.deepEqual(traveled, ['0']);
});

test('bounded restock count prevents endless supply travel', async () => {
  const client = clientAt('0', 0);
  const gate = createPostEngagementSupplyGate({
    travel: async () => {},
    navigateNear: async () => {},
    maxRestocks: 1,
    restock: async owner => {
      owner.snapshot = snapshot('0', 1);
      return { status: 'restocked' };
    },
  });
  await gate(client);
  client.snapshot = snapshot('0', 0);
  await assert.rejects(() => gate(client), /restock limit exceeded \(1\/1\)/);
});

test('HP stock matches supply use across Small, Medium and Large without counting MP drugs', () => {
  assert.equal(hpDrugCount({
    inventoryItems: [{ name: '(HP)DrugSmall', uniqueId: 0, count: 2 }],
    beltItems: [
      { key: '(HP)DrugMedium', uniqueId: 3, quantity: 3 },
      { name: '(HP)DrugLarge', uniqueId: 4 },
      { name: '(MP)DrugSmall', count: 9 },
    ],
  }), 6);
});

test('caster supply gate requires MP independently while Warrior HP remains sufficient', async () => {
  const caster = clientAt('0', 6);
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    travel: async () => {},
    navigateNear: async () => {},
    restock: async (owner, _navigateNear, options) => {
      calls.push(options);
      owner.snapshot.beltItems.push({ name: '(MP)DrugSmall', quantity: 12 });
      return { status: 'restocked' };
    },
  });
  const result = await gate(caster, { minimumHpStock: 4, minimumMpStock: 12 });
  assert.equal(result.status, 'restocked');
  assert.equal(result.mp, 12);
  assert.deepEqual(calls, [{ targetHp: 4, targetMp: 12, lowStockHp: 4, lowStockMp: 12 }]);
  assert.equal(mpDrugCount(caster.snapshot), 12);

  const warrior = clientAt('0', 6);
  assert.equal((await gate(warrior, { minimumHpStock: 4, minimumMpStock: 0 })).status, 'sufficient');
});

test('a caster expedition refills its departure MP stock without raising the field trigger', async () => {
  const caster = clientAt('2', 6);
  caster.snapshot.beltItems.push({ name: '(MP)DrugSmall', quantity: 3 });
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    travel: async map => {
      calls.push(['travel', map]);
      caster.snapshot = { ...caster.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async (owner, _navigateNear, options) => {
      calls.push(['restock', options]);
      owner.snapshot.beltItems = owner.snapshot.beltItems
        .filter(item => !String(item?.name ?? '').startsWith('(MP)Drug'));
      owner.snapshot.beltItems.push({ name: '(MP)DrugSmall', quantity: 32 });
      return { status: 'restocked' };
    },
  });

  const result = await gate(caster, {
    minimumHpStock: 4,
    minimumMpStock: 4,
    requiredAfterRestockMpStock: 32,
  });
  assert.equal(result.status, 'restocked');
  assert.equal(result.mp, 32);
  assert.deepEqual(calls, [
    ['travel', '0'],
    ['restock', { targetHp: 4, targetMp: 32, lowStockHp: 4, lowStockMp: 32 }],
    ['travel', '2'],
  ]);
});

test('only a healthy near-complete kill quest may make a cash-poor no-stock finish attempt', () => {
  const state = snapshot('0', 0, 37);
  Object.assign(state, { playerHp: 130, playerMaxHp: 135 });
  state.questLog = [{
    questId: 49,
    stage: 'inProgress',
    objectives: [{ label: 'Kill Skeleton', current: 8, required: 10 }],
  }];
  assert.equal(canFinishNearCompleteKillQuestWithoutHpStock(state, 49), true);

  state.questLog[0].objectives[0].current = 7;
  assert.equal(canFinishNearCompleteKillQuestWithoutHpStock(state, 49), false);
  state.questLog[0].objectives[0].current = 8;
  state.playerHp = 100;
  assert.equal(canFinishNearCompleteKillQuestWithoutHpStock(state, 49), false);
  assert.equal(canFinishNearCompleteKillQuestWithoutHpStock(state, 49, {
    minimumHealthRatio: 0,
  }), true);
  state.playerHp = 130;
  state.questLog[0].objectives[0].label = 'Collect Antidote';
  assert.equal(canFinishNearCompleteKillQuestWithoutHpStock(state, 49), false);
  state.questLog[0].objectives[0].label = 'Kill Skeleton';
  state.inventoryItems.push({ name: '(HP)DrugSmall', quantity: 1 });
  assert.equal(canFinishNearCompleteKillQuestWithoutHpStock(state, 49), false);
});

test('a healthy q42-style hunt can finish two kills from measured reduced stock', () => {
  const state = snapshot('2', 10, 400);
  Object.assign(state, { playerHp: 89, playerMaxHp: 89 });
  state.questLog = [{
    questId: 42,
    stage: 'inProgress',
    objectives: [
      { label: 'Kill RedViper', current: 10, required: 10 },
      { label: 'Kill TigerViper', current: 8, required: 10 },
    ],
  }];
  assert.equal(canFinishNearCompleteKillQuestWithReducedHpStock(state, 42), true);

  state.beltItems.length = 7;
  assert.equal(canFinishNearCompleteKillQuestWithReducedHpStock(state, 42), false);
  state.beltItems = Array.from({ length: 10 }, (_, index) => ({ name: '(HP)DrugSmall', uniqueId: index }));
  state.playerHp = 79;
  assert.equal(canFinishNearCompleteKillQuestWithReducedHpStock(state, 42), false);
});

test('passive recovery proves exact HP through bounded fresh snapshots', async () => {
  const client = clientAt('2', 0);
  Object.assign(client.snapshot, { playerHp: 91, playerMaxHp: 135 });
  let clock = 0;
  client.send = function send(command) {
    this.sent.push(command);
    if (command.type !== 'clientVersion') return;
    this.snapshot.playerHp = Math.min(this.snapshot.playerMaxHp, this.snapshot.playerHp + 5);
    this.sequence += 1;
    this.events.push({
      sequence: this.sequence,
      direction: 'received',
      type: 'worldSnapshot',
      payload: structuredClone(this.snapshot),
    });
  };

  const result = await waitForPassiveHealthRecovery(client, {
    requiredRatio: 0.75,
    timeoutMs: 20_000,
    pollMs: 3_100,
    sleep: async milliseconds => { clock += milliseconds; },
    now: () => clock,
  });

  assert.deepEqual(result, { status: 'recovered', hp: 106, maxHp: 135, refreshes: 3 });
  assert.deepEqual(client.sent, [
    { type: 'clientVersion' },
    { type: 'clientVersion' },
    { type: 'clientVersion' },
  ]);
});

test('evasive recovery moves away from a nearby follower before refreshing HP', async () => {
  const client = clientAt('2', 0);
  Object.assign(client.snapshot, {
    playerHp: 60,
    playerMaxHp: 100,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', x: 20, y: 20 },
      { objectId: 61, kind: 'monster', name: 'Currish', x: 21, y: 20, hp: 48 },
    ],
  });
  let clock = 0;
  client.send = function send(command) {
    this.sent.push(command);
    if (command.type !== 'clientVersion') return;
    this.snapshot.playerHp = 80;
    this.sequence += 1;
    this.events.push({
      sequence: this.sequence,
      direction: 'received',
      type: 'worldSnapshot',
      payload: structuredClone(this.snapshot),
    });
  };
  const moves = [];
  const navigateNear = async (target, range, _stopWhen, options) => {
    moves.push({ target, range, options });
    Object.assign(client.snapshot.entities[0], target);
  };

  const result = await recoverHealthWhileEvading(client, navigateNear, {
    requiredRatio: 0.75,
    timeoutMs: 10_000,
    pollMs: 1_000,
    retreatSteps: 6,
    dangerDistance: 7,
    sleep: async milliseconds => { clock += milliseconds; },
    now: () => clock,
  });

  assert.deepEqual(result, {
    status: 'recovered', hp: 80, maxHp: 100, refreshes: 1, evasiveMoves: 1,
  });
  assert.equal(moves.length, 1);
  assert.equal(moves[0].range, 0);
  assert.equal(moves[0].options.hostileAvoidanceRadius, 0);
  assert.equal(moves[0].options.maxNoPathRefreshes, 0);
  assert.deepEqual(moves[0].options.allowedHostileObjectIds, [61]);
  assert.ok(Math.max(
    Math.abs(Number(moves[0].target.x) - 21),
    Math.abs(Number(moves[0].target.y) - 20),
  ) > 1);
});

test('evasive recovery keeps sustaining after reaching a safe gap', async () => {
  const client = clientAt('2', 0);
  Object.assign(client.snapshot, {
    playerHp: 42,
    playerMaxHp: 89,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', x: 20, y: 20 },
      { objectId: 61, kind: 'monster', name: 'TigerSnake', x: 30, y: 30, hp: 48 },
    ],
  });
  let clock = 0;
  client.send = function send(command) {
    this.sent.push(command);
    if (command.type !== 'clientVersion') return;
    this.sequence += 1;
    this.events.push({
      sequence: this.sequence,
      direction: 'received',
      type: 'worldSnapshot',
      payload: structuredClone(this.snapshot),
    });
  };
  const sustainAt = [];
  let navigationAttempts = 0;

  const result = await recoverHealthWhileEvading(client, async () => {
    navigationAttempts += 1;
  }, {
    requiredRatio: 0.75,
    timeoutMs: 20_000,
    pollMs: 1_000,
    dangerDistance: 7,
    sustainCadenceMs: 6_000,
    sustain: async () => {
      sustainAt.push(clock);
      client.snapshot.playerHp = Math.min(client.snapshot.playerMaxHp, client.snapshot.playerHp + 20);
    },
    sleep: async milliseconds => { clock += milliseconds; },
    now: () => clock,
  });

  assert.equal(navigationAttempts, 0);
  assert.deepEqual(sustainAt, [0, 6_000], 'the six-second potion cadence must remain enforced');
  assert.equal(result.hp, 82);
  assert.equal(result.refreshes, 6);
});

test('expedition recovery budgets span cadence-limited doses beyond the short field timeout', async () => {
  assert.equal(evasiveRecoveryTimeoutMsForQuest(42), 90_000);
  assert.equal(evasiveRecoveryTimeoutMsForQuest(60), 90_000);
  assert.equal(evasiveRecoveryTimeoutMsForQuest(62), 90_000);
  assert.equal(evasiveRecoveryTimeoutMsForQuest(41), 45_000);
  assert.equal(evasiveRecoveryDangerDistanceForQuest(42), 8);
  assert.equal(evasiveRecoveryDangerDistanceForQuest(60), 8);
  assert.equal(evasiveRecoveryDangerDistanceForQuest(62), 8);
  assert.equal(evasiveRecoveryDangerDistanceForQuest(54), 7);

  const run = async timeoutMs => {
    const client = clientAt('2', 0);
    Object.assign(client.snapshot, {
      playerHp: 32,
      playerMaxHp: 56,
      playerObjectId: 1,
      entities: [
        { objectId: 1, kind: 'player', x: 20, y: 20 },
        { objectId: 61, kind: 'monster', name: 'TigerSnake', x: 28, y: 20, hp: 48 },
      ],
    });
    let clock = 0;
    const sustainAt = [];
    client.send = function send(command) {
      this.sent.push(command);
      if (command.type !== 'clientVersion') return;
      this.sequence += 1;
      this.events.push({
        sequence: this.sequence,
        direction: 'received',
        type: 'worldSnapshot',
        payload: structuredClone(this.snapshot),
      });
    };
    const result = await recoverHealthWhileEvading(client, async () => {
      throw new Error('the eight-tile safety gap should not navigate');
    }, {
      requiredRatio: 0.75,
      timeoutMs,
      pollMs: 1_000,
      dangerDistance: evasiveRecoveryDangerDistanceForQuest(42),
      sustainCadenceMs: 6_000,
      sustain: async () => {
        sustainAt.push(clock);
        client.snapshot.playerHp += 1;
      },
      sleep: async milliseconds => { clock += milliseconds; },
      now: () => clock,
    });
    return { result, sustainAt };
  };

  await assert.rejects(() => run(45_000), /Evasive HP recovery timed out/);
  const recovered = await run(evasiveRecoveryTimeoutMsForQuest(42));
  assert.equal(recovered.result.hp, 42);
  assert.ok(recovered.sustainAt.at(-1) >= 54_000);
  for (let index = 1; index < recovered.sustainAt.length; index += 1) {
    assert.ok(recovered.sustainAt[index] - recovered.sustainAt[index - 1] >= 6_000);
  }
});

test('q62 leaves D2041 packs toward the real D2042 transfer', () => {
  assert.deepEqual(questRetreatProfile(62), {
    allowLowHealthFollowerRecovery: true,
    continueTravelWhileHealthy: true,
    maxTravelThreatEvasionsPerEdge: 3,
    maxRetreatBreakoutKills: 3,
    multiAggressorRetreatRatio: undefined,
    retreatAtActiveAggressorCount: undefined,
    unsafeRetreatSteps: 24,
    unsafeRetreatSafeDistance: 8,
  });
  assert.deepEqual(questRetreatBiasPosition(62, { mapFileName: 'D2041' }), { x: 262, y: 13 });
  assert.equal(questRetreatBiasPosition(62, { mapFileName: 'D2042' }), null);
});

test('q54 keeps Warrior mine thresholds and gives a trapped healing Taoist bounded breakout room', () => {
  assert.deepEqual(questRetreatProfile(54, 'Warrior'), {
    allowLowHealthFollowerRecovery: true,
    continueTravelWhileHealthy: true,
    maxTravelThreatEvasionsPerEdge: 1,
    maxRetreatBreakoutKills: 3,
    multiAggressorRetreatRatio: 0.55,
    retreatAtActiveAggressorCount: 3,
    unsafeRetreatSteps: 12,
    unsafeRetreatSafeDistance: 6,
  });
  for (const className of ['Wizard', 'Taoist']) {
    const profile = questRetreatProfile(54, className);
    assert.equal(profile.continueTravelWhileHealthy, true);
    assert.equal(profile.maxRetreatBreakoutKills, className === 'Taoist' ? 3 : 1);
    assert.equal(profile.multiAggressorRetreatRatio, 0.75);
    assert.equal(profile.retreatAtActiveAggressorCount, 1);
    assert.equal(profile.unsafeRetreatSteps, 24);
    assert.equal(profile.unsafeRetreatSafeDistance, 8);
  }
});

test('q54 finishes available D401 zombies before preferring the D406 source', () => {
  assert.equal(shouldPreferObjectiveMapOverCurrent(54), false);
  assert.equal(shouldPreferObjectiveMapOverCurrent(49), true);
  assert.equal(shouldPreferObjectiveMapOverCurrent(62), false);
});

test('a completed combat objective leaves for turn-in instead of recovering in its old field', () => {
  const state = snapshot('2', 9, 2_342);
  Object.assign(state, {
    playerObjectId: 1,
    playerHp: 27,
    playerMaxHp: 56,
    entities: [
      { objectId: 1, kind: 'selfPlayer', x: 469, y: 328, hp: 27, maxHp: 56 },
      { objectId: 2, kind: 'monster', disposition: 'hostile', x: 470, y: 328, hp: 20 },
    ],
  });
  state.questLog = [{ questId: 42, stage: 'readyToTurnIn' }];
  assert.equal(questNeedsPostRetreatRecovery(state, 42), false);
  assert.deepEqual(journeyResumeDisposition(state), { status: 'readyToTurnIn', questId: 42 });
  state.questLog[0].stage = 'inProgress';
  assert.equal(questNeedsPostRetreatRecovery(state, 42), true);
  assert.equal(journeyResumeDisposition(state).status, 'evade');
});

test('evasive recovery recomputes from a partial failed move and reaches clearance before sleeping', async () => {
  const client = clientAt('2', 0);
  Object.assign(client.snapshot, {
    playerHp: 60,
    playerMaxHp: 100,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', x: 20, y: 20 },
      { objectId: 61, kind: 'monster', name: 'Currish', x: 21, y: 20, hp: 48 },
    ],
  });
  let clock = 0;
  client.send = function send(command) {
    this.sent.push(command);
    if (command.type !== 'clientVersion') return;
    this.snapshot.playerHp = 80;
    this.sequence += 1;
    this.events.push({
      sequence: this.sequence,
      direction: 'received',
      type: 'worldSnapshot',
      payload: structuredClone(this.snapshot),
    });
  };
  const moves = [];
  const navigateNear = async (target, _range, _stopWhen, options) => {
    moves.push({ target: { ...target }, options });
    if (moves.length === 1) {
      Object.assign(client.snapshot.entities[0], { x: 20, y: 21 });
      throw new Error('partial movement before dynamic obstruction');
    }
    Object.assign(client.snapshot.entities[0], target);
  };

  const result = await recoverHealthWhileEvading(client, navigateNear, {
    requiredRatio: 0.75,
    timeoutMs: 10_000,
    pollMs: 1_000,
    retreatSteps: 6,
    dangerDistance: 7,
    sleep: async milliseconds => { clock += milliseconds; },
    now: () => clock,
  });

  assert.equal(moves.length, 2);
  assert.deepEqual(moves[0].target, { x: 14, y: 26 });
  assert.deepEqual(moves[1].target, { x: 26, y: 27 });
  assert.equal(clock, 1_000);
  assert.equal(result.evasiveMoves, 2);
  assert.equal(result.hp, 80);
});

test('evasive recovery keeps kiting a same-speed follower across bounded refresh batches', async () => {
  const client = clientAt('2', 0);
  Object.assign(client.snapshot, {
    playerHp: 60,
    playerMaxHp: 100,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', x: 20, y: 20 },
      { objectId: 61, kind: 'monster', name: 'Currish', x: 21, y: 20, hp: 48 },
    ],
  });
  let clock = 0;
  client.send = function send(command) {
    this.sent.push(command);
    if (command.type !== 'clientVersion') return;
    this.snapshot.playerHp += 10;
    this.sequence += 1;
    this.events.push({
      sequence: this.sequence,
      direction: 'received',
      type: 'worldSnapshot',
      payload: structuredClone(this.snapshot),
    });
  };
  const moves = [];
  let sustains = 0;
  const actionOrder = [];
  const navigateNear = async target => {
    actionOrder.push('move');
    moves.push({ ...target });
    Object.assign(client.snapshot.entities[0], target);
    Object.assign(client.snapshot.entities[1], { x: target.x + 1, y: target.y });
  };

  const result = await recoverHealthWhileEvading(client, navigateNear, {
    requiredRatio: 0.75,
    timeoutMs: 10_000,
    pollMs: 1_000,
    dangerPollMs: 1,
    retreatSteps: 2,
    dangerDistance: 7,
    maxEvasiveMoves: 1,
    sustainCadenceMs: 1,
    sustain: async () => { sustains += 1; actionOrder.push('sustain'); },
    sleep: async milliseconds => { clock += milliseconds; },
    now: () => clock,
  });

  assert.equal(result.hp, 80);
  assert.equal(result.refreshes, 2);
  assert.equal(result.evasiveMoves, 2);
  assert.equal(moves.length, 2);
  assert.equal(sustains, 2);
  assert.deepEqual(actionOrder, ['move', 'sustain', 'move', 'sustain']);
  assert.equal(clock, 2);
});

test('evasive recovery spends a remaining emergency scroll when a dense pack closes again', async () => {
  const client = clientAt('D2041', 0);
  Object.assign(client.snapshot, {
    playerHp: 55,
    playerMaxHp: 100,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', x: 38, y: 98 },
      { objectId: 61, kind: 'monster', x: 39, y: 98, hp: 48 },
      { objectId: 62, kind: 'monster', x: 39, y: 99, hp: 48 },
      { objectId: 63, kind: 'monster', x: 40, y: 98, hp: 48 },
    ],
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let escapeCalls = 0;

  const result = await recoverHealthWhileEvading(client, async () => {
    throw new Error('movement should not run before the dense-pack escape');
  }, {
    requiredRatio: 0.75,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscape: async owner => {
      escapeCalls += 1;
      Object.assign(owner.snapshot.entities[0], { x: 170, y: 40 });
      owner.snapshot.playerHp = 80;
      owner.snapshot.entities.splice(1);
      return { to: { x: 170, y: 40 } };
    },
  });

  assert.equal(escapeCalls, 1);
  assert.equal(result.hp, 80);
  assert.ok(diagnostics.some(entry => entry.type === 'recoveryEmergencyEscapeAttempt'));
  assert.ok(diagnostics.some(entry => entry.type === 'recoveryEmergencyEscapeSuccess'));
});

test('evasive recovery bias keeps a dungeon retreat moving toward its transfer', async () => {
  const client = clientAt('D2041', 0);
  Object.assign(client.snapshot, {
    playerHp: 60,
    playerMaxHp: 100,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', x: 38, y: 110 },
      { objectId: 61, kind: 'monster', x: 38, y: 113, hp: 48 },
    ],
  });
  let clock = 0;
  client.send = function send(command) {
    this.sent.push(command);
    if (command.type !== 'clientVersion') return;
    this.snapshot.playerHp = 80;
    this.sequence += 1;
    this.events.push({
      sequence: this.sequence,
      direction: 'received',
      type: 'worldSnapshot',
      payload: structuredClone(this.snapshot),
    });
  };
  const moves = [];

  await recoverHealthWhileEvading(client, async target => {
    moves.push({ ...target });
    Object.assign(client.snapshot.entities[0], target);
  }, {
    requiredRatio: 0.75,
    pollMs: 1_000,
    dangerDistance: 8,
    retreatSteps: 12,
    biasPosition: { x: 262, y: 13 },
    sleep: async milliseconds => { clock += milliseconds; },
    now: () => clock,
  });

  assert.deepEqual(moves, [{ x: 50, y: 98 }]);
});

test('blocked evasive poll refreshes authoritative recovery instead of failing immediately', async () => {
  const client = clientAt('D421', 0);
  Object.assign(client.snapshot, {
    playerHp: 70,
    playerMaxHp: 100,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', x: 20, y: 20 },
      { objectId: 61, kind: 'monster', name: 'Zombie4', x: 21, y: 20, hp: 48 },
    ],
  });
  let clock = 0;
  client.send = function send(command) {
    this.sent.push(command);
    if (command.type !== 'clientVersion') return;
    this.snapshot.playerHp = 80;
    this.sequence += 1;
    this.events.push({
      sequence: this.sequence,
      direction: 'received',
      type: 'worldSnapshot',
      payload: structuredClone(this.snapshot),
    });
  };
  let attempts = 0;

  const result = await recoverHealthWhileEvading(client, async () => {
    attempts += 1;
    throw new Error('dynamic corridor occupied');
  }, {
    requiredRatio: 0.75,
    timeoutMs: 10_000,
    dangerPollMs: 250,
    retreatSteps: 6,
    sleep: async milliseconds => { clock += milliseconds; },
    now: () => clock,
  });

  assert.equal(result.hp, 80);
  assert.equal(result.refreshes, 1);
  assert.equal(result.evasiveMoves, 0);
  assert.ok(attempts > 8, 'the full bounded perimeter should be considered');
  assert.equal(clock, 250);
});

test('evasive recovery can use an off-axis endpoint in a narrow corridor', async () => {
  const client = clientAt('D421', 0);
  Object.assign(client.snapshot, {
    playerHp: 60,
    playerMaxHp: 100,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', x: 20, y: 20 },
      { objectId: 61, kind: 'monster', name: 'Zombie4', x: 21, y: 20, hp: 48 },
    ],
  });
  let clock = 0;
  client.send = function send(command) {
    this.sent.push(command);
    if (command.type !== 'clientVersion') return;
    this.snapshot.playerHp = 80;
    this.sequence += 1;
    this.events.push({
      sequence: this.sequence,
      direction: 'received',
      type: 'worldSnapshot',
      payload: structuredClone(this.snapshot),
    });
  };
  const candidates = [];
  const navigateNear = async target => {
    candidates.push({ ...target });
    const dx = Math.abs(Number(target.x) - 20);
    const dy = Math.abs(Number(target.y) - 20);
    if (Math.max(dx, dy) !== 6 || dx === 0 || dy === 0 || dx === dy) {
      throw new Error('only an off-axis corridor is open');
    }
    Object.assign(client.snapshot.entities[0], target);
  };

  const result = await recoverHealthWhileEvading(client, navigateNear, {
    requiredRatio: 0.75,
    timeoutMs: 10_000,
    pollMs: 1_000,
    retreatSteps: 6,
    sleep: async milliseconds => { clock += milliseconds; },
    now: () => clock,
  });

  assert.equal(result.hp, 80);
  assert.equal(result.evasiveMoves, 1);
  assert.ok(candidates.some(target => {
    const dx = Math.abs(Number(target.x) - 20);
    const dy = Math.abs(Number(target.y) - 20);
    return Math.max(dx, dy) === 6 && dx > 0 && dy > 0 && dx !== dy;
  }));
});

test('evasive recovery aborts a dead snapshot before navigation or sustain', async () => {
  const state = snapshot('D406', 1, 1_000);
  Object.assign(state, {
    playerObjectId: 1,
    playerHp: 0,
    playerMaxHp: 113,
    entities: [{ objectId: 1, kind: 'selfPlayer', x: 42, y: 31, hp: 0, maxHp: 113, dead: true }],
  });
  const client = { snapshot: state };
  let navigations = 0;
  let sustains = 0;
  await assert.rejects(() => recoverHealthWhileEvading(client, async () => {
    navigations += 1;
  }, {
    sustain: async () => { sustains += 1; },
  }), /Player died during quest combat/);
  assert.equal(navigations, 0);
  assert.equal(sustains, 0);
});
