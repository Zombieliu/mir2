import type { RenderTier } from "./render-tier";

const MEBIBYTE = 1024 * 1024;
const DEFAULT_RELEASE_WATERMARK = 0.85;

export type MapTextureResidency = {
  key: string;
  byteSize: number;
  lastUsedAt: number;
};

export type MapTextureBudgetSignals = {
  tier: RenderTier;
  deviceMemoryGiB: number | null;
  maxTextureSize: number;
};

export type MapTextureEvictionPlan = {
  evictKeys: string[];
  bytesBefore: number;
  bytesAfter: number;
  pinnedBytes: number;
};

export function estimateRgba8TextureBytes(width: number, height: number) {
  return Math.max(0, Math.floor(width)) * Math.max(0, Math.floor(height)) * 4;
}

export function mapTextureResidencyBytes(
  records: Iterable<MapTextureResidency>,
) {
  let total = 0;
  for (const record of records) {
    total += Math.max(0, record.byteSize);
  }
  return total;
}

export function resolveMapTextureByteBudget({
  tier,
  deviceMemoryGiB,
  maxTextureSize,
}: MapTextureBudgetSignals) {
  if (tier === "low") {
    // Keep the 2 GiB escape hatch conservative. Normal 3-4 GiB devices can
    // retain six 1024x4096 RGBA8 pages without consuming unbounded GPU memory.
    const budgetMiB =
      deviceMemoryGiB !== null && deviceMemoryGiB <= 2 ? 64 : 96;
    return budgetMiB * MEBIBYTE;
  }
  if (tier === "medium") {
    return (maxTextureSize <= 4096 ? 144 : 160) * MEBIBYTE;
  }
  return 256 * MEBIBYTE;
}

export function planMapTextureEvictions(
  records: Iterable<MapTextureResidency>,
  pinnedKeys: ReadonlySet<string>,
  maxBytes: number,
  releaseWatermark = DEFAULT_RELEASE_WATERMARK,
): MapTextureEvictionPlan {
  const entries = [...records];
  const bytesBefore = mapTextureResidencyBytes(entries);
  const pinnedBytes = mapTextureResidencyBytes(
    entries.filter((record) => pinnedKeys.has(record.key)),
  );
  const normalizedMaxBytes = Math.max(0, maxBytes);
  if (bytesBefore <= normalizedMaxBytes) {
    return { evictKeys: [], bytesBefore, bytesAfter: bytesBefore, pinnedBytes };
  }

  const normalizedWatermark = Math.max(0, Math.min(1, releaseWatermark));
  const targetBytes = Math.max(
    pinnedBytes,
    Math.floor(normalizedMaxBytes * normalizedWatermark),
  );
  const candidates = entries
    .filter((record) => !pinnedKeys.has(record.key))
    .sort(
      (left, right) =>
        left.lastUsedAt - right.lastUsedAt || left.key.localeCompare(right.key),
    );

  const evictKeys: string[] = [];
  let bytesAfter = bytesBefore;
  for (const record of candidates) {
    if (bytesAfter <= targetBytes) break;
    evictKeys.push(record.key);
    bytesAfter -= Math.max(0, record.byteSize);
  }

  return { evictKeys, bytesBefore, bytesAfter, pinnedBytes };
}
