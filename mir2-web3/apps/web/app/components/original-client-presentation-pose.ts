import type { ViewportOffset } from "./original-client-scene-layout";

export const BEVY_PRESENTATION_POSE_MAX_AGE_MS = 250;
export const BEVY_PRESENTATION_POSE_MAX_ENTITIES = 256;
export const BEVY_PRESENTATION_POSE_DEFAULT_PHASE_COUNT = 6;
export const BEVY_PRESENTATION_POSE_MAX_PHASE_COUNT = 8;

export type BevyPresentationPoseSource =
  | "localCommand"
  | "remotePacket"
  | "snapshotWindow"
  | "static";
export type BevyPresentationCameraSource = "localCommand" | "selfWindow" | "static";

export type BevyPresentationEntityMotion = {
  frameIndex: number;
  phaseCount: number;
  mode: "walk" | "run";
  direction: string;
};

export type BevyPresentationEntityPose = ViewportOffset & {
  source: BevyPresentationPoseSource;
  motion: BevyPresentationEntityMotion | null;
};

export type BevyPresentationPosePoint = {
  x: number;
  y: number;
};

export type BevyPresentationPoseProvenance = {
  mapRevision: number | null;
  mapCenter: BevyPresentationPosePoint | null;
  entityCenter: BevyPresentationPosePoint | null;
};

export type BevyPresentationPoseExpectedProvenance = BevyPresentationPoseProvenance;

export type BevyPresentationPoseProvenanceComparison =
  | "match"
  | "unavailable"
  | "internalCenterMismatch"
  | "mapRevisionMismatch"
  | "mapCenterMismatch"
  | "entityCenterMismatch";

export type BevyPresentationPoseFrame = {
  frameId: number;
  generatedAtMs: number;
  ageMs: number;
  camera: ViewportOffset & { source: BevyPresentationCameraSource };
  entities: ReadonlyMap<string, BevyPresentationEntityPose>;
  frameOverflowCount: number;
  totalOverflowCount: number;
  provenance: BevyPresentationPoseProvenance;
};

export type BevyPresentationPoseRuntime = {
  getMir2PresentationPoses?: () => string;
  setMir2PresentationPoseSink?: (sink: (json: string) => void) => void;
  clearMir2PresentationPoseSink?: () => void;
};

type UnknownRecord = Record<string, unknown>;

export function readBevyPresentationPoseFrame(
  runtime: BevyPresentationPoseRuntime | null | undefined,
  nowMs = Date.now(),
  maxAgeMs = BEVY_PRESENTATION_POSE_MAX_AGE_MS,
): BevyPresentationPoseFrame | null {
  const read = runtime?.getMir2PresentationPoses;
  if (typeof read !== "function") return null;

  try {
    return parseBevyPresentationPoseFrame(read.call(runtime), nowMs, maxAgeMs);
  } catch {
    return null;
  }
}

export function parseBevyPresentationPoseFrame(
  json: string,
  nowMs = Date.now(),
  maxAgeMs = BEVY_PRESENTATION_POSE_MAX_AGE_MS,
): BevyPresentationPoseFrame | null {
  if (typeof json !== "string" || !Number.isFinite(nowMs) || !Number.isFinite(maxAgeMs) || maxAgeMs < 0) {
    return null;
  }

  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch {
    return null;
  }
  if (
    !isRecord(value) ||
    value.ready !== true ||
    value.version !== 1 ||
    value.bridgeEnabled !== true ||
    value.rendererEnabled !== true
  ) {
    return null;
  }

  const frameId = finiteNonNegativeInteger(value.frameId);
  const generatedAtMs = finiteNumber(value.generatedAtMs);
  const frameOverflowCount = finiteNonNegativeInteger(value.frameOverflowCount);
  const totalOverflowCount = finiteNonNegativeInteger(value.totalOverflowCount);
  const camera = parseCamera(value.camera);
  const provenance = parseProvenance(value);
  if (
    frameId === null ||
    generatedAtMs === null ||
    frameOverflowCount === null ||
    totalOverflowCount === null ||
    !camera ||
    !provenance
  ) {
    return null;
  }

  const ageMs = nowMs - generatedAtMs;
  // A small future tolerance covers adjacent Date.now() samples without accepting
  // a frame from an unrelated clock domain.
  if (ageMs > maxAgeMs || ageMs < -1_000) return null;

  if (!Array.isArray(value.entities) || value.entities.length > BEVY_PRESENTATION_POSE_MAX_ENTITIES) {
    return null;
  }
  const entities = new Map<string, BevyPresentationEntityPose>();
  for (const entry of value.entities) {
    const parsed = parseEntity(entry);
    if (!parsed || entities.has(parsed.objectId)) return null;
    entities.set(parsed.objectId, parsed.pose);
  }

  return {
    frameId,
    generatedAtMs,
    ageMs,
    camera,
    entities,
    frameOverflowCount,
    totalOverflowCount,
    provenance,
  };
}

