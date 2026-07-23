import type { ClientScreen } from "../../lib/original-ui";
import type {
  DisplayEntity,
  DisplayProjectile,
  EntityMotionSnapshot,
  EntitySpriteAnimationState,
} from "./original-client-types";
import {
  CRYSTAL_MOVE_FRAME_COUNT,
  CRYSTAL_MOVE_FRAME_INTERVAL_MS,
  EMPTY_VIEWPORT_OFFSET,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  type ViewportOffset,
} from "./original-client-scene-layout";

function animationStateForMovement(
  entity: DisplayEntity,
  tileDistance: number,
  now: number,
): EntitySpriteAnimationState {
  if (entity.dead || entity.kind === "npc") {
    return "standing";
  }

  if (isEntityMovementAnimationActive(entity, now)) {
    return entity.movementAnimation ?? "walking";
  }

  if (entity.kind === "monster") {
    return "walking";
  }

  return tileDistance > 1 ? "running" : "walking";
}

function movementFrameCountForEntity(
  entity: DisplayEntity,
  animationState: EntitySpriteAnimationState,
) {
  if (entity.movementFrameCount && entity.movementFrameCount > 0) {
    return entity.movementFrameCount;
  }
  return entity.sprite?.mountLibrary && animationState === "walking" ? 8 : CRYSTAL_MOVE_FRAME_COUNT;
}

function animationStateLifetimeMs(
  entity: DisplayEntity,
  animationState: EntitySpriteAnimationState,
  tileDistance: number,
) {
  switch (animationState) {
    case "running":
    case "walking":
      return tileDistance > 0
        ? movementFrameCountForEntity(entity, animationState) * CRYSTAL_MOVE_FRAME_INTERVAL_MS
        : 0;
    default:
      return 0;
  }
}

export function entityAnimationStateForEntity(
  entity: DisplayEntity,
  snapshots: Record<string, EntityMotionSnapshot>,
  now: number,
): EntitySpriteAnimationState {
  if (entity.kind === "npc") {
    return "standing";
  }

  if (isEntityReviving(entity, now)) {
    return "reviving";
  }

  if (isEntityDying(entity, now)) {
    return "dying";
  }

  if (entity.dead) {
    return "dead";
  }

  if (isEntityStruck(entity, now)) {
    return "struck";
  }

  if (isEntityAttacking(entity, now)) {
    return entity.attackAnimation === "range" || entity.attackAnimation === "spell"
      ? "attackRange"
      : "attackMelee";
  }

  if (isEntityMovementAnimationActive(entity, now)) {
    return entity.movementAnimation ?? "walking";
  }

  const snapshot = snapshots[entity.objectId];
  if (!snapshot || snapshot.expiresAt <= now) {
    return "standing";
  }

  return snapshot.animationState;
}

export function entityMotionOffsetForEntity(
  entity: DisplayEntity,
  snapshots: Record<string, EntityMotionSnapshot>,
  now: number,
): ViewportOffset {
  const snapshot = snapshots[entity.objectId];
  if (!snapshot) {
    return EMPTY_VIEWPORT_OFFSET;
  }

  const remaining = remainingMotionRatio(snapshot, now);
  if (remaining <= 0) {
    return EMPTY_VIEWPORT_OFFSET;
  }

  return {
    x: crystalMovementPixelOffset((snapshot.fromX - snapshot.toX) * VIEWPORT_CELL_WIDTH * remaining),
    y: crystalMovementPixelOffset((snapshot.fromY - snapshot.toY) * VIEWPORT_CELL_HEIGHT * remaining),
  };
}

export function rebaseViewportEntitiesToRenderPlayer(
  entities: Array<DisplayEntity & { dx: number; dy: number }>,
  renderPlayer: DisplayEntity | null,
): Array<DisplayEntity & { dx: number; dy: number }> {
  if (!renderPlayer) {
    return entities;
  }

  return entities.map((entity) => {
    const renderEntity =
      entity.objectId === renderPlayer.objectId
        ? { ...entity, ...renderPlayer }
        : entity;
    const dx = renderEntity.x - renderPlayer.x;
    const dy = renderEntity.y - renderPlayer.y;
    if (renderEntity === entity && entity.dx === dx && entity.dy === dy) {
      return entity;
    }
    return { ...renderEntity, dx, dy };
  });
}

