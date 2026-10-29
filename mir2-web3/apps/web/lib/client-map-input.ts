import type { QuestCollisionGeometry } from "./quest-collision-cache";

export type MapImageRouteSource = Readonly<{
  owner: object;
  mapIndex: number;
  mapFileName: string;
  playerObjectId: string;
  mapWidth: number;
  mapHeight: number;
  miniMapIndex: number | null;
  bigMapIndex: number | null;
}>;

export type MapImageRouteIntent = Readonly<{
  source: MapImageRouteSource;
  kind: "big" | "mini";
  x: number;
  y: number;
  imageSrc: string;
  imageWidth: number;
  imageHeight: number;
}>;

export type MapRouteTile = Readonly<{ x: number; y: number }>;
export type MapRouteEdge = Readonly<{ from: MapRouteTile; to: MapRouteTile }>;
export type MiniMapCrop = Readonly<{ left: number; top: number; width: number; height: number }>;

const BIG_MAP_VIEW_WIDTH = 568;
const BIG_MAP_VIEW_HEIGHT = 380;
const MINI_MAP_VIEW_WIDTH = 120;
const MINI_MAP_VIEW_HEIGHT = 108;
const MAX_MAP_CELLS = 16_777_216;
const MAX_OCCUPIED_TILES = 65_536;
const MAX_REJECTED_EDGES = 262_144;
const MASK_CHUNK_CELLS = 65_536;
const BLOCKED_BITS_CHUNK_BYTES = 8_192;
const DIRECTIONS = [
  [0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1],
] as const;
const staticRouteMaskCache = new WeakMap<QuestCollisionGeometry, Uint8Array>();

function finitePositiveInteger(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0;
}

export function normalizeMapRouteFileName(value: string | null | undefined): string {
  const fileName = String(value ?? "").trim().replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? "";
  const lower = fileName.toLocaleLowerCase();
  return lower.endsWith(".map") ? lower.slice(0, -4) : lower;
}

export function mapRouteSourceMatchesWorld(
  source: MapImageRouteSource | null | undefined,
  world: Readonly<{
    mapFileName: string | null;
    miniMapIndex: number | null;
    bigMapIndex?: number | null;
    originalMapRegion: Readonly<{ mapWidth: number; mapHeight: number }> | null;
  }>,
  playerObjectId: string | null | undefined,
  kind: "big" | "mini",
): source is MapImageRouteSource {
  const region = world.originalMapRegion;
  return Boolean(source && typeof source.owner === "object" && source.owner !== null
    && Number.isSafeInteger(source.mapIndex) && source.mapIndex >= 0
    && source.playerObjectId && source.playerObjectId === playerObjectId
    && normalizeMapRouteFileName(source.mapFileName) !== ""
    && normalizeMapRouteFileName(source.mapFileName) === normalizeMapRouteFileName(world.mapFileName)
    && region && finitePositiveInteger(source.mapWidth) && finitePositiveInteger(source.mapHeight)
    && source.mapWidth === region.mapWidth && source.mapHeight === region.mapHeight
    && source.miniMapIndex === world.miniMapIndex
    && source.bigMapIndex === (world.bigMapIndex ?? null)
    && (kind === "mini" ? source.miniMapIndex !== null && source.miniMapIndex > 0
      : source.bigMapIndex !== null && source.bigMapIndex > 0));
}

export function sameMapRouteSource(
  left: MapImageRouteSource | null | undefined,
  right: MapImageRouteSource | null | undefined,
): boolean {
  return left === right || Boolean(left && right && left.owner === right.owner
    && left.mapIndex === right.mapIndex && left.mapFileName === right.mapFileName
    && left.playerObjectId === right.playerObjectId && left.mapWidth === right.mapWidth
    && left.mapHeight === right.mapHeight && left.miniMapIndex === right.miniMapIndex
    && left.bigMapIndex === right.bigMapIndex);
}

