import assert from 'node:assert/strict';
import test from 'node:test';

import {
  canUseSafeDeerFunding,
  collectSafeFundingVenison,
  finishReadySupplyFundingQuest,
  safeFundingVenisonTargetCount,
} from './protocol-funding.mjs';

test('safe funding budget covers expedition HP and caster MP working capital', () => {
  assert.equal(safeFundingVenisonTargetCount(4, { className: 'Warrior' }), 2);
  assert.equal(safeFundingVenisonTargetCount(12, { className: 'Warrior' }), 3);
  assert.equal(safeFundingVenisonTargetCount(12, { className: 'Wizard', requiredMpStock: 12 }), 6);
  assert.equal(safeFundingVenisonTargetCount(12, { className: 'Taoist', requiredMpStock: 12 }), 6);
  assert.equal(safeFundingVenisonTargetCount(12, { className: 'Wizard', requiredMpStock: 0 }), 3);
  assert.equal(safeFundingVenisonTargetCount(24, { className: 'Warrior' }), 5);
  assert.equal(safeFundingVenisonTargetCount(24, {
    className: 'Warrior', additionalGold: 1500,
  }), 13);
  assert.throws(() => safeFundingVenisonTargetCount(0), /positive integer/);
  assert.throws(() => safeFundingVenisonTargetCount(4, { requiredMpStock: -1 }), /nonnegative integer/);
  assert.throws(() => safeFundingVenisonTargetCount(4, { additionalGold: -1 }), /nonnegative integer/);
});

test('safe Deer funding is available to any healthy living class stranded in village', () => {
  assert.equal(canUseSafeDeerFunding({ mapFileName: '0', playerHp: 39, playerMaxHp: 52 }), true);
  assert.equal(canUseSafeDeerFunding({ mapFileName: '0', playerHp: 9, playerMaxHp: 68 }), false);
  assert.equal(canUseSafeDeerFunding({ mapFileName: '2', playerHp: 52, playerMaxHp: 52 }), false);
  assert.equal(canUseSafeDeerFunding({ mapFileName: '0', playerHp: 0, playerMaxHp: 52 }), false);
});

test('turns in the first ready positive-gold quest with its class reward before restocking', async () => {
  const owner = {
    snapshot: {
      mapFileName: '0',
      gold: 27,
      questLog: [
        { questId: 30, stage: 'InProgress' },
        { questId: 33, stage: 'ReadyToTurnIn' },
      ],
      inventoryItems: [],
      beltItems: [],
      equipmentItems: [],
    },
  };
  const route = {
    classMask: 1,
    quests: [
      { questId: 30, finishNpc: { mapFileName: '0120' }, rewards: { gold: 800, selectableItems: [] } },
      {
        questId: 33,
        finishNpc: { mapFileName: '2' },
        rewards: {
          gold: 250,
          fixedItems: [],
          selectableItems: [{
            selectionIndex: 0,
            itemIndex: 1190,
            itemName: 'SolidArmour(M)',
            count: 1,
            requiredClass: 31,
            requiredGender: 1,
          }],
        },
      },
    ],
  };
  const calls = [];
  const result = await finishReadySupplyFundingQuest(owner, {
    route,
    navigate: async () => {},
    travel: async mapFileName => {
      calls.push(['travel', mapFileName]);
      owner.snapshot.mapFileName = mapFileName;
    },
    interact: async (_owner, quest, operation) => {
      calls.push(['finish', quest.questId, operation.selectedItemIndex]);
      owner.snapshot.gold += 250;
      owner.snapshot.questLog.find(entry => entry.questId === 33).stage = 'Completed';
      owner.snapshot.inventoryItems.push({ key: 'crystal-item-1190', name: 'SolidArmour(M)', quantity: 1 });
      return { accepted: true };
    },
  });

  assert.equal(result.questId, 33);
  assert.equal(result.after.gold, 277);
  assert.deepEqual(calls, [['travel', '2'], ['finish', 33, 0]]);
  assert.equal(result.itemRewards.length, 1);
  assert.equal(owner.snapshot.questLog.find(entry => entry.questId === 30).stage, 'InProgress');
});

