"use client";

import { useEffect, useRef, useState, type CSSProperties, type MouseEvent } from "react";

import type { ClientScreen } from "../../lib/original-ui";
import type { EffectAssets } from "../../lib/crystal-magic-effects";
import { loadEffectAssets } from "../../lib/crystal-magic-effects";
import {
  collectViewportFallbackVfx,
  fallbackVfxStyle,
  paletteForElement,
  type FallbackVfx,
} from "../../lib/vfx-fallback";
import type {
  DisplayEntity,
  DisplayProjectile,
  DisplayWorld,
  EntityKind,
  EntityMotionSnapshot,
  TranslateFn,
} from "./original-client-types";
import {
  EMPTY_VIEWPORT_OFFSET,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  VIEWPORT_ENTITY_LEFT_ORIGIN,
  VIEWPORT_ENTITY_TOP_ORIGIN,
  VIEWPORT_TILE_CENTER_X,
  VIEWPORT_TILE_CENTER_Y,
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
  mapSpriteBlendMode,
  mapSpriteRenderPath,
  questIconForEntity,
  ratio,
  viewportDepthForCell,
  type ViewportEntitySprite,
  type ViewportMapSprites,
  type ViewportOffset,
} from "./original-client-scene-rendering";

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
  sprite: ViewportEntitySprite | null;
};

// --- Procedural magic / map VFX fallback ----------------------------------
// The real effect atlases (lib/crystal-magic-effects) are PREFERRED: loadEffectAssets fetches the
// exported manifest once, and resolveSpell/MapEffect take priority for any effect it can resolve.
// Until those atlases exist the loader yields an empty set, the resolver returns null, and we draw
// the data-driven CSS fallback (lib/vfx-fallback) instead — so casting / skills are visibly
// reactive rather than inert. The loader is memoised at module scope so it never refetches.
let effectAssetsPromise: Promise<EffectAssets> | null = null;
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

