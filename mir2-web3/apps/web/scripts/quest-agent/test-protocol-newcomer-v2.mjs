import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { firstReachableDestination } from './protocol-combat.mjs';
import {
  assertV2FlagsConfirmed, availableV2Growth, buildNewcomerV2Route,
  approachPracticeTarget, BICHON_SAFE_AREA, checkpointV2Progress, countBasicHpPotion, executeV2PracticePlan, hasFreshV2MapEntryReceipt,
  completeV2Objectives, directionalApproachPoints, directionalRayDistance, loadNewcomerV2Route, practicePlan, practiceSpawnCandidates, practiceSpawnWaypoints, purchaseRequiredBasicPotion, requiredV2Skills,
  recoverV2Death, runNewcomerV2Journey, v2CompletionState, v2FlagState,
} from './protocol-newcomer-v2.mjs';
import { parseV2RecoveryLedger } from './newcomer-v2-recovery-ledger.mjs';

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

test('real V2 factory keeps class training separate from the other class requirements', async () => {
  for (const [className, expected] of [['Warrior', ['Fencing']], ['Wizard', ['FireBall']], ['Taoist', ['Healing']]]) {
    const route = await loadNewcomerV2Route({ className, gender: 'Male' });
    assert.deepEqual(requiredV2Skills(route.quests.find(row => row.questId === 2110004), className), expected);
  }
});
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

test('V2 Basic Potion condition uses the nearby live Samuel shop without adding MP or Amulets', async () => {
  const sent = [];
  const client = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0', gold: 100, playerObjectId: 1,
      entities: [
        { objectId: 1, kind: 'player', x: 328, y: 264 },
        { objectId: 205, kind: 'npc', name: 'Alchemist_Samuel', x: 324, y: 291 },
      ],
      inventoryItems: [], beltItems: [], equipmentItems: [], activeNpcDialog: null,
    },
    receive(next) {
      this.snapshot = next;
      this.events.push({ sequence: ++this.sequence, direction: 'received', type: 'worldSnapshot', payload: structuredClone(next) });
    },
    send(command) {
      sent.push(structuredClone(command));
      if (command.type === 'interact') {
        this.receive({ ...this.snapshot, activeNpcDialog: { npcObjectId: command.objectId, links: [{ text: 'Buy and sell', target: '@BuySell' }] } });
      }
      if (command.type === 'buyItem') {
        const next = structuredClone(this.snapshot);
        next.gold -= 40;
        next.inventoryItems.push({ name: '(HP)DrugSmall', quantity: 1, tooltipSource: { info: { item_index: 658 } } });
        this.receive(next);
      }
    },
    async request(command, packet) {
      sent.push(structuredClone(command));
      assert.equal(packet, 'NPCGoods');
      return { packet, payload: { panelType: 0, list: [{ id: 92005, itemIndex: 658, name: '(HP)DrugSmall', price: 40, count: 1 }] } };
    },
    async wait(predicate, label) {
      const value = predicate();
      assert.ok(value, `authoritative ${label} predicate`);
      return value;
    },
  };
  const navigations = [];
  await purchaseRequiredBasicPotion(client, async (...args) => navigations.push(args), { questId: 2110006 }, () => {});
  assert.deepEqual(navigations, [[{ x: 324, y: 291 }, 1]]);
  assert.deepEqual(sent, [
    { type: 'interact', objectId: 205 },
    { type: 'selectNpcDialog', target: '@BuySell' },
    { type: 'buyItem', itemIndex: 92005, count: 1, panelType: 0 },
  ]);
  assert.equal(countBasicHpPotion(client.snapshot), 1);
  assert.equal(client.snapshot.inventoryItems.some(item => Number(item.tooltipSource?.info?.item_index) === 659 || item.name === 'Amulet'), false);
});

test('V2 Wizard combat refreshes a stale cooldown snapshot and then casts normally', async () => {
  const quest = {
    questId: 2110007,
    objectiveMaps: ['0'],
    objectives: {
      kill: [{ monsterName: 'Scarecrow', spawnCandidates: [{ mapFileName: '0', position: { x: 2, y: 0 }, spread: 0, count: 1 }] }],
      item: [], flag: [],
    },
  };
  const sent = [];
  const client = {
    sequence: 0,
    events: [],
    snapshot: {
      mapFileName: '0', playerObjectId: 1, playerHp: 100, playerMaxHp: 100, playerMp: 30, playerMaxMp: 30,
      entities: [
        { objectId: 1, kind: 'player', class: 'Wizard', level: 8, x: 0, y: 0, hp: 100, maxHp: 100 },
        { objectId: 9, kind: 'monster', name: 'Scarecrow', x: 2, y: 0, hp: 10, dead: false },
      ],
      knownSkills: [{ spell: 'FireBall', mpCost: 4, cooldownRemainingTicks: 1 }],
      inventoryItems: [], beltItems: [], equipmentItems: [],
      questLog: [{ questId: 2110007, stage: 'InProgress', objectives: [{ label: 'Scarecrow', current: 0, required: 1, done: false }] }],
    },
    receive(next) {
      this.snapshot = next;
      this.events.push({ sequence: ++this.sequence, direction: 'received', type: 'worldSnapshot', payload: structuredClone(next) });
    },
    send(command) {
      sent.push(structuredClone(command));
      const next = structuredClone(this.snapshot);
      if (command.type === 'clientVersion') {
        next.knownSkills[0].cooldownRemainingTicks = 0;
      }
      if (command.type === 'magic') {
        next.entities[1].hp = 0;
        next.entities[1].dead = true;
        next.questLog[0].objectives[0] = { label: 'Scarecrow', current: 1, required: 1, done: true };
        next.questLog[0].stage = 'ReadyToTurnIn';
      }
      this.receive(next);
    },
    async wait(predicate, label) {
      const value = predicate();
      assert.ok(value, `authoritative ${label} predicate`);
      return value;
    },
  };
  await completeV2Objectives(client, quest, {
    navigate: async point => { client.snapshot.entities[0].x = Number(point.x) - 1; },
    travel: async () => {}, className: 'Wizard', checkDeadline: () => {},
  });
  assert.equal(sent[0].type, 'clientVersion');
  assert.equal(sent[1].type, 'magic');
  assert.equal(sent[1].spell, 'FireBall');
  assert.equal(client.snapshot.questLog[0].stage, 'ReadyToTurnIn');
});

