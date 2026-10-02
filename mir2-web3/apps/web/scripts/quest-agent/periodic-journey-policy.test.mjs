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
import * as periodicPolicy from './periodic-journey-policy.mjs';

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

const lostTargetEvidence = () => {
  const engagementStartedAtMs = Date.parse('2026-10-02T00:34:38.801Z');
  const target = { objectId: 205119, kind: 'monster', name: 'Scarecrow', x: 234, y: 384, hp: 11, dead: false };
  const beforeSnapshot = { playerObjectId: 1, playerExperience: 152, mapFileName: '0', entities: [owner(10),
    { objectId: 50001, kind: 'player', class: 'Wizard' }, target] };
  const afterSnapshot = structuredClone(beforeSnapshot);
  Object.assign(afterSnapshot.entities[2], { hp: 0, dead: true });
  return { beforeSnapshot, afterSnapshot, target, beforeSequence: 1240, engagementStartedAtMs, credits: [],
    events: [{ sequence: 1228, at: new Date(engagementStartedAtMs - 11).toISOString(), direction: 'received',
      packet: 'ObjectMagic', payload: { objectId: 50001, targetId: 205119, cast: true } }] };
};

test('lost target: replay the shared Scarecrow death without granting any owner credit', () => {
  const proof = periodicPolicy.lostPeriodicTarget(lostTargetEvidence());
  assert.equal(proof.objectId, 205119);
  assert.equal(proof.foreignPlayerObjectId, 50001);
  assert.equal(proof.foreignAttackSequence, 1228);
  assert.equal(proof.reason, 'unattacked-target-killed-by-other-player');
  assert.equal(proof.ownCreditGranted, false);
});

for (const type of ['attack', 'attackDirection', 'magic', 'petAttack']) {
  test(`lost target: ${type} attempt keeps missing credit as an error`, () => {
    const evidence = lostTargetEvidence();
    evidence.events.push({ sequence: 1241, direction: 'sent', type, objectId: 205119 });
    assert.equal(periodicPolicy.lostPeriodicTarget(evidence), null);
  });
}

test('lost target: own packet or possible owned pet makes the credit dispute unresolved', () => {
  const ownPacket = lostTargetEvidence();
  ownPacket.events.push({ sequence: 1241, direction: 'received', packet: 'ObjectAttack', payload: { objectId: 1 } });
  assert.equal(periodicPolicy.lostPeriodicTarget(ownPacket), null);
  const pet = lostTargetEvidence();
  pet.beforeSnapshot.entities.push({ objectId: 88, kind: 'monster', ownerObjectId: 1 });
  assert.equal(periodicPolicy.lostPeriodicTarget(pet), null);
  const unknownPet = lostTargetEvidence(); unknownPet.afterSnapshot.pets = [{ objectId: 88 }];
  assert.equal(periodicPolicy.lostPeriodicTarget(unknownPet), null);
});

test('lost target: offscreen/remove alone, living target or changed map is not a death proof', () => {
  for (const mutation of [e => e.afterSnapshot.entities.pop(), e => Object.assign(e.afterSnapshot.entities[2], { hp: 1, dead: false }),
    e => { e.afterSnapshot.mapFileName = 'D001'; }]) {
    const evidence = lostTargetEvidence(); mutation(evidence);
    evidence.events.push({ sequence: 1241, direction: 'received', packet: 'ObjectRemove', payload: { objectId: 205119 } });
    assert.equal(periodicPolicy.lostPeriodicTarget(evidence), null);
  }
});

test('lost target: wrong target, unknown caster and stale foreign attack remain errors', () => {
  for (const mutation of [e => { e.events[0].payload.targetId = 205118; }, e => { e.events[0].payload.objectId = 77; },
    e => { e.events[0].at = new Date(e.engagementStartedAtMs - 10_001).toISOString(); },
    e => { e.events[0].payload.objectId = 1; }]) {
    const evidence = lostTargetEvidence(); mutation(evidence);
    assert.equal(periodicPolicy.lostPeriodicTarget(evidence), null);
  }
});

test('lost target: actual quest or experience credit prevents retry classification', () => {
  const credited = lostTargetEvidence(); credited.credits = [{ questId: 92001, delta: 1 }];
  assert.equal(periodicPolicy.lostPeriodicTarget(credited), null);
  const experience = lostTargetEvidence(); experience.afterSnapshot.playerExperience += 1;
  assert.equal(periodicPolicy.lostPeriodicTarget(experience), null);
});

