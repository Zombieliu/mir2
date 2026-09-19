import test from 'node:test';
import assert from 'node:assert/strict';
import { claimAvailableMilestones, milestoneRewardsFor, verifyMilestoneOffer } from './protocol-milestones.mjs';

const board = { objectId: 24, name: 'BichonWall_Board', mapFileName: '0', position: { x: 334, y: 259 }, scriptKey: 'BichonProvince/BichonWall/Board' };
const items = [{ item_index: 41, name: 'Staff' }, { item_index: 42, name: 'SpellBook' }, { item_index: 43, name: 'Robe(M)' }, { item_index: 44, name: 'Robe(F)' }];
const profile = { profile: 'newcomer-v1', milestoneRewards: [
  { level: 15, classRewards: { Wizard: [{ item: 'Staff', count: 1 }, { item: 'SpellBook', count: 1 }] } },
  { level: 25, classRewards: { Wizard: [{ maleItem: 'Robe(M)', femaleItem: 'Robe(F)', count: 1 }] } },
] };
function fixture(level = 15) {
  const info = {
    index: 2100015, npc_index: 24, finish_npc_index: 24, reward_gold: 5000,
    rewards_fixed_item: [{ count: 1, item: { index: 41 } }, { count: 1, item: { index: 42 } }], rewards_select_item: [],
  };
  const client = {
    events: [{ direction: 'received', packet: 'NewQuestInfo', payload: { info } }],
    questDefinitions: new Map([[2100015, info]]),
    snapshot: { playerObjectId: 1000, entities: [{ objectId: 1000, level, class: 'Wizard', gender: 'Female' }], questLog: [], inventoryItems: [], gold: 100 },
  };
  const calls = [];
  const options = { profile, route: { quests: [{ startNpc: board }] }, items,
    travel: async map => calls.push(['travel', map]), navigate: async () => {},
    act: async (owner, quest, action) => {
      await action.verifyDefinition(owner);
      calls.push([action.type, quest.questId]);
      if (action.type === 'accept') owner.snapshot.questLog.push({ questId: quest.questId, stage: 'readyToTurnIn' });
      else {
        owner.snapshot.questLog.find(q => q.questId === quest.questId).stage = 'completed';
        owner.snapshot.gold += 5000;
        owner.snapshot.inventoryItems.push(...[41, 42].map(index => ({ key: `crystal-item-${index}`, quantity: 1 })));
      }
    },
  };
  return { client, options, calls };
}

test('milestone claims use Board accept/finish and require real item and gold deltas once', async () => {
  const { client, options, calls } = fixture();
  const result = await claimAvailableMilestones(client, options);
  assert.deepEqual(calls, [['travel', '0'], ['accept', 2100015], ['finish', 2100015]]);
  assert.equal(result[0].rewards.length, 2);
  assert.equal(result[0].gold, 5000);
  assert.deepEqual(await claimAvailableMilestones(client, options), []);
  assert.equal(calls.length, 3);
});

test('unreached milestones cause no travel or quest commands', async () => {
  const { client, options, calls } = fixture(14);
  assert.deepEqual(await claimAvailableMilestones(client, options), []);
  assert.deepEqual(calls, []);
});

test('old gold-only server offer is rejected before accepting a class milestone', async () => {
  const { client, options, calls } = fixture();
  client.questDefinitions.get(2100015).rewards_fixed_item = [];
  await assert.rejects(claimAvailableMilestones(client, options), /server milestone rewards differ/);
  assert.deepEqual(calls, [['travel', '0']]);
  assert.equal(client.snapshot.questLog.length, 0);
});

test('milestone verification uses the session definition cache after event history eviction', async () => {
  const { client, options } = fixture();
  client.events = Array.from({ length: 10_001 }, (_, sequence) => ({
    sequence,
    direction: 'received',
    type: 'worldSnapshot',
    payload: {},
  }));

  const result = await claimAvailableMilestones(client, options);

  assert.equal(result[0].questId, 2100015);
  assert.equal(client.events.some(event => event.packet === 'NewQuestInfo'), false);
});

