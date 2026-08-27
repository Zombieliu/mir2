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

/**
 * Crystal drops an ObjectStruck while the same struck action is already queued.
 * The web renderer has one transient slot rather than an ActionFeed, so an
 * active struck window is the fail-closed equivalent: it must not restart the
 * animation or replay its two player hit sounds.
 */
export function actorStruckIsAlreadyPending(
  entity: Pick<ActorCombatEntity, "dead" | "dieUntil" | "reviveUntil"> | null | undefined,
  now: number,
): boolean {
  // Crystal removes the active Struck action from ActionFeed before playing
  // it, so another real hit during the three-frame action may queue next. Do
  // not treat the current struck window itself as a duplicate.
  return entity?.dead === true
    || (typeof entity?.dieUntil === "number" && entity.dieUntil > now)
    || (typeof entity?.reviveUntil === "number" && entity.reviveUntil > now);
}

export function applyActorStruck<T extends ActorCombatEntity>(
  entity: T,
  now: number,
  durationMs: number,
  location: ActorLocationPatch = {},
): T {
  return {
    ...entity,
    x: typeof location.x === "number" ? location.x : entity.x,
    y: typeof location.y === "number" ? location.y : entity.y,
    direction: location.direction ?? entity.direction,
    struckStartedAt: now,
    struckUntil: now + durationMs,
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
