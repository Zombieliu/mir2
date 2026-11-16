const finite = value => typeof value === "number" && Number.isFinite(value);
const nonnegative = value => finite(value) && value >= 0;
const uint = value => Number.isSafeInteger(value) && value >= 0;
const revision = value => typeof value === "string" && /^[a-f0-9]{40}$/.test(value);
const requireInput = (condition, message) => { if (!condition) throw new Error(message); };
const projectHealth = row => Object.fromEntries(["status", "startedAt", "finishedAt", "revision", "complete",
  "sinceInitializationMs", "count", "sumUs"].map(key => [key, row[key]]));

/** Analyze observations without equating a global counter with one player's ACK. */
export function analyzeStartupDelivery(receipt, probe, serverReadback) {
  const movement = receipt?.movement, timing = receipt?.timing;
  requireInput(nonnegative(movement?.wire?.atMs) && nonnegative(movement?.ack?.atMs)
    && movement.ack.atMs >= movement.wire.atMs && nonnegative(timing?.clockEpochMs)
    && revision(receipt.expectedRevision), "invalid movement clock or source");
  requireInput((probe?.health === undefined || Array.isArray(probe.health))
    && (probe?.runtime === undefined || Array.isArray(probe.runtime)), "invalid probe arrays");
  const start = movement.wire.atMs, end = movement.ack.atMs;
  const output = {
    schema: "mir2.startup-delivery-analysis.v1",
    source: receipt.expectedRevision,
    movement: { wireAtMs: start, ackAtMs: end, durationMs: end - start,
      originalEightSecondPassed: end - start <= 8000,
      websocketFrameMs: nonnegative(receipt.firstWalkTransport?.websocketFrameMs)
        ? receipt.firstWalkTransport.websocketFrameMs : null },
    aggregateAdmission: { observed: false, actionIdentityBound: false, exactPlayerExecutionLatencyProven: false },
    browser: { observed: false, mainThreadCauseExcluded: false },
    server: { observed: false, exactCrossClockLatencyInferred: false, ackEgressBound: false },
    rootCauseProven: false,
  };

  const rows = (probe?.health ?? []).filter(row => row?.status === 200 && row.complete === true
    && nonnegative(row.startedAt) && nonnegative(row.finishedAt) && row.finishedAt >= row.startedAt
    && uint(row.count) && uint(row.sumUs) && nonnegative(row.sinceInitializationMs) && revision(row.revision))
    .map(projectHealth);
  const before = rows.filter(row => row.finishedAt < start).sort((a, b) => b.finishedAt - a.finishedAt)[0];
  const after = rows.filter(row => row.startedAt >= start && row.finishedAt <= end)
    .sort((a, b) => a.finishedAt - b.finishedAt)[0];
  if (before && after) {
    const interval = rows.filter(row => row.startedAt >= before.startedAt && row.finishedAt <= after.finishedAt)
      .sort((a, b) => a.startedAt - b.startedAt);
    const monotonic = interval.every((row, index) => row.revision === before.revision && (!index
      || row.sinceInitializationMs >= interval[index - 1].sinceInitializationMs
      && row.count >= interval[index - 1].count && row.sumUs >= interval[index - 1].sumUs));
    output.aggregateAdmission = { ...output.aggregateAdmission, observed: true,
      sameRevisionAndMonotonicCounters: monotonic, before, after,
      countDelta: monotonic ? after.count - before.count : null,
      sumUsDelta: monotonic ? after.sumUs - before.sumUs : null,
      observedAfterSendByMs: after.finishedAt - start,
      limitation: "No instance ID, player ID or command ID is present in these aggregate samples." };
  }

  const final = probe?.final;
  if (finite(final?.timeOrigin) && Array.isArray(final.longTasks) && Array.isArray(final.drifts)) {
    const offset = final.timeOrigin - timing.clockEpochMs;
    requireInput(final.longTasks.every(row => nonnegative(row?.startMs) && nonnegative(row?.durationMs))
      && final.drifts.every(row => nonnegative(row?.startMs) && nonnegative(row?.delayMs)), "invalid browser timing");
    const matching = (probe.runtime ?? []).filter(row => row?.timeOrigin === final.timeOrigin
      && nonnegative(row.observedAtMs) && nonnegative(row.atMs));
    const callbacks = matching.filter(row => row.observedAtMs >= start - 3000 && row.observedAtMs <= end + 3000)
      .map(row => ({ observedAtMs: row.observedAtMs, browserAtMs: offset + row.atMs,
        booted: typeof row.booted === "boolean" ? row.booted : null,
        bootAttempted: typeof row.bootAttempted === "boolean" ? row.bootAttempted : null }))
      .sort((a, b) => a.observedAtMs - b.observedAtMs);
    const longTasks = final.longTasks.map(row => ({ startMs: offset + row.startMs,
      endMs: offset + row.startMs + row.durationMs, durationMs: row.durationMs }))
      .filter(row => row.endMs >= start && row.startMs <= end);
    const drifts = final.drifts.map(row => ({ startMs: offset + row.startMs, delayMs: row.delayMs }))
      .filter(row => row.startMs <= end && row.startMs + row.delayMs >= start);
    output.browser = { ...output.browser, observed: true, pageEpochOffsetMs: offset,
      callbackCoverage: callbacks.length >= 2 && callbacks[0].observedAtMs <= start
        && callbacks.at(-1).observedAtMs >= end,
      callbacks, maxObservedCallbackGapMs: callbacks.length > 1
        ? Math.max(...callbacks.slice(1).map((row, i) => row.observedAtMs - callbacks[i].observedAtMs)) : null,
      longTasksInWindow: longTasks, driftsInWindow: drifts,
      fullObservedTaskMaxMs: Math.max(0, ...final.longTasks.map(row => row.durationMs)),
      taskCollectorMayBeCapped: final.longTasks.length >= 200,
      driftCollectorMayBeCapped: final.drifts.length >= 100,
      limitation: "Recorded callbacks and task observations are evidence, not a complete renderer exclusion." };
  }

  if (serverReadback) {
    const q = serverReadback.queryResult;
    requireInput(serverReadback.ok === true && serverReadback.remoteExitStatus === 0
      && serverReadback.remoteExitStatusReceived === true && serverReadback.boundedWindowExportComplete === true
      && q?.identityStableAcrossRead === true && q.projectionComplete === true
      && q.boundedWindowExportComplete === true && q.rowBoundReached === false
      && Array.isArray(q.diagnosticEvents), "incomplete server projection");
    requireInput(Array.isArray(serverReadback.serviceOperations) && !serverReadback.serviceOperations.length
      && Array.isArray(serverReadback.networkOrVpnOperations) && !serverReadback.networkOrVpnOperations.length,
    "server projection has mutations");
    const pins = serverReadback.candidatePins;
    requireInput(uint(pins?.pid) && pins.pid > 1 && uint(pins.processStartTicks)
      && /^[a-f0-9]{32}$/.test(pins.invocationId ?? "") && revision(pins.revision)
      && q.backendRevision === pins.revision
      && q.diagnosticFlags?.readComplete === true
      && ["zoneDeliveryTraceLiteral1", "slowStagePositiveU64"].every(key => typeof q.diagnosticFlags[key] === "boolean")
      && [q.targetIdentityBefore, q.targetIdentityAfter].every(identity => identity?.pid === pins.pid
        && identity.processStartTicks === pins.processStartTicks && identity.invocationId === pins.invocationId
        && identity.activeState === "active")
      && uint(q.journalRowsRead) && Array.isArray(q.journalWindowUtc) && q.journalWindowUtc.length === 2
      && q.journalWindowUtc.every(value => typeof value === "string" && Number.isFinite(Date.parse(value)))
      && q.diagnosticEvents.every(row => row?.fieldStatus === "valid" && uint(row.atUnixUs)
        && row.pid === pins.pid && row.invocationId === pins.invocationId
        && row.prefix === "[gateway-slow-stage]" && /^[a-z_]+(?:\.[a-z_]+)+$/.test(row.stage)
        && uint(row.duration_ms)), "invalid server identity or events");
    const events = q.diagnosticEvents.map(row => ({atUnixUs: row.atUnixUs, stage: row.stage, durationMs: row.duration_ms}));
    const stageCounts = {}, maximumDurationMsByStage = {};
    for (const row of events) {
      stageCounts[row.stage] = (stageCounts[row.stage] ?? 0) + 1;
      maximumDurationMsByStage[row.stage] = Math.max(maximumDurationMsByStage[row.stage] ?? 0, row.durationMs);
    }
    output.server = { ...output.server, observed: true, pins: {pid: pins.pid,
      processStartTicks: pins.processStartTicks, invocationId: pins.invocationId, revision: pins.revision},
      journalWindowUtc: q.journalWindowUtc, rowsRead: q.journalRowsRead,
      diagnosticFlags: Object.fromEntries(["readComplete", "zoneDeliveryTraceLiteral1", "slowStagePositiveU64"]
        .map(key => [key, q.diagnosticFlags?.[key] === true])), stageCounts, maximumDurationMsByStage, events,
      limitation: "A bounded journal projection lacks an independently bound per-ACK send/TCP identity; missing logs do not prove fast egress." };
  }
  return output;
}