const foreignMeleeEvidence = () => {
  const target = { objectId: 205114, kind: 'monster', name: 'Scarecrow', x: 213, y: 368, hp: 20, dead: false };
  const beforeSnapshot = { playerObjectId: 50001, playerExperience: 3150, mapFileName: '0',
    entities: [{ ...owner(14), objectId: 50001, x: 229, y: 366 },
      { objectId: 50002, kind: 'player', class: 'Taoist', name: 'OwnedTaoist14', x: 214, y: 369 }, target] };
  const afterSnapshot = structuredClone(beforeSnapshot);
  Object.assign(afterSnapshot.entities[2], { hp: 0, dead: true });
  return { beforeSnapshot, afterSnapshot, target, beforeSequence: 16325,
    engagementStartedAtMs: Date.parse('2026-10-02T03:33:53.091Z'), credits: [], events: [
      { sequence: 16341, at: '2026-10-02T03:33:54.719Z', direction: 'received', packet: 'ObjectAttack', payload: { objectId: 50002 } },
      { sequence: 16342, at: '2026-10-02T03:33:54.719Z', direction: 'received', packet: 'ObjectStruck', payload: { attackerId: 50002, objectId: 205114 } },
      { sequence: 16355, at: '2026-10-02T03:33:55.395Z', direction: 'received', packet: 'ObjectAttack', payload: { objectId: 50002 } },
      { sequence: 16356, at: '2026-10-02T03:33:55.473Z', direction: 'received', packet: 'ObjectStruck', payload: { attackerId: 50002, objectId: 205114 } },
      { sequence: 16359, at: '2026-10-02T03:33:55.474Z', direction: 'received', packet: 'ObjectDied', payload: { objectId: 205114 } },
      { sequence: 16364, at: '2026-10-02T03:33:55.897Z', direction: 'received', type: 'worldSnapshot', payload: structuredClone(afterSnapshot) },
    ] };
};

test('foreign melee: actual Wizard14 no-targetId attack/struck/death race permits reselection only', () => {
  const evidence = foreignMeleeEvidence(), original = structuredClone(evidence.afterSnapshot);
  const proof = periodicPolicy.lostPeriodicTarget(evidence);
  assert.equal(proof.objectId, 205114);
  assert.equal(proof.foreignPlayerObjectId, 50002);
  assert.equal(proof.foreignAttackPacket, 'ObjectStruck');
  assert.equal(proof.foreignAttackSequence, 16356);
  assert.equal(proof.deathObservationSequence, 16359);
  assert.equal(proof.snapshotSequence, 16364);
  assert.equal(proof.ownCreditGranted, false);
  assert.deepEqual(evidence.afterSnapshot, original);
});

test('foreign melee: victim, attacker and engagement sequence identities must match', () => {
  for (const mutate of [
    e => { e.events[1].payload.objectId = e.events[3].payload.objectId = 205113; },
    e => { e.events[1].payload.attackerId = e.events[3].payload.attackerId = 999; },
    e => { e.events[1].payload.attackerId = e.events[3].payload.attackerId = 50001; },
    e => { e.events[1].sequence = e.events[3].sequence = 16324; },
    e => { e.events[1].at = e.events[3].at = '2026-10-02T03:33:52.000Z'; },
    e => { e.events[4].payload.objectId = 205113; },
  ]) {
    const evidence = foreignMeleeEvidence(); mutate(evidence);
    assert.equal(periodicPolicy.lostPeriodicTarget(evidence), null);
  }
});

test('foreign melee: death and fresh same-map authoritative corpse are both required', () => {
  for (const mutate of [
    e => { e.events.splice(4, 1); },
    e => { e.events[4].sequence = 16340; },
    e => { e.events.pop(); },
    e => { e.events[5].sequence = 16358; },
    e => { e.events[5].payload.mapFileName = 'D001'; },
    e => { e.events[5].payload.entities[2].hp = 10; e.events[5].payload.entities[2].dead = false; },
    e => { e.afterSnapshot.mapFileName = 'D001'; },
    e => { e.afterSnapshot.entities[2].hp = 10; e.afterSnapshot.entities[2].dead = false; },
  ]) {
    const evidence = foreignMeleeEvidence(); mutate(evidence);
    assert.equal(periodicPolicy.lostPeriodicTarget(evidence), null);
  }
});

test('foreign melee: owner actions, accepted damage and XP/quest credit remain unresolved errors', () => {
  for (const mutate of [
    e => e.events.push({ sequence: 16330, direction: 'sent', type: 'attack', objectId: 205114 }),
    e => e.events.push({ sequence: 16330, direction: 'sent', type: 'magic', targetId: 205114 }),
    e => e.events.push({ sequence: 16330, direction: 'sent', type: 'petAttack' }),
    e => e.events.push({ sequence: 16330, direction: 'received', packet: 'ObjectStruck', payload: { attackerId: 50001, objectId: 205114 } }),
    e => e.events.push({ sequence: 16330, direction: 'received', packet: 'Magic', payload: { cast: true } }),
    e => e.events.push({ sequence: 16330, direction: 'received', packet: 'GainExperience', payload: { amount: 1 } }),
    e => { e.afterSnapshot.playerExperience += 1; },
    e => { e.credits = [{ questId: 92001, delta: 1 }]; },
  ]) {
    const evidence = foreignMeleeEvidence(); mutate(evidence);
    assert.equal(periodicPolicy.lostPeriodicTarget(evidence), null);
  }
});

