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
  clampMovementLeadToCap,
  stepMovementTowardWithinCap,
  createPendingSelfMove,
  effectiveCrystalMovementMode,
  movementTileMatches,
  movementTransformMatches,
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
  assert.equal(
    movementTileMatches({ x: 10, y: 10, direction: "Left" }, { x: 10, y: 10, direction: "Right" }),
    true,
  );
  assert.equal(
    movementTransformMatches({ x: 10, y: 10, direction: "Left" }, { x: 10, y: 10, direction: "Right" }),
    false,
  );
}

{
  const pending = createPendingSelfMove({
    from: { x: 40, y: 40, direction: "Right" },
    direction: "Right",
    requestedMode: "walk",
    now: 5_400,
    runPrimedUntil: 0,
  });
  const result = reconcileMovementAck({
    state: {
      ...initialState(),
      pending,
      prediction: pending.to,
    },
    ack: { x: 41, y: 40, direction: "Left" },
    packetName: "UserLocation",
    now: 5_500,
  });
  assert.equal(result.outcome, "confirmed", "movement ACK confirms by tile, not render direction");
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
  const result = reconcileMovementSnapshot({
    state: {
      ...initialState(),
      pending: null,
      prediction: { x: 50, y: 50, direction: "Right" },
      runPrimedUntil: 5_900,
    },
    snapshot: { x: 50, y: 50, direction: "Left" },
    now: 5_800,
  });
  assert.equal(result.corrected, true, "snapshot direction mismatch must correct same-tile prediction");
  assert.equal(result.state.prediction, null);
  assert.equal(result.state.runPrimedUntil, 0);
}

{
  // clampMovementLeadToCap — the lead-cap clamp that replaces the hard discard so
  // an over-cap self-prediction eases to the cap boundary instead of snapping the
  // sprite back to the server tile (the "overshoot then snap" drift).

  // In-cap candidates pass through untouched (normal movement is unaffected).
  assert.deepEqual(
    clampMovementLeadToCap({ x: 10, y: 10 }, { x: 12, y: 10, direction: "Right" }, 2),
    { x: 12, y: 10, direction: "Right" },
    "a candidate already within the cap is returned unchanged",
  );

  // An axis-aligned over-cap lead (3 tiles) clamps to exactly the cap, preserving direction.
  assert.deepEqual(
    clampMovementLeadToCap({ x: 10, y: 10 }, { x: 13, y: 10, direction: "Right" }, 2),
    { x: 12, y: 10, direction: "Right" },
    "an over-cap lead clamps to the cap along the travel axis",
  );

  // Diagonal over-cap lead clamps on both axes (Chebyshev distance == cap).
  assert.deepEqual(
    clampMovementLeadToCap({ x: 0, y: 0 }, { x: 3, y: 3, direction: "DownRight" }, 2),
    { x: 2, y: 2, direction: "DownRight" },
    "a diagonal over-cap lead clamps on both axes",
  );

  // Negative direction and a mixed axis (one within, one beyond) both clamp correctly.
  assert.deepEqual(
    clampMovementLeadToCap({ x: 0, y: 0 }, { x: -4, y: 0, direction: "Left" }, 2),
    { x: -2, y: 0, direction: "Left" },
    "a negative over-cap lead clamps toward the origin",
  );
  assert.deepEqual(
    clampMovementLeadToCap({ x: 0, y: 0 }, { x: 3, y: 1, direction: "Right" }, 2),
    { x: 2, y: 1, direction: "Right" },
    "only the over-cap axis is clamped; the in-cap axis is preserved",
  );

  // The clamped result is never behind the origin (stays between origin and candidate)
  // and never leads by more than the cap.
  const clamped = clampMovementLeadToCap({ x: 100, y: 100 }, { x: 109, y: 94, direction: "UpRight" }, 2);
  const clampedLead = Math.max(Math.abs(clamped.x - 100), Math.abs(clamped.y - 100));
  assert.equal(clampedLead, 2, "clamped lead equals the cap");
  assert.equal(Math.sign(clamped.x - 100), 1, "clamp preserves the +x travel direction");
  assert.equal(Math.sign(clamped.y - 100), -1, "clamp preserves the -y travel direction");
}

