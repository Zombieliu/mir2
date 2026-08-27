import type { SceneEffectState } from "./types";

export const CRYSTAL_PLAYER_STRUCK_DURATION_MS = 300;
export const CRYSTAL_PLAYER_DIE_DURATION_MS = 400;
export const CRYSTAL_PLAYER_REVIVE_DURATION_MS = 400;
export const CRYSTAL_PLAYER_REVIVE_EFFECT_DURATION_MS = 2_000;
export const CRYSTAL_PLAYER_REVIVE_EFFECT_NAME = "PlayerRevive";

export type ActorCombatEntity = {
  objectId: string;
  kind: string;
  x: number;
  y: number;
  direction?: string;
  hp?: number;
  dead?: boolean;
  attackAnimation?: string;
  attackStartedAt?: number;
  attackUntil?: number;
  struckStartedAt?: number;
  struckUntil?: number;
  pendingStruck?: ActorPendingStruck;
  dieStartedAt?: number;
  dieUntil?: number;
  deathHandled?: boolean;
  reviveStartedAt?: number;
  reviveUntil?: number;
};

export type ActorLocationPatch = {
  x?: number;
  y?: number;
  direction?: string;
};

export type ActorPendingStruck = ActorLocationPatch & {
  attackerId?: string;
  durationMs: number;
};

/**
 * Crystal drops an ObjectStruck only when the ActionFeed tail already contains
 * Struck. The currently-playing action has already been removed from the feed,
 * so one later real hit may wait behind it; a third hit while that tail exists
 * is the duplicate that must be ignored.
 */
export function actorStruckIsAlreadyPending(
  entity:
    | Pick<ActorCombatEntity, "dead" | "dieUntil" | "reviveUntil" | "pendingStruck">
    | null
    | undefined,
  now: number,
): boolean {
  return entity?.dead === true
    || (typeof entity?.dieUntil === "number" && entity.dieUntil > now)
    || (typeof entity?.reviveUntil === "number" && entity.reviveUntil > now)
    || entity?.pendingStruck !== undefined;
}

export function actorStruckIsActive(
  entity: Pick<ActorCombatEntity, "struckUntil"> | null | undefined,
  now: number,
): boolean {
  return typeof entity?.struckUntil === "number" && entity.struckUntil > now;
}

export function applyActorStruck<T extends ActorCombatEntity>(
  entity: T,
  now: number,
  durationMs: number,
  location: ActorLocationPatch = {},
  attackerId?: string,
): T {
  if (actorStruckIsAlreadyPending(entity, now)) return entity;
  if (actorStruckIsActive(entity, now)) {
    return {
      ...entity,
      pendingStruck: {
        ...location,
        attackerId,
        durationMs,
      },
    };
  }
  return {
    ...entity,
    x: typeof location.x === "number" ? location.x : entity.x,
    y: typeof location.y === "number" ? location.y : entity.y,
    direction: location.direction ?? entity.direction,
    struckStartedAt: now,
    struckUntil: now + durationMs,
    pendingStruck: undefined,
  };
}

/** Consume the one Struck action waiting at the Crystal ActionFeed tail. */
export function advanceActorStruck<T extends ActorCombatEntity>(entity: T, now: number): T {
  const pending = entity.pendingStruck;
  if (!pending || actorStruckIsActive(entity, now)) return entity;
  if (
    entity.dead === true
    || (typeof entity.dieUntil === "number" && entity.dieUntil > now)
    || (typeof entity.reviveUntil === "number" && entity.reviveUntil > now)
  ) {
    return { ...entity, pendingStruck: undefined };
  }
  const startedAt = Math.max(entity.struckUntil ?? now, now);
  return {
    ...entity,
    x: typeof pending.x === "number" ? pending.x : entity.x,
    y: typeof pending.y === "number" ? pending.y : entity.y,
    direction: pending.direction ?? entity.direction,
    struckStartedAt: startedAt,
    struckUntil: startedAt + pending.durationMs,
    pendingStruck: undefined,
  };
}

/** Crystal MapChanged clears the active action feed before re-establishing the self pose. */
export function clearActorActionFeed<T extends ActorCombatEntity>(entity: T): T {
  return {
    ...entity,
    attackAnimation: undefined,
    attackStartedAt: undefined,
    attackUntil: undefined,
    struckStartedAt: undefined,
    struckUntil: undefined,
    pendingStruck: undefined,
    dieStartedAt: undefined,
    dieUntil: undefined,
    deathHandled: false,
    reviveStartedAt: undefined,
    reviveUntil: undefined,
  };
}

export function applyActorDeath<T extends ActorCombatEntity>(
  entity: T,
  now: number,
  durationMs: number,
  location: ActorLocationPatch = {},
): T {
  // ObjectHealth(0) arrives before ObjectDied in the shared Zone transcript.
  // Only an established Die clock proves this corpse packet was already
  // consumed; `dead` alone must not discard ObjectDied's pose or death cue.
  if (entity.deathHandled === true) return entity;
  return {
    ...entity,
    x: typeof location.x === "number" ? location.x : entity.x,
    y: typeof location.y === "number" ? location.y : entity.y,
    direction: location.direction ?? entity.direction,
    hp: 0,
    dead: true,
    deathHandled: true,
    dieStartedAt: now,
    dieUntil: now + durationMs,
    attackAnimation: undefined,
    attackStartedAt: undefined,
    attackUntil: undefined,
    struckStartedAt: undefined,
    struckUntil: undefined,
    pendingStruck: undefined,
    reviveStartedAt: undefined,
    reviveUntil: undefined,
  };
}

/** ObjectHealth owns numeric HP only; Death/ObjectDied own lifecycle state. */
export function applyActorHealth<T extends ActorCombatEntity>(
  entity: T,
  hp: number | undefined,
  maxHp: number | undefined,
): T {
  return {
    ...entity,
    hp,
    maxHp,
  };
}

export function applyActorRevive<T extends ActorCombatEntity>(
  entity: T,
  now: number,
  durationMs: number,
  mode: "standing" | "animated",
): T {
  return {
    ...entity,
    dead: false,
    deathHandled: false,
    dieStartedAt: undefined,
    dieUntil: undefined,
    attackAnimation: undefined,
    attackStartedAt: undefined,
    attackUntil: undefined,
    struckStartedAt: undefined,
    struckUntil: undefined,
    pendingStruck: undefined,
    // Crystal's self Revived handler calls User.SetAction immediately. Remote
    // ObjectRevived instead queues the reverse four-frame Revive action.
    reviveStartedAt: mode === "animated" ? now : undefined,
    reviveUntil: mode === "animated" ? now + durationMs : undefined,
  };
}

/** Exact client-owned Magic2 1220..1239 / 20x100ms revive glow. */
export function createPlayerReviveSceneEffect(
  entity: Pick<ActorCombatEntity, "objectId" | "x" | "y">,
  now: number,
): SceneEffectState {
  return {
    key: `crystal-player-revive:${entity.objectId}`,
    source: "actorEffect",
    spellOrEffect: CRYSTAL_PLAYER_REVIVE_EFFECT_NAME,
    objectId: entity.objectId,
    x: entity.x,
    y: entity.y,
    direction: 0,
    value: 0,
    startedAt: now,
    expiresAt: now + CRYSTAL_PLAYER_REVIVE_EFFECT_DURATION_MS,
  };
}
