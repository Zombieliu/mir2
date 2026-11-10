import type { CrystalEntityAnimationPose, DisplayEntity, DisplaySceneEffect } from "../app/components/original-client-types";
import type { OriginalSoundEvent } from "./original-sound-events";

export type FishingFeedback = Readonly<{ x: number; y: number; foundFish: boolean }>;

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

type FishingEffect = DisplaySceneEffect & { incarnation?: number };
const NO_EFFECTS: FishingEffect[] = [];
const EFFECT_DURATION_MS = 720;

/** Samples server feedback once at frame one of each shared fishing clip. */
export class FishingFeedbackPresentation {
  private worldKey: string | null = null;
  private effects: FishingEffect[] = NO_EFFECTS;
  private emitted = new Map<string, string>();

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
    }
    const sounds: OriginalSoundEvent[] = [];
    if (worldKey === null) return { effects: this.effects, sounds };

    const live = new Set(entities.filter(entity => !entity.dead && !entity.deathHandled).map(entity => entity.objectId));
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
        created.push({ ...timing, key: `${key}:bite`, source: "fishingBite", spellOrEffect: FISHING_BITE_NOTICE,
          x: entity.x, y: entity.y, objectId, incarnation });
        sounds.push("fishing");
      }
    }
    if (created.length) this.effects = [...this.effects, ...created];
    return { effects: this.effects, sounds };
  }
}