export function refreshEntityMotionSnapshots(
  screen: ClientScreen,
  entities: DisplayEntity[],
  renderPlayer: DisplayEntity | null,
  snapshots: Record<string, EntityMotionSnapshot>,
  now: number,
): Record<string, EntityMotionSnapshot> {
  if (screen !== "game") {
    return {};
  }

  const nextSnapshots: Record<string, EntityMotionSnapshot> = {};
  const motionEntities = entities.map((entity) =>
    renderPlayer && entity.objectId === renderPlayer.objectId
      ? { ...entity, ...renderPlayer }
      : entity,
  );

  for (const entity of motionEntities) {
    const isRenderPlayer = Boolean(renderPlayer && entity.objectId === renderPlayer.objectId);
    const previous = snapshots[entity.objectId];
    if (previous && previous.toX === entity.x && previous.toY === entity.y) {
      nextSnapshots[entity.objectId] = previous;
      continue;
    }
    if (isRenderPlayer && previous && isStaleSelfSourceEcho(entity, previous)) {
      nextSnapshots[entity.objectId] = previous;
      continue;
    }

    let previousX = previous ? currentMotionCoordinate(previous.fromX, previous.toX, previous, now) : entity.x;
    let previousY = previous ? currentMotionCoordinate(previous.fromY, previous.toY, previous, now) : entity.y;
    let tileDistance = Math.max(Math.abs(entity.x - previousX), Math.abs(entity.y - previousY));
    if (isRenderPlayer && tileDistance < 0.125) {
      previousX = entity.x;
      previousY = entity.y;
      tileDistance = 0;
    }
    if (isRenderPlayer && previous && tileDistance > 3) {
      const previousTargetDistance = Math.max(
        Math.abs(entity.x - previous.toX),
        Math.abs(entity.y - previous.toY),
      );
      if (previousTargetDistance <= 3) {
        previousX = previous.toX;
        previousY = previous.toY;
        tileDistance = previousTargetDistance;
      }
    }

    const maxSmoothTileDistance = 3;
    if (tileDistance > maxSmoothTileDistance) {
      nextSnapshots[entity.objectId] = {
        fromX: entity.x,
        fromY: entity.y,
        toX: entity.x,
        toY: entity.y,
        animationState: "standing",
        startedAt: now,
        expiresAt: 0,
      };
      continue;
    }

    if (tileDistance > 0.001) {
      const animationState = animationStateForMovement(entity, tileDistance, now);
      const frameCount = movementFrameCountForEntity(entity, animationState);
      const packetStartedAt =
        entity.movementStartedAt !== undefined &&
        entity.movementStartedAt <= now &&
        entity.movementUntil !== undefined &&
        entity.movementUntil > now
          ? entity.movementStartedAt
          : now;
      const packetExpiresAt =
        entity.movementUntil !== undefined && entity.movementUntil > now
          ? entity.movementUntil
          : now + animationStateLifetimeMs(entity, animationState, tileDistance);
      nextSnapshots[entity.objectId] = {
        fromX: previousX,
        fromY: previousY,
        toX: entity.x,
        toY: entity.y,
        animationState,
        frameCount,
        startedAt: packetStartedAt,
        expiresAt: packetExpiresAt,
      };
      continue;
    }

    nextSnapshots[entity.objectId] = previous
      ? {
          ...previous,
          toX: entity.x,
          toY: entity.y,
        }
      : {
          fromX: entity.x,
          fromY: entity.y,
          toX: entity.x,
          toY: entity.y,
          animationState: "standing",
          startedAt: now,
          expiresAt: 0,
        };
  }

  return nextSnapshots;
}

export function cameraMotionOffsetForEntity(
  entity: DisplayEntity,
  snapshots: Record<string, EntityMotionSnapshot>,
  now: number,
): ViewportOffset {
  const snapshot = snapshots[entity.objectId];
  if (!snapshot) {
    return EMPTY_VIEWPORT_OFFSET;
  }

  const remaining = remainingMotionRatio(snapshot, now);
  if (remaining <= 0) {
    return EMPTY_VIEWPORT_OFFSET;
  }

  return {
    x: crystalMovementPixelOffset((snapshot.toX - snapshot.fromX) * VIEWPORT_CELL_WIDTH * remaining),
    y: crystalMovementPixelOffset((snapshot.toY - snapshot.fromY) * VIEWPORT_CELL_HEIGHT * remaining),
  };
}

function remainingMotionRatio(snapshot: EntityMotionSnapshot, now: number) {
  if (snapshot.expiresAt <= snapshot.startedAt) {
    return 0;
  }

  return 1 - movementProgressRatio(snapshot, now);
}

