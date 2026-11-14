import test from 'node:test';
import assert from 'node:assert/strict';
import { CAPACITY_TARGET_STAGES, capacityActorLimit, capacityPreparationPolicy, capacityPolicy, validateCapacityPool } from './playtest-capacity-soak-core.mjs';

const endpoint = 'ws://127.0.0.1:17510/ws';
const pool = count => ({ schemaVersion: 1, purpose: 'isolated-load-actors', realm: 'isolated', endpoint,
  accounts: Array.from({ length: count }, (_, i) => ({ accountId: 'ltabcdef' + i.toString(36).padStart(2, '0'),
    name: 'Labcdef' + i.toString(36).padStart(2, '0'), password: 'private-pass-123456', state: 'registered', provisioning: 'test-ordinary-registration' })) });

test('legacy capacity policies keep their defaults, actor ceiling and report metadata', () => {
  const legacy = capacityPolicy();
  assert.deepEqual(legacy.stages, [10, 20, 30, 40, 50]);
  assert.equal(legacy.activityMode, 'baseline');
  assert.equal(legacy.combatActors, 0);
  assert.equal(legacy.stageSeconds, 300);
  assert.equal(legacy.soakSeconds, 3600);
  assert.equal(legacy.wallSeconds, 10800);
  for (const key of ['targetActors', 'preparationProfile', 'preparationHpTarget']) assert.equal(Object.hasOwn(legacy, key), false);
  assert.deepEqual(capacityPolicy(JSON.parse(JSON.stringify(legacy))), legacy);
  assert.equal(capacityActorLimit(), 100);
  assert.equal(capacityPolicy({ activityMode: 'native', stages: '100', combatActors: 25 }).combatActors, 25);
  assert.throws(() => capacityPolicy({ stages: '250' }), /maximum of 100/);
  assert.throws(() => capacityPolicy({ stages: '500' }), /maximum of 100/);
  assert.throws(() => capacityPolicy({ activityMode: 'native', stages: '100', combatActors: 26 }), /finite scenario/);
});

test('the explicit 500 target declares finite native stages and a fifty-fighter default', () => {
  const policy = capacityPolicy({ targetActors: '500' });
  assert.equal(capacityActorLimit(policy), 500);
  assert.equal(policy.targetActors, 500);
  assert.deepEqual(policy.stages, [2, 10, 25, 50, 100, 250, 500]);
  assert.deepEqual(policy.stages, CAPACITY_TARGET_STAGES);
  assert.equal(policy.profile, 'standard');
  assert.equal(policy.activityMode, 'native');
  assert.equal(policy.combatActors, 50);
  assert.equal(policy.stageSeconds, 300);
  assert.equal(policy.soakSeconds, 7200);
  assert.equal(policy.wallSeconds, 21600);
  assert.equal(policy.preparationProfile, 'held-only');
  assert.equal(policy.preparationHpTarget, 0);
  assert.equal(Object.isFrozen(policy), true);
  // Report policy serialization must retain opt-in intent and a literal role
  // count; a target is configuration, never evidence of achieved capacity.
  const report = JSON.parse(JSON.stringify({ policy, capacityAccepted: false, highestPassedStage: 100 }));
  assert.deepEqual(capacityPolicy(report.policy), policy);
  assert.equal(report.capacityAccepted, false);
  assert.equal(report.highestPassedStage, 100);
  assert.equal(capacityPolicy({ targetActors: 500, stages: '250' }).combatActors, 25);
  assert.equal(capacityPolicy({ targetActors: 500, stages: '100' }).stages.at(-1), 100);
  assert.equal(capacityPolicy({ targetActors: 500, combatActors: 125 }).combatActors, 125);
});

