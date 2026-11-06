import { isLoopbackWebSocketUrl } from "./load-gateway-ws-client-identity.mjs";

export const CANDIDATE_PROFILE = "candidate-64-active-30m";
export const CANDIDATE_CLIENTS = 64;
export const CANDIDATE_HOLD_OPEN_MS = 1_800_000;
export const CANDIDATE_MIN_HOLD_ACTION_INTERVAL_MS = 100;
export const CANDIDATE_MAX_HOLD_ACTION_INTERVAL_MS = 60_000;
export const CANDIDATE_MIN_RESOURCE_SAMPLE_MS = 10_000;
export const CANDIDATE_MAX_RESOURCE_SAMPLE_MS = 30_000;
export const CANDIDATE_MIN_KEEPALIVE_ACK_RATIO = 0.95;

const CANDIDATE_CAPACITY_FIELDS = [
  "maxWsConnections",
  "maxActiveSessions",
  "maxReconnectLeases",
];

export function isCandidateClientIndices(indices) {
  return Array.isArray(indices) &&
    indices.length === CANDIDATE_CLIENTS &&
    indices.every((value, index) => Number.isSafeInteger(value) && value === index);
}

export function validateLoadProfile({ env = process.env, wsUrl, healthUrl, clients, pool, holdOpenMs, holdActionIntervalMs, chatEvery, readyBarrier, reuseExistingAccounts, skipAccountCreate, simulatedDistinctClients, bootstrapOnly, stage5CommandsEnabled, mapTargets, expectedReady, expectedRejected, checkpointMs, resourceSampleMs, accountPrefix, accountIndexWidth, characterIndex, gatewayPid, expectedKeepAliveAckRatio, sendKeepAlive, sendMovement, sendChat } = {}) {
  const profile = String(env.MIR2_WS_LOAD_PROFILE ?? "").trim();
  if (profile === "") return { profile: null, errors: [] };

  const errors = [];
  if (profile !== CANDIDATE_PROFILE) {
    errors.push(`unsupported MIR2_WS_LOAD_PROFILE: ${profile}`);
    return { profile, errors };
  }

  requireExactNumber(env, "MIR2_WS_LOAD_CLIENTS", CANDIDATE_CLIENTS, clients, errors);
  requireExactNumber(env, "MIR2_WS_LOAD_POOL", CANDIDATE_CLIENTS, pool, errors);
  requireExactNumber(env, "MIR2_WS_LOAD_HOLD_OPEN_MS", CANDIDATE_HOLD_OPEN_MS, holdOpenMs, errors);
  requirePositiveNumber(env, "MIR2_WS_LOAD_HOLD_ACTION_INTERVAL_MS", holdActionIntervalMs, errors, {
    min: CANDIDATE_MIN_HOLD_ACTION_INTERVAL_MS,
    max: CANDIDATE_MAX_HOLD_ACTION_INTERVAL_MS,
  });
  requirePositiveNumber(env, "MIR2_WS_LOAD_CHAT_EVERY", chatEvery, errors, { min: 1, max: 12 });
  requireExactBoolean(env, "MIR2_WS_LOAD_READY_BARRIER", true, readyBarrier, errors);
  requireExactBoolean(env, "MIR2_WS_LOAD_REUSE_EXISTING_ACCOUNTS", true, reuseExistingAccounts, errors);
  requireExactBoolean(env, "MIR2_WS_LOAD_SKIP_ACCOUNT_CREATE", true, skipAccountCreate, errors);
  requireExactBoolean(env, "MIR2_WS_LOAD_SIMULATE_DISTINCT_CLIENTS", true, simulatedDistinctClients, errors);
  requireExactBoolean(env, "MIR2_WS_LOAD_SEND_KEEPALIVE", true, sendKeepAlive, errors);
  requireExactBoolean(env, "MIR2_WS_LOAD_SEND_MOVEMENT", true, sendMovement, errors);
  requireExactBoolean(env, "MIR2_WS_LOAD_SEND_CHAT", true, sendChat, errors);
  if (bootstrapOnly) errors.push("MIR2_WS_LOAD_BOOTSTRAP_ONLY must be disabled");
  if (stage5CommandsEnabled) errors.push("MIR2_WS_LOAD_ENABLE_STAGE5_COMMANDS must be disabled");
  if (Array.isArray(mapTargets) && mapTargets.length > 0) {
    errors.push("MIR2_WS_LOAD_MAP_TARGETS must be empty for ordinary-player Candidate load");
  }
  requireExactNumber(env, "MIR2_WS_LOAD_EXPECT_READY", CANDIDATE_CLIENTS, expectedReady, errors);
  requireExactNumber(env, "MIR2_WS_LOAD_EXPECT_REJECTED", 0, expectedRejected, errors);
  requirePositiveNumber(env, "MIR2_WS_LOAD_CHECKPOINT_MS", checkpointMs, errors);
  requirePositiveNumber(env, "MIR2_WS_LOAD_RESOURCE_SAMPLE_MS", resourceSampleMs, errors, {
    min: CANDIDATE_MIN_RESOURCE_SAMPLE_MS,
    max: CANDIDATE_MAX_RESOURCE_SAMPLE_MS,
  });
  requireExactNumber(env, "MIR2_WS_LOAD_ACCOUNT_INDEX_WIDTH", 3, accountIndexWidth, errors);
  requireExactNumber(env, "MIR2_WS_LOAD_CHARACTER_INDEX", 0, characterIndex, errors);
  requireMinimumNumber(
    env,
    "MIR2_WS_LOAD_EXPECT_KEEPALIVE_ACK_RATIO",
    CANDIDATE_MIN_KEEPALIVE_ACK_RATIO,
    expectedKeepAliveAckRatio,
    errors,
  );
  requirePositiveInteger(env, "MIR2_GATEWAY_PID", gatewayPid, errors);

  if (!hasConfiguredValue(env, "MIR2_GATEWAY_WS_URL")) {
    errors.push("MIR2_GATEWAY_WS_URL must be explicitly configured for the candidate profile");
  } else {
    try {
      if (!isLoopbackWebSocketUrl(wsUrl)) {
        errors.push("candidate profile requires a loopback WebSocket URL");
      }
      if (!candidateHealthMatchesWebSocket(wsUrl, healthUrl)) {
        errors.push("MIR2_GATEWAY_HEALTH_URL must use the same host and port as MIR2_GATEWAY_WS_URL");
      }
    } catch {
      errors.push("MIR2_GATEWAY_WS_URL must be a valid ws:// or wss:// URL");
    }
  }

  if (!String(accountPrefix ?? "").trim()) {
    errors.push("MIR2_WS_LOAD_ACCOUNT_PREFIX must identify pre-seeded accounts");
  }

  return { profile, errors };
}

