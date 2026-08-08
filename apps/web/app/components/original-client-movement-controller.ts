export type CrystalMovementMode = "walk" | "run";

export type MovementPoint = {
  x: number;
  y: number;
  direction?: string;
};

export type PendingSelfMove = {
  from: MovementPoint;
  to: MovementPoint;
  direction: string;
  mode: CrystalMovementMode;
  sentAt: number;
  visualUntil: number;
  phaseCount: number;
};

export type CrystalMovementProfile = Readonly<{
  mode: CrystalMovementMode;
  distance: 1 | 2 | 3;
  phaseCount: 6 | 8;
  frameIntervalMs: 100;
  durationMs: 600 | 800;
}>;

export type QueuedMoveIntent = {
  kind: "direction" | "target";
  direction?: string;
  targetX?: number;
  targetY?: number;
  requestedMode: CrystalMovementMode;
  requestedAt: number;
  consumeAfterSend?: boolean;
};

export type MovementControllerState = {
  pending: PendingSelfMove | null;
  prediction: MovementPoint | null;
  nextMoveSendAt: number;
  runPrimedUntil: number;
  inputBlockedUntil: number;
};

export type MovementAckOutcome = "confirmed" | "correction" | "accepted";

export const CRYSTAL_MOVE_DELAY_MS = 600;
export const CRYSTAL_MOVE_FRAME_INTERVAL_MS = 100;
export const CRYSTAL_RUN_PRIME_MS = 1200;
export const CRYSTAL_CORRECTION_BLOCK_MS = 400;
export const MOVEMENT_PENDING_MAX_AGE_MS = 3000;

export function effectiveCrystalMovementMode(
  requestedMode: CrystalMovementMode,
  now: number,
  runPrimedUntil: number,
): CrystalMovementMode {
  return requestedMode === "run" && now <= runPrimedUntil ? "run" : "walk";
}

export function crystalMovementProfile(input: {
  mode: CrystalMovementMode;
  mounted?: boolean;
  swiftFeet?: boolean;
  sneaking?: boolean;
}): CrystalMovementProfile {
  const mounted = Boolean(input.mounted);
  const threeTileRun = input.mode === "run" && (mounted || (input.swiftFeet && !input.sneaking));
  const distance = input.mode === "walk" ? 1 : threeTileRun ? 3 : 2;
  const phaseCount = mounted && input.mode === "walk" ? 8 : 6;
  return {
    mode: input.mode,
    distance,
    phaseCount,
    frameIntervalMs: CRYSTAL_MOVE_FRAME_INTERVAL_MS,
    durationMs: phaseCount === 8 ? 800 : 600,
  };
}

export function movementPointInDirection(
  source: MovementPoint,
  direction: string,
  distance: number,
): MovementPoint {
  switch (direction) {
    case "Up":
      return { x: source.x, y: source.y - distance, direction };
    case "UpRight":
      return { x: source.x + distance, y: source.y - distance, direction };
    case "Right":
      return { x: source.x + distance, y: source.y, direction };
    case "DownRight":
      return { x: source.x + distance, y: source.y + distance, direction };
    case "Down":
      return { x: source.x, y: source.y + distance, direction };
    case "DownLeft":
      return { x: source.x - distance, y: source.y + distance, direction };
    case "Left":
      return { x: source.x - distance, y: source.y, direction };
    case "UpLeft":
      return { x: source.x - distance, y: source.y - distance, direction };
    default:
      return { x: source.x, y: source.y, direction };
  }
}

export function createPendingSelfMove(input: {
  from: MovementPoint;
  direction: string;
  requestedMode: CrystalMovementMode;
  now: number;
  runPrimedUntil: number;
  mounted?: boolean;
  swiftFeet?: boolean;
  sneaking?: boolean;
}): PendingSelfMove {
  const mode = effectiveCrystalMovementMode(input.requestedMode, input.now, input.runPrimedUntil);
  const profile = crystalMovementProfile({
    mode,
    mounted: input.mounted,
    swiftFeet: input.swiftFeet,
    sneaking: input.sneaking,
  });
  const to = movementPointInDirection(input.from, input.direction, profile.distance);
  return {
    from: input.from,
    to,
    direction: input.direction,
    mode,
    sentAt: input.now,
    visualUntil: input.now + profile.durationMs,
    phaseCount: profile.phaseCount,
  };
}

// Clamp a render candidate so it leads `origin` by at most `maxLeadTiles` on each
// axis (Chebyshev distance), preserving the candidate's travel direction. The
// locally-predicted self sprite is allowed to render ahead of the authoritative
// server tile by a fixed lead cap; when a long/dropped frame or a run/walk
// resolution mismatch briefly pushes the prediction PAST that cap, the renderer
// would otherwise discard the prediction entirely and snap the sprite back to the
// server tile (the visible "overshoot then snap"). Clamping instead pins the
// rendered tile to the cap boundary along the same vector, so the sprite eases
// forward and the server catches up under it with no backward jump. The clamped
// tile stays strictly between `origin` and `candidate`, so a candidate that is
// genuinely ahead of the server remains ahead after clamping.
export function clampMovementLeadToCap(
  origin: { x: number; y: number },
  candidate: MovementPoint,
  maxLeadTiles: number,
): MovementPoint {
  const cap = Math.max(0, maxLeadTiles);
  const clampAxis = (delta: number) => Math.max(-cap, Math.min(cap, delta));
  return {
    ...candidate,
    x: origin.x + clampAxis(candidate.x - origin.x),
    y: origin.y + clampAxis(candidate.y - origin.y),
  };
}