test('V2 objective combat explicitly opts into the bounded timed-search respawn observation', async () => {
  const source = await readFile(new URL('./protocol-newcomer-v2.mjs', import.meta.url), 'utf8');
  const start = source.indexOf('export async function completeV2Objectives');
  const end = source.indexOf('\nasync function v2Sustain', start);
  assert.ok(start >= 0 && end > start, 'V2 objective combat block');
  const options = source.slice(start, end);
  assert.match(options, /retrySpawnSearchTimeout:\s*true/);
  assert.match(options, /allowHistoricalSpawnHints:\s*false/);
  assert.match(options, /spawnSearchTimeoutMs:\s*30_000/);
  assert.match(options, /maxSpawnWaypoints:\s*120/);
  assert.match(options, /maxSpawnRespawnWaits:\s*1/);
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
  const report = { startedAt: '2026-09-18T01:00:00.000Z', v2: { status: 'paused', finishedAt: 'earlier-process' } };
  const result = {
    status: 'running',
    ordinaryStartedAt: '2026-09-18T01:00:00.000Z',
    attempts: [{ questId: 2110001, completedAt: '2026-09-18T01:01:00.000Z' }],
  };
  let persisted = null;
  const wrote = await checkpointV2Progress({
    report, result, inWorldStartedAt: Date.now() - 100, startedAt: Date.now() - 60_000,
    checkpoint: async currentReport => { persisted = structuredClone(currentReport); },
  });
  assert.equal(wrote, true);
  assert.equal(persisted.v2.status, 'running');
  assert.equal(persisted.v2.ordinaryStartedAt, '2026-09-18T01:00:00.000Z');
  assert.deepEqual(persisted.v2.attempts.map(attempt => [attempt.questId, attempt.completedAt]), [[2110001, '2026-09-18T01:01:00.000Z']]);
});

test('authoritative V2 death is revived once, checkpointed, and leaves the unfinished server quest unchanged', async () => {
  const snapshot = {
    playerObjectId: 1, mapFileName: 'D001', playerHp: 0,
    entities: [{ kind: 'player', objectId: 1, x: 25, y: 30, hp: 0, dead: true }],
    questLog: [{ questId: 2110010, stage: 'InProgress' }],
  };
  const client = { snapshot };
  const result = { status: 'running', attempts: [{ questId: 2110010 }], recoveries: [] };
  const report = {};
  const writes = [];
  const recovered = await recoverV2Death({
    client, result, report, questId: 2110010, phase: 'quest',
    checkpoint: async current => writes.push(structuredClone(current.v2.recoveries)),
    inWorldStartedAt: Date.now() - 20, startedAt: Date.now() - 100,
    recovery: {
      maxRecoveries: 3,
      isDead: current => current.playerHp <= 0,
      revive: async current => {
        current.snapshot.playerHp = 40;
        Object.assign(current.snapshot.entities[0], { hp: 40, dead: false, x: 328, y: 264 });
        current.snapshot.mapFileName = '0';
        return { at: 'revived', before: { map: 'D001' }, after: { map: '0' } };
      },
    },
  });
  assert.equal(recovered, true);
  assert.equal(result.recoveries.length, 1);
  assert.equal(result.recoveries[0].questId, 2110010);
  assert.equal(result.recoveries[0].phase, 'quest');
  assert.equal(result.recoveries[0].revive.after.map, '0');
  assert.equal(writes.length, 2, 'death and confirmed revive each persist local QA evidence');
  assert.deepEqual(snapshot.questLog, [{ questId: 2110010, stage: 'InProgress' }], 'recovery cannot complete or advance a quest locally');
});

