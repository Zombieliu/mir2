"use client";

import type { R16EntrySpriteImages } from "../../lib/r16-entry-scene";
import { crystalEntityDepth } from "../../lib/crystal-world-depth";

import { Fragment, memo, useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";

import type { ClientScreen } from "../../lib/original-ui";
import { originalAssetPath } from "../../lib/asset-url";
import type { EffectAnimation, EffectAssets, EffectFrameMeta } from "../../lib/crystal-magic-effects";
import {
  loadEffectAssets,
  resolveMapEffect,
  resolveSpellImpactEffect,
  resolveSpellProjectileEffect,
  resolveSpellReturnEffect,
  spellNameForNumber,
} from "../../lib/crystal-magic-effects";
import { FISHING_BITE_NOTICE, FISHING_WATER_EFFECTS } from "../../lib/world-fishing-feedback";
import {
  collectResolvedSceneEffectFrames,
  CRYSTAL_ADDITIVE_MIX_BLEND_MODE,
  crystalSceneEffectLayerOffset,
  sceneEffectAnimationAssetUrls,
} from "../../lib/scene-effect-runtime";
import { r16ProjectileFrameInterval, r16TargetedSpell } from "../../lib/r16-targeted-projectiles";
import { originalItemIconPath } from "./original-client-inventory-utils";
import { CrystalGdiTextImage, findCrystalGdiTextAsset } from "./crystal-gdi-text";
import { localizeCrystalEntityName } from "../../lib/crystal-content-localization";
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
  DisplaySceneEffect,
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
import { DecodedSelfSprite, type ResidentActorImage, type ResidentActorImages } from "./original-client-self-sprite";

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

// R16 draws these transient effects after the complete world and before item labels.
const R16_TARGETED_EFFECT_DEPTH = 3_100_000;

function isTransientSceneEffect(source: DisplaySceneEffect["source"]) {
  return source === "spell" || source === "attackOverlay" || source === "fishingWater" || source === "fishingBite";
}

function projectilePhaseFrame(
  animation: EffectAnimation | null,
  startedAt: number,
  now: number,
  loop: boolean,
  interval?: number,
): EffectFrameMeta | null {
  if (!animation || now < startedAt || animation.frames.length === 0) return null;
  const rawIndex = Math.floor((now - startedAt) / Math.max(interval ?? animation.interval, 1));
  const index = loop ? rawIndex % animation.frames.length : rawIndex;
  return index >= 0 && index < animation.frames.length ? animation.frames[index] ?? null : null;
}

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
  packedActors: R16EntrySpriteImages | null;
  useBevyEntityRenderer: boolean;
  effectsEnabled: boolean;
  sprite: ViewportEntitySprite | null;
  objectId: number | string;
  residentImages?: ResidentActorImages;
  onResidentError?: (key: string, url: string) => void;
};

