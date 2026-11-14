import assert from "node:assert/strict";
import test from "node:test";
import {analyzeStartupDelivery} from "./lib/startup-delivery-analysis.mjs";

const revision = "a".repeat(40), gatewayRevision = "b".repeat(40), invocationId = "c".repeat(32);
const receipt = () => ({expectedRevision: revision, timing: {clockEpochMs: 1_000_000},
  movement: {wire: {atMs: 10_000}, ack: {atMs: 23_286}}, firstWalkTransport: {websocketFrameMs: 13_283}});
const health = (startedAt, count, sumUs, uptime, extra = {}) => ({status: 200, complete: true,
  startedAt, finishedAt: startedAt + 20, revision: gatewayRevision, count, sumUs,
  sinceInitializationMs: uptime, ...extra});
const probe = () => ({health: [health(9000, 0, 0, 1000), health(10_300, 1, 524, 2300)],
  runtime: [8000, 10_000, 12_000, 14_000, 16_000, 18_000, 20_000, 22_000, 24_000].map(at => ({
    observedAtMs: at, atMs: at - 100, timeOrigin: 1_000_100, booted: false, bootAttempted: false})),
  final: {timeOrigin: 1_000_100, longTasks: [{startMs: 500, durationMs: 194}], drifts: []}});
const server = () => {
  const pins = {pid: 123, processStartTicks: 456, invocationId, revision: gatewayRevision};
  const identity = {pid: 123, processStartTicks: 456, invocationId, activeState: "active"};
  return {ok: true, remoteExitStatus: 0, remoteExitStatusReceived: true, boundedWindowExportComplete: true,
    serviceOperations: [], networkOrVpnOperations: [], candidatePins: pins,
    queryResult: {identityStableAcrossRead: true, projectionComplete: true, boundedWindowExportComplete: true,
      rowBoundReached: false, backendRevision: gatewayRevision, targetIdentityBefore: {...identity},
      targetIdentityAfter: {...identity}, journalRowsRead: 1, journalWindowUtc: ["2026-11-14T06:54:50Z", "2026-11-14T06:55:25Z"],
      diagnosticFlags: {readComplete: true, zoneDeliveryTraceLiteral1: false, slowStagePositiveU64: true},
      diagnosticEvents: [{pid: 123, invocationId, fieldStatus: "valid", prefix: "[gateway-slow-stage]",
        atUnixUs: 1_794_639_295_874_575, stage: "session_action.execute", duration_ms: 3117}]}};
};

test("aggregate movement timing remains unbound to a player even with exactly one increment", () => {
  const result = analyzeStartupDelivery(receipt(), probe());
  assert.equal(result.movement.durationMs, 13_286);
  assert.equal(result.movement.originalEightSecondPassed, false);
  assert.equal(result.aggregateAdmission.countDelta, 1);
  assert.equal(result.aggregateAdmission.sumUsDelta, 524);
  assert.equal(result.aggregateAdmission.observedAfterSendByMs, 320);
  assert.equal(result.aggregateAdmission.actionIdentityBound, false);
  assert.equal(result.aggregateAdmission.exactPlayerExecutionLatencyProven, false);
  assert.equal(result.rootCauseProven, false);
});

test("revision change, uptime reset and an intermediate counter reset invalidate counter deltas", () => {
  for (const change of [p => {p.health[1].revision = "d".repeat(40);},
    p => {p.health[1].sinceInitializationMs = 1;},
    p => {p.health.splice(1, 0, health(9500, 2, 2000, 1500));}]) {
    const p = probe(); change(p);
    const result = analyzeStartupDelivery(receipt(), p).aggregateAdmission;
    assert.equal(result.sameRevisionAndMonotonicCounters, false);
    assert.equal(result.countDelta, null);
    assert.equal(result.sumUsDelta, null);
  }
});

test("a missing pre-sample, in-flight sample or failed status does not manufacture an interval", () => {
  for (const rows of [[health(10_300, 1, 524, 2300)],
    [health(9000, 0, 0, 1000), health(9990, 1, 524, 2000)],
    [health(9000, 0, 0, 1000), health(10_300, 1, 524, 2300, {status: 503})]]) {
    assert.equal(analyzeStartupDelivery(receipt(), {health: rows}).aggregateAdmission.observed, false);
  }
});