export function candidateHealthMatchesWebSocket(wsUrl, healthUrl) {
  try {
    const ws = new URL(wsUrl);
    const health = new URL(healthUrl);
    const expectedHealthProtocol = ws.protocol === "wss:" ? "https:" : "http:";
    return health.protocol === expectedHealthProtocol &&
      normalizedHost(health.hostname) === normalizedHost(ws.hostname) &&
      effectivePort(health) === effectivePort(ws) &&
      health.pathname === "/health" &&
      health.search === "";
  } catch {
    return false;
  }
}

export function evaluateCandidateResourceSamples(samples, { gatewayPid, holdOpenMs, resourceSampleMs }) {
  const values = Array.isArray(samples) ? samples : [];
  const expected = Math.floor(holdOpenMs / resourceSampleMs) + 1;
  const minimumCount = Math.ceil(expected * 0.9);
  const firstAt = values[0]?.atUnixMs ?? null;
  const lastAt = values.at(-1)?.atUnixMs ?? null;
  const coverageMs = Number.isFinite(firstAt) && Number.isFinite(lastAt) ? lastAt - firstAt : 0;
  const minimumCoverageMs = Math.max(0, holdOpenMs - resourceSampleMs * 2);
  const allSamplesMatchPid = values.length > 0 && values.every((sample) => sample.pid === gatewayPid);
  const allSamplesComplete = values.length > 0 && values.every((sample) =>
    [sample.workingSetBytes, sample.privateBytes, sample.handleCount, sample.threadCount, sample.cpuTimeMs]
      .every(Number.isFinite));
  const warmupWindow = values.filter((sample) =>
    Number.isFinite(firstAt) && sample.atUnixMs >= firstAt + 8 * 60_000 &&
    sample.atUnixMs <= firstAt + 10 * 60_000);
  const tailWindow = values.filter((sample) =>
    Number.isFinite(lastAt) && sample.atUnixMs >= lastAt - 2 * 60_000);
  const postWarmup = values.filter((sample) =>
    Number.isFinite(firstAt) && sample.atUnixMs >= firstAt + 10 * 60_000);
  const stability = resourceStability(warmupWindow, tailWindow, postWarmup);
  return {
    count: values.length,
    expected,
    minimumCount,
    coverageMs,
    minimumCoverageMs,
    stability,
    assertions: {
      resourceSampleCountSufficient: values.length >= minimumCount,
      resourceSamplesMatchGatewayPid: allSamplesMatchPid,
      resourceSamplesCoverHold: coverageMs >= minimumCoverageMs,
      resourceSamplesComplete: allSamplesComplete,
      resourceWarmupWindowPresent: warmupWindow.length >= 5,
      resourceTailWindowPresent: tailWindow.length >= 5,
      rssTailBounded: ratioAtMost(stability.tail.rss, stability.warmup.rss, 1.25),
      privateBytesTailBounded: ratioAtMost(
        stability.tail.privateBytes,
        stability.warmup.privateBytes,
        1.25,
      ),
      rssPostWarmupPeakBounded: ratioAtMost(
        stability.postWarmupPeak.rss,
        stability.warmup.rss,
        1.5,
      ),
      privateBytesPostWarmupPeakBounded: ratioAtMost(
        stability.postWarmupPeak.privateBytes,
        stability.warmup.privateBytes,
        1.5,
      ),
      handleCountTailBounded: ratioAtMost(
        stability.tail.handles,
        stability.warmup.handles,
        1.25,
      ),
      threadCountTailBounded: ratioAtMost(
        stability.tail.threads,
        stability.warmup.threads,
        1.25,
      ),
    },
  };
}

