export type Mir2GamepadButtonLike = {
  pressed?: boolean;
  value?: number;
};

export type Mir2GamepadLike = {
  id?: string;
  mapping?: string;
  connected?: boolean;
  axes: readonly number[];
  buttons: readonly Mir2GamepadButtonLike[];
};

export type Mir2GamepadFamily = "xbox" | "playstation" | "generic";
export type Mir2GamepadMappingMode =
  | "standard"
  | "known-fallback"
  | "platform"
  | "unverified";

export type Mir2GamepadProfile = {
  family: Mir2GamepadFamily;
  displayName: string;
  connected: boolean;
  mapping: string;
  mappingMode: Mir2GamepadMappingMode;
  supported: boolean;
};

export type Mir2GamepadLabels = {
  primary: string;
  cancel: string;
  pick: string;
  approach: string;
  leftBumper: string;
  rightBumper: string;
  leftTrigger: string;
  rightTrigger: string;
  view: string;
  menu: string;
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
const XBOX_GAMEPAD_ID = /\b(?:xbox|xinput)\b|045e/i;
const PLAYSTATION_GAMEPAD_ID =
  /\b(?:dualsense|dualshock|playstation|sony interactive)\b|^Wireless Controller(?:\s|$)|054c/i;
const XBOX_USER_AGENT = /\bXbox\b/i;
const PLAYSTATION_USER_AGENT = /\bPlayStation\b|\bPS5\b/i;

const GAMEPAD_LABELS: Record<Mir2GamepadFamily, Mir2GamepadLabels> = {
  xbox: {
    primary: "A",
    cancel: "B",
    pick: "X",
    approach: "Y",
    leftBumper: "LB",
    rightBumper: "RB",
    leftTrigger: "LT",
    rightTrigger: "RT",
    view: "View",
    menu: "Menu",
  },
  playstation: {
    primary: "×",
    cancel: "○",
    pick: "□",
    approach: "△",
    leftBumper: "L1",
    rightBumper: "R1",
    leftTrigger: "L2",
    rightTrigger: "R2",
    view: "Create",
    menu: "Options",
  },
  generic: {
    primary: "1",
    cancel: "2",
    pick: "3",
    approach: "4",
    leftBumper: "L1",
    rightBumper: "R1",
    leftTrigger: "L2",
    rightTrigger: "R2",
    view: "Select",
    menu: "Start",
  },
};

export function resolveMir2GamepadProfile(
  gamepad: Pick<Mir2GamepadLike, "id" | "mapping" | "connected"> | null | undefined,
  userAgent = "",
  forcedFamily?: Mir2GamepadFamily | null,
): Mir2GamepadProfile {
  const id = gamepad?.id?.trim() ?? "";
  const mapping = gamepad?.mapping?.trim() ?? "";
  const connected = Boolean(gamepad) && gamepad?.connected !== false;
  const family =
    forcedFamily ??
    (PLAYSTATION_GAMEPAD_ID.test(id) || PLAYSTATION_USER_AGENT.test(userAgent)
      ? "playstation"
      : XBOX_GAMEPAD_ID.test(id) || XBOX_USER_AGENT.test(userAgent)
        ? "xbox"
        : "generic");
  const mappingMode: Mir2GamepadMappingMode =
    mapping === "standard"
      ? "standard"
      : connected && family !== "generic"
        ? "known-fallback"
        : !connected && family !== "generic"
          ? "platform"
          : "unverified";

  return {
    family,
    displayName:
      family === "playstation"
        ? "PlayStation controller"
        : family === "xbox"
          ? "Xbox controller"
          : id || "Game controller",
    connected,
    mapping,
    mappingMode,
    supported: mappingMode !== "unverified",
  };
}

export function mir2GamepadLabels(family: Mir2GamepadFamily): Mir2GamepadLabels {
  return GAMEPAD_LABELS[family];
}

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
