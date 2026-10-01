import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import {
  PERIODIC_TIMEOUT_MS, PERIODIC_GATEWAY_URL, PERIODIC_BOUNDARIES, PERIODIC_COMMANDS,
  periodicTier, validatePeriodicCatalog, validateScenario, assertFixtureIdentity, buildPeriodicRoutes,
  assertPeriodicOffers, periodicQuestStates, periodicCreditDeltas, dailyReady, selectPeriodicFarmPlan,
  monsterMatchesPeriodicKill, periodicRewardPreview, experienceGain, validatePeriodicFinish,
  createPeriodicRedactor, periodicTraceProjection,
} from './periodic-journey-policy.mjs';
import { loadCrystalQuestRouteSources } from './route-manifest.mjs';
import { deadlineBoundPeriodicClient } from './run-periodic-journey.mjs';

const catalog = JSON.parse(await fs.readFile(new URL('../../../../config/quest-guidance/daily-weekly-v1.json', import.meta.url)));
const sources = await loadCrystalQuestRouteSources();
const fixture = () => ({ scenarioId: 'measure-Wizard-14', className: 'Wizard', initialLevel: 14,
  accountId: 'owned-only-account', password: 'owned-only-password', name: 'OwnedWizard14',
  fixture: { kind: 'offline-owned-level-equipment', source: 'owned-qa-seed-receipt', sha256: 'a'.repeat(64) } });
const owner = (level = 14, className = 'Wizard') => ({ objectId: 1, name: 'OwnedWizard14', kind: 'player', class: className, level, x: 300, y: 260, hp: 100, dead: false });
const row = (route, current = 0, stage = 'InProgress') => ({ questId: route.questId, stage,
  objectives: route.objectives.kill.map(kill => ({ label: `Defeat ${kill.monsterName}`, current, required: kill.count })) });
const snapshot = (routes, level = 14) => ({ playerObjectId: 1, mapFileName: '0', entities: [owner(level)],
  questLog: routes.map(route => row(route)), playerExperience: 0, gold: 100_000 });

for (const [level, firstId] of [[10, 92001], [14, 92001], [15, 92006], [24, 92006], [25, 92011], [34, 92011], [35, 92016], [50, 92016]]) {
  test(`level ${level} selects exact three-daily/two-weekly acceptance tier`, () => {
    const tier = periodicTier(catalog, level);
    assert.deepEqual(tier.map(quest => quest.id), Array.from({ length: 5 }, (_, index) => firstId + index));
    assert.equal(tier.filter(quest => quest.cadence === 'daily').length, 3);
  });
}

test('outside levels, missing cadence slot and duplicate IDs fail closed', () => {
  for (const level of [0, 9, 51, 500, 10.1]) assert.throws(() => periodicTier(catalog, level));
  const duplicated = structuredClone(catalog); duplicated.quests[1].id = duplicated.quests[0].id;
  assert.throws(() => validatePeriodicCatalog(duplicated), /Duplicate/);
  const weeklyCollision = structuredClone(catalog); weeklyCollision.quests[4].slot = 0;
  assert.throws(() => validatePeriodicCatalog(weeklyCollision), /slot/);
});

test('ordinary QA requires explicit owned fixture provenance and safe plain loopback endpoint', () => {
  const input = validateScenario(fixture());
  assert.equal(input.timeoutMs, PERIODIC_TIMEOUT_MS);
  assert.equal(input.gatewayUrl, PERIODIC_GATEWAY_URL);
  assert.deepEqual(PERIODIC_BOUNDARIES, [10, 14, 15, 24, 25, 34, 35, 50]);
  for (const url of ['wss://127.0.0.1:7290/ws', 'ws://165.154.65.136:7290/ws', 'ws://x:y@localhost:7290/ws', 'ws://localhost/ws?token=private']) {
    assert.throws(() => validateScenario({ ...fixture(), gatewayUrl: url }), /loopback/);
  }
  assert.throws(() => validateScenario({ ...fixture(), fixture: null }), /provenance/);
  assert.throws(() => validateScenario({ ...fixture(), initialLevel: 16 }), /boundary/);
  assert.throws(() => validateScenario({ ...fixture(), timeoutMs: 0 }), /timeout/);
  assert.equal(validateScenario(fixture(), { phase: 'smoke', timeoutMs: 120_000 }).timeoutMs, 120_000);
});

test('character identity uses server-owned class and initial level', () => {
  const routes = buildPeriodicRoutes(catalog, 14, sources);
  const world = snapshot(routes);
  assert.equal(assertFixtureIdentity(world, fixture()).initialLevel, 14);
  assert.throws(() => assertFixtureIdentity(snapshot(routes, 15), fixture()), /does not match/);
  world.entities[0].class = 'Taoist';
  assert.throws(() => assertFixtureIdentity(world, fixture()), /does not match/);
});