export function compareBevyPresentationPoseProvenance(
  frame: BevyPresentationPoseFrame,
  expected: BevyPresentationPoseExpectedProvenance | null | undefined,
): BevyPresentationPoseProvenanceComparison {
  const actual = frame.provenance;
  if (
    !expected ||
    expected.mapRevision === null ||
    !expected.mapCenter ||
    !expected.entityCenter ||
    actual.mapRevision === null ||
    !actual.mapCenter ||
    !actual.entityCenter
  ) {
    return "unavailable";
  }
  if (
    !samePoint(actual.mapCenter, actual.entityCenter) ||
    !samePoint(expected.mapCenter, expected.entityCenter)
  ) {
    return "internalCenterMismatch";
  }
  if (actual.mapRevision !== expected.mapRevision) return "mapRevisionMismatch";
  if (!samePoint(actual.mapCenter, expected.mapCenter)) return "mapCenterMismatch";
  if (!samePoint(actual.entityCenter, expected.entityCenter)) return "entityCenterMismatch";
  return "match";
}

function parseProvenance(value: UnknownRecord): BevyPresentationPoseProvenance | null {
  const nested = isRecord(value.provenance) ? value.provenance : null;
  const mapRevision = optionalFiniteNonNegativeInteger(
    nested ? nested.appliedMapRevision : value.mapRevision,
  );
  const mapCenter = nested
    ? optionalRecordPoint(nested.mapCenter)
    : optionalPoint(value.mapCenterX, value.mapCenterY);
  const entityCenter = nested
    ? optionalRecordPoint(nested.entityCenter)
    : optionalPoint(value.entityCenterX, value.entityCenterY);
  if (mapRevision === undefined || mapCenter === undefined || entityCenter === undefined) {
    return null;
  }
  return { mapRevision, mapCenter, entityCenter };
}

function parseCamera(value: unknown): BevyPresentationPoseFrame["camera"] | null {
  if (!isRecord(value)) return null;
  const x = finiteNumber(value.x);
  const y = finiteNumber(value.y);
  const source = value.source;
  if (
    x === null ||
    y === null ||
    (source !== "localCommand" && source !== "selfWindow" && source !== "static")
  ) {
    return null;
  }
  return { x, y, source };
}

function parseEntity(
  value: unknown,
): { objectId: string; pose: BevyPresentationEntityPose } | null {
  if (!isRecord(value) || typeof value.objectId !== "string" || value.objectId.length === 0) return null;
  const x = finiteNumber(value.x);
  const y = finiteNumber(value.y);
  const source = value.source;
  const motion = parseEntityMotion(value.motion);
  if (
    x === null ||
    y === null ||
    motion === undefined ||
    (source !== "localCommand" &&
      source !== "remotePacket" &&
      source !== "snapshotWindow" &&
      source !== "static")
  ) {
    return null;
  }
  return { objectId: value.objectId, pose: { x, y, source, motion } };
}

function parseEntityMotion(value: unknown): BevyPresentationEntityMotion | null | undefined {
  if (value === undefined || value === null) return null;
  if (!isRecord(value)) return undefined;
  const frameIndex = finiteNonNegativeInteger(value.frameIndex);
  const phaseCount = value.phaseCount === undefined
    ? BEVY_PRESENTATION_POSE_DEFAULT_PHASE_COUNT
    : finiteNonNegativeInteger(value.phaseCount);
  const mode = value.mode;
  const direction = value.direction;
  if (
    frameIndex === null ||
    phaseCount === null ||
    phaseCount < 1 ||
    phaseCount > BEVY_PRESENTATION_POSE_MAX_PHASE_COUNT ||
    frameIndex >= phaseCount ||
    (mode !== "walk" && mode !== "run") ||
    typeof direction !== "string" ||
    direction.length === 0
  ) {
    return undefined;
  }
  return { frameIndex, phaseCount, mode, direction };
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function finiteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function finiteNonNegativeInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function optionalFiniteNonNegativeInteger(value: unknown): number | null | undefined {
  if (value === undefined || value === null) return null;
  const parsed = finiteNonNegativeInteger(value);
  return parsed === null ? undefined : parsed;
}

function optionalPoint(xValue: unknown, yValue: unknown): BevyPresentationPosePoint | null | undefined {
  if ((xValue === undefined || xValue === null) && (yValue === undefined || yValue === null)) {
    return null;
  }
  const x = finiteNumber(xValue);
  const y = finiteNumber(yValue);
  return x === null || y === null ? undefined : { x, y };
}

function optionalRecordPoint(value: unknown): BevyPresentationPosePoint | null | undefined {
  if (value === undefined || value === null) return null;
  if (!isRecord(value)) return undefined;
  return optionalPoint(value.x, value.y);
}

function samePoint(left: BevyPresentationPosePoint, right: BevyPresentationPosePoint) {
  return left.x === right.x && left.y === right.y;
}