test('newly unlocked milestone verifies the exact authoritative Board preview before its definition packet arrives', () => {
  const { client } = fixture(20);
  client.events = [];
  client.questDefinitions.clear();
  client.snapshot.questLog = [{
    questId: 2100020,
    stage: 'available',
    rewardPreview: 'Gold 10000, SpearWithHook x1',
  }];
  client.snapshot.activeNpcDialog = {
    npcObjectId: 24,
    links: [{ text: 'Accept Level 20 Growth Reward', target: '@quest:accept:2100020' }],
  };

  const expected = [{ itemIndex: 1217, itemName: 'SpearWithHook', count: 1 }];
  assert.doesNotThrow(() => verifyMilestoneOffer(client, 2100020, expected, 10000, 24));

  client.snapshot.questLog[0].rewardPreview = 'Gold 10000';
  assert.throws(
    () => verifyMilestoneOffer(client, 2100020, expected, 10000, 24),
    /definition is absent or mismatched/,
  );
});

test('milestone claim follows the real Board dialog links through interactQuest', async () => {
  const { client, options, calls } = fixture();
  delete options.act;
  client.sequence = 0;
  client.sent = [];
  client.snapshot.entities.push({
    objectId: 24, kind: 'npc', name: 'BichonWall_Board', x: 334, y: 259,
  });
  const navigated = [];
  options.navigate = async (...args) => navigated.push(args);

  function receive(snapshot) {
    client.snapshot = snapshot;
    client.events.push({
      sequence: ++client.sequence,
      direction: 'received',
      type: 'worldSnapshot',
      payload: snapshot,
    });
  }
  client.send = command => {
    client.sent.push(command);
    if (command.type === 'interact') {
      const current = client.snapshot.questLog.find(q => q.questId === 2100015);
      const target = current?.stage === 'readyToTurnIn'
        ? '@quest:finish:2100015'
        : '@quest:show:2100015';
      receive({
        ...client.snapshot,
        activeNpcDialog: { npcObjectId: 24, links: [{ text: 'Milestone', target }] },
      });
    } else if (command.type === 'selectNpcDialog') {
      receive({
        ...client.snapshot,
        activeNpcDialog: {
          npcObjectId: 24,
          links: [{ text: 'Accept', target: '@quest:accept:2100015' }],
        },
      });
    } else if (command.type === 'acceptQuest') {
      receive({
        ...client.snapshot,
        questLog: [{ questId: 2100015, stage: 'readyToTurnIn' }],
        questOperationAck: {
          operation: 'acceptQuest', requestId: command.requestId,
          npcIndex: 24, questIndex: 2100015, success: true,
        },
      });
    } else if (command.type === 'finishQuest') {
      receive({
        ...client.snapshot,
        questLog: [{ questId: 2100015, stage: 'completed' }],
        gold: client.snapshot.gold + 5000,
        inventoryItems: [
          ...client.snapshot.inventoryItems,
          ...[41, 42].map(index => ({ key: `crystal-item-${index}`, quantity: 1 })),
        ],
        questOperationAck: {
          operation: 'finishQuest', requestId: command.requestId,
          questIndex: 2100015, selectedItemIndex: -1, success: true,
        },
      });
    }
  };
  client.wait = async (predicate, label) => {
    const value = predicate();
    if (!value) throw new Error(`Fake timeout waiting for ${label}`);
    return value;
  };

  const result = await claimAvailableMilestones(client, options);

  assert.equal(result[0].questId, 2100015);
  assert.deepEqual(client.sent.map(command => command.type), [
    'interact', 'selectNpcDialog', 'acceptQuest', 'interact', 'finishQuest',
  ]);
  assert.deepEqual(navigated, [
    [{ x: 334, y: 259 }, 16],
    [{ x: 334, y: 259 }, 16],
  ]);
  assert.deepEqual(calls, [['travel', '0']]);
});

test('milestone gender comes from the authoritative actor and unknown classes fail closed', () => {
  assert.equal(milestoneRewardsFor(profile, 25, { class: 'Wizard', gender: 'Female' }, items)[0].itemIndex, 44);
  assert.equal(milestoneRewardsFor(profile, 25, { class: 'Wizard', gender: 'Male' }, items)[0].itemIndex, 43);
  assert.throws(() => milestoneRewardsFor(profile, 25, { class: 'Unknown', gender: 'Male' }, items), /Unknown milestone class/);
});

test('a completed stage without promised item delivery does not pass acceptance', async () => {
  const { client, options } = fixture();
  const act = options.act;
  options.act = async (...args) => { await act(...args); if (args[2].type === 'finish') client.snapshot.inventoryItems = []; };
  await assert.rejects(claimAvailableMilestones(client, options), /Reward missing/);
});
