"use client";

import { Fragment, memo, useEffect, useRef, useState, type CSSProperties, type MouseEvent } from "react";

import type { ClientScreen } from "../../lib/original-ui";
import { originalAssetPath } from "../../lib/asset-url";
import type { EffectAnimation, EffectAssets } from "../../lib/crystal-magic-effects";
import { loadEffectAssets } from "../../lib/crystal-magic-effects";
import {
  collectResolvedSceneEffectFrames,
  CRYSTAL_ADDITIVE_MIX_BLEND_MODE,
  crystalSceneEffectLayerOffset,
  sceneEffectAnimationAssetUrls,
} from "../../lib/scene-effect-runtime";
import { originalItemIconPath } from "./original-client-inventory-utils";
import { CrystalGdiTextImage, findCrystalGdiTextAsset } from "./crystal-gdi-text";
import {
  collectViewportFallbackVfx,
  fallbackVfxStyle,
  paletteForElement,
  type FallbackVfx,
} from "../../lib/vfx-fallback";
import type {
  CrystalEntityAnimationPose,
  DisplayEntity,
  DisplayProjectile,
  DisplayWorld,
  EntityKind,
  EntityMotionSnapshot,
  TranslateFn,
} from "./original-client-types";
import {
  crystalLightTexturePath,
  crystalMapLightSpec,
  crystalMapLightTopLeft,
  crystalObjectLightSpec,
  crystalObjectLightTopLeft,
  crystalSceneDarknessColor,
  crystalSceneLightClassName,
  type CrystalMapLightSpec,
} from "./original-client-scene-lighting";
import {
  EMPTY_VIEWPORT_OFFSET,
  DEFAULT_VIEWPORT_LAYOUT,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  argbToCssColor,
  entityMotionOffsetForEntity,
  entityNameplateColor,
  entityNameplateLeftOffset,
  entityNameplateTopOffset,
  entityQuestIconLeftOffset,
  entityQuestIconTopOffset,
  entitySpriteHitBounds,
  handleSceneAssetImageError,
  handleSceneAssetImageLoad,
  isEntityAttacking,
  isEntityReviving,
  isEntityStruck,
  mapSpriteRenderPath,
  resolvedMapSpriteBlendMode,
  questIconForEntity,
  ratio,
  viewportDepthForCell,
  type ViewportEntitySprite,
  type ViewportLayout,
  type ViewportMapSprites,
  type ViewportOffset,
} from "./original-client-scene-rendering";
import { OriginalClientWeatherLayer } from "./original-client-weather-layer";

type ViewportGroundDrop = DisplayWorld["groundDrops"][number] & {
  dx: number;
  dy: number;
};

type ViewportProjectile = DisplayProjectile & {
  fromDx: number;
  fromDy: number;
  toDx: number;
  toDy: number;
  progress: number;
};

type ViewportEntitySpriteEntry = {
  entity: DisplayEntity & { dx: number; dy: number };
  animationPose?: CrystalEntityAnimationPose | null;
  sprite: ViewportEntitySprite | null;
};

type ViewportMapLight = {
  key: string;
  value: number;
  range: number;
  left: number;
  top: number;
  width: number;
  height: number;
  tone: CrystalMapLightSpec["tone"];
  opacity: number;
};

type EntitySpriteLayersProps = {
  useBevyEntityRenderer: boolean;
  sprite: ViewportEntitySprite | null;
  objectId: number | string;
};

