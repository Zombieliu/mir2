import assert from "node:assert/strict";

import { analyzeLocalCommandPoseLatency } from "./local-command-pose-latency.mjs";

const commands = [
  { movementSeq: 3, type: "walk", direction: "Left", at: 2_000 },
  { movementSeq: 2, type: "turn", direction: "Left", at: 1_500 },
  { movementSeq: 1, type: "walk", direction: "Right", at: 1_000 },
];
const probe = {
  armedAtMs: 900,
  sinkCallbackCount: 20,
  droppedSinkEventCount: 0,
  sinkEvents: [
    {
      frameId: 11,
      generatedAtMs: 1_042,
      sinkAtMs: 1_044,
      cameraSource: "localCommand",
      cameraX: -8,
      cameraY: 0,
    },
    // This event is after the turn boundary and must not be assigned to seq 1.
    {
      frameId: 12,
      generatedAtMs: 1_520,
      sinkAtMs: 1_521,
      cameraSource: "localCommand",
      cameraX: -16,
      cameraY: 0,
    },
    {
      frameId: 13,
      generatedAtMs: 2_048,
      sinkAtMs: 2_050,
      cameraSource: "localCommand",
      cameraX: 8,
      cameraY: 0,
    },
  ],
};

const result = analyzeLocalCommandPoseLatency(commands, probe, 75);
assert.equal(result.eligibleCommandCount, 2);
assert.equal(result.matchedCommandCount, 2);
assert.equal(result.coverageComplete, true);
assert.equal(result.responsive, true);
assert.equal(result.maxCommandToPoseMs, 48);
assert.equal(result.maxCommandToSinkMs, 50);
assert.deepEqual(result.samples.map((sample) => sample.movementSeq), [1, 3]);

const missing = analyzeLocalCommandPoseLatency(
  commands,
  { ...probe, sinkEvents: probe.sinkEvents.slice(1) },
  75,
);
assert.equal(missing.coverageComplete, false);
assert.deepEqual(missing.missingCommands.map((command) => command.movementSeq), [1]);

const slow = analyzeLocalCommandPoseLatency(
  [{ movementSeq: 1, type: "walk", at: 1_000 }],
  {
    sinkCallbackCount: 1,
    droppedSinkEventCount: 0,
    sinkEvents: [
      {
        frameId: 1,
        generatedAtMs: 1_080,
        sinkAtMs: 1_081,
        cameraSource: "localCommand",
        cameraX: 8,
        cameraY: 0,
      },
    ],
  },
  75,
);
assert.equal(slow.coverageComplete, true);
assert.equal(slow.responsive, false);

console.log("local command pose latency tests passed");