/** Native BigMapImageGeometry rect relative to the big-map dialog origin. */
export function nativeBigMapImageRect(imageWidth: number, imageHeight: number) {
  if (!finitePositiveInteger(imageWidth) || !finitePositiveInteger(imageHeight)) return null;
  const width = Math.min(imageWidth, BIG_MAP_VIEW_WIDTH);
  const height = Math.min(imageHeight, BIG_MAP_VIEW_HEIGHT);
  return {
    left: 14 + Math.floor((BIG_MAP_VIEW_WIDTH - width) / 2),
    top: 52 + Math.floor((BIG_MAP_VIEW_HEIGHT - height) / 2),
    width,
    height,
  } as const;
}

/** Inverse of Native's half-open image hit test and independent-axis floor projection. */
export function nativeBigMapImagePointToTile(
  imageX: number,
  imageY: number,
  mapWidth: number,
  mapHeight: number,
  imageWidth: number,
  imageHeight: number,
): MapRouteTile | null {
  const rect = nativeBigMapImageRect(imageWidth, imageHeight);
  if (!rect || !finitePositiveInteger(mapWidth) || !finitePositiveInteger(mapHeight)) return null;
  const x = Math.fround(imageX);
  const y = Math.fround(imageY);
  if (!Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0 || x >= rect.width || y >= rect.height) return null;
  return {
    x: Math.floor(Math.fround(Math.fround(x * Math.fround(mapWidth)) / Math.fround(rect.width))),
    y: Math.floor(Math.fround(Math.fround(y * Math.fround(mapHeight)) / Math.fround(rect.height))),
  };
}

/** Exact Native source crop from the current player coordinate and loaded raster dimensions. */
export function nativeMiniMapCrop(
  centerX: number,
  centerY: number,
  mapWidth: number,
  mapHeight: number,
  imageWidth: number,
  imageHeight: number,
): MiniMapCrop | null {
  if (![centerX, centerY].every(Number.isFinite)
    || !finitePositiveInteger(mapWidth) || !finitePositiveInteger(mapHeight)
    || !finitePositiveInteger(imageWidth) || !finitePositiveInteger(imageHeight)) return null;
  const imageWidthF = Math.fround(imageWidth);
  const imageHeightF = Math.fround(imageHeight);
  const mapWidthF = Math.fround(mapWidth);
  const mapHeightF = Math.fround(mapHeight);
  const width = Math.min(MINI_MAP_VIEW_WIDTH, imageWidthF);
  const height = Math.min(MINI_MAP_VIEW_HEIGHT, imageHeightF);
  const scaleX = Math.fround(imageWidthF / mapWidthF);
  const scaleY = Math.fround(imageHeightF / mapHeightF);
  const centerImageX = Math.fround(Math.fround(Math.fround(centerX) * scaleX));
  const centerImageY = Math.fround(Math.fround(Math.fround(centerY) * scaleY));
  return {
    left: Math.max(0, Math.min(Math.fround(centerImageX - MINI_MAP_VIEW_WIDTH * 0.5), Math.max(0, imageWidthF - MINI_MAP_VIEW_WIDTH))),
    top: Math.max(0, Math.min(Math.fround(centerImageY - MINI_MAP_VIEW_HEIGHT * 0.5), Math.max(0, imageHeightF - MINI_MAP_VIEW_HEIGHT))),
    width,
    height,
  };
}

/** Inverse of Native's prior-frame 120x108 viewport crop and f32 floor projection. */
export function nativeMiniMapViewportPointToTile(
  viewportX: number,
  viewportY: number,
  crop: MiniMapCrop,
  mapWidth: number,
  mapHeight: number,
  imageWidth: number,
  imageHeight: number,
): MapRouteTile | null {
  if (![viewportX, viewportY, crop.left, crop.top, crop.width, crop.height].every(Number.isFinite)
    || !finitePositiveInteger(mapWidth) || !finitePositiveInteger(mapHeight)
    || !finitePositiveInteger(imageWidth) || !finitePositiveInteger(imageHeight)
    || crop.width <= 0 || crop.height <= 0) return null;
  const x = Math.fround(viewportX);
  const y = Math.fround(viewportY);
  if (x < 0 || y < 0 || x >= MINI_MAP_VIEW_WIDTH || y >= MINI_MAP_VIEW_HEIGHT) return null;
  const sourceX = Math.fround(Math.fround(crop.left + Math.fround(Math.fround(x * Math.fround(crop.width)) / MINI_MAP_VIEW_WIDTH)));
  const sourceY = Math.fround(Math.fround(crop.top + Math.fround(Math.fround(y * Math.fround(crop.height)) / MINI_MAP_VIEW_HEIGHT)));
  const tileX = Math.floor(Math.fround(Math.fround(sourceX * Math.fround(mapWidth)) / Math.fround(imageWidth)));
  const tileY = Math.floor(Math.fround(Math.fround(sourceY * Math.fround(mapHeight)) / Math.fround(imageHeight)));
  return tileX >= 0 && tileY >= 0 && tileX < mapWidth && tileY < mapHeight
    ? { x: tileX, y: tileY } : null;
}