test('does not create funding by accepting, advancing, or turning in an unavailable quest', async () => {
  const owner = {
    snapshot: {
      mapFileName: '0', gold: 27,
      questLog: [{ questId: 30, stage: 'InProgress' }],
      inventoryItems: [], beltItems: [], equipmentItems: [],
    },
  };
  let called = false;
  const result = await finishReadySupplyFundingQuest(owner, {
    route: { classMask: 1, quests: [{ questId: 30, finishNpc: { mapFileName: '0120' }, rewards: { gold: 800 } }] },
    travel: async () => { called = true; },
    navigate: async () => {},
    interact: async () => { called = true; },
  });
  assert.equal(result, null);
  assert.equal(called, false);
});

test('does not let a ready optional legacy quest take over mandatory journey recovery', async () => {
  const owner = {
    snapshot: {
      mapFileName: 'D2041', gold: 27,
      questLog: [{ questId: 62, stage: 'ReadyToTurnIn' }],
      inventoryItems: [], beltItems: [], equipmentItems: [],
    },
  };
  let called = false;
  const result = await finishReadySupplyFundingQuest(owner, {
    route: {
      classMask: 1,
      quests: [{ questId: 62, finishNpc: { mapFileName: '0' }, rewards: { gold: 800 } }],
    },
    questIds: [50, 61, 65, 60],
    travel: async () => { called = true; },
    navigate: async () => { called = true; },
    interact: async () => { called = true; },
  });
  assert.equal(result, null);
  assert.equal(called, false);
});

test('ready q110 funding turn-in keeps the guarded Sabuk endpoint on its quest-safe route', async () => {
  const actor = { objectId: 1, kind: 'selfPlayer', x: 505, y: 483 };
  const owner = {
    snapshot: {
      mapFileName: '2', playerObjectId: 1, entities: [actor], gold: 5000,
      questLog: [{ questId: 110, stage: 'ReadyToTurnIn' }],
      inventoryItems: [], beltItems: [], equipmentItems: [],
    },
  };
  const calls = [];
  const result = await finishReadySupplyFundingQuest(owner, {
    route: {
      classMask: 1,
      quests: [{
        questId: 110,
        finishNpc: { mapFileName: '3' },
        rewards: { gold: 1000, fixedItems: [], selectableItems: [] },
      }],
    },
    travel: async (mapFileName, options) => {
      calls.push(['travel', mapFileName, options]);
      owner.snapshot.mapFileName = mapFileName;
      Object.assign(actor, mapFileName === 'D701'
        ? { x: 28, y: 22 }
        : options?.preferDirectScriptedEdge
          ? { x: 361, y: 342 }
          : { x: 660, y: 276 });
    },
    navigate: async (target, distance) => {
      calls.push(['navigate', target, distance]);
      Object.assign(actor, target);
    },
    interact: async () => {
      owner.snapshot.gold += 1000;
      owner.snapshot.questLog[0].stage = 'Completed';
      return { accepted: true };
    },
  });

  assert.equal(result.questId, 110);
  assert.deepEqual(calls, [
    ['travel', '3', { preferDirectScriptedEdge: true }],
    ['travel', 'D701', { preferredTransferSource: { x: 564, y: 287 } }],
    ['travel', '3', { preferredTransferSource: { x: 171, y: 132 } }],
  ]);
});

