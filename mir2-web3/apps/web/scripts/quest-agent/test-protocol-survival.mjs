import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';
import {
  applyProtocolObservation,
  hasAuthoritativePlayerDeath,
  observedPlayerHp,
} from './protocol-observation.mjs';

const runnerSource = await fs.readFile(new URL('./run-protocol-journey.mjs', import.meta.url), 'utf8');

function runnerFunctionSource(name) {
  const start = runnerSource.indexOf(`async function ${name}(`);
  assert.notEqual(start, -1, `missing runner function ${name}`);
  // This function takes a destructured options argument, so its first brace
  // belongs to the parameter list rather than the executable body.
  const signatureEnd = runnerSource.indexOf('} = {}) {', start);
  assert.notEqual(signatureEnd, -1, `missing options boundary for ${name}`);
  const open = signatureEnd + '} = {}) '.length;
  let depth = 0;
  let quote = null;
  for (let index = open; index < runnerSource.length; index += 1) {
    const character = runnerSource[index];
    const next = runnerSource[index + 1];
    if (quote) {
      if (character === '\\') { index += 1; continue; }
      if (character === quote) quote = null;
      continue;
    }
    if (character === '/' && next === '/') {
      index = runnerSource.indexOf('\n', index + 2);
      if (index < 0) break;
      continue;
    }
    if (character === '/' && next === '*') {
      index = runnerSource.indexOf('*/', index + 2);
      if (index < 0) break;
      index += 1;
      continue;
    }
    if (["'", '"', '`'].includes(character)) { quote = character; continue; }
    if (character === '{') depth += 1;
    if (character === '}' && --depth === 0) return runnerSource.slice(start, index + 1);
  }
  throw new Error(`unterminated runner function ${name}`);
}

function stabilizeJourneyResumeHarness(overrides = {}) {
  const dependencies = {
    journeyResumeDisposition: () => ({ status: 'evade' }),
    canResumeStockedCombatExpedition: () => false,
    playerIsDead: snapshot => snapshot?.entities?.[0]?.dead === true,
    hpDrugCount: () => 1,
    recoverHealthWhileEvading: async () => { throw new Error('Evasive HP recovery timed out'); },
    selfPlayer: owner => owner.snapshot?.entities?.[0] ?? null,
    observedPlayerHp: snapshot => Number(snapshot?.playerHp ?? 0),
    isLivingEvasiveRecoveryTimeout: (snapshot, error) =>
      Number(snapshot?.playerHp ?? 0) > 0 && String(error?.message ?? error) === 'Evasive HP recovery timed out',
    reviveInTown: async () => ({ revived: true }),
    startEmergencyHpRecovery: () => [],
    String,
    Number,
    ...overrides,
  };
  return vm.runInNewContext(`(${runnerFunctionSource('stabilizeJourneyResume')})`, dependencies);
}

test('a healthy q98 finisher keeps enough held fuel for its last visible wounded soldier', () => {
  const snapshot = {
    playerObjectId: 1000, playerHp: 159, playerMaxHp: 159,
    entities: [
      { objectId: 1000, kind: 'selfPlayer', hp: 159, maxHp: 159, x: 257, y: 211 },
      { objectId: 2000, kind: 'monster', name: 'WoomaSoldier', hp: 285, maxHp: 285, healthPercent: 19, healthObservation: 'percent', x: 253, y: 213 },
    ],
    questLog: [{ questId: 98, stage: 'inProgress', objectives: [
      { label: 'Kill Dung', current: 3, required: 3 },
      { label: 'Kill WoomaSoldier', current: 2, required: 3 },
    ] }],
  };
  assert.deepEqual(journeyAmuletSupplyPolicyForQuest(98, 'Taoist', snapshot), { minimum: 13, departure: 100 });
  snapshot.entities[1].healthPercent = 100;
  assert.equal(journeyAmuletSupplyPolicyForQuest(98, 'Taoist', snapshot).minimum, 48);
  snapshot.entities[1].healthPercent = 19;
  snapshot.questLog[0].objectives[1].current = 1;
  assert.equal(journeyAmuletSupplyPolicyForQuest(98, 'Taoist', snapshot).minimum, 48);
  snapshot.questLog[0].objectives[1].current = 2;
  snapshot.playerHp = 90;
  assert.equal(journeyAmuletSupplyPolicyForQuest(98, 'Taoist', snapshot).minimum, 48);
});

test('evasive recovery stops on fresh full-health packets without another retreat', async () => {
  const snapshot = {
    playerObjectId: 1000, playerHp: 65, playerMaxHp: 100,
    entities: [{ objectId: 1000, kind: 'selfPlayer', hp: 65, maxHp: 100, dead: false, x: 129, y: 251 }],
  };
  applyProtocolObservation(snapshot, { type: 'packet', packet: 'ObjectHealth', payload: { objectId: 1000, percent: 100 } });
  const client = { snapshot };
  const result = await recoverHealthWhileEvading(client, async () => {
    assert.fail('a full-health player should resume its objective instead of retreating again');
  }, { requiredRatio: 0.75 });
  assert.deepEqual(result, { status: 'recovered', hp: 100, maxHp: 100, refreshes: 0, evasiveMoves: 0 });
  assert.equal(snapshot.playerHp, 65, 'control estimates do not overwrite exact snapshot evidence');
});

