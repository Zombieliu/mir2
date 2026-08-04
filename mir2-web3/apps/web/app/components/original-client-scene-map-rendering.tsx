"use client";

import { memo, type SyntheticEvent } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import blendFramesManifest from "../../public/generated/original-map-blend/blend-frames.json";
import {
  type MapAtlasIndex,
  mapAtlasPathRequiresAlphaKey,
  mapAtlasRectKeyForPath,
} from "../../lib/map-atlas-manifest";
import type { OriginalMapRegion, OriginalMapSpriteFrame } from "../../lib/scene-types";
import {
  alphaKeyMapObjectPixels,
  keyMapObjectImageOffThread,
  offThreadAlphaKeyAvailable,
} from "../../lib/scene-alpha-key";
import type { DisplayEntity, DisplayWorld } from "./original-client-types";
import type { MapStandaloneTileDraw, MapTileDraw } from "./webgl2-map-atlas-layer";
import {
  EMPTY_VIEWPORT_MAP_SPRITES,
  EMPTY_VIEWPORT_OFFSET,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  DEFAULT_VIEWPORT_LAYOUT,
  viewportDepthForCell,
  viewportFloorDepthForCell,
  type SceneBackdropTile,
  type ViewportMapSprite,
  type ViewportMapSprites,
  type ViewportOffset,
  type ViewportLayout,
} from "./original-client-scene-layout";

type OriginalMapCell = OriginalMapRegion["cells"][number];
type OriginalMapSpriteKind = OriginalMapRegion["sprites"][string]["kind"];

const mapRegionCellIndexCache = new WeakMap<OriginalMapRegion, Map<string, OriginalMapCell>>();
const FLOOR_LAYER_ORDERS: Record<OriginalMapSpriteKind, number> = {
  back: 0,
  middle: 1,
  front: 2,
  tileAnimation: 3,
};

function GameSceneBackdropInner({
  world,
  player,
  floorSprites,
  cameraOffset,
  viewportLayout = DEFAULT_VIEWPORT_LAYOUT,
  imperativeCamera = false,
  registerCameraSurface,
}: {
  world: DisplayWorld;
  player: DisplayEntity | null;
  floorSprites: ViewportMapSprite[];
  cameraOffset: ViewportOffset;
  viewportLayout?: ViewportLayout;
  imperativeCamera?: boolean;
  registerCameraSurface?: (key: string) => (el: HTMLElement | null) => void;
}) {
  const tiles = world.originalMapRegion ? [] : buildSceneBackdropTiles(world, player, viewportLayout);

  if (!tiles.length && !floorSprites.length) {
    return null;
  }

  const renderOffset = floorSprites.length ? cameraOffset : EMPTY_VIEWPORT_OFFSET;

  return (
    <div
      ref={imperativeCamera ? registerCameraSurface?.("backdrop") : undefined}
      className="game-scene-backdrop"
    >
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
          decoding="async"
          onError={handleSceneAssetImageError}
          onLoad={handleSceneAssetImageLoad}
          style={{
            left: sprite.left + cameraOffset.x,
            top: sprite.top + cameraOffset.y,
            width: sprite.width,
            height: sprite.height,
            zIndex: sprite.zIndex,
          }}
        />
      ))}
    </div>
  );
}

// Memoised so the 30Hz motion tick skips the DOM floor-tile backdrop when neither the world, the
// floor sprite list, nor the camera offset changed. (Only the DOM fallback path — the GPU map
// atlas layer is unaffected.)
export const GameSceneBackdrop = memo(GameSceneBackdropInner);

// Splitting the viewport build by animation cadence keeps the ~500-sprite static
// layer off the 120ms animation tick: callers memoize "static" on [player.x,y,region]
// (rebuilt only on movement) and "animated" on the frame index (cheap — only multi-frame
// sprites do work; single-frame sprites are skipped early). "all" preserves old behavior.
export type ViewportSpriteAnimationFilter = "all" | "static" | "animated";

