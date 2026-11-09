import type { ProjectileState } from "./world-model/types";

export const R16_TARGETED_CAST_MS = 600;
const IMPACT_MS = 600;
const TILE_TRAVEL_MS = 50;
const MISSILE_STEP_MS = 30;

export type R16TargetedSpell = "GreatFireBall" | "SoulFireBall";

export type R16TargetedProjectile = {
  spell: R16TargetedSpell;
} & (
  | { phase: "pending" }
  | { phase: "projectile"; direction: number; bound: boolean }
  | { phase: "impact" }
);

type Tile = { x: number; y: number };
type Target = Tile & { objectId: string; dead?: boolean; dieUntil?: number };
type PhaseSound = { spell: R16TargetedSpell; phase: "projectile" | "impact" };

export function r16TargetedSpell(value: unknown): R16TargetedSpell | null {
  if (value === "GreatFireBall" || value === 34 || value === "34") return "GreatFireBall";
  if (value === "SoulFireBall" || value === 64 || value === "64") return "SoulFireBall";
  return null;
}

export function createR16TargetedProjectile(
  key: string,
  spell: R16TargetedSpell,
  attackerId: string,
  targetId: string,
  from: Tile,
  to: Tile,
  castAt: number,
): ProjectileState {
  const startedAt = castAt + R16_TARGETED_CAST_MS;
  const travelEndsAt = startedAt + travelMs(from.x, from.y, to);
  return {
    key, attackerId, targetId, spellOrEffect: spell,
    fromX: from.x, fromY: from.y, toX: to.x, toY: to.y,
    startedAt, travelEndsAt, expiresAt: travelEndsAt + IMPACT_MS,
    r16Targeted: { spell, phase: "pending" },
  };
}

function travelMs(fromX: number, fromY: number, to: Tile) {
  return Math.max(1, Math.max(Math.abs(to.x - fromX), Math.abs(to.y - fromY)) * TILE_TRAVEL_MS);
}

function direction16(fromX: number, fromY: number, to: Tile) {
  if (fromX === to.x && fromY === to.y) return 0;
  const degrees = Math.atan2(to.x - fromX, fromY - to.y) * 180 / Math.PI;
  return (Math.round(degrees / 22.5) + 16) % 16;
}

export function r16ProjectileFrameInterval(duration: number) {
  const frameCount = Math.max(1, Math.floor(duration / MISSILE_STEP_MS));
  return Math.max(1, Math.floor(duration / frameCount));
}

/** Advance on authoritative position changes and at the next phase deadline. */
export function advanceR16TargetedProjectiles(
  projectiles: ProjectileState[],
  entities: readonly Target[],
  now: number,
) {
  const next: ProjectileState[] = [];
  const sounds: PhaseSound[] = [];
  let deadline = Infinity;
  let changed = false;
  for (const entry of projectiles) {
    let projectile = entry;
    let targeted = projectile.r16Targeted;
    if (!targeted) {
      next.push(entry);
      continue;
    }
    if (targeted.phase === "pending" && now < projectile.startedAt) {
      deadline = Math.min(deadline, projectile.startedAt);
      next.push(entry);
      continue;
    }

    const target = entities.find((entity) => entity.objectId === entry.targetId);
    if (targeted.phase === "pending") {
      // Bind a visible target at launch. Otherwise fly to the packet point only.
      const to = target ?? { x: projectile.toX, y: projectile.toY };
      const travelEndsAt = projectile.startedAt + travelMs(projectile.fromX, projectile.fromY, to);
      targeted = {
        spell: targeted.spell, phase: "projectile", bound: Boolean(target),
        direction: direction16(projectile.fromX, projectile.fromY, to),
      };
      projectile = {
        ...projectile, toX: to.x, toY: to.y, travelEndsAt,
        expiresAt: travelEndsAt + IMPACT_MS, r16Targeted: targeted,
      };
      sounds.push({ spell: targeted.spell, phase: "projectile" });
    }

    if (targeted.phase === "projectile") {
      if (targeted.bound && !target) {
        changed = true;
        continue;
      }
      let travelEndsAt = projectile.travelEndsAt ?? projectile.expiresAt;
      if (targeted.bound && target && (projectile.toX !== target.x || projectile.toY !== target.y)) {
        // Lock the launch direction while following the target's tile in flight.
        if (now < travelEndsAt) {
          travelEndsAt = projectile.startedAt + travelMs(projectile.fromX, projectile.fromY, target);
        }
        projectile = {
          ...projectile, toX: target.x, toY: target.y, travelEndsAt,
          expiresAt: travelEndsAt + IMPACT_MS,
        };
      }
      if (now < travelEndsAt) {
        deadline = Math.min(deadline, travelEndsAt);
        changed ||= projectile !== entry;
        next.push(projectile);
        continue;
      }

      // Crystal's Die action can show the impact. A settled Dead action cannot.
      if (!targeted.bound || !target || (target.dead && (target.dieUntil ?? 0) <= now)) {
        changed = true;
        continue;
      }
      sounds.push({ spell: targeted.spell, phase: "impact" });
      projectile = { ...projectile, r16Targeted: { spell: targeted.spell, phase: "impact" } };
    }

    if (!target || now >= projectile.expiresAt) {
      changed = true;
      continue;
    }
    if (projectile.toX !== target.x || projectile.toY !== target.y) {
      projectile = { ...projectile, toX: target.x, toY: target.y };
    }
    deadline = Math.min(deadline, projectile.expiresAt);
    changed ||= projectile !== entry;
    next.push(projectile);
  }
  return { projectiles: changed ? next : projectiles, deadline: Number.isFinite(deadline) ? deadline : null, sounds };
}
