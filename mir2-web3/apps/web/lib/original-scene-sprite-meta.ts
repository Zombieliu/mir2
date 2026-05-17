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

const libraryCache = new Map<string, Promise<OriginalSceneSpriteLibraryMeta>>();
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
    return cached;
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
      return {
        ...payload,
        frameMap: new Map(payload.frames.map((frame) => [frame.index, frame])),
      };
    })
    .catch((error) => {
      libraryCache.delete(normalizedKey);
      throw error;
    });

  libraryCache.set(normalizedKey, pending);
  return pending;
}

async function fetchOriginalSceneSpriteMeta(normalizedKey: string) {
  const staticResponse = await fetch(`/original-ui/${normalizedKey}/meta.json`);
  if (staticResponse.ok || !sourceSceneSpriteLibraries.has(normalizedKey)) {
    return staticResponse;
  }

  return fetch(`/api/original-ui-meta?library=${encodeURIComponent(normalizedKey)}`);
}

export function frameMetaForIndex(
  library: OriginalSceneSpriteLibraryMeta | null | undefined,
  frameIndex: number,
) {
  return library?.frameMap.get(frameIndex) ?? null;
}
