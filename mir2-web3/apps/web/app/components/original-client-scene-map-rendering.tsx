"use client";

import type { SyntheticEvent } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import type { OriginalMapRegion, OriginalMapSpriteFrame } from "../../lib/scene-types";
import type { DisplayEntity, DisplayWorld } from "./original-client-types";
import {
  EMPTY_VIEWPORT_MAP_SPRITES,
  EMPTY_VIEWPORT_OFFSET,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  VIEWPORT_RANGE_X,
  VIEWPORT_RANGE_Y,
  VIEWPORT_TILE_LEFT_ORIGIN,
  VIEWPORT_TILE_TOP_ORIGIN,
  viewportDepthForCell,
  type SceneBackdropTile,
  type ViewportMapSprite,
  type ViewportMapSprites,
  type ViewportOffset,
} from "./original-client-scene-layout";

type OriginalMapCell = OriginalMapRegion["cells"][number];

const mapRegionCellIndexCache = new WeakMap<OriginalMapRegion, Map<string, OriginalMapCell>>();

export function GameSceneBackdrop({
  world,
  player,
  floorSprites,
  cameraOffset,
}: {
  world: DisplayWorld;
  player: DisplayEntity | null;
  floorSprites: ViewportMapSprite[];
  cameraOffset: ViewportOffset;
}) {
  const tiles = world.originalMapRegion ? [] : buildSceneBackdropTiles(world, player);

  if (!tiles.length && !floorSprites.length) {
    return null;
  }

  const renderOffset = floorSprites.length ? cameraOffset : EMPTY_VIEWPORT_OFFSET;

  return (
    <div className="game-scene-backdrop">
      {tiles.map((tile) => (
        <div
          key={tile.key}
          className="scene-backdrop-tile"
          data-map-sprite-key={tile.key}
          style={{
            left: tile.left + renderOffset.x,
            top: tile.top + renderOffset.y,
            backgroundImage: `linear-gradient(${tile.tint}, ${tile.tint}), url("${tile.texture}")`,
          }}
        />
      ))}
      {floorSprites.map((sprite) => (
        <img
          key={sprite.key}
          className="scene-backdrop-sprite"
          data-map-sprite-key={sprite.key}
          data-mir2-original-src={sprite.path}
          src={sprite.path}
          crossOrigin="anonymous"
          alt=""
          draggable={false}
          onError={handleSceneAssetImageError}
          onLoad={handleSceneAssetImageLoad}
          style={{
            left: sprite.left + cameraOffset.x,
            top: sprite.top + cameraOffset.y,
            width: sprite.width,
            height: sprite.height,
          }}
        />
      ))}
    </div>
  );
}

export function buildViewportMapSprites(
  world: DisplayWorld,
  player: DisplayEntity,
  animationFrameIndex: number,
): ViewportMapSprites {
  const region = world.originalMapRegion;
  if (!region) {
    return EMPTY_VIEWPORT_MAP_SPRITES;
  }

  const floorMinX = player.x - VIEWPORT_RANGE_X;
  const floorMaxX = player.x + VIEWPORT_RANGE_X;
  const floorMinY = player.y - VIEWPORT_RANGE_Y;
  const floorMaxY = player.y + VIEWPORT_RANGE_Y;
  const objectMinX = floorMinX - 4;
  const objectMaxX = floorMaxX + 4;
  const objectMinY = floorMinY - 4;
  const objectMaxY = floorMaxY + 25;
  const floor: ViewportMapSprite[] = [];
  const objects: ViewportMapSprite[] = [];

  for (const cell of viewportMapCells(region, objectMinX, objectMaxX, objectMinY, objectMaxY)) {
    const inFloorBounds =
      cell.x >= floorMinX && cell.x <= floorMaxX && cell.y >= floorMinY && cell.y <= floorMaxY;

    appendViewportMapSprite(
      floor,
      objects,
      region,
      cell.back,
      cell,
      player,
      animationFrameIndex,
      inFloorBounds,
      true,
    );
    appendViewportMapSprite(
      floor,
      objects,
      region,
      cell.middle,
      cell,
      player,
      animationFrameIndex,
      inFloorBounds,
      true,
    );
    appendViewportMapSprite(
      floor,
      objects,
      region,
      cell.front,
      cell,
      player,
      animationFrameIndex,
      inFloorBounds,
      true,
    );
    appendViewportMapSprite(
      floor,
      objects,
      region,
      cell.tileAnimation,
      cell,
      player,
      animationFrameIndex,
      false,
      true,
    );
  }

  return {
    floor,
    objects,
  };
}

