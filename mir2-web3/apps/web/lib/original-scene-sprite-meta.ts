import originalSceneSpriteManifest from "../public/original-ui/manifest.generated.json";
import originalSceneSpriteSourceIndex from "../public/original-ui/source-libraries.generated.json";

export type OriginalSceneSpriteFrameMeta = {
  index: number;
  width: number;
  height: number;
  x: number;
  y: number;
  shadowX: number;
  shadowY: number;
  path: string;
  maskPath: string | null;
};

type OriginalSceneSpriteLibraryPayload = {
  version: number;
  count: number;
  frames: OriginalSceneSpriteFrameMeta[];
};

type OriginalSceneSpriteManifestPayload = {
  libraries?: Record<string, unknown>;
};

export type OriginalSceneSpriteLibraryMeta = OriginalSceneSpriteLibraryPayload & {
  frameMap: Map<number, OriginalSceneSpriteFrameMeta>;
};

const SCENE_SPRITE_LIBRARY_CACHE_MAX_BYTES = 8 * 1024 * 1024;
const libraryCache = new Map<string, { promise: Promise<OriginalSceneSpriteLibraryMeta>; bytes: number }>();
let libraryCacheBytes = 0;
const availableSceneSpriteLibraries = new Set(
  Object.keys((originalSceneSpriteManifest as OriginalSceneSpriteManifestPayload).libraries ?? {}).map(
    normalizeSceneSpriteLibraryKey,
  ),
);
const sourceSceneSpriteLibraries = new Set(
  Object.keys((originalSceneSpriteSourceIndex as OriginalSceneSpriteManifestPayload).libraries ?? {})
    .map(normalizeSceneSpriteLibraryKey)
    .filter((libraryKey) => !libraryKey.startsWith("Map/")),
);

export function normalizeSceneSpriteLibraryKey(libraryKey: string) {
  return libraryKey.replaceAll("\\", "/");
}

export function originalSceneSpriteLibraryExists(libraryKey: string) {
  const normalizedKey = normalizeSceneSpriteLibraryKey(libraryKey);
  return availableSceneSpriteLibraries.has(normalizedKey) || sourceSceneSpriteLibraries.has(normalizedKey);
}

export function loadOriginalSceneSpriteLibrary(
  libraryKey: string,
): Promise<OriginalSceneSpriteLibraryMeta> {
  const normalizedKey = normalizeSceneSpriteLibraryKey(libraryKey);
  const cached = libraryCache.get(normalizedKey);
  if (cached) {
    libraryCache.delete(normalizedKey);
    libraryCache.set(normalizedKey, cached);
    return cached.promise;
  }
  if (!originalSceneSpriteLibraryExists(normalizedKey)) {
    return Promise.reject(new Error(`sprite meta ${normalizedKey} is not exported`));
  }

  const pending = fetchOriginalSceneSpriteMeta(normalizedKey)
    .then(async (response) => {
      if (!response.ok) {
        throw new Error(`sprite meta ${normalizedKey} returned ${response.status}`);
      }

      const payload = (await response.json()) as OriginalSceneSpriteLibraryPayload;
      const library = {
        ...payload,
        frameMap: new Map(payload.frames.map((frame) => [frame.index, frame])),
      };
      updateOriginalSceneSpriteLibraryCacheBytes(normalizedKey, estimateOriginalSceneSpriteLibraryBytes(library));
      return library;
    })
    .catch((error) => {
      const deleted = libraryCache.get(normalizedKey);
      if (deleted) {
        libraryCacheBytes -= deleted.bytes;
        libraryCache.delete(normalizedKey);
      }
      throw error;
    });

  libraryCache.set(normalizedKey, { promise: pending, bytes: 0 });
  return pending;
}

async function fetchOriginalSceneSpriteMeta(normalizedKey: string) {
  if (availableSceneSpriteLibraries.has(normalizedKey)) {
    const staticResponse = await fetch(`/original-ui/${normalizedKey}/meta.json`);
    if (staticResponse.ok || !sourceSceneSpriteLibraries.has(normalizedKey)) {
      return staticResponse;
    }
  }

  return fetch(`/api/original-ui-meta?library=${encodeURIComponent(normalizedKey)}`);
}

export function frameMetaForIndex(
  library: OriginalSceneSpriteLibraryMeta | null | undefined,
  frameIndex: number,
) {
  return library?.frameMap.get(frameIndex) ?? null;
}

export function originalSceneSpriteLibraryCacheStats() {
  return {
    loadedLibraryCount: Array.from(libraryCache.values()).filter((entry) => entry.bytes > 0).length,
    cachedLibraryCount: libraryCache.size,
    cachedLibraryBytes: libraryCacheBytes,
  };
}

function updateOriginalSceneSpriteLibraryCacheBytes(normalizedKey: string, bytes: number) {
  const entry = libraryCache.get(normalizedKey);
  if (!entry) return;
  libraryCacheBytes -= entry.bytes;
  entry.bytes = bytes;
  libraryCacheBytes += bytes;
  trimOriginalSceneSpriteLibraryCache();
}

function trimOriginalSceneSpriteLibraryCache() {
  while (libraryCacheBytes > SCENE_SPRITE_LIBRARY_CACHE_MAX_BYTES && libraryCache.size > 1) {
    const oldestKey = libraryCache.keys().next().value as string | undefined;
    if (!oldestKey) break;
    const oldest = libraryCache.get(oldestKey);
    libraryCache.delete(oldestKey);
    libraryCacheBytes -= oldest?.bytes ?? 0;
  }
}

function estimateOriginalSceneSpriteLibraryBytes(library: OriginalSceneSpriteLibraryMeta) {
  return 256 + library.frames.length * 220 + JSON.stringify(library.frames).length;
}
