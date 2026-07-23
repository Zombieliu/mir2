import type { RenderTier } from "./render-tier";

export type BackgroundPrewarmMode = "off" | "immediate" | "afterPlayable";

export type AssetPrewarmPolicy = {
  tier: RenderTier;
  criticalConcurrency: number;
  backgroundConcurrency: number;
  maxSceneFrames: number | null;
  backgroundMode: BackgroundPrewarmMode;
};

export function resolveAssetPrewarmPolicy(
  tier: RenderTier,
  backgroundMode?: BackgroundPrewarmMode | null,
): AssetPrewarmPolicy {
  const defaultBackgroundMode: BackgroundPrewarmMode = tier === "low" ? "off" : "afterPlayable";
  const resolvedBackgroundMode = backgroundMode ?? defaultBackgroundMode;

  if (tier === "low") {
    return {
      tier,
      criticalConcurrency: 3,
      backgroundConcurrency: 1,
      maxSceneFrames: 192,
      backgroundMode: resolvedBackgroundMode,
    };
  }
  if (tier === "high") {
    return {
      tier,
      criticalConcurrency: 8,
      backgroundConcurrency: 3,
      maxSceneFrames: null,
      backgroundMode: resolvedBackgroundMode,
    };
  }
  return {
    tier,
    criticalConcurrency: 5,
    backgroundConcurrency: 2,
    maxSceneFrames: 480,
    backgroundMode: resolvedBackgroundMode,
  };
}