function viewportMapCells(
  region: OriginalMapRegion,
  minX: number,
  maxX: number,
  minY: number,
  maxY: number,
) {
  const index = mapRegionCellIndex(region);
  const cells: OriginalMapCell[] = [];
  const clampedMinX = Math.max(region.regionBounds.minX, minX);
  const clampedMaxX = Math.min(region.regionBounds.maxX, maxX);
  const clampedMinY = Math.max(region.regionBounds.minY, minY);
  const clampedMaxY = Math.min(region.regionBounds.maxY, maxY);

  for (let x = clampedMinX; x <= clampedMaxX; x += 1) {
    for (let y = clampedMinY; y <= clampedMaxY; y += 1) {
      const cell = index.get(mapCellKey(x, y));
      if (cell) {
        cells.push(cell);
      }
    }
  }

  return cells;
}

function mapRegionCellIndex(region: OriginalMapRegion) {
  const cached = mapRegionCellIndexCache.get(region);
  if (cached) {
    return cached;
  }

  const index = new Map<string, OriginalMapCell>();
  for (const cell of region.cells) {
    index.set(mapCellKey(cell.x, cell.y), cell);
  }
  mapRegionCellIndexCache.set(region, index);
  return index;
}

function mapCellKey(x: number, y: number) {
  return `${x}:${y}`;
}

function appendViewportMapSprite(
  floorTarget: ViewportMapSprite[],
  objectTarget: ViewportMapSprite[],
  region: OriginalMapRegion,
  spriteId: string | null | undefined,
  cell: { x: number; y: number },
  player: DisplayEntity,
  animationFrameIndex: number,
  inFloorBounds: boolean,
  inObjectBounds: boolean,
) {
  if (!spriteId) {
    return;
  }

  const sprite = region.sprites[spriteId];
  if (!sprite || !sprite.frames.length) {
    return;
  }

  const target =
    sprite.drawMode === "floor"
      ? inFloorBounds
        ? floorTarget
        : null
      : inObjectBounds
        ? objectTarget
        : null;

  if (!target) {
    return;
  }

  const frame = sprite.frames[animationFrameIndex % sprite.frames.length] ?? sprite.frames[0];
  if (!frame) {
    return;
  }
  if (staticSceneAssetRecentlyFailed(frame.path)) {
    return;
  }

  const cellLeft = VIEWPORT_TILE_LEFT_ORIGIN + (cell.x - player.x) * VIEWPORT_CELL_WIDTH;
  const cellTop = VIEWPORT_TILE_TOP_ORIGIN + (cell.y - player.y) * VIEWPORT_CELL_HEIGHT;
  const crystalOffset = crystalMapFrameOffset(frame);
  const useCrystalOffset = sprite.drawMode === "object" && crystalMapFrameUsesOffset(frame);
  const left = cellLeft + (useCrystalOffset ? crystalOffset.x : 0);
  const top =
    sprite.drawMode === "object"
      ? cellTop + VIEWPORT_CELL_HEIGHT - frame.height + (useCrystalOffset ? crystalOffset.y : 0)
      : cellTop;

  if (sprite.drawMode === "object" && !mapSpriteIntersectsViewport(left, top, frame.width, frame.height)) {
    return;
  }

  target.push({
    key: `${spriteId}:${cell.x}:${cell.y}:${animationFrameIndex % sprite.frames.length}`,
    path: frame.path,
    cellX: cell.x,
    cellY: cell.y,
    left,
    top,
    width: frame.width,
    height: frame.height,
    zIndex: viewportDepthForCell(cell.x, cell.y, player, sprite.drawMode === "object" ? 1 : 0),
  });
}