test("zero counters are valid evidence without proving a failed movement", () => {
  const p = probe(); p.health[1].count = 0; p.health[1].sumUs = 0;
  assert.equal(analyzeStartupDelivery(receipt(), p).aggregateAdmission.countDelta, 0);
});

test("browser tasks use timeOrigin offset and preserve incomplete-exclusion boundaries", () => {
  const p = probe();
  p.final.longTasks.push({startMs: 9800, durationMs: 200});
  p.final.drifts.push({startMs: 9500, delayMs: 500});
  const result = analyzeStartupDelivery(receipt(), p).browser;
  assert.equal(result.pageEpochOffsetMs, 100);
  assert.deepEqual(result.longTasksInWindow, [{startMs: 9900, endMs: 10_100, durationMs: 200}]);
  assert.deepEqual(result.driftsInWindow, [{startMs: 9600, delayMs: 500}]);
  assert.equal(result.callbackCoverage, true);
  assert.equal(result.maxObservedCallbackGapMs, 2000);
  assert.equal(result.mainThreadCauseExcluded, false);
});

test("callback gaps, different document origins and saturated collectors stay visible", () => {
  const p = probe(); p.runtime = [p.runtime[0], p.runtime.at(-1)];
  p.runtime[1].timeOrigin++;
  p.final.longTasks = Array.from({length: 200}, () => ({startMs: 1, durationMs: 1}));
  p.final.drifts = Array.from({length: 100}, () => ({startMs: 1, delayMs: 1}));
  const result = analyzeStartupDelivery(receipt(), p).browser;
  assert.equal(result.callbackCoverage, false);
  assert.equal(result.maxObservedCallbackGapMs, null);
  assert.equal(result.taskCollectorMayBeCapped, true);
  assert.equal(result.driftCollectorMayBeCapped, true);
});

test("bounded server projection validates identity and never asserts cross-clock or ACK correlation", () => {
  const result = analyzeStartupDelivery(receipt(), probe(), server());
  assert.equal(result.server.observed, true);
  assert.equal(result.server.maximumDurationMsByStage["session_action.execute"], 3117);
  assert.equal(result.server.stageCounts["session_action.execute"], 1);
  assert.equal(result.server.exactCrossClockLatencyInferred, false);
  assert.equal(result.server.ackEgressBound, false);
  assert.equal(result.rootCauseProven, false);
});

test("partial, capped, changed-identity or mutated server reads are rejected", () => {
  for (const change of [s => {s.remoteExitStatusReceived = false;}, s => {s.remoteExitStatus = 1;},
    s => {s.queryResult.rowBoundReached = true;}, s => {s.queryResult.projectionComplete = false;},
    s => {s.queryResult.targetIdentityAfter.pid++;}, s => {s.queryResult.backendRevision = revision;},
    s => {delete s.queryResult.diagnosticFlags.zoneDeliveryTraceLiteral1;},
    s => {s.queryResult.diagnosticEvents[0].fieldStatus = "partial";},
    s => {s.serviceOperations.push("restart");}, s => {s.networkOrVpnOperations.push("change");}]) {
    const s = server(); change(s);
    assert.throws(() => analyzeStartupDelivery(receipt(), probe(), s), /server/);
  }
});

test("analysis exports numeric timing and allowlisted fields without arbitrary private metadata", () => {
  const p = probe(), s = server();
  p.health[0].cookie = "PRIVATE_SENTINEL";
  p.runtime[0].url = "PRIVATE_SENTINEL";
  s.candidatePins.extra = "PRIVATE_SENTINEL";
  s.queryResult.diagnosticEvents[0].rawMessage = "PRIVATE_SENTINEL";
  assert.doesNotMatch(JSON.stringify(analyzeStartupDelivery(receipt(), p, s)), /PRIVATE_SENTINEL/);
});

test("invalid clocks, arrays and task samples fail instead of producing a success", () => {
  const r = receipt(); r.movement.ack.atMs = 1;
  assert.throws(() => analyzeStartupDelivery(r, probe()), /invalid movement/);
  assert.throws(() => analyzeStartupDelivery(receipt(), {health: {}}), /probe arrays/);
  const p = probe(); p.final.longTasks = [null];
  assert.throws(() => analyzeStartupDelivery(receipt(), p), /browser timing/);
});
