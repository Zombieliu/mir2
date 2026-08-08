export type CrystalSceneBlueprintRequest = {
  mapFileName: string | null;
  centerX: number | null;
  centerY: number | null;
  width: number | null;
  height: number | null;
};

export type NormalizedCrystalSceneBlueprintRequest = {
  mapFileName: string;
  centerX: number;
  centerY: number;
  width: number;
  height: number;
};

export const SCENE_BLUEPRINT_SCHEMA_VERSION = "2026-08-05-v5-canonical-request";
export const SCENE_BLUEPRINT_CHUNK_WIDTH = 16;
export const SCENE_BLUEPRINT_CHUNK_HEIGHT = 17;
export const SCENE_BLUEPRINT_WIDTH_BUCKET = 8;
export const SCENE_BLUEPRINT_HEIGHT_BUCKET = 8;

export function normalizeCrystalSceneBlueprintRequest(
  request: CrystalSceneBlueprintRequest,
): NormalizedCrystalSceneBlueprintRequest {
  const mapFileName = normalizeMapFileName(request.mapFileName);
  const centerX = canonicalNumber(request.centerX) ?? 0;
  const centerY = canonicalNumber(request.centerY) ?? 0;
  const chunkX = Math.floor(centerX / SCENE_BLUEPRINT_CHUNK_WIDTH);
  const chunkY = Math.floor(centerY / SCENE_BLUEPRINT_CHUNK_HEIGHT);
  return {
    mapFileName,
    centerX: chunkX * SCENE_BLUEPRINT_CHUNK_WIDTH + Math.floor(SCENE_BLUEPRINT_CHUNK_WIDTH / 2),
    centerY: chunkY * SCENE_BLUEPRINT_CHUNK_HEIGHT + Math.floor(SCENE_BLUEPRINT_CHUNK_HEIGHT / 2),
    width: bucketDimension(request.width, SCENE_BLUEPRINT_WIDTH_BUCKET),
    height: bucketDimension(request.height, SCENE_BLUEPRINT_HEIGHT_BUCKET),
  };
}

export function createCrystalSceneBlueprintRequestKey(
  request: CrystalSceneBlueprintRequest | NormalizedCrystalSceneBlueprintRequest,
  suffix?: string,
) {
  const normalized = normalizeCrystalSceneBlueprintRequest(request);
  const chunkX = Math.floor(normalized.centerX / SCENE_BLUEPRINT_CHUNK_WIDTH);
  const chunkY = Math.floor(normalized.centerY / SCENE_BLUEPRINT_CHUNK_HEIGHT);
  return [
    SCENE_BLUEPRINT_SCHEMA_VERSION,
    encodeURIComponent(normalized.mapFileName),
    `cx${chunkX}`,
    `cy${chunkY}`,
    `w${normalized.width}`,
    `h${normalized.height}`,
    ...(suffix ? [suffix] : []),
  ].join("-");
}

export function createCrystalSceneBlueprintRequestUrl(
  request: CrystalSceneBlueprintRequest | NormalizedCrystalSceneBlueprintRequest,
  pathname = "/api/scene/crystal",
) {
  const normalized = normalizeCrystalSceneBlueprintRequest(request);
  const params = new URLSearchParams({
    v: SCENE_BLUEPRINT_SCHEMA_VERSION,
    map: normalized.mapFileName,
    x: String(normalized.centerX),
    y: String(normalized.centerY),
    width: String(normalized.width),
    height: String(normalized.height),
  });
  return `${pathname}?${params.toString()}`;
}

function normalizeMapFileName(mapFileName: string | null) {
  return (mapFileName ?? "0").trim().replaceAll("\\", "/").split("/").pop()?.replace(/\.map$/i, "") || "0";
}

function bucketDimension(value: number | null, bucketSize: number) {
  const canonical = canonicalNumber(value) ?? bucketSize;
  return Math.max(bucketSize, Math.ceil(canonical / bucketSize) * bucketSize);
}

function canonicalNumber(value: number | null) {
  return Number.isFinite(value) ? value : null;
}
