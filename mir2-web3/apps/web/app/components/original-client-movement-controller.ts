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
