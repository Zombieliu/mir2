export type BevyRuntimeBootMode = "eager" | "compatibility" | "disabled";

export type BevyRuntimeBootDecision = {
  mode: BevyRuntimeBootMode;
  reason:
    | "desktop-default"
    | "forced"
    | "explicitly-disabled"
    | "touch-first-device";
};

export type BevyRuntimeBootEnvironment = {
  layout: "desktop" | "touch" | "tv";
  input: "keyboardMouse" | "touch" | "gamepad";
  coarsePointer: boolean;
  maxTouchPoints: number;
  userAgent: string;
  params: Pick<URLSearchParams, "get">;
};

/**
 * The DOM/WebGL2 Crystal renderer is the guaranteed compatibility path. The
 * Bevy/WASM renderer is an enhancement: desktop loads it eagerly, while phones
 * and touch-first tablets avoid spending tens of megabytes before gameplay.
 * Operators can force either path for QA with `bevyRuntime=1|0`.
 */
export function resolveBevyRuntimeBootDecision({
  layout,
  input,
  coarsePointer,
  maxTouchPoints,
  userAgent,
  params,
}: BevyRuntimeBootEnvironment): BevyRuntimeBootDecision {
  if (params.get("skipRuntime") === "1" || params.get("bevyRuntime") === "0") {
    return { mode: "disabled", reason: "explicitly-disabled" };
  }
  if (params.get("bevyRuntime") === "1" || params.get("skipRuntime") === "0") {
    return { mode: "eager", reason: "forced" };
  }

  const normalizedUserAgent = userAgent.toLowerCase();
  const mobileUserAgent = /android|iphone|ipod|mobile/.test(normalizedUserAgent);
  const ipadDesktopUserAgent =
    normalizedUserAgent.includes("macintosh") && maxTouchPoints > 1;
  const touchFirst =
    layout === "touch" ||
    input === "touch" ||
    coarsePointer ||
    mobileUserAgent ||
    ipadDesktopUserAgent;

  return touchFirst
    ? { mode: "compatibility", reason: "touch-first-device" }
    : { mode: "eager", reason: "desktop-default" };
}

const NETWORK_FAILURE_PATTERNS = [
  "load failed",
  "failed to fetch",
  "networkerror",
  "network error",
  "dynamically imported module",
  "importing a module script failed",
  "aborterror",
  "aborted",
  "timed out",
  "timeout",
  "connection reset",
  "connection closed",
  "webassembly streaming compilation failed",
];

export function isBevyRuntimeNetworkFailure(error: unknown) {
  const message = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
  const normalized = message.toLowerCase();
  return NETWORK_FAILURE_PATTERNS.some((pattern) => normalized.includes(pattern));
}

export function shouldRetryBevyRuntimeWithWebGl2(
  backend: "webgpu" | "webgl2",
  webGl2Supported: boolean,
  error: unknown,
) {
  return backend === "webgpu" && webGl2Supported && !isBevyRuntimeNetworkFailure(error);
}
