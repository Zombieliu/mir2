import assert from "node:assert/strict";
import test from "node:test";

import {
  CANDIDATE_PROFILE,
  candidateHealthMatchesWebSocket,
  evaluateCandidateClientActivity,
  evaluateCandidateCapacityHealthPair,
  evaluateCandidateCapacityHealth,
  evaluateCandidateResourceSamples,
  isCandidateClientIndices,
  isHealthyGatewaySnapshot,
  validateLoadProfile,
} from "./load-gateway-ws-profile.mjs";

test("candidate client indices must be the unique ordered range 0 through 63", () => {
  const valid = Array.from({ length: 64 }, (_, index) => index);
  assert.equal(isCandidateClientIndices(valid), true);
  assert.equal(isCandidateClientIndices([...valid.slice(0, 63), 62]), false);
  assert.equal(isCandidateClientIndices([...valid].reverse()), false);
  assert.equal(isCandidateClientIndices(valid.slice(0, 63)), false);
});

function validEnv(overrides = {}) {
  return {
    MIR2_WS_LOAD_PROFILE: CANDIDATE_PROFILE,
    MIR2_WS_LOAD_CLIENTS: "64",
    MIR2_WS_LOAD_POOL: "64",
    MIR2_WS_LOAD_HOLD_OPEN_MS: "1800000",
    MIR2_WS_LOAD_HOLD_ACTION_INTERVAL_MS: "1000",
    MIR2_WS_LOAD_CHAT_EVERY: "10",
    MIR2_WS_LOAD_READY_BARRIER: "1",
    MIR2_WS_LOAD_REUSE_EXISTING_ACCOUNTS: "1",
    MIR2_WS_LOAD_SKIP_ACCOUNT_CREATE: "1",
    MIR2_WS_LOAD_SIMULATE_DISTINCT_CLIENTS: "1",
    MIR2_WS_LOAD_SEND_KEEPALIVE: "1",
    MIR2_WS_LOAD_SEND_MOVEMENT: "1",
    MIR2_WS_LOAD_SEND_CHAT: "1",
    MIR2_WS_LOAD_EXPECT_READY: "64",
    MIR2_WS_LOAD_EXPECT_REJECTED: "0",
    MIR2_WS_LOAD_CHECKPOINT_MS: "30000",
    MIR2_WS_LOAD_RESOURCE_SAMPLE_MS: "10000",
    MIR2_WS_LOAD_ACCOUNT_INDEX_WIDTH: "3",
    MIR2_WS_LOAD_CHARACTER_INDEX: "0",
    MIR2_WS_LOAD_EXPECT_KEEPALIVE_ACK_RATIO: "0.95",
    MIR2_GATEWAY_PID: "1234",
    MIR2_GATEWAY_WS_URL: "ws://127.0.0.1:7210/ws",
    MIR2_WS_LOAD_ACCOUNT_PREFIX: "soak64-",
    ...overrides,
  };
}

function validDerived(overrides = {}) {
  return {
    wsUrl: "ws://127.0.0.1:7210/ws",
    healthUrl: "http://127.0.0.1:7210/health",
    clients: 64,
    pool: 64,
    holdOpenMs: 1_800_000,
    holdActionIntervalMs: 1000,
    chatEvery: 10,
    readyBarrier: true,
    reuseExistingAccounts: true,
    skipAccountCreate: true,
    simulatedDistinctClients: true,
    bootstrapOnly: false,
    stage5CommandsEnabled: false,
    mapTargets: [],
    expectedReady: 64,
    expectedRejected: 0,
    checkpointMs: 30000,
    resourceSampleMs: 10000,
    accountPrefix: "soak64-",
    accountIndexWidth: 3,
    characterIndex: 0,
    gatewayPid: 1234,
    expectedKeepAliveAckRatio: 0.95,
    sendKeepAlive: true,
    sendMovement: true,
    sendChat: true,
    ...overrides,
  };
}

test("candidate-64-active-30m accepts only the complete explicit profile", () => {
  assert.deepEqual(
    validateLoadProfile({ env: validEnv(), ...validDerived() }),
    { profile: CANDIDATE_PROFILE, errors: [] },
  );
});

