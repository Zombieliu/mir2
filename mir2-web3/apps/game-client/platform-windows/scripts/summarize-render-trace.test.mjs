import test from 'node:test';
import assert from 'node:assert/strict';
import { summarize } from './summarize-render-trace.mjs';

test('keeps full health population separate from anomaly-selected samples and processes', async () => {
  const events = [
    { type: 'renderTraceSession', processId: 1 },
    { type: 'renderHealthSummary', processId: 1, frames: 600, frameDeltaBucketsLt33Lt50Lt100Ge100: [598, 1, 0, 1], maxFrameWallDeltaMs: 120 },
    { type: 'renderFrame', processId: 1, anomalies: ['presentationReverseCandidate'], frameId: 9, droppedRecords: 3 },
    { type: 'renderMarker', processId: 1, kind: 'movement', marker: { type: 'authoritative', tsDisposition: 'confirmed' } },
    { type: 'renderMarker', processId: 1, kind: 'cpuStage', marker: { stage: 'decode', durationMs: 42 } },
    { type: 'renderTraceSession', processId: 2 },
  ].map(JSON.stringify);
  const result = await summarize([...events, 'incomplete json']);
  assert.equal(result.malformedLines, 1);
  const p = result.processes[0];
  assert.equal(p.healthyWindowFrames, 600);
  assert.equal(p.capturedFrames, 1);
  assert.equal(p.maxDroppedRecords, 3);
  assert.equal(p.movementOutcomes.confirmed, 1);
  assert.equal(p.slowCpuStages.decode.ge33Ms, 1);
  assert.equal(p.reverseExamples[0].frameId, 9);
  assert.equal(result.processes[1].capturedFrames, 0);
});
