import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';

// Frame samples are anomaly-selected, not a representative FPS sample.
// Health summaries cover every consumed frame; keep the two populations apart.
export async function summarize(lines) {
  const processes = new Map();
  let malformedLines = 0;
  for await (const line of lines) {
    if (!line.trim()) continue;
    let e;
    try { e = JSON.parse(line); } catch { malformedLines++; continue; }
    const pid = String(e.processId ?? 'unknown');
    let p = processes.get(pid);
    if (!p) {
      p = { processId: pid, sessions: 0, captures: 0, capturedFrames: 0,
        healthyWindowFrames: 0, frameDeltaBucketsLt33Lt50Lt100Ge100: [0, 0, 0, 0],
        maxFrameWallDeltaMs: 0, maxDroppedRecords: 0, anomalies: {},
        movementOutcomes: {}, slowCpuStages: {}, reverseExamples: [], writerEvents: [] };
      processes.set(pid, p);
    }
    p.maxDroppedRecords = Math.max(p.maxDroppedRecords, Number(e.droppedRecords) || 0);
    if (e.type === 'renderTraceSession') p.sessions++;
    if (e.type === 'renderCaptureTriggered') p.captures++;
    if (e.type === 'renderHealthSummary') {
      p.healthyWindowFrames += Number(e.frames) || 0;
      (e.frameDeltaBucketsLt33Lt50Lt100Ge100 ?? []).forEach((n, i) => {
        if (i < 4) p.frameDeltaBucketsLt33Lt50Lt100Ge100[i] += Number(n) || 0;
      });
      p.maxFrameWallDeltaMs = Math.max(p.maxFrameWallDeltaMs, Number(e.maxFrameWallDeltaMs) || 0);
    }
    if (e.type === 'renderFrame') {
      p.capturedFrames++;
      for (const name of e.anomalies ?? []) p.anomalies[name] = (p.anomalies[name] ?? 0) + 1;
      if ((e.anomalies ?? []).includes('presentationReverseCandidate') && p.reverseExamples.length < 10) {
        p.reverseExamples.push({ unixMs: e.unixMs, frameId: e.frameId,
          visualWorldCenterStagePx: e.visualWorldCenterStagePx, movementCorrelation: e.movementCorrelation });
      }
    }
    if (e.type === 'renderMarker') {
      const m = e.marker ?? {};
      if (e.kind === 'movement' && m.type === 'authoritative') {
        const outcome = m.tsDisposition ?? 'unknown';
        p.movementOutcomes[outcome] = (p.movementOutcomes[outcome] ?? 0) + 1;
      }
      if (e.kind === 'cpuStage' && Number.isFinite(m.durationMs)) {
        const s = p.slowCpuStages[m.stage] ??= { count: 0, maxMs: 0, ge33Ms: 0 };
        s.count++; s.maxMs = Math.max(s.maxMs, m.durationMs);
        if (m.durationMs >= 33) s.ge33Ms++;
      }
    }
    if (/error|stop|budget|limit/i.test(e.type ?? '') && p.writerEvents.length < 20) p.writerEvents.push(e);
  }
  return { measurement: 'CPU main-world committed transforms; not GPU present or video',
    limitation: 'Captured frames are anomaly-selected. Health windows include focused/unfocused and startup frames; inspect focus/boundary flags. No zero-anomaly result proves visual acceptance.',
    malformedLines, processes: [...processes.values()] };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (!process.argv[2]) { console.error('Usage: node summarize-render-trace.mjs <render.jsonl>'); process.exitCode = 2; }
  else {
    const lines = createInterface({ input: createReadStream(process.argv[2]), crlfDelay: Infinity });
    console.log(JSON.stringify(await summarize(lines), null, 2));
  }
}
