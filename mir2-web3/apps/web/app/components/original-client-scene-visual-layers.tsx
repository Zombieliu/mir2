"use client";

import { useEffect, useRef, useState, type CSSProperties, type MouseEvent } from "react";

import type { ClientScreen } from "../../lib/original-ui";
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
  const floatingNumbers = useFloatingCombatNumbers(viewportEntitySprites);
  const entityById = new Map(viewportEntitySprites.map((entry) => [entry.entity.objectId, entry]));
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

      <div className={`viewport-combat-numbers ${screen !== "game" ? "hidden" : ""}`}>
        {floatingNumbers.map((number) => {
          const entry = entityById.get(number.objectId);
          if (!entry) return null;
          const { entity } = entry;
          const isPlayer = player?.objectId === entity.objectId;
          const entityMotionOffset = isPlayer
            ? EMPTY_VIEWPORT_OFFSET
            : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
          const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
          const left =
            VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x + VIEWPORT_CELL_WIDTH / 2;
          const top =
            VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y - 24;
          return (
            <span
              key={`combat-number-${number.id}`}
              className={`combat-number ${number.kind}`}
              style={{
                left: `${left}px`,
                top: `${top}px`,
                zIndex: viewportDepthForCell(entity.x, entity.y, viewportDepthPlayer, 96),
              }}
            >
              {number.kind === "heal" ? "+" : "-"}
              {number.amount}
            </span>
          );
        })}
      </div>

      <div className={`viewport-vignette ${screen === "game" && viewportMapSprites.floor.length ? "hidden" : ""}`} />
    </>
  );
}

type FloatingCombatNumber = {
  id: number;
  objectId: string;
  amount: number;
  kind: "damage" | "heal";
};

const FLOATING_COMBAT_NUMBER_DURATION_MS = 900;

// Derive floating damage/heal numbers from per-entity HP deltas between world
// snapshots (the server does not send discrete combat-text events). A decrease
// is damage, an increase is a heal. Full-bar swings (respawn, max-HP
// corrections, first sighting) are ignored so they do not flash as huge hits.
function useFloatingCombatNumbers(entries: ViewportEntitySpriteEntry[]): FloatingCombatNumber[] {
  const [numbers, setNumbers] = useState<FloatingCombatNumber[]>([]);
  const lastHpRef = useRef<Map<string, number>>(new Map());
  const idRef = useRef(0);
  // Pending removal timers are tracked in a ref and cleared only on unmount;
  // this effect re-runs on every render (entries identity changes), so a
  // per-render cleanup would cancel the removal timers and leak numbers.
  const timersRef = useRef<number[]>([]);
  useEffect(() => () => timersRef.current.forEach((timer) => window.clearTimeout(timer)), []);

  useEffect(() => {
    const lastHp = lastHpRef.current;
    const seen = new Set<string>();
    const spawned: FloatingCombatNumber[] = [];

    for (const { entity } of entries) {
      const hp = entity.hp;
      if (typeof hp !== "number") continue;
      seen.add(entity.objectId);
      const previous = lastHp.get(entity.objectId);
      lastHp.set(entity.objectId, hp);
      if (previous === undefined) continue;
      const delta = hp - previous;
      if (delta === 0) continue;
      if (entity.maxHp && Math.abs(delta) >= entity.maxHp) continue;
      // Suppress tiny heals (passive HP regen ticks) so they do not spam green
      // numbers; always show damage.
      if (delta > 0 && delta < Math.max(3, Math.round((entity.maxHp ?? 0) * 0.03))) continue;
      idRef.current += 1;
      spawned.push({
        id: idRef.current,
        objectId: entity.objectId,
        amount: Math.abs(delta),
        kind: delta < 0 ? "damage" : "heal",
      });
    }

    for (const key of Array.from(lastHp.keys())) {
      if (!seen.has(key)) lastHp.delete(key);
    }

    if (!spawned.length) return;
    setNumbers((current) => [...current, ...spawned]);
    for (const number of spawned) {
      const timer = window.setTimeout(() => {
        setNumbers((current) => current.filter((entry) => entry.id !== number.id));
        timersRef.current = timersRef.current.filter((entry) => entry !== timer);
      }, FLOATING_COMBAT_NUMBER_DURATION_MS);
      timersRef.current.push(timer);
    }
  }, [entries]);

  return numbers;
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
