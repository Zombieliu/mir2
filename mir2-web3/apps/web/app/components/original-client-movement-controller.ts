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
};

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
export const CRYSTAL_RUN_PRIME_MS = 1200;
export const CRYSTAL_CORRECTION_BLOCK_MS = 400;
export const MOVEMENT_PENDING_MAX_AGE_MS = 1500;

export function effectiveCrystalMovementMode(
  requestedMode: CrystalMovementMode,
  now: number,
  runPrimedUntil: number,
): CrystalMovementMode {
  return requestedMode === "run" && now <= runPrimedUntil ? "run" : "walk";
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
}): PendingSelfMove {
  const mode = effectiveCrystalMovementMode(input.requestedMode, input.now, input.runPrimedUntil);
  const to = movementPointInDirection(input.from, input.direction, mode === "run" ? 2 : 1);
  return {
    from: input.from,
    to,
    direction: input.direction,
    mode,
    sentAt: input.now,
    visualUntil: input.now + CRYSTAL_MOVE_DELAY_MS,
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

export function reconcileMovementAck(input: {
  state: MovementControllerState;
  ack: MovementPoint;
  packetName: string;
  now: number;
}): { state: MovementControllerState; outcome: MovementAckOutcome } {
  const pending = input.state.pending;
  if (!pending) {
    return {
      outcome: "accepted",
      state: {
        ...input.state,
        prediction: null,
      },
    };
  }

  const hardFailure = input.packetName === "UserDashFail";
  if (!hardFailure && movementTileMatches(input.ack, pending.to)) {
    return {
      outcome: "confirmed",
      state: {
        ...input.state,
        pending: null,
        prediction: null,
        nextMoveSendAt: Math.max(input.state.nextMoveSendAt, pending.sentAt + CRYSTAL_MOVE_DELAY_MS),
        runPrimedUntil: input.now + CRYSTAL_RUN_PRIME_MS,
      },
    };
  }

  return {
    outcome: "correction",
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