function mapSpriteIntersectsViewport(left: number, top: number, width: number, height: number) {
  const marginX = VIEWPORT_CELL_WIDTH * 2;
  const marginY = VIEWPORT_CELL_HEIGHT * 4;
  return (
    left + width >= -marginX &&
    left <= ORIGINAL_UI.game.sceneWidth + marginX &&
    top + height >= -marginY &&
    top <= ORIGINAL_UI.game.sceneHeight + marginY
  );
}

function crystalMapFrameUsesOffset(frame: OriginalMapSpriteFrame) {
  return (
    typeof frame.offsetX === "number" ||
    typeof frame.offsetY === "number" ||
    crystalMapFrameHasLegacyOffsetFallback(frame.path)
  );
}

function crystalMapFrameOffset(frame: OriginalMapSpriteFrame): ViewportOffset {
  if (typeof frame.offsetX === "number" || typeof frame.offsetY === "number") {
    return {
      x: frame.offsetX ?? 0,
      y: frame.offsetY ?? 0,
    };
  }

  // Crystal draws the Bichon torch/fire blend frames with the Lib frame offset enabled.
  // Older packaged starter-map JSON predates offset export; these 100x100 light frames
  // are anchored around the red torch head, not the tile floor or lamp base.
  if (crystalMapFrameHasLegacyOffsetFallback(frame.path)) {
    return { x: -50, y: -100 };
  }

  return EMPTY_VIEWPORT_OFFSET;
}

function crystalMapFrameHasLegacyOffsetFallback(path: string) {
  return /\/original-map\/WemadeMir2\/Objects\/27(2[3-9]|3[0-2])\.png$/i.test(path);
}

function buildSceneBackdropTiles(world: DisplayWorld, player: DisplayEntity | null): SceneBackdropTile[] {
  const center = player
    ? { x: player.x, y: player.y }
    : world.sceneView?.center
      ? { x: world.sceneView.center.x, y: world.sceneView.center.y }
      : null;

  if (!center) {
    return [];
  }

  const startX = center.x - VIEWPORT_RANGE_X;
  const endX = center.x + VIEWPORT_RANGE_X;
  const startY = center.y - VIEWPORT_RANGE_Y;
  const endY = center.y + VIEWPORT_RANGE_Y;
  const tiles: SceneBackdropTile[] = [];

  for (let y = startY; y <= endY; y += 1) {
    for (let x = startX; x <= endX; x += 1) {
      const terrain = terrainKindAt(world.terrainPatches, x, y);
      const variation = Math.abs((x * 31 + y * 17) % 2);

      tiles.push({
        key: `${x}:${y}`,
        left: VIEWPORT_TILE_LEFT_ORIGIN + (x - center.x) * VIEWPORT_CELL_WIDTH,
        top: VIEWPORT_TILE_TOP_ORIGIN + (y - center.y) * VIEWPORT_CELL_HEIGHT,
        texture: sceneTextureForTerrain(terrain, variation),
        tint: sceneTintForTerrain(terrain, variation),
      });
    }
  }

  return tiles;
}

function terrainKindAt(
  patches: Array<{ x: number; y: number; width: number; height: number; kind: string }>,
  x: number,
  y: number,
) {
  for (let index = patches.length - 1; index >= 0; index -= 1) {
    const patch = patches[index];
    if (x >= patch.x && x < patch.x + patch.width && y >= patch.y && y < patch.y + patch.height) {
      return patch.kind;
    }
  }

  return patches[0]?.kind ?? "grass";
}

function sceneTextureForTerrain(terrain: string, variation: number) {
  switch (terrain) {
    case "dirt":
      return variation === 0 ? "/debug/map-samples/smtile-0.png" : "/debug/map-samples/smtile-104.png";
    case "road":
      return variation === 0 ? "/debug/map-samples/smtile-32.png" : "/debug/map-samples/smtile-52.png";
    case "water":
      return variation === 0 ? "/debug/map-samples/smtile-0.png" : "/debug/map-samples/tiles-1.png";
    case "stone":
      return variation === 0 ? "/debug/map-samples/tiles-0.png" : "/debug/map-samples/tiles-1.png";
    default:
      return variation === 0 ? "/debug/map-samples/smtile-72.png" : "/debug/map-samples/smtile-80.png";
  }
}

