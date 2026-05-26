export type CrystalMiniMapProjection = "linear" | "isometric";

export type CrystalMiniMapTransform = {
  mapFileName: string;
  miniMapIndex: number;
  bigMapIndex?: number;
  worldMinX: number;
  worldMinY: number;
  worldMaxX: number;
  worldMaxY: number;
  imageMinX: number;
  imageMinY: number;
  imageMaxX: number;
  imageMaxY: number;
  projection?: CrystalMiniMapProjection;
};

export type CrystalMiniMapPoint = {
  x: number;
  y: number;
};

export function normalizeCrystalMiniMapFileName(mapFileName: string | null | undefined) {
  const fileName = String(mapFileName ?? "")
    .trim()
    .replace(/\\/g, "/")
    .split("/")
    .filter(Boolean)
    .pop() ?? "";
  const lower = fileName.toLocaleLowerCase();
  return lower.endsWith(".map") ? lower.slice(0, -4) : lower;
}

export function createLinearMiniMapTransform({
  mapFileName,
  miniMapIndex,
  bigMapIndex,
  worldWidth,
  worldHeight,
  imageWidth,
  imageHeight,
}: {
  mapFileName: string | null | undefined;
  miniMapIndex: number | null | undefined;
  bigMapIndex?: number | null | undefined;
  worldWidth: number;
  worldHeight: number;
  imageWidth: number;
  imageHeight: number;
}): CrystalMiniMapTransform {
  return {
    mapFileName: normalizeCrystalMiniMapFileName(mapFileName),
    miniMapIndex: Number(miniMapIndex ?? 0),
    bigMapIndex: bigMapIndex ?? undefined,
    worldMinX: 0,
    worldMinY: 0,
    worldMaxX: Math.max(worldWidth, 1),
    worldMaxY: Math.max(worldHeight, 1),
    imageMinX: 0,
    imageMinY: 0,
    imageMaxX: Math.max(imageWidth, 1),
    imageMaxY: Math.max(imageHeight, 1),
    projection: "linear",
  };
}

export function findCrystalMiniMapTransform(
  transforms: readonly CrystalMiniMapTransform[],
  {
    mapFileName,
    miniMapIndex,
    bigMapIndex,
    kind,
  }: {
    mapFileName: string | null | undefined;
    miniMapIndex: number | null | undefined;
    bigMapIndex?: number | null | undefined;
    kind: "mini" | "big";
  },
) {
  const normalized = normalizeCrystalMiniMapFileName(mapFileName);
  return transforms.find((transform) => {
    if (normalizeCrystalMiniMapFileName(transform.mapFileName) !== normalized) return false;
    if (kind === "mini") {
      return typeof miniMapIndex === "number" && transform.miniMapIndex === miniMapIndex;
    }
    return typeof bigMapIndex === "number" && transform.bigMapIndex === bigMapIndex;
  }) ?? null;
}

export function worldToMiniMapImagePoint(
  transform: CrystalMiniMapTransform,
  point: CrystalMiniMapPoint,
): CrystalMiniMapPoint {
  const worldX = point.x - transform.worldMinX;
  const worldY = point.y - transform.worldMinY;
  const worldWidth = Math.max(transform.worldMaxX - transform.worldMinX, 1);
  const worldHeight = Math.max(transform.worldMaxY - transform.worldMinY, 1);
  const imageWidth = transform.imageMaxX - transform.imageMinX;
  const imageHeight = transform.imageMaxY - transform.imageMinY;

  if (transform.projection === "isometric") {
    const isoSpan = Math.max(worldWidth + worldHeight, 1);
    return {
      x: transform.imageMinX + (worldX - worldY + worldHeight) * (imageWidth / isoSpan),
      y: transform.imageMinY + (worldX + worldY) * (imageHeight / isoSpan),
    };
  }

  return {
    x: transform.imageMinX + worldX * (imageWidth / worldWidth),
    y: transform.imageMinY + worldY * (imageHeight / worldHeight),
  };
}

export function miniMapImagePointToViewportPoint(
  imagePoint: CrystalMiniMapPoint,
  viewport: { imageLeft: number; imageTop: number },
): CrystalMiniMapPoint {
  return {
    x: imagePoint.x - viewport.imageLeft,
    y: imagePoint.y - viewport.imageTop,
  };
}