test('bankrupt newcomer hunts and harvests passive Deer through normal protocol commands', async () => {
  const owner = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0',
      playerObjectId: 1,
      inventoryItems: [],
      beltItems: [],
      groundDrops: [],
      entities: [
        { objectId: 1, kind: 'selfPlayer', x: 5, y: 5, hp: 60, dead: false },
        { objectId: 20, kind: 'monster', name: 'Deer', x: 6, y: 5, hp: 25, dead: false },
        { objectId: 21, kind: 'monster', name: 'Deer', x: 5, y: 6, hp: 25, dead: false },
      ],
    },
    send(command) {
      this.sequence += 1;
      if (command.type === 'harvest') {
        this.snapshot.groundDrops.push({
          objectId: 100 + currentCorpseId,
          name: 'Venison',
          x: 6,
          y: 5,
          loot: { kind: 'inventoryItem', name: 'Venison' },
        });
        this.events.push({
          sequence: this.sequence,
          direction: 'received',
          packet: 'ObjectHarvested',
        });
      }
      if (command.type === 'pickUp') {
        this.snapshot.groundDrops = this.snapshot.groundDrops.filter(drop =>
          Number(drop.objectId) !== Number(command.objectId));
        this.snapshot.inventoryItems.push({ name: 'Venison', quantity: 1, sellValue: 250 });
      }
    },
    async wait(predicate, label) {
      const value = predicate();
      if (!value) throw new Error(`fixture did not satisfy ${label}`);
      return value;
    },
  };
  let currentCorpseId = null;
  const calls = [];
  const refreshWhileWaiting = async () => {};
  const result = await collectSafeFundingVenison(owner, {
    route: {
      quests: [{ objectives: { item: [], kill: [{
        monsterName: 'Deer',
        spawnCandidates: [{ mapFileName: '0', position: { x: 6, y: 5 }, count: 2 }],
      }] } }],
    },
    travel: async mapFileName => { calls.push(['travel', mapFileName]); },
    navigate: async target => { calls.push(['navigate', target.objectId ?? target.x, target.y]); },
    clearMonster: async (_owner, target, _navigate, options) => {
      currentCorpseId = Number(target.objectId);
      assert.equal(options.approachRange, 1);
      const attack = await options.action(owner, target);
      assert.deepEqual(attack, {
        kind: 'attack',
        targetId: currentCorpseId,
        command: { type: 'attack', objectId: currentCorpseId },
      });
      Object.assign(target, { dead: true, hp: 0 });
      owner.events.push({
        sequence: ++owner.sequence, direction: 'received', packet: 'ObjectDied',
        payload: { objectId: currentCorpseId },
      });
      assert.equal(options.refreshWhileWaiting, refreshWhileWaiting);
      calls.push(['clear', currentCorpseId]);
      return { objectId: currentCorpseId, cleared: true };
    },
    action: async () => { throw new Error('generic class magic must not be used for passive Deer'); },
    approachRange: 9,
    refreshWhileWaiting,
    requiredCount: 2,
    sleep: async () => {},
  });

  assert.deepEqual(result, {
    method: 'DeerVenison', collected: 2, hunts: 2, beforeCount: 0, afterCount: 2,
  });
  assert.deepEqual(calls.filter(call => call[0] === 'clear'), [['clear', 20], ['clear', 21]]);
  assert.equal(owner.snapshot.inventoryItems.length, 2);
});

test('safe funding waits through a bounded empty field until a Deer respawns', async () => {
  let corpseId = null;
  let refreshes = 0;
  const diagnostics = [];
  const owner = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0', playerObjectId: 1, inventoryItems: [], beltItems: [], groundDrops: [],
      entities: [{ objectId: 1, kind: 'selfPlayer', x: 5, y: 5, hp: 60, dead: false }],
    },
    send(command) {
      if (command.type === 'harvest') {
        this.snapshot.groundDrops.push({
          objectId: 120, name: 'Venison', x: 6, y: 5,
          loot: { kind: 'inventoryItem', name: 'Venison' },
        });
      }
      if (command.type === 'pickUp') {
        this.snapshot.groundDrops = [];
        this.snapshot.inventoryItems.push({ name: 'Venison', quantity: 1, sellValue: 250 });
      }
    },
    record(_direction, payload) { diagnostics.push(payload); },
    async wait(predicate, label) {
      const value = predicate();
      if (!value) throw new Error(`fixture did not satisfy ${label}`);
      return value;
    },
  };

  const result = await collectSafeFundingVenison(owner, {
    route: { quests: [{ objectives: { item: [], kill: [{
      monsterName: 'Deer',
      spawnCandidates: [{ mapFileName: '0', position: { x: 6, y: 5 }, count: 1 }],
    }] } }] },
    travel: async () => {},
    navigate: async () => {},
    refreshWhileWaiting: async () => {
      refreshes += 1;
      if (refreshes === 2) {
        owner.snapshot.entities.push({
          objectId: 20, kind: 'monster', name: 'Deer', x: 6, y: 5, hp: 25, dead: false,
        });
      }
    },
    clearMonster: async (_owner, target) => {
      corpseId = Number(target.objectId);
      Object.assign(target, { dead: true, hp: 0 });
      owner.events.push({
        sequence: ++owner.sequence, direction: 'received', packet: 'ObjectDied',
        payload: { objectId: corpseId },
      });
      return { objectId: corpseId, cleared: true };
    },
    requiredCount: 1,
    maxEmptySearchRefreshes: 3,
    sleep: async () => {},
  });

  assert.equal(result.collected, 1);
  assert.equal(refreshes, 2);
  assert.equal(diagnostics.filter(entry => entry.type === 'safeSupplyFundingAwaitRespawn').length, 2);
});