function sceneTintForTerrain(terrain: string, variation: number) {
  switch (terrain) {
    case "dirt":
      return variation === 0 ? "rgba(121, 84, 38, 0.16)" : "rgba(88, 58, 24, 0.10)";
    case "road":
      return variation === 0 ? "rgba(146, 108, 52, 0.14)" : "rgba(117, 85, 39, 0.12)";
    case "water":
      return variation === 0 ? "rgba(34, 84, 106, 0.48)" : "rgba(20, 58, 79, 0.52)";
    case "stone":
      return variation === 0 ? "rgba(76, 74, 66, 0.34)" : "rgba(54, 53, 46, 0.28)";
    default:
      return variation === 0 ? "rgba(58, 96, 36, 0.10)" : "rgba(42, 74, 28, 0.08)";
  }
}

export function mapSpriteBlendMode(path: string) {
  return /\/original-map\/WemadeMir2\/Objects\/27(2[3-9]|3[0-2])\.png$/i.test(path) ? "screen" : undefined;
}

export function mapSpriteRenderPath(path: string) {
  const frame = bichonTorchLightFrame(path);
  return frame ? `/generated/original-map-blend/WemadeMir2/Objects/${frame}.png` : path;
}

const SCENE_ASSET_DELAYED_RETRY_DELAYS_MS = [500, 1500, 3500, 7000, 12000];
const SCENE_ASSET_STALLED_RETRY_DELAYS_MS = [2500, 5000, 10000, 15000];
const MAP_OBJECT_ALPHA_KEY_SOLID_BRIGHTNESS = 18;
const MAP_OBJECT_ALPHA_KEY_FEATHER_BRIGHTNESS = 72;
// Once a scene asset has exhausted its retries it is negatively cached briefly. These assets are
// immutable, but the failure mode we see in production is usually a transient fetch/load miss while
// a scene asks for hundreds of small PNGs at once, not an actual absent R2 object.
const STATIC_SCENE_ASSET_NEGATIVE_CACHE_MS = 30 * 1000;
const ALPHA_KEYED_SCENE_ASSET_MAX_BYTES = 32 * 1024 * 1024;
const ALPHA_KEYED_SCENE_ASSET_MAX_ENTRIES = 256;
const FAILED_STATIC_SCENE_ASSET_MAX_ENTRIES = 1024;
type AlphaKeyedSceneAssetEntry = {
  promise: Promise<string | null>;
  url: string | null;
  bytes: number;
};
const alphaKeyedSceneAssetUrls = new Map<string, AlphaKeyedSceneAssetEntry>();
const failedStaticSceneAssetUrls = new Map<string, number>();
const loggedStaticSceneAssetFailures = new Set<string>();
let alphaKeyedSceneAssetBytes = 0;

export function sceneAssetCandidateUrls(url: string, retryAttempt = 1): string[] {
  const candidates: string[] = [];
  const add = (candidate: string | null) => {
    if (candidate && !candidates.includes(candidate)) {
      candidates.push(candidate);
    }
  };

  add(url);
  const remoteUrl = remoteSceneAssetUrl(url);
  add(remoteUrl);

  add(cacheBustedSceneAssetUrl(url, retryAttempt));
  if (remoteUrl) {
    add(cacheBustedSceneAssetUrl(remoteUrl, retryAttempt));
  }

  return candidates;
}

export function handleSceneAssetImageError(event: SyntheticEvent<HTMLImageElement>) {
  const image = event.currentTarget;
  const originalSrc = image.dataset.mir2OriginalSrc ?? image.getAttribute("src") ?? "";
  if (!originalSrc) {
    image.style.visibility = "hidden";
    return;
  }

  image.dataset.mir2OriginalSrc = originalSrc;
  if (image.dataset.mir2RetryOriginalSrc !== originalSrc) {
    delete image.dataset.mir2DelayedRetryCount;
    delete image.dataset.mir2IncompleteSince;
    delete image.dataset.mir2StalledRetryCount;
  }
  const candidates = sceneAssetCandidateUrls(originalSrc);
  const currentIndex =
    image.dataset.mir2RetryOriginalSrc === originalSrc
      ? Number.parseInt(image.dataset.mir2RetryIndex ?? "0", 10)
      : 0;
  const nextIndex = Number.isFinite(currentIndex) ? currentIndex + 1 : 1;
  const nextSrc = candidates[nextIndex];

  if (!nextSrc) {
    scheduleSceneAssetImageDelayedRetry(image, originalSrc);
    return;
  }

  image.dataset.mir2RetryOriginalSrc = originalSrc;
  image.dataset.mir2RetryIndex = String(nextIndex);
  image.style.visibility = "";
  image.src = nextSrc;
}

