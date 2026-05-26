import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const controllerPath = new URL("../app/components/original-client-movement-controller.ts", import.meta.url);
const pagePath = new URL("../app/page.tsx", import.meta.url);
const source = readFileSync(controllerPath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(controllerPath),
});

const module = { exports: {} };
const load = new Function("exports", "module", compiled.outputText);
load(module.exports, module);

const {
  CRYSTAL_MOVE_DELAY_MS,
  CRYSTAL_RUN_PRIME_MS,
  canSendMovement,
  createPendingSelfMove,
  effectiveCrystalMovementMode,
  reconcileMovementAck,
  reconcileMovementSnapshot,
} = module.exports;

const initialState = () => ({
  pending: null,
  prediction: null,
  nextMoveSendAt: 0,
  runPrimedUntil: 0,
  inputBlockedUntil: 0,
});

{
  const pending = createPendingSelfMove({
    from: { x: 10, y: 20, direction: "Right" },
    direction: "Right",
    requestedMode: "run",
    now: 1_000,
    runPrimedUntil: 0,
  });
  assert.equal(effectiveCrystalMovementMode("run", 1_000, 0), "walk");
  assert.equal(pending.mode, "walk");
  assert.deepEqual(pending.to, { x: 11, y: 20, direction: "Right" });
}

{
  const pending = createPendingSelfMove({
    from: { x: 10, y: 20, direction: "Right" },
    direction: "Right",
    requestedMode: "walk",
    now: 2_000,
    runPrimedUntil: 0,
  });
  const result = reconcileMovementAck({
    state: {
      ...initialState(),
      pending,
      prediction: pending.to,
      nextMoveSendAt: pending.sentAt + CRYSTAL_MOVE_DELAY_MS,
    },
    ack: { x: 11, y: 20, direction: "Right" },
    packetName: "UserLocation",
    now: 2_100,
  });
  assert.equal(result.outcome, "confirmed");
  assert.equal(result.state.pending, null);
  assert.equal(result.state.runPrimedUntil, 2_100 + CRYSTAL_RUN_PRIME_MS);
}

{
  const now = 3_000;
  const runPrimedUntil = now + 500;
  const pending = createPendingSelfMove({
    from: { x: 11, y: 20, direction: "Right" },
    direction: "Right",
    requestedMode: "run",
    now,
    runPrimedUntil,
  });
  const state = {
    ...initialState(),
    pending,
    prediction: pending.to,
    nextMoveSendAt: now + CRYSTAL_MOVE_DELAY_MS,
    runPrimedUntil,
  };
  assert.equal(pending.mode, "run");
  assert.deepEqual(pending.to, { x: 13, y: 20, direction: "Right" });
  assert.equal(canSendMovement(state, now + 599), false);
  assert.equal(canSendMovement({ ...state, pending: null }, now + 600), true);
}

{
  const pending = createPendingSelfMove({
    from: { x: 20, y: 20, direction: "Right" },
    direction: "Right",
    requestedMode: "walk",
    now: 4_000,
    runPrimedUntil: 0,
  });
  const result = reconcileMovementAck({
    state: {
      ...initialState(),
      pending,
      prediction: pending.to,
      runPrimedUntil: 4_500,
    },
    ack: { x: 20, y: 20, direction: "Right" },
    packetName: "UserLocation",
    now: 4_120,
  });
  assert.equal(result.outcome, "correction");
  assert.equal(result.state.pending, null);
  assert.equal(result.state.prediction, null);
  assert.equal(result.state.runPrimedUntil, 0);
}

{
  const pending = createPendingSelfMove({
    from: { x: 30, y: 30, direction: "Down" },
    direction: "Down",
    requestedMode: "walk",
    now: 5_000,
    runPrimedUntil: 0,
  });
  const result = reconcileMovementSnapshot({
    state: {
      ...initialState(),
      pending,
      prediction: pending.to,
      runPrimedUntil: 5_500,
    },
    snapshot: { x: 30, y: 30, direction: "Down" },
    now: 5_200,
  });
  assert.equal(result.corrected, true);
  assert.equal(result.state.pending, null);
  assert.equal(result.state.prediction, null);
  assert.equal(result.state.runPrimedUntil, 0);
}

{
  const pageSource = readFileSync(pagePath, "utf8");
  assert.equal(
    /send\(\{\s*type:\s*["']moveTo["']/.test(pageSource),
    false,
    "normal UI movement must not send debug moveTo packets",
  );
}

console.log("movement controller tests passed");
