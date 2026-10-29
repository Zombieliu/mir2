import assert from 'node:assert/strict';
import test from 'node:test';
import { applyProtocolObservation } from './protocol-observation.mjs';

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
  assert.equal(safeFundingVenisonTargetCount(3, {
    className: 'Taoist', requiredMpStock: 9, requiredAmuletStock: 31, additionalGold: 400,
  }), 10);
  assert.throws(() => safeFundingVenisonTargetCount(0), /positive integer/);
  assert.throws(() => safeFundingVenisonTargetCount(4, { requiredMpStock: -1 }), /nonnegative integer/);
  assert.throws(() => safeFundingVenisonTargetCount(4, { requiredHpMediumStock: -1 }), /nonnegative integer/);
  assert.throws(() => safeFundingVenisonTargetCount(4, { requiredAmuletStock: -1 }), /nonnegative integer/);
  assert.throws(() => safeFundingVenisonTargetCount(4, { additionalGold: -1 }), /nonnegative integer/);
});

test('Wizard cave funding includes the Medium reserve before buying the missing TownTeleport', () => {
  // R128: Small 59/64, MP 5/12, Medium 0/6, Random 8/8,
  // TownTeleport 1/2, and 9 gold. The old eight-Venison plan stranded the
  // player after Ruben purchased Small and Medium bottles before the scroll.
  const options = { className: 'Wizard', requiredMpStock: 7, additionalGold: 991 };
  assert.equal(safeFundingVenisonTargetCount(5, options), 8);
  const target = safeFundingVenisonTargetCount(5, { ...options, requiredHpMediumStock: 6 });
  assert.equal(target, 12);
  assert.ok(target * 200 + 9 >= 5 * 40 + 7 * 50 + 6 * 110 + 1000);
  assert.equal(safeFundingVenisonTargetCount(5, { ...options, requiredHpMediumStock: 0 }), 8);
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

function capitalFundingFixture({ count = 8, value = 250, goal = 480, requiredCount = 3, callback } = {}) {
  let corpse;
  const owner = { sequence: 0, events: [], snapshot: { mapFileName: '0', playerObjectId: 1,
    inventoryItems: [], beltItems: [], groundDrops: [], entities: [
      {objectId:1,kind:'selfPlayer',x:5,y:5,hp:60,dead:false},
      ...Array.from({length:count},(_,i)=>({objectId:20+i,kind:'monster',name:'Deer',x:6,y:5,hp:25,dead:false}))] },
    send(command) {
      this.sequence++;
      if(command.type==='harvest') { this.snapshot.groundDrops.push({objectId:100+corpse,name:'Venison',x:6,y:5,loot:{kind:'inventoryItem',name:'Venison'}});
        this.events.push({sequence:this.sequence,direction:'received',packet:'ObjectHarvested'}); }
      if(command.type==='pickUp') { this.snapshot.groundDrops=[]; this.snapshot.inventoryItems.push({name:'Venison',quantity:1,sellValue:value}); }
    },
    async wait(predicate,label) { const result=predicate();if(!result)throw new Error(`fixture did not satisfy ${label}`);return result; }
  };
  let attacks=0,calls=0;
  const options={route:{quests:[{objectives:{item:[],kill:[{monsterName:'Deer',spawnCandidates:[{mapFileName:'0',position:{x:6,y:5},count}]}]}}]},
    travel:async()=>{},navigate:async()=>{},sleep:async()=>{},requiredCount,maxHunts:8,
    clearMonster:async(_owner,target)=>{attacks++;corpse=target.objectId;Object.assign(target,{dead:true,hp:0});owner.events.push({sequence:++owner.sequence,direction:'received',packet:'ObjectDied',payload:{objectId:corpse}});},
    hasRequiredBudget:async()=>{calls++;return callback?callback(owner,calls):owner.snapshot.inventoryItems.reduce((sum,i)=>sum+i.sellValue*i.quantity,0)>=goal;}};
  return {owner,options,run:()=>collectSafeFundingVenison(owner,options),attacks:()=>attacks,calls:()=>calls};
}
test('opt-in actual capital stops after two valuable meats without claiming the estimated third',async()=>{
  const f=capitalFundingFixture();const result=await f.run();
  assert.equal(f.attacks(),2);assert.equal(result.collected,2);assert.equal(result.estimatedRequiredCount,3);
  assert.equal(result.huntAttempts,2);assert.equal(result.completionMode,'actual-capital');assert.equal(result.budgetSatisfied,true);
});
test('capital goal ignores insufficient count but never exceeds eight charged attempts',async()=>{
  const f=capitalFundingFixture({value:1});await assert.rejects(f.run(),/capital insufficient.*8 hunts and 8 attempts/);
  assert.equal(f.attacks(),8);assert.equal(f.calls(),9);
  const eighth=capitalFundingFixture({value:60});assert.equal((await eighth.run()).huntAttempts,8);
});
test('capital callback requires literal booleans and preserves fatal callback errors',async()=>{
  for(const result of [1,0,null,undefined,'true',{}]) {
    const f=capitalFundingFixture({callback:()=>result});await assert.rejects(f.run(),/literal boolean/);assert.equal(f.attacks(),0);
  }
  const original=new Error('fresh owner changed');const f=capitalFundingFixture({callback:()=>{throw original;}});
  await assert.rejects(f.run(),error=>error===original);assert.equal(f.attacks(),0);
});
test('capital selection losses still consume the unchanged eight-attempt cap',async()=>{
  const f=capitalFundingFixture();let rejected=0;
  f.options.clearMonster=async()=>{rejected++;throw new Error('target 20 left the authoritative snapshot before attack');};
  await assert.rejects(f.run(),/0 hunts and 8 attempts/);assert.equal(rejected,8);assert.equal(f.calls(),9);
});

test('default count rechecks actual inventory after the asynchronous completion boundary',async()=>{
  const f=capitalFundingFixture({requiredCount:2});delete f.options.hasRequiredBudget;
  const actualInventory=[{name:'Venison',quantity:2}];let reads=0,removed=false;
  Object.defineProperty(f.owner.snapshot,'inventoryItems',{get(){
    if(++reads===2)queueMicrotask(()=>{actualInventory.splice(0);removed=true;});
    return actualInventory;
  }});
  await assert.rejects(f.run(),/holds 0\/2 Venison after 0 hunts and 0 attempts/);
  assert.equal(removed,true);assert.equal(f.attacks(),0);assert.equal(f.calls(),0);
});

test('opt-in candidate priority preserves distance and ID ties without filtering the remaining Deer',async()=>{
  const f=capitalFundingFixture();const selected=[],evaluated=[];
  f.owner.snapshot.entities.find(e=>e.objectId===21).x=8;
  f.owner.snapshot.entities.find(e=>e.objectId===23).x=8;
  f.options.preferCandidate=candidate=>{evaluated.push(candidate.objectId);return [21,23].includes(candidate.objectId);};
  const clear=f.options.clearMonster;f.options.clearMonster=async(...args)=>{selected.push(args[1].objectId);
    Object.assign(f.owner.snapshot.entities[0],{x:args[1].x-1,y:args[1].y});return clear(...args);};
  const result=await f.run();assert.deepEqual(selected,[21,23]);assert.equal(result.huntAttempts,2);assert.equal(result.collected,2);
  assert.equal(evaluated.length,15);assert.ok(evaluated.includes(20));assert.ok(evaluated.includes(27));
});
test('no preferred candidate falls back to the old nearest order and rejected selections still exhaust the cap',async()=>{
  const f=capitalFundingFixture();const selected=[];f.options.preferCandidate=()=>false;
  f.options.clearMonster=async(_owner,target)=>{selected.push(target.objectId);throw new Error(`target ${target.objectId} left the authoritative snapshot before attack`);};
  await assert.rejects(f.run(),/0 hunts and 8 attempts/);assert.deepEqual(selected,[20,21,22,23,24,25,26,27]);assert.equal(f.calls(),9);
});
test('a selected preferred Deer lost in a race still charges its attempt and remains excluded',async()=>{
  const f=capitalFundingFixture();const selected=[];f.options.preferCandidate=()=>true;
  const clear=f.options.clearMonster;f.options.clearMonster=async(...args)=>{const id=args[1].objectId;selected.push(id);
    if(id===20)throw new Error('target 20 left the authoritative snapshot before attack');return clear(...args);};
  const result=await f.run();assert.deepEqual(selected,[20,21,22]);assert.equal(result.huntAttempts,3);assert.equal(result.hunts,2);
});
test('candidate priority requires synchronous literal booleans and preserves callback failures before selection',async()=>{
  for(const value of [1,0,null,undefined,'true',{},Promise.resolve(true)]){
    const f=capitalFundingFixture();f.options.preferCandidate=()=>value;await assert.rejects(f.run(),/literal boolean/);assert.equal(f.attacks(),0);
  }
  const original=new Error('candidate guard failed');const f=capitalFundingFixture();
  f.options.preferCandidate=candidate=>{if(candidate.objectId===21)throw original;return true;};
  await assert.rejects(f.run(),error=>error===original);assert.equal(f.attacks(),0);
});

function fundingDiscoveryFixture(limit=3) {
  const f=capitalFundingFixture({count:0,goal:250,requiredCount:1});
  f.options.refreshUnqualifiedCandidates=true;f.options.maxEmptySearchRefreshes=limit;
  f.options.route.quests[0].objectives.kill[0].spawnCandidates[0].position={x:288,y:608};
  f.inject=(id=20)=>{
    const message={type:'packet',packet:'ObjectMonster',payload:{objectId:id,name:'Deer',location:{x:6,y:5},ai:2,masterObjectId:0,dead:false}};
    f.owner.snapshot=applyProtocolObservation(f.owner.snapshot,message);
    f.owner.events.push({...message,direction:'received',sequence:++f.owner.sequence});
  };
  f.raw=(update)=>{
    const payload=structuredClone(f.owner.snapshot);update(payload);
    f.owner.snapshot=applyProtocolObservation(f.owner.snapshot,{type:'worldSnapshot',payload});++f.owner.sequence;
  };
  return f;
}
test('opt-in wire Deer without exact HP stops once and only fresh raw can permit the actual hunt',async()=>{
  const f=fundingDiscoveryFixture();let cues=0,nav=0;
  f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;nav++;f.inject();
    assert.equal(f.owner.snapshot.entities.find(e=>e.objectId===20).hp,undefined);
    assert.equal(stop(),true);assert.equal(stop(),true);};
  f.options.refreshWhileWaiting=async(_owner,cue)=>{cues++;assert.equal(cue.type,'fundingCandidateDiscovery');assert.equal(cue.objectId,20);
    assert.equal(cue.refresh,1);assert.equal(cue.cueRefresh,1);assert.equal(f.attacks(),0);
    f.raw(payload=>Object.assign(payload.entities.find(e=>e.objectId===20),{hp:25,dead:false}));};
  const result=await f.run();assert.equal(result.collected,1);assert.equal(f.attacks(),1);assert.equal(nav,1);assert.equal(cues,1);
});
test('one unknown ID across repeated getters and search passes cannot renew discovery and shares the consecutive empty quota',async()=>{
  const f=fundingDiscoveryFixture(3);let refreshes=0,cues=0;const stops=[];
  f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;f.inject();stops.push(stop());};
  f.options.refreshWhileWaiting=async(_owner,cue)=>{refreshes++;if(cue)cues++;};
  await assert.rejects(f.run(),/after 3 bounded refreshes/);
  assert.equal(cues,1);assert.equal(refreshes,3);assert.deepEqual(stops,[true,false,false]);assert.equal(f.attacks(),0);
});
test('new discovery IDs cannot exceed the original cue ceiling or obtain a second consecutive empty quota',async()=>{
  const f=fundingDiscoveryFixture(2);let id=20,cues=0,nav=0;
  f.options.route.quests[0].objectives.kill[0].spawnCandidates[0].spread=32;
  f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;nav++;f.inject(id++);stop();};
  f.options.refreshWhileWaiting=async(_owner,cue)=>{assert.ok(cue);cues++;assert.equal(cue.refresh,cues);};
  await assert.rejects(f.run(),/after 2 bounded refreshes/);assert.equal(cues,2);assert.ok(nav>2);assert.equal(f.attacks(),0);
});
test('finding a real target retains the original consecutive-counter reset while the cue ceiling stays per collection',async()=>{
  const f=fundingDiscoveryFixture(1);f.options.hasRequiredBudget=()=>f.owner.snapshot.inventoryItems.length>=2;
  let id=20,cues=0,normal=0;
  f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;f.inject(id++);stop();};
  f.options.refreshWhileWaiting=async(_owner,cue)=>{if(cue)cues++;else normal++;
    f.raw(payload=>{for(const e of payload.entities)if(e.name==='Deer'&&!e.dead)e.hp=25;});};
  const result=await f.run();assert.equal(result.hunts,2);assert.equal(cues,1);assert.equal(normal,1,'original reset allows its normal empty refresh; no second cue ceiling');
});
test('discovery rejects malformed identity dead state location and nonmissing invalid HP without inventing a live target',async t=>{
  const cases={zero:e=>e.objectId=0,malformed:e=>e.objectId='020',duplicate:(e,f)=>f.owner.snapshot.entities.push({...e,objectId:'20'}),
    wrongName:e=>e.name='deer',wrongKind:e=>e.kind='npc',dead:e=>e.dead=true,unknownDead:e=>delete e.dead,
    invalidHp:e=>e.hp=NaN,zeroHp:e=>e.hp=0,stringHp:e=>e.hp='unknown',invalidLocation:e=>e.x=NaN};
  for(const [name,change] of Object.entries(cases))await t.test(name,async()=>{
    const f=fundingDiscoveryFixture(1);let cues=0;
    f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;f.owner.snapshot.entities=f.owner.snapshot.entities.filter(e=>e.kind==='selfPlayer');
      f.inject();change(f.owner.snapshot.entities.find(e=>e.objectId===20),f);assert.equal(stop(),false);};
    f.options.refreshWhileWaiting=async(_owner,cue)=>{if(cue)cues++;};
    await assert.rejects(f.run(),/after 1 bounded refreshes/);assert.equal(cues,0);assert.equal(f.attacks(),0);
  });
});
test('fresh dead or still unknown HP after a cue never grants a hunt or retries that same cue ID',async t=>{
  for(const state of ['dead','unknown','invalid','missing'])await t.test(state,async()=>{
    const f=fundingDiscoveryFixture(1);let cues=0;
    f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;f.inject();stop();};
    f.options.refreshWhileWaiting=async()=>{cues++;f.raw(payload=>{if(state==='missing')payload.entities=payload.entities.filter(e=>e.objectId!==20);
      else Object.assign(payload.entities.find(e=>e.objectId===20),state==='dead'?{dead:true,hp:0}:state==='invalid'?{hp:NaN}:{hp:null});});};
    await assert.rejects(f.run(),/after 1 bounded refreshes/);assert.equal(cues,1);assert.equal(f.attacks(),0);
  });
});
test('a previously contested ID cannot use missing HP as a new discovery opportunity',async()=>{
  const f=fundingDiscoveryFixture(1);f.inject();f.raw(p=>p.entities.find(e=>e.objectId===20).hp=25);let cues=0,selected=0;
  f.options.clearMonster=async()=>{selected++;throw new Error('target 20 left the authoritative snapshot before attack');};
  f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;delete f.owner.snapshot.entities.find(e=>e.objectId===20).hp;assert.equal(stop(),false);};
  f.options.refreshWhileWaiting=async(_owner,cue)=>{if(cue)cues++;};
  await assert.rejects(f.run(),/after 1 bounded refreshes/);assert.equal(selected,1);assert.equal(cues,0);
});
test('discovery is strict opt-in with a required refresh adapter and default searches do not request cue refreshes',async()=>{
  for(const value of [1,0,null,'true',{}]){const f=fundingDiscoveryFixture();f.options.refreshUnqualifiedCandidates=value;await assert.rejects(f.run(),/literal boolean/);}
  const missing=fundingDiscoveryFixture();await assert.rejects(missing.run(),/requires refreshWhileWaiting/);
  const f=fundingDiscoveryFixture(1);delete f.options.refreshUnqualifiedCandidates;let cues=0;
  f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;f.inject();assert.equal(stop(),false);};
  f.options.refreshWhileWaiting=async(_owner,cue)=>{if(cue)cues++;};
  await assert.rejects(f.run(),/after 1 bounded refreshes/);assert.equal(cues,0);assert.equal(f.attacks(),0);
});
test('a signalled cue cannot swallow a navigation error and refresh errors retain their exact identity',async()=>{
  for(const at of ['navigation','refresh']){
    const f=fundingDiscoveryFixture();const original=new Error(at==='navigation'?'Navigation successful step budget exceeded (222)':'No walk path from a failing refresh');let refreshed=0;
    f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;f.inject();assert.equal(stop(),true);if(at==='navigation')throw original;};
    f.options.refreshWhileWaiting=async()=>{refreshed++;throw original;};
    await assert.rejects(f.run(),error=>error===original);assert.equal(refreshed,at==='navigation'?0:1);assert.equal(f.attacks(),0);
  }
});