export function buildViewportMapSprites(
  world: DisplayWorld,
  player: DisplayEntity,
  animationFrameIndex: number,
  animationFilter: ViewportSpriteAnimationFilter = "all",
  rangeExpansion = 0,
  viewportLayout: ViewportLayout = DEFAULT_VIEWPORT_LAYOUT,
): ViewportMapSprites {
  const region = world.originalMapRegion;
  if (!region) {
    return EMPTY_VIEWPORT_MAP_SPRITES;
  }

  // rangeExpansion widens the cell window beyond the visible viewport — used by the
  // off-screen prefetch ring to warm tiles in the player's surroundings before they
  // scroll into view (eliminates walk-time pop-in). 0 = exact visible viewport.
  const floorMinX = player.x - viewportLayout.rangeX - rangeExpansion;
  const floorMaxX = player.x + viewportLayout.rangeX + rangeExpansion;
  const floorMinY = player.y - viewportLayout.rangeY - rangeExpansion;
  const floorMaxY = player.y + viewportLayout.rangeY + rangeExpansion;
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
      animationFilter,
      viewportLayout,
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
      animationFilter,
      viewportLayout,
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
      animationFilter,
      viewportLayout,
    );
    appendViewportMapSprite(
      floor,
      objects,
      region,
      cell.tileAnimation,
      cell,
      player,
      animationFrameIndex,
      inFloorBounds,
      true,
      animationFilter,
      viewportLayout,
    );
  }

  return {
    floor,
    objects,
  };
}

// Map a viewport's DOM map sprites (floor + objects) to a GPU draw list against the packed
// map atlases. Each sprite's (library, frame) is recovered from its per-tile PNG path and
// looked up in the atlas index; cameraOffset is folded in so GPU quads align with the DOM /
// entity layers. Sprites whose frame is NOT in the atlas (e.g. a frame the local export — and
// thus the packed atlas — happens to lack) are returned in `uncovered` so the caller can still
// render exactly those cells via the DOM <img> path. This keeps the GPU fast path for the
// covered majority while guaranteeing no black holes for any frame the atlas is missing.
export function buildMapTileDrawList(
  mapSprites: ViewportMapSprites,
  index: MapAtlasIndex,
  cameraOffset: ViewportOffset,
): { tiles: MapTileDraw[]; uncovered: ViewportMapSprites } {
  const tiles: MapTileDraw[] = [];
  const uncoveredFloor: ViewportMapSprite[] = [];
  const uncoveredObjects: ViewportMapSprite[] = [];
  const add = (sprite: ViewportMapSprite, uncovered: ViewportMapSprite[]) => {
    if (resolvedMapSpriteBlendMode(sprite) || mapAtlasPathRequiresAlphaKey(sprite.path)) {
      // Additive cells need SourceAlpha + One, while legacy object libraries need
      // Crystal's black-key conversion. Both require the decoded standalone path.
      uncovered.push(sprite);
      return;
    }
    const rectKey = mapAtlasRectKeyForPath(sprite.path);
    const atlasKey = rectKey ? index.rectToAtlas.get(rectKey) : undefined;
    if (!rectKey || !atlasKey) {
      uncovered.push(sprite);
      return;
    }
    tiles.push({
      key: sprite.key,
      atlasKey,
      rectKey,
      left: sprite.left + cameraOffset.x,
      top: sprite.top + cameraOffset.y,
      width: sprite.width,
      height: sprite.height,
      z: sprite.zIndex,
    });
  };
  for (const sprite of mapSprites.floor) add(sprite, uncoveredFloor);
  for (const sprite of mapSprites.objects) add(sprite, uncoveredObjects);
  return { tiles, uncovered: { floor: uncoveredFloor, objects: uncoveredObjects } };
}

export type MapStandaloneTileImageSource = {
  imageKey: string;
  fetchUrl: string;
  alphaKeyMapObject: boolean;
};