test('foreign melee: actual snapshot ownerName and fresh owned Monster packets forbid lost-target credit guesses', () => {
  for (const mutate of [
    e => e.afterSnapshot.entities.push({ objectId: 88, kind: 'monster', name: 'Skeleton', ownerName: 'OwnedWizard14' }),
    e => e.beforeSnapshot.entities.push({ objectId: 88, kind: 'monster', name: 'Skeleton', ownerName: ' ownedwizard14 ' }),
    e => e.events.push({ sequence: 16330, direction: 'received', packet: 'ObjectMonster', payload: { objectId: 88, masterObjectId: 50001 } }),
    e => e.events[5].payload.entities.push({ objectId: 88, kind: 'monster', ownerName: 'OwnedWizard14' }),
  ]) {
    const evidence = foreignMeleeEvidence(); mutate(evidence);
    assert.equal(periodicPolicy.lostPeriodicTarget(evidence), null);
  }
});

test('lost target: a target-bound foreign cast never overrides owner damage or actual pet ownership', () => {
  for (const mutate of [
    e => e.events.push({ sequence: 1241, direction: 'received', packet: 'ObjectStruck', payload: { attackerId: 1, objectId: 205119 } }),
    e => e.events.push({ sequence: 1241, direction: 'received', packet: 'Magic', payload: { cast: true } }),
    e => e.events.push({ sequence: 1241, direction: 'received', packet: 'ObjectMonster', payload: { objectId: 88, masterObjectId: 1 } }),
    e => e.afterSnapshot.entities.push({ objectId: 88, kind: 'monster', name: 'Skeleton', ownerName: 'OwnedWizard14' }),
  ]) {
    const evidence = lostTargetEvidence(); mutate(evidence);
    assert.equal(periodicPolicy.lostPeriodicTarget(evidence), null);
  }
});

const staleCorpseEvidence = () => {
  const route = buildPeriodicRoutes(catalog, 15, sources).find(entry => entry.questId === 92006);
  const beforeSnapshot = snapshot(buildPeriodicRoutes(catalog, 15, sources), 15);
  beforeSnapshot.playerExperience = 420;
  const afterSnapshot = structuredClone(beforeSnapshot);
  afterSnapshot.entities.push({ objectId: 202101, kind: 'monster', name: 'Oma', x: 463, y: 122, hp: 0, dead: true });
  return { error: new Error('Timeout waiting for q92006 Oma kill progress'), route, kill: route.objectives.kill[0],
    beforeSnapshot, afterSnapshot, beforeSequence: 1567, credits: [], events: [
      { sequence: 1376, direction: 'sent', type: 'magic', targetId: 202101 },
      { sequence: 1386, direction: 'received', packet: 'ObjectDied', payload: { objectId: 202101 } },
      { sequence: 1656, direction: 'received', packet: 'ObjectMonster', payload: { objectId: 202101, name: 'Oma', dead: true } },
      { sequence: 1659, direction: 'received', packet: 'ObjectMonster', payload: { objectId: 202101, name: 'Oma', dead: false } },
      { sequence: 1660, direction: 'received', packet: 'ObjectHealth', payload: { objectId: 202101, percent: 100 } },
      { sequence: 1679, direction: 'received', type: 'worldSnapshot', payload: structuredClone(afterSnapshot) },
    ] };
};

test('stale corpse: replay an earlier own Oma corpse without counting another kill', () => {
  const evidence = staleCorpseEvidence(), original = structuredClone(evidence.afterSnapshot);
  const proof = periodicPolicy.staleObservedPeriodicCorpse(evidence);
  assert.equal(proof.objectId, 202101);
  assert.equal(proof.reason, 'unattacked-authoritative-stale-corpse');
  assert.equal(proof.observedAliveSequence, 1659);
  assert.equal(proof.deathObservationSequence, 1656);
  assert.equal(proof.snapshotSequence, 1679);
  assert.equal(proof.priorDeathSequence, 1386);
  assert.equal(proof.ownCreditGranted, false);
  assert.deepEqual(evidence.afterSnapshot, original);
});

for (const type of ['attack', 'attackDirection', 'magic', 'petAttack']) {
  test(`stale corpse: a new ${type} attempt keeps missing credit as an error`, () => {
    const evidence = staleCorpseEvidence();
    evidence.events.push({ sequence: 1661, direction: 'sent', type });
    assert.equal(periodicPolicy.staleObservedPeriodicCorpse(evidence), null);
  });
}

