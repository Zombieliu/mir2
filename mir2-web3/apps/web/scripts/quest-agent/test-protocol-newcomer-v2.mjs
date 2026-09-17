import assert from 'node:assert/strict';
import test from 'node:test';
import { firstReachableDestination } from './protocol-combat.mjs';
import {
  assertV2FlagsConfirmed, availableV2Growth, buildNewcomerV2Route,
  BICHON_SAFE_AREA, checkpointV2Progress, countBasicHpPotion, executeV2PracticePlan, hasFreshV2MapEntryReceipt, practicePlan, requiredV2Skills,
  v2CompletionState, v2FlagState,
} from './protocol-newcomer-v2.mjs';

const npcManifest = { npcs: [
  { npc_index: 3, name: 'Assistant Jane', map_file_name: '0', script_key: 'BichonProvince/BorderVillage/Jane', location: { x: 284, y: 606 } },
  { npc_index: 35, name: 'BichonWall_Board', map_file_name: '0', script_key: 'BichonProvince/BichonWall/Board', location: { x: 334, y: 259 } },
] };
const respawnManifest = { maps: [
  { map_file_name: '0', map_title: 'BichonProvince', respawns: [
    { monster_index: 39, location: { x: 110, y: 60 }, count: 10, spread: 70, respawn_index: 5 },
    { monster_index: 44, location: { x: 140, y: 100 }, count: 20, spread: 50, delay_minutes: 4, respawn_index: 9 },
  ] },
] };
const itemManifest = { items: [
  { item_index: 1, name: 'WoodenSword' }, { item_index: 2, name: '(HP)DrugSmall' },
  { item_index: 3, name: 'SharpSword' }, { item_index: 4, name: 'Slaying' }, { item_index: 5, name: '(MP)DrugSmall' },
] };
function config() {
  return {
    profile: 'newcomer-v2',
    quests: Array.from({ length: 22 }, (_, index) => ({
      id: 2110001 + index, order: index + 1, title: `Quest ${index + 1}`, minLevel: 1,
      requiredQuestId: index ? 2110000 + index : 0, afterQuestIds: [], startNpcId: index === 0 ? 3 : 0,
      finishNpcId: index === 0 ? 3 : 0, maps: ['0'], kills: index === 0 ? [{ monster: 'Scarecrow', monsterIndex: 39, count: 2 }] : [],
      flags: index === 0 ? [{ number: 2210011, message: 'Initial report', kind: 'serverEvent', conditions: ['initialNpcReport'] }] : [], rewards: { common: index === 0 ? [{ name: 'WoodenSword', count: 1 }, { name: '(HP)DrugSmall', count: 2 }] : [], wizard: index === 0 ? [{ name: '(MP)DrugSmall', count: 3 }] : [] },
    })),
    growthRewards: [15, 20, 25, 30].map((level, index) => ({ id: 2120015 + index * 5, level, afterQuestId: 2110007 + index * 5, finishNpcId: index ? 24 : 0, rewards: { common: [], warrior: index === 0 ? [{ name: 'SharpSword', count: 1 }, { name: 'Slaying', count: 1 }] : [] } })),
  };
}

test('V2 route factory produces all 22 main and 4 growth nodes from manifests', () => {
  const route = buildNewcomerV2Route({ config: config(), npcManifest, respawnManifest, itemManifest, className: 'Warrior', gender: 'Male' });
  assert.equal(route.quests.length, 26);
  assert.deepEqual(route.quests[0].startNpc, { objectId: 3, name: 'Assistant Jane', mapFileName: '0', position: { x: 284, y: 606 }, scriptKey: 'BichonProvince/BorderVillage/Jane' });
  assert.equal(route.quests[0].objectives.kill[0].spawns[0].mapFileName, '0');
  assert.deepEqual(route.quests[0].rewards.fixedItems, [{ itemIndex: 1, itemName: 'WoodenSword', count: 1 }, { itemIndex: 2, itemName: '(HP)DrugSmall', count: 2 }]);
  assert.equal(route.quests.find(quest => quest.questId === 2120020).finishNpc.objectId, 24);
});