test('a living V2 client never sends a revive request or consumes a recovery slot', async () => {
  const client = { snapshot: { playerHp: 30, entities: [{ objectId: 1, hp: 30, dead: false }] } };
  const result = { recoveries: [] };
  let reviveCalls = 0;
  const recovered = await recoverV2Death({
    client, result, inWorldStartedAt: Date.now(), startedAt: Date.now(),
    recovery: { isDead: current => current.playerHp <= 0, revive: async () => { reviveCalls += 1; } },
  });
  assert.equal(recovered, false);
  assert.equal(reviveCalls, 0);
  assert.deepEqual(result.recoveries, []);
});

test('V2 recovery fails closed after three confirmed deaths in one ordinary process', async () => {
  const client = { snapshot: { playerHp: 0, entities: [{ objectId: 1, hp: 0, dead: true }] } };
  const result = { recoveries: [] };
  const recovery = {
    maxRecoveries: 3,
    isDead: current => current.playerHp <= 0,
    revive: async current => {
      current.snapshot.playerHp = 10;
      Object.assign(current.snapshot.entities[0], { hp: 10, dead: false });
      return { at: 'revived' };
    },
  };
  for (let index = 0; index < 3; index += 1) {
    assert.equal(await recoverV2Death({ client, result, inWorldStartedAt: Date.now(), startedAt: Date.now(), recovery }), true);
    client.snapshot.playerHp = 0;
    Object.assign(client.snapshot.entities[0], { hp: 0, dead: true });
  }
  await assert.rejects(
    recoverV2Death({ client, result, inWorldStartedAt: Date.now(), startedAt: Date.now(), recovery }),
    /exceeded the 3 authoritative V2 recovery limit/,
  );
  assert.equal(result.recoveries.length, 3);
});

test('a resumed V2 recovery ledger preserves its confirmed receipt and leaves only two recovery slots', async () => {
  const priorRecovery = {
    phase: 'quest', questId: 2110004,
    deathAt: '2026-09-17T20:41:15.152Z',
    before: { mapFileName: '0', objectId: 1, x: 20, y: 30, hp: 0 },
    revive: {
      at: '2026-09-17T20:41:15.615Z',
      before: { map: '0', objectId: 1, x: 20, y: 30, hp: 0, dead: true },
      after: { map: '0', objectId: 1, x: 288, y: 616, hp: 39, dead: false },
    },
    revivedAt: '2026-09-17T20:41:15.615Z',
    after: { mapFileName: '0', objectId: 1, x: 288, y: 616, hp: 39 },
  };
  const ordinaryStartedAt = new Date().toISOString();
  const ledger = parseV2RecoveryLedger({ v2: {
    profile: 'newcomer-v2', ordinaryStartedAt, recoveries: [priorRecovery],
  } });
  const route = await loadNewcomerV2Route({ className: 'Taoist', gender: 'Male' });
  const client = { snapshot: {
    playerObjectId: 1, playerHp: 10, playerLevel: 30,
    entities: [{ objectId: 1, hp: 10, dead: false, level: 30 }],
    questLog: route.quests.map(quest => ({ questId: quest.questId, stage: 'Completed' })),
  } };
  const recovery = {
    maxRecoveries: 3,
    priorRecoveries: ledger.recoveries,
    isDead: current => current.playerHp <= 0,
    revive: async current => {
      current.snapshot.playerHp = 10;
      Object.assign(current.snapshot.entities[0], { hp: 10, dead: false });
      return { at: 'revived' };
    },
  };
  const result = await runNewcomerV2Journey({
    client, className: 'Taoist', gender: 'Male', report: { startedAt: new Date().toISOString() },
    ordinaryStartedAt: ledger.ordinaryStartedAt, recovery,
  });
  assert.equal(result.completed, true);
  assert.notStrictEqual(result.recoveries[0], priorRecovery, 'runner clone-isolates the carried receipt');
  client.snapshot.playerHp = 0;
  Object.assign(client.snapshot.entities[0], { hp: 0, dead: true });
  for (let index = 0; index < 2; index += 1) {
    assert.equal(await recoverV2Death({ client, result, inWorldStartedAt: Date.now(), startedAt: Date.now(), recovery }), true);
    client.snapshot.playerHp = 0;
    Object.assign(client.snapshot.entities[0], { hp: 0, dead: true });
  }
  await assert.rejects(
    recoverV2Death({ client, result, inWorldStartedAt: Date.now(), startedAt: Date.now(), recovery }),
    /exceeded the 3 authoritative V2 recovery limit/,
  );
  assert.equal(result.recoveries.length, 3);
  assert.deepEqual(result.recoveries[0], priorRecovery, 'the prior confirmed town-revive remains in the new report');
});

function practiceQuest(requirements, { questId = 2110004, kills = [{ monsterName: 'Scarecrow', spawns: [{ mapFileName: '0', position: { x: 2, y: 0 }, spread: 0 }] }] } = {}) {
  return {
    questId,
    objectives: {
      kill: kills,
      flag: [{ number: 2210041, message: 'Class practice', conditions: [], requirements }],
    },
  };
}

function noWalkPath(successfulSteps = 0, attempts = 1) {
  const error = new Error('No walk path on D001');
  error.successfulSteps = successfulSteps;
  error.attempts = attempts;
  return error;
}

