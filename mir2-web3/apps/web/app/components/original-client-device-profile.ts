import {
  resolveMir2GamepadProfile,
  type Mir2GamepadFamily,
  type Mir2GamepadProfile,
} from "./original-client-gamepad-input";

export type Mir2LayoutProfile = "desktop" | "touch" | "tv";
export type Mir2InputProfile = "keyboardMouse" | "touch" | "gamepad";

export type Mir2ClientProfile = {
  layout: Mir2LayoutProfile;
  input: Mir2InputProfile;
  gamepad: Mir2GamepadProfile;
  layoutForced: boolean;
  inputForced: boolean;
};

export type Mir2ClientEnvironment = {
  search?: string;
  userAgent?: string;
  coarsePointer?: boolean;
  touchPoints?: number;
  gamepadConnected?: boolean;
  gamepadId?: string;
  gamepadMapping?: string;
};

const LAYOUT_VALUES = new Set<Mir2LayoutProfile>(["desktop", "touch", "tv"]);
const INPUT_VALUES = new Set<Mir2InputProfile>(["keyboardMouse", "touch", "gamepad"]);
const GAMEPAD_FAMILY_VALUES = new Set<Mir2GamepadFamily>(["xbox", "playstation", "generic"]);

export function resolveMir2ClientProfile(environment: Mir2ClientEnvironment): Mir2ClientProfile {
  const params = new URLSearchParams(environment.search ?? "");
  const forcedLayout = readEnum(params.get("layout"), LAYOUT_VALUES);
  const forcedInput = readEnum(params.get("input"), INPUT_VALUES);
  const forcedGamepadFamily = readEnum(params.get("controller"), GAMEPAD_FAMILY_VALUES);
  const legacyMobile = params.get("mobileControls") === "1" || params.get("mobile") === "1";
  const xboxBrowser = /\bXbox\b/i.test(environment.userAgent ?? "");
  // A fine-pointer laptop may expose touch points without being a touch-first
  // play surface. Only the primary coarse-pointer signal selects touch layout.
  const touchPrimary = Boolean(environment.coarsePointer);
  const gamepad = resolveMir2GamepadProfile(
    environment.gamepadConnected
      ? {
          id: environment.gamepadId,
          mapping: environment.gamepadMapping,
          connected: true,
        }
      : null,
    environment.userAgent,
    forcedGamepadFamily,
  );

  const layout =
    forcedLayout ??
    (legacyMobile
      ? "touch"
      : xboxBrowser
        ? "tv"
        : touchPrimary
          ? "touch"
          : "desktop");

  const input =
    forcedInput ??
    (legacyMobile
      ? "touch"
      : xboxBrowser || environment.gamepadConnected
        ? "gamepad"
        : touchPrimary
          ? "touch"
          : "keyboardMouse");

  return {
    layout,
    input,
    gamepad,
    layoutForced: forcedLayout !== null || legacyMobile,
    inputForced: forcedInput !== null || legacyMobile,
  };
}

export function readMir2ClientEnvironment(): Mir2ClientEnvironment {
  if (typeof window === "undefined") return {};
  const gamepad =
    Array.from(navigator.getGamepads?.() ?? []).find(
      (candidate): candidate is Gamepad => Boolean(candidate?.connected),
    ) ?? null;
  return {
    search: window.location.search,
    userAgent: navigator.userAgent,
    coarsePointer: window.matchMedia("(pointer: coarse)").matches,
    touchPoints: navigator.maxTouchPoints,
    gamepadConnected: Boolean(gamepad),
    gamepadId: gamepad?.id,
    gamepadMapping: gamepad?.mapping,
  };
}

function readEnum<T extends string>(value: string | null, allowed: Set<T>): T | null {
  return value && allowed.has(value as T) ? (value as T) : null;
}
