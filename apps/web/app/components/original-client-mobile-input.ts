export type Mir2MobileDirection =
  | "Up"
  | "UpRight"
  | "Right"
  | "DownRight"
  | "Down"
  | "DownLeft"
  | "Left"
  | "UpLeft";

export type Mir2MobileMoveMode = "walk" | "run";

export type Mir2MobileAnalogInput = {
  x: number;
  y: number;
  force?: number;
};

export type Mir2MobileMoveIntent = {
  direction: Mir2MobileDirection;
  mode: Mir2MobileMoveMode;
  force: number;
};

// Entry dead-zone: below this the stick reads as centered (no movement).
const MIR2_MOBILE_DEAD_ZONE = 0.24;
// Exit dead-zone: once moving, the stick must fall further toward center before
// we report a stop. The gap (hysteresis) stops a stick held near the rim of the
// dead-zone from rapidly toggling move/stop and spamming direction packets.
const MIR2_MOBILE_RELEASE_DEAD_ZONE = 0.18;
const MIR2_MOBILE_RUN_FORCE = 0.78;
// Angular hysteresis: when a direction is already active, an adjacent octant
// only wins once the angle is this far past the shared boundary. Prevents
// jittery flicker between, e.g., "Right" and "DownRight" on diagonal holds.
const MIR2_MOBILE_DIRECTION_HYSTERESIS_RAD = Math.PI / 18; // 10 degrees

const MIR2_MOBILE_OCTANT_DIRECTIONS: Mir2MobileDirection[] = [
  "Right",
  "DownRight",
  "Down",
  "DownLeft",
  "Left",
  "UpLeft",
  "Up",
  "UpRight",
];

const MIR2_MOBILE_OCTANT_INDEX: Record<Mir2MobileDirection, number> = {
  Right: 0,
  DownRight: 1,
  Down: 2,
  DownLeft: 3,
  Left: 4,
  UpLeft: 5,
  Up: 6,
  UpRight: 7,
};

export function mir2MobileDirectionFromVector(
  input: Mir2MobileAnalogInput,
  previousDirection: Mir2MobileDirection | null = null,
): Mir2MobileDirection | null {
  const x = clampAxis(input.x);
  const y = clampAxis(input.y);
  const magnitude = Math.hypot(x, y);
  // Use the lower release threshold while a direction is held so we don't drop
  // movement on tiny wobbles; otherwise require the full entry dead-zone.
  const threshold = previousDirection ? MIR2_MOBILE_RELEASE_DEAD_ZONE : MIR2_MOBILE_DEAD_ZONE;
  if (magnitude < threshold) return null;

  const angle = Math.atan2(y, x);
  const octant = positiveModulo(Math.round(angle / (Math.PI / 4)), 8);
  const nextDirection = MIR2_MOBILE_OCTANT_DIRECTIONS[octant];

  if (previousDirection && nextDirection !== previousDirection) {
    // Only switch away from the active direction once the stick has moved
    // clearly past the boundary, not while hovering exactly on it.
    const previousOctant = MIR2_MOBILE_OCTANT_INDEX[previousDirection];
    if (octantsAreAdjacent(previousOctant, octant)) {
      const distanceFromPrevCenter = angularDistance(angle, previousOctant * (Math.PI / 4));
      if (distanceFromPrevCenter < Math.PI / 4 + MIR2_MOBILE_DIRECTION_HYSTERESIS_RAD) {
        return previousDirection;
      }
    }
  }

  return nextDirection;
}

function octantsAreAdjacent(a: number, b: number) {
  const diff = positiveModulo(a - b, 8);
  return diff === 1 || diff === 7;
}

function angularDistance(a: number, b: number) {
  const diff = Math.abs(positiveModulo(a - b + Math.PI, Math.PI * 2) - Math.PI);
  return diff;
}

export function mir2MobileMoveModeFromVector(input: Mir2MobileAnalogInput, runLocked: boolean): Mir2MobileMoveMode {
  if (runLocked) return "run";
  return Math.max(Math.abs(input.x), Math.abs(input.y), input.force ?? 0) >= MIR2_MOBILE_RUN_FORCE ? "run" : "walk";
}

export function mir2MobileMoveIntentFromVector(
  input: Mir2MobileAnalogInput,
  runLocked: boolean,
  previousDirection: Mir2MobileDirection | null = null,
): Mir2MobileMoveIntent | null {
  const direction = mir2MobileDirectionFromVector(input, previousDirection);
  if (!direction) return null;
  return {
    direction,
    mode: mir2MobileMoveModeFromVector(input, runLocked),
    force: Math.max(Math.abs(input.x), Math.abs(input.y), input.force ?? 0),
  };
}

function clampAxis(value: number) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(-1, Math.min(1, value));
}

function positiveModulo(value: number, divisor: number) {
  return ((value % divisor) + divisor) % divisor;
}