function isCurrentSafely(isCurrent: () => boolean): boolean {
  try { return isCurrent(); } catch { return false; }
}

async function yieldToBrowser(): Promise<void> {
  await new Promise<void>((resolve) => globalThis.setTimeout(resolve, 0));
}

function clearIncomingEdges(edges: Uint8Array, width: number, height: number, x: number, y: number) {
  for (let direction = 0; direction < DIRECTIONS.length; direction += 1) {
    const [dx, dy] = DIRECTIONS[direction];
    const fromX = x - dx;
    const fromY = y - dy;
    if (fromX >= 0 && fromX < width && fromY >= 0 && fromY < height) {
      const fromIndex = fromY * width + fromX;
      edges[fromIndex] &= ~(1 << direction);
    }
  }
}

function routeDirection(from: MapRouteTile, to: MapRouteTile): number | null {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const direction = DIRECTIONS.findIndex(([x, y]) => x === dx && y === dy);
  return direction >= 0 ? direction : null;
}

async function buildStaticRouteMask(
  geometry: QuestCollisionGeometry,
  width: number,
  height: number,
  cells: number,
  isCurrent: () => boolean,
): Promise<Uint8Array | null> {
  const edges = new Uint8Array(cells);
  for (let start = 0; start < cells; start += MASK_CHUNK_CELLS) {
    edges.fill(0xff, start, Math.min(start + MASK_CHUNK_CELLS, cells));
    if (!isCurrentSafely(isCurrent)) return null;
    if (start + MASK_CHUNK_CELLS < cells) {
      await yieldToBrowser();
      if (!isCurrentSafely(isCurrent)) return null;
    }
  }
  for (let x = 0; x < width; x += 1) {
    edges[x] &= ~((1 << 0) | (1 << 1) | (1 << 7));
    const bottom = (height - 1) * width + x;
    edges[bottom] &= ~((1 << 3) | (1 << 4) | (1 << 5));
  }
  for (let y = 0; y < height; y += 1) {
    const left = y * width;
    edges[left] &= ~((1 << 5) | (1 << 6) | (1 << 7));
    const right = left + width - 1;
    edges[right] &= ~((1 << 1) | (1 << 2) | (1 << 3));
  }

  const blockedBits = geometry.blockedBits;
  for (let byteStart = 0; byteStart < blockedBits.length; byteStart += BLOCKED_BITS_CHUNK_BYTES) {
    const byteEnd = Math.min(byteStart + BLOCKED_BITS_CHUNK_BYTES, blockedBits.length);
    for (let byteIndex = byteStart; byteIndex < byteEnd; byteIndex += 1) {
      let bits = blockedBits[byteIndex];
      while (bits !== 0) {
        const lowBit = 31 - Math.clz32(bits & -bits);
        const cell = byteIndex * 8 + lowBit;
        if (cell < cells) {
          const x = Math.floor(cell / height);
          const y = cell - x * height;
          clearIncomingEdges(edges, width, height, x, y);
        }
        bits &= bits - 1;
      }
    }
    if (!isCurrentSafely(isCurrent)) return null;
    if (byteEnd < blockedBits.length) {
      await yieldToBrowser();
      if (!isCurrentSafely(isCurrent)) return null;
    }
  }
  return isCurrentSafely(isCurrent) ? edges : null;
}

