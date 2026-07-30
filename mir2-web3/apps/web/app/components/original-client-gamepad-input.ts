export type Mir2GamepadButtonLike = {
  pressed?: boolean;
  value?: number;
};

export type Mir2GamepadLike = {
  axes: readonly number[];
  buttons: readonly Mir2GamepadButtonLike[];
};

export type Mir2GamepadVector = {
  x: number;
  y: number;
  force: number;
};

export type Mir2GamepadSpatialDirection = "up" | "down" | "left" | "right";

export const MIR2_GAMEPAD_BUTTON = {
  primary: 0,
  cancel: 1,
  pick: 2,
  approach: 3,
  leftBumper: 4,
  rightBumper: 5,
  leftTrigger: 6,
  rightTrigger: 7,
  view: 8,
  menu: 9,
  dpadUp: 12,
  dpadDown: 13,
  dpadLeft: 14,
  dpadRight: 15,
} as const;

const GAMEPAD_AXIS_DEAD_ZONE = 0.18;

export function mir2GamepadButtonDown(gamepad: Mir2GamepadLike, index: number) {
  const button = gamepad.buttons[index];
  return Boolean(button?.pressed) || (button?.value ?? 0) >= 0.5;
}

export function mir2GamepadButtonPressed(
  gamepad: Mir2GamepadLike,
  previousButtons: readonly boolean[],
  index: number,
) {
  return mir2GamepadButtonDown(gamepad, index) && !previousButtons[index];
}

export function mir2GamepadButtons(gamepad: Mir2GamepadLike) {
  return gamepad.buttons.map((_, index) => mir2GamepadButtonDown(gamepad, index));
}

export function mir2GamepadVector(gamepad: Mir2GamepadLike): Mir2GamepadVector {
  let x = finiteAxis(gamepad.axes[0]);
  let y = finiteAxis(gamepad.axes[1]);

  if (mir2GamepadButtonDown(gamepad, MIR2_GAMEPAD_BUTTON.dpadLeft)) x = -1;
  if (mir2GamepadButtonDown(gamepad, MIR2_GAMEPAD_BUTTON.dpadRight)) x = 1;
  if (mir2GamepadButtonDown(gamepad, MIR2_GAMEPAD_BUTTON.dpadUp)) y = -1;
  if (mir2GamepadButtonDown(gamepad, MIR2_GAMEPAD_BUTTON.dpadDown)) y = 1;

  const rawForce = Math.min(1, Math.hypot(x, y));
  if (rawForce < GAMEPAD_AXIS_DEAD_ZONE) return { x: 0, y: 0, force: 0 };

  // Rescale the remaining range after the dead zone so a slightly worn stick
  // still reaches the full walk/run range without a discontinuous first step.
  const force = Math.min(1, (rawForce - GAMEPAD_AXIS_DEAD_ZONE) / (1 - GAMEPAD_AXIS_DEAD_ZONE));
  const normalization = rawForce > 1 ? 1 / rawForce : 1;
  return {
    x: x * normalization,
    y: y * normalization,
    force,
  };
}

export function mir2GamepadSpatialDirection(
  vector: Mir2GamepadVector,
): Mir2GamepadSpatialDirection | null {
  if (vector.force < 0.55) return null;
  if (Math.abs(vector.x) > Math.abs(vector.y)) return vector.x < 0 ? "left" : "right";
  return vector.y < 0 ? "up" : "down";
}

function finiteAxis(value: number | undefined) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(-1, Math.min(1, value ?? 0));
}