test('all twenty tasks resolve real, indexed ordinary monster sources on authored allowed maps', () => {
  for (const level of [10, 15, 25, 35]) {
    for (const route of buildPeriodicRoutes(catalog, level, sources)) {
      for (const kill of route.objectives.kill) {
        assert.ok(kill.spawnCandidates.length > 0);
        assert.ok(kill.spawnCandidates.every(spawn => spawn.count > 0 && spawn.monsterIndex === kill.monsterIndex &&
          kill.allowedMaps.includes(spawn.mapFileName.toLowerCase()) && spawn.source === 'crystal-respawn-manifest'));
      }
    }
  }
  const missing = { ...sources, respawnManifest: { maps: [] } };
  assert.throws(() => buildPeriodicRoutes(catalog, 35, missing), /no ordinary/);
  const ambiguous = { ...sources, monsterManifest: { monsters: [...sources.monsterManifest.monsters,
    { name: 'Scarecrow', monster_index: 999 }] } };
  assert.throws(() => buildPeriodicRoutes(catalog, 10, ambiguous), /uniquely imported/);
});

test('offers must be unique and Available; claimed or resumed rows are not a fresh timing run', () => {
  const routes = buildPeriodicRoutes(catalog, 14, sources), world = snapshot(routes);
  world.questLog = routes.map(route => row(route, 0, 'Available'));
  assert.equal(assertPeriodicOffers(world, routes).length, 5);
  world.questLog[0].stage = 'Completed';
  assert.throws(() => assertPeriodicOffers(world, routes), /fresh server offer/);
  world.questLog = routes.map(route => row(route, 0, 'Available'));
  world.questLog.push(world.questLog[0]);
  assert.throws(() => assertPeriodicOffers(world, routes), /unique/);
});

test('merged farming picks visible pending daily targets and excludes weekly-only or completed objectives', () => {
  const routes = buildPeriodicRoutes(catalog, 14, sources), world = snapshot(routes);
  world.entities.push({ objectId: 9, name: 'HookingCat', kind: 'monster', x: 301, y: 260, hp: 10 });
  assert.equal(selectPeriodicFarmPlan(world, routes).route.questId, 92003);
  world.questLog.find(quest => quest.questId === 92003).stage = 'ReadyToTurnIn';
  world.questLog.find(quest => quest.questId === 92003).objectives[0].current = 60;
  assert.notEqual(selectPeriodicFarmPlan(world, routes).route.questId, 92003);
  for (const route of routes.filter(route => route.cadence === 'daily')) {
    const quest = world.questLog.find(quest => quest.questId === route.questId);
    quest.stage = 'ReadyToTurnIn'; quest.objectives[0].current = route.objectives.kill[0].count;
  }
  assert.equal(dailyReady(world, routes), true);
  assert.equal(selectPeriodicFarmPlan(world, routes), null);
});

test('same name with an explicit wrong monster index, dead actor or disallowed map is never a target', () => {
  const kill = buildPeriodicRoutes(catalog, 15, sources).find(route => route.questId === 92007).objectives.kill[0];
  const skeleton = { objectId: 22, name: 'Skeleton', kind: 'monster', x: 10, y: 10, hp: 80, monsterIndex: 85 };
  assert.equal(monsterMatchesPeriodicKill(skeleton, kill, 'd001.map'), true);
  assert.equal(monsterMatchesPeriodicKill({ ...skeleton, monsterIndex: 999 }, kill, 'D001'), false);
  assert.equal(monsterMatchesPeriodicKill(skeleton, kill, 'D501'), false);
  assert.equal(monsterMatchesPeriodicKill({ ...skeleton, dead: true }, kill, 'D001'), false);
  assert.equal(monsterMatchesPeriodicKill({ ...skeleton, kind: 'npc' }, kill, 'D001'), false);
});

test('kill credit is exact-ID and exact-index, with daily and weekly increments independently recorded', () => {
  const routes = buildPeriodicRoutes(catalog, 10, sources), world = snapshot(routes, 10);
  const before = periodicQuestStates(world, routes);
  world.questLog.find(quest => quest.questId === 92001).objectives[0].current = 1;
  world.questLog.find(quest => quest.questId === 92004).objectives[0].current = 1;
  const credits = periodicCreditDeltas(before, periodicQuestStates(world, routes));
  assert.deepEqual(credits.map(credit => [credit.questId, credit.monsterIndex, credit.delta]), [[92001, 39, 1], [92004, 39, 1]]);
  world.questLog.find(quest => quest.questId === 92001).objectives[0].required = 79;
  assert.throws(() => periodicQuestStates(world, routes), /identity\/count/);
});

