"use client";

import type { CSSProperties, MouseEvent } from "react";

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
  entityKindClassName: (kind: EntityKind) => string;
  onPickGroundDrop: (objectId: string) => void;
  onActivateEntity: (objectId: string) => void;
}) {
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
            data-map-cell-x={sprite.cellX}
            data-map-cell-y={sprite.cellY}
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
              {sprite?.rearWeapons.map((weapon, index) => (
                <img
                  key={`rear-${entity.objectId}-${index}-${weapon.path}`}
                  className="entity-sprite-layer weapon rear"
                  src={weapon.path}
                  alt=""
                  draggable={false}
                  style={{
                    left: weapon.x,
                    top: weapon.y,
                    width: weapon.width,
                    height: weapon.height,
                  }}
                />
              ))}
              {sprite?.body ? (
                <img
                  className="entity-sprite-layer body"
                  src={sprite.body.path}
                  alt=""
                  draggable={false}
                  style={{
                    left: sprite.body.x,
                    top: sprite.body.y,
                    width: sprite.body.width,
                    height: sprite.body.height,
                  }}
                />
              ) : null}
              {sprite?.hair ? (
                <img
                  className="entity-sprite-layer hair"
                  src={sprite.hair.path}
                  alt=""
                  draggable={false}
                  style={{
                    left: sprite.hair.x,
                    top: sprite.hair.y,
                    width: sprite.hair.width,
                    height: sprite.hair.height,
                  }}
                />
              ) : null}
              {sprite?.frontWeapons.map((weapon, index) => (
                <img
                  key={`front-${entity.objectId}-${index}-${weapon.path}`}
                  className="entity-sprite-layer weapon front"
                  src={weapon.path}
                  alt=""
                  draggable={false}
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