export function evaluateCandidateClientActivity(activity, {
  clients = CANDIDATE_CLIENTS,
  holdOpenMs = CANDIDATE_HOLD_OPEN_MS,
  holdActionIntervalMs,
  minimumAckRatio = CANDIDATE_MIN_KEEPALIVE_ACK_RATIO,
} = {}) {
  const values = Array.isArray(activity) ? activity : [];
  const minimumCycles = Math.floor((holdOpenMs / holdActionIntervalMs) * 0.9);
  const completed = values.filter((entry) => entry.completed === true);
  const assertions = {
    clientActivityCountMatches: values.length === clients,
    everyClientCompletedHold: completed.length === clients &&
      completed.every((entry) => entry.holdDurationMs >= holdOpenMs),
    everyClientReachedMinimumHoldActions: completed.length === clients &&
      completed.every((entry) => entry.holdActionCycles >= minimumCycles),
    everyClientHoldKeepAliveRatioPasses: completed.length === clients &&
      completed.every((entry) =>
        entry.holdKeepAliveSent > 0 && entry.holdKeepAliveAckRatio >= minimumAckRatio),
    noClientUnexpectedClosure: values.length === clients &&
      values.every((entry) => entry.unexpectedClosure !== true),
  };
  return {
    clients: values.length,
    completed: completed.length,
    minimumCycles,
    minimumAckRatio,
    assertions,
    ok: Object.values(assertions).every(Boolean),
  };
}

function resourceStability(warmup, tail, postWarmup) {
  const warmRss = percentile(warmup.map((sample) => sample.workingSetBytes), 0.95);
  const warmPrivate = percentile(warmup.map((sample) => sample.privateBytes), 0.95);
  const warmHandles = percentile(warmup.map((sample) => sample.handleCount), 0.95);
  const warmThreads = percentile(warmup.map((sample) => sample.threadCount), 0.95);
  const tailRss = percentile(tail.map((sample) => sample.workingSetBytes), 0.95);
  const tailPrivate = percentile(tail.map((sample) => sample.privateBytes), 0.95);
  const tailHandles = percentile(tail.map((sample) => sample.handleCount), 0.95);
  const tailThreads = percentile(tail.map((sample) => sample.threadCount), 0.95);
  const peakRss = maximum(postWarmup.map((sample) => sample.workingSetBytes));
  const peakPrivate = maximum(postWarmup.map((sample) => sample.privateBytes));
  return {
    warmupSampleCount: warmup.length,
    tailSampleCount: tail.length,
    postWarmupSampleCount: postWarmup.length,
    warmup: { rss: warmRss, privateBytes: warmPrivate, handles: warmHandles, threads: warmThreads },
    tail: { rss: tailRss, privateBytes: tailPrivate, handles: tailHandles, threads: tailThreads },
    postWarmupPeak: { rss: peakRss, privateBytes: peakPrivate },
    rssTailToWarmupRatio: ratio(tailRss, warmRss),
    privateTailToWarmupRatio: ratio(tailPrivate, warmPrivate),
    handleTailToWarmupRatio: ratio(tailHandles, warmHandles),
    threadTailToWarmupRatio: ratio(tailThreads, warmThreads),
    rssPeakToWarmupRatio: ratio(peakRss, warmRss),
    privatePeakToWarmupRatio: ratio(peakPrivate, warmPrivate),
  };
}

function percentile(values, fraction) {
  const clean = values.filter(Number.isFinite).sort((left, right) => left - right);
  if (clean.length === 0) return null;
  return clean[Math.min(clean.length - 1, Math.floor(clean.length * fraction))];
}

