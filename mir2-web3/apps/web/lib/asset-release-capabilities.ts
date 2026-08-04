export type CrystalFullPackReleaseCapability = {
  enabled: boolean;
  verified: boolean;
  indexPath: string | null;
  contentHash: string | null;
  libraryCount: number | null;
  pageCount: number | null;
};

export type AssetReleaseCapabilities = {
  releaseId: string | null;
  crystalFullPack: CrystalFullPackReleaseCapability;
  mapAtlas: MapAtlasReleaseCapability;
};

export type MapAtlasReleaseCapability = {
  enabled: boolean;
  verified: boolean;
  manifestPath: string | null;
  contentHash: string | null;
  pageCount: number | null;
  maxPageBytes: number | null;
};

declare global {
  interface Window {
    __mir2AssetReleaseCapabilities?: AssetReleaseCapabilities;
  }
}

let pendingCapabilities: Promise<AssetReleaseCapabilities> | null = null;

export function setAssetReleaseCapabilities(value: unknown): AssetReleaseCapabilities {
  const normalized = normalizeAssetReleaseCapabilities(value);
  if (typeof window !== "undefined") {
    window.__mir2AssetReleaseCapabilities = normalized;
  }
  return normalized;
}

export async function loadAssetReleaseCapabilities(
  fetcher: typeof fetch = fetch,
): Promise<AssetReleaseCapabilities> {
  if (typeof window !== "undefined" && window.__mir2AssetReleaseCapabilities) {
    return window.__mir2AssetReleaseCapabilities;
  }
  if (pendingCapabilities) return pendingCapabilities;

  pendingCapabilities = fetcher("/api/asset-manifest", { cache: "force-cache" })
    .then(async (response) => {
      if (!response.ok) {
        throw new Error(`asset manifest returned ${response.status}`);
      }
      const manifest = (await response.json()) as {
        version?: unknown;
        capabilities?: unknown;
      };
      return setAssetReleaseCapabilities({
        releaseId: manifest.version,
        ...(isRecord(manifest.capabilities) ? manifest.capabilities : {}),
      });
    })
    .catch(() => normalizeAssetReleaseCapabilities(null))
    .finally(() => {
      pendingCapabilities = null;
    });
  return pendingCapabilities;
}

export function normalizeAssetReleaseCapabilities(value: unknown): AssetReleaseCapabilities {
  const record = isRecord(value) ? value : {};
  const fullPack = isRecord(record.crystalFullPack) ? record.crystalFullPack : {};
  const contentHash = normalizeHash(fullPack.contentHash);
  const indexPath = normalizeIndexPath(fullPack.indexPath ?? fullPack.path);
  const verified = fullPack.verified === true;
  const enabled = fullPack.enabled === true && verified && Boolean(contentHash && indexPath);
  const mapAtlas = isRecord(record.mapAtlas) ? record.mapAtlas : {};
  const mapAtlasContentHash = normalizeHash(mapAtlas.contentHash);
  const mapAtlasManifestPath = normalizeMapAtlasManifestPath(
    mapAtlas.manifestPath ?? mapAtlas.path,
    mapAtlasContentHash,
  );
  const mapAtlasVerified = mapAtlas.verified === true;
  const mapAtlasEnabled =
    mapAtlas.enabled === true &&
    mapAtlasVerified &&
    Boolean(mapAtlasContentHash && mapAtlasManifestPath);

  return {
    releaseId: typeof record.releaseId === "string" && record.releaseId.trim()
      ? record.releaseId.trim()
      : null,
    crystalFullPack: {
      enabled,
      verified,
      indexPath,
      contentHash,
      libraryCount: normalizePositiveInteger(fullPack.libraryCount),
      pageCount: normalizePositiveInteger(fullPack.pageCount),
    },
    mapAtlas: {
      enabled: mapAtlasEnabled,
      verified: mapAtlasVerified,
      manifestPath: mapAtlasManifestPath,
      contentHash: mapAtlasContentHash,
      pageCount: normalizePositiveInteger(mapAtlas.pageCount),
      maxPageBytes: normalizePositiveInteger(mapAtlas.maxPageBytes),
    },
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function normalizeHash(value: unknown) {
  return typeof value === "string" && /^[a-f0-9]{64}$/i.test(value)
    ? value.toLowerCase()
    : null;
}

function normalizeIndexPath(value: unknown) {
  if (typeof value !== "string") return null;
  const normalized = value.trim();
  return normalized.startsWith("/generated/crystal-packs/full/") ? normalized : null;
}

function normalizeMapAtlasManifestPath(value: unknown, contentHash: string | null) {
  if (typeof value !== "string") return null;
  const normalized = value.trim();
  const match = /^\/generated\/map-atlas\/manifest\.([a-f0-9]{64})\.json$/i.exec(normalized);
  return match && contentHash && match[1].toLowerCase() === contentHash ? normalized : null;
}

function normalizePositiveInteger(value: unknown) {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : null;
}