import {
  canFinishNearCompleteKillQuestWithoutHpStock,
  canFinishNearCompleteKillQuestWithReducedHpStock,
  canResumeStockedCombatExpedition,
  createPostEngagementSupplyGate,
  evasiveRecoveryDangerDistanceForQuest,
  evasiveRecoveryTimeoutMsForQuest,
  hpDrugCount,
  hpMediumDrugCount,
  hasJourneyEmergencyEscapeQuest,
  isLivingEvasiveRecoveryTimeout,
  isLivingExpeditionNoWalkPath,
  isLivingLostCombatTarget,
  isLivingUnsafePackRetreatFailure,
  mpDrugCount,
  hpRestockTargetForActiveQuests,
  journeyExpeditionDepartureFloorForQuest,
  journeyEmergencyEscapeRestockTarget,
  journeyEmergencyTownTeleportRestockTarget,
  journeyEmergencyTeleportDepartureTarget,
  journeyExpeditionSupplyActive,
  journeyNavigationEmergencyEscapeBudget,
  journeyEmergencyTeleportCriticalHpRatio,
  shouldUseQ89WizardSameMapRandomEscape,
  journeyResumeDisposition,
  journeyMpRestockTargetForQuest,
  journeyAmuletSupplyPolicyForQuest,
  questCombatMpUseThresholdForQuest,
  questEmergencyEscapeHpRatio,
  questPostRetreatRecoveryRatio,
  q89WizardFreshCursedShamanRecoveryOptions,
  preferredObjectiveMapsForQuest,
  minimumJourneyMpStockForQuest,
  amuletStock,
  questRetreatBiasPosition,
  questRetreatProfile,
  questSpawnSearchHostileClearanceFallback,
  questNeedsPostRetreatRecovery,
  requiresTaoistAmuletRestock,
  shouldPreferObjectiveMapOverCurrent,
  recoverHealthWhileEvading,
  requiresExpeditionEscapeRestock,
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

test('Taoist Wooma departure funds enough SoulFireBall casts for a full-health pull', () => {
  assert.deepEqual(journeyAmuletSupplyPolicyForQuest(98, 'Taoist'), { minimum: 48, departure: 100 });
  assert.deepEqual(journeyAmuletSupplyPolicyForQuest(99, 'Taoist'), { minimum: 48, departure: 100 });
  assert.ok(journeyAmuletSupplyPolicyForQuest(98, 'Taoist').minimum * 7 >= 285);
  assert.deepEqual(journeyAmuletSupplyPolicyForQuest(60, 'Taoist'), { minimum: 12, departure: 32 });
  assert.deepEqual(journeyAmuletSupplyPolicyForQuest(98, 'Wizard'), { minimum: 0, departure: 0 });
  assert.deepEqual(journeyAmuletSupplyPolicyForQuest(42, 'Taoist'), { minimum: 0, departure: 0 });
});

test('q89 Taoist reserves 100 Amulet while other class and quest policies stay unchanged', () => {
  assert.deepEqual(journeyAmuletSupplyPolicyForQuest(89, 'Taoist'), { minimum: 32, departure: 100 });
  assert.equal(amuletStock({ beltItems: [{ name: 'Amulet', quantity: 31 }] }), 31);
  assert.equal(requiresTaoistAmuletRestock({
    beltItems: [{ name: 'Amulet', quantity: 31 }],
    knownSkills: [{ spell: 'SoulFireBall' }],
  }, 89, 'Taoist', 32), true);
  assert.equal(requiresTaoistAmuletRestock({
    beltItems: [{ name: 'Amulet', quantity: 32 }],
    knownSkills: [{ spell: 'SoulFireBall' }],
  }, 89, 'Taoist', 32), false);
  assert.deepEqual(journeyAmuletSupplyPolicyForQuest(89, 'Wizard'), { minimum: 0, departure: 0 });
  assert.deepEqual(journeyAmuletSupplyPolicyForQuest(60, 'Taoist'), { minimum: 12, departure: 32 });
});

test('caster expedition restock targets stay separate from their field triggers', () => {
  assert.equal(journeyMpRestockTargetForQuest(54, 'Wizard'), 80);
  assert.equal(journeyMpRestockTargetForQuest(54, 'Taoist'), 80);
  assert.equal(journeyMpRestockTargetForQuest(65, 'Taoist'), 80);
  assert.equal(journeyMpRestockTargetForQuest(42, 'Wizard'), 32);
  assert.equal(journeyMpRestockTargetForQuest(49, 'Taoist'), 12);
  assert.equal(journeyMpRestockTargetForQuest(54, 'Warrior'), 0);
  assert.equal(journeyMpRestockTargetForQuest(49, 'Wizard', { fallback: 18 }), 18);
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(98, 'Taoist'), { hp: 64, mp: 12 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(99, 'Taoist'), { hp: 64, mp: 12 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(98, 'Wizard'), { hp: 64, mp: 0 });
  assert.equal(questCombatMpUseThresholdForQuest(54, 'Wizard'), 0.75);
  assert.equal(questCombatMpUseThresholdForQuest(54, 'Taoist'), 0.75);
  assert.equal(questCombatMpUseThresholdForQuest(65, 'Taoist'), 0.75);
  assert.equal(questCombatMpUseThresholdForQuest(54, 'Warrior'), 0.3);
  assert.equal(questCombatMpUseThresholdForQuest(49, 'Wizard', { fallback: 0.4 }), 0.4);
  assert.equal(questPostRetreatRecoveryRatio(54, 'Wizard'), 0.65);
  assert.equal(questPostRetreatRecoveryRatio(54, 'Taoist'), 0.65);
  assert.equal(questPostRetreatRecoveryRatio(65, 'Wizard'), 0.65);
  assert.equal(questPostRetreatRecoveryRatio(89, 'Wizard'), 0.75);
  assert.equal(questPostRetreatRecoveryRatio(89, 'Taoist'), 0.75);
  assert.equal(questPostRetreatRecoveryRatio(54, 'Warrior'), 0.75);
  assert.equal(questPostRetreatRecoveryRatio(62, 'Warrior'), 0.75);
  assert.equal(questEmergencyEscapeHpRatio(54), 0.35);
  assert.equal(questEmergencyEscapeHpRatio(60), 0.35);
  assert.equal(questEmergencyEscapeHpRatio(60, 'Wizard'), 0.65);
  assert.equal(questEmergencyEscapeHpRatio(60, 'Taoist'), 0.35);
  assert.equal(questEmergencyEscapeHpRatio(62), 0.35);
  assert.equal(questEmergencyEscapeHpRatio(65, 'Wizard'), 0.65);
  assert.equal(questEmergencyEscapeHpRatio(65, 'Taoist'), 0.35);
  assert.equal(questEmergencyEscapeHpRatio(98, 'Wizard'), 0.9);
  assert.equal(questEmergencyEscapeHpRatio(98, 'Taoist'), 0.65);
  assert.equal(questEmergencyEscapeHpRatio(99, 'Wizard'), 0.9);
  assert.equal(questEmergencyEscapeHpRatio(99, 'Taoist'), 0.65);
  assert.equal(questEmergencyEscapeHpRatio(113, 'Wizard'), 0.65);
  assert.equal(questEmergencyEscapeHpRatio(113, 'Taoist'), 0.35);
  assert.equal(questEmergencyEscapeHpRatio(49), 0);
  assert.equal(journeyNavigationEmergencyEscapeBudget('Wizard'), 4);
  assert.equal(journeyNavigationEmergencyEscapeBudget('Taoist'), 4);
  assert.equal(journeyNavigationEmergencyEscapeBudget('Warrior'), 4);
  assert.equal(journeyEmergencyTeleportCriticalHpRatio('Wizard'), 0.65);
  assert.equal(journeyEmergencyTeleportCriticalHpRatio('Taoist'), 0.65);
  assert.equal(journeyEmergencyTeleportCriticalHpRatio('Warrior'), 0.35);
});

test('dangerous expedition escape remains armed for the completed cave return', () => {
  assert.equal(hasJourneyEmergencyEscapeQuest({
    questLog: [{ questId: 60, stage: 'inProgress' }],
  }), true);
  assert.equal(hasJourneyEmergencyEscapeQuest({
    questLog: [{ questId: 62, stage: 'readyToTurnIn' }],
  }), true);
  assert.equal(hasJourneyEmergencyEscapeQuest({
    questLog: [{ questId: 65, stage: 'inProgress' }],
  }), true);
  assert.equal(hasJourneyEmergencyEscapeQuest({
    questLog: [{ questId: 98, stage: 'inProgress' }],
  }), true);
  assert.equal(hasJourneyEmergencyEscapeQuest({
    questLog: [{ questId: 113, stage: 'inProgress' }],
  }), true);
  assert.equal(hasJourneyEmergencyEscapeQuest({
    questLog: [{ questId: 54, stage: 'completed' }],
  }), false);
  assert.equal(hasJourneyEmergencyEscapeQuest({
    questLog: [{ questId: 49, stage: 'readyToTurnIn' }],
  }), false);
});

test('dangerous Taoist expeditions restock missing equipped or carried Amulets', () => {
  const empty = {
    inventoryItems: [], beltItems: [], equipmentItems: [],
    knownSkills: [{ spell: 'SoulFireBall' }],
  };
  assert.equal(requiresTaoistAmuletRestock(empty, 60, 'Taoist'), true);
  assert.equal(requiresTaoistAmuletRestock(empty, 65, 'Taoist'), true);
  assert.equal(requiresTaoistAmuletRestock(empty, 49, 'Taoist'), false);
  assert.equal(requiresTaoistAmuletRestock(empty, 60, 'Wizard'), false);
  assert.equal(requiresTaoistAmuletRestock({ ...empty, knownSkills: [] }, 60, 'Taoist'), false);
  assert.equal(requiresTaoistAmuletRestock({
    inventoryItems: [{ name: 'Amulet', quantity: 2 }],
    beltItems: [{ key: 'crystal-item-712', count: 2 }],
    equipmentItems: [{ name: 'Amulet', quantity: 3 }],
    knownSkills: [{ spell: 'SoulFireBall' }],
  }, 60, 'Taoist', 7), false);
});

test('only a living exact evasive recovery timeout is retryable', () => {
  assert.equal(isLivingEvasiveRecoveryTimeout(
    { playerHp: 102, playerMaxHp: 224 },
    new Error('Evasive HP recovery timed out'),
  ), true);
  assert.equal(isLivingEvasiveRecoveryTimeout(
    { playerHp: 0, playerMaxHp: 224 },
    new Error('Evasive HP recovery timed out'),
  ), false);
  assert.equal(isLivingEvasiveRecoveryTimeout(
    { playerHp: 102, playerMaxHp: 224 },
    new Error('No walk path'),
  ), false);

  const freshLiving = {
    playerObjectId: 1000,
    playerHp: 109,
    playerMaxHp: 180,
    entities: [{ objectId: 1000, kind: 'selfPlayer', hp: 109, maxHp: 180, dead: false }],
  };
  assert.equal(hasAuthoritativePlayerDeath(freshLiving), false);
  assert.equal(isLivingEvasiveRecoveryTimeout(
    freshLiving,
    new Error('Evasive HP recovery timed out'),
  ), true);

  const authoritativeDeath = structuredClone(freshLiving);
  authoritativeDeath.playerHp = 0;
  authoritativeDeath.entities[0].hp = 0;
  authoritativeDeath.entities[0].dead = true;
  assert.equal(hasAuthoritativePlayerDeath(authoritativeDeath), true);
  assert.equal(isLivingEvasiveRecoveryTimeout(
    authoritativeDeath,
    new Error('Evasive HP recovery timed out'),
  ), false);
});

test('startup recovery defers only a fresh living timeout into the guarded quest path', async () => {
  const stabilize = stabilizeJourneyResumeHarness();
  const diagnostics = [];
  const owner = {
    snapshot: {
      mapFileName: 'D2031',
      playerHp: 109,
      playerMaxHp: 180,
      entities: [{ objectId: 1000, kind: 'selfPlayer', hp: 109, maxHp: 180, dead: false }],
    },
    record: (_direction, payload) => diagnostics.push(payload),
  };
  const deferred = await stabilize(owner, async () => {});
  assert.deepEqual({ ...deferred }, { status: 'evasiveRecoveryDeferred', hp: 109, maxHp: 180 });
  assert.deepEqual(diagnostics.map(payload => ({ ...payload })), [{
    type: 'livingResumeEvasiveRecoveryTimeoutRetry', hp: 109, maxHp: 180, mapFileName: 'D2031',
  }]);

  let revivals = 0;
  const deadStabilize = stabilizeJourneyResumeHarness({
    reviveInTown: async () => { revivals += 1; return { revived: true }; },
  });
  const deadOwner = {
    snapshot: {
      mapFileName: 'D2031', playerHp: 0, playerMaxHp: 180,
      entities: [{ objectId: 1000, kind: 'selfPlayer', hp: 0, maxHp: 180, dead: true }],
    },
    record: () => assert.fail('death must revive rather than defer'),
  };
  assert.deepEqual({ ...await deadStabilize(deadOwner, async () => {}) }, {
    status: 'revived', revival: { revived: true },
  });
  assert.equal(revivals, 1);

  const missingSelf = stabilizeJourneyResumeHarness({ selfPlayer: () => null });
  await assert.rejects(() => missingSelf({
    snapshot: { playerHp: 109, playerMaxHp: 180, entities: [] }, record: () => {},
  }, async () => {}), /Evasive HP recovery timed out/);

  const unrelated = stabilizeJourneyResumeHarness({
    recoverHealthWhileEvading: async () => { throw new Error('No walk path'); },
  });
  await assert.rejects(() => unrelated(owner, async () => {}), /No walk path/);
});

test('only living dangerous expeditions replan transient no-walk-path failures', () => {
  const alive = { playerHp: 51, playerMaxHp: 76 };
  assert.equal(isLivingExpeditionNoWalkPath(
    alive,
    65,
    new Error('No walk path on D421 from 106,149 to 361,19'),
  ), true);
  assert.equal(isLivingExpeditionNoWalkPath(
    alive,
    60,
    new Error('No walk path to visible SpiderFrog'),
  ), true);
  assert.equal(isLivingExpeditionNoWalkPath(
    alive,
    99,
    new Error('Travel from D021 to 1 is blocked by monster 293711'),
  ), true);
  assert.equal(isLivingExpeditionNoWalkPath(
    alive,
    49,
    new Error('No walk path on D011'),
  ), false);
  assert.equal(isLivingExpeditionNoWalkPath(
    { playerHp: 0, playerMaxHp: 76 },
    65,
    new Error('No walk path on D421'),
  ), false);
  assert.equal(isLivingExpeditionNoWalkPath(
    alive,
    65,
    new Error('Map transfer timed out'),
  ), false);
});

test('a living partial unsafe-pack retreat can replan without discarding field progress', () => {
  assert.equal(isLivingUnsafePackRetreatFailure(
    { playerHp: 110, playerMaxHp: 224 },
    new Error('unsafe hostile pack retreat failed from 145,103'),
  ), true);
  assert.equal(isLivingUnsafePackRetreatFailure(
    { playerHp: 0, playerMaxHp: 224 },
    new Error('unsafe hostile pack retreat failed from 145,103'),
  ), false);
  assert.equal(isLivingUnsafePackRetreatFailure(
    { playerHp: 110, playerMaxHp: 224 },
    new Error('No walk path'),
  ), false);
});

test('a living target lost at the AOI edge is retryable but death and unrelated errors are not', () => {
  const living = { playerHp: 40, playerMaxHp: 80 };
  assert.equal(isLivingLostCombatTarget(
    living,
    new Error('target 293721 left the authoritative snapshot before attack'),
  ), true);
  assert.equal(isLivingLostCombatTarget(
    living,
    new Error('target 293721 left the authoritative snapshot before death was confirmed'),
  ), true);
  assert.equal(isLivingLostCombatTarget(
    living,
    new Error('target 293711 has no walk path from the current map region'),
  ), true);
  assert.equal(isLivingLostCombatTarget(
    { playerHp: 0, playerMaxHp: 80 },
    new Error('target 293721 left the authoritative snapshot before attack'),
  ), false);
  assert.equal(isLivingLostCombatTarget(living, new Error('target 293721 made no progress')), false);
});

test('a resumed expedition preserves field progress while it still has a public escape reserve', () => {
  const scroll = quantity => ({
    name: 'RandomTeleport',
    quantity,
    container: 'bag1',
    tooltipSource: { info: { item_index: 717 } },
  });
  assert.equal(requiresExpeditionEscapeRestock({ inventoryItems: [] }, 62), true);
  assert.equal(requiresExpeditionEscapeRestock({ inventoryItems: [] }, 60), true);
  assert.equal(requiresExpeditionEscapeRestock({ inventoryItems: [] }, 65), true);
  assert.equal(requiresExpeditionEscapeRestock({ inventoryItems: [] }, 98), true);
  assert.equal(requiresExpeditionEscapeRestock({ inventoryItems: [] }, 113), true);
  assert.equal(requiresExpeditionEscapeRestock({ inventoryItems: [scroll(1)] }, 54), false);
  assert.equal(requiresExpeditionEscapeRestock({ inventoryItems: [scroll(3)] }, 54), false);
  assert.equal(requiresExpeditionEscapeRestock({ inventoryItems: [scroll(4)] }, 54), false);
  assert.equal(requiresExpeditionEscapeRestock({ inventoryItems: [] }, 49), false);
});

test('a dangerous expedition keeps partial field scrolls but restocks once the reserve is empty', () => {
  const scroll = quantity => ({
    name: 'RandomTeleport',
    quantity,
    container: 'bag1',
    tooltipSource: { info: { item_index: 717 } },
  });
  const active = quantity => ({
    mapFileName: 'D022',
    questLog: [{ questId: 98, stage: 'inProgress' }],
    inventoryItems: quantity > 0 ? [scroll(quantity)] : [],
  });
  assert.equal(journeyEmergencyEscapeRestockTarget(active(3), 98), 0);
  assert.equal(journeyEmergencyEscapeRestockTarget(active(0), 98), 4);
  assert.equal(journeyEmergencyEscapeRestockTarget(active(3), 98, { force: true }), 4);
  assert.equal(journeyEmergencyEscapeRestockTarget({ ...active(3), mapFileName: '0' }, 98), 4);
  assert.equal(journeyEmergencyEscapeRestockTarget({
    ...active(0), questLog: [{ questId: 98, stage: 'readyToTurnIn' }],
  }, 98), 0);
});

test('q89 Wizard TownTeleport reserve is departure-only and map-0 gated', () => {
  const active = (mapFileName, stage = 'inProgress') => ({
    mapFileName,
    questLog: [{ questId: 89, stage }],
  });
  assert.equal(journeyEmergencyTownTeleportRestockTarget(active('D022'), 89, 'Wizard'), 0);
  assert.equal(journeyEmergencyTownTeleportRestockTarget(active('0'), 89, 'Wizard'), 2);
  assert.equal(journeyEmergencyTownTeleportRestockTarget(active('D022'), 89, 'Wizard', { force: true }), 2);
  assert.equal(journeyEmergencyTownTeleportRestockTarget(active('0'), 89, 'Taoist'), 0);
  assert.equal(journeyEmergencyTownTeleportRestockTarget(active('0', 'readyToTurnIn'), 89, 'Wizard'), 0);
});

test('TownTeleport departure proof triggers Scott reserve when map-0 stock is missing', async () => {
  const client = clientAt('0', 24);
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    minimumHpStock: 4,
    travel: async map => calls.push(['travel', map]),
    navigateNear: async () => {},
    restock: async (owner, _navigateNear, options) => {
      calls.push(['restock', options]);
      owner.snapshot.inventoryItems.push({ name: 'TownTeleport', quantity: 2 });
      return { status: 'restocked' };
    },
  });

  const result = await gate(client, {
    minimumHpStock: 4,
    minimumEmergencyTownTeleportStock: 2,
    requiredAfterRestockEmergencyTownTeleportStock: 2,
    returnMapFileName: '',
  });
  assert.equal(result.status, 'restocked');
  assert.deepEqual(calls, [
    ['restock', { targetHp: 4, emergencyTownTeleportCount: 2, lowStockHp: 4 }],
  ]);
});