test("candidate profile fails fast for missing or weakened gates", () => {
  const result = validateLoadProfile({
    env: validEnv({
      MIR2_WS_LOAD_POOL: "32",
      MIR2_WS_LOAD_HOLD_ACTION_INTERVAL_MS: "0",
      MIR2_WS_LOAD_SIMULATE_DISTINCT_CLIENTS: "0",
      MIR2_WS_LOAD_RESOURCE_SAMPLE_MS: "500",
      MIR2_WS_LOAD_EXPECT_KEEPALIVE_ACK_RATIO: "0.5",
      MIR2_GATEWAY_PID: "",
      MIR2_GATEWAY_WS_URL: "ws://example.test/ws",
    }),
    ...validDerived({
      pool: 32,
      holdActionIntervalMs: 0,
      simulatedDistinctClients: false,
      resourceSampleMs: 500,
      expectedKeepAliveAckRatio: 0.5,
      gatewayPid: null,
      wsUrl: "ws://example.test/ws",
      healthUrl: "http://127.0.0.1:9999/health",
      bootstrapOnly: true,
      stage5CommandsEnabled: true,
      mapTargets: [{ mapFileName: "0", x: 1, y: 1 }],
    }),
  });
  assert.ok(result.errors.length >= 5);
  assert.match(result.errors.join("\n"), /POOL/);
  assert.match(result.errors.join("\n"), /HOLD_ACTION_INTERVAL/);
  assert.match(result.errors.join("\n"), /RESOURCE_SAMPLE/);
  assert.match(result.errors.join("\n"), /EXPECT_KEEPALIVE_ACK_RATIO/);
  assert.match(result.errors.join("\n"), /loopback/);
  assert.match(result.errors.join("\n"), /positive PID/);
  assert.match(result.errors.join("\n"), /BOOTSTRAP_ONLY/);
  assert.match(result.errors.join("\n"), /STAGE5/);
  assert.match(result.errors.join("\n"), /MAP_TARGETS/);
});

test("candidate health endpoint must be the HTTP equivalent of the WebSocket origin", () => {
  assert.equal(
    candidateHealthMatchesWebSocket(
      "ws://127.0.0.1:7210/ws",
      "http://127.0.0.1:7210/health",
    ),
    true,
  );
  assert.equal(
    candidateHealthMatchesWebSocket(
      "ws://127.0.0.1:7210/ws",
      "http://127.0.0.1:9999/health",
    ),
    false,
  );
});

test("candidate resource samples require PID fidelity, count, completeness, and hold coverage", () => {
  const start = 1_000_000;
  const samples = Array.from({ length: 181 }, (_, index) => ({
    atUnixMs: start + index * 10_000,
    pid: 1234,
    workingSetBytes: 100_000_000 + index * 10_000,
    privateBytes: 120_000_000 + index * 10_000,
    handleCount: 10,
    threadCount: 20,
    cpuTimeMs: index * 100,
  }));
  const result = evaluateCandidateResourceSamples(samples, {
    gatewayPid: 1234,
    holdOpenMs: 1_800_000,
    resourceSampleMs: 10_000,
  });
  assert.equal(Object.values(result.assertions).every(Boolean), true);
  samples[10].pid = 9999;
  assert.equal(
    evaluateCandidateResourceSamples(samples, {
      gatewayPid: 1234,
      holdOpenMs: 1_800_000,
      resourceSampleMs: 10_000,
    }).assertions.resourceSamplesMatchGatewayPid,
    false,
  );
});

test("candidate resource samples reject sustained post-warmup growth", () => {
  const start = 1_000_000;
  const samples = Array.from({ length: 181 }, (_, index) => ({
    atUnixMs: start + index * 10_000,
    pid: 1234,
    workingSetBytes: 100_000_000 + index * 100_000_000,
    privateBytes: 120_000_000 + index * 100_000_000,
    handleCount: 10 + index,
    threadCount: 20 + index,
    cpuTimeMs: index * 100,
  }));
  const result = evaluateCandidateResourceSamples(samples, {
    gatewayPid: 1234,
    holdOpenMs: 1_800_000,
    resourceSampleMs: 10_000,
  });
  assert.equal(result.assertions.rssTailBounded, false);
  assert.equal(result.assertions.privateBytesTailBounded, false);
  assert.equal(result.assertions.rssPostWarmupPeakBounded, false);
  assert.equal(result.assertions.privateBytesPostWarmupPeakBounded, false);
  assert.equal(result.assertions.handleCountTailBounded, false);
  assert.equal(result.assertions.threadCountTailBounded, false);
});

