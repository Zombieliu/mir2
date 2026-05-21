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

const MIR2_MOBILE_DEAD_ZONE = 0.24;
const MIR2_MOBILE_RUN_FORCE = 0.78;

export function mir2MobileDirectionFromVector(input: Mir2MobileAnalogInput): Mir2MobileDirection | null {
  const x = clampAxis(input.x);
  const y = clampAxis(input.y);
  const magnitude = Math.hypot(x, y);
  if (magnitude < MIR2_MOBILE_DEAD_ZONE) return null;

  const angle = Math.atan2(y, x);
  const octant = positiveModulo(Math.round(angle / (Math.PI / 4)), 8);

  switch (octant) {
    case 0:
      return "Right";
    case 1:
      return "DownRight";
    case 2:
      return "Down";
    case 3:
      return "DownLeft";
    case 4:
      return "Left";
    case 5:
      return "UpLeft";
    case 6:
      return "Up";
    case 7:
      return "UpRight";
    default:
      return null;
  }
}

export function mir2MobileMoveModeFromVector(input: Mir2MobileAnalogInput, runLocked: boolean): Mir2MobileMoveMode {
  if (runLocked) return "run";
  return Math.max(Math.abs(input.x), Math.abs(input.y), input.force ?? 0) >= MIR2_MOBILE_RUN_FORCE ? "run" : "walk";
}

export function mir2MobileMoveIntentFromVector(
  input: Mir2MobileAnalogInput,
  runLocked: boolean,
): Mir2MobileMoveIntent | null {
  const direction = mir2MobileDirectionFromVector(input);
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