// The body/hair/weapon <img> layers for one actor. Memoised so they only re-render when their
// *sprite frame data* actually changes (direction / animation / the 120ms tick) — NOT on every
// 60fps motion-clock tick. The smooth per-frame position lives on the parent
// `.entity-sprite-stack` wrapper, so these inner layers stay byte-identical between frame changes.
// That removes the per-frame restyle/reconcile churn that made running janky and standing NPCs
// (a single static frame, redrawn 60×/sec for nothing) visibly flicker.
const EntitySpriteLayers = memo(function EntitySpriteLayers({
  useBevyEntityRenderer,
  sprite,
  objectId,
}: EntitySpriteLayersProps) {
  return (
    <>
      {!useBevyEntityRenderer && sprite?.mount ? (
        <img
          className="entity-sprite-layer mount"
          src={originalAssetPath(sprite.mount.path)}
          alt=""
          draggable={false}
          data-mir2-original-src={originalAssetPath(sprite.mount.path)}
          onError={handleSceneAssetImageError}
          onLoad={handleSceneAssetImageLoad}
          style={{
            left: sprite.mount.x,
            top: sprite.mount.y,
            width: sprite.mount.width,
            height: sprite.mount.height,
          }}
        />
      ) : null}
      {!useBevyEntityRenderer &&
        sprite?.rearWeapons.map((weapon, index) => (
          <img
            key={`rear-${objectId}-${index}-${weapon.path}`}
            className="entity-sprite-layer weapon rear"
            src={originalAssetPath(weapon.path)}
            alt=""
            draggable={false}
            data-mir2-original-src={originalAssetPath(weapon.path)}
            onError={handleSceneAssetImageError}
            onLoad={handleSceneAssetImageLoad}
            style={{ left: weapon.x, top: weapon.y, width: weapon.width, height: weapon.height }}
          />
        ))}
      {!useBevyEntityRenderer && sprite?.body ? (
        <img
          className="entity-sprite-layer body"
          src={originalAssetPath(sprite.body.path)}
          alt=""
          draggable={false}
          data-mir2-original-src={originalAssetPath(sprite.body.path)}
          onError={handleSceneAssetImageError}
          onLoad={handleSceneAssetImageLoad}
          style={{
            left: sprite.body.x,
            top: sprite.body.y,
            width: sprite.body.width,
            height: sprite.body.height,
          }}
        />
      ) : null}
      {!useBevyEntityRenderer && sprite?.hair ? (
        <img
          className="entity-sprite-layer hair"
          src={originalAssetPath(sprite.hair.path)}
          alt=""
          draggable={false}
          data-mir2-original-src={originalAssetPath(sprite.hair.path)}
          onError={handleSceneAssetImageError}
          onLoad={handleSceneAssetImageLoad}
          style={{
            left: sprite.hair.x,
            top: sprite.hair.y,
            width: sprite.hair.width,
            height: sprite.hair.height,
          }}
        />
      ) : null}
      {!useBevyEntityRenderer &&
        sprite?.frontWeapons.map((weapon, index) => (
          <img
            key={`front-${objectId}-${index}-${weapon.path}`}
            className="entity-sprite-layer weapon front"
            src={originalAssetPath(weapon.path)}
            alt=""
            draggable={false}
            data-mir2-original-src={originalAssetPath(weapon.path)}
            onError={handleSceneAssetImageError}
            onLoad={handleSceneAssetImageLoad}
            style={{ left: weapon.x, top: weapon.y, width: weapon.width, height: weapon.height }}
          />
        ))}
      {sprite?.effect ? (
        <>
          <img
            className="entity-sprite-layer action-effect"
            src={sprite.effect.path}
            alt=""
            aria-hidden="true"
            draggable={false}
            data-mir2-original-src={sprite.effect.path}
            data-effect-blend={sprite.effect.blend ? "additive" : "alpha"}
            onError={handleSceneAssetImageError}
            onLoad={handleSceneAssetImageLoad}
            style={{
              left: sprite.effect.x,
              top: sprite.effect.y,
              width: sprite.effect.width,
              height: sprite.effect.height,
              mixBlendMode: sprite.effect.blend
                ? CRYSTAL_ADDITIVE_MIX_BLEND_MODE
                : "normal",
              pointerEvents: "none",
            }}
          />
          {sprite.effect.maskPath ? (
            <img
              className="entity-sprite-layer action-effect mask"
              src={sprite.effect.maskPath}
              alt=""
              aria-hidden="true"
              draggable={false}
              data-mir2-original-src={sprite.effect.maskPath}
              onError={handleSceneAssetImageError}
              onLoad={handleSceneAssetImageLoad}
              style={{
                left: sprite.effect.x,
                top: sprite.effect.y,
                width: sprite.effect.width,
                height: sprite.effect.height,
                mixBlendMode: CRYSTAL_ADDITIVE_MIX_BLEND_MODE,
                pointerEvents: "none",
              }}
            />
          ) : null}
        </>
      ) : null}
    </>
  );
});

// --- Procedural magic / map VFX fallback ----------------------------------
// The real effect atlases (lib/crystal-magic-effects) are PREFERRED: loadEffectAssets fetches the
// exported manifest once, and resolveSpell/MapEffect take priority for any effect it can resolve.
// Until those atlases exist the loader yields an empty set, the resolver returns null, and we draw
// the data-driven CSS fallback (lib/vfx-fallback) instead — so casting / skills are visibly
// reactive rather than inert. The loader is memoised at module scope so it never refetches.
let effectAssetsPromise: Promise<EffectAssets> | null = null;
const decodedSceneEffectFrameUrls = new Set<string>();
const sceneEffectFrameDecodePromises = new Map<string, Promise<boolean>>();
const SCENE_EFFECT_FRAME_DECODE_CACHE_LIMIT = 256;

function decodeSceneEffectFrame(url: string): Promise<boolean> {
  if (decodedSceneEffectFrameUrls.has(url)) return Promise.resolve(true);
  const cached = sceneEffectFrameDecodePromises.get(url);
  if (cached) return cached;

  const promise = new Promise<boolean>((resolve) => {
    const image = new Image();
    let settled = false;
    const finish = (loaded: boolean) => {
      if (settled) return;
      settled = true;
      if (loaded) decodedSceneEffectFrameUrls.add(url);
      resolve(loaded);
    };
    const finishLoaded = () => {
      if (typeof image.decode !== "function") {
        finish(image.naturalWidth > 0);
        return;
      }
      void image
        .decode()
        .then(() => finish(image.naturalWidth > 0))
        .catch(() => finish(image.naturalWidth > 0));
    };
    image.onload = finishLoaded;
    image.onerror = () => finish(false);
    image.decoding = "async";
    image.src = url;
    if (image.complete) finishLoaded();
  });

  sceneEffectFrameDecodePromises.set(url, promise);
  void promise.then((loaded) => {
    if (!loaded) sceneEffectFrameDecodePromises.delete(url);
  });
  while (sceneEffectFrameDecodePromises.size > SCENE_EFFECT_FRAME_DECODE_CACHE_LIMIT) {
    const oldest = sceneEffectFrameDecodePromises.keys().next().value as string | undefined;
    if (!oldest) break;
    sceneEffectFrameDecodePromises.delete(oldest);
  }
  return promise;
}

function loadEffectAssetsOnce(): Promise<EffectAssets> {
  if (!effectAssetsPromise) {
    effectAssetsPromise = loadEffectAssets().catch(
      () =>
        ({
          available: new Set<string>(),
          libraries: new Map(),
          spellByName: new Map(),
          mapByName: new Map(),
          groundBySpell: new Map(),
          effectNameByNumber: new Map(),
        }) as EffectAssets,
    );
  }
  return effectAssetsPromise;
}

