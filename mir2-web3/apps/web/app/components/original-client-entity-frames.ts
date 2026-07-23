import type {
  OriginalSceneFrameSet,
  OriginalSceneFrameSetAction,
} from "../../lib/original-scene-sprite-meta";
import type { EntitySpriteAnimationState } from "./original-client-types";

export type CrystalEntityAnimationMeta = {
  frameBaseOffset: number;
  frameCount: number;
  directionStride: number;
  frameIntervalMs: number;
  reverse?: boolean;
  blend?: boolean;
  effect?: CrystalEntityEffectAnimationMeta;
};

export type CrystalEntityEffectAnimationMeta = {
  frameBaseOffset: number;
  frameCount: number;
  directionStride: number;
  frameIntervalMs: number;
  reverse?: boolean;
  blend?: boolean;
};

export function crystalFrameSetActionForState(
  frameSet: OriginalSceneFrameSet | null | undefined,
  animationState: EntitySpriteAnimationState,
  attackAnimation?: "melee1" | "melee2" | "melee3" | "melee4" | "range" | "spell",
): OriginalSceneFrameSetAction | null {
  if (!frameSet?.actions.length) return null;
  const byName = new Map(frameSet.actions.map((action) => [action.actionName, action]));
  for (const name of actionNamesForState(animationState, attackAnimation)) {
    const action = byName.get(name);
    if (action) return action;
  }
  return null;
}

export function crystalEntityAnimationMeta(
  frameSet: OriginalSceneFrameSet | null | undefined,
  animationState: EntitySpriteAnimationState,
  frameBaseOffset = 0,
  attackAnimation?: "melee1" | "melee2" | "melee3" | "melee4" | "range" | "spell",
): CrystalEntityAnimationMeta | null {
  const action = crystalFrameSetActionForState(frameSet, animationState, attackAnimation);
  if (!action) return null;
  const reverse = animationState === "reviving" && action.actionName !== "Revive" ? true : action.reverse;
  return {
    frameBaseOffset: frameBaseOffset + action.start,
    frameCount: Math.max(action.count, 1),
    directionStride: action.count + action.skip,
    frameIntervalMs: Math.max(action.interval, 1),
    reverse: reverse || undefined,
    blend: action.blend || undefined,
    ...(action.effectCount > 0
      ? {
          effect: {
            frameBaseOffset: frameBaseOffset + action.effectStart,
            frameCount: action.effectCount,
            directionStride: action.effectCount + action.effectSkip,
            frameIntervalMs: Math.max(action.effectInterval || action.interval, 1),
            reverse: action.reverse || undefined,
            blend: action.blend || undefined,
          },
        }
      : {}),
  };
}

function actionNamesForState(
  animationState: EntitySpriteAnimationState,
  attackAnimation?: "melee1" | "melee2" | "melee3" | "melee4" | "range" | "spell",
) {
  switch (animationState) {
    case "harvesting":
      return ["Harvest", "Standing"];
    case "walking":
      return ["Walking", "Standing"];
    case "running":
      return ["Running", "Walking", "Standing"];
    case "attackMelee":
      return [
        attackAnimation === "melee4"
          ? "Attack4"
          : attackAnimation === "melee3"
            ? "Attack3"
            : attackAnimation === "melee2"
              ? "Attack2"
              : "Attack1",
        "Attack1",
        "Standing",
      ];
    case "attackRange":
      return attackAnimation === "spell"
        ? ["Spell", "AttackRange1", "Attack1", "Standing"]
        : ["AttackRange1", "Attack1", "Standing"];
    case "struck":
      return ["Struck", "Standing"];
    case "dying":
      return ["Die", "Dead", "Standing"];
    case "dead":
      return ["Dead", "Die", "Standing"];
    case "reviving":
      return ["Revive", "Die", "Standing"];
    default:
      return ["Standing"];
  }
}