test('the 500 target retains original latency, activity, resource and command safeguards', () => {
  const legacy = capacityPolicy({ activityMode: 'native' }), target = capacityPolicy({ targetActors: 500 });
  for (const key of ['commandsPerSecond', 'actionTimeoutMs', 'maximumSendLatenessMs', 'movementP95Ms', 'chatP95Ms',
    'timeoutExclusive', 'maximumMissedPlanRatio', 'maximumCorrectionRatio', 'minimumMovesPerMinute',
    'minimumCombatHitsPerMinute', 'minimumCombatAttacksPerMinute', 'aoiRange', 'aoiGraceMs', 'nativeChatIntervalMs',
    'nativeTurnIntervalMs', 'minimumNativeRunRatio', 'minimumNativeCellsPerMinute', 'maximumEvidenceInFlight',
    'maximumMemorySlopeMiBPerHour', 'memoryWarmupSeconds', 'maximumDriverLagP99Ms', 'resumeSamples', 'saveSamples']) {
    assert.equal(target[key], legacy[key], key);
  }
  for (const changes of [{ stages: '501' }, { stages: '500,250' }, { profile: 'probe' }, { profile: 'movement' },
    { activityMode: 'baseline' }, { stageSeconds: 120 }, { stageSeconds: 600 }, { soakSeconds: 3600 }, { soakSeconds: 14400 },
    { wallSeconds: Infinity }, { wallSeconds: 43201 }, { combatActors: 49 }, { combatActors: 126 }, { combatActors: 500 },
    { resumeSamples: 0 }, { saveSamples: 0 }, { movementIntervalMs: 1000 }]) {
    assert.throws(() => capacityPolicy({ targetActors: 500, ...changes }), JSON.stringify(changes));
  }
  for (const targetActors of [0, 100, 250, 501, Infinity, NaN, null, true, 'unknown']) assert.throws(() => capacityPolicy({ targetActors }));
});

test('large account pools require the explicit target and retain dedicated account validation', () => {
  const original = pool(100), large = pool(500);
  assert.equal(validateCapacityPool(original, endpoint), original);
  assert.throws(() => validateCapacityPool(large, endpoint), /Invalid isolated capacity account pool/);
  assert.equal(validateCapacityPool(large, endpoint, capacityPolicy({ targetActors: 500 })), large);
  assert.throws(() => validateCapacityPool(pool(501), endpoint, { targetActors: 500 }), /Invalid isolated capacity account pool/);
  const duplicate = structuredClone(large); duplicate.accounts[499] = duplicate.accounts[0];
  assert.throws(() => validateCapacityPool(duplicate, endpoint, { targetActors: 500 }), /Duplicate capacity actor/);
  const publicAccount = structuredClone(large); publicAccount.accounts[499].accountId = 'normal-account';
  assert.throws(() => validateCapacityPool(publicAccount, endpoint, { targetActors: 500 }), /pre-provisioned dedicated lt actors/);
  assert.throws(() => validateCapacityPool(large, endpoint, { targetActors: 100 }), /explicit capacity target/);
});

test('ordinary economic preparation is an explicitly named bounded plan and cannot rewrite old held-only reports', () => {
  assert.deepEqual(capacityPreparationPolicy(), { preparationProfile: 'held-only', preparationHpTarget: 0 });
  assert.deepEqual(capacityPreparationPolicy({ preparationProfile: 'ordinary-economic-v1' }),
    { preparationProfile: 'ordinary-economic-v1', preparationHpTarget: 12 });
  const policy = capacityPolicy({ targetActors: 500, preparationProfile: 'ordinary-economic-v1', preparationHpTarget: '40' });
  assert.equal(policy.preparationHpTarget, 40);
  assert.equal(policy.preparationProfile, 'ordinary-economic-v1');
  assert.deepEqual(capacityPolicy(JSON.parse(JSON.stringify(policy))), policy);
  for (const options of [{ preparationProfile: 'unknown' }, { preparationProfile: 'held-only', preparationHpTarget: 12 },
    { preparationProfile: 'ordinary-economic-v1', preparationHpTarget: 0 }, { preparationProfile: 'ordinary-economic-v1', preparationHpTarget: 41 },
    { preparationProfile: 'ordinary-economic-v1', preparationHpTarget: 1.5 }, { preparationProfile: 'ordinary-economic-v1', preparationHpTarget: Infinity }]) {
    assert.throws(() => capacityPolicy(options), JSON.stringify(options));
  }
  assert.equal(capacityPolicy({ preparationProfile: 'ordinary-economic-v1' }).preparationHpTarget, 12);
  const historical = JSON.parse(JSON.stringify({ policy: capacityPolicy(), highestPassedStage: 50 }));
  assert.equal(Object.hasOwn(historical.policy, 'preparationProfile'), false);
  assert.equal(Object.hasOwn(historical, 'preparation'), false);
});