export function handleSceneAssetImageLoad(event: SyntheticEvent<HTMLImageElement>) {
  const image = event.currentTarget;
  clearSceneAssetRetryState(image);
  void applyMapObjectAlphaKey(image);
}

export function rescueStalledSceneAssetImages(root: ParentNode = document) {
  if (typeof window === "undefined") {
    return { checked: 0, retried: 0 };
  }

  const now = Date.now();
  let checked = 0;
  let retried = 0;
  const images = Array.from(root.querySelectorAll<HTMLImageElement>("img[data-mir2-original-src]"));

  for (const image of images) {
    checked += 1;
    if (image.complete && image.naturalWidth > 0 && image.naturalHeight > 0) {
      clearSceneAssetRetryState(image);
      continue;
    }
    if (image.dataset.mir2LoadFailed === "retrying") {
      continue;
    }

    const originalSrc = image.dataset.mir2OriginalSrc ?? image.getAttribute("src") ?? "";
    if (!originalSrc) {
      continue;
    }
    if (staticSceneAssetRecentlyFailed(originalSrc)) {
      image.dataset.mir2LoadFailed = "true";
      image.style.visibility = "hidden";
      continue;
    }

    const firstIncompleteAt = Number.parseInt(image.dataset.mir2IncompleteSince ?? "0", 10);
    if (!Number.isFinite(firstIncompleteAt) || firstIncompleteAt <= 0) {
      image.dataset.mir2IncompleteSince = String(now);
      continue;
    }

    const stalledRetryCount = Number.parseInt(image.dataset.mir2StalledRetryCount ?? "0", 10);
    const retryCount = Number.isFinite(stalledRetryCount) ? stalledRetryCount : 0;
    const delay =
      SCENE_ASSET_STALLED_RETRY_DELAYS_MS[
        Math.min(retryCount, SCENE_ASSET_STALLED_RETRY_DELAYS_MS.length - 1)
      ];
    if (now - firstIncompleteAt < delay) {
      continue;
    }

    const retrySrc = sceneAssetDelayedRetryUrl(originalSrc, retryCount + 10);
    if (!retrySrc) continue;

    image.dataset.mir2IncompleteSince = String(now);
    image.dataset.mir2StalledRetryCount = String(retryCount + 1);
    image.dataset.mir2RetryOriginalSrc = originalSrc;
    image.dataset.mir2RetryIndex = String(sceneAssetCandidateUrls(originalSrc).length + retryCount);
    image.style.visibility = "";
    image.src = retrySrc;
    retried += 1;
  }

  return { checked, retried };
}

function clearSceneAssetRetryState(image: HTMLImageElement) {
  image.style.visibility = "";
  delete image.dataset.mir2LoadFailed;
  delete image.dataset.mir2DelayedRetryCount;
  delete image.dataset.mir2IncompleteSince;
  delete image.dataset.mir2StalledRetryCount;
  delete image.dataset.mir2RetryIndex;
  delete image.dataset.mir2RetryOriginalSrc;
}

async function applyMapObjectAlphaKey(image: HTMLImageElement) {
  if (typeof window === "undefined") {
    return;
  }
  if (image.dataset.mir2AlphaKeyed === "true" || image.dataset.mir2AlphaKeyProcessing === "true") {
    return;
  }

  const originalSrc = image.dataset.mir2OriginalSrc ?? image.getAttribute("src") ?? "";
  if (!isBlackKeyedMapObjectPath(originalSrc)) {
    return;
  }

  const cacheKey = normalizedSceneAssetPath(originalSrc);
  if (!cacheKey || image.naturalWidth <= 0 || image.naturalHeight <= 0) {
    return;
  }

  image.dataset.mir2AlphaKeyProcessing = "true";
  image.style.visibility = "hidden";
  const keyedUrl = await alphaKeyedSceneAssetUrl(cacheKey, image).catch(() => null);
  delete image.dataset.mir2AlphaKeyProcessing;
  if (!keyedUrl) {
    image.style.visibility = "";
    return;
  }
  if (!image.isConnected || image.dataset.mir2OriginalSrc !== originalSrc) {
    return;
  }

  image.dataset.mir2AlphaKeyed = "true";
  image.src = keyedUrl;
}