// The body/hair/weapon <img> layers for one actor. Memoised so they only re-render when their
// *sprite frame data* actually changes (direction / animation / the 120ms tick) — NOT on every
// 60fps motion-clock tick. The smooth per-frame position lives on the parent
// `.entity-sprite-stack` wrapper, so these inner layers stay byte-identical between frame changes.
// That removes the per-frame restyle/reconcile churn that made running janky and standing NPCs
// (a single static frame, redrawn 60×/sec for nothing) visibly flicker.
const EntitySpriteLayers = memo(function EntitySpriteLayers({
  useBevyEntityRenderer,
  effectsEnabled,
  sprite,
  objectId,
  packedActors, residentImages, onResidentError,
}: EntitySpriteLayersProps) {
  return (
    <>
      {!useBevyEntityRenderer && sprite?.mount ? <ActorSpriteImage role="mount" layer={sprite.mount}
        className="entity-sprite-layer mount" residentImages={residentImages} onResidentError={onResidentError} /> : null}
      {!useBevyEntityRenderer && sprite?.rearWeapons.map((weapon, index) => <ActorSpriteImage
        key={`rear-${objectId}-${index}-${weapon.path}`} role={"rear:" + index} layer={weapon}
        className="entity-sprite-layer weapon rear" residentImages={residentImages} onResidentError={onResidentError} />)}
      {!useBevyEntityRenderer && sprite?.body ? <ActorSpriteImage role="body" layer={sprite.body}
        className="entity-sprite-layer body" packedActors={packedActors} residentImages={residentImages} onResidentError={onResidentError} /> : null}
      {!useBevyEntityRenderer && sprite?.hair ? <ActorSpriteImage role="hair" layer={sprite.hair}
        className="entity-sprite-layer hair" packedActors={packedActors} residentImages={residentImages} onResidentError={onResidentError} /> : null}
      {!useBevyEntityRenderer && sprite?.frontWeapons.map((weapon, index) => <ActorSpriteImage
        key={`front-${objectId}-${index}-${weapon.path}`} role={"front:" + index} layer={weapon}
        className="entity-sprite-layer weapon front" residentImages={residentImages} onResidentError={onResidentError} />)}
      {effectsEnabled && sprite?.effect ? (
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

function ActorSpriteImage({ role, layer, className, packedActors, residentImages, onResidentError }: {
  role: string; layer: NonNullable<ViewportEntitySprite["body"]>; className: string;
  packedActors?: R16EntrySpriteImages | null; residentImages?: ResidentActorImages;
  onResidentError?: (key: string, url: string) => void;
}) {
  const resident = residentImages?.get(role);
  if (resident) return <ResidentActorSpriteImage layer={layer} className={className} resident={resident} onError={onResidentError} />;
  const source = packedActors?.[layer.path] ?? originalAssetPath(layer.path);
  return <img className={className} src={source} alt="" draggable={false}
    data-mir2-original-src={originalAssetPath(layer.path)}
    data-mir2-packed-asset={source.startsWith("data:") ? "true" : undefined}
    onError={handleSceneAssetImageError}
    onLoad={handleSceneAssetImageLoad}
    style={{ left: layer.x, top: layer.y, width: layer.width, height: layer.height }} />;
}

function ResidentActorSpriteImage({ layer, className, resident, onError }: {
  layer: NonNullable<ViewportEntitySprite["body"]>; className: string;
  resident: ResidentActorImage;
  onError?: (key: string, url: string) => void;
}) {
  const slot = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const container = slot.current, image = resident.image;
    if (!container || !image.complete || image.naturalWidth < 1) return;
    // Move the exact decoded element into this React-owned slot. Assigning its
    // URL to a different <img> can briefly clear the body's decoded/paint state.
    image.className = className; image.alt = ""; image.draggable = false;
    image.dataset.mir2OriginalSrc = originalAssetPath(layer.path);
    image.dataset.mir2ActorResident = "true";
    if (resident.url.startsWith("data:")) image.dataset.mir2PackedAsset = "true";
    else delete image.dataset.mir2PackedAsset;
    Object.assign(image.style, { left: layer.x + "px", top: layer.y + "px", width: layer.width + "px", height: layer.height + "px" });
    const failed = () => onError?.(resident.key, resident.url);
    image.addEventListener("error", failed);
    container.replaceChildren(image);
    // The native load occurred before attachment. Wake the existing body gate;
    // it still checks this actual element's complete/naturalWidth/visibility.
    image.dispatchEvent(new Event("load"));
    return () => { image.removeEventListener("error", failed); if (image.parentNode === container) image.remove(); };
  }, [resident.image, resident.key, resident.url, layer.path, layer.x, layer.y, layer.width, layer.height, className, onError]);
  return <span ref={slot} style={{ display: "contents" }} />;
}

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
  const zIndex = viewportDepthForCell(effect.worldX, effect.worldY, viewportDepthPlayer, 10);

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
  assetReleaseId = null,
  t,
  world,
  fishingEffects,
  warmFishingEffects,
  player,
  selectedEntity,
  viewportGroundDrops,
  viewportMapSprites,
  packedObjects,
  packedActors,
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
  nameView = true, dropView = true, effectsEnabled = true,
  hoveredObjectId = null, questTargetObjectIds,
  entityKindClassName,
  onPickGroundDrop,
  onActivateEntity,
  viewportLayout,
}: {
  screen: ClientScreen;
  assetReleaseId?: string | null;
  t: TranslateFn;
  world: DisplayWorld;
  fishingEffects: readonly DisplaySceneEffect[];
  warmFishingEffects: boolean;
  player: DisplayEntity | null;
  selectedEntity: DisplayEntity | null;
  viewportGroundDrops: ViewportGroundDrop[];
  viewportMapSprites: ViewportMapSprites;
  packedObjects: R16EntrySpriteImages | null;
  packedActors: R16EntrySpriteImages | null;
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
  nameView?: boolean; dropView?: boolean; hpView?: boolean; effectsEnabled?: boolean;
  /** Current world hover and Native tracker targets; selected entity is not hover authority. */
  hoveredObjectId?: string | null;
  questTargetObjectIds?: ReadonlySet<string>;
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
  useEffect(() => {
    if (!effectAssets || !warmFishingEffects || !effectsEnabled) return;
    for (const name of [...FISHING_WATER_EFFECTS.ripple, ...FISHING_WATER_EFFECTS.bite, FISHING_BITE_NOTICE]) {
      const animation = resolveMapEffect(effectAssets, name);
      if (animation) for (const url of sceneEffectAnimationAssetUrls(animation)) void decodeSceneEffectFrame(url);
    }
  }, [effectAssets, effectsEnabled, warmFishingEffects]);
  const resolvedEffectFrames = collectResolvedSceneEffectFrames(
    effectAssets,
    fishingEffects.length ? [...world.effects, ...fishingEffects] : world.effects,
    motionNow,
  );
  const persistentEffectAssetUrls = Array.from(
    new Set(
      resolvedEffectFrames.flatMap(({ effect, animation }) =>
        isTransientSceneEffect(effect.source)
          ? []
          : sceneEffectAnimationAssetUrls(animation),
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
    : resolvedEffectFrames.filter(
        ({ effect }) => isTransientSceneEffect(effect.source),
      );
  const spellByCaster = new Map<string, string>();
  const r16CastStarts = new Map<string, number>();
  for (const effect of world.effects) {
    if (effect.source === "spell" && effect.objectId && r16TargetedSpell(effect.spellOrEffect)) {
      r16CastStarts.set(effect.objectId, effect.startedAt);
    }
  }
  for (const resolved of resolvedEffectFrames) {
    if (resolved.effect.source === "spell" && resolved.effect.objectId) {
      spellByCaster.set(resolved.effect.objectId, resolved.animation.name);
    }
  }
  for (const projectile of viewportProjectiles) {
    const packetSpell = typeof projectile.spellOrEffect === "number"
      ? spellNameForNumber(projectile.spellOrEffect)
      : projectile.spellOrEffect;
    if (packetSpell) {
      spellByCaster.set(projectile.key, packetSpell);
      continue;
    }
    const resolved = resolvedEffectFrames.find(
      (entry) =>
        entry.effect.source === "spell" &&
        entry.effect.objectId === projectile.attackerId &&
        Math.abs(entry.effect.startedAt - projectile.startedAt) <= 1_000,
    );
    if (resolved) spellByCaster.set(projectile.key, resolved.animation.name);
  }
  const exactProjectileEffects = new Map<
    string,
    {
      spell: string;
      projectile: EffectAnimation | null;
      impact: EffectAnimation | null;
      returnEffect: EffectAnimation | null;
    }
  >();
  if (effectAssets) {
    for (const projectile of viewportProjectiles) {
      const spell = spellByCaster.get(projectile.key);
      if (!spell) continue;
      const targeted = projectile.r16Targeted;
      const direction = targeted?.phase === "projectile" ? targeted.direction : 0;
      const phases = {
        spell,
        projectile: resolveSpellProjectileEffect(effectAssets, spell, direction),
        impact: resolveSpellImpactEffect(effectAssets, spell),
        returnEffect: resolveSpellReturnEffect(effectAssets, spell),
      };
      if (phases.projectile || phases.impact || phases.returnEffect) {
        exactProjectileEffects.set(projectile.key, phases);
      }
    }
  }
  // Ground-drop item icons resolve from /original-ui/Items/{icon}.png (same pipeline as the bag).
  // Any icon index whose PNG fails to load (stale R2 / unmapped item) falls back to the dot marker.
  const [failedDropIcons, setFailedDropIcons] = useState<ReadonlySet<number>>(() => new Set());
  const fallbackVfx = collectViewportFallbackVfx(
    {
      entities: viewportEntitySprites
        .filter(({ entity }) => entity.attackStartedAt === undefined ||
          r16CastStarts.get(entity.objectId) !== entity.attackStartedAt)
        .map(({ entity }) => ({
          objectId: entity.objectId,
          x: entity.x,
          y: entity.y,
          dx: entity.dx,
          dy: entity.dy,
          attackAnimation: entity.attackAnimation,
          attackStartedAt: entity.attackStartedAt,
        })),
      projectiles: viewportProjectiles.filter((projectile) => !projectile.r16Targeted),
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
          return exactProjectileEffects.has(instance.projectile.key);
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
                zIndex: viewportDepthForCell(drop.x, drop.y, viewportDepthPlayer, 1),
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
              <span className="drop-label" style={{ color: argbToCssColor(drop.nameColourArgb), visibility: dropView ? "visible" : "hidden" }}>
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
          const packedPath = packedObjects?.[renderPath];
          // Tall additive beams need a brighter curve than compact torch glows.
          const isBlendColumn = Boolean(blendMode && sprite.width <= 64 && sprite.height >= 180);
          return (
            <img
              key={packedPath ? sprite.key + ":packed" : sprite.key}
              className="scene-map-object-sprite"
              src={packedPath ?? renderPath}
              data-mir2-packed-asset={packedPath ? "true" : undefined}
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
          const entityDepth = crystalEntityDepth(
            entity, viewportDepthPlayer, entityMotionSnapshots[entity.objectId], motionNow,
          );
          const isInteractiveEntity = !isPlayer;
          // In the imperative path the sub-tile glide is written by the motion driver
          // (display Hz) onto this stack's transform; render at the cell base here.
          const entityMotionOffset =
            isPlayer || imperativeCamera
              ? EMPTY_VIEWPORT_OFFSET
              : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
          const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
          const label = entityDisplayName(entity, t);
          const hitBounds = entitySpriteHitBounds(sprite);
          const hitWidth = hitBounds.right - hitBounds.left;
          const hitHeight = hitBounds.bottom - hitBounds.top;
          return (
            <div
              key={`sprite-${entity.objectId}`}
              ref={imperativeCamera ? registerEntityEl(`stack:${entity.objectId}`, entity.objectId) : undefined}
              className={`entity-sprite-stack ${entityKindClassName(entity.kind)} ${entity.objectId === selectedEntity?.objectId ? "selected" : ""} ${entity.dead ? "dead" : ""} ${isEntityAttacking(entity, motionNow) ? "attacking" : ""} ${isEntityStruck(entity, motionNow) ? "struck" : ""} ${isEntityReviving(entity, motionNow) ? "reviving" : ""}`}
              style={{
                left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x}px`,
                top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y}px`,
                zIndex: entityDepth.sortZ + 5,
              }}
              data-ui-interactive={isInteractiveEntity ? "true" : "false"}
              data-object-id={entity.objectId}
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
                  onClick={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                  }}
                />
              ) : null}
              {isPlayer && screen === "game" && !useBevyEntityRenderer ? (
                <DecodedSelfSprite key={JSON.stringify([world.mapFileName, entity.objectId, entity.genderKey, entity.classKey,
                  entity.sprite?.bodyLibrary, entity.sprite?.hairLibrary, entity.sprite?.weaponLibrary,
                  entity.sprite?.weaponLibrarySecondary, entity.sprite?.mountLibrary,
                  entity.sprite?.altBodyLibrary, entity.sprite?.altHairLibrary, entity.sprite?.altWeaponLibrary,
                  entity.sprite?.altWeaponLibrarySecondary, entity.sprite?.frameBaseOffset, entity.sprite?.weaponFrameOffset,
                  entity.sprite?.altFrameBaseOffset, entity.sprite?.altWeaponFrameOffset, entity.sprite?.mountFrameOffset])}
                  sprite={sprite} packedActors={packedActors} assetReleaseId={assetReleaseId}>
                  {(displayed, images, failed) => <EntitySpriteLayers useBevyEntityRenderer={false}
                    effectsEnabled={effectsEnabled} packedActors={packedActors} sprite={displayed}
                    objectId={entity.objectId} residentImages={images} onResidentError={failed} />}
                </DecodedSelfSprite>
              ) : <EntitySpriteLayers useBevyEntityRenderer={useBevyEntityRenderer} effectsEnabled={effectsEnabled}
                packedActors={packedActors} sprite={sprite} objectId={entity.objectId} />}
            </div>
          );
        })}
        {(effectsEnabled ? viewportProjectiles : []).map((projectile) => {
          const targeted = projectile.r16Targeted;
          if (targeted?.phase === "pending") return null;
          const travelEndsAt = projectile.travelEndsAt ?? projectile.expiresAt;
          const exact = exactProjectileEffects.get(projectile.key);
          const exactSpell = exact?.spell ?? "";
          const impactStartedAt = travelEndsAt;
          const returnStartedAt = impactStartedAt + (exact?.impact?.durationMs ?? 0);
          const exactFrames = exact
            ? [
                {
                  phase: "projectile",
                  animation: exact.projectile,
                  frame: (!targeted || targeted.phase === "projectile") && motionNow < travelEndsAt
                    ? projectilePhaseFrame(
                        exact.projectile,
                        projectile.startedAt,
                        motionNow,
                        true,
                        targeted ? r16ProjectileFrameInterval(travelEndsAt - projectile.startedAt) : undefined,
                      )
                    : null,
                  dx: projectile.fromDx + (projectile.toDx - projectile.fromDx) * projectile.progress,
                  dy: projectile.fromDy + (projectile.toDy - projectile.fromDy) * projectile.progress,
                  worldX: projectile.toX,
                  worldY: projectile.toY,
                },
                {
                  phase: "impact",
                  animation: exact.impact,
                  frame: !targeted || targeted.phase === "impact"
                    ? projectilePhaseFrame(exact.impact, impactStartedAt, motionNow, false)
                    : null,
                  dx: projectile.toDx,
                  dy: projectile.toDy,
                  worldX: projectile.toX,
                  worldY: projectile.toY,
                },
                {
                  phase: "return",
                  animation: exact.returnEffect,
                  frame: projectilePhaseFrame(exact.returnEffect, returnStartedAt, motionNow, false),
                  dx: projectile.fromDx,
                  dy: projectile.fromDy,
                  worldX: projectile.fromX,
                  worldY: projectile.fromY,
                },
              ].filter(
                (entry): entry is typeof entry & { animation: EffectAnimation; frame: EffectFrameMeta } =>
                  Boolean(entry.animation && entry.frame),
              )
            : [];
          if (exactFrames.length > 0) {
            return (
              <Fragment key={projectile.key}>
                {exactFrames.map(({ phase, animation, frame, dx, dy, worldX, worldY }) => (
                  <img
                    key={`${projectile.key}:${phase}`}
                    ref={
                      imperativeCamera
                        ? registerCameraSurface(`projectile:${projectile.key}:${phase}`)
                        : undefined
                    }
                    className="scene-crystal-effect-frame projectile"
                    src={frame.path}
                    alt=""
                    aria-hidden="true"
                    draggable={false}
                    data-projectile-key={projectile.key}
                    data-projectile-spell={exactSpell}
                    data-projectile-phase={phase}
                    data-mir2-original-src={frame.path}
                    onError={handleSceneAssetImageError}
                    onLoad={handleSceneAssetImageLoad}
                    style={{
                      position: "absolute",
                      left:
                        VIEWPORT_ENTITY_LEFT_ORIGIN +
                        dx * VIEWPORT_CELL_WIDTH +
                        playerCameraMotionOffset.x +
                        animation.offset.x +
                        frame.x,
                      top:
                        VIEWPORT_ENTITY_TOP_ORIGIN +
                        dy * VIEWPORT_CELL_HEIGHT +
                        playerCameraMotionOffset.y +
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
                      zIndex: targeted
                        ? R16_TARGETED_EFFECT_DEPTH
                        : viewportDepthForCell(worldX, worldY, viewportDepthPlayer, 9),
                    }}
                  />
                ))}
              </Fragment>
            );
          }
          if (targeted || exact || motionNow >= travelEndsAt) return null;
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
                zIndex: viewportDepthForCell(projectile.toX, projectile.toY, viewportDepthPlayer, 9),
              }}
            />
          );
        })}
        {/* Procedural magic / map VFX fallback nodes (atlas-free; see FallbackVfxNode). */}
        {(effectsEnabled ? fallbackVfx : []).map((effect) => (
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
        {(effectsEnabled ? displayResolvedEffectFrames : []).map(({ effect, animation, frame }) => {
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
          // Crystal draws the free water effects after world objects, then draws the actor notice.
          const fishingDepth = effect.source === "fishingWater" ? R16_TARGETED_EFFECT_DEPTH
            : effect.source === "fishingBite" ? R16_TARGETED_EFFECT_DEPTH + 1 : null;
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
                  opacity: animation.opacity,
                  filter:
                    animation.light > 0
                      ? `drop-shadow(0 0 ${Math.min(animation.light * 2, 16)}px #fff)`
                      : undefined,
                  pointerEvents: "none",
                  zIndex: fishingDepth ?? (effect.source === "spell" && r16TargetedSpell(effect.spellOrEffect)
                    ? R16_TARGETED_EFFECT_DEPTH
                    : viewportDepthForCell(
                        worldX,
                        worldY,
                        viewportDepthPlayer,
                        crystalSceneEffectLayerOffset(effect.source),
                      )),
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
                    opacity: animation.opacity,
                    pointerEvents: "none",
                    zIndex: fishingDepth ?? viewportDepthForCell(
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
              const labelLines = entityDisplayLabelLines(entity, t);
              const labelColour = entityNameplateColor(entity);
              const questIcon =
                entity.kind === "npc"
                  ? questIconForEntity(entity, world.questLog, sceneSpriteFrameIndex)
                  : null;
              // Crystal creates an AutoSize MirLabel for each NPC name line;
              // 48/50px is its centering anchor, never a text clipping width.
              const labelAnchorWidth = entity.kind === "npc" || entity.kind === "monster" ? 48 : 50;
              const labelGdiTexts = labelLines.map((line) => entity.dead
                ? null
                : findCrystalGdiTextAsset({
                    text: line.text,
                    foreground: line.role === "secondary" && entity.kind === "npc" ? "#ffffff" : labelColour,
                    outline: true,
                  }));
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
                        if (event.button === 0) {
                          event.preventDefault();
                          event.stopPropagation();
                          onActivateEntity(entity.objectId);
                        }
                      }}
                      style={{
                        left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x + entityQuestIconLeftOffset(entity, sprite)}px`,
                        top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y + entityQuestIconTopOffset(sprite)}px`,
                        zIndex: viewportDepthForCell(entity.x, entity.y, viewportDepthPlayer, 12),
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
                    aria-label={labelLines.map((line) => line.text).join(" ")}
                    style={{
                      left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x + entityNameplateLeftOffset(entity, sprite)}px`,
                      top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y + entityNameplateTopOffset(entity, sprite)}px`,
                      "--entity-name-color": labelColour,
                      visibility: nameView || hoveredObjectId === entity.objectId
                        || (entity.kind === "monster" && !entity.dead && questTargetObjectIds?.has(entity.objectId))
                        ? "visible" : "hidden",
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
                            if (event.button === 0) {
                              event.preventDefault();
                              event.stopPropagation();
                            }
                          }
                        : undefined
                    }
                    onClick={isInteractiveEntity ? () => onActivateEntity(entity.objectId) : undefined}
                  >
                    {labelLines.map((line, index) => {
                      const gdiText = labelGdiTexts[index];
                      return gdiText ? (
                        <CrystalGdiTextImage
                          key={`${entity.objectId}-label-${index}`}
                          asset={gdiText}
                          className="entity-nameplate-gdi"
                          accessibleText={line.text}
                          style={{
                            marginLeft: Math.trunc((labelAnchorWidth - gdiText.size.output.width) / 2),
                            top: Math.trunc(gdiText.size.output.height / 2) - 6,
                          }}
                        />
                      ) : (
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
                      );
                    })}
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

function entityDisplayName(entity: DisplayEntity, t: TranslateFn): string {
  return entity.kind === "npc" || entity.kind === "monster" ? localizeCrystalEntityName(entity.name, t) : entity.name;
}

function entityDisplayLabelLines(
  entity: DisplayEntity,
  t: TranslateFn,
): Array<{ text: string; role: "primary" | "secondary" }> {
  const displayName = entityDisplayName(entity, t);
  if (entity.ownerName) {
    return [
      { text: displayName, role: "primary" },
      {
        text: t("client.OwnerHero", [entity.ownerName], `${entity.ownerName}'s Hero`),
        role: "secondary",
      },
    ];
  }

  if (entity.kind !== "npc" && entity.kind !== "monster") {
    return [{ text: displayName, role: "primary" }];
  }

  const parts = displayName.split("_").filter(Boolean);
  if (parts.length <= 1) {
    return [{ text: displayName.replace(/_/g, " "), role: "primary" }];
  }

  return parts.map((part, index) => ({ text: part, role: index === 0 ? "primary" : "secondary" }));
}