test('V2 kill routes expose the manifest source contract consumed by ordinary multi-objective combat', async () => {
  const v2 = config();
  v2.quests[2].kills = [{ monster: 'Scarecrow', monsterIndex: 39, count: 2 }, { monster: 'RakingCat', monsterIndex: 44, count: 2 }];
  const route = buildNewcomerV2Route({ config: v2, npcManifest, respawnManifest, itemManifest, className: 'Warrior', gender: 'Male' });
  const rakingCat = route.quests.find(quest => quest.questId === 2110003)
    .objectives.kill.find(kill => kill.monsterName === 'RakingCat');
  assert.strictEqual(rakingCat.spawns, rakingCat.spawnCandidates);
  assert.deepEqual(rakingCat.spawnCandidates, [{
    mapFileName: '0', mapTitle: 'BichonProvince', position: { x: 140, y: 100 }, count: 20, spread: 50,
    delayMinutes: 4, respawnIndex: 9, isBoss: false, monsterIndex: 44, monsterName: 'RakingCat',
  }]);
  const travel = async () => {};
  travel.canReach = async mapFileName => mapFileName === '0';
  assert.equal((await firstReachableDestination(rakingCat.spawnCandidates, travel, ['0'])).mapFileName, '0');
});

test('missing manifest respawns fail closed instead of inventing a monster location', () => {
  const broken = config();
  broken.quests[0].kills[0].monsterIndex = 999;
  assert.throws(() => buildNewcomerV2Route({ config: broken, npcManifest, respawnManifest, itemManifest, className: 'Warrior', gender: 'Male' }), /has no Crystal respawn/);
});

test('disabled zero-count respawns are not promoted into an objective source', () => {
  const disabled = { maps: [{ map_file_name: '0', respawns: [{ monster_index: 39, location: { x: 110, y: 60 }, count: 0, spread: 0, respawn_index: 5 }] }] };
  assert.throws(() => buildNewcomerV2Route({ config: config(), npcManifest, respawnManifest: disabled, itemManifest, className: 'Warrior', gender: 'Male' }), /has no Crystal respawn/);
});

test('Board keeps runtime object ID 24 even though Crystal stores it as npc index 35', () => {
  const route = buildNewcomerV2Route({ config: config(), npcManifest, respawnManifest, itemManifest, className: 'Warrior', gender: 'Male' });
  const boardClaim = route.quests.find(quest => quest.questId === 2120020);
  assert.equal(boardClaim.finishNpc.objectId, 24);
  assert.equal(boardClaim.finishNpc.scriptKey, 'BichonProvince/BichonWall/Board');
  assert.deepEqual(boardClaim.finishNpc.position, { x: 334, y: 259 });
});

test('reward verification selects only the current class and resolves real item indexes', () => {
  const route = buildNewcomerV2Route({ config: config(), npcManifest, respawnManifest, itemManifest, className: 'Wizard', gender: 'Female' });
  assert.deepEqual(route.quests[0].rewards.fixedItems, [
    { itemIndex: 1, itemName: 'WoodenSword', count: 1 },
    { itemIndex: 2, itemName: '(HP)DrugSmall', count: 2 },
    { itemIndex: 5, itemName: '(MP)DrugSmall', count: 3 },
  ]);
  assert.equal(route.quests.find(quest => quest.questId === 2120015).rewards.fixedItems.length, 0);
});

test('server flag projection cannot be replaced by a local completion boolean', () => {
  const route = buildNewcomerV2Route({ config: config(), npcManifest, respawnManifest, itemManifest, className: 'Warrior', gender: 'Male' });
  const quest = route.quests[0];
  const localOnly = { questLog: [{ questId: quest.questId, objectives: [] }], v2Complete: true };
  assert.deepEqual(v2FlagState(localOnly, quest.questId, quest.objectives.flag[0]), { known: false, complete: false });
  assert.throws(() => assertV2FlagsConfirmed(localOnly, quest), /lacks server confirmation/);
  const confirmed = { questLog: [{ questId: quest.questId, objectives: [{ number: 2210011, current: 1, required: 1, done: true }] }] };
  assert.equal(assertV2FlagsConfirmed(confirmed, quest), true);
});