function collectViewportMapLights(
  world: DisplayWorld,
  player: DisplayEntity | null,
  cameraOffset: ViewportOffset,
  viewportLayout: ViewportLayout = DEFAULT_VIEWPORT_LAYOUT,
): ViewportMapLight[] {
  if (!player || !world.originalMapRegion) return [];

  const {
    entityLeftOrigin: VIEWPORT_ENTITY_LEFT_ORIGIN,
    entityTopOrigin: VIEWPORT_ENTITY_TOP_ORIGIN,
    rangeX: VIEWPORT_RANGE_X,
    rangeY: VIEWPORT_RANGE_Y,
  } = viewportLayout;

  const lights: ViewportMapLight[] = [];
  const maxDx = VIEWPORT_RANGE_X + 24;
  const maxDy = VIEWPORT_RANGE_Y + 24;

  for (const cell of world.originalMapRegion.cells) {
    const lightValue = typeof cell.light === "number" && Number.isFinite(cell.light) ? Math.trunc(cell.light) : 0;
    const spec = crystalMapLightSpec(lightValue);
    if (!spec) continue;

    const dx = cell.x - player.x;
    const dy = cell.y - player.y;
    if (Math.abs(dx) > maxDx || Math.abs(dy) > maxDy) continue;

    const position = crystalMapLightTopLeft(
      VIEWPORT_ENTITY_LEFT_ORIGIN + dx * VIEWPORT_CELL_WIDTH + cameraOffset.x,
      VIEWPORT_ENTITY_TOP_ORIGIN + dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y,
      cell.lightOffsetX ?? 0,
      cell.lightOffsetY ?? 0,
      spec,
      VIEWPORT_CELL_WIDTH,
      VIEWPORT_CELL_HEIGHT,
    );

    lights.push({
      key: `map-light-${cell.x}-${cell.y}-${lightValue}`,
      value: lightValue,
      range: spec.range,
      left: position.left,
      top: position.top,
      width: spec.width,
      height: spec.height,
      tone: spec.tone,
      opacity: spec.opacity,
    });

  }

  return lights;
}