test('safe funding retries a Deer contested by another player within the hunt budget', async () => {
  let currentCorpseId = null;
  const diagnostics = [];
  const owner = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0', playerObjectId: 1, inventoryItems: [], beltItems: [], groundDrops: [],
      entities: [
        { objectId: 1, kind: 'selfPlayer', x: 5, y: 5, hp: 60, dead: false },
        { objectId: 20, kind: 'monster', name: 'Deer', x: 6, y: 5, hp: 25, dead: false },
        { objectId: 21, kind: 'monster', name: 'Deer', x: 5, y: 6, hp: 25, dead: false },
      ],
    },
    send(command) {
      this.sequence += 1;
      if (command.type === 'harvest') {
        this.snapshot.groundDrops.push({
          objectId: 121, name: 'Venison', x: 5, y: 6,
          loot: { kind: 'inventoryItem', name: 'Venison' },
        });
        this.events.push({ sequence: this.sequence, direction: 'received', packet: 'ObjectHarvested' });
      }
      if (command.type === 'pickUp') {
        this.snapshot.groundDrops = [];
        this.snapshot.inventoryItems.push({ name: 'Venison', quantity: 1, sellValue: 250 });
      }
    },
    record(_direction, payload) { diagnostics.push(payload); },
    async wait(predicate, label) {
      const value = predicate();
      if (!value) throw new Error(`fixture did not satisfy ${label}`);
      return value;
    },
  };
  const cleared = [];
  const result = await collectSafeFundingVenison(owner, {
    route: { quests: [{ objectives: { item: [], kill: [{
      monsterName: 'Deer',
      spawnCandidates: [{ mapFileName: '0', position: { x: 6, y: 5 }, count: 2 }],
    }] } }] },
    travel: async () => {},
    navigate: async () => {},
    clearMonster: async (_owner, target) => {
      cleared.push(Number(target.objectId));
      if (Number(target.objectId) === 20) {
        owner.snapshot.entities = owner.snapshot.entities.filter(entry => entry.objectId !== 20);
        throw new Error('target 20 left the authoritative snapshot before death was confirmed');
      }
      currentCorpseId = Number(target.objectId);
      Object.assign(target, { dead: true, hp: 0 });
      owner.events.push({
        sequence: ++owner.sequence, direction: 'received', packet: 'ObjectDied',
        payload: { objectId: currentCorpseId },
      });
      return { objectId: currentCorpseId, cleared: true };
    },
    requiredCount: 1,
    maxHunts: 2,
    sleep: async () => {},
  });

  assert.deepEqual(cleared, [20, 21]);
  assert.equal(result.hunts, 1);
  assert.equal(result.collected, 1);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'safeSupplyFundingTargetContested' && entry.objectId === 20));
});