export function buildStandaloneMapTiles(
  mapSprites: ViewportMapSprites,
  cameraOffset: ViewportOffset,
): {
  tiles: MapStandaloneTileDraw[];
  images: MapStandaloneTileImageSource[];
  domFallback: ViewportMapSprites;
  imageKeyBySpriteKey: Map<string, string>;
  requiredImageKeysBySpriteKey: Map<string, readonly string[]>;
  requiredImageKeysByTileKey: Map<string, readonly string[]>;
} {
  const tiles: MapStandaloneTileDraw[] = [];
  const images = new Map<string, MapStandaloneTileImageSource>();
  const domFallbackFloor: ViewportMapSprite[] = [];
  const domFallbackObjects: ViewportMapSprite[] = [];
  const imageKeyBySpriteKey = new Map<string, string>();
  const requiredImageKeysBySpriteKey = new Map<string, readonly string[]>();
  const requiredImageKeysByTileKey = new Map<string, readonly string[]>();
  const addImageSource = (path: string, additive: boolean) => {
    const rectKey = mapAtlasRectKeyForPath(path);
    const imageKey = `${additive ? "standalone-additive" : "standalone"}:${rectKey ?? path}`;
    // Additive blending needs the original dark-matte pixels. Black contributes
    // zero under SourceAlpha + One; the cleaned DOM asset would attenuate twice.
    const fetchUrl = additive ? path : mapSpriteRenderPath(path);
    const alphaKeyMapObject = !additive && mapAtlasPathRequiresAlphaKey(fetchUrl);
    const existing = images.get(imageKey);
    if (!existing) {
      images.set(imageKey, { imageKey, fetchUrl, alphaKeyMapObject });
    } else if (alphaKeyMapObject && !existing.alphaKeyMapObject) {
      images.set(imageKey, { ...existing, alphaKeyMapObject: true });
    }
    return imageKey;
  };
  const add = (
    sprite: ViewportMapSprite,
    domFallback: ViewportMapSprite[],
  ) => {
    // Keep every atlas miss in DOM until the shell confirms the decoded image is
    // resident in the current Bevy runtime. Additive misses use a dedicated
    // SourceAlpha + One material once that handoff completes.
    domFallback.push(sprite);
    const additive = Boolean(resolvedMapSpriteBlendMode(sprite));
    const imageKey = addImageSource(sprite.path, additive);
    const tileKey = `${additive ? "standalone-additive" : "standalone"}:${sprite.key}`;
    const familyPaths =
      additive && sprite.animationFramePaths?.length
        ? sprite.animationFramePaths
        : [sprite.path];
    const requiredImageKeys = Array.from(
      new Set(familyPaths.map((path) => addImageSource(path, additive))),
    );
    tiles.push({
      key: tileKey,
      imageKey,
      left: sprite.left + cameraOffset.x,
      top: sprite.top + cameraOffset.y,
      width: sprite.width,
      height: sprite.height,
      z: sprite.zIndex,
      additive: additive || undefined,
    });
    imageKeyBySpriteKey.set(sprite.key, imageKey);
    requiredImageKeysBySpriteKey.set(sprite.key, requiredImageKeys);
    requiredImageKeysByTileKey.set(tileKey, requiredImageKeys);
  };
  for (const sprite of mapSprites.floor) add(sprite, domFallbackFloor);
  for (const sprite of mapSprites.objects) add(sprite, domFallbackObjects);
  return {
    tiles,
    images: Array.from(images.values()),
    domFallback: { floor: domFallbackFloor, objects: domFallbackObjects },
    imageKeyBySpriteKey,
    requiredImageKeysBySpriteKey,
    requiredImageKeysByTileKey,
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
  animationFilter: ViewportSpriteAnimationFilter = "all",
  viewportLayout: ViewportLayout = DEFAULT_VIEWPORT_LAYOUT,
) {
  if (!spriteId) {
    return;
  }

  const sprite = region.sprites[spriteId];
  if (!sprite || !sprite.frames.length) {
    return;
  }

  // Animation-cadence partition: single-frame sprites are static (their output never
  // changes with animationFrameIndex), multi-frame sprites animate. Skip early — before
  // any position/frame work — so the per-tick "animated" pass does almost nothing on
  // maps without animated tiles.
  const isAnimatedSprite = sprite.frames.length > 1;
  if (animationFilter === "static" && isAnimatedSprite) {
    return;
  }
  if (animationFilter === "animated" && !isAnimatedSprite) {
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

  const cellLeft = viewportLayout.tileLeftOrigin + (cell.x - player.x) * VIEWPORT_CELL_WIDTH;
  const cellTop = viewportLayout.tileTopOrigin + (cell.y - player.y) * VIEWPORT_CELL_HEIGHT;
  const crystalOffset = crystalMapFrameOffset(frame);
  const useCrystalOffset = sprite.drawMode === "object" && crystalMapFrameUsesOffset(frame);
  const left = cellLeft + (useCrystalOffset ? crystalOffset.x : 0);
  const top =
    sprite.drawMode === "object"
      ? cellTop + VIEWPORT_CELL_HEIGHT - frame.height + (useCrystalOffset ? crystalOffset.y : 0)
      : cellTop;

  if (
    sprite.drawMode === "object" &&
    !mapSpriteIntersectsViewport(left, top, frame.width, frame.height, viewportLayout)
  ) {
    return;
  }

  target.push({
    key: `${spriteId}:${cell.x}:${cell.y}:${animationFrameIndex % sprite.frames.length}`,
    path: frame.path,
    animationFramePaths:
      sprite.frames.length > 1
        ? Array.from(new Set(sprite.frames.map((entry) => entry.path)))
        : undefined,
    kind: sprite.kind,
    blendMode: sprite.blendMode,
    cellX: cell.x,
    cellY: cell.y,
    left,
    top,
    width: frame.width,
    height: frame.height,
    zIndex:
      sprite.drawMode === "floor"
        ? viewportFloorDepthForCell(
            cell.x,
            cell.y,
            player,
            FLOOR_LAYER_ORDERS[sprite.kind],
            viewportLayout,
          )
        : viewportDepthForCell(cell.x, cell.y, player, 1),
  });
}

function mapSpriteIntersectsViewport(
  left: number,
  top: number,
  width: number,
  height: number,
  viewportLayout: ViewportLayout = DEFAULT_VIEWPORT_LAYOUT,
) {
  const marginX = VIEWPORT_CELL_WIDTH * 2;
  const marginY = VIEWPORT_CELL_HEIGHT * 4;
  return (
    left + width >= -marginX &&
    left <= viewportLayout.stageWidth + marginX &&
    top + height >= -marginY &&
    top <= viewportLayout.stageHeight + marginY
  );
}

function crystalMapFrameUsesOffset(frame: OriginalMapSpriteFrame) {
  // Crystal applies the library frame (X,Y) offset (GetOffSet / point.Offset(mi.X,mi.Y))
  // to map sprites in only two floor/object draw cases:
  //   - blend overloads: torch/fire frames WemadeMir2/Objects 2723-2732
  //     (Crystal GameScene.cs:10928 + MLibrary.cs:699, offSet=true)
  //   - fileIndex==28 (WemadeMir2/Objects27) non-blend with a real offset
  //     (Crystal GameScene.cs:10932-10933, anchored at drawY - CellHeight)
  // General front objects (WemadeMir2/Objects = fileIndex 2, e.g. the Bichon shore/cliff
  // strips Objects/102,103,104,213) draw via the raw Draw(index, drawX, drawY - s.Height)
  // overload (MLibrary.cs:640-657) with NO offset. The loader stamps offsetX/offsetY onto
  // EVERY frame (crystal-map-loader.ts), where (7,-44) is a library-wide constant that
  // Crystal deliberately ignores for fileIndex 2 — so a numeric offset must NOT enable it,
  // or every tall front object is shoved up 44px/right 7px off its bottom-cell anchor.
  if (crystalMapFrameHasLegacyOffsetFallback(frame.path)) return true;
  if (
    /\/original-map\/WemadeMir2\/Objects27\//i.test(frame.path) &&
    ((typeof frame.offsetX === "number" && frame.offsetX !== 0) ||
      (typeof frame.offsetY === "number" && frame.offsetY !== 0))
  ) {
    return true;
  }
  return false;
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

function buildSceneBackdropTiles(
  world: DisplayWorld,
  player: DisplayEntity | null,
  viewportLayout: ViewportLayout = DEFAULT_VIEWPORT_LAYOUT,
): SceneBackdropTile[] {
  const center = player
    ? { x: player.x, y: player.y }
    : world.sceneView?.center
      ? { x: world.sceneView.center.x, y: world.sceneView.center.y }
      : null;

  if (!center) {
    return [];
  }

  const startX = center.x - viewportLayout.rangeX;
  const endX = center.x + viewportLayout.rangeX;
  const startY = center.y - viewportLayout.rangeY;
  const endY = center.y + viewportLayout.rangeY;
  const tiles: SceneBackdropTile[] = [];

  for (let y = startY; y <= endY; y += 1) {
    for (let x = startX; x <= endX; x += 1) {
      const terrain = terrainKindAt(world.terrainPatches, x, y);
      const variation = Math.abs((x * 31 + y * 17) % 2);

      tiles.push({
        key: `${x}:${y}`,
        left: viewportLayout.tileLeftOrigin + (x - center.x) * VIEWPORT_CELL_WIDTH,
        top: viewportLayout.tileTopOrigin + (y - center.y) * VIEWPORT_CELL_HEIGHT,
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
  return blendObjectFrameKey(path) ? "screen" : undefined;
}

export function resolvedMapSpriteBlendMode(
  sprite: Pick<ViewportMapSprite, "path" | "blendMode">,
) {
  if (sprite.blendMode === "additive") return "screen";
  if (sprite.blendMode === "normal") return undefined;
  return mapSpriteBlendMode(sprite.path);
}

export function mapSpriteRenderPath(path: string) {
  const key = blendObjectFrameKey(path);
  return key ? `/generated/original-map-blend/${BLEND_MANIFEST_LIB}/${key}.png` : path;
}

const SCENE_ASSET_DELAYED_RETRY_DELAYS_MS = [500, 1500, 3500, 7000, 12000];
const SCENE_ASSET_STALLED_RETRY_DELAYS_MS = [2500, 5000, 10000, 15000];
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
  add(cacheBustedSceneAssetUrl(url, retryAttempt));

  for (const remoteUrl of remoteSceneAssetUrls(url)) {
    add(remoteUrl);
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
  if (!mapAtlasPathRequiresAlphaKey(originalSrc)) {
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

  // Preferred path: run the flood-fill + PNG encode in a worker so it never blocks the main
  // thread (which also drives Bevy's render loop). Falls through to the synchronous path on
  // unsupported browsers or a worker error/timeout.
  if (offThreadAlphaKeyAvailable()) {
    try {
      return await keyMapObjectImageOffThread(image, width, height);
    } catch {
      // fall back to main-thread keying below
    }
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
  const changed = alphaKeyMapObjectPixels(imageData.data, width, height);
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

function normalizedSceneAssetPath(path: string) {
  try {
    const baseUrl =
      typeof window === "undefined" ? "https://mir2.invalid/" : window.location.href;
    return new URL(path, baseUrl).pathname;
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

// Crystal blends glow objects ADDITIVELY (DXManager.SetBlend: SourceAlpha + One). On the DOM
// fallback path we fake additive with a darkness->alpha cleaned sprite plus per-shape
// opacity/filter-tuned mix-blend-mode:screen. The set of glow frames
// is no longer a hardcoded index range (2723..2732 was the DrawBlend `offSet` argument, GameScene.cs:10928,
// NOT the blend gate) — it is the data-driven manifest emitted by
// scripts/generate-crystal-map-blend-assets.mjs (dark-matte/bright-core pixel classification). The ten
// original Bichon torches remain in that manifest and render byte-identically.
const BLEND_MANIFEST_LIB = blendFramesManifest.lib;
const BLEND_OBJECT_FRAMES = new Set<string>(blendFramesManifest.frames);

// Returns the manifest key ("<objLib>/<frame>", e.g. "Objects/2723") if this sprite path is a blend
// glow frame, else null.
function blendObjectFrameKey(path: string): string | null {
  const match = path.match(
    new RegExp(`/original-map/${BLEND_MANIFEST_LIB}/(Objects[0-9]*/\\d+)\\.png$`, "i"),
  );
  if (!match?.[1]) {
    return null;
  }
  const key = match[1];
  return BLEND_OBJECT_FRAMES.has(key) ? key : null;
}

type SceneAssetCacheWindow = Window & {
  __mir2AssetCache?: {
    remoteAssetBaseUrl?: string | null;
    remoteAssetFallbackBaseUrls?: string[];
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

function remoteSceneAssetUrls(url: string) {
  if (typeof window === "undefined") {
    return [];
  }

  const assetCache = (window as SceneAssetCacheWindow).__mir2AssetCache;
  const remoteAssetBaseUrls = [
    ...(assetCache?.remoteAssetFallbackBaseUrls ?? []),
    assetCache?.remoteAssetBaseUrl,
  ].filter((value): value is string => Boolean(value));

  try {
    const parsed = new URL(url, window.location.href);
    if (!isRemoteBackedSceneAssetPath(parsed.pathname)) {
      return [];
    }
    return Array.from(
      new Set(
        remoteAssetBaseUrls.flatMap((baseUrl) => {
          const normalizedBase = baseUrl.replace(/\/+$/, "");
          return parsed.href.startsWith(`${normalizedBase}/`)
            ? []
            : [`${normalizedBase}/${parsed.pathname.replace(/^\/+/, "")}`];
        }),
      ),
    );
  } catch {
    return [];
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

/** Scene asset paths currently inside the negative-cache window (i.e. recently failed to load). */
export function listFailedSceneAssets(limit = 60): string[] {
  const now = Date.now();
  const out: string[] = [];
  for (const [key, failedAt] of failedStaticSceneAssetUrls) {
    if (now - failedAt <= STATIC_SCENE_ASSET_NEGATIVE_CACHE_MS) {
      out.push(key);
      if (out.length >= limit) break;
    }
  }
  return out;
}

/** "/original-map/WemadeMir2/Tiles/901.png" -> { library: "WemadeMir2/Tiles", frame: "901" }. */
function parseSceneAssetPath(path: string): { library: string; frame: string } {
  const match = path.match(/\/((?:Wemade|Shanda)Mir[23])\/([^/]+)\/(\d+)\.[a-z0-9]+$/i);
  if (match) {
    return { library: `${match[1]}/${match[2]}`, frame: match[3] };
  }
  const segments = path.split("/").filter(Boolean);
  return { library: segments.slice(-2, -1)[0] ?? path, frame: segments.slice(-1)[0] ?? "?" };
}

export type RenderStateSummary = {
  available: boolean;
  reason?: string;
  viewport?: Record<string, unknown>;
  layerCounts?: Record<string, number>;
  librariesInUse?: Record<string, number>;
  /** Player-centred grid: [x, y, "b:WemadeMir2/Tiles#901 f:WemadeMir2/Objects#256"]. */
  cells?: Array<[number, number, string]>;
  failedAssets?: string[];
};

/**
 * Compact, bounded summary of what the scene renderer is drawing right now — the data
 * you would otherwise have to reverse-engineer from the .map binary. Reuses the exact
 * sprite-build path the renderer uses, so layerCounts/librariesInUse reflect reality
 * (e.g. layerCounts.tileAnimation === 0 means "no animated tiles here, water is a
 * back/front tile"). Kept small (player ± RADIUS cells, deduped libraries) because the
 * snapshot payload already approaches the MCP read-size limit.
 */
export function buildRenderStateSummary(world: DisplayWorld, player: DisplayEntity | null): RenderStateSummary {
  const region = world.originalMapRegion;
  if (!region) return { available: false, reason: "no-region" };
  if (!player) return { available: false, reason: "no-player" };

  const { floor, objects } = buildViewportMapSprites(world, player, 0);
  const layerCounts: Record<string, number> = { back: 0, middle: 0, front: 0, tileAnimation: 0 };
  const librariesInUse: Record<string, number> = {};
  for (const sprite of [...floor, ...objects]) {
    layerCounts[sprite.kind] = (layerCounts[sprite.kind] ?? 0) + 1;
    const { library } = parseSceneAssetPath(sprite.path);
    librariesInUse[library] = (librariesInUse[library] ?? 0) + 1;
  }

  const RADIUS = 5;
  const cellIndex = mapRegionCellIndex(region);
  const cells: Array<[number, number, string]> = [];
  const describeLayer = (code: string, spriteId: string | null | undefined): string | null => {
    if (!spriteId) return null;
    const sprite = region.sprites[spriteId];
    const frame = sprite?.frames[0];
    if (!frame) return `${code}:?`;
    const { library, frame: frameIndex } = parseSceneAssetPath(frame.path);
    return `${code}:${library}#${frameIndex}`;
  };
  for (let y = player.y - RADIUS; y <= player.y + RADIUS; y += 1) {
    for (let x = player.x - RADIUS; x <= player.x + RADIUS; x += 1) {
      const cell = cellIndex.get(mapCellKey(x, y));
      if (!cell) continue;
      const parts = [
        describeLayer("b", cell.back),
        describeLayer("m", cell.middle),
        describeLayer("f", cell.front),
        describeLayer("a", cell.tileAnimation),
      ].filter((part): part is string => Boolean(part));
      if (parts.length) cells.push([x, y, parts.join(" ")]);
    }
  }

  return {
    available: true,
    viewport: {
      mapFile: region.mapFileName,
      mapTitle: world.mapTitle ?? null,
      playerX: player.x,
      playerY: player.y,
      floorSprites: floor.length,
      objectSprites: objects.length,
      regionCells: region.cells.length,
    },
    layerCounts,
    librariesInUse,
    cells,
    failedAssets: listFailedSceneAssets(60),
  };
}