function useEffectAssets(): EffectAssets | null {
  const [assets, setAssets] = useState<EffectAssets | null>(null);
  useEffect(() => {
    let cancelled = false;
    void loadEffectAssetsOnce().then((value) => {
      if (!cancelled) {
        setAssets(value);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);
  return assets;
}

// Renders one procedural fallback effect as an inline-styled div (no globals.css changes). Returns
// null once the effect has expired so finished effects drop out of the tree automatically.
function FallbackVfxNode({
  effect,
  now,
  cameraOffset,
  viewportDepthPlayer,
}: {
  effect: FallbackVfx;
  now: number;
  cameraOffset: ViewportOffset;
  viewportDepthPlayer: Pick<DisplayEntity, "x" | "y">;
}) {
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

export function OriginalClientSceneVisualLayers({
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
  sceneSpriteFrameIndex,
  useBevyEntityRenderer,
  entityKindClassName,
  onPickGroundDrop,
  onActivateEntity,
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
  sceneSpriteFrameIndex: number;
  useBevyEntityRenderer: boolean;
  entityKindClassName: (kind: EntityKind) => string;
  onPickGroundDrop: (objectId: string) => void;
  onActivateEntity: (objectId: string) => void;
}) {
  // --- Magic / map VFX fallback integration (single block) ---------------
  // Atlas-first: when loadEffectAssets resolves real frames, the collectors below skip the
  // procedural fallback for those effects. Until then (assets empty / null) we derive short-lived
  // CSS effects from the SAME viewport-delta data the projectiles use, so casting and skills are
  // visibly distinct instead of inert. collectViewportFallbackVfx returns [] on idle frames, so
  // this costs nothing when nothing is casting and no projectiles are live.
  const effectAssets = useEffectAssets();
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
    { now: motionNow, assets: effectAssets },
  );

  return (
    <>
      <div className={`viewport-drop-overlay ${screen !== "game" ? "hidden" : ""}`}>
        {viewportGroundDrops.map((drop) => (
          <button
            key={`drop-${drop.objectId}`}
            type="button"
            className="ground-drop-marker"
            style={{
              left: `${VIEWPORT_TILE_CENTER_X + drop.dx * VIEWPORT_CELL_WIDTH + playerCameraMotionOffset.x}px`,
              top: `${VIEWPORT_TILE_CENTER_Y + drop.dy * VIEWPORT_CELL_HEIGHT + playerCameraMotionOffset.y - 12}px`,
              zIndex: viewportDepthForCell(drop.x, drop.y, viewportDepthPlayer, 16),
            }}
            onClick={() => onPickGroundDrop(drop.objectId)}
            data-ui-interactive="true"
            title={`${drop.name} x${drop.quantity}`}
          >
            <span className="drop-dot" />
            <span className="drop-label" style={{ color: argbToCssColor(drop.nameColourArgb) }}>
              {drop.quantity > 1 ? `${drop.name} x${drop.quantity}` : drop.name}
            </span>
          </button>
        ))}
      </div>

      <div className={`viewport-sprite-overlay ${screen !== "game" ? "hidden" : ""}`}>
        {viewportMapSprites.objects.map((sprite) => (
          <img
            key={sprite.key}
            className="scene-map-object-sprite"
            src={mapSpriteRenderPath(sprite.path)}
            alt=""
            draggable={false}
            data-map-sprite-path={sprite.path}
            data-map-render-path={mapSpriteRenderPath(sprite.path)}
            data-mir2-original-src={mapSpriteRenderPath(sprite.path)}
            data-map-cell-x={sprite.cellX}
            data-map-cell-y={sprite.cellY}
            onError={handleSceneAssetImageError}
            onLoad={handleSceneAssetImageLoad}
            style={{
              left: sprite.left + playerCameraMotionOffset.x,
              top: sprite.top + playerCameraMotionOffset.y,
              width: sprite.width,
              height: sprite.height,
              mixBlendMode: mapSpriteBlendMode(sprite.path),
              zIndex: sprite.zIndex,
            }}
          />
        ))}
        {viewportEntitySprites.map(({ entity, sprite }) => {
          const isPlayer = player?.objectId === entity.objectId;
          const entityMotionOffset = isPlayer
            ? EMPTY_VIEWPORT_OFFSET
            : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
          const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
          const label = entityDisplayName(entity);
          const hitBounds = entitySpriteHitBounds(sprite);
          const hitWidth = hitBounds.right - hitBounds.left;
          const hitHeight = hitBounds.bottom - hitBounds.top;
          const healthRatio =
            isPlayer && entity.hp !== undefined && entity.maxHp ? ratio(entity.hp, entity.maxHp) : null;
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
              className={`entity-sprite-stack ${entityKindClassName(entity.kind)} ${entity.objectId === selectedEntity?.objectId ? "selected" : ""} ${entity.dead ? "dead" : ""} ${isEntityAttacking(entity, motionNow) ? "attacking" : ""} ${isEntityStruck(entity, motionNow) ? "struck" : ""} ${isEntityReviving(entity, motionNow) ? "reviving" : ""}`}
              style={{
                left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x}px`,
                top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y}px`,
                zIndex: viewportDepthForCell(entity.x, entity.y, viewportDepthPlayer, 64),
              }}
              data-ui-interactive="true"
              onMouseDown={handleEntityPointerActivate}
              onContextMenu={handleEntityContextActivate}
            >
              {healthRatio !== null ? (
                <div className="entity-health-bar">
                  <span style={{ width: `${healthRatio * 100}%` }} />
                </div>
              ) : null}
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
              {!useBevyEntityRenderer && sprite?.rearWeapons.map((weapon, index) => (
                <img
                  key={`rear-${entity.objectId}-${index}-${weapon.path}`}
                  className="entity-sprite-layer weapon rear"
                  src={weapon.path}
                  alt=""
                  draggable={false}
                  data-mir2-original-src={weapon.path}
                  onError={handleSceneAssetImageError}
                  onLoad={handleSceneAssetImageLoad}
                  style={{
                    left: weapon.x,
                    top: weapon.y,
                    width: weapon.width,
                    height: weapon.height,
                  }}
                />
              ))}
              {!useBevyEntityRenderer && sprite?.body ? (
                <img
                  className="entity-sprite-layer body"
                  src={sprite.body.path}
                  alt=""
                  draggable={false}
                  data-mir2-original-src={sprite.body.path}
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
                  src={sprite.hair.path}
                  alt=""
                  draggable={false}
                  data-mir2-original-src={sprite.hair.path}
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
              {!useBevyEntityRenderer && sprite?.frontWeapons.map((weapon, index) => (
                <img
                  key={`front-${entity.objectId}-${index}-${weapon.path}`}
                  className="entity-sprite-layer weapon front"
                  src={weapon.path}
                  alt=""
                  draggable={false}
                  data-mir2-original-src={weapon.path}
                  onError={handleSceneAssetImageError}
                  onLoad={handleSceneAssetImageLoad}
                  style={{
                    left: weapon.x,
                    top: weapon.y,
                    width: weapon.width,
                    height: weapon.height,
                  }}
                />
              ))}
              {entity.kind === "npc" ? (
                (() => {
                  const questIcon = questIconForEntity(entity, world.questLog, sceneSpriteFrameIndex);
                  return questIcon ? (
                    <img
                      className="entity-quest-icon"
                      src={questIcon}
                      alt=""
                      draggable={false}
                      data-mir2-original-src={questIcon}
                      onError={handleSceneAssetImageError}
                      onLoad={handleSceneAssetImageLoad}
                      style={{
                        left: entityQuestIconLeftOffset(entity, sprite),
                        top: entityQuestIconTopOffset(sprite),
                      }}
                    />
                  ) : null;
                })()
              ) : null}
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
          />
        ))}
      </div>

      <div className={`viewport-entity-overlay ${screen !== "game" ? "hidden" : ""}`}>
        {player
          ? viewportEntitySprites.map(({ entity, sprite }) => {
              const isPlayer = player.objectId === entity.objectId;
              const entityMotionOffset = isPlayer
                ? EMPTY_VIEWPORT_OFFSET
                : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
              const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
              const labelLines = entityDisplayLabelLines(entity);

              return (
                <button
                  key={`entity-${entity.objectId}`}
                  type="button"
                  className={`entity-nameplate ${entityKindClassName(entity.kind)} ${entity.objectId === selectedEntity?.objectId ? "selected" : ""}`}
                  style={{
                    left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x + entityNameplateLeftOffset(entity, sprite)}px`,
                    top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y + entityNameplateTopOffset(entity, sprite)}px`,
                    "--entity-name-color": entityNameplateColor(entity),
                  } as CSSProperties}
                  data-ui-interactive="true"
                  onMouseDown={(event) => {
                    if (event.button === 0 || event.button === 2) {
                      event.preventDefault();
                      event.stopPropagation();
                    }
                  }}
                  onClick={() => onActivateEntity(entity.objectId)}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    onActivateEntity(entity.objectId);
                  }}
                >
                  {labelLines.map((line, index) => (
                    <strong
                      key={`${entity.objectId}-label-${index}`}
                      className={line.role === "secondary" ? "entity-subname" : undefined}
                    >
                      {line.text}
                    </strong>
                  ))}
                  {entity.dead ? <strong className="entity-state-label">{t("ui.dead")}</strong> : null}
                </button>
              );
            })
          : null}
      </div>

      <div className={`viewport-vignette ${screen === "game" && viewportMapSprites.floor.length ? "hidden" : ""}`} />
    </>
  );
}

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