{
  // stepMovementTowardWithinCap — caps the rendered self tile to <=1 tile/frame so
  // a long/dropped frame (or a direction reversal across the server tile) cannot
  // collapse several 1-tile transitions into one visible jump (the residual
  // "overshoot then snap" left after the lead-cap clamp).

  // A <=cap advance passes through untouched (normal movement is fully responsive).
  assert.deepEqual(
    stepMovementTowardWithinCap({ x: 5, y: 5 }, { x: 6, y: 5, direction: "Right" }, 1),
    { x: 6, y: 5, direction: "Right" },
    "a within-cap advance is returned unchanged",
  );
  assert.deepEqual(
    stepMovementTowardWithinCap({ x: 5, y: 5 }, { x: 6, y: 4, direction: "UpRight" }, 1),
    { x: 6, y: 4, direction: "UpRight" },
    "a within-cap diagonal advance is returned unchanged",
  );

  // A >1-tile single-frame jump eases ONE tile toward the target along the vector.
  assert.deepEqual(
    stepMovementTowardWithinCap({ x: 5, y: 5 }, { x: 7, y: 5, direction: "Right" }, 1),
    { x: 6, y: 5, direction: "Right" },
    "a 2-tile forward jump eases one tile toward the target",
  );

  // The reversal-across-server case from the captured snap: render leading at
  // 303,91 (down-left), new prediction 305,89 (up-right); a single-frame jump would
  // be 2 tiles. Easing routes it through the server tile 304,90 (one tile), then the
  // next frame reaches 305,89 — two physical 1-tile steps, no snap.
  const easedReversal = stepMovementTowardWithinCap({ x: 303, y: 91 }, { x: 305, y: 89, direction: "UpRight" }, 1);
  assert.deepEqual(
    easedReversal,
    { x: 304, y: 90, direction: "UpRight" },
    "a reversal across the server eases through the in-between (server) tile",
  );
  assert.deepEqual(
    stepMovementTowardWithinCap(easedReversal, { x: 305, y: 89, direction: "UpRight" }, 1),
    { x: 305, y: 89, direction: "UpRight" },
    "the following frame reaches the target (delta now within cap)",
  );

  // No single step ever exceeds the cap, regardless of how far the target is.
  const farStep = stepMovementTowardWithinCap({ x: 0, y: 0 }, { x: 9, y: -4, direction: "UpRight" }, 1);
  assert.equal(Math.max(Math.abs(farStep.x), Math.abs(farStep.y)), 1, "a far target still advances at most the cap");
}

{
  // page.tsx must clamp the over-cap self-render lead rather than discard it: both
  // chooseCrystalSelfRenderPosition (the candidate gate) and
  // preserveCrystalSelfRenderPosition (the final position gate) route an over-cap
  // lead through clampMovementLeadToCap instead of dropping it to null.
  const pageSource = readFileSync(pagePath, "utf8");
  assert.match(
    pageSource,
    /clampMovementLeadToCap\(serverSelf,\s*candidate,\s*MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES\)/,
    "chooseCrystalSelfRenderPosition must clamp over-cap candidates to the render lead cap",
  );
  assert.match(
    pageSource,
    /clampMovementLeadToCap\(serverSelf,\s*next,\s*MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES\)/,
    "preserveCrystalSelfRenderPosition must clamp an over-cap lead instead of discarding it",
  );
  // …and must additionally ease a valid leading prediction to <=1 tile/frame so a
  // dropped-frame collapse / reversal cannot snap (the residual after #136).
  assert.match(
    pageSource,
    /easeSelfRenderTile\(next,\s*serverSelf,\s*now\)/,
    "preserveCrystalSelfRenderPosition must ease a valid lead via easeSelfRenderTile",
  );
  assert.match(
    pageSource,
    /stepMovementTowardWithinCap\(base,\s*target,\s*MOVEMENT_RENDER_MAX_STEP_TILES_PER_FRAME\)/,
    "easeSelfRenderTile must cap the per-frame render advance",
  );
}

