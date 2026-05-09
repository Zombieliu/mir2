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

function animationStateLifetimeMs(animationState: EntitySpriteAnimationState, _tileDistance: number) {
  switch (animationState) {
    case "running":
    case "walking":
      return 600;
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
    return entity.attackAnimation === "range" ? "attackRange" : "attackMelee";
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
      ? { ...entity, x: renderPlayer.x, y: renderPlayer.y }
      : entity,
  );

  for (const entity of motionEntities) {
    const previous = snapshots[entity.objectId];
    if (previous && previous.toX === entity.x && previous.toY === entity.y) {
      nextSnapshots[entity.objectId] = previous;
      continue;
    }

    const previousX = previous ? currentMotionCoordinate(previous.fromX, previous.toX, previous, now) : entity.x;
    const previousY = previous ? currentMotionCoordinate(previous.fromY, previous.toY, previous, now) : entity.y;
    const tileDistance = Math.max(Math.abs(entity.x - previousX), Math.abs(entity.y - previousY));

    if (tileDistance > 3) {
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
          : now + animationStateLifetimeMs(animationState, tileDistance);
      nextSnapshots[entity.objectId] = {
        fromX: previousX,
        fromY: previousY,
        toX: entity.x,
        toY: entity.y,
        animationState,
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

  // Crystal ties walk/run displacement to the same six 100ms frames used by the body sprite.
  const frameIndex = Math.min(
    Math.floor(elapsed / CRYSTAL_MOVE_FRAME_INTERVAL_MS),
    CRYSTAL_MOVE_FRAME_COUNT - 1,
  );
  return Math.min((frameIndex + 1) / CRYSTAL_MOVE_FRAME_COUNT, 1);
}

function crystalMovementPixelOffset(value: number) {
  if (!Number.isFinite(value) || Math.abs(value) < 0.001) {
    return 0;
  }

  const integer = value < 0 ? Math.ceil(value) : Math.floor(value);
  const even = integer + (integer % 2);
  return Object.is(even, -0) ? 0 : even;
}

function currentMotionCoordinate(from: number, to: number, snapshot: EntityMotionSnapshot, now: number) {
  const remaining = remainingMotionRatio(snapshot, now);
  if (remaining <= 0) {
    return to;
  }

  return to + (from - to) * remaining;
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