test('safe funding quarantines moving Deer that stall or become unreachable', async () => {
  const diagnostics = [];
  const owner = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0', playerObjectId: 1, inventoryItems: [], beltItems: [], groundDrops: [],
      entities: [
        { objectId: 1, kind: 'selfPlayer', x: 5, y: 5, hp: 60, dead: false },
        { objectId: 20, kind: 'monster', name: 'Deer', x: 6, y: 5, hp: 25, dead: false },
        { objectId: 21, kind: 'monster', name: 'Deer', x: 7, y: 5, hp: 25, dead: false },
        { objectId: 22, kind: 'monster', name: 'Deer', x: 8, y: 5, hp: 25, dead: false },
      ],
    },
    send(command) {
      this.sequence += 1;
      if (command.type === 'harvest') {
        this.snapshot.inventoryItems.push({ name: 'Venison', quantity: 1 });
      }
    },
    record(_direction, payload) { diagnostics.push(payload); },
    async wait(predicate, label) {
      const value = predicate();
      if (!value) throw new Error(`fixture did not satisfy ${label}`);
      return value;
    },
  };
  const cleared = [];

  const result = await collectSafeFundingVenison(owner, {
    route: { quests: [{ objectives: { item: [], kill: [{
      monsterName: 'Deer',
      spawnCandidates: [{ mapFileName: '0', position: { x: 6, y: 5 }, count: 2 }],
    }] } }] },
    travel: async () => {},
    navigate: async target => {
      Object.assign(owner.snapshot.entities[0], { x: Number(target.x) - 1, y: Number(target.y) });
    },
    clearMonster: async (_owner, target, _navigate, options) => {
      cleared.push(Number(target.objectId));
      assert.equal(options.maxMovingTargetNoProgressMs, 20_000);
      if (Number(target.objectId) === 20) {
        throw new Error('target 20 made no authoritative combat progress after 12 attacks');
      }
      if (Number(target.objectId) === 21) {
        throw new Error('target 21 has no walk path from the current map region');
      }
      Object.assign(target, { dead: true, hp: 0 });
      owner.events.push({
        sequence: ++owner.sequence, direction: 'received', packet: 'ObjectDied',
        payload: { objectId: Number(target.objectId) },
      });
      return { objectId: Number(target.objectId), cleared: true };
    },
    requiredCount: 1,
    maxHunts: 3,
    sleep: async () => {},
  });

  assert.deepEqual(cleared, [20, 21, 22]);
  assert.equal(result.collected, 1);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'safeSupplyFundingTargetContested' && entry.objectId === 20));
  assert.ok(diagnostics.some(entry =>
    entry.type === 'safeSupplyFundingTargetContested' && entry.objectId === 21));
});

test('safe funding waits for the exact ObjectDied before harvesting a snapshot-dead Deer', async () => {
  const order = [];
  const owner = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0', playerObjectId: 1, inventoryItems: [], beltItems: [], groundDrops: [],
      entities: [
        { objectId: 1, kind: 'selfPlayer', x: 5, y: 5, hp: 60, dead: false },
        { objectId: 20, kind: 'monster', name: 'Deer', x: 6, y: 5, hp: 25, dead: false },
      ],
    },
    send(command) {
      this.sequence += 1;
      if (command.type === 'harvest') {
        assert.ok(this.events.some(event => event.packet === 'ObjectDied' && event.payload.objectId === 20));
        order.push('harvest');
        this.snapshot.inventoryItems.push({ name: 'Venison', quantity: 1 });
      }
    },
    async wait(predicate, label) {
      if (label.includes('authoritative death')) {
        order.push('death');
        this.events.push({
          sequence: ++this.sequence, direction: 'received', packet: 'ObjectDied',
          payload: { objectId: 20 },
        });
      }
      const value = predicate();
      if (!value) throw new Error(`fixture did not satisfy ${label}`);
      return value;
    },
  };

  const result = await collectSafeFundingVenison(owner, {
    route: { quests: [{ objectives: { item: [], kill: [{
      monsterName: 'Deer', spawnCandidates: [{ mapFileName: '0', position: { x: 6, y: 5 } }],
    }] } }] },
    travel: async () => {},
    navigate: async () => {},
    clearMonster: async (_owner, target) => {
      Object.assign(target, { dead: true, hp: 0 });
      return { objectId: 20, cleared: true };
    },
    requiredCount: 1,
    sleep: async () => {},
  });

  assert.deepEqual(order, ['death', 'harvest']);
  assert.equal(result.collected, 1);
});

