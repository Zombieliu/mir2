import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';
import crypto from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const directory = path.dirname(fileURLToPath(import.meta.url));
const base = path.dirname(directory);
const realm = path.join(base, 'release-native-v2-account-store-wait-ladder-13');
const runId = '1792959622496-365f0175';
const reportFile = path.join(realm, 'reports', `${runId}.report.json`);
const historyFile = path.join(realm, 'monitor-history.jsonl');
const read = async file => JSON.parse(await fs.readFile(file, 'utf8'));
const pin = async file => {
  const bytes = await fs.readFile(file);
  return { file, bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
};
const stats = values => {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  const percentile = p => sorted.length ? sorted[Math.max(0, Math.ceil(p * sorted.length) - 1)] : null;
  return { samples: sorted.length, minimum: sorted[0] ?? null, p50: percentile(.5), p95: percentile(.95), p99: percentile(.99), maximum: sorted.at(-1) ?? null };
};
const report = await read(reportFile);
if (report.runId !== runId || report.ok !== false || report.playableCapacityAccepted !== false) throw Error('Original failed report binding changed');
const phases = report.phaseWindows.filter(p => p.kind === 'measurement');
if (phases.map(p => p.name).join(',') !== 'stage-2,stage-10,stage-25') throw Error('Unexpected actually executed phases');
const before = await fs.stat(historyFile), hash = crypto.createHash('sha256');
const stream = createReadStream(historyFile);
stream.on('data', bytes => hash.update(bytes));
const samples = [];
let rows = 0, nonSampleRows = 0;
for await (const line of createInterface({ input: stream, crlfDelay: Infinity })) {
  if (!line.length) throw Error('Empty monitor row');
  rows++;
  const sample = JSON.parse(line);
  if (!Number.isSafeInteger(sample.sampledAt) || !sample.memory || !sample.process || !sample.host) { nonSampleRows++; continue; }
  if (!Number.isFinite(sample.memory.currentBytes) || !Number.isFinite(sample.memory.workingSetBytes) || !Number.isFinite(sample.host.freeBytes)) throw Error('Malformed resource sample');
  samples.push(sample);
}
const after = await fs.stat(historyFile);
if (before.size !== after.size || before.mtimeMs !== after.mtimeMs || stream.bytesRead !== after.size) throw Error('Ended monitor source changed');
const summarize = selected => ({
  sampleCount: selected.length,
  firstSampledAtMs: selected[0]?.sampledAt ?? null,
  lastSampledAtMs: selected.at(-1)?.sampledAt ?? null,
  maximumSampleGapMs: selected.length > 1 ? Math.max(...selected.slice(1).map((s, i) => s.sampledAt - selected[i].sampledAt)) : null,
  gatewayLogicalCpuEquivalents: stats(selected.map(s => s.process.rates?.cpuCores)),
  gatewayPrivateBytes: stats(selected.map(s => s.memory.currentBytes)),
  gatewayWorkingSetBytes: stats(selected.map(s => s.memory.workingSetBytes)),
  hostFreeBytes: stats(selected.map(s => s.host.freeBytes)),
  hostBusiestLogicalCorePercent: stats(selected.map(s => Math.max(...(s.host.cores ?? []).map(c => c.busyPercent).filter(Number.isFinite)))),
  gatewayProcessReadBytesPerSecond: stats(selected.map(s => s.process.rates?.readBytesPerSecond)),
  gatewayProcessWriteBytesPerSecond: stats(selected.map(s => s.process.rates?.writeBytesPerSecond)),
  gatewayProcessOtherIoBytesPerSecond: stats(selected.map(s => s.process.rates?.otherIoBytesPerSecond)),
  hostNicBytesPerSecond: stats(selected.map(s => s.network?.hostBytesPerSecond)),
  ownerWorkObservedMaxUs: Math.max(...selected.map(s => s.zoneMetrics?.zoneWork?.maxUs ?? 0)),
  ownerDeadlineLagObservedMaxUs: Math.max(...selected.map(s => s.zoneMetrics?.ownerTickDeadlineLag?.maxUs ?? 0)),
});
const durationSeconds = (Date.parse(report.finishedAt) - Date.parse(report.startedAt)) / 1000;
const result = {
  schemaVersion: 1, analyzedAtUtc: new Date().toISOString(), runId,
  scope: 'Ended Formal13 sampled resource observations, including a failed combat-supply phase. No capacity acceptance or hardware ceiling inferred.',
  method: 'Stream original ended monitor JSONL; select measurement samples by sampledAt >= phase start and < phase end. Percentiles use nearest rank. Rate intervals may straddle a phase boundary; observed maxima omit between-sample spikes. Cumulative owner maxima include preceding phases.',
  sources: { analyzer: await pin(fileURLToPath(import.meta.url)), report: await pin(reportFile), monitorHistory: { file: historyFile, bytes: after.size, sha256: hash.digest('hex'), rows, sampleRows: samples.length, nonSampleRows }, profile: await pin(path.join(realm, 'resource-profile.json')) },
  environment: samples[0]?.environment ?? null,
  allMonitorSamples: summarize(samples),
  phases: phases.map(phase => ({ name: phase.name, targetActors: phase.targetActors, startedAtMs: phase.startedAtMs, endedAtMs: phase.endedAtMs, measuredSeconds: (phase.endedAtMs - phase.startedAtMs) / 1000, ...summarize(samples.filter(s => s.sampledAt >= phase.startedAtMs && s.sampledAt < phase.endedAtMs)) })),
  wholeDriverPayload: { startedAt: report.startedAt, finishedAt: report.finishedAt, durationSeconds, sentBytes: report.bytes.sent, receivedBytes: report.bytes.received, definition: report.bytesDefinition, averageCombinedPayloadMbitPerSecond: (report.bytes.sent + report.bytes.received) * 8 / durationSeconds / 1e6, scope: 'Whole driver run, including bootstrap/ramp and stages; not a stage25 bandwidth measurement, wire framing, TLS or physical NIC ceiling.' },
  originalReportOk: false, capacityAccepted: false, p0Passed: false, p2Started: false,
  limits: ['Host logical-core samples include the load clients and unrelated host processes.', 'Gateway process IO counters include file/device IO; they do not establish pure disk throughput, latency, fsync p99 or storage capacity.', 'Host NIC counters may omit loopback traffic and do not measure this gateway alone.', 'No stage50, stage100, two-hour soak, production database, TLS/WAN, browser, hard cloud quota or N+1 result exists here.', 'Memory grows during this short mixed-cohort run; this does not establish a two-hour plateau or a leak.', 'Stage25 failed the original combat-supply windows despite successful issued attacks; sampled low CPU is not a passing capacity result.'],
};
const output = path.join(directory, 'analysis-v1.json');
await fs.writeFile(output, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ output: await pin(output), rows, sampleRows: samples.length, nonSampleRows, phases: result.phases.map(p => ({ name: p.name, samples: p.sampleCount, cpuP95: p.gatewayLogicalCpuEquivalents.p95, cpuMax: p.gatewayLogicalCpuEquivalents.maximum, privateMaxMiB: p.gatewayPrivateBytes.maximum / 1048576, hostFreeMinGiB: p.hostFreeBytes.minimum / 1073741824 })) }));