test('TownTeleport departure proof fails closed when Scott purchase did not increase stock', async () => {
  const client = clientAt('0', 24);
  const gate = createPostEngagementSupplyGate({
    minimumHpStock: 4,
    travel: async () => {},
    navigateNear: async () => {},
    restock: async () => ({ status: 'restocked' }),
  });
  await assert.rejects(() => gate(client, {
    minimumHpStock: 4,
    minimumEmergencyTownTeleportStock: 2,
    requiredAfterRestockEmergencyTownTeleportStock: 2,
    returnMapFileName: '',
  }), /needsFunds: unable to restock supplies to TownTeleport 2/);
});

test('Wooma and Stone Tomb carry navigation plus combat escape reserves', () => {
  assert.equal(journeyEmergencyTeleportDepartureTarget(98), 8);
  assert.equal(journeyEmergencyTeleportDepartureTarget(99), 8);
  assert.equal(journeyEmergencyTeleportDepartureTarget(114), 8);
  assert.equal(journeyEmergencyTeleportDepartureTarget(113), 4);
  assert.equal(journeyEmergencyTeleportDepartureTarget(49), 0);
});

test('completed expedition objectives stop requiring departure supplies before turn-in', () => {
  assert.equal(journeyExpeditionSupplyActive({
    questLog: [{ questId: 60, stage: 'inProgress' }],
  }, 60), true);
  assert.equal(journeyExpeditionSupplyActive({
    questLog: [{ questId: 60, stage: 'readyToTurnIn' }],
  }, 60), false);
  assert.equal(journeyExpeditionSupplyActive({
    questLog: [{ questId: 60, stage: 'completed' }],
  }, 60), false);
  assert.equal(journeyExpeditionSupplyActive({
    questLog: [{ questId: 98, stage: 'inProgress' }],
  }, 98), true);
  assert.equal(journeyExpeditionSupplyActive({
    questLog: [{ questId: 113, stage: 'inProgress' }],
  }, 113), true);
  assert.equal(journeyExpeditionSupplyActive({
    questLog: [{ questId: 49, stage: 'inProgress' }],
  }, 49), false);
});