function practiceClient({ knownSkills, inventoryItems = [], requirements, completeOn = 1, questId, magicCast = true, magicTargetId = null, toggleAcknowledged = true, toggleReceiptOverride = null, preexistingToggleReceipt = null }) {
  const sent = [];
  const requests = [];
  const quest = practiceQuest(requirements, { questId });
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
  const liveMonster = objectId => snapshot.entities.find(entity =>
    entity.kind === 'monster' && entity.dead !== true && Number(entity.hp ?? 1) > 0 && Number(entity.objectId) === Number(objectId));
  const directionalMonster = (command, maximumRange) => {
    const actor = snapshot.entities[0];
    return snapshot.entities.filter(entity => entity.kind === 'monster' && entity.dead !== true && Number(entity.hp ?? 1) > 0)
      .find(entity => {
        const range = directionalRayDistance(actor, entity);
        if (range == null || range > maximumRange) return false;
        const dx = Math.sign(Number(entity.x) - Number(actor.x));
        const dy = Math.sign(Number(entity.y) - Number(actor.y));
        const directions = new Map([['0,-1', 'Up'], ['1,-1', 'UpRight'], ['1,0', 'Right'], ['1,1', 'DownRight'], ['0,1', 'Down'], ['-1,1', 'DownLeft'], ['-1,0', 'Left'], ['-1,-1', 'UpLeft']]);
        return directions.get(`${dx},${dy}`) === command.direction;
      });
  };
  const client = {
    snapshot,
    sequence: 0,
    events: [],
    send(command) {
      sent.push(command);
      client.sequence += 1;
      if (['attack', 'attackDirection', 'magic'].includes(command.type)) actions += 1;
      if (command.type === 'attack') {
        const target = liveMonster(command.objectId);
        if (target) receive('ObjectStruck', { objectId: target.objectId, attackerId: 1 });
      }
      if (command.type === 'attackDirection') {
        const target = directionalMonster(command, command.spell === 3 ? 2 : 1);
        receive('ObjectAttack', { objectId: 1, spell: command.spell });
        if (target) receive('ObjectStruck', { objectId: target.objectId, attackerId: 1 });
      }
      if (command.type === 'spellToggle' && toggleAcknowledged) {
        receive('SpellToggle', toggleReceiptOverride ?? { objectId: 1, spell: command.spell, canUse: command.canUse });
      }
      if (command.type === 'magic') {
        const ground = command.spell === 'FireWall';
        const target = command.spell === 'Lightning'
          ? directionalMonster(command, 6)
          : ground
            ? snapshot.entities.find(entity => entity.kind === 'monster' && entity.dead !== true &&
              Number(entity.hp ?? 1) > 0 && Number(entity.x) === Number(command.x) && Number(entity.y) === Number(command.y))
          : liveMonster(command.targetId);
        receive('ObjectMagic', {
          objectId: 1, spell: command.spell,
          targetId: magicTargetId ?? command.targetId, cast: magicCast,
        });
        if (command.spell !== 'Healing' && command.spell !== 'Poisoning' && target) receive('ObjectStruck', { objectId: target.objectId, attackerId: 1 });
        if (command.spell === 'Poisoning') {
          snapshot.equipmentItems[0].quantity -= 1;
          if (target) receive('ObjectPoisoned', { objectId: target.objectId, poison: 1 });
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
  if (preexistingToggleReceipt) receive('SpellToggle', preexistingToggleReceipt);
  return { client, quest, sent, requests };
}

test('practice search ranks a near spawn field ahead of a remote manifest first entry and stops for an AOI target', async () => {
  const quest = {
    questId: 2110004,
    objectives: { kill: [{ monsterName: 'Oma', spawnCandidates: [
      { mapFileName: '0', position: { x: 90, y: 240 }, spread: 60, count: 20, delayMinutes: 4, respawnIndex: 10 },
      { mapFileName: '0', position: { x: 220, y: 470 }, spread: 70, count: 40, delayMinutes: 7, respawnIndex: 56 },
    ] }], item: [], flag: [] },
  };
  const snapshot = {
    playerObjectId: 1, mapFileName: '0',
    entities: [{ kind: 'selfPlayer', objectId: 1, x: 300, y: 591, hp: 60, maxHp: 60 }],
  };
  assert.deepEqual(practiceSpawnCandidates(snapshot, quest).map(entry => entry.respawnIndex), [56, 10]);
  // The nearest waypoint is inside the close field's real spread, not the
  // remote first center nor an artificial six-tile center radius.
  assert.deepEqual(practiceSpawnWaypoints(snapshot, quest)[0], { x: 250, y: 540 });
  const calls = [];
  const client = { snapshot };
  const target = await approachPracticeTarget(client, quest, async (point, distance, stopWhen, options) => {
    calls.push({ point, distance, options });
    if (calls.length === 1) {
      snapshot.entities.push({ kind: 'monster', objectId: 9, name: 'Oma', x: 250, y: 540, hp: 30, dead: false });
      Object.assign(snapshot.entities[0], { x: 250, y: 539 });
      assert.equal(stopWhen(), true, 'a newly authoritative target ends field navigation immediately');
      return { reached: false, successfulSteps: 16, attempts: 16 };
    }
    return { reached: true, successfulSteps: 0 };
  }, 1, () => {});
  assert.equal(target.objectId, 9);
  assert.equal(calls.length, 2, 'the practice pass must not continue through later coverage after an AOI target appears');
  assert.equal(calls[0].options.maxSuccessfulSteps, 120);
  assert.ok(calls[0].options.maxAttempts <= 180);
});

test('practice reserves the shared attempt budget for the distant first Oma field edge', async () => {
  const quest = {
    questId: 2110004,
    objectives: { kill: [{ monsterName: 'Oma', spawnCandidates: [
      { mapFileName: '0', position: { x: 220, y: 470 }, spread: 70, count: 40, respawnIndex: 56 },
      { mapFileName: '0', position: { x: 140, y: 500 }, spread: 50, count: 30, respawnIndex: 57 },
      { mapFileName: '0', position: { x: 110, y: 440 }, spread: 70, count: 40, respawnIndex: 55 },
    ] }], item: [], flag: [] },
  };
  const snapshot = {
    playerObjectId: 1, mapFileName: '0',
    entities: [{ kind: 'selfPlayer', objectId: 1, x: 293, y: 610, hp: 60, maxHp: 60 }],
  };
  // The field has several equally near y=540 edge cells; ordering selects
  // x=230 first, while x=290 is the same 70-tile distance from this actor.
  assert.deepEqual(practiceSpawnWaypoints(snapshot, quest)[0], { x: 230, y: 540 });
  const calls = [];
  const target = await approachPracticeTarget({ snapshot }, quest, async (point, distance, _stopWhen, options) => {
    calls.push({ point, distance, options });
    if (calls.length === 1) {
      Object.assign(snapshot.entities[0], { x: 230, y: 539 });
      snapshot.entities.push({ kind: 'monster', objectId: 9, name: 'Oma', x: 230, y: 540, hp: 30, dead: false });
      return { reached: false, successfulSteps: 71, attempts: 75 };
    }
    return { reached: true, successfulSteps: 0, attempts: 0 };
  }, 1);
  assert.equal(target.objectId, 9);
  assert.equal(calls[0].options.maxSuccessfulSteps, 120);
  assert.equal(calls[0].options.maxAttempts, 88, 'the initial 70-tile edge receives a bounded detour reservation');
  assert.equal(calls[1].options.maxAttempts, 180, 'the ordinary final target approach keeps its existing independent cap');
});

test('practice debits actual exhausted attempts and successful steps across bounded waypoints', async () => {
  const quest = practiceQuest({ warrior: ['Fencing learned'] }, {
    kills: [{ monsterName: 'Scarecrow', spawnCandidates: [
      { mapFileName: '0', position: { x: 40, y: 0 }, spread: 0, count: 1 },
      { mapFileName: '0', position: { x: 50, y: 0 }, spread: 0, count: 1 },
    ] }],
  });
  const snapshot = { mapFileName: '0', playerObjectId: 1, entities: [{ kind: 'player', objectId: 1, x: 0, y: 0 }] };
  const calls = [];
  const diagnostics = [];
  await assert.rejects(
    approachPracticeTarget({ snapshot, record: (type, payload) => diagnostics.push({ type, payload }) }, quest,
      async (_point, _distance, _stopWhen, options) => {
        calls.push(options);
        const error = new Error(`Navigation attempt budget exceeded (${options.maxAttempts})`);
        error.successfulSteps = 40;
        error.attempts = options.maxAttempts;
        throw error;
      }, 1),
    /has no live configured objective monster/,
  );
  assert.deepEqual(calls.map(options => ({ steps: options.maxSuccessfulSteps, attempts: options.maxAttempts })), [
    { steps: 120, attempts: 90 }, { steps: 80, attempts: 90 },
  ]);
  assert.deepEqual(diagnostics.map(entry => entry.payload.type), [
    'practiceSpawnWaypointAttemptBudgetExhausted', 'practiceSpawnWaypointAttemptBudgetExhausted',
  ]);
});

test('practice keeps a target revealed by the final accepted move at its attempt cap', async () => {
  const quest = practiceQuest({ warrior: ['Fencing learned'] }, {
    kills: [{ monsterName: 'Scarecrow', spawnCandidates: [
      { mapFileName: '0', position: { x: 100, y: 0 }, spread: 0, count: 1 },
    ] }],
  });
  const snapshot = { mapFileName: '0', playerObjectId: 1, entities: [{ kind: 'player', objectId: 1, x: 0, y: 0 }] };
  const calls = [];
  const target = await approachPracticeTarget({ snapshot }, quest,
    async (point, distance, _stopWhen, options) => {
      calls.push({ point, distance, options });
      if (calls.length === 1) {
        Object.assign(snapshot.entities[0], { x: 99, y: 0 });
        snapshot.entities.push({ kind: 'monster', objectId: 17, name: 'Scarecrow', x: 100, y: 0, hp: 10, dead: false });
        const error = new Error(`Navigation attempt budget exceeded (${options.maxAttempts})`);
        error.successfulSteps = 100;
        error.attempts = options.maxAttempts;
        throw error;
      }
      return { reached: true, successfulSteps: 0, attempts: 0 };
    }, 1);
  assert.equal(target.objectId, 17);
  assert.deepEqual(calls.map(call => ({ point: { x: call.point.x, y: call.point.y }, distance: call.distance, attempts: call.options.maxAttempts })), [
    { point: { x: 100, y: 0 }, distance: 6, attempts: 180 },
    { point: { x: 100, y: 0 }, distance: 1, attempts: 180 },
  ]);
});

test('practice skips an unreachable spread waypoint and completes only after a fresh live target and server flag', async () => {
  const { client, quest, sent } = practiceClient({
    knownSkills: ['Fencing'], requirements: { warrior: ['Fencing learned', 'normal attack landed with Fencing learned'] },
  });
  client.snapshot.entities.splice(1);
  quest.objectives.kill[0].spawnCandidates = [
    { mapFileName: '0', position: { x: 10, y: 10 }, spread: 0, count: 1 },
    { mapFileName: '0', position: { x: 20, y: 20 }, spread: 0, count: 1 },
  ];
  const diagnostics = [];
  client.record = (type, payload) => diagnostics.push({ type, payload });
  let navigationCalls = 0;
  await executeV2PracticePlan({
    client,
    quest,
    navigate: async point => {
      navigationCalls += 1;
      if (navigationCalls === 1) throw noWalkPath(0);
      if (navigationCalls === 2) {
        Object.assign(client.snapshot.entities[0], { x: 19, y: 20 });
        client.snapshot.entities.push({ kind: 'monster', objectId: 17, name: 'Scarecrow', x: 20, y: 20, hp: 10, dead: false });
        client.events.push({ sequence: ++client.sequence, direction: 'received', packet: 'worldSnapshot', payload: structuredClone(client.snapshot) });
      }
      return { reached: true, successfulSteps: 1, attempts: 1 };
    },
    plan: [{ kind: 'normal' }],
  });
  assert.deepEqual(diagnostics, [{
    type: 'diagnostic',
    payload: {
      type: 'practiceSpawnWaypointUnreachable', questId: quest.questId, mapFileName: '0', waypoint: { x: 10, y: 10 },
    },
  }]);
  assert.deepEqual(sent, [{ type: 'attack', objectId: 17 }]);
  assert.equal(v2FlagState(client.snapshot, quest.questId, quest.objectives.flag[0]).complete, true);
});

test('practice still fails after every bounded waypoint is unreachable and does not swallow other errors', async () => {
  const quest = practiceQuest({ warrior: ['Fencing learned'] }, {
    kills: [{ monsterName: 'Scarecrow', spawnCandidates: [
      { mapFileName: '0', position: { x: 10, y: 10 }, spread: 0, count: 1 },
      { mapFileName: '0', position: { x: 20, y: 20 }, spread: 0, count: 1 },
    ] }],
  });
  const snapshot = { mapFileName: '0', playerObjectId: 1, entities: [{ kind: 'player', objectId: 1, x: 0, y: 0 }] };
  const diagnostics = [];
  await assert.rejects(
    approachPracticeTarget({ snapshot, record: (type, payload) => diagnostics.push({ type, payload }) }, quest,
      async () => { throw noWalkPath(0); }, 1),
    /has no live configured objective monster/,
  );
  assert.equal(diagnostics.length, 2);
  await assert.rejects(
    approachPracticeTarget({ snapshot }, quest, async () => { throw new Error('ordinary practice navigation failure'); }, 1),
    /ordinary practice navigation failure/,
  );
  const missingCount = new Error('No walk path on D001');
  await assert.rejects(
    approachPracticeTarget({ snapshot }, quest, async () => { throw missingCount; }, 1),
    error => error === missingCount,
  );
  const stringCounts = new Error('Navigation attempt budget exceeded (90)');
  stringCounts.successfulSteps = '0';
  stringCounts.attempts = '90';
  await assert.rejects(
    approachPracticeTarget({ snapshot }, quest, async () => { throw stringCounts; }, 1),
    error => error === stringCounts,
  );
});

test('practice consumes trusted no-path movement before trying the next bounded waypoint', async () => {
  const quest = practiceQuest({ warrior: ['Fencing learned'] }, {
    kills: [{ monsterName: 'Scarecrow', spawnCandidates: [
      { mapFileName: '0', position: { x: 10, y: 10 }, spread: 0, count: 1 },
      { mapFileName: '0', position: { x: 20, y: 20 }, spread: 0, count: 1 },
    ] }],
  });
  const freshSnapshot = () => ({ mapFileName: '0', playerObjectId: 1, entities: [{ kind: 'player', objectId: 1, x: 0, y: 0 }] });
  const callsAfter119 = [];
  await assert.rejects(
    approachPracticeTarget({ snapshot: freshSnapshot() }, quest, async (_point, _distance, _stopWhen, options) => {
      callsAfter119.push(options.maxSuccessfulSteps);
      throw noWalkPath(callsAfter119.length === 1 ? 119 : 1);
    }, 1),
    /has no live configured objective monster/,
  );
  assert.deepEqual(callsAfter119, [120, 1]);

  const callsAfter120 = [];
  await assert.rejects(
    approachPracticeTarget({ snapshot: freshSnapshot() }, quest, async (_point, _distance, _stopWhen, options) => {
      callsAfter120.push(options.maxSuccessfulSteps);
      throw noWalkPath(120);
    }, 1),
    /has no live configured objective monster/,
  );
  assert.deepEqual(callsAfter120, [120]);
});

test('directional practice aligns exact Thrusting and Lightning rays instead of accepting off-axis Chebyshev range', () => {
  assert.equal(directionalRayDistance({ x: 0, y: 0 }, { x: 2, y: 1 }), null);
  assert.equal(directionalRayDistance({ x: 0, y: 0 }, { x: 2, y: 2 }), 2);
  const thrusting = directionalApproachPoints({ x: 0, y: 0 }, { x: 2, y: 1 }, { minRange: 1, maxRange: 2 });
  assert.ok(thrusting.length > 0);
  assert.ok(directionalRayDistance(thrusting[0], { x: 2, y: 1 }) >= 1);
  assert.ok(directionalRayDistance(thrusting[0], { x: 2, y: 1 }) <= 2);

  const lightning = directionalApproachPoints({ x: 0, y: 0 }, { x: 8, y: 1 }, { minRange: 1, maxRange: 6, preferDistant: true });
  assert.equal(lightning[0].range, 6, 'an off-axis Wizard approaches a safer four-to-six tile line before a close tile');
  assert.equal(directionalRayDistance(lightning[0], { x: 8, y: 1 }), 6);
  const closeLightning = directionalApproachPoints({ x: 0, y: 0 }, { x: 1, y: 0 }, { minRange: 1, maxRange: 6, preferDistant: true });
  assert.ok(closeLightning[0].range >= 4, 'an aligned adjacent target still moves the Wizard to a safer ray tile');
});

test('directional practice moves to a line before packets and refreshes a different live target for the next q21 action', async () => {
  const thrusting = practiceClient({
    knownSkills: ['Thrusting'], requirements: { warrior: ['Thrusting attack damage committed'] }, completeOn: 1,
  });
  thrusting.client.snapshot.entities[0].class = 'Warrior';
  Object.assign(thrusting.client.snapshot.entities[1], { x: 2, y: 1 });
  const thrustMoves = [];
  await executeV2PracticePlan({
    client: thrusting.client,
    quest: thrusting.quest,
    navigate: async (point, desiredDistance) => {
      thrustMoves.push({ point: { x: point.x, y: point.y }, desiredDistance });
      Object.assign(thrusting.client.snapshot.entities[0], { x: point.x, y: point.y });
      return { reached: true, successfulSteps: 1, attempts: 1 };
    },
    plan: [{ kind: 'technique', spell: 'Thrusting' }],
  });
  assert.equal(thrustMoves[0].desiredDistance, 0);
  assert.ok(directionalRayDistance(thrusting.client.snapshot.entities[0], thrusting.client.snapshot.entities[1]) <= 2);
  assert.deepEqual(thrusting.sent, [
    { type: 'spellToggle', spell: 'Thrusting', canUse: true },
    { type: 'attackDirection', direction: 'DownRight', spell: 3 },
  ]);

  const { client, quest, sent } = practiceClient({
    knownSkills: ['Lightning', 'FireWall'], requirements: { wizard: ['Lightning damage committed', 'owned FireWall damage committed'] }, completeOn: 2,
  });
  client.snapshot.entities[0].class = 'Wizard';
  Object.assign(client.snapshot.entities[1], { x: 2, y: 1 }); // Chebyshev two, but not a Zone ray.
  client.snapshot.entities.push({ kind: 'monster', objectId: 10, name: 'Scarecrow', x: 8, y: 0, hp: 10, dead: false });
  const movement = [];
  await executeV2PracticePlan({
    client,
    quest,
    navigate: async (point, desiredDistance) => {
      movement.push({ point: { x: point.x, y: point.y }, desiredDistance });
      Object.assign(client.snapshot.entities[0], desiredDistance === 0
        ? { x: point.x, y: point.y }
        : { x: point.x - desiredDistance, y: point.y });
      return { reached: true, successfulSteps: 1, attempts: 1 };
    },
    plan: [{ kind: 'spell', spell: 'Lightning' }, { kind: 'spell', spell: 'FireWall' }],
  });
  assert.equal(movement[0].desiredDistance, 0);
  assert.deepEqual(sent.map(command => command.spell), ['Lightning', 'FireWall']);
  assert.deepEqual(client.events.filter(event => event.packet === 'ObjectStruck').map(event => event.payload.objectId), [9, 10]);
});

test('N16 and N21 FireWall practice use the public ground-cast packet and require the target-zero cast receipt', async () => {
  for (const [questId, requirements] of [
    [2110016, ['FireWall learned', 'owned FireWall damage committed']],
    [2110021, ['Lightning damage committed', 'legal reposition between attacks', 'FireWall learned', 'owned FireWall damage committed']],
  ]) {
    const { client, quest, sent } = practiceClient({
      questId, knownSkills: ['FireWall'], completeOn: 1, requirements: { wizard: requirements },
    });
    client.snapshot.entities[0].class = 'Wizard';
    await executeV2PracticePlan({ client, quest, navigate: async () => {}, plan: [{ kind: 'spell', spell: 'FireWall' }] });
    assert.deepEqual(sent, [{
      type: 'magic', objectId: 1, spell: 'FireWall', direction: 'Right',
      targetId: 0, x: 1, y: 0, spellTargetLock: false,
    }], `q${questId} uses the shared public FireWall ground packet`);
  }
});

test('FireWall rejects a monster-target or non-cast acknowledgement even when the live target is struck', async () => {
  for (const options of [
    { magicTargetId: 9, magicCast: true },
    { magicTargetId: 0, magicCast: false },
  ]) {
    const { client, quest } = practiceClient({
      knownSkills: ['FireWall'], completeOn: 1,
      requirements: { wizard: ['FireWall learned', 'owned FireWall damage committed'] },
      ...options,
    });
    client.snapshot.entities[0].class = 'Wizard';
    await assert.rejects(
      executeV2PracticePlan({ client, quest, navigate: async () => {}, plan: [{ kind: 'spell', spell: 'FireWall' }] }),
      /FireWall lacked a positive post-send target damage receipt/,
    );
  }
});

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

test('HalfMoon arms through its public toggle before attacking, and Lightning self-routes toward its current target', async () => {
  const warrior = practiceClient({ knownSkills: ['HalfMoon'], requirements: { warrior: ['HalfMoon attack damage committed'] } });
  warrior.client.snapshot.entities[0].class = 'Warrior';
  await executeV2PracticePlan({ client: warrior.client, quest: warrior.quest, navigate: async () => {}, plan: [{ kind: 'technique', spell: 'HalfMoon' }] });
  assert.deepEqual(warrior.sent, [
    { type: 'spellToggle', spell: 'HalfMoon', canUse: true },
    { type: 'attackDirection', direction: 'Right', spell: 4 },
  ]);

  const wizard = practiceClient({ knownSkills: ['Lightning'], requirements: { wizard: ['Lightning damage committed'] } });
  wizard.client.snapshot.entities[0].class = 'Wizard';
  wizard.client.snapshot.entities[0].direction = 'Left';
  await executeV2PracticePlan({
    client: wizard.client, quest: wizard.quest,
    navigate: async (point, desiredDistance) => {
      assert.equal(desiredDistance, 0);
      Object.assign(wizard.client.snapshot.entities[0], { x: point.x, y: point.y });
      return { reached: true, successfulSteps: 1, attempts: 1 };
    },
    plan: [{ kind: 'spell', spell: 'Lightning' }],
  });
  assert.deepEqual(wizard.sent, [{ type: 'magic', objectId: 1, spell: 'Lightning', direction: 'Right', targetId: 1, x: -3, y: 0, spellTargetLock: false }]);
});

for (const [label, options] of [
  ['wrong owner', { toggleReceiptOverride: { objectId: 2, spell: 'Thrusting', canUse: true } }],
  ['wrong spell', { toggleReceiptOverride: { objectId: 1, spell: 'HalfMoon', canUse: true } }],
  ['disabled receipt', { toggleReceiptOverride: { objectId: 1, spell: 'Thrusting', canUse: false } }],
  ['old pre-send receipt', {
    toggleAcknowledged: false,
    preexistingToggleReceipt: { objectId: 1, spell: 'Thrusting', canUse: true },
  }],
]) {
  test(`Thrusting fails closed for ${label} toggle receipt`, async () => {
    const { client, quest, sent } = practiceClient({
      knownSkills: ['Thrusting'], requirements: { warrior: ['Thrusting attack damage committed'] }, ...options,
    });
    client.snapshot.entities[0].class = 'Warrior';
    await assert.rejects(
      executeV2PracticePlan({ client, quest, navigate: async () => {}, plan: [{ kind: 'technique', spell: 'Thrusting' }] }),
      /Thrusting lacked an authoritative arming receipt/,
    );
    assert.deepEqual(sent, [{ type: 'spellToggle', spell: 'Thrusting', canUse: true }]);
  });
}

test('HalfMoon fails closed until the server confirms its public toggle', async () => {
  const { client, quest, sent } = practiceClient({
    knownSkills: ['HalfMoon'], requirements: { warrior: ['HalfMoon attack damage committed'] }, toggleAcknowledged: false,
  });
  client.snapshot.entities[0].class = 'Warrior';
  await assert.rejects(
    executeV2PracticePlan({ client, quest, navigate: async () => {}, plan: [{ kind: 'technique', spell: 'HalfMoon' }] }),
    /HalfMoon lacked an authoritative arming receipt/,
  );
  assert.deepEqual(sent, [{ type: 'spellToggle', spell: 'HalfMoon', canUse: true }]);
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