test('accepted reward preview is bound to server ID, tier, gold and task counts', () => {
  const route = buildPeriodicRoutes(catalog, 10, sources)[0];
  const info = { index: 92001, min_level_needed: 10, max_level_needed: 14, reward_gold: 4000, reward_exp: 600,
    task_description: ['Defeat 80 Scarecrow.'] };
  assert.equal(periodicRewardPreview(info, route).experience, 600);
  assert.throws(() => periodicRewardPreview({ ...info, index: 92004 }, route), /definition/);
  assert.throws(() => periodicRewardPreview({ ...info, task_description: ['Defeat 8 Scarecrow.'] }, route), /task definition/);
});

test('reward proof survives level rollover and rejects unrelated ACK, missing removal or reward mismatch', () => {
  const before = { level: 14, experience: 29_800, gold: 5000 }, after = { level: 15, experience: 400, gold: 9000 };
  assert.equal(experienceGain(before, after, sources.contentProfile.experienceCurve), 600);
  const evidence = { before, after, preview: { questId: 92001, experience: 600, gold: 4000 },
    operation: { command: { type: 'finishQuest', requestId: 'owned-request' }, acknowledgement:
      { operation: 'finishQuest', requestId: 'owned-request', questIndex: 92001, selectedItemIndex: -1, success: true } },
    changeQuest: { packet: 'ChangeQuest', payload: { questId: 92001, questState: 2, completed: true, taken: false } },
    quest: { stage: 'Completed' }, curve: sources.contentProfile.experienceCurve };
  assert.equal(validatePeriodicFinish(evidence).verified, true);
  const wrongAck = structuredClone(evidence); wrongAck.operation.acknowledgement.requestId = 'stale-request';
  assert.throws(() => validatePeriodicFinish(wrongAck), /acknowledgement/);
  assert.throws(() => validatePeriodicFinish({ ...evidence, changeQuest: null }), /removal/);
  assert.throws(() => validatePeriodicFinish({ ...evidence, after: { ...after, gold: 9001 } }), /delta/);
});

test('private account, password, nested tokens and embedded rejection strings are redacted from disk projection', () => {
  const redact = createPeriodicRedactor(['owned-only-account', 'owned-only-password']);
  const event = { type: 'worldSnapshot', payload: { accountId: 'owned-only-account', token: 'abc',
    playerExperience: 123, gold: 4000, entities: [{ name: 'owned-only-account' }],
    questLog: [{ questId: 92001, stage: 'InProgress' }], questOperationAck: { requestId: 'q1', success: true },
    inventoryItems: [{ name: 'Potion', uniqueId: 42, quantity: 3, tooltipSource: { private: 'owned-only-password' } }] } };
  const sanitized = redact(periodicTraceProjection(event));
  assert.equal(sanitized.payload.entities[0].name, '[redacted]');
  assert.equal(sanitized.payload.playerExperience, 123);
  assert.equal(sanitized.payload.questOperationAck.success, true);
  assert.equal(sanitized.payload.inventoryItems[0].quantity, 3);
  assert.equal(redact({ account_id: 'abc', password: 'anything', nested: { token: 'secret' }, error: 'Rejected owned-only-account' }).nested.token, '[redacted]');
  const serialized = JSON.stringify(sanitized);
  assert.ok(!serialized.includes('owned-only-account') && !serialized.includes('owned-only-password'));
  assert.equal(event.payload.entities[0].name, 'owned-only-account', 'in-memory observations remain untouched');
});

test('bounded wrapper preserves normal commands and forbids all runtime injection and account creation', async () => {
  const sent = [], waits = [];
  const client = { send: command => sent.push(command), request: (command, label, timeout) => { sent.push(command); return { label, timeout }; },
    wait: async (predicate, label, timeout) => { waits.push(timeout); return predicate(); } };
  const bounded = deadlineBoundPeriodicClient(client, Date.now() + 1000, () => {});
  bounded.send({ type: 'attack', objectId: 3 });
  for (const type of ['newAccount', 'newCharacter', 'qa.giveItem', 'MoveTo', 'Stage5Command', 'teleport']) {
    assert.ok(!PERIODIC_COMMANDS.has(type)); assert.throws(() => bounded.send({ type }), /Forbidden/);
  }
  await bounded.wait(() => true, 'receipt', 60_000);
  assert.ok(waits[0] <= 1000);
  assert.equal(sent.length, 1);
  const expired = deadlineBoundPeriodicClient(client, Date.now() - 1, () => { throw new Error('deadline'); });
  assert.throws(() => expired.send({ type: 'walk' }), /deadline/);
});