function alphaKeyedSceneAssetUrl(cacheKey: string, image: HTMLImageElement) {
  const cached = alphaKeyedSceneAssetUrls.get(cacheKey);
  if (cached) {
    alphaKeyedSceneAssetUrls.delete(cacheKey);
    alphaKeyedSceneAssetUrls.set(cacheKey, cached);
    return cached.promise;
  }

  const entry: AlphaKeyedSceneAssetEntry = {
    promise: Promise.resolve(null),
    url: null,
    bytes: 0,
  };
  entry.promise = createAlphaKeyedSceneAssetUrl(image).then((result) => {
    if (!result) {
      alphaKeyedSceneAssetUrls.delete(cacheKey);
      return null;
    }
    entry.url = result.url;
    entry.bytes = result.bytes;
    alphaKeyedSceneAssetBytes += result.bytes;
    trimAlphaKeyedSceneAssetUrls();
    return result.url;
  }).catch((error) => {
    alphaKeyedSceneAssetUrls.delete(cacheKey);
    throw error;
  });
  alphaKeyedSceneAssetUrls.set(cacheKey, entry);
  return entry.promise;
}

async function createAlphaKeyedSceneAssetUrl(image: HTMLImageElement): Promise<{ url: string; bytes: number } | null> {
  const width = image.naturalWidth;
  const height = image.naturalHeight;
  if (width <= 0 || height <= 0) {
    return null;
  }

  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) {
    return null;
  }

  context.drawImage(image, 0, 0, width, height);
  const imageData = context.getImageData(0, 0, width, height);
  const pixels = imageData.data;
  const queued = new Uint8Array(width * height);
  const stack: number[] = [];
  let changed = 0;

  const enqueue = (x: number, y: number) => {
    if (x < 0 || y < 0 || x >= width || y >= height) {
      return;
    }
    const pixelIndex = y * width + x;
    if (queued[pixelIndex]) {
      return;
    }
    const offset = pixelIndex * 4;
    if (pixels[offset + 3] === 0 || pixelBrightness(pixels, offset) > MAP_OBJECT_ALPHA_KEY_FEATHER_BRIGHTNESS) {
      return;
    }
    queued[pixelIndex] = 1;
    stack.push(pixelIndex);
  };

  for (let x = 0; x < width; x += 1) {
    enqueue(x, 0);
    enqueue(x, height - 1);
  }
  for (let y = 1; y < height - 1; y += 1) {
    enqueue(0, y);
    enqueue(width - 1, y);
  }

  while (stack.length) {
    const pixelIndex = stack.pop()!;
    const offset = pixelIndex * 4;
    const brightness = pixelBrightness(pixels, offset);
    const alpha = pixels[offset + 3];
    const nextAlpha =
      brightness <= MAP_OBJECT_ALPHA_KEY_SOLID_BRIGHTNESS
        ? 0
        : Math.round(
            alpha *
              ((brightness - MAP_OBJECT_ALPHA_KEY_SOLID_BRIGHTNESS) /
                (MAP_OBJECT_ALPHA_KEY_FEATHER_BRIGHTNESS - MAP_OBJECT_ALPHA_KEY_SOLID_BRIGHTNESS)),
          );
    if (nextAlpha < alpha) {
      pixels[offset + 3] = Math.max(0, Math.min(alpha, nextAlpha));
      changed += 1;
    }

    const x = pixelIndex % width;
    const y = Math.floor(pixelIndex / width);
    enqueue(x + 1, y);
    enqueue(x - 1, y);
    enqueue(x, y + 1);
    enqueue(x, y - 1);
  }

  if (!changed) {
    return null;
  }

  context.putImageData(imageData, 0, 0);
  return new Promise<{ url: string; bytes: number } | null>((resolve) => {
    canvas.toBlob((blob) => {
      resolve(blob ? { url: URL.createObjectURL(blob), bytes: blob.size } : null);
    }, "image/png");
  });
}

