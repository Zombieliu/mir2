import test from 'node:test';
import assert from 'node:assert/strict';
import { GATEWAY_BUCKET_UPPER_US, publicZoneMetrics, zoneMetricsDelta } from './playtest-capacity-telemetry.mjs';

function histogram(durations = []) {
  const buckets = GATEWAY_BUCKET_UPPER_US.map(upperBoundUs => ({ upperBoundUs, count: 0 }));
  for (const duration of durations) buckets.find(bucket => bucket.upperBoundUs == null || bucket.upperBoundUs >= duration).count++;
  const sorted = [...durations].sort((a, b) => a - b);
  const quantile = p => sorted.length ? buckets.find(bucket => bucket.upperBoundUs == null || bucket.upperBoundUs >= sorted[Math.ceil(sorted.length * p) - 1]).upperBoundUs : null;
  return { count: durations.length, sumUs: durations.reduce((a, b) => a + b, 0), maxUs: Math.max(0, ...durations),
    overBudgetCount: durations.filter(value => value > 50000).length, buckets, saturated: false, discardedSamples: 0,
    p50UpperUs: quantile(.5), p95UpperUs: quantile(.95), p99UpperUs: quantile(.99), p999UpperUs: quantile(.999) };
}
function snapshot(durations = [], since = 1000) {
  return { version: 2, scope: 'gateway shared-owner transaction, may contain multiple maps', budgetUs: 50000,
    complete: true, sinceInitializationMs: since, immediateRequests: { successCount: durations.length, failureCount: 0, duration: histogram(durations) },
    lifecycleRequests: { successCount: 0, failureCount: 0, duration: histogram() },
    movementAdmissions: { successCount: 0, failureCount: 0, duration: histogram() },
    zoneWork: histogram(durations), ownerTickDeadlineLag: histogram(), pendingMovementDeadlineLag: histogram() };
}

test('old development endpoints explicitly lack telemetry; a required frozen profile fails closed', () => {
  assert.equal(publicZoneMetrics(undefined), null);
  assert.throws(() => publicZoneMetrics(undefined, { required: true }), /requires Gateway/);
  assert.equal(publicZoneMetrics(snapshot()).zoneWork.count, 0);
});

test('the complete fixed histogram and counters survive public whitelist projection', () => {
  const value = snapshot([1, 50001, 100000, 30000001]); value.password = 'must not copy';
  const projected = publicZoneMetrics(value);
  assert.deepEqual(projected.zoneWork, value.zoneWork);
  assert.equal(projected.password, undefined); assert.equal(JSON.stringify(projected).includes('must not copy'), false);
  assert.equal(projected.zoneWork.p999UpperUs, null, 'Overflow is not assigned a false finite upper bound');
});

test('window percentiles subtract raw bucket counts rather than lifetime percentile/max values', () => {
  const old = Array(100).fill(100000), recent = Array(1000).fill(100);
  const before = snapshot(old, 1000), after = snapshot([...old, ...recent], 61000);
  assert.equal(after.zoneWork.p99UpperUs, 100000);
  const delta = zoneMetricsDelta(before, after);
  assert.equal(delta.zoneWork.count, 1000); assert.equal(delta.zoneWork.p99UpperUs, 100);
  assert.equal(delta.zoneWork.p999UpperUs, 100); assert.equal(delta.zoneWork.maxUs, null);
  assert.equal(delta.zoneWork.processLifetimeMaxBeforeUs, 100000); assert.equal(delta.zoneWork.processLifetimeMaxAfterUs, 100000);
  assert.equal(delta.zoneWork.overBudgetCount, 0); assert.equal(delta.durationMs, 60000);
});

test('profile deltas keep failed completions and exact over-budget fractions', () => {
  const before = snapshot([10]); const after = snapshot([10, 10, 50001], 2000);
  after.immediateRequests.successCount = 2; after.immediateRequests.failureCount = 1;
  const delta = zoneMetricsDelta(before, after);
  assert.equal(delta.immediateRequests.successCount, 1); assert.equal(delta.immediateRequests.failureCount, 1);
  assert.equal(delta.zoneWork.count, 2); assert.equal(delta.zoneWork.overBudgetCount, 1); assert.equal(delta.zoneWork.overBudgetRatio, .5);
});

test('empty windows and overflow retain null quantiles instead of passing as zero latency', () => {
  const before = snapshot([1]), after = snapshot([1], 2000), empty = zoneMetricsDelta(before, after);
  assert.equal(empty.zoneWork.count, 0); assert.equal(empty.zoneWork.p99UpperUs, null); assert.equal(empty.zoneWork.overBudgetRatio, null);
  const overflow = zoneMetricsDelta(before, snapshot([1, 30000001], 3000));
  assert.equal(overflow.zoneWork.p99UpperUs, null); assert.equal(overflow.zoneWork.buckets.at(-1).count, 1);
});

test('unknown versions, incomplete telemetry, saturation, unsafe integers and inconsistent buckets are rejected', () => {
  for (const mutate of [value => { value.version = 1; }, value => { value.version = 3; }, value => { value.complete = false; },
    value => { value.scope = 'independent Cell'; }, value => { value.budgetUs = 100000; },
    value => { value.zoneWork.saturated = true; }, value => { value.zoneWork.discardedSamples = 1; },
    value => { value.zoneWork.count = Number.MAX_SAFE_INTEGER + 1; }, value => { value.zoneWork.buckets[0].count++; },
    value => { value.zoneWork.buckets[0].upperBoundUs = 99; }, value => { value.zoneWork.p99UpperUs = 0; },
    value => { value.immediateRequests.failureCount++; }]) {
    const value = snapshot([100]); mutate(value); assert.throws(() => publicZoneMetrics(value));
  }
});

test('movement admission has its own counters and never becomes proof of position application', () => {
  const before = snapshot(), after = snapshot([], 2000);
  after.movementAdmissions = { successCount: 2, failureCount: 1, duration: histogram([100, 200, 300]) };
  const delta = zoneMetricsDelta(before, after);
  assert.equal(delta.movementAdmissions.successCount, 2); assert.equal(delta.movementAdmissions.failureCount, 1);
  assert.equal(delta.movementAdmissions.duration.count, 3); assert.equal(delta.immediateRequests.duration.count, 0);
  assert.match(delta.movementAdmissionsDefinition, /not proof/); assert.match(delta.pendingMovementDeadlineLagDefinition, /not a movement ACK/);
});

test('counter resets, bucket redistribution and process-lifetime reversal invalidate a profile delta', () => {
  assert.throws(() => zoneMetricsDelta(snapshot([100], 2000), snapshot([100], 1000)), /lifetime/);
  assert.throws(() => zoneMetricsDelta(snapshot([100, 1000]), snapshot([100], 2000)), /backwards/);
  assert.throws(() => zoneMetricsDelta(snapshot([100]), snapshot([1000], 2000)), /backwards/);
});