test('safe funding abandons a harvested corpse with no Venison award and tries another Deer', async () => {
  let currentCorpseId = null;
  const owner = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0', playerObjectId: 1, inventoryItems: [], beltItems: [], groundDrops: [],
      entities: [
        { objectId: 1, kind: 'selfPlayer', x: 5, y: 5, hp: 60, dead: false },
        { objectId: 20, kind: 'monster', name: 'Deer', x: 6, y: 5, hp: 25, dead: false },
        { objectId: 21, kind: 'monster', name: 'Deer', x: 5, y: 6, hp: 25, dead: false },
      ],
    },
    send(command) {
      this.sequence += 1;
      if (command.type !== 'harvest') return;
      if (currentCorpseId === 20) {
        this.events.push({
          sequence: this.sequence, direction: 'received', packet: 'ObjectHarvest',
          payload: { objectId: 20 },
        });
      } else {
        this.snapshot.inventoryItems.push({ name: 'Venison', quantity: 1 });
      }
    },
    async wait(predicate, label) {
      const value = predicate();
      if (!value && label.includes('funding harvest') && currentCorpseId === 20) {
        throw new Error(`Timeout waiting for ${label}`);
      }
      if (!value) throw new Error(`fixture did not satisfy ${label}`);
      return value;
    },
  };
  const cleared = [];

  const result = await collectSafeFundingVenison(owner, {
    route: { quests: [{ objectives: { item: [], kill: [{
      monsterName: 'Deer', spawnCandidates: [{ mapFileName: '0', position: { x: 6, y: 5 }, count: 2 }],
    }] } }] },
    travel: async () => {},
    navigate: async () => {},
    clearMonster: async (_owner, target) => {
      currentCorpseId = Number(target.objectId);
      cleared.push(currentCorpseId);
      Object.assign(target, { dead: true, hp: 0 });
      owner.events.push({
        sequence: ++owner.sequence, direction: 'received', packet: 'ObjectDied',
        payload: { objectId: currentCorpseId },
      });
      return { objectId: currentCorpseId, cleared: true };
    },
    requiredCount: 1,
    maxHunts: 2,
    sleep: async () => {},
  });

  assert.deepEqual(cleared, [20, 21]);
  assert.equal(result.collected, 1);
  assert.equal(result.hunts, 2);
});

