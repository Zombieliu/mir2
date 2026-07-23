export type RenderTier = "low" | "medium" | "high";

export type RenderTierSignals = {
  forcedTier?: string | null;
  deviceMemoryGiB?: unknown;
  coarsePointer?: boolean;
  maxTextureSize?: number | null;
};

export function normalizeDeviceMemoryGiB(value: unknown): number | null {
  const numericValue = Number(value);
  return Number.isFinite(numericValue) && numericValue > 0 ? numericValue : null;
}

export function resolveRenderTier({
  forcedTier,
  deviceMemoryGiB,
  coarsePointer = false,
  maxTextureSize = null,
}: RenderTierSignals): RenderTier {
  if (forcedTier === "low" || forcedTier === "medium" || forcedTier === "high") {
    return forcedTier;
  }

  const memoryGiB = normalizeDeviceMemoryGiB(deviceMemoryGiB);
  if (
    (memoryGiB !== null && memoryGiB <= 4) ||
    coarsePointer ||
    (maxTextureSize !== null && maxTextureSize <= 4096)
  ) {
    return "low";
  }
  return memoryGiB !== null && memoryGiB > 8 ? "high" : "medium";
}