test('growth claims require a server Available quest and never infer readiness from level', () => {
  const route = buildNewcomerV2Route({ config: config(), npcManifest, respawnManifest, itemManifest, className: 'Warrior', gender: 'Male' });
  assert.deepEqual(availableV2Growth({ entities: [{ level: 30 }], questLog: [] }, route), []);
  assert.deepEqual(availableV2Growth({ questLog: [{ questId: 2120020, stage: 'Available' }] }, route).map(quest => quest.questId), [2120020]);
});

test('final V2 completion requires every one of 26 server Completed rows and authoritative level 30', () => {
  const route = buildNewcomerV2Route({ config: config(), npcManifest, respawnManifest, itemManifest, className: 'Warrior', gender: 'Male' });
  const snapshot = {
    playerObjectId: 1,
    entities: [{ kind: 'player', objectId: 1, level: 29 }],
    questLog: route.quests.map(quest => ({ questId: quest.questId, stage: 'Completed' })),
  };
  assert.deepEqual(v2CompletionState(snapshot, route), {
    completedQuestIds: route.quests.map(quest => quest.questId), allQuestsCompleted: true, level: 29, completed: false,
  });
  snapshot.entities[0].level = 30;
  assert.equal(v2CompletionState(snapshot, route).completed, true);
  snapshot.questLog.pop();
  assert.equal(v2CompletionState(snapshot, route).completed, false);
});

test('completed-node checkpoint retains the ordinary start and server receipt in the local QA report', async () => {
  const report = { startedAt: '2026-09-18T01:00:00.000Z' };
  const result = {
    ordinaryStartedAt: '2026-09-18T01:00:00.000Z',
    attempts: [{ questId: 2110001, completedAt: '2026-09-18T01:01:00.000Z' }],
  };
  let persisted = null;
  const wrote = await checkpointV2Progress({
    report, result, inWorldStartedAt: Date.now() - 100, startedAt: Date.now() - 60_000,
    checkpoint: async currentReport => { persisted = structuredClone(currentReport); },
  });
  assert.equal(wrote, true);
  assert.equal(persisted.v2.ordinaryStartedAt, '2026-09-18T01:00:00.000Z');
  assert.deepEqual(persisted.v2.attempts.map(attempt => [attempt.questId, attempt.completedAt]), [[2110001, '2026-09-18T01:01:00.000Z']]);
});

function practiceQuest(requirements, { kills = [{ monsterName: 'Scarecrow', spawns: [{ mapFileName: '0', position: { x: 2, y: 0 }, spread: 0 }] }] } = {}) {
  return {
    questId: 2110004,
    objectives: {
      kill: kills,
      flag: [{ number: 2210041, message: 'Class practice', conditions: [], requirements }],
    },
  };
}

function practiceClient({ knownSkills, inventoryItems = [], requirements, completeOn = 1 }) {
  const sent = [];
  const requests = [];
  const quest = practiceQuest(requirements);
  const snapshot = {
    playerObjectId: 1, mapFileName: '0', playerHp: 100, playerMaxHp: 100,
    knownSkills: knownSkills.map(spell => ({ spell })), inventoryItems, beltItems: [], equipmentItems: [],
    entities: [
      { kind: 'player', objectId: 1, x: 0, y: 0, class: 'Taoist' },
      { kind: 'monster', objectId: 9, name: 'Scarecrow', x: 1, y: 0, hp: 10, dead: false },
    ],
    questLog: [{ questId: quest.questId, objectives: [{ number: 2210041, current: 0, required: 1, done: false }] }],
  };
  let actions = 0;
  const receive = (packet, payload) => {
    client.events.push({ sequence: ++client.sequence, direction: 'received', packet, payload });
  };
  const client = {
    snapshot,
    sequence: 0,
    events: [],
    send(command) {
      sent.push(command);
      client.sequence += 1;
      actions += 1;
      if (command.type === 'attack') receive('ObjectStruck', { objectId: 9, attackerId: 1 });
      if (command.type === 'attackDirection') {
        receive('ObjectAttack', { objectId: 1, spell: command.spell });
        receive('ObjectStruck', { objectId: 9, attackerId: 1 });
      }
      if (command.type === 'magic') {
        receive('ObjectMagic', { objectId: 1, spell: command.spell, targetId: command.targetId });
        if (command.spell !== 'Healing' && command.spell !== 'Poisoning') receive('ObjectStruck', { objectId: 9, attackerId: 1 });
        if (command.spell === 'Poisoning') {
          snapshot.equipmentItems[0].quantity -= 1;
          receive('ObjectPoisoned', { objectId: 9, poison: 1 });
        }
      }
      if (actions >= completeOn) snapshot.questLog[0].objectives[0] = { number: 2210041, current: 1, required: 1, done: true };
    },
    async request(command) {
      requests.push(command);
      if (command.type === 'equipItem') {
        const item = snapshot.inventoryItems.find(candidate => Number(candidate.uniqueId) === Number(command.uniqueId));
        snapshot.equipmentItems = [{ ...item, slot: 'amulet' }];
      }
      return { payload: { success: true } };
    },
    async wait(predicate) {
      assert.equal(Boolean(predicate()), true, 'wait predicate must be backed by the updated authoritative snapshot');
      return true;
    },
  };
  return { client, quest, sent, requests };
}

