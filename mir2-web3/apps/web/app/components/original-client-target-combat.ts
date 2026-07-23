export const LOCKED_MONSTER_ATTACK_TICK_MS = 50;
export const LOCKED_MONSTER_REPATH_MIN_MS = 160;

export type LockedMonsterAttack = {
  objectId: string;
  lastTargetX: number;
  lastTargetY: number;
  nextApproachAt: number;
};

type TargetPoint = {
  x: number;
  y: number;
};

type MonsterTarget = TargetPoint & {
  objectId: string;
  kind: string;
  dead?: boolean;
};

export type LockedMonsterAttackDecision =
  | { kind: "clear" }
  | { kind: "wait"; lock: LockedMonsterAttack }
  | { kind: "approach"; lock: LockedMonsterAttack; destination: TargetPoint }
  | { kind: "attack"; lock: LockedMonsterAttack };

/** Creates the client-side pursuit lock; the server remains authoritative. */
export function createLockedMonsterAttack(
  objectId: string,
  target: TargetPoint,
): LockedMonsterAttack {
  return {
    objectId,
    lastTargetX: target.x,
    lastTargetY: target.y,
    nextApproachAt: 0,
  };
}

export function decideLockedMonsterAttack(input: {
  lock: LockedMonsterAttack;
  selectedObjectId: string | null;
  self: TargetPoint | null;
  target: MonsterTarget | null;
  approachDestination: TargetPoint;
  queuedApproach: TargetPoint | null;
  movementPending: boolean;
  nextAttackAt: number;
  now: number;
}): LockedMonsterAttackDecision {
  const {
    lock,
    selectedObjectId,
    self,
    target,
    approachDestination,
    queuedApproach,
    movementPending,
    nextAttackAt,
    now,
  } = input;

  if (
    !self ||
    !target ||
    target.objectId !== lock.objectId ||
    target.kind !== "monster" ||
    target.dead ||
    selectedObjectId !== lock.objectId
  ) {
    // Selection is part of the lock contract: a ground click or a different
    // target must stop auto-pursuit instead of silently retargeting.
    return { kind: "clear" };
  }

  const targetMoved = target.x !== lock.lastTargetX || target.y !== lock.lastTargetY;
  const nextLock: LockedMonsterAttack = {
    ...lock,
    lastTargetX: target.x,
    lastTargetY: target.y,
  };
  const distance = Math.max(Math.abs(self.x - target.x), Math.abs(self.y - target.y));

  if (distance <= 1) {
    // Do not overlap a movement acknowledgement with an attack intent. This
    // prevents a late movement correction from snapping an attacking player.
    if (movementPending || now < nextAttackAt) {
      return { kind: "wait", lock: nextLock };
    }
    return { kind: "attack", lock: nextLock };
  }

  if (now < nextAttackAt) {
    return { kind: "wait", lock: nextLock };
  }

  const queuedApproachMatches =
    queuedApproach?.x === approachDestination.x && queuedApproach?.y === approachDestination.y;
  if (queuedApproachMatches || (movementPending && !targetMoved)) {
    // A moving target invalidates the old approach destination; a stationary
    // target can reuse the in-flight movement and avoid command spam.
    return { kind: "wait", lock: nextLock };
  }
  if (!targetMoved && now < lock.nextApproachAt) {
    return { kind: "wait", lock: nextLock };
  }

  return {
    kind: "approach",
    destination: approachDestination,
    lock: {
      ...nextLock,
      nextApproachAt: now + LOCKED_MONSTER_REPATH_MIN_MS,
    },
  };
}