test("candidate resource bounds compare raw ratios rather than rounded report ratios", () => {
  const start = 1_000_000;
  const samples = Array.from({ length: 181 }, (_, index) => {
    const inTail = index >= 169;
    return {
      atUnixMs: start + index * 10_000,
      pid: 1234,
      workingSetBytes: inTail ? 125_001_000 : 100_000_000,
      privateBytes: 100_000_000,
      handleCount: 100,
      threadCount: 100,
      cpuTimeMs: index * 100,
    };
  });
  const result = evaluateCandidateResourceSamples(samples, {
    gatewayPid: 1234,
    holdOpenMs: 1_800_000,
    resourceSampleMs: 10_000,
  });
  assert.equal(result.stability.rssTailToWarmupRatio, 1.25);
  assert.equal(result.assertions.rssTailBounded, false);
  assert.equal(result.assertions.rssPostWarmupPeakBounded, true);
});

test("candidate activity requires every client to finish hold and pass its hold-only KeepAlive ratio", () => {
  const activity = Array.from({ length: 64 }, (_, index) => ({
    index,
    completed: true,
    holdDurationMs: 1_800_000,
    holdActionCycles: 360,
    holdKeepAliveSent: 360,
    holdKeepAliveAcknowledged: 360,
    holdKeepAliveAckRatio: 1,
    unexpectedClosure: false,
  }));
  assert.equal(
    evaluateCandidateClientActivity(activity, {
      holdActionIntervalMs: 5_000,
    }).ok,
    true,
  );
  activity[0].holdKeepAliveAckRatio = 0.5;
  assert.equal(
    evaluateCandidateClientActivity(activity, {
      holdActionIntervalMs: 5_000,
    }).assertions.everyClientHoldKeepAliveRatioPasses,
    false,
  );
});

test("ordinary runs do not activate the strict profile", () => {
  assert.deepEqual(
    validateLoadProfile({ env: {}, ...validDerived({ wsUrl: "ws://gateway.example/ws" }) }),
    { profile: null, errors: [] },
  );
});

test("candidate capacity health requires all three finite limits on both snapshots", () => {
  const healthy = (capacity) => ({ ok: true, value: { ok: true, capacity } });
  const pair = evaluateCandidateCapacityHealthPair(
    healthy({ maxWsConnections: 64, maxActiveSessions: 64, maxReconnectLeases: 64 }),
    healthy({ maxWsConnections: 128, maxActiveSessions: 96, maxReconnectLeases: 80 }),
  );
  assert.equal(pair.ok, true);
  assert.deepEqual(pair.assertions, {
    healthBeforeCapacityConfigured: true,
    healthAfterCapacityConfigured: true,
    healthBeforeMaxWsConnectionsConfigured: true,
    healthBeforeMaxActiveSessionsConfigured: true,
    healthBeforeMaxReconnectLeasesConfigured: true,
    healthAfterMaxWsConnectionsConfigured: true,
    healthAfterMaxActiveSessionsConfigured: true,
    healthAfterMaxReconnectLeasesConfigured: true,
  });
});

test("candidate capacity health rejects null, unlimited, and undersized limits", () => {
  const result = evaluateCandidateCapacityHealth({
    ok: true,
    value: {
      ok: true,
      capacity: {
        maxWsConnections: null,
        maxActiveSessions: 64,
        maxReconnectLeases: 63,
      },
    },
  });
  assert.equal(result.ok, false);
  assert.deepEqual(result.values, {
    maxWsConnections: null,
    maxActiveSessions: 64,
    maxReconnectLeases: 63,
  });
  assert.deepEqual(result.assertions, {
    maxWsConnectionsConfigured: false,
    maxActiveSessionsConfigured: true,
    maxReconnectLeasesConfigured: false,
  });
  assert.equal(
    evaluateCandidateCapacityHealthPair({ ok: true, value: { capacity: null } }, { ok: false }).ok,
    false,
  );
  assert.equal(isHealthyGatewaySnapshot({ ok: true, value: { ok: false } }), false);
});
