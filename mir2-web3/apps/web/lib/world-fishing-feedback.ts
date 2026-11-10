import type { CrystalEntityAnimationPose, DisplayEntity, DisplaySceneEffect } from "../app/components/original-client-types";
import type { OriginalSoundEvent } from "./original-sound-events";

export type FishingFeedback = Readonly<{ x: number; y: number; foundFish: boolean }>;
export type FishingAnimationEdge = Readonly<{ active: boolean; receivedAtMs?: number }>;

const MAX_RETAINED_FISHING_EDGES = 16;

export function appendFishingAnimationEdge(
  history: readonly FishingAnimationEdge[] | undefined,
  previous: boolean | undefined,
  active: boolean,
  receivedAtMs?: number,
): readonly FishingAnimationEdge[] | undefined {
  if (previous === undefined || previous === active) return history;
  const prior = history?.at(-1)?.active === previous
    ? history.slice(1 - MAX_RETAINED_FISHING_EDGES)
    : [{ active: previous }];
  return [...prior, { active, receivedAtMs }];
}

export function readFishingFeedback(payload: Record<string, unknown>): FishingFeedback | undefined {
  const point = payload.fishingPoint as { x?: unknown; y?: unknown } | undefined;
  if (typeof payload.foundFish !== "boolean" || typeof point?.x !== "number" || typeof point.y !== "number"
    || !Number.isSafeInteger(point.x) || !Number.isSafeInteger(point.y) || point.x < 0 || point.y < 0) return;
  return { x: point.x, y: point.y, foundFish: payload.foundFish };
}

export const FISHING_WATER_EFFECTS = {
  ripple: ["FishingRippleInner", "FishingRippleOuter"],
  bite: ["FishingBiteInner", "FishingBiteOuter"],
} as const;
export const FISHING_BITE_NOTICE = "FishingBiteNotice";

export function fishingBiteButtonFrame(now: number, startedAt: number): number {
  return 170 + Math.floor(Math.max(0, now - startedAt) / 130) % 10;
}

type FishingEffect = DisplaySceneEffect & { incarnation?: number };
const NO_EFFECTS: FishingEffect[] = [];
const EFFECT_DURATION_MS = 720;

function isCurrentSelfBite(entity: DisplayEntity | undefined, pose: CrystalEntityAnimationPose | undefined) {
  if (entity?.kind !== "selfPlayer" || !entity.fishing || !entity.fishingFeedback?.foundFish
    || pose?.action !== "fishingWait" || pose.fishingStartedAtMs === undefined) return false;
  // A recast can be acknowledged while the previous Wait clip still runs.
  const castReceivedAt = entity.fishingEdges?.at(-1)?.receivedAtMs;
  return castReceivedAt === undefined || pose.fishingStartedAtMs >= castReceivedAt;
}

/** Samples server feedback once at frame one of each shared fishing clip. */
export class FishingFeedbackPresentation {
  private worldKey: string | null = null;
  private effects: FishingEffect[] = NO_EFFECTS;
  private emitted = new Map<string, string>();
  private selfBite: { objectId: string; incarnation: number; startedAt: number } | null = null;

  get biteStartedAt(): number | null { return this.selfBite?.startedAt ?? null; }

  advance(
    worldKey: string | null,
    now: number,
    entities: readonly DisplayEntity[],
    poses: Readonly<Record<string, CrystalEntityAnimationPose>>,
  ): { effects: readonly DisplaySceneEffect[]; sounds: OriginalSoundEvent[] } {
    if (worldKey !== this.worldKey) {
      this.worldKey = worldKey;
      this.effects = NO_EFFECTS;
      this.emitted.clear();
      this.selfBite = null;
    }
    const sounds: OriginalSoundEvent[] = [];
    if (worldKey === null) return { effects: this.effects, sounds };

    const live = new Set(entities.filter(entity => !entity.dead && !entity.deathHandled).map(entity => entity.objectId));
    if (this.selfBite) {
      const { objectId, incarnation } = this.selfBite;
      const owner = entities.find(entity => entity.objectId === objectId);
      const pose = poses[objectId];
      if (!isCurrentSelfBite(owner, pose) || !live.has(objectId) || pose?.incarnation !== incarnation) this.selfBite = null;
    }
    const remaining = this.effects.filter(effect => effect.expiresAt > now &&
      (!effect.objectId || (live.has(effect.objectId) && poses[effect.objectId]?.incarnation === effect.incarnation)));
    if (remaining.length !== this.effects.length) this.effects = remaining;
    for (const id of this.emitted.keys()) {
      if (!live.has(id) || !poses[id]) this.emitted.delete(id);
    }

    const created: FishingEffect[] = [];
    for (const entity of entities) {
      const { objectId, fishingFeedback } = entity;
      const pose = poses[objectId];
      if (!pose || !live.has(objectId) || pose.logicalFrameIndex < 1) continue;
      const { action, incarnation, fishingStartedAtMs } = pose;
      if (action !== "fishingCast" && action !== "fishingWait" && action !== "fishingReel") continue;
      if (fishingStartedAtMs === undefined || !Number.isSafeInteger(fishingStartedAtMs) || fishingStartedAtMs < 0) continue;
      const startedAt = fishingStartedAtMs + (action === "fishingWait" ? 120 : 100);
      const key = `${objectId}:${incarnation}:${action}:${fishingStartedAtMs}`;
      if (startedAt > now || this.emitted.get(objectId) === key) continue;
      this.emitted.set(objectId, key);

      if (action === "fishingCast" || action === "fishingReel") {
        sounds.push(action === "fishingCast" ? "fishingThrow" : "fishingPull");
        continue;
      }
      if (!fishingFeedback) continue;
      const { x, y, foundFish } = fishingFeedback;
      const timing = { startedAt, expiresAt: startedAt + EFFECT_DURATION_MS, direction: 0, value: 0 };
      for (const name of FISHING_WATER_EFFECTS[foundFish ? "bite" : "ripple"]) {
        created.push({ ...timing, key: `${key}:${name}`, source: "fishingWater", spellOrEffect: name, x, y });
      }
      if (foundFish) {
        if (!this.selfBite && isCurrentSelfBite(entity, pose)) {
          this.selfBite = { objectId, incarnation, startedAt };
        }
        created.push({ ...timing, key: `${key}:bite`, source: "fishingBite", spellOrEffect: FISHING_BITE_NOTICE,
          x: entity.x, y: entity.y, objectId, incarnation });
        sounds.push("fishing");
      }
    }
    if (created.length) this.effects = [...this.effects, ...created];
    return { effects: this.effects, sounds };
  }
}