test('stale corpse: pets, accepted owner combat and real XP/quest credit remain errors', () => {
  for (const mutate of [
    e => e.afterSnapshot.entities.push({ objectId: 88, kind: 'monster', ownerObjectId: 1 }),
    e => e.events.push({ sequence: 1661, direction: 'received', packet: 'Magic', payload: { cast: true, targetId: 202101 } }),
    e => e.events.push({ sequence: 1661, direction: 'received', packet: 'ObjectAttack', payload: { objectId: 1 } }),
    e => e.events.push({ sequence: 1661, direction: 'received', packet: 'ObjectStruck', payload: { attackerId: 1, objectId: 202101 } }),
    e => { e.afterSnapshot.playerExperience += 1; },
    e => { e.afterSnapshot.entities[0].level += 1; },
    e => { e.credits = [{ questId: 92006, delta: 1 }]; },
  ]) {
    const evidence = staleCorpseEvidence(); mutate(evidence);
    assert.equal(periodicPolicy.staleObservedPeriodicCorpse(evidence), null);
  }
});

test('stale corpse: actual ownerName and fresh owned monster/damage packets also keep missing credit unresolved', () => {
  for (const mutate of [
    e => e.afterSnapshot.entities.push({ objectId: 88, kind: 'monster', name: 'Skeleton', ownerName: 'OwnedWizard14' }),
    e => e.events.at(-1).payload.entities.push({ objectId: 88, kind: 'monster', ownerName: ' ownedwizard14 ' }),
    e => e.events.push({ sequence: 1661, direction: 'received', packet: 'ObjectMonster', payload: { objectId: 88, masterObjectId: 1 } }),
    e => e.events.push({ sequence: 1661, direction: 'received', packet: 'ObjectStruck', payload: { attackerId: 1, objectId: 202101 } }),
  ]) {
    const evidence = staleCorpseEvidence(); mutate(evidence);
    assert.equal(periodicPolicy.staleObservedPeriodicCorpse(evidence), null);
  }
});

test('stale corpse: no fresh snapshot death, remove only, live target or changed map cannot retry', () => {
  for (const mutate of [
    e => e.events.pop(),
    e => { e.events.at(-1).sequence = 1500; },
    e => { e.events.at(-1).payload.entities.pop(); },
    e => { e.afterSnapshot.entities.at(-1).hp = 30; e.afterSnapshot.entities.at(-1).dead = false; },
    e => { e.afterSnapshot.mapFileName = 'D001'; },
    e => { e.events.at(-1).payload.mapFileName = 'D001'; },
    e => { e.afterSnapshot.entities.at(-1).monsterIndex = 999; },
    e => { delete e.beforeSnapshot.playerExperience; delete e.afterSnapshot.playerExperience; },
  ]) {
    const evidence = staleCorpseEvidence(); mutate(evidence);
    evidence.events.push({ sequence: 1680, direction: 'received', packet: 'ObjectRemove', payload: { objectId: 202101 } });
    assert.equal(periodicPolicy.staleObservedPeriodicCorpse(evidence), null);
  }
});

test('stale corpse: only the planned species exact kill-progress timeout can retry', () => {
  for (const message of ['Timeout waiting for q92007 Oma kill progress', 'Timeout waiting for q92006 Skeleton kill progress',
    'Timeout waiting for combat cooldown worldSnapshot', 'Timeout waiting for worldSnapshot',
    'Connection closed waiting for q92006 Oma kill progress']) {
    const evidence = staleCorpseEvidence(); evidence.error = new Error(message);
    assert.equal(periodicPolicy.staleObservedPeriodicCorpse(evidence), null);
  }
  const wrongSpecies = staleCorpseEvidence(); wrongSpecies.events[3].payload.name = 'Skeleton';
  assert.equal(periodicPolicy.staleObservedPeriodicCorpse(wrongSpecies), null);
  const coded = staleCorpseEvidence(); coded.error.code = 'PERIODIC_DEADLINE';
  assert.equal(periodicPolicy.staleObservedPeriodicCorpse(coded), null);
});

test('stale corpse: two possible observed corpse targets are ambiguous and remain errors', () => {
  const evidence = staleCorpseEvidence();
  const second = { ...evidence.afterSnapshot.entities.at(-1), objectId: 202102 };
  evidence.afterSnapshot.entities.push(second);
  evidence.events.splice(-1, 0, { sequence: 1662, direction: 'received', packet: 'ObjectMonster', payload: { objectId: 202102, name: 'Oma', dead: false } });
  evidence.events.at(-1).payload.entities.push(structuredClone(second));
  assert.equal(periodicPolicy.staleObservedPeriodicCorpse(evidence), null);
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