/** Build a complete row-major eight-direction can-step mask; null means stale or invalid. */
export async function buildMapRouteEdges(
  geometry: QuestCollisionGeometry,
  occupiedTiles: readonly MapRouteTile[],
  rejectedEdges: readonly MapRouteEdge[],
  isCurrent: () => boolean,
): Promise<Uint8Array | null> {
  if (!geometry || typeof geometry !== "object") return null;
  const width = geometry.width;
  const height = geometry.height;
  const cells = width * height;
  if (!isCurrentSafely(isCurrent) || !finitePositiveInteger(width) || !finitePositiveInteger(height)
    || width > 65_535 || height > 65_535 || !Number.isSafeInteger(cells) || cells > MAX_MAP_CELLS
    || !(geometry.blockedBits instanceof Uint8Array) || geometry.blockedBits.length !== Math.ceil(cells / 8)
    || !/^[a-f0-9]{64}$/i.test(geometry.fingerprint)
    || typeof geometry.mapFileName !== "string" || !geometry.mapFileName || geometry.mapFileName.length > 128
    || !Array.isArray(occupiedTiles) || !Array.isArray(rejectedEdges)
    || occupiedTiles.length > MAX_OCCUPIED_TILES || rejectedEdges.length > MAX_REJECTED_EDGES) return null;

  let staticMask = staticRouteMaskCache.get(geometry);
  if (!staticMask) {
    staticMask = await buildStaticRouteMask(geometry, width, height, cells, isCurrent) ?? undefined;
    if (!staticMask || !isCurrentSafely(isCurrent)) return null;
    staticRouteMaskCache.set(geometry, staticMask);
  }
  const edges = new Uint8Array(cells);
  for (let start = 0; start < cells; start += MASK_CHUNK_CELLS) {
    edges.set(staticMask.subarray(start, Math.min(start + MASK_CHUNK_CELLS, cells)), start);
    if (!isCurrentSafely(isCurrent)) return null;
    if (start + MASK_CHUNK_CELLS < cells) {
      await yieldToBrowser();
      if (!isCurrentSafely(isCurrent)) return null;
    }
  }

  for (let index = 0; index < occupiedTiles.length; index += 1) {
    const tile = occupiedTiles[index];
    if (!tile || typeof tile !== "object" || !Number.isSafeInteger(tile.x) || !Number.isSafeInteger(tile.y)
      || tile.x < 0 || tile.x >= width || tile.y < 0 || tile.y >= height) return null;
    clearIncomingEdges(edges, width, height, tile.x, tile.y);
    if (index > 0 && index % MASK_CHUNK_CELLS === 0) {
      if (!isCurrentSafely(isCurrent)) return null;
      await yieldToBrowser();
      if (!isCurrentSafely(isCurrent)) return null;
    }
  }
  for (let index = 0; index < rejectedEdges.length; index += 1) {
    const edge = rejectedEdges[index];
    if (!edge || typeof edge !== "object" || !edge.from || !edge.to
      || typeof edge.from !== "object" || typeof edge.to !== "object") return null;
    const direction = routeDirection(edge.from, edge.to);
    if (direction === null || !Number.isSafeInteger(edge.from.x) || !Number.isSafeInteger(edge.from.y)
      || !Number.isSafeInteger(edge.to.x) || !Number.isSafeInteger(edge.to.y)
      || edge.from.x < 0 || edge.from.x >= width || edge.from.y < 0 || edge.from.y >= height
      || edge.to.x < 0 || edge.to.x >= width || edge.to.y < 0 || edge.to.y >= height) return null;
    edges[edge.from.y * width + edge.from.x] &= ~(1 << direction);
    if (index > 0 && index % MASK_CHUNK_CELLS === 0) {
      if (!isCurrentSafely(isCurrent)) return null;
      await yieldToBrowser();
      if (!isCurrentSafely(isCurrent)) return null;
    }
  }
  return isCurrentSafely(isCurrent) ? edges : null;
}