{
  const pageSource = readFileSync(pagePath, "utf8");
  assert.equal(
    /send\(\{\s*type:\s*["']moveTo["']/.test(pageSource),
    false,
    "normal UI movement must not send debug moveTo packets",
  );
  for (const functionName of ["consumeQueuedDirectionStep", "tickMovementPlan"]) {
    const functionBody = pageSource.match(new RegExp(`function ${functionName}\\\\(\\\\) \\\\{([\\\\s\\\\S]*?)\\\\n  \\\\}`))?.[1] ??
      pageSource.match(new RegExp(`const ${functionName} = \\\\(\\\\) => \\\\{([\\\\s\\\\S]*?)\\\\n    \\\\};`))?.[1] ??
      "";
    assert.equal(
      /send\(\{\s*type:\s*["'](?:walk|run|turn)["']/.test(functionBody),
      false,
      `${functionName} must not send self movement packets`,
    );
  }
}

{
  // Replay the qa-load-stress overshoot/snap scenario through faithful models of the
  // OLD and NEW render-position selection (chooseCrystalSelfRenderPosition) and assert
  // the metric that harness measures — a backward render jump of >= 2 tiles in one
  // frame (SNAP_BACK_TILES) — drops from >0 (OLD: hard-discard → snap to server) to 0
  // (NEW: clamp the over-cap lead to the cap so the sprite eases forward). The clamp
  // under test is the REAL exported helper; only the tiny selection wrapper is modelled.
  const CAP = 2;
  const cheby = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
  // All replay candidates run strictly forward of the server, so "not behind" holds.

  // OLD gate: filter out any candidate leading past the cap → null when all exceed it,
  // and the renderer then falls back to the authoritative server tile.
  const chooseOld = (server, candidate) => {
    if (!candidate || cheby(candidate, server) > CAP) return null;
    return candidate;
  };
  // NEW gate: clamp an over-cap candidate to the cap instead of dropping it.
  const chooseNew = (server, candidate) => {
    if (!candidate) return null;
    return cheby(candidate, server) <= CAP
      ? candidate
      : clampMovementLeadToCap(server, candidate, CAP);
  };
  const renderTile = (gate, server, candidate) => {
    const chosen = gate(server, candidate);
    // The render falls back to the server tile when no prediction survives the gate.
    return chosen ? { x: chosen.x, y: chosen.y } : { x: server.x, y: server.y };
  };

  // server tile (lags) + raw prediction tile (leads): a sustained run where a dropped
  // frame over-advances the prediction to a 3-tile lead (frame 2), then the server
  // catches up step by step.
  const frames = [
    { server: { x: 10, y: 10 }, pred: { x: 11, y: 10, direction: "Right" } }, // lead 1
    { server: { x: 10, y: 10 }, pred: { x: 12, y: 10, direction: "Right" } }, // lead 2 (cap)
    { server: { x: 10, y: 10 }, pred: { x: 13, y: 10, direction: "Right" } }, // lead 3 (over-advance)
    { server: { x: 11, y: 10 }, pred: { x: 13, y: 10, direction: "Right" } }, // server catches up
    { server: { x: 12, y: 10 }, pred: { x: 13, y: 10, direction: "Right" } },
    { server: { x: 13, y: 10 }, pred: { x: 13, y: 10, direction: "Right" } }, // settled
  ];

  const countBackSnaps = (gate) => {
    let snaps = 0;
    let prev = null;
    let maxRenderLead = 0;
    for (const frame of frames) {
      const rendered = renderTile(gate, frame.server, frame.pred);
      maxRenderLead = Math.max(maxRenderLead, cheby(rendered, frame.server));
      if (prev) {
        // A backward render jump = the rendered tile moving toward the server by >= 2
        // tiles in a single frame (the visible snap; forward motion is excluded).
        const jump = cheby(rendered, prev);
        const towardServer = cheby(prev, frame.server) > cheby(rendered, frame.server);
        if (jump >= 2 && towardServer) snaps += 1;
      }
      prev = rendered;
    }
    return { snaps, maxRenderLead };
  };

  const oldResult = countBackSnaps(chooseOld);
  const newResult = countBackSnaps(chooseNew);
  assert.ok(oldResult.snaps > 0, "baseline (hard-discard) reproduces the overshoot/snap drift");
  assert.equal(newResult.snaps, 0, "clamping the over-cap lead eliminates the overshoot/snap drift");
  assert.ok(
    newResult.maxRenderLead <= CAP,
    "the clamped render never leads the server by more than the cap",
  );
}

console.log("movement controller tests passed");