// Step the rendered self tile AT MOST `maxStepTiles` toward `target` from `base`,
// along the travel vector (Chebyshev / 8-direction). When `target` is already
// within the per-step cap it is returned unchanged, so normal movement (a <=1-tile
// advance per frame) passes straight through and is fully responsive. When a
// long/dropped frame — or a direction reversal whose old and new predicted leads
// sit on opposite sides of the server tile — would otherwise move the rendered
// tile >1 tile in a single frame (several 1-tile transitions collapsed by the
// dropped frames into one visible jump = the "overshoot then snap"), the tile is
// instead eased one step toward the target so it routes through the in-between
// tiles over consecutive frames with no non-physical jump. Stateless and pure;
// the caller owns the per-frame baseline + commit gating.
export function stepMovementTowardWithinCap(
  base: { x: number; y: number },
  target: MovementPoint,
  maxStepTiles: number,
): MovementPoint {
  const cap = Math.max(0, maxStepTiles);
  const dx = target.x - base.x;
  const dy = target.y - base.y;
  if (Math.max(Math.abs(dx), Math.abs(dy)) <= cap) {
    return target;
  }
  const stepAxis = (delta: number) =>
    delta === 0 ? 0 : Math.sign(delta) * Math.min(cap, Math.abs(delta));
  return {
    ...target,
    x: base.x + stepAxis(dx),
    y: base.y + stepAxis(dy),
  };
}

export function movementTileMatches(left: MovementPoint, right: MovementPoint) {
  return left.x === right.x && left.y === right.y;
}

export function movementTransformMatches(left: MovementPoint, right: MovementPoint) {
  if (!movementTileMatches(left, right)) {
    return false;
  }
  if (left.direction && right.direction && left.direction !== right.direction) {
    return false;
  }
  return true;
}

export const movementPointMatches = movementTileMatches;

export function classifyMovementAckOutcome(input: {
  pending: PendingSelfMove | null;
  ack: MovementPoint;
  packetName: string;
}): MovementAckOutcome {
  const pending = input.pending;
  if (!pending) return "accepted";
  if (input.packetName === "UserDashFail") return "correction";
  if (movementTileMatches(input.ack, pending.to)) return "confirmed";
  if (pending.mode === "run") {
    const degraded = movementPointInDirection(pending.from, pending.direction, 1);
    if (movementTileMatches(input.ack, degraded)) return "confirmed";
  }
  return "correction";
}

export function reconcileMovementAck(input: {
  state: MovementControllerState;
  ack: MovementPoint;
  packetName: string;
  now: number;
}): { state: MovementControllerState; outcome: MovementAckOutcome } {
  const pending = input.state.pending;
  const outcome = classifyMovementAckOutcome({
    pending,
    ack: input.ack,
    packetName: input.packetName,
  });
  if (outcome === "accepted") {
    return {
      outcome,
      state: {
        ...input.state,
        prediction: null,
      },
    };
  }

  if (outcome === "confirmed" && pending) {
    return {
      outcome,
      state: {
        ...input.state,
        pending: null,
        prediction: null,
        nextMoveSendAt: Math.max(input.state.nextMoveSendAt, pending.visualUntil),
        runPrimedUntil: input.now + CRYSTAL_RUN_PRIME_MS,
      },
    };
  }

  // The shared classifier already returned confirmed for an on-path one-tile
  // run degradation. Anything reaching this branch is a real correction.
  return {
    outcome,
    state: {
      ...input.state,
      pending: null,
      prediction: null,
      runPrimedUntil: 0,
      inputBlockedUntil: input.now + CRYSTAL_CORRECTION_BLOCK_MS,
      nextMoveSendAt: Math.max(input.state.nextMoveSendAt, input.now + CRYSTAL_CORRECTION_BLOCK_MS),
    },
  };
}

export function reconcileMovementSnapshot(input: {
  state: MovementControllerState;
  snapshot: MovementPoint;
  now: number;
}): { state: MovementControllerState; corrected: boolean } {
  const pending = input.state.pending;
  const prediction = input.state.prediction;
  const snapshotDiffersFromPending = pending ? !movementTransformMatches(input.snapshot, pending.to) : false;
  const snapshotDiffersFromPrediction = prediction ? !movementTransformMatches(input.snapshot, prediction) : false;

  if (!snapshotDiffersFromPending && !snapshotDiffersFromPrediction) {
    return { corrected: false, state: input.state };
  }

  return {
    corrected: true,
    state: {
      ...input.state,
      pending: null,
      prediction: null,
      runPrimedUntil: 0,
      inputBlockedUntil: input.now + CRYSTAL_CORRECTION_BLOCK_MS,
    },
  };
}

export function canSendMovement(state: MovementControllerState, now: number) {
  return !state.pending && now >= state.nextMoveSendAt && now >= state.inputBlockedUntil;
}
