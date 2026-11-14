import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import readline from 'node:readline';
import { pathToFileURL } from 'node:url';

const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const base = 'C:/mir2-build/p01-completion-20261024', root = path.join(base, 'formal-native-v2-ladder-02');
const output = path.join(base, 'formal02-analysis'), runId = '1792871143682-f3d8c161';
const rawFile = path.join(root, 'reports', runId + '.samples.jsonl'), reportFile = path.join(root, 'reports', runId + '.report.json');
const monitorFile = path.join(root, 'monitor-history.jsonl');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pin = file => { const bytes = fs.readFileSync(file); return { file, bytes: bytes.length, sha256: sha(bytes) }; };
const load = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const { zoneMetricsDelta } = await import(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-capacity-telemetry.mjs')).href);
const report = load(reportFile), cleanup = load(path.join(root, 'realm-cleanup.json')), provenance = load(path.join(root, 'report-provenance.json'));
const counts = {}, failures = [], windows = [], coverages = [], monitors = [], actions = [], selected = [], wallOffsets = [];
const rawHash = crypto.createHash('sha256'), monitorHash = crypto.createHash('sha256');
let rawBytes = 0, rawRows = 0, monitorBytes = 0, monitorRows = 0;
async function lines(file, hash, visit) {
  const input = fs.createReadStream(file);
  input.on('data', chunk => hash.update(chunk));
  const reader = readline.createInterface({ input, crlfDelay: Infinity });
  let bytes = 0, rows = 0;
  input.on('data', chunk => { bytes += chunk.length; });
  for await (const line of reader) {
    if (!line.trim()) continue;
    if (Buffer.byteLength(line) > 2_000_000) throw new Error('Unexpected oversized diagnostic input row');
    const row = JSON.parse(line); visit(row, ++rows);
  }
  return { bytes, rows, sha256: hash.digest('hex') };
}
const rawPin = { file: rawFile, ...await lines(rawFile, rawHash, (row, line) => {
  counts[row.type] = (counts[row.type] ?? 0) + 1;
  if (row.type === 'continuousFailure') failures.push({ line, ...row });
  if (row.type === 'continuousWindow' && !row.passed) windows.push({ line, ...row });
  if (row.type === 'coverage') coverages.push({ line, ...row });
  if (row.type === 'action') actions.push({ line, ...row });
  if (row.type === 'selfTransform' && wallOffsets.length < 2000) wallOffsets.push(row.at - row.monotonicMs);
  if (['phaseStarted', 'phaseEnded', 'safetyStop', 'finished'].includes(row.type)) selected.push({ line, ...row });
}) };
const monitorPin = { file: monitorFile, ...await lines(monitorFile, monitorHash, (row, line) => {
  if (row.type === 'monitor' && row.sample) monitors.push({ line, ...row.sample });
  else if (row.sampledAt != null && row.process) monitors.push({ line, ...row });
}) };
if (!monitors.length || failures.length !== 5 || windows.length !== 5 || report.continuousAcceptance.failureCount !== 5) throw new Error('Unexpected formal02 diagnostic shape');
const median = values => { const sorted = [...values].sort((a, b) => a - b); return sorted[Math.floor(sorted.length / 2)]; };
const quantiles = values => {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  const value = ratio => sorted.length ? sorted[Math.max(0, Math.ceil(sorted.length * ratio) - 1)] : null;
  return { count: sorted.length, p50: value(.5), p95: value(.95), p99: value(.99), max: sorted.at(-1) ?? null };
};
const wallOffsetMs = median(wallOffsets);
const timedActions = actions.map(row => ({ ...row, ...(Number.isFinite(row.sentAtMs) ? { estimatedSentWallMs: row.sentAtMs + wallOffsetMs } : {}) }));
function nearestBefore(samples, at) { return samples.findLast(row => row.sampledAt <= at); }
function nearestAfter(samples, at) { return samples.find(row => row.sampledAt >= at); }
function metricSummary(delta) {
  const hist = value => ({ count: value.count, p50UpperUs: value.p50UpperUs, p95UpperUs: value.p95UpperUs,
    p99UpperUs: value.p99UpperUs, p999UpperUs: value.p999UpperUs, overBudgetCount: value.overBudgetCount,
    overBudgetRatio: value.overBudgetRatio, lifetimeMaxBeforeUs: value.processLifetimeMaxBeforeUs, lifetimeMaxAfterUs: value.processLifetimeMaxAfterUs,
    exactWindowMaxUs: null });
  return Object.fromEntries(['zoneWork', 'ownerTickDeadlineLag', 'pendingMovementDeadlineLag', 'immediateRequests', 'lifecycleRequests', 'movementAdmissions']
    .map(key => [key, delta[key].duration ? { successCount: delta[key].successCount, failureCount: delta[key].failureCount, duration: hist(delta[key].duration) } : hist(delta[key])]));
}
function bracketDelta(start, end) {
  const before = nearestBefore(monitors, start), after = nearestAfter(monitors, end);
  if (!before || !after) return { available: false, start, end };
  const delta = zoneMetricsDelta(before.zoneMetrics, after.zoneMetrics);
  return { available: true, requestedStartAtMs: start, requestedEndAtMs: end, beforeSampleAtMs: before.sampledAt,
    afterSampleAtMs: after.sampledAt, beforeMarginMs: start - before.sampledAt, afterMarginMs: after.sampledAt - end,
    actualBracketSeconds: (after.sampledAt - before.sampledAt) / 1000, definition: delta.definition, scope: delta.scope,
    metrics: metricSummary(delta), rawDelta: delta };
}
function monitorProjection(row) {
  return { sampledAt: row.sampledAt, stop: row.stop, reason: row.reason, health: row.health, privateBytes: row.memory.currentBytes,
    workingSetBytes: row.memory.workingSetBytes, process: row.process, host: { freeBytes: row.host.freeBytes, totalBytes: row.host.totalBytes,
      cores: row.host.cores, meanLogicalBusyPercent: row.host.cores.reduce((sum, core) => sum + core.busyPercent, 0) / row.host.cores.length },
    network: row.network, zoneMetrics: row.zoneMetrics };
}
function resourceSummary(start, end) {
  const rows = monitors.filter(row => row.sampledAt >= start && row.sampledAt <= end), rates = rows.map(row => row.process.rates).filter(Boolean);
  const coreIds = [...new Set(rows.flatMap(row => row.host.cores.map(core => core.id)))].sort((a, b) => Number(a) - Number(b));
  return { samples: rows.length, firstSampleAt: rows[0]?.sampledAt, lastSampleAt: rows.at(-1)?.sampledAt,
    processCpuCores: quantiles(rates.map(row => row.cpuCores)), processIoOperationsPerSecond: quantiles(rates.map(row => row.ioOperationsPerSecond)),
    processReadBytesPerSecond: quantiles(rates.map(row => row.readBytesPerSecond)), processWriteBytesPerSecond: quantiles(rates.map(row => row.writeBytesPerSecond)),
    processOtherIoBytesPerSecond: quantiles(rates.map(row => row.otherIoBytesPerSecond)),
    privateBytes: quantiles(rows.map(row => row.memory.currentBytes)), workingSetBytes: quantiles(rows.map(row => row.memory.workingSetBytes)),
    hostFreeBytes: { minimum: Math.min(...rows.map(row => row.host.freeBytes)), ...quantiles(rows.map(row => row.host.freeBytes)) },
    hostLogicalCoreBusyPercent: coreIds.map(id => ({ id, ...quantiles(rows.map(row => row.host.cores.find(core => core.id === id)?.busyPercent)) })),
    driverEventLoop: { p99SamplesMs: quantiles(coverages.filter(row => row.at >= start && row.at <= end).map(row => row.driver.eventLoopLagP99Ms)),
      maximumSamplesMs: quantiles(coverages.filter(row => row.at >= start && row.at <= end).map(row => row.driver.eventLoopLagMaxMs)) },
    limits: { cpuCores: 24, ioOperationsPerSecond: 100000, ioBytesPerSecond: 536870912, privateBytes: 34359738368, workingSetBytes: 42949672960,
      hostFreeBytesMinimum: 8589934592, driverEventLoopP99MsFor3Samples: 250 } };
}
const allTimeouts = timedActions.filter(row => ['timeout', 'transportError', 'error', 'cancelled'].includes(row.status));
const slowActions = timedActions.filter(row => Number.isFinite(row.latencyMs) && row.latencyMs >= 250);
const peakActions = [...slowActions].sort((a, b) => b.latencyMs - a.latencyMs).slice(0, 80);
const failedWindowDetails = failures.map((failure, index) => {
  const matches = windows.filter(row => row.actor === failure.evidence.actor && row.seconds === failure.evidence.seconds && row.final === failure.evidence.final);
  if (matches.length !== 1) throw new Error('Failure has no unique matching original continuousWindow');
  const match = matches[0], pausedSeconds = failure.evidence.final ? report.continuousAcceptance.actors.find(row => row.name === match.actor).plannedPauseSeconds : 0;
  const end = match.at - pausedSeconds * 1000, start = end - failure.evidence.seconds * 1000;
  const timeoutRows = allTimeouts.filter(row => row.actor === failure.evidence.actor && row.at >= start - 5 && row.at <= end + 5);
  return { index: index + 1, originalFailure: failure, originalContinuousWindow: match,
    wallWindow: { estimatedStartAtMs: start, estimatedActiveEndAtMs: end, emittedWindowAtMs: match.at, emittedFailureAtMs: failure.at,
      finalPauseSecondsRemoved: pausedSeconds,
      rule: 'Interval is an estimate from emitted wall time minus recorded active monotonic duration. For final segments, remove the reported actor planned-pause duration (this run has only the common final drain pause). No original per-window wall-start exists; do not claim exact packet boundaries.' },
    timeoutRows, actorSlowRows: slowActions.filter(row => row.actor === failure.evidence.actor && row.at >= start && row.at <= end),
    resources: resourceSummary(start, end), serverTelemetryBracket: bracketDelta(start, end) };
});
const firstFailureAt = failures[0].at;
const firstFaultAt = Math.min(...allTimeouts.filter(row => row.phase === 'stage-25').map(row => row.at));
const episodeStart = Number.isFinite(firstFaultAt) ? firstFaultAt - 10000 : firstFailureAt - 10000;
const episodeEnd = firstFailureAt + 6000;
const timeoutAssociations = allTimeouts.filter(row => row.kind.startsWith('fanout-')).map(row => {
  const estimate = row.at - row.latencyMs;
  const candidates = timedActions.filter(action => action.actor === row.actor && action.kind === row.kind.slice(7) &&
    Number.isFinite(action.estimatedSentWallMs) && Math.abs(action.estimatedSentWallMs - estimate) <= 15).map(action => ({
      line: action.line, at: action.at, actor: action.actor, kind: action.kind, status: action.status, sentAtMs: action.sentAtMs,
      estimatedSentWallMs: action.estimatedSentWallMs, latencyMs: action.latencyMs, plannedLatencyMs: action.plannedLatencyMs,
      actual: action.actual, plannedTarget: action.plannedTarget, startDifferenceMs: action.estimatedSentWallMs - estimate }));
  return { timeout: row, estimatedCommonSendWallMs: estimate, candidates, associationRule: 'Time-correlated candidates only. Movement/fanout raw rows do not contain a request ID, so do not claim an exact packet association.' };
});
const phaseAnalysis = report.phaseWindows.map(phase => ({ ...phase, measuredDurationSeconds: (phase.endedAtMs - phase.startedAtMs) / 1000,
  resources: resourceSummary(phase.startedAtMs, phase.endedAtMs), serverTelemetryBracket: bracketDelta(phase.startedAtMs, phase.endedAtMs) }));
const perInterval = monitors.slice(1).map((after, index) => {
  const before = monitors[index], delta = zoneMetricsDelta(before.zoneMetrics, after.zoneMetrics);
  return { from: before.sampledAt, to: after.sampledAt, seconds: (after.sampledAt - before.sampledAt) / 1000,
    metrics: metricSummary(delta), resources: monitorProjection(after) };
});
const criticalIntervals = perInterval.filter(row => row.from <= episodeEnd && row.to >= episodeStart);
const coverageEpisode = coverages.filter(row => row.at >= episodeStart && row.at <= episodeEnd);
const peakDriverRows = [...coverages].sort((a, b) => b.driver.eventLoopLagMaxMs - a.driver.eventLoopLagMaxMs).slice(0, 30);
const final = {
  schemaVersion: 1, analyzedAt: new Date().toISOString(), runId, backendSourceCommit: provenance.backendSourceCommit, driverSourceCommit: provenance.driverSourceCommit,
  scope: 'Read-only streaming posthoc diagnosis of the original failed formal02. No thresholds, source, verdict or original evidence are changed. No private files or recording payloads read.',
  originalOutcome: { ok: report.ok, error: report.error, highestPassedStage: report.highestPassedStage, highestMeasuredStage: report.highestMeasuredStage,
    playableCapacityAccepted: report.playableCapacityAccepted, failureCount: report.continuousAcceptance.failureCount, stages: report.stages.map(({ name, controlledActors, passed }) => ({ name, controlledActors, passed })),
    phaseWindows: report.phaseWindows, cleanupChecks: cleanup.checks },
  inputPins: [rawPin, monitorPin, pin(reportFile), pin(path.join(root, 'realm-cleanup.json')), pin(path.join(root, 'report-provenance.json')),
    pin(path.join(project, 'apps/web/scripts/playtest-capacity-soak.mjs')), pin(path.join(project, 'apps/web/scripts/playtest-capacity-soak-core.mjs')),
    pin(path.join(project, 'apps/web/scripts/playtest-capacity-monitor.mjs')), pin(path.join(project, 'apps/web/scripts/playtest-capacity-telemetry.mjs'))],
  rawRows: rawPin.rows, rawTypeCounts: counts, standaloneMonitorRows: monitorPin.rows, standaloneMonitorSamples: monitors.length,
  clockMapping: { medianWallMinusMonotonicMs: wallOffsetMs, minimumOffsetMs: Math.min(...wallOffsets), maximumOffsetMs: Math.max(...wallOffsets),
    samples: wallOffsets.length, use: 'Estimated action send wall times for diagnostic correlation only, never protocol ACK identity.' },
  allFiveFailedWindows: failedWindowDetails, phaseAnalysis,
  episode: { startAtMs: episodeStart, endAtMs: episodeEnd, firstFanoutTimeoutAt: firstFaultAt, firstFailureEmittedAt: firstFailureAt,
    allTimeouts, timeoutAssociations, peakActions, criticalIntervals, coverageEpisode, peakDriverRows },
  limitations: [
    'Owner/fanout timings are observed by the same Node process. They include local socket scheduling and callback execution; raw movement rows have no correlated request ID or complete packet receive trace.',
    'CIM resource rates are sampled intervals (roughly seconds), not exact spans of a subsecond or one-second stall. Host logical-core utilization includes other processes and cannot assign a busy core to Gateway.',
    'Process IO aggregates file/device/network operations and is not disk IOPS, flush latency or queue depth. No ETW, thread scheduling, storage latency or per-lock tracing was recorded.',
    'Zone histograms measure shared-owner work and handler admissions, not authoritative position-ACK or outbound peer delivery. Only raw bucket differences are used; cumulative maxima cannot yield exact interval maxima.',
    'No spectator frame-count timeline is present in the public monitor whitelist. Ring-full timing or a particular recording flush cannot be proven by these inputs; no recording payload was opened.',
    'Failure of stage25 fanout is real under the frozen profile. Correlation or spare aggregate CPU cannot establish a unique root cause or classify the result as a physical-machine maximum.',
    'The5 original failures include post-stop/final actor segments if present. Their original final/phases/seconds are retained rather than treating them as5 independent pre-stop overload episodes.'
  ],
  operations: { harnessChanged: false, trackedFilesWritten: false, testsOrBuildsRun: false, privateRead: false, recordingPayloadRead: false, externalAnalysisOnly: true }
};
await fsp.mkdir(output, { recursive: true });
for (const [name, value] of [['diagnostic-v1.json', final], ['selected-events-v1.json', { runId, phaseEvents: selected, failures,
  timeoutRows: allTimeouts, slowActionRows: slowActions, driverEpisode: coverageEpisode, timeoutAssociations }]]) {
  await fsp.writeFile(path.join(output, name), JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
}
console.log(JSON.stringify({ outputs: ['diagnostic-v1.json', 'selected-events-v1.json'].map(name => pin(path.join(output, name))), rawPin,
  failWindows: failedWindowDetails.map(({ originalFailure: { at, evidence }, resources }) => ({ at, actor: evidence.actor, final: evidence.final,
    seconds: evidence.seconds, activityPassed: evidence.activityPassed, latencyPassed: evidence.latencyPassed, fanoutPassed: evidence.fanoutPassed,
    timeoutRatio: evidence.timeoutRatio, correctionRatio: evidence.correctionRatio, processCpu: resources.processCpuCores,
    driverLag: resources.driverEventLoop })), timeoutCount: allTimeouts.length, episodeStart, episodeEnd, peakDriver: peakDriverRows[0]?.driver }));