test('practice plans execute every q21 class action rather than one priority spell', () => {
  const warrior = practiceQuest({ warrior: ['HalfMoon attack damage committed', 'Thrusting attack damage committed'] });
  const wizard = practiceQuest({ wizard: ['Lightning damage committed', 'owned FireWall damage committed', 'legal reposition between attacks'] });
  const taoist = practiceQuest({ taoist: ['owned skeleton damage committed', 'SoulFireBall damage committed', 'owned poison effect committed'] });
  assert.deepEqual(practicePlan({ knownSkills: [] }, warrior, 'Warrior'), [
    { kind: 'technique', spell: 'HalfMoon' }, { kind: 'technique', spell: 'Thrusting' },
  ]);
  assert.deepEqual(practicePlan({ knownSkills: [] }, wizard, 'Wizard'), [
    { kind: 'spell', spell: 'Lightning' }, { kind: 'reposition' }, { kind: 'spell', spell: 'FireWall' },
  ]);
  assert.deepEqual(practicePlan({ knownSkills: [] }, taoist, 'Taoist'), [
    { kind: 'poison' }, { kind: 'summon' }, { kind: 'spell', spell: 'SoulFireBall' },
  ]);
  assert.deepEqual(requiredV2Skills(taoist, 'Taoist'), ['SoulFireBall', 'SummonSkeleton', 'Poisoning']);
});

test('N18 loadout-only SummonSkeleton requirement learns the skill without entering owned-pet combat', () => {
  const n18 = practiceQuest({ taoist: [
    'SummonSkeleton learned',
    'level-eligible weapon equipped',
    'eligible Amulet equipped and legal poison available in bag for re-equip between casts',
  ] }, { kills: [] });
  assert.deepEqual(requiredV2Skills(n18, 'Taoist'), ['SummonSkeleton']);
  assert.deepEqual(practicePlan({ knownSkills: [] }, n18, 'Taoist'), []);
});

test('mixed map-entry flag waits for a fresh map-and-position receipt, not its unprepared AND group', () => {
  const client = {
    snapshot: {
      mapFileName: 'D401', playerObjectId: 1,
      entities: [{ kind: 'player', objectId: 1, x: 20, y: 30 }],
      questLog: [{ questId: 2110018, objectives: [{ number: 2210181, current: 0, required: 1, done: false }] }],
    },
    events: [{ sequence: 17, direction: 'received', packet: 'MapInformation', payload: { fileName: 'D401' } }],
  };
  assert.equal(hasFreshV2MapEntryReceipt(client, 16, ['D401']), true);
  // The unchanged 0/1 aggregate flag never blocks preparation after a real
  // transfer, while a snapshot without a fresh map packet remains unproven.
  assert.equal(hasFreshV2MapEntryReceipt(client, 17, ['D401']), false);
});