test('safe funding budgets a long map-0 entrance route from authoritative distance', async () => {
  let currentCorpseId = null;
  const owner = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0', playerObjectId: 1, inventoryItems: [], beltItems: [], groundDrops: [],
      entities: [{ objectId: 1, kind: 'selfPlayer', x: 663, y: 92, hp: 68, dead: false }],
    },
    send(command) {
      this.sequence += 1;
      if (command.type === 'harvest') {
        this.snapshot.groundDrops.push({
          objectId: 120, name: 'Venison', x: 271, y: 625,
          loot: { kind: 'inventoryItem', name: 'Venison' },
        });
        this.events.push({ sequence: this.sequence, direction: 'received', packet: 'ObjectHarvested' });
      }
      if (command.type === 'pickUp') {
        this.snapshot.groundDrops = [];
        this.snapshot.inventoryItems.push({ name: 'Venison', quantity: 1, sellValue: 250 });
      }
    },
    async wait(predicate, label) {
      const value = predicate();
      if (!value) throw new Error(`fixture did not satisfy ${label}`);
      return value;
    },
  };
  let searchBudget = null;
  let successfulStepBudget = null;
  const searched = [];
  const result = await collectSafeFundingVenison(owner, {
    route: { quests: [{ objectives: { item: [], kill: [{
      monsterName: 'Deer',
      spawnCandidates: [
        { mapFileName: '0', position: { x: 650, y: 90 }, count: 1 },
        { mapFileName: '0', position: { x: 270, y: 625 }, count: 50 },
      ],
    }] } }] },
    travel: async () => {},
    navigate: async (target, _range, _stopWhen, options) => {
      if (options?.maxAttempts) {
        searched.push({ ...target });
        if (target.x === 650) throw new Error('No walk path on 0 from 663,92 to 650,90');
        searchBudget = options.maxAttempts;
        successfulStepBudget = options.maxSuccessfulSteps;
        Object.assign(owner.snapshot.entities[0], { x: 270, y: 625 });
        owner.snapshot.entities.push({
          objectId: 20, kind: 'monster', name: 'Deer', x: 271, y: 625, hp: 25, dead: false,
        });
      }
    },
    clearMonster: async (_owner, target) => {
      currentCorpseId = Number(target.objectId);
      Object.assign(target, { dead: true, hp: 0 });
      owner.events.push({
        sequence: ++owner.sequence, direction: 'received', packet: 'ObjectDied',
        payload: { objectId: currentCorpseId },
      });
      return { objectId: currentCorpseId, cleared: true };
    },
    requiredCount: 1,
    sleep: async () => {},
  });

  assert.equal(searchBudget, 2048);
  assert.equal(successfulStepBudget, 1258);
  assert.deepEqual(searched, [{ x: 270, y: 625 }]);
  assert.equal(result.collected, 1);
  assert.equal(result.hunts, 1);
});

test('safe funding scans the village Deer footprint without blindly walking to remote static spawns', async () => {
  const owner = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0', playerObjectId: 1, inventoryItems: [], beltItems: [], groundDrops: [],
      entities: [{ objectId: 1, kind: 'selfPlayer', x: 288, y: 608, hp: 68, dead: false }],
    },
    send() {},
    record() {},
    async wait(predicate) { return predicate(); },
  };
  const searched = [];
  await assert.rejects(() => collectSafeFundingVenison(owner, {
    route: { quests: [{ objectives: { item: [], kill: [{
      monsterName: 'Deer',
      spawnCandidates: [
        { mapFileName: '0', position: { x: 270, y: 625 }, spread: 50, count: 50 },
        { mapFileName: '0', position: { x: 260, y: 380 }, spread: 50, count: 30 },
      ],
    }] } }] },
    travel: async () => {},
    navigate: async target => { searched.push({ x: target.x, y: target.y }); },
    refreshWhileWaiting: async () => {},
    requiredCount: 1,
    maxEmptySearchRefreshes: 1,
    sleep: async () => {},
  }), /found no live Deer/);

  assert.ok(searched.length > 1);
  assert.ok(searched.every(point =>
    Math.max(Math.abs(point.x - 288), Math.abs(point.y - 608)) <= 120));
  assert.equal(searched.some(point => point.y <= 430), false);
});

test('safe funding reuses already-held Venison without repeating the hunt', async () => {
  const owner = {
    snapshot: {
      mapFileName: '0', playerObjectId: 1, beltItems: [], groundDrops: [],
      inventoryItems: [
        { name: 'Venison', quantity: 1 },
        { name: 'Venison', quantity: 1 },
      ],
      entities: [{ objectId: 1, kind: 'selfPlayer', x: 290, y: 608 }],
    },
    send() {},
    async wait(predicate) { return predicate(); },
  };
  let cleared = false;
  const result = await collectSafeFundingVenison(owner, {
    route: { quests: [{ objectives: { item: [], kill: [{
      monsterName: 'Deer',
      spawnCandidates: [{ mapFileName: '0', position: { x: 270, y: 625 }, count: 50 }],
    }] } }] },
    travel: async () => {},
    navigate: async () => {},
    clearMonster: async () => { cleared = true; },
    requiredCount: 2,
  });

  assert.equal(cleared, false);
  assert.deepEqual(result, {
    method: 'DeerVenison', collected: 0, hunts: 0, beforeCount: 2, afterCount: 2,
  });
});
