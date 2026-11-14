// Public, bounded Gateway telemetry only. No packet/request/account identifiers.
// This schema is paired with apps/gateway/src/zone_metrics.rs version 2.
export const GATEWAY_BUCKET_UPPER_US = Object.freeze([100, 250, 500, 1000, 2000, 5000, 10000, 20000,
  35000, 50000, 75000, 100000, 150000, 250000, 500000, 1000000, 5000000, 30000000, null]);
const SCOPE = 'gateway shared-owner transaction, may contain multiple maps';
const HISTOGRAMS = ['zoneWork', 'ownerTickDeadlineLag', 'pendingMovementDeadlineLag'];
const REQUESTS = ['immediateRequests', 'lifecycleRequests', 'movementAdmissions'];
const QUANTILES = [['p50UpperUs', 1n, 2n], ['p95UpperUs', 95n, 100n],
  ['p99UpperUs', 99n, 100n], ['p999UpperUs', 999n, 1000n]];
const counter = value => Number.isSafeInteger(value) && value >= 0;

function upperQuantile(buckets, count, numerator, denominator) {
  if (!count) return null;
  const rank = (BigInt(count) * numerator + denominator - 1n) / denominator;
  let cumulative = 0n;
  for (const bucket of buckets) {
    cumulative += BigInt(bucket.count);
    if (cumulative >= rank) return bucket.upperBoundUs;
  }
  throw new Error('Gateway histogram buckets do not contain their claimed count');
}

function histogram(value) {
  if (!value || !['count', 'sumUs', 'maxUs', 'overBudgetCount', 'discardedSamples'].every(key => counter(value[key])) ||
      typeof value.saturated !== 'boolean' || value.overBudgetCount > value.count ||
      !Array.isArray(value.buckets) || value.buckets.length !== GATEWAY_BUCKET_UPPER_US.length ||
      !value.buckets.every((bucket, index) => bucket.upperBoundUs === GATEWAY_BUCKET_UPPER_US[index] && counter(bucket.count)) ||
      value.buckets.reduce((sum, bucket) => sum + BigInt(bucket.count), 0n) !== BigInt(value.count)) {
    throw new Error('Gateway histogram schema, finite counters or bucket total is invalid');
  }
  if (value.saturated || value.discardedSamples !== 0) throw new Error('Gateway telemetry counters are incomplete or saturated');
  const result = Object.fromEntries(['count', 'sumUs', 'maxUs', 'overBudgetCount', 'saturated', 'discardedSamples'].map(key => [key, value[key]]));
  result.buckets = value.buckets.map(bucket => ({ upperBoundUs: bucket.upperBoundUs, count: bucket.count }));
  for (const [key, numerator, denominator] of QUANTILES) {
    const expected = upperQuantile(result.buckets, result.count, numerator, denominator);
    if (value[key] !== expected) throw new Error('Gateway lifetime quantile disagrees with its raw buckets');
    result[key] = expected;
  }
  return result;
}

export function publicZoneMetrics(value, { required = false } = {}) {
  if (value == null) {
    if (required) throw new Error('Frozen profile requires Gateway zone_metrics; endpoint has none');
    return null;
  }
  if (value.version !== 2 || value.scope !== SCOPE || value.budgetUs !== 50000 ||
      !counter(value.sinceInitializationMs) || value.complete !== true) {
    throw new Error('Gateway zone_metrics version, scope, budget or completeness is invalid');
  }
  const result = { version: value.version, scope: value.scope, sinceInitializationMs: value.sinceInitializationMs,
    budgetUs: value.budgetUs, complete: true };
  for (const key of HISTOGRAMS) result[key] = histogram(value[key]);
  for (const key of REQUESTS) {
    const request = value[key];
    if (!counter(request?.successCount) || !counter(request?.failureCount) ||
        BigInt(request.successCount) + BigInt(request.failureCount) !== BigInt(request.duration?.count ?? -1)) {
      throw new Error('Gateway request completion counters disagree with duration count');
    }
    result[key] = { successCount: request.successCount, failureCount: request.failureCount, duration: histogram(request.duration) };
  }
  return result;
}

function difference(after, before, key) {
  const value = after[key] - before[key];
  if (!counter(value)) throw new Error(`Gateway cumulative counter reset or moved backwards: ${key}`);
  return value;
}

function histogramDelta(before, after) {
  const count = difference(after, before, 'count');
  const buckets = after.buckets.map((bucket, index) => ({ upperBoundUs: bucket.upperBoundUs,
    count: difference(bucket, before.buckets[index], 'count') }));
  if (buckets.reduce((sum, bucket) => sum + BigInt(bucket.count), 0n) !== BigInt(count)) throw new Error('Gateway delta bucket totals disagree');
  const result = { count, sumUs: difference(after, before, 'sumUs'), overBudgetCount: difference(after, before, 'overBudgetCount'),
    buckets, maxUs: null, maxUsDefinition: 'A cumulative maximum cannot be subtracted into an exact window maximum',
    processLifetimeMaxBeforeUs: before.maxUs, processLifetimeMaxAfterUs: after.maxUs,
    saturated: false, discardedSamples: 0 };
  if (after.maxUs < before.maxUs || result.overBudgetCount > count) throw new Error('Gateway cumulative histogram continuity lost');
  for (const [key, numerator, denominator] of QUANTILES) result[key] = upperQuantile(buckets, count, numerator, denominator);
  result.overBudgetRatio = count ? result.overBudgetCount / count : null;
  return result;
}

export function zoneMetricsDelta(beforeValue, afterValue) {
  const before = publicZoneMetrics(beforeValue, { required: true }), after = publicZoneMetrics(afterValue, { required: true });
  if (after.sinceInitializationMs < before.sinceInitializationMs) throw new Error('Gateway telemetry process lifetime moved backwards');
  const result = { version: 2, scope: SCOPE, budgetUs: 50000, complete: true,
    fromSinceInitializationMs: before.sinceInitializationMs, toSinceInitializationMs: after.sinceInitializationMs,
    durationMs: after.sinceInitializationMs - before.sinceInitializationMs,
    definition: 'Disjoint cumulative bucket/counter subtraction from this profile starting snapshot; quantiles are bucket upper bounds; null may mean empty or overflow, never zero latency',
    movementAdmissionsDefinition: 'Successful movement handler admission is not proof of authoritative position application or an outbound ACK',
    pendingMovementDeadlineLagDefinition: 'Owner work start lateness after the original deadline, not a movement ACK latency' };
  for (const key of HISTOGRAMS) result[key] = histogramDelta(before[key], after[key]);
  for (const key of REQUESTS) {
    result[key] = { successCount: difference(after[key], before[key], 'successCount'),
      failureCount: difference(after[key], before[key], 'failureCount'), duration: histogramDelta(before[key].duration, after[key].duration) };
  }
  return result;
}