test('Healing executes against self at full HP only after a server objective receipt', async () => {
  const { client, quest, sent } = practiceClient({
    knownSkills: ['Healing'], requirements: { taoist: ['Healing learned', 'Healing cast accepted on self'] },
  });
  await executeV2PracticePlan({ client, quest, navigate: async () => {}, plan: practicePlan(client.snapshot, quest, 'Taoist') });
  assert.deepEqual(sent, [{ type: 'magic', objectId: 1, spell: 'Healing', direction: 'Down', targetId: 1, x: 0, y: 0, spellTargetLock: true }]);
});

test('Fencing and SpiritSword practice use normal attacks, never magic packets', async () => {
  const { client, quest, sent } = practiceClient({
    knownSkills: ['Fencing'], requirements: { warrior: ['Fencing learned', 'normal attack landed with Fencing learned'] },
  });
  client.snapshot.entities[0].class = 'Warrior';
  await executeV2PracticePlan({ client, quest, navigate: async () => {}, plan: practicePlan(client.snapshot, quest, 'Warrior') });
  assert.deepEqual(sent, [{ type: 'attack', objectId: 9 }]);
});

test('HalfMoon uses the ordinary attack-direction technique packet and Lightning self-routes toward its current target', async () => {
  const warrior = practiceClient({ knownSkills: ['HalfMoon'], requirements: { warrior: ['HalfMoon attack damage committed'] } });
  warrior.client.snapshot.entities[0].class = 'Warrior';
  await executeV2PracticePlan({ client: warrior.client, quest: warrior.quest, navigate: async () => {}, plan: [{ kind: 'technique', spell: 'HalfMoon' }] });
  assert.deepEqual(warrior.sent, [{ type: 'attackDirection', direction: 'Right', spell: 4 }]);

  const wizard = practiceClient({ knownSkills: ['Lightning'], requirements: { wizard: ['Lightning damage committed'] } });
  wizard.client.snapshot.entities[0].class = 'Wizard';
  wizard.client.snapshot.entities[0].direction = 'Left';
  await executeV2PracticePlan({ client: wizard.client, quest: wizard.quest, navigate: async () => {}, plan: [{ kind: 'spell', spell: 'Lightning' }] });
  assert.deepEqual(wizard.sent, [{ type: 'magic', objectId: 1, spell: 'Lightning', direction: 'Right', targetId: 1, x: 0, y: 0, spellTargetLock: false }]);
});

test('Poisoning equips bag poison before its public magic packet and waits for server progress', async () => {
  const poison = { uniqueId: 41, name: 'GreenPoison', quantity: 20 };
  const { client, quest, sent, requests } = practiceClient({
    knownSkills: ['Poisoning'], inventoryItems: [poison], requirements: { taoist: ['Poisoning learned', 'owned poison effect committed'] },
  });
  await executeV2PracticePlan({ client, quest, navigate: async () => {}, plan: practicePlan(client.snapshot, quest, 'Taoist') });
  assert.deepEqual(requests, [{ type: 'equipItem', grid: 'inventory', uniqueId: 41, to: 9 }]);
  assert.equal(sent[0].type, 'magic');
  assert.equal(sent[0].spell, 'Poisoning');
});

test('three independently receipted practice actions do not wait for an AND-group before its final action', async () => {
  const { client, quest, sent } = practiceClient({
    knownSkills: ['Healing'], requirements: { taoist: ['Healing cast accepted on self'] }, completeOn: 3,
  });
  await executeV2PracticePlan({
    client, quest, navigate: async () => {},
    plan: [{ kind: 'healing' }, { kind: 'healing' }, { kind: 'healing' }],
  });
  assert.equal(sent.length, 3);
  assert.equal(v2FlagState(client.snapshot, quest.questId, quest.objectives.flag[0]).complete, true);
});

test('safe-arrival uses a Bichon city safe-area coordinate and potion aliases count by item template', () => {
  assert.deepEqual(BICHON_SAFE_AREA, { x: 328, y: 264 });
  assert.ok(BICHON_SAFE_AREA.y < 400);
  assert.equal(countBasicHpPotion({ inventoryItems: [{ name: 'HP Potion', itemIndex: 658, quantity: 7 }] }), 7);
  assert.equal(countBasicHpPotion({ inventoryItems: [{ name: '(HP)DrugSmall', itemIndex: 659, quantity: 7 }] }), 0);
});