function trimAlphaKeyedSceneAssetUrls() {
  while (
    (alphaKeyedSceneAssetUrls.size > ALPHA_KEYED_SCENE_ASSET_MAX_ENTRIES ||
      alphaKeyedSceneAssetBytes > ALPHA_KEYED_SCENE_ASSET_MAX_BYTES) &&
    alphaKeyedSceneAssetUrls.size > 1
  ) {
    const oldestKey = alphaKeyedSceneAssetUrls.keys().next().value as string | undefined;
    if (!oldestKey) break;
    const oldest = alphaKeyedSceneAssetUrls.get(oldestKey);
    alphaKeyedSceneAssetUrls.delete(oldestKey);
    alphaKeyedSceneAssetBytes -= oldest?.bytes ?? 0;
    if (oldest?.url) URL.revokeObjectURL(oldest.url);
  }
}

export function sceneAssetRuntimeStats() {
  return {
    alphaKeyedBlobCount: Array.from(alphaKeyedSceneAssetUrls.values()).filter((entry) => entry.url).length,
    alphaKeyedPendingCount: Array.from(alphaKeyedSceneAssetUrls.values()).filter((entry) => !entry.url).length,
    alphaKeyedBlobBytes: alphaKeyedSceneAssetBytes,
    failedStaticSceneAssetCount: failedStaticSceneAssetUrls.size,
  };
}

function pixelBrightness(pixels: Uint8ClampedArray, offset: number) {
  return Math.max(pixels[offset], pixels[offset + 1], pixels[offset + 2]);
}

function isBlackKeyedMapObjectPath(path: string) {
  const normalized = normalizedSceneAssetPath(path);
  return (
    normalized !== null &&
    normalized.startsWith("/original-map/") &&
    /\/(?:objects|objects2|smobjects|furnitures|walls|animations)/i.test(normalized)
  );
}

function normalizedSceneAssetPath(path: string) {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    return new URL(path, window.location.href).pathname;
  } catch {
    return null;
  }
}

function scheduleSceneAssetImageDelayedRetry(image: HTMLImageElement, originalSrc: string) {
  if (typeof window === "undefined") {
    image.dataset.mir2LoadFailed = "true";
    image.style.visibility = "hidden";
    return;
  }

  const retryCount = Number.parseInt(image.dataset.mir2DelayedRetryCount ?? "0", 10);
  const nextRetryCount = Number.isFinite(retryCount) ? retryCount + 1 : 1;
  const delay = SCENE_ASSET_DELAYED_RETRY_DELAYS_MS[nextRetryCount - 1];

  if (delay === undefined) {
    image.dataset.mir2LoadFailed = "true";
    image.style.visibility = "hidden";
    return;
  }

  image.dataset.mir2LoadFailed = "retrying";
  image.dataset.mir2DelayedRetryCount = String(nextRetryCount);
  image.style.visibility = "hidden";

  window.setTimeout(() => {
    if (!image.isConnected || image.dataset.mir2OriginalSrc !== originalSrc) {
      return;
    }
    const retrySrc = sceneAssetDelayedRetryUrl(originalSrc, nextRetryCount);
    if (!retrySrc) {
      image.dataset.mir2LoadFailed = "true";
      image.style.visibility = "hidden";
      return;
    }
    image.dataset.mir2RetryOriginalSrc = originalSrc;
    image.dataset.mir2RetryIndex = String(sceneAssetCandidateUrls(originalSrc).length);
    image.style.visibility = "";
    image.src = retrySrc;
  }, delay);
}

function sceneAssetDelayedRetryUrl(originalSrc: string, retryCount: number) {
  const retryCandidates = sceneAssetCandidateUrls(originalSrc, retryCount + 1).filter(
    (candidate) => candidate !== originalSrc,
  );
  if (!retryCandidates.length) {
    return cacheBustedSceneAssetUrl(originalSrc, retryCount + 1);
  }
  return retryCandidates[(retryCount - 1) % retryCandidates.length] ?? retryCandidates[0] ?? null;
}