test('discovery NAV classification cannot replace an original error whose message getter throws',async()=>{
  const f=fundingDiscoveryFixture();let refreshed=0;
  const original=new Error();Object.defineProperty(original,'message',{get(){throw new Error('message getter failed');}});
  f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;f.inject();assert.equal(stop(),true);throw original;};
  f.options.refreshWhileWaiting=async()=>{refreshed++;};
  await assert.rejects(f.run(),error=>error===original);assert.equal(refreshed,0);assert.equal(f.attacks(),0);
});

test('qualified-only all-unsafe candidates enter the original bounded search without selected attempts',async()=>{
  const f=capitalFundingFixture();f.options.qualifiedOnlyCandidates=true;f.options.preferCandidate=()=>false;
  f.options.route.quests[0].objectives.kill[0].spawnCandidates[0].position={x:288,y:608};
  f.options.maxEmptySearchRefreshes=1;let nav=0,refreshes=0;
  f.options.navigate=async(_point,range,stop)=>{if(range!==8)return;nav++;assert.equal(stop(),false);};
  f.options.refreshWhileWaiting=async()=>{refreshes++;};
  await assert.rejects(f.run(),/after 1 bounded refreshes/);assert.equal(f.attacks(),0);assert.equal(nav,2);assert.equal(refreshes,1);
  assert.equal(f.owner.snapshot.entities.filter(e=>e.name==='Deer').length,8);
});
test('qualified-only NAV ignores known unsafe live Deer then stops for a cue whose fresh raw becomes eligible',async()=>{
  const f=fundingDiscoveryFixture();f.inject(20);f.raw(p=>p.entities.find(e=>e.objectId===20).hp=25);
  f.options.qualifiedOnlyCandidates=true;f.options.preferCandidate=candidate=>candidate.objectId===21;
  let nav=0,refreshes=0;const selected=[];const clear=f.options.clearMonster;
  f.options.clearMonster=async(...args)=>{selected.push(args[1].objectId);return clear(...args);};
  f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;nav++;assert.equal(stop(),false);f.inject(21);assert.equal(stop(),true);};
  f.options.refreshWhileWaiting=async(_owner,cue)=>{refreshes++;assert.equal(cue.objectId,21);f.raw(p=>p.entities.find(e=>e.objectId===21).hp=25);};
  const result=await f.run();assert.equal(result.huntAttempts,1);assert.deepEqual(selected,[21]);assert.equal(nav,1);assert.equal(refreshes,1);
  assert.equal(f.owner.snapshot.entities.find(e=>e.objectId===20).dead,false,'declined candidates were never selected or excluded');
});
test('qualified-only option is strict and default preferred-all-false NAV still has its old presence behavior',async()=>{
  for(const value of [1,0,null,'true',{}]){const f=capitalFundingFixture();f.options.qualifiedOnlyCandidates=value;await assert.rejects(f.run(),/literal boolean/);}
  const missing=capitalFundingFixture();missing.options.qualifiedOnlyCandidates=true;await assert.rejects(missing.run(),/requires preferCandidate/);
  const f=fundingDiscoveryFixture(1);f.options.refreshUnqualifiedCandidates=false;f.options.preferCandidate=()=>false;let checked=false;
  f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;f.inject();f.raw(p=>p.entities.find(e=>e.objectId===20).hp=25);assert.equal(stop(),true);checked=true;};
  f.options.refreshWhileWaiting=async()=>{};
  assert.equal((await f.run()).hunts,1);assert.equal(checked,true);
});
test('qualified-only NAV callback faults stay fatal including forged NoPath text and throwing message getters',async()=>{
  for(const fault of ['NoPath','getter','number','promise']){
    const f=capitalFundingFixture();f.options.qualifiedOnlyCandidates=true;let inNav=false,nav=0;
    const original=new Error('No walk path callback guard failure');if(fault==='getter')Object.defineProperty(original,'message',{get(){throw new Error('getter failed');}});
    f.options.preferCandidate=()=>{if(!inNav)return false;if(fault==='number')return 1;if(fault==='promise')return Promise.resolve(true);throw original;};
    f.options.route.quests[0].objectives.kill[0].spawnCandidates[0].position={x:288,y:608};
    f.options.navigate=async(_target,range,stop)=>{if(range!==8)return;inNav=true;nav++;stop();};
    await assert.rejects(f.run(),error=>['number','promise'].includes(fault)?error instanceof TypeError:error===original);
    assert.equal(nav,1);assert.equal(f.attacks(),0);
  }
});
test('qualified-only selected races still consume all eight attempts and never restore an excluded ID',async()=>{
  const f=capitalFundingFixture();f.options.qualifiedOnlyCandidates=true;f.options.preferCandidate=()=>true;const selected=[];
  f.options.clearMonster=async(_owner,target)=>{selected.push(target.objectId);throw new Error(`target ${target.objectId} left the authoritative snapshot before attack`);};
  await assert.rejects(f.run(),/0 hunts and 8 attempts/);assert.deepEqual(selected,[20,21,22,23,24,25,26,27]);assert.equal(f.calls(),9);
});
test('selected target error classification cannot replace an error whose message getter throws',async()=>{
  const f=capitalFundingFixture();const original=new Error();Object.defineProperty(original,'message',{get(){throw new Error('getter failed');}});
  f.options.clearMonster=async()=>{throw original;};await assert.rejects(f.run(),error=>error===original);assert.equal(f.calls(),1);
});


test('qualified-only callback failure survives a NAV adapter wrapping it as NoPath or swallowing it',async(t)=>{
  for(const adapter of ['wrap-NoPath','fulfilled'])await t.test(adapter,async()=>{
    const f=capitalFundingFixture();f.options.qualifiedOnlyCandidates=true;
    const original=new Error('private qualification callback fault');let inNav=false,nav=0,refreshes=0,clears=0;
    f.options.preferCandidate=()=>{if(!inNav)return false;throw original;};
    f.options.route.quests[0].objectives.kill[0].spawnCandidates[0].position={x:288,y:608};
    f.options.navigate=async(_target,range,stop)=>{
      if(range!==8)return;inNav=true;nav++;
      try{stop();}catch(error){assert.equal(error,original);if(adapter==='wrap-NoPath')throw new Error('No walk path adapter wrapper',{cause:error});}
    };
    f.options.refreshWhileWaiting=async()=>{refreshes++;};
    f.options.clearMonster=async()=>{clears++;};
    await assert.rejects(f.run(),error=>error===original);
    assert.equal(nav,1,'no next waypoint');assert.equal(refreshes,0);assert.equal(clears,0);assert.equal(f.attacks(),0);
  });
});