function maximum(values) {
  const clean = values.filter(Number.isFinite);
  return clean.length === 0 ? null : Math.max(...clean);
}

function ratio(value, baseline) {
  return Number.isFinite(value) && Number.isFinite(baseline) && baseline > 0
    ? Math.round((value / baseline) * 10_000) / 10_000
    : null;
}

function ratioAtMost(value, baseline, maximum) {
  return Number.isFinite(value) && Number.isFinite(baseline) && baseline > 0 &&
    Number.isFinite(maximum) && value / baseline <= maximum;
}

export function evaluateCandidateCapacityHealth(snapshot, minimum = CANDIDATE_CLIENTS) {
  const capacity = isHealthyGatewaySnapshot(snapshot) && snapshot.value?.capacity && typeof snapshot.value.capacity === "object"
    ? snapshot.value.capacity
    : null;
  const values = Object.fromEntries(
    CANDIDATE_CAPACITY_FIELDS.map((field) => [field, capacity?.[field] ?? null]),
  );
  const assertions = Object.fromEntries(
    CANDIDATE_CAPACITY_FIELDS.map((field) => [
      `${field}Configured`,
      Number.isSafeInteger(values[field]) && values[field] >= minimum,
    ]),
  );
  return {
    ok: Object.values(assertions).every(Boolean),
    values,
    assertions,
  };
}

export function isHealthyGatewaySnapshot(snapshot) {
  return snapshot?.ok === true && snapshot.value?.ok === true;
}

export function evaluateCandidateCapacityHealthPair(healthBefore, healthAfter, minimum = CANDIDATE_CLIENTS) {
  const before = evaluateCandidateCapacityHealth(healthBefore, minimum);
  const after = evaluateCandidateCapacityHealth(healthAfter, minimum);
  const fieldAssertions = Object.fromEntries([
    ...Object.entries(before.assertions).map(([field, value]) => [`healthBefore${capitalize(field)}`, value]),
    ...Object.entries(after.assertions).map(([field, value]) => [`healthAfter${capitalize(field)}`, value]),
  ]);
  return {
    before,
    after,
    assertions: {
      healthBeforeCapacityConfigured: before.ok,
      healthAfterCapacityConfigured: after.ok,
      ...fieldAssertions,
    },
    ok: before.ok && after.ok,
  };
}

function capitalize(value) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function normalizedHost(value) {
  return String(value).replace(/^\[|\]$/g, "").toLowerCase();
}

function effectivePort(url) {
  if (url.port) return Number(url.port);
  return url.protocol === "wss:" || url.protocol === "https:" ? 443 : 80;
}

function hasConfiguredValue(env, name) {
  return typeof env[name] === "string" && env[name].trim() !== "";
}

function requireExactNumber(env, name, expected, actual, errors) {
  if (!hasConfiguredValue(env, name)) {
    errors.push(`${name} must be explicitly set to ${expected}`);
    return;
  }
  if (actual !== expected || Number(env[name]) !== expected) {
    errors.push(`${name} must equal ${expected}`);
  }
}

function requirePositiveNumber(env, name, actual, errors, { min = 0, max = Number.POSITIVE_INFINITY } = {}) {
  if (!hasConfiguredValue(env, name)) {
    errors.push(`${name} must be explicitly configured`);
    return;
  }
  const value = Number(env[name]);
  if (!Number.isFinite(value) || value <= 0 || actual !== value || value < min || value > max) {
    const range = min > 0 || Number.isFinite(max) ? ` (${min}..${max})` : "";
    errors.push(`${name} must be a positive number${range}`);
  }
}

function requirePositiveInteger(env, name, actual, errors) {
  if (!hasConfiguredValue(env, name)) {
    errors.push(`${name} must be explicitly configured with a positive PID`);
    return;
  }
  const value = Number(env[name]);
  if (!Number.isSafeInteger(value) || value <= 0 || actual !== value) {
    errors.push(`${name} must be a positive integer PID`);
  }
}

function requireMinimumNumber(env, name, minimum, actual, errors) {
  if (!hasConfiguredValue(env, name)) {
    errors.push(`${name} must be explicitly configured to at least ${minimum}`);
    return;
  }
  const value = Number(env[name]);
  if (!Number.isFinite(value) || value < minimum || actual !== value) {
    errors.push(`${name} must be at least ${minimum}`);
  }
}

function requireExactBoolean(env, name, expected, actual, errors) {
  if (!hasConfiguredValue(env, name)) {
    errors.push(`${name} must be explicitly set to ${expected}`);
    return;
  }
  const value = /^(1|true|yes|on)$/i.test(env[name].trim());
  if (value !== expected || actual !== expected) {
    errors.push(`${name} must be explicitly enabled`);
  }
}
