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

export type OriginalSceneSpriteLibraryMeta = OriginalSceneSpriteLibraryPayload & {
  frameMap: Map<number, OriginalSceneSpriteFrameMeta>;
};

const libraryCache = new Map<string, Promise<OriginalSceneSpriteLibraryMeta>>();

export function normalizeSceneSpriteLibraryKey(libraryKey: string) {
  return libraryKey.replaceAll("\\", "/");
}

export function loadOriginalSceneSpriteLibrary(
  libraryKey: string,
): Promise<OriginalSceneSpriteLibraryMeta> {
  const normalizedKey = normalizeSceneSpriteLibraryKey(libraryKey);
  const cached = libraryCache.get(normalizedKey);
  if (cached) {
    return cached;
  }

  const pending = fetch(`/original-ui/${normalizedKey}/meta.json`)
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

export function frameMetaForIndex(
  library: OriginalSceneSpriteLibraryMeta | null | undefined,
  frameIndex: number,
) {
  return library?.frameMap.get(frameIndex) ?? null;
}
