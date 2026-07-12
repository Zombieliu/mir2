export const BEVY_MOVEMENT_SHADOW_COUNTER_MAX = Number.MAX_SAFE_INTEGER;

export type BevyMovementShadowMode = "walk" | "run" | "turn";

export type BevyMovementShadowTsDisposition =
  | "none"
  | "confirmed"
  | "accepted"
  | "staleEcho"
  | "correction";

type BevyMovementShadowEventBase<T extends string> = Readonly<{
  type: T;
  atMs: number;
}>;

export type BevyMovementShadowResetEvent = BevyMovementShadowEventBase<"reset"> &
  Readonly<{
    objectId: string;
    x: number;
    y: number;
    direction: string;
  }>;

export type BevyMovementShadowClearEvent = BevyMovementShadowEventBase<"clear">;

export type BevyMovementShadowIntentEvent = BevyMovementShadowEventBase<"intent"> &
  Readonly<{
    direction: string;
    mode: BevyMovementShadowMode;
    fromX: number;
    fromY: number;
    toX: number;
    toY: number;
    phaseCount?: number;
  }>;

export type BevyMovementShadowCommandSentEvent = BevyMovementShadowEventBase<"commandSent"> &
  Readonly<{
    direction: string;
    mode: BevyMovementShadowMode;
    fromX: number;
    fromY: number;
    toX: number;
    toY: number;
    phaseCount?: number;
  }>;

export type BevyMovementShadowAuthoritativeEvent = BevyMovementShadowEventBase<"authoritative"> &
  Readonly<{
    packet: string;
    objectId: string;
    isSelf: boolean;
    x: number;
    y: number;
    direction: string;
    tsPredictedX?: number;
    tsPredictedY?: number;
    tsDisposition?: BevyMovementShadowTsDisposition;
  }>;

export type BevyMovementShadowRemoteMotionEvent = BevyMovementShadowEventBase<"remoteMotion"> &
  Readonly<{
    packet: string;
    objectId: string;
    fromX: number;
    fromY: number;
    toX: number;
    toY: number;
    direction: string;
    mode: BevyMovementShadowMode;
    phaseCount?: number;
  }>;

export type BevyMovementShadowRemoteRemoveEvent =
  BevyMovementShadowEventBase<"remoteRemove"> &
    Readonly<{
      objectId: string;
    }>;

export type BevyMovementShadowEvent =
  | BevyMovementShadowClearEvent
  | BevyMovementShadowResetEvent
  | BevyMovementShadowIntentEvent
  | BevyMovementShadowCommandSentEvent
  | BevyMovementShadowAuthoritativeEvent
  | BevyMovementShadowRemoteMotionEvent
  | BevyMovementShadowRemoteRemoveEvent;

export type BevyMovementShadowEventType = BevyMovementShadowEvent["type"];

export type BevyMovementShadowRuntime = Readonly<{
  pushMir2MovementShadowEvent?: (json: string) => void;
  getMir2MovementShadowDiagnostics?: () => unknown;
  getMir2RemoteMotionPresentationDiagnostics?: () => unknown;
  getMir2LocalMotionDiagnostics?: () => unknown;
}>;

export type BevyMovementShadowRuntimeSource =
  | BevyMovementShadowRuntime
  | null
  | undefined
  | (() => BevyMovementShadowRuntime | null | undefined);

export type BevyMovementShadowBridgeDiagnostics = Readonly<{
  submitted: number;
  dropped: number;
  errors: number;
  lastEventType: BevyMovementShadowEventType | null;
}>;

export type BevyMovementShadowBridgeOptions = Readonly<{
  maxCounterValue?: number;
}>;

export type BevyMovementShadowBridge = Readonly<{
  push: (event: BevyMovementShadowEvent) => void;
  getDiagnostics: () => BevyMovementShadowBridgeDiagnostics;
  getRuntimeDiagnostics: () => unknown | null;
  getPresentationDiagnostics: () => unknown | null;
  getLocalPresentationDiagnostics: () => unknown | null;
}>;

function stringifyMovementShadowPayload(payload: object): string {
  const json = JSON.stringify(payload);
  if (json === undefined) {
    throw new TypeError("Movement shadow event could not be serialized");
  }
  return json;
}

function unsupportedMovementShadowEvent(event: never): never {
  throw new TypeError(`Unsupported movement shadow event: ${String(event)}`);
}