function movementProgressRatio(snapshot: EntityMotionSnapshot, now: number) {
  const duration = snapshot.expiresAt - snapshot.startedAt;
  const elapsed = Math.min(Math.max(now - snapshot.startedAt, 0), duration);

  if (elapsed >= duration) {
    return 1;
  }

  if (snapshot.animationState !== "walking" && snapshot.animationState !== "running") {
    return elapsed / duration;
  }

  return crystalSteppedMovementProgressRatio(elapsed, snapshot.frameCount);
}

export function crystalSteppedMovementProgressRatio(
  elapsedMs: number,
  frameCount = CRYSTAL_MOVE_FRAME_COUNT,
) {
  if (!Number.isFinite(elapsedMs)) {
    return 0;
  }

  const normalizedFrameCount = Math.max(1, Math.trunc(frameCount));
  const frameIndex = Math.floor(Math.max(elapsedMs, 0) / CRYSTAL_MOVE_FRAME_INTERVAL_MS);
  // Crystal couples sprite frame p with movement progress (p + 1) / frameCount.
  return Math.min(1, (frameIndex + 1) / normalizedFrameCount);
}

function crystalMovementPixelOffset(value: number) {
  if (!Number.isFinite(value) || Math.abs(value) < 0.001) {
    return 0;
  }

  const crystalValue = Math.trunc(value);
  const evenCrystalValue = crystalValue + (crystalValue % 2);
  return Object.is(evenCrystalValue, -0) ? 0 : evenCrystalValue;
}

function currentMotionCoordinate(from: number, to: number, snapshot: EntityMotionSnapshot, now: number) {
  const remaining = remainingMotionRatio(snapshot, now);
  if (remaining <= 0) {
    return to;
  }

  return to + (from - to) * remaining;
}

function isStaleSelfSourceEcho(entity: DisplayEntity, previous: EntityMotionSnapshot) {
  const sourceDistance = Math.max(
    Math.abs(entity.x - previous.fromX),
    Math.abs(entity.y - previous.fromY),
  );
  const targetDistance = Math.max(
    Math.abs(entity.x - previous.toX),
    Math.abs(entity.y - previous.toY),
  );
  if (sourceDistance >= 0.125 || targetDistance < 0.125) {
    return false;
  }

  return entity.direction === directionForMotionDelta(
    previous.toX - previous.fromX,
    previous.toY - previous.fromY,
  );
}

function directionForMotionDelta(dx: number, dy: number) {
  const horizontal = Math.sign(dx);
  const vertical = Math.sign(dy);
  if (horizontal === 0 && vertical < 0) return "Up";
  if (horizontal > 0 && vertical < 0) return "UpRight";
  if (horizontal > 0 && vertical === 0) return "Right";
  if (horizontal > 0 && vertical > 0) return "DownRight";
  if (horizontal === 0 && vertical > 0) return "Down";
  if (horizontal < 0 && vertical > 0) return "DownLeft";
  if (horizontal < 0 && vertical === 0) return "Left";
  if (horizontal < 0 && vertical < 0) return "UpLeft";
  return null;
}

export function projectileProgress(projectile: DisplayProjectile, now: number) {
  if (projectile.expiresAt <= projectile.startedAt) {
    return 1;
  }

  const duration = projectile.expiresAt - projectile.startedAt;
  const elapsed = Math.min(Math.max(now - projectile.startedAt, 0), duration);
  return elapsed / duration;
}

export function transientFrameCycle(
  now: number,
  startedAt: number | undefined,
  frameIntervalMs: number,
  frameCount: number,
) {
  if (typeof startedAt !== "number" || frameIntervalMs <= 0 || frameCount <= 1) {
    return 0;
  }

  const raw = Math.floor(Math.max(now - startedAt, 0) / frameIntervalMs);
  return Math.min(raw, frameCount - 1);
}

export function isEntityMovementAnimationActive(entity: DisplayEntity, now: number) {
  return (
    (entity.movementAnimation === "walking" || entity.movementAnimation === "running") &&
    typeof entity.movementUntil === "number" &&
    entity.movementUntil > now
  );
}

export function isEntityAttacking(entity: DisplayEntity, now: number) {
  return typeof entity.attackUntil === "number" && entity.attackUntil > now;
}

export function isEntityStruck(entity: DisplayEntity, now: number) {
  return typeof entity.struckUntil === "number" && entity.struckUntil > now;
}

function isEntityDying(entity: DisplayEntity, now: number) {
  return typeof entity.dieUntil === "number" && entity.dieUntil > now;
}

export function isEntityReviving(entity: DisplayEntity, now: number) {
  return typeof entity.reviveUntil === "number" && entity.reviveUntil > now;
}