function useEffectAssets(enabled: boolean): EffectAssets | null {
  const [assets, setAssets] = useState<EffectAssets | null>(null);
  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    void loadEffectAssetsOnce().then((value) => {
      if (!cancelled) {
        setAssets(value);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [enabled]);
  return assets;
}

// Renders one procedural fallback effect as an inline-styled div (no globals.css changes). Returns
// null once the effect has expired so finished effects drop out of the tree automatically.
function FallbackVfxNode({
  effect,
  now,
  cameraOffset,
  viewportDepthPlayer,
  viewportLayout = DEFAULT_VIEWPORT_LAYOUT,
}: {
  effect: FallbackVfx;
  now: number;
  cameraOffset: ViewportOffset;
  viewportDepthPlayer: Pick<DisplayEntity, "x" | "y">;
  viewportLayout?: ViewportLayout;
}) {
  const {
    tileCenterX: VIEWPORT_TILE_CENTER_X,
    tileCenterY: VIEWPORT_TILE_CENTER_Y,
  } = viewportLayout;
  const style = fallbackVfxStyle(effect, now);
  if (!style) {
    return null;
  }
  const palette = paletteForElement(effect.element);
  const zIndex = viewportDepthForCell(effect.worldX, effect.worldY, viewportDepthPlayer, 90);

  if (effect.kind === "streak") {
    // Coloured trail along the projectile path (origin -> target), drawn as a thin rotated bar.
    const fromX = VIEWPORT_TILE_CENTER_X + effect.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x;
    const fromY = VIEWPORT_TILE_CENTER_Y + effect.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y - 28;
    const toX = VIEWPORT_TILE_CENTER_X + effect.toDx * VIEWPORT_CELL_WIDTH + cameraOffset.x;
    const toY = VIEWPORT_TILE_CENTER_Y + effect.toDy * VIEWPORT_CELL_HEIGHT + cameraOffset.y - 28;
    const dxPx = toX - fromX;
    const dyPx = toY - fromY;
    const length = Math.max(Math.hypot(dxPx, dyPx) * style.progress, 2);
    const angle = Math.atan2(dyPx, dxPx);
    return (
      <div
        aria-hidden
        style={{
          position: "absolute",
          left: fromX,
          top: fromY,
          width: `${length}px`,
          height: "4px",
          transformOrigin: "0 50%",
          transform: `rotate(${angle}rad) translateY(-50%)`,
          background: `linear-gradient(90deg, transparent, ${palette.glow}, ${palette.core})`,
          borderRadius: "2px",
          opacity: style.opacity,
          filter: "blur(0.5px)",
          pointerEvents: "none",
          zIndex,
        }}
      />
    );
  }

  // cast / impact / aura: a glowing ring/burst centred on the tile.
  const left = VIEWPORT_TILE_CENTER_X + effect.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x;
  const baseSize = effect.kind === "aura" ? 56 : effect.kind === "impact" ? 44 : 38;
  const top =
    VIEWPORT_TILE_CENTER_Y +
    effect.dy * VIEWPORT_CELL_HEIGHT +
    cameraOffset.y +
    (effect.kind === "aura" ? 6 : -18);
  const size = baseSize * style.scale;
  const ring = effect.kind === "aura";
  return (
    <div
      aria-hidden
      style={{
        position: "absolute",
        left,
        top,
        width: `${size}px`,
        height: `${effect.kind === "aura" ? size * 0.6 : size}px`,
        transform: "translate(-50%, -50%)",
        borderRadius: "50%",
        background: ring
          ? `radial-gradient(closest-side, transparent 55%, ${palette.glow} 75%, transparent)`
          : `radial-gradient(closest-side, ${palette.core}, ${palette.glow} 60%, transparent)`,
        border: ring ? `2px solid ${palette.core}` : undefined,
        opacity: style.opacity,
        mixBlendMode: "screen",
        pointerEvents: "none",
        zIndex,
      }}
    />
  );
}

function OriginalClientSceneVisualLayersInner({
  screen,
  t,
  world,
  player,
  selectedEntity,
  viewportGroundDrops,
  viewportMapSprites,
  viewportEntitySprites,
  viewportProjectiles,
  viewportDepthPlayer,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  imperativeCamera,
  registerCameraSurface,
  registerEntityEl,
  sceneSpriteFrameIndex,
  useBevyEntityRenderer,
  entityKindClassName,
  onPickGroundDrop,
  onActivateEntity,
  viewportLayout,
}: {
  screen: ClientScreen;
  t: TranslateFn;
  world: DisplayWorld;
  player: DisplayEntity | null;
  selectedEntity: DisplayEntity | null;
  viewportGroundDrops: ViewportGroundDrop[];
  viewportMapSprites: ViewportMapSprites;
  viewportEntitySprites: ViewportEntitySpriteEntry[];
  viewportProjectiles: ViewportProjectile[];
  viewportDepthPlayer: Pick<DisplayEntity, "x" | "y">;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  imperativeCamera: boolean;
  registerCameraSurface: (key: string) => (el: HTMLElement | null) => void;
  registerEntityEl: (key: string, objectId: string) => (el: HTMLElement | null) => void;
  sceneSpriteFrameIndex: number;
  useBevyEntityRenderer: boolean;
  entityKindClassName: (kind: EntityKind) => string;
  onPickGroundDrop: (objectId: string) => void;
  onActivateEntity: (objectId: string) => void;
  viewportLayout: ViewportLayout;
}) {
  const {
    entityLeftOrigin: VIEWPORT_ENTITY_LEFT_ORIGIN,
    entityTopOrigin: VIEWPORT_ENTITY_TOP_ORIGIN,
    rangeX: VIEWPORT_RANGE_X,
    rangeY: VIEWPORT_RANGE_Y,
    tileCenterX: VIEWPORT_TILE_CENTER_X,
    tileCenterY: VIEWPORT_TILE_CENTER_Y,
  } = viewportLayout;
  // --- Magic / map VFX fallback integration (single block) ---------------
  // Atlas-first: when loadEffectAssets resolves real frames, the collectors below skip the
  // procedural fallback for those effects. Until then (assets empty / null) we derive short-lived
  // CSS effects from the SAME viewport-delta data the projectiles use, so casting and skills are
  // visibly distinct instead of inert. collectViewportFallbackVfx returns [] on idle frames, so
  // this costs nothing when nothing is casting and no projectiles are live.
  const effectAssets = useEffectAssets(screen === "game");
  const resolvedEffectFrames = collectResolvedSceneEffectFrames(
    effectAssets,
    world.effects,
    motionNow,
  );
  const persistentEffectAssetUrls = Array.from(
    new Set(
      resolvedEffectFrames.flatMap(({ effect, animation }) =>
        effect.source === "spell" ? [] : sceneEffectAnimationAssetUrls(animation),
      ),
    ),
  );
  const persistentEffectAssetKey = persistentEffectAssetUrls.slice().sort().join("\n");
  const [readyPersistentEffectAssetKey, setReadyPersistentEffectAssetKey] = useState("");
  useEffect(() => {
    if (!persistentEffectAssetKey) {
      setReadyPersistentEffectAssetKey("");
      return;
    }
    if (persistentEffectAssetUrls.every((url) => decodedSceneEffectFrameUrls.has(url))) {
      setReadyPersistentEffectAssetKey(persistentEffectAssetKey);
      return;
    }

    let cancelled = false;
    void Promise.all(persistentEffectAssetUrls.map(decodeSceneEffectFrame)).then(() => {
      if (!cancelled) setReadyPersistentEffectAssetKey(persistentEffectAssetKey);
    });
    return () => {
      cancelled = true;
    };
    // The sorted key is a complete value signature for the captured URL list.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [persistentEffectAssetKey]);
  const persistentEffectsReady =
    !persistentEffectAssetKey || readyPersistentEffectAssetKey === persistentEffectAssetKey;
  const displayResolvedEffectFrames = persistentEffectsReady
    ? resolvedEffectFrames
    : resolvedEffectFrames.filter(({ effect }) => effect.source === "spell");
  const spellByCaster = new Map<string, string>();
  for (const resolved of resolvedEffectFrames) {
    if (resolved.effect.source === "spell" && resolved.effect.objectId) {
      spellByCaster.set(resolved.effect.objectId, resolved.animation.name);
    }
  }
  for (const projectile of viewportProjectiles) {
    const resolved = resolvedEffectFrames.find(
      (entry) =>
        entry.effect.source === "spell" &&
        entry.effect.objectId === projectile.attackerId &&
        Math.abs(entry.effect.startedAt - projectile.startedAt) <= 1_000,
    );
    if (resolved) spellByCaster.set(projectile.key, resolved.animation.name);
  }
  // Ground-drop item icons resolve from /original-ui/Items/{icon}.png (same pipeline as the bag).
  // Any icon index whose PNG fails to load (stale R2 / unmapped item) falls back to the dot marker.
  const [failedDropIcons, setFailedDropIcons] = useState<ReadonlySet<number>>(() => new Set());
  const fallbackVfx = collectViewportFallbackVfx(
    {
      entities: viewportEntitySprites.map(({ entity }) => ({
        objectId: entity.objectId,
        x: entity.x,
        y: entity.y,
        dx: entity.dx,
        dy: entity.dy,
        attackAnimation: entity.attackAnimation,
        attackStartedAt: entity.attackStartedAt,
      })),
      projectiles: viewportProjectiles,
    },
    {
      now: motionNow,
      assets: effectAssets,
      spellByCaster,
      rendererOwnsEffect: (instance) => {
        if (instance.kind === "cast") {
          return resolvedEffectFrames.some(
            (entry) =>
              entry.effect.source === "spell" &&
              entry.effect.objectId === instance.entity.objectId &&
              entry.animation.name === instance.spell,
          );
        }
        if (instance.kind === "projectile") {
          return spellByCaster.has(instance.projectile.key);
        }
        return false;
      },
    },
  );
  const sceneLightClassName = crystalSceneLightClassName(world.lightSetting);
  const viewportMapLights = sceneLightClassName
    ? collectViewportMapLights(world, player, playerCameraMotionOffset, viewportLayout)
    : [];
  const viewportObjectLights = sceneLightClassName
    ? viewportEntitySprites.flatMap(({ entity }) => {
        const spec = crystalObjectLightSpec(entity, entity.objectId === player?.objectId);
        return spec ? [{ entity, spec }] : [];
      })
    : [];

  return (
    <>
      <div
        ref={imperativeCamera ? registerCameraSurface("drops") : undefined}
        className={`viewport-drop-overlay ${screen !== "game" ? "hidden" : ""}`}
      >
        {viewportGroundDrops.map((drop) => {
          const dropsOnTile = viewportGroundDrops.filter(
            (candidate) => candidate.x === drop.x && candidate.y === drop.y,
          );
          const dropStackIndex = Math.max(
            0,
            dropsOnTile.findIndex((candidate) => candidate.objectId === drop.objectId),
          );
          const dropStackColumns = Math.min(3, dropsOnTile.length);
          const dropStackColumn = dropStackIndex % 3;
          const dropStackRow = Math.floor(dropStackIndex / 3);
          // Crystal can place several independent drops on one world tile. A
          // literal overlap makes every marker except the topmost one
          // impossible to click with a real pointer, so fan the labels into a
          // compact three-column stack while keeping them anchored to the
          // authoritative tile.
          const dropStackOffsetX =
            (dropStackColumn - (dropStackColumns - 1) / 2) * 76;
          const dropStackOffsetY = dropStackRow * 28;
          const showIcon =
            typeof drop.icon === "number" && drop.icon > 0 && !failedDropIcons.has(drop.icon);
          return (
            <button
              key={`drop-${drop.objectId}`}
              type="button"
              className="ground-drop-marker"
              style={{
                left: `${VIEWPORT_TILE_CENTER_X + drop.dx * VIEWPORT_CELL_WIDTH + playerCameraMotionOffset.x + dropStackOffsetX}px`,
                top: `${VIEWPORT_TILE_CENTER_Y + drop.dy * VIEWPORT_CELL_HEIGHT + playerCameraMotionOffset.y - 12 + dropStackOffsetY}px`,
                zIndex: viewportDepthForCell(drop.x, drop.y, viewportDepthPlayer, 16),
              }}
              onClick={() => onPickGroundDrop(drop.objectId)}
              data-ui-interactive="true"
              data-object-id={drop.objectId}
              aria-label={`${drop.name} x${drop.quantity}`}
              title={`${drop.name} x${drop.quantity}`}
            >
              {showIcon ? (
                <img
                  className="drop-icon"
                  src={originalItemIconPath(drop.icon as number)}
                  alt=""
                  draggable={false}
                  onError={() =>
                    setFailedDropIcons((prev) => {
                      const next = new Set(prev);
                      next.add(drop.icon as number);
                      return next;
                    })
                  }
                />
              ) : (
                <span className="drop-dot" />
              )}
              <span className="drop-label" style={{ color: argbToCssColor(drop.nameColourArgb) }}>
                {drop.quantity > 1 ? `${drop.name} x${drop.quantity}` : drop.name}
              </span>
            </button>
          );
        })}
      </div>

      <div
        ref={imperativeCamera ? registerCameraSurface("sprites") : undefined}
        className={`viewport-sprite-overlay ${screen !== "game" ? "hidden" : ""}`}
      >
        {viewportMapSprites.objects.map((sprite) => {
          const blendMode = resolvedMapSpriteBlendMode(sprite);
          const renderPath = mapSpriteRenderPath(sprite.path);
          // Tall additive beams need a brighter curve than compact torch glows.
          const isBlendColumn = Boolean(blendMode && sprite.width <= 64 && sprite.height >= 180);
          return (
            <img
              key={sprite.key}
              className="scene-map-object-sprite"
              src={renderPath}
              alt=""
              draggable={false}
              data-map-sprite-path={sprite.path}
              data-map-render-path={renderPath}
              data-mir2-original-src={renderPath}
              data-map-cell-x={sprite.cellX}
              data-map-cell-y={sprite.cellY}
              onError={handleSceneAssetImageError}
              onLoad={handleSceneAssetImageLoad}
              style={{
                left: sprite.left + playerCameraMotionOffset.x,
                top: sprite.top + playerCameraMotionOffset.y,
                width: sprite.width,
                height: sprite.height,
                mixBlendMode: blendMode,
                opacity: blendMode ? (isBlendColumn ? 1 : 0.78) : undefined,
                filter: blendMode
                  ? isBlendColumn
                    ? "brightness(2.35) saturate(1.08)"
                    : "brightness(2.25) saturate(0.72)"
                  : undefined,
                zIndex: sprite.zIndex,
              }}
            />
          );
        })}
        {viewportEntitySprites.map(({ entity, sprite }) => {
          const isPlayer = player?.objectId === entity.objectId;
          const isInteractiveEntity = !isPlayer;
          // In the imperative path the sub-tile glide is written by the motion driver
          // (display Hz) onto this stack's transform; render at the cell base here.
          const entityMotionOffset =
            isPlayer || imperativeCamera
              ? EMPTY_VIEWPORT_OFFSET
              : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
          const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
          const label = entityDisplayName(entity);
          const hitBounds = entitySpriteHitBounds(sprite);
          const hitWidth = hitBounds.right - hitBounds.left;
          const hitHeight = hitBounds.bottom - hitBounds.top;
          const handleEntityPointerActivate = (event: MouseEvent<HTMLElement>) => {
            if (event.button !== 0 && event.button !== 2) {
              return;
            }
            event.preventDefault();
            event.stopPropagation();
            onActivateEntity(entity.objectId);
          };
          const handleEntityContextActivate = (event: MouseEvent<HTMLElement>) => {
            event.preventDefault();
            event.stopPropagation();
            onActivateEntity(entity.objectId);
          };
          return (
            <div
              key={`sprite-${entity.objectId}`}
              ref={imperativeCamera ? registerEntityEl(`stack:${entity.objectId}`, entity.objectId) : undefined}
              className={`entity-sprite-stack ${entityKindClassName(entity.kind)} ${entity.objectId === selectedEntity?.objectId ? "selected" : ""} ${entity.dead ? "dead" : ""} ${isEntityAttacking(entity, motionNow) ? "attacking" : ""} ${isEntityStruck(entity, motionNow) ? "struck" : ""} ${isEntityReviving(entity, motionNow) ? "reviving" : ""}`}
              style={{
                left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x}px`,
                top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y}px`,
                zIndex: viewportDepthForCell(entity.x, entity.y, viewportDepthPlayer, 64),
              }}
              data-ui-interactive={isInteractiveEntity ? "true" : "false"}
              data-object-id={entity.objectId}
              onMouseDown={isInteractiveEntity ? handleEntityPointerActivate : undefined}
              onContextMenu={isInteractiveEntity ? handleEntityContextActivate : undefined}
            >
              {isInteractiveEntity ? (
                <button
                  type="button"
                  className="entity-sprite-hit"
                  style={{
                    left: `${hitBounds.left}px`,
                    top: `${hitBounds.top}px`,
                    width: `${hitWidth}px`,
                    height: `${hitHeight}px`,
                  }}
                  aria-label={label}
                  onMouseDown={handleEntityPointerActivate}
                  onContextMenu={handleEntityContextActivate}
                  onClick={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                  }}
                />
              ) : null}
              <EntitySpriteLayers
                useBevyEntityRenderer={useBevyEntityRenderer}
                sprite={sprite}
                objectId={entity.objectId}
              />
            </div>
          );
        })}
        {viewportProjectiles.map((projectile) => {
          const currentLeft =
            VIEWPORT_TILE_CENTER_X +
            (projectile.fromDx + (projectile.toDx - projectile.fromDx) * projectile.progress) * VIEWPORT_CELL_WIDTH +
            playerCameraMotionOffset.x;
          const currentTop =
            VIEWPORT_TILE_CENTER_Y +
            (projectile.fromDy + (projectile.toDy - projectile.fromDy) * projectile.progress) * VIEWPORT_CELL_HEIGHT +
            playerCameraMotionOffset.y -
            28;
          const deltaX = (projectile.toDx - projectile.fromDx) * VIEWPORT_CELL_WIDTH;
          const deltaY = (projectile.toDy - projectile.fromDy) * VIEWPORT_CELL_HEIGHT;
          const angle = Math.atan2(deltaY, deltaX);

          return (
            <div
              key={projectile.key}
              className="viewport-projectile"
              style={{
                left: currentLeft,
                top: currentTop,
                transform: `translate(-50%, -50%) rotate(${angle}rad)`,
                zIndex: viewportDepthForCell(projectile.toX, projectile.toY, viewportDepthPlayer, 80),
              }}
            />
          );
        })}
        {/* Procedural magic / map VFX fallback nodes (atlas-free; see FallbackVfxNode). */}
        {fallbackVfx.map((effect) => (
          <FallbackVfxNode
            key={effect.key}
            effect={effect}
            now={motionNow}
            cameraOffset={playerCameraMotionOffset}
            viewportDepthPlayer={viewportDepthPlayer}
            viewportLayout={viewportLayout}
          />
        ))}
      </div>

      <div
        className={`viewport-effect-overlay ${screen !== "game" ? "hidden" : ""}`}
        aria-hidden="true"
      >
        {displayResolvedEffectFrames.map(({ effect, animation, frame }) => {
          const anchor = effect.objectId
            ? viewportEntitySprites.find(({ entity }) => entity.objectId === effect.objectId)?.entity
            : undefined;
          const isPlayerAnchor = anchor?.objectId === player?.objectId;
          const anchorMotionOffset =
            anchor && !isPlayerAnchor && !imperativeCamera
              ? entityMotionOffsetForEntity(anchor, entityMotionSnapshots, motionNow)
              : EMPTY_VIEWPORT_OFFSET;
          const cameraOffset = isPlayerAnchor ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
          const worldX = anchor?.x ?? effect.x;
          const worldY = anchor?.y ?? effect.y;
          const dx = anchor?.dx ?? worldX - (player?.x ?? worldX);
          const dy = anchor?.dy ?? worldY - (player?.y ?? worldY);
          return (
            <Fragment key={effect.key}>
              <img
                ref={
                  imperativeCamera
                    ? registerCameraSurface(`effect:${effect.key}`)
                    : undefined
                }
                className="scene-crystal-effect-frame"
                src={frame.path}
                alt=""
                aria-hidden="true"
                draggable={false}
                data-effect-key={effect.key}
                data-effect-source={effect.source}
                data-effect-name={animation.name}
                data-effect-blend={animation.blend ? "additive" : "alpha"}
                data-mir2-original-src={frame.path}
                onError={handleSceneAssetImageError}
                onLoad={handleSceneAssetImageLoad}
                style={{
                  position: "absolute",
                  left:
                    // Crystal passes the tile's top-left DrawLocation to MLibrary;
                    // the exported frame x/y already contains the library offset.
                    VIEWPORT_ENTITY_LEFT_ORIGIN +
                    dx * VIEWPORT_CELL_WIDTH +
                    cameraOffset.x +
                    anchorMotionOffset.x +
                    animation.offset.x +
                    frame.x,
                  top:
                    VIEWPORT_ENTITY_TOP_ORIGIN +
                    dy * VIEWPORT_CELL_HEIGHT +
                    cameraOffset.y +
                    anchorMotionOffset.y +
                    animation.offset.y +
                    frame.y,
                  width: frame.width,
                  height: frame.height,
                  mixBlendMode: animation.blend
                    ? CRYSTAL_ADDITIVE_MIX_BLEND_MODE
                    : "normal",
                  filter:
                    animation.light > 0
                      ? `drop-shadow(0 0 ${Math.min(animation.light * 2, 16)}px #fff)`
                      : undefined,
                  pointerEvents: "none",
                  zIndex: viewportDepthForCell(
                    worldX,
                    worldY,
                    viewportDepthPlayer,
                    crystalSceneEffectLayerOffset(effect.source),
                  ),
                }}
              />
              {frame.maskPath ? (
                <img
                  ref={
                    imperativeCamera
                      ? registerCameraSurface(`effect:${effect.key}:mask`)
                      : undefined
                  }
                  className="scene-crystal-effect-frame mask"
                  src={frame.maskPath}
                  alt=""
                  aria-hidden="true"
                  draggable={false}
                  data-effect-key={`${effect.key}:mask`}
                  data-mir2-original-src={frame.maskPath}
                  onError={handleSceneAssetImageError}
                  onLoad={handleSceneAssetImageLoad}
                  style={{
                    position: "absolute",
                    left:
                      VIEWPORT_ENTITY_LEFT_ORIGIN +
                      dx * VIEWPORT_CELL_WIDTH +
                      cameraOffset.x +
                      anchorMotionOffset.x +
                      animation.offset.x +
                      (frame.maskX ?? frame.x),
                    top:
                      VIEWPORT_ENTITY_TOP_ORIGIN +
                      dy * VIEWPORT_CELL_HEIGHT +
                      cameraOffset.y +
                      anchorMotionOffset.y +
                      animation.offset.y +
                      (frame.maskY ?? frame.y),
                    width: frame.maskWidth ?? frame.width,
                    height: frame.maskHeight ?? frame.height,
                    mixBlendMode: CRYSTAL_ADDITIVE_MIX_BLEND_MODE,
                    pointerEvents: "none",
                    zIndex: viewportDepthForCell(
                      worldX,
                      worldY,
                      viewportDepthPlayer,
                      crystalSceneEffectLayerOffset(effect.source, true),
                    ),
                  }}
                />
              ) : null}
            </Fragment>
          );
        })}
      </div>

      {screen === "game" ? <OriginalClientWeatherLayer weatherParticles={world.weatherParticles} /> : null}

      {screen === "game" && sceneLightClassName ? (
        <div
          aria-hidden="true"
          className={`viewport-crystal-light-overlay ${sceneLightClassName}`}
          data-light-setting={world.lightSetting ?? ""}
          data-map-dark-light={world.mapDarkLight ?? 0}
          style={{ background: crystalSceneDarknessColor(world.lightSetting, world.mapDarkLight) ?? undefined }}
        >
          <div
            ref={imperativeCamera ? registerCameraSurface("map-lights") : undefined}
            className="viewport-map-light-surface"
          >
            {viewportMapLights.map((light) => (
              <img
                key={light.key}
                className={`viewport-map-light ${light.tone}`}
                src={crystalLightTexturePath(light.range)}
                alt=""
                draggable={false}
                data-light-value={light.value}
                style={{
                  left: light.left,
                  top: light.top,
                  width: light.width,
                  height: light.height,
                  opacity: light.opacity,
                }}
              />
            ))}
            {viewportObjectLights.map(({ entity, spec }) => {
              const isPlayer = entity.objectId === player?.objectId;
              const entityMotionOffset =
                isPlayer || imperativeCamera
                  ? EMPTY_VIEWPORT_OFFSET
                  : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
              const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
              const drawX =
                VIEWPORT_ENTITY_LEFT_ORIGIN +
                entity.dx * VIEWPORT_CELL_WIDTH +
                cameraOffset.x +
                entityMotionOffset.x;
              const drawY =
                VIEWPORT_ENTITY_TOP_ORIGIN +
                entity.dy * VIEWPORT_CELL_HEIGHT +
                cameraOffset.y +
                entityMotionOffset.y;
              const position = crystalObjectLightTopLeft(
                drawX,
                drawY,
                spec,
                VIEWPORT_CELL_WIDTH,
                VIEWPORT_CELL_HEIGHT,
              );

              return (
                <img
                  key={`object-light-${entity.objectId}`}
                  ref={
                    imperativeCamera
                      ? registerEntityEl(`light:${entity.objectId}`, entity.objectId)
                      : undefined
                  }
                  className={`viewport-object-light ${spec.tone}`}
                  src={crystalLightTexturePath(spec.range)}
                  alt=""
                  draggable={false}
                  data-object-id={entity.objectId}
                  data-light-value={spec.value}
                  data-light-range={spec.range}
                  data-light-strength-bucket={spec.strengthBucket}
                  style={{
                    left: position.left,
                    top: position.top,
                    width: spec.width,
                    height: spec.height,
                    opacity: spec.opacity,
                  }}
                />
              );
            })}
          </div>
        </div>
      ) : null}

      <div
        ref={imperativeCamera ? registerCameraSurface("names") : undefined}
        className={`viewport-entity-overlay ${screen !== "game" ? "hidden" : ""}`}
      >
        {player
          ? viewportEntitySprites.map(({ entity, animationPose, sprite }) => {
              const isPlayer = player.objectId === entity.objectId;
              const isInteractiveEntity = !isPlayer;
              // Imperative path: the driver writes the sub-tile glide onto this
              // nameplate's transform (display Hz); render at the cell base here.
              const entityMotionOffset =
                isPlayer || imperativeCamera
                  ? EMPTY_VIEWPORT_OFFSET
                  : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
              const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
              const labelLines = entityDisplayLabelLines(entity);
              const labelText = labelLines.map((line) => line.text).join("\r\n");
              const labelColour = entityNameplateColor(entity);
              const questIcon =
                entity.kind === "npc"
                  ? questIconForEntity(entity, world.questLog, sceneSpriteFrameIndex)
                  : null;
              const entityGdiText = entity.dead
                ? null
                : findCrystalGdiTextAsset({
                    text: labelText,
                    foreground: labelColour,
                    outline: true,
                    width: labelLines.length > 1 ? 54 : 50,
                    height: labelLines.length > 1 ? 32 : 15,
                  }) ??
                  findCrystalGdiTextAsset({
                    text: entity.name.replace(/_/g, " "),
                    foreground: labelColour,
                    outline: true,
                    width: 50,
                    height: 15,
                  });
              const healthRatio =
                isPlayer && entity.hp !== undefined && entity.maxHp
                  ? ratio(entity.hp, entity.maxHp)
                  : null;

              return (
                <Fragment key={`entity-overlay-${entity.objectId}`}>
                  {questIcon ? (
                    <img
                      ref={
                        imperativeCamera
                          ? registerEntityEl(`quest:${entity.objectId}`, entity.objectId)
                          : undefined
                      }
                      className="entity-quest-icon"
                      src={questIcon}
                      alt=""
                      draggable={false}
                      data-object-id={entity.objectId}
                      data-mir2-original-src={questIcon}
                      onError={handleSceneAssetImageError}
                      onLoad={handleSceneAssetImageLoad}
                      onMouseDown={(event) => {
                        if (event.button === 0 || event.button === 2) {
                          event.preventDefault();
                          event.stopPropagation();
                          onActivateEntity(entity.objectId);
                        }
                      }}
                      onContextMenu={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        onActivateEntity(entity.objectId);
                      }}
                      style={{
                        left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x + entityQuestIconLeftOffset(entity, sprite)}px`,
                        top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y + entityQuestIconTopOffset(sprite)}px`,
                        zIndex: viewportDepthForCell(entity.x, entity.y, viewportDepthPlayer, 96),
                      }}
                    />
                  ) : null}
                  {healthRatio !== null ? (
                    <div
                      ref={
                        imperativeCamera
                          ? registerEntityEl(`health:${entity.objectId}`, entity.objectId)
                          : undefined
                      }
                      className="entity-health-bar entity-overlay-health-bar"
                      data-object-id={entity.objectId}
                      style={{
                        left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x + 8}px`,
                        top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y - 64}px`,
                      }}
                    >
                      <span style={{ width: `${healthRatio * 100}%` }} />
                    </div>
                  ) : null}
                  <button
                    ref={
                      imperativeCamera
                        ? registerEntityEl(`name:${entity.objectId}`, entity.objectId)
                        : undefined
                    }
                    type="button"
                    className={`entity-nameplate ${entityKindClassName(entity.kind)} ${entity.objectId === selectedEntity?.objectId ? "selected" : ""}`}
                    style={{
                      left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x + entityNameplateLeftOffset(entity, sprite)}px`,
                      top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y + entityNameplateTopOffset(entity, sprite)}px`,
                      "--entity-name-color": labelColour,
                    } as CSSProperties}
                    data-ui-interactive={isInteractiveEntity ? "true" : "false"}
                    data-object-id={entity.objectId}
                    data-animation-action={animationPose?.action}
                    data-animation-frame={animationPose?.logicalFrameIndex}
                    data-animation-incarnation={animationPose?.incarnation}
                    tabIndex={isInteractiveEntity ? undefined : -1}
                    onMouseDown={
                      isInteractiveEntity
                        ? (event) => {
                            if (event.button === 0 || event.button === 2) {
                              event.preventDefault();
                              event.stopPropagation();
                            }
                          }
                        : undefined
                    }
                    onClick={isInteractiveEntity ? () => onActivateEntity(entity.objectId) : undefined}
                    onContextMenu={
                      isInteractiveEntity
                        ? (event) => {
                            event.preventDefault();
                            event.stopPropagation();
                            onActivateEntity(entity.objectId);
                          }
                        : undefined
                    }
                  >
                    {entityGdiText ? (
                      <CrystalGdiTextImage
                        asset={entityGdiText}
                        className="entity-nameplate-gdi"
                        accessibleText={labelLines.map((line) => line.text).join(" ")}
                      />
                    ) : labelLines.map((line, index) => (
                        <strong
                          key={`${entity.objectId}-label-${index}`}
                          className={
                            line.role === "secondary" && entity.kind === "npc"
                              ? "entity-subname"
                              : undefined
                          }
                        >
                          {line.text}
                        </strong>
                      ))}
                    {entity.dead ? (
                      <strong className="entity-state-label">{t("ui.dead")}</strong>
                    ) : null}
                  </button>
                </Fragment>
              );
            })
          : null}
      </div>

      <div className={`viewport-vignette ${screen === "game" && viewportMapSprites.floor.length ? "hidden" : ""}`} />
    </>
  );
}

// Memoised so the 30Hz motion clock (shell motionNow tick) skips this whole DOM scene layer when
// nothing it reads actually changed. The shell already memoises every prop it passes, so a tick
// that only advances `motionNow` (no new world/sprite data) re-renders nothing here.
export const OriginalClientSceneVisualLayers = memo(OriginalClientSceneVisualLayersInner);

function entityDisplayName(entity: DisplayEntity): string {
  return entity.name;
}

function entityDisplayLabelLines(entity: DisplayEntity): Array<{ text: string; role: "primary" | "secondary" }> {
  if (entity.ownerName) {
    return [
      { text: entity.name, role: "primary" },
      { text: `${entity.ownerName}'s Hero`, role: "secondary" },
    ];
  }

  if (entity.kind !== "npc" && entity.kind !== "monster") {
    return [{ text: entity.name, role: "primary" }];
  }

  const parts = entity.name.split("_").filter(Boolean);
  if (parts.length <= 1) {
    return [{ text: entity.name.replace(/_/g, " "), role: "primary" }];
  }

  return parts.map((part, index) => ({ text: part, role: index === 0 ? "primary" : "secondary" }));
}