test('long expeditions accept a partial but still conservative funded departure stock', () => {
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(54, 'Wizard'), { hp: 64, mp: 64 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(54, 'Taoist'), { hp: 64, mp: 64 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(54, 'Warrior'), { hp: 64, mp: 0 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(62, 'Warrior'), { hp: 64, mp: 0 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(60, 'Wizard'), { hp: 64, mp: 0 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(65, 'Wizard'), { hp: 64, mp: 64 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(65, 'Taoist'), { hp: 64, mp: 64 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(98, 'Wizard'), { hp: 64, mp: 0 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(98, 'Taoist'), { hp: 64, mp: 12 });
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(113, 'Warrior'), { hp: 64, mp: 0 });
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

test('a field expedition does not turn around solely to replace one used escape scroll', async () => {
  const client = clientAt('D2041', 51);
  client.snapshot.inventoryItems.push({
    name: 'RandomTeleport',
    quantity: 3,
    container: 'bag1',
    tooltipSource: { info: { item_index: 717 } },
  });
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    minimumHpStock: 24,
    travel: async map => calls.push(['travel', map]),
    navigateNear: async () => {},
    restock: async () => calls.push(['restock']),
  });

  const result = await gate(client, {
    minimumHpStock: 24,
    minimumEmergencyTeleportStock: 0,
    requiredAfterRestockHpStock: 64,
    requiredAfterRestockEmergencyTeleportStock: 4,
  });

  assert.equal(result.status, 'sufficient');
  assert.equal(result.hp, 51);
  assert.deepEqual(calls, []);
});

test('an expedition refill fails closed when its emergency scroll reserve was not funded', async () => {
  const client = clientAt('D401', 80);
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    minimumHpStock: 24,
    travel: async map => {
      calls.push(['travel', map]);
      client.snapshot = { ...client.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async owner => {
      calls.push(['restock', owner.snapshot.mapFileName]);
      return { status: 'needsFunds', gold: 83 };
    },
  });

  await assert.rejects(() => gate(client, {
    forceRestock: true,
    requiredAfterRestockHpStock: 80,
    requiredAfterRestockEmergencyTeleportStock: 4,
    returnMapFileName: '',
  }), /needsFunds: unable to restock supplies to RandomTeleport 4/);
  assert.deepEqual(calls, [['travel', '0'], ['restock', '0']]);
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

test('HP Medium departure reserve is an opt-in trigger and does not become a field invariant', async () => {
  const client = clientAt('D2031', 6);
  client.snapshot.beltItems.push({ name: '(HP)DrugMedium', uniqueId: 100, quantity: 1 });
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    travel: async map => {
      calls.push(['travel', map]);
      client.snapshot = { ...client.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async owner => {
      calls.push(['restock', owner.snapshot.mapFileName]);
      owner.snapshot.beltItems = [{ name: '(HP)DrugMedium', uniqueId: 101, quantity: 6 }];
      return { status: 'restocked' };
    },
  });
  const result = await gate(client, {
    minimumHpStock: 4,
    minimumHpMediumStock: 3,
    requiredAfterRestockHpMediumStock: 6,
    returnMapFileName: '',
  });
  assert.equal(result.status, 'restocked');
  assert.equal(result.hpMedium, 6);
  assert.equal(result.hpMediumReady, true);
  assert.deepEqual(calls, [['travel', '0'], ['restock', '0']]);
  assert.equal(hpMediumDrugCount(client.snapshot), 6);
});

test('a q89 field pass can keep Small HP without a standalone Medium town loop', async () => {
  const client = clientAt('D2031', 6);
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    travel: async map => calls.push(['travel', map]),
    navigateNear: async () => {},
    restock: async () => {
      calls.push(['restock']);
      throw new Error('field-only Medium must not trigger restock');
    },
  });
  const result = await gate(client, {
    minimumHpStock: 4,
    minimumHpMediumStock: 0,
    requiredAfterRestockHpMediumStock: 6,
  });
  assert.equal(result.status, 'sufficient');
  assert.deepEqual(calls, []);
  assert.equal(hpMediumDrugCount(client.snapshot), 0);
});

test('q89 Wizard fresh CursedShaman recovery is limited to an injured Medium-equipped Wizard', () => {
  const snapshot = {
    playerHp: 85, playerMaxHp: 100,
    beltItems: [{ name: '(HP)DrugMedium', uniqueId: 74, quantity: 1 }],
    questLog: [{ questId: 89, stage: 'inProgress' }],
  };
  const shaman = { kind: 'monster', name: 'CursedShaman' };
  assert.deepEqual(q89WizardFreshCursedShamanRecoveryOptions(snapshot, shaman, 'Wizard'), {
    hpThreshold: 0.85, preferredHpPotion: 'medium', restorativeReuseDelayMs: 2_500,
  });
  assert.equal(q89WizardFreshCursedShamanRecoveryOptions(snapshot, shaman, 'Taoist'), null);
  assert.equal(q89WizardFreshCursedShamanRecoveryOptions(snapshot, { ...shaman, name: 'CursedZombie' }, 'Wizard'), null);
  assert.equal(q89WizardFreshCursedShamanRecoveryOptions({ ...snapshot, playerHp: 86 }, shaman, 'Wizard'), null);
  assert.equal(q89WizardFreshCursedShamanRecoveryOptions({ ...snapshot, beltItems: [] }, shaman, 'Wizard'), null);
  assert.equal(q89WizardFreshCursedShamanRecoveryOptions({ ...snapshot, questLog: [] }, shaman, 'Wizard'), null);
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

test('q89 casters retain four MP doses in the field but replenish twelve before leaving town', async () => {
  for (const className of ['Wizard', 'Taoist']) {
    const floor = journeyExpeditionDepartureFloorForQuest(89, className);
    const trigger = minimumJourneyMpStockForQuest(89, className);
    assert.deepEqual(floor, { hp: 64, mp: 12 });
    assert.equal(trigger, 4);
    const caster = clientAt('D2031', floor.hp);
    caster.snapshot.beltItems.push({ name: '(MP)DrugSmall', uniqueId: 900, quantity: 4 });
    const calls = [];
    const gate = createPostEngagementSupplyGate({
      travel: async map => calls.push(['travel', map]),
      navigateNear: async () => {},
      restock: async (owner, _navigateNear, options) => {
        calls.push(['restock', options]);
        owner.snapshot.beltItems.find(item => item.uniqueId === 900).quantity = 12;
        return { status: 'restocked' };
      },
    });
    const options = {
      minimumHpStock: 24,
      minimumMpStock: trigger,
      requiredAfterRestockHpStock: floor.hp,
      requiredAfterRestockMpStock: floor.mp,
      returnMapFileName: '',
    };
    assert.equal((await gate(caster, options)).status, 'sufficient');
    assert.deepEqual(calls, []);
    caster.snapshot.mapFileName = '0';
    const result = await gate(caster, {
      ...options,
      forceRestock: mpDrugCount(caster.snapshot) < floor.mp,
    });
    assert.equal(result.status, 'restocked');
    assert.equal(result.mp, 12);
    assert.equal(caster.snapshot.mapFileName, '0');
    assert.deepEqual(calls, [['restock', {
      targetHp: 64, targetMp: 12, lowStockHp: 64, lowStockMp: 12,
    }]]);
  }
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(89, 'Warrior'), { hp: 64, mp: 0 });
});

test('a Taoist expedition refills a full Amulet reserve at its lower field trigger', async () => {
  const taoist = clientAt('D2041', 24);
  taoist.snapshot.beltItems.push({ name: 'Amulet', quantity: 11 });
  const calls = [];
  const gate = createPostEngagementSupplyGate({
    travel: async map => {
      calls.push(['travel', map]);
      taoist.snapshot = { ...taoist.snapshot, mapFileName: map };
    },
    navigateNear: async () => {},
    restock: async (owner, _navigateNear, options) => {
      calls.push(['restock', options]);
      owner.snapshot.beltItems = owner.snapshot.beltItems
        .filter(item => String(item?.name ?? '') !== 'Amulet');
      owner.snapshot.beltItems.push({ name: 'Amulet', quantity: 32 });
      return { status: 'restocked' };
    },
  });

  const result = await gate(taoist, {
    minimumHpStock: 4,
    minimumAmuletStock: 12,
    requiredAfterRestockAmuletStock: 32,
  });
  assert.equal(result.status, 'restocked');
  assert.equal(result.amulet, 32);
  assert.deepEqual(calls, [
    ['travel', '0'],
    ['restock', {
      targetHp: 4,
      targetAmulet: 32,
      lowStockHp: 4,
      lowStockAmulet: 32,
    }],
    ['travel', 'D2041'],
  ]);
});

test('a Taoist expedition rejects an underfilled Amulet departure reserve', async () => {
  const taoist = clientAt('0', 24);
  taoist.snapshot.beltItems.push({ name: 'Amulet', quantity: 3 });
  const gate = createPostEngagementSupplyGate({
    travel: async () => {},
    navigateNear: async () => {},
    restock: async () => ({ status: 'needsFunds' }),
  });

  await assert.rejects(() => gate(taoist, {
    minimumAmuletStock: 12,
    requiredAfterRestockAmuletStock: 32,
  }), /needsFunds: unable to restock supplies to Amulet 32/);
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
    preferTravelAggressorCombat: true,
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

test('q60 ranged classes admit the measured D2041 SpiderFrog spawn density', () => {
  for (const className of ['Wizard', 'Taoist']) {
    const profile = questRetreatProfile(60, className);
    assert.equal(profile.continueTravelWhileHealthy, true);
    assert.equal(profile.maxTargetAdjacent, 1);
    assert.equal(profile.maxTargetNearby, 4);
  }
  assert.equal(questRetreatProfile(60, 'Warrior').continueTravelWhileHealthy, false);
  assert.equal(questRetreatProfile(60, 'Warrior').maxTargetAdjacent, undefined);
  assert.equal(questRetreatProfile(60, 'Warrior').maxTargetNearby, undefined);
});

test('q65 preserves D421 progress toward its D422 Zombie1 field', () => {
  assert.deepEqual(questRetreatBiasPosition(65, { mapFileName: 'D421' }, 'Warrior'), { x: 361, y: 19 });
  assert.deepEqual(questRetreatBiasPosition(65, { mapFileName: 'D421' }, 'Wizard'), { x: 30, y: 374 });
  assert.deepEqual(questRetreatBiasPosition(65, { mapFileName: 'D421' }, 'Taoist'), { x: 30, y: 374 });
  assert.deepEqual(questRetreatBiasPosition(65, { mapFileName: 'D401' }, 'Wizard'), { x: 76, y: 15 });
  assert.deepEqual(questRetreatBiasPosition(65, { mapFileName: 'D411' }, 'Taoist'), { x: 95, y: 49 });
  assert.equal(questRetreatBiasPosition(65, { mapFileName: 'D401' }, 'Warrior'), null);
  assert.equal(questRetreatBiasPosition(65, { mapFileName: 'D422' }), null);
  assert.equal(questRetreatBiasPosition(65, { mapFileName: '0' }), null);
  assert.deepEqual(preferredObjectiveMapsForQuest(65, 'Wizard'), ['D406']);
  assert.deepEqual(preferredObjectiveMapsForQuest(65, 'Taoist'), ['D406']);
  assert.deepEqual(preferredObjectiveMapsForQuest(65, 'Warrior'), []);
  assert.equal(shouldPreferObjectiveMapOverCurrent(65, {}, 'Wizard'), true);
  assert.equal(shouldPreferObjectiveMapOverCurrent(65, {}, 'Taoist'), true);
  assert.equal(shouldPreferObjectiveMapOverCurrent(65, {}, 'Warrior'), false);
  assert.deepEqual(questRetreatProfile(65, 'Taoist'), {
    allowLowHealthFollowerRecovery: true,
    continueTravelWhileHealthy: true,
    preferTravelAggressorCombat: true,
    maxTravelThreatEvasionsPerEdge: 3,
    maxRetreatBreakoutKills: 3,
    multiAggressorRetreatRatio: 0.75,
    retreatAtActiveAggressorCount: 1,
    unsafeRetreatSteps: 24,
    unsafeRetreatSafeDistance: 8,
    maxTargetAdjacent: 1,
    maxTargetNearby: 4,
  });
  assert.equal(questRetreatProfile(65, 'Wizard').maxTargetAdjacent, 1);
  assert.equal(questRetreatProfile(65, 'Wizard').maxTargetNearby, 4);
  assert.equal(questRetreatProfile(65, 'Warrior').maxTargetAdjacent, undefined);
});

test('q99 Warrior admits the measured final WoomaFighter spawn group', () => {
  const warrior = questRetreatProfile(99, 'Warrior');
  assert.equal(warrior.maxTargetAdjacent, 2);
  assert.equal(warrior.maxTargetNearby, 3);
  assert.equal(questRetreatProfile(99, 'Wizard').maxTargetAdjacent, 1);
  assert.equal(questRetreatProfile(99, 'Taoist').maxTargetNearby, 4);
});

test('q89 Wizard clears one entrance blocker while retaining its retreat gates', () => {
  const profile = questRetreatProfile(89, 'Wizard');
  assert.equal(profile.maxTargetAdjacent, 0);
  assert.equal(profile.maxTargetNearby, 2);
  assert.equal(profile.directAggressorDistance, 8);
  assert.equal(profile.multiAggressorRetreatRatio, 0.75);
  assert.equal(profile.retreatAtActiveAggressorCount, 2);
  assert.equal(profile.unsafeRetreatSafeDistance, 8);
  assert.equal(profile.focusTargetThroughAggressors, false);
  assert.equal(profile.emergencyEscapeRequiresProvenAggressors, true);
  assert.equal(profile.finishableTargetHealthRatio, 0.25);
  assert.equal(profile.finishableTargetMinimumPlayerHpRatio, 0.7);
  assert.equal(journeyEmergencyTeleportDepartureTarget(89), 8);
  assert.equal(questEmergencyEscapeHpRatio(89, 'Wizard'), 0.65);
});

test('q89 Wizard uses a held ordinary RandomTeleport in the living D2031 60-percent band only', () => {
  const active = {
    playerObjectId: 1000,
    playerHp: 62,
    playerMaxHp: 100,
    mapFileName: 'D2031',
    entities: [{ objectId: 1000, kind: 'selfPlayer', dead: false, hp: 62, maxHp: 100 }],
    questLog: [{ questId: 89, stage: 'InProgress' }],
  };
  assert.equal(shouldUseQ89WizardSameMapRandomEscape(active, 8), true);
  assert.equal(shouldUseQ89WizardSameMapRandomEscape({ ...active, playerHp: 59 }, 8), false,
    'Town remains first at the critical floor');
  assert.equal(shouldUseQ89WizardSameMapRandomEscape({ ...active, playerHp: 70 }, 8), false,
    'the policy cannot start an unneeded healthy relocation');
  assert.equal(shouldUseQ89WizardSameMapRandomEscape({ ...active, mapFileName: 'D2032' }, 8), false);
  assert.equal(shouldUseQ89WizardSameMapRandomEscape(active, 0), false);
  assert.equal(shouldUseQ89WizardSameMapRandomEscape({
    ...active,
    playerHp: 0,
    entities: [{ ...active.entities[0], dead: true, hp: 0 }],
  }, 8), false, 'a dead player never issues an escape item');
  assert.equal(shouldUseQ89WizardSameMapRandomEscape({
    ...active,
    questLog: [{ questId: 89, stage: 'ReadyToTurnIn' }],
  }, 8), false);
});

test('runner dispatches the q89 same-map RandomTeleport policy before Town only in its living band', async () => {
  const start = runnerSource.indexOf('    const emergencyTeleport = createRandomTeleportEmergencyEscape({');
  const end = runnerSource.indexOf('    const journeyCombatAction =', start);
  assert.ok(start >= 0 && end > start, 'runner must retain its emergency teleport policy block');
  const snippet = runnerSource.slice(start, end).replace(/^    /gm, '');
  const snapshotAt = hp => ({
    playerObjectId: 1000, playerHp: hp, playerMaxHp: 100, mapFileName: 'D2031',
    entities: [{ objectId: 1000, kind: 'selfPlayer', dead: false, hp, maxHp: 100 }],
    questLog: [{ questId: 89, stage: 'InProgress' }],
  });
  const dispatch = async (hp, random = 8) => {
    const calls = [];
    const emergencyTeleport = vm.runInNewContext(`(() => { ${snippet}; return emergencyTeleport; })()`, {
      className: 'Wizard',
      observedPlayerHp,
      journeyEmergencyTeleportCriticalHpRatio: () => 0.65,
      isWizardQ89Expedition: snapshot => (snapshot.questLog ?? []).some(entry =>
        Number(entry.questId) === 89 && String(entry.stage).toLowerCase() === 'inprogress'),
      shouldUseQ89WizardSameMapRandomEscape,
      randomTeleportCount: () => random,
      townTeleportCount: () => 2,
      useRandomTeleport: async () => { calls.push('random'); return { item: 'RandomTeleport' }; },
      useTownTeleport: async () => { calls.push('town'); return { item: 'TownTeleport' }; },
      createRandomTeleportEmergencyEscape: ({ teleport }) => teleport,
      Math,
      Number,
    });
    await emergencyTeleport({ snapshot: snapshotAt(hp) });
    return calls;
  };
  assert.deepEqual(await dispatch(62), ['random']);
  assert.deepEqual(await dispatch(59), ['town']);
  assert.deepEqual(await dispatch(62, 0), ['town']);
});

test('q89 focus relaxation is isolated from other ranged expeditions', () => {
  assert.equal(questRetreatProfile(89, 'Taoist').focusTargetThroughAggressors, undefined);
  assert.equal(questRetreatProfile(89, 'Taoist').emergencyEscapeRequiresProvenAggressors, undefined);
  assert.equal(questRetreatProfile(98, 'Wizard').focusTargetThroughAggressors, true);
  assert.equal(questRetreatProfile(98, 'Wizard').emergencyEscapeRequiresProvenAggressors, undefined);
  assert.equal(questRetreatProfile(99, 'Wizard').focusTargetThroughAggressors, true);
});

test('q98 Wooma expedition keeps long wide recovery armed for ranged classes', () => {
  for (const className of ['Wizard', 'Taoist']) {
    const profile = questRetreatProfile(98, className);
    assert.equal(profile.allowLowHealthFollowerRecovery, true);
    assert.equal(profile.continueTravelWhileHealthy, true);
    assert.equal(profile.maxTargetAdjacent, 1);
    assert.equal(profile.maxTargetNearby, 4);
  }
  assert.equal(questRetreatProfile(98, 'Warrior').continueTravelWhileHealthy, false);
  assert.equal(questRetreatProfile(98, 'Warrior').maxTargetAdjacent, undefined);
  assert.equal(evasiveRecoveryTimeoutMsForQuest(98), 90_000);
  assert.equal(evasiveRecoveryDangerDistanceForQuest(98), 8);
});

test('q99 keeps the same Wooma expedition recovery and supplies as q98', () => {
  for (const className of ['Wizard', 'Taoist']) {
    const profile = questRetreatProfile(99, className);
    assert.equal(profile.allowLowHealthFollowerRecovery, true);
    assert.equal(profile.continueTravelWhileHealthy, true);
    assert.equal(profile.maxTargetAdjacent, 1);
    assert.equal(profile.maxTargetNearby, 4);
  }
  assert.equal(evasiveRecoveryTimeoutMsForQuest(99), 90_000);
  assert.equal(evasiveRecoveryDangerDistanceForQuest(99), 8);
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(99, 'Wizard'), { hp: 64, mp: 0 });
  assert.equal(questSpawnSearchHostileClearanceFallback(98), 0);
  assert.equal(questSpawnSearchHostileClearanceFallback(99), 0);
  assert.equal(questSpawnSearchHostileClearanceFallback(54), 0);
  assert.equal(questSpawnSearchHostileClearanceFallback(30), 2);
  assert.equal(questSpawnSearchHostileClearanceFallback(42), 1);
});

test('ranged Wooma hunts finish a nearly dead target only while the caster is healthy', () => {
  for (const questId of [98, 99]) {
    for (const className of ['Wizard', 'Taoist']) {
      const profile = questRetreatProfile(questId, className);
      assert.equal(profile.focusTargetThroughAggressors, true);
      assert.equal(profile.finishableTargetHealthRatio, 0.25);
      assert.equal(profile.finishableTargetMinimumPlayerHpRatio, 0.7);
    }
  }
});

test('q122/q123 Warrior resumes a damaged live Prajna objective through the measured pack', () => {
  for (const questId of [122, 123]) {
    const profile = questRetreatProfile(questId, 'Warrior');
    assert.equal(profile.maxTargetAdjacent, 2);
    assert.equal(profile.maxTargetNearby, 5);
    assert.equal(profile.focusTargetThroughAggressors, true);
    assert.equal(profile.finishableTargetHealthRatio, 0.5);
    assert.equal(profile.finishableTargetMinimumPlayerHpRatio, 0.65);
    assert.equal(profile.allowLowHealthFollowerRecovery, true);
  }
});

test('q118 Warrior admits the measured Mineral Mine pack for ordinary aggressor clearing', () => {
  const profile = questRetreatProfile(118, 'Warrior');
  assert.equal(profile.maxTargetAdjacent, 2);
  assert.equal(profile.maxTargetNearby, 5);
  assert.equal(profile.focusTargetThroughAggressors, undefined);
});

test('q113 deep Bug Cave hunt uses expedition recovery and ranged density limits', () => {
  for (const className of ['Wizard', 'Taoist']) {
    const profile = questRetreatProfile(113, className);
    assert.equal(profile.allowLowHealthFollowerRecovery, true);
    assert.equal(profile.continueTravelWhileHealthy, true);
    assert.equal(profile.maxTargetAdjacent, 1);
    assert.equal(profile.maxTargetNearby, 4);
  }
  assert.equal(evasiveRecoveryTimeoutMsForQuest(113), 90_000);
  assert.equal(evasiveRecoveryDangerDistanceForQuest(113), 8);
});

test('q114 Stone Tomb boars keep expedition supplies and wide recovery armed', () => {
  const profile = questRetreatProfile(114, 'Warrior');
  assert.equal(profile.allowLowHealthFollowerRecovery, true);
  assert.equal(evasiveRecoveryTimeoutMsForQuest(114), 90_000);
  assert.equal(evasiveRecoveryDangerDistanceForQuest(114), 8);
  assert.deepEqual(journeyExpeditionDepartureFloorForQuest(114, 'Warrior'), { hp: 64, mp: 0 });
});

test('q89 Taoist admits the measured 0-adjacent/2-nearby Priest without inheriting Wizard safety thresholds', () => {
  const profile = questRetreatProfile(89, 'Taoist');
  assert.equal(profile.continueTravelWhileHealthy, true);
  assert.equal(profile.maxTargetAdjacent, 0);
  assert.equal(profile.maxTargetNearby, 2);
  assert.equal(profile.directAggressorDistance, undefined);
  assert.equal(profile.multiAggressorRetreatRatio, undefined);
  assert.equal(profile.retreatAtActiveAggressorCount, undefined);
  assert.equal(profile.focusTargetThroughAggressors, undefined);
  assert.equal(profile.emergencyEscapeRequiresProvenAggressors, undefined);
  assert.equal(questRetreatProfile(89, 'Wizard').continueTravelWhileHealthy, true);
  assert.equal(questRetreatProfile(89, 'Wizard').directAggressorDistance, 8);
  assert.equal(questRetreatProfile(89, 'Warrior').continueTravelWhileHealthy, false);
  assert.equal(questRetreatProfile(89, 'Warrior').maxTargetAdjacent, undefined);
  assert.equal(questRetreatProfile(89, 'Warrior').maxTargetNearby, undefined);
  assert.equal(questRetreatProfile(42, 'Taoist').continueTravelWhileHealthy, false);
  assert.equal(questRetreatProfile(42, 'Taoist').maxTargetAdjacent, undefined);
  assert.equal(questRetreatProfile(42, 'Taoist').maxTargetNearby, undefined);
});

test('q54 keeps Warrior mine thresholds and gives a trapped healing Taoist bounded breakout room', () => {
  assert.deepEqual(questRetreatProfile(54, 'Warrior'), {
    allowLowHealthFollowerRecovery: true,
    continueTravelWhileHealthy: true,
    preferTravelAggressorCombat: true,
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
    assert.equal(profile.preferTravelAggressorCombat, true);
    assert.equal(profile.maxRetreatBreakoutKills, className === 'Taoist' ? 3 : 1);
    assert.equal(profile.multiAggressorRetreatRatio, 0.75);
    assert.equal(profile.retreatAtActiveAggressorCount, 1);
    assert.equal(profile.unsafeRetreatSteps, 24);
    assert.equal(profile.unsafeRetreatSafeDistance, 8);
  }
});

test('q54 finishes available D401 zombies before preferring the D406 source', () => {
  assert.equal(shouldPreferObjectiveMapOverCurrent(54), false);
  assert.equal(shouldPreferObjectiveMapOverCurrent(54, {
    objectives: [
      { label: 'Kill Zombie5', current: 4, required: 5 },
      { label: 'Kill Zombie1', current: 0, required: 5 },
    ],
  }), false);
  assert.equal(shouldPreferObjectiveMapOverCurrent(54, {
    objectives: [
      { label: 'Kill Zombie5', current: 0, required: 5 },
      { label: 'Kill Zombie1', current: 0, required: 5 },
    ],
  }, 'Wizard'), true);
  assert.equal(shouldPreferObjectiveMapOverCurrent(54, {
    objectives: [
      { label: 'Kill Zombie3', current: 3, required: 5 },
      { label: 'Kill Zombie1', current: 0, required: 5 },
    ],
  }, 'Taoist'), true);
  assert.equal(shouldPreferObjectiveMapOverCurrent(54, {
    objectives: [
      { label: 'Kill Zombie5', current: 5, required: 5 },
      { label: 'Kill Zombie2', current: 5, required: 5 },
      { label: 'Kill Zombie3', current: 5, required: 5 },
      { label: 'Kill Zombie4', current: 5, required: 5 },
      { label: 'Kill Zombie1', current: 2, required: 5 },
    ],
  }), true);
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

test('safe-zone recovery ignores neutral residents and does not spend an escape', async () => {
  const client = clientAt('0', 0);
  Object.assign(client.snapshot, {
    inSafeZone: true,
    playerHp: 20,
    playerMaxHp: 100,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', x: 288, y: 616, hp: 20, dead: false },
      { objectId: 61, kind: 'monster', disposition: 'neutral', name: 'Guard', x: 289, y: 616, hp: 48 },
      { objectId: 62, kind: 'monster', disposition: 'neutral', name: 'Deer', x: 288, y: 617, hp: 48 },
      { objectId: 63, kind: 'monster', disposition: 'neutral', name: 'Scarecrow', x: 289, y: 617, hp: 48 },
    ],
  });
  let escapeCalls = 0;
  let sustainCalls = 0;
  const result = await recoverHealthWhileEvading(client, async () => {
    throw new Error('neutral safe-zone residents must not trigger retreat movement');
  }, {
    requiredRatio: 0.75,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscape: async () => { escapeCalls += 1; return { success: true }; },
    sustain: async owner => {
      sustainCalls += 1;
      owner.snapshot.playerHp = 80;
      owner.snapshot.entities[0].hp = 80;
    },
  });

  assert.equal(result.hp, 80);
  assert.equal(escapeCalls, 0);
  assert.equal(sustainCalls, 1);
  assert.equal(result.evasiveMoves, 0);
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