function bichonTorchLightFrame(path: string) {
  const match = path.match(/\/original-map\/WemadeMir2\/Objects\/(27(?:2[3-9]|3[0-2]))\.png$/i);
  return match?.[1] ?? null;
}

type SceneAssetCacheWindow = Window & {
  __mir2AssetCache?: {
    remoteAssetBaseUrl?: string | null;
  };
};

function cacheBustedSceneAssetUrl(url: string, retryAttempt = 1) {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    const parsed = new URL(url, window.location.href);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }
    parsed.searchParams.set("mir2ImgRetry", String(Math.max(1, retryAttempt)));
    parsed.searchParams.set("mir2ImgRetryTs", Date.now().toString(36));
    return parsed.origin === window.location.origin
      ? `${parsed.pathname}${parsed.search}${parsed.hash}`
      : parsed.toString();
  } catch {
    return null;
  }
}

function remoteSceneAssetUrl(url: string) {
  if (typeof window === "undefined") {
    return null;
  }

  const remoteAssetBaseUrl = (window as SceneAssetCacheWindow).__mir2AssetCache?.remoteAssetBaseUrl;
  if (!remoteAssetBaseUrl) {
    return null;
  }

  try {
    const parsed = new URL(url, window.location.href);
    const normalizedBase = remoteAssetBaseUrl.replace(/\/+$/, "");
    if (parsed.href.startsWith(`${normalizedBase}/`)) {
      return null;
    }
    if (!isRemoteBackedSceneAssetPath(parsed.pathname)) {
      return null;
    }
    return `${normalizedBase}/${parsed.pathname.replace(/^\/+/, "")}`;
  } catch {
    return null;
  }
}

function isRemoteBackedSceneAssetPath(path: string) {
  return (
    path.startsWith("/original-ui/") ||
    path.startsWith("/original-map/") ||
    path.startsWith("/generated/original-map-blend/") ||
    path.startsWith("/bevy-entity-atlases/") ||
    path.startsWith("/bevy-runtime/")
  );
}

function isImmutableSceneAssetUrl(url: string) {
  if (typeof window === "undefined") {
    return isRemoteBackedSceneAssetPath(url);
  }

  try {
    return isRemoteBackedSceneAssetPath(new URL(url, window.location.href).pathname);
  } catch {
    return false;
  }
}

function staticSceneAssetFailureKey(url: string) {
  if (typeof window === "undefined") {
    return url;
  }

  try {
    return new URL(url, window.location.href).pathname;
  } catch {
    return url;
  }
}

function staticSceneAssetRecentlyFailed(url: string) {
  const key = staticSceneAssetFailureKey(url);
  const failedAt = failedStaticSceneAssetUrls.get(key);
  if (!failedAt) {
    return false;
  }

  if (Date.now() - failedAt <= STATIC_SCENE_ASSET_NEGATIVE_CACHE_MS) {
    return true;
  }

  failedStaticSceneAssetUrls.delete(key);
  loggedStaticSceneAssetFailures.delete(key);
  return false;
}

function markStaticSceneAssetFailed(image: HTMLImageElement, originalSrc: string) {
  const failureKey = staticSceneAssetFailureKey(originalSrc);
  failedStaticSceneAssetUrls.set(failureKey, Date.now());
  if (!loggedStaticSceneAssetFailures.has(failureKey)) {
    loggedStaticSceneAssetFailures.add(failureKey);
    console.warn("[mir2] scene asset missing", { path: failureKey });
  }
  while (failedStaticSceneAssetUrls.size > FAILED_STATIC_SCENE_ASSET_MAX_ENTRIES) {
    const oldestKey = failedStaticSceneAssetUrls.keys().next().value as string | undefined;
    if (!oldestKey) break;
    failedStaticSceneAssetUrls.delete(oldestKey);
    loggedStaticSceneAssetFailures.delete(oldestKey);
  }
  image.dataset.mir2LoadFailed = "true";
  image.style.visibility = "hidden";
}