/** Serialize only the documented camelCase schema, excluding accidental caller fields. */
export function serializeBevyMovementShadowEvent(event: BevyMovementShadowEvent): string {
  switch (event.type) {
    case "clear":
      return stringifyMovementShadowPayload({
        type: event.type,
        atMs: event.atMs,
      });
    case "reset":
      return stringifyMovementShadowPayload({
        type: event.type,
        atMs: event.atMs,
        objectId: event.objectId,
        x: event.x,
        y: event.y,
        direction: event.direction,
      });
    case "intent":
    case "commandSent":
      return stringifyMovementShadowPayload({
        type: event.type,
        atMs: event.atMs,
        direction: event.direction,
        mode: event.mode,
        fromX: event.fromX,
        fromY: event.fromY,
        toX: event.toX,
        toY: event.toY,
        phaseCount: event.phaseCount,
      });
    case "authoritative":
      return stringifyMovementShadowPayload({
        type: event.type,
        atMs: event.atMs,
        packet: event.packet,
        objectId: event.objectId,
        isSelf: event.isSelf,
        x: event.x,
        y: event.y,
        direction: event.direction,
        tsPredictedX: event.tsPredictedX,
        tsPredictedY: event.tsPredictedY,
        tsDisposition: event.tsDisposition,
      });
    case "remoteMotion":
      return stringifyMovementShadowPayload({
        type: event.type,
        atMs: event.atMs,
        packet: event.packet,
        objectId: event.objectId,
        fromX: event.fromX,
        fromY: event.fromY,
        toX: event.toX,
        toY: event.toY,
        direction: event.direction,
        mode: event.mode,
        phaseCount: event.phaseCount,
      });
    case "remoteRemove":
      return stringifyMovementShadowPayload({
        type: event.type,
        atMs: event.atMs,
        objectId: event.objectId,
      });
    default:
      return unsupportedMovementShadowEvent(event);
  }
}

function normalizedCounterMaximum(value: number | undefined): number {
  if (value === undefined || !Number.isFinite(value) || value < 1) {
    return BEVY_MOVEMENT_SHADOW_COUNTER_MAX;
  }
  return Math.min(BEVY_MOVEMENT_SHADOW_COUNTER_MAX, Math.trunc(value));
}

function incrementBounded(value: number, maximum: number): number {
  return value >= maximum ? maximum : value + 1;
}

/**
 * One-way diagnostic/presentation bridge. Runtime availability and failures are
 * deliberately unable to affect authoritative movement because pushes never
 * throw and return no result.
 */
export function createBevyMovementShadowBridge(
  runtimeSource: BevyMovementShadowRuntimeSource = null,
  options: BevyMovementShadowBridgeOptions = {},
): BevyMovementShadowBridge {
  const resolveRuntime =
    typeof runtimeSource === "function" ? runtimeSource : () => runtimeSource;
  const counterMaximum = normalizedCounterMaximum(options.maxCounterValue);

  let submitted = 0;
  let dropped = 0;
  let errors = 0;
  let lastEventType: BevyMovementShadowEventType | null = null;

  const push = (event: BevyMovementShadowEvent): void => {
    try {
      lastEventType = event.type;
      const runtime = resolveRuntime();
      const runtimePush = runtime?.pushMir2MovementShadowEvent;
      if (typeof runtimePush !== "function") {
        dropped = incrementBounded(dropped, counterMaximum);
        return;
      }

      const json = serializeBevyMovementShadowEvent(event);
      runtimePush.call(runtime, json);
      submitted = incrementBounded(submitted, counterMaximum);
    } catch {
      dropped = incrementBounded(dropped, counterMaximum);
      errors = incrementBounded(errors, counterMaximum);
    }
  };

  const getDiagnostics = (): BevyMovementShadowBridgeDiagnostics =>
    Object.freeze({
      submitted,
      dropped,
      errors,
      lastEventType,
    });

  const getRuntimeDiagnostics = (): unknown | null => {
    try {
      const runtime = resolveRuntime();
      const runtimeGetDiagnostics = runtime?.getMir2MovementShadowDiagnostics;
      if (typeof runtimeGetDiagnostics !== "function") {
        return null;
      }
      const diagnostics = runtimeGetDiagnostics.call(runtime);
      if (typeof diagnostics === "string") {
        return JSON.parse(diagnostics) as unknown;
      }
      return diagnostics ?? null;
    } catch {
      errors = incrementBounded(errors, counterMaximum);
      return null;
    }
  };

  const getPresentationDiagnostics = (): unknown | null => {
    try {
      const runtime = resolveRuntime();
      const runtimeGetDiagnostics = runtime?.getMir2RemoteMotionPresentationDiagnostics;
      if (typeof runtimeGetDiagnostics !== "function") {
        return null;
      }
      const diagnostics = runtimeGetDiagnostics.call(runtime);
      if (typeof diagnostics === "string") {
        return JSON.parse(diagnostics) as unknown;
      }
      return diagnostics ?? null;
    } catch {
      errors = incrementBounded(errors, counterMaximum);
      return null;
    }
  };

  const getLocalPresentationDiagnostics = (): unknown | null => {
    try {
      const runtime = resolveRuntime();
      const runtimeGetDiagnostics = runtime?.getMir2LocalMotionDiagnostics;
      if (typeof runtimeGetDiagnostics !== "function") {
        return null;
      }
      const diagnostics = runtimeGetDiagnostics.call(runtime);
      if (typeof diagnostics === "string") {
        return JSON.parse(diagnostics) as unknown;
      }
      return diagnostics ?? null;
    } catch {
      errors = incrementBounded(errors, counterMaximum);
      return null;
    }
  };

  return Object.freeze({
    push,
    getDiagnostics,
    getRuntimeDiagnostics,
    getPresentationDiagnostics,
    getLocalPresentationDiagnostics,
  });
}
