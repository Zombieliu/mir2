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

export type OriginalSceneFrameSetAction = {
  actionId: number;
  actionName: string | null;
  start: number;
  count: number;
  skip: number;
  interval: number;
  effectStart: number;
  effectCount: number;
  effectSkip: number;
  effectInterval: number;
  reverse: boolean;
  blend: boolean;
};

export type OriginalSceneFrameSet = {
  count: number;
  actions: OriginalSceneFrameSetAction[];
};

type OriginalSceneSpriteLibraryPayload = {
  version: number;
  count: number;
  frameSet?: OriginalSceneFrameSet | null;
  frames: OriginalSceneSpriteFrameMeta[];
};

type OriginalSceneSpriteManifestPayload = {
  libraries?: Record<string, unknown>;
};

export type OriginalSceneSpriteLibraryMeta = OriginalSceneSpriteLibraryPayload & {
  frameSet: OriginalSceneFrameSet | null;
  frameMap: Map<number, OriginalSceneSpriteFrameMeta>;
};

type OriginalSceneFrameSetCatalog = {
  libraries?: Record<string, { actionCount?: number; actions?: OriginalSceneFrameSetAction[] }>;
};

const SCENE_SPRITE_LIBRARY_CACHE_MAX_BYTES = 8 * 1024 * 1024;
const libraryCache = new Map<string, { promise: Promise<OriginalSceneSpriteLibraryMeta>; bytes: number }>();
let libraryCacheBytes = 0;
let frameSetCatalogPromise: Promise<OriginalSceneFrameSetCatalog | null> | null = null;
// Libraries that returned a definitive "not available" (4xx) for this origin —
// e.g. source-only libraries that were never exported to the asset CDN. The
// scene renderer requests a library's metadata on every frame that needs it,
// so without remembering these the same missing library is re-fetched
// continuously, flooding the console with 404s and wasting requests. Transient
// failures (5xx / network errors) are intentionally not recorded here so they
// stay retryable.
const missingSceneSpriteLibraries = new Set<string>();
const availableSceneSpriteLibraries = new Set(
  Object.keys((originalSceneSpriteManifest as OriginalSceneSpriteManifestPayload).libraries ?? {}).map(
    normalizeSceneSpriteLibraryKey,
  ),
);
const sourceSceneSpriteLibraries = new Set(
  process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL?.trim()
    ? Object.keys((originalSceneSpriteSourceIndex as OriginalSceneSpriteManifestPayload).libraries ?? {})
        .map(normalizeSceneSpriteLibraryKey)
        .filter((libraryKey) => !libraryKey.startsWith("Map/"))
    : [],
);
// True when an R2 asset base is configured, so the asset Service Worker can backfill
// frame PNGs that are absent same-origin (mir2-asset-worker.js). The kept actor libraries
// (CArmour/Monster/...) ship only their movement frames same-origin; the action frames
// (attack/struck/die/dead) live on the R2 release. Without an R2 base those PNGs would 404,
// so we only reach for the fuller R2 meta when this is set.
const REMOTE_ASSET_BASE_CONFIGURED =
  typeof process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL === "string" &&
  process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL.trim().length > 0;

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
  if (missingSceneSpriteLibraries.has(normalizedKey)) {
    return Promise.reject(new Error(`sprite meta ${normalizedKey} is not available`));
  }
  if (!originalSceneSpriteLibraryExists(normalizedKey)) {
    return Promise.reject(new Error(`sprite meta ${normalizedKey} is not exported`));
  }

  const pending = fetchOriginalSceneSpriteMeta(normalizedKey)
    .then(async (response) => {
      if (!response.ok) {
        if (response.status >= 400 && response.status < 500) {
          missingSceneSpriteLibraries.add(normalizedKey);
        }
        throw new Error(`sprite meta ${normalizedKey} returned ${response.status}`);
      }

      const payload = (await response.json()) as OriginalSceneSpriteLibraryPayload;
      const frameSet = normalizeFrameSet(payload.frameSet) ?? await loadCatalogFrameSet(normalizedKey);
      const library = {
        ...payload,
        frameSet,
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

export async function fetchOriginalSceneSpriteMeta(normalizedKey: string) {
  const staticResponse = await fetch(`/original-ui/${normalizedKey}/meta.json`);
  if (staticResponse.ok) {
    // The kept same-origin actor libraries can ship a TRUNCATED meta (movement frames
    // only). When it is incomplete AND an R2 base is configured, fall through to the API
    // route, which resolves the complete meta from R2; the missing-frame PNGs are then
    // backfilled by the asset Service Worker. Without an R2 base we keep the truncated meta
    // (those PNGs would 404) and skip the redundant fetch.
    if (!REMOTE_ASSET_BASE_CONFIGURED || (await spriteMetaResponseIsComplete(staticResponse))) {
      return staticResponse;
    }
  } else if (!sourceSceneSpriteLibraries.has(normalizedKey)) {
    return staticResponse;
  }

  return fetch(`/api/original-ui-meta?library=${encodeURIComponent(normalizedKey)}`);
}

// Peek a meta response without consuming the caller's body: truncated actor metas declare
// `count` (the full frame total) but list fewer `frames`. Unparseable bodies are treated as
// complete so an odd payload never breaks the same-origin fast path.
async function spriteMetaResponseIsComplete(response: Response) {
  try {
    const meta = (await response.clone().json()) as { count?: number; frames?: unknown[] };
    const count = typeof meta.count === "number" ? meta.count : 0;
    const frames = Array.isArray(meta.frames) ? meta.frames.length : 0;
    return frames >= count;
  } catch {
    return true;
  }
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
  return 256 + library.frames.length * 220 + JSON.stringify(library.frames).length + JSON.stringify(library.frameSet).length;
}

function normalizeFrameSet(frameSet: OriginalSceneFrameSet | null | undefined): OriginalSceneFrameSet | null {
  if (!frameSet || !Array.isArray(frameSet.actions) || frameSet.actions.length === 0) return null;
  return { count: frameSet.actions.length, actions: frameSet.actions };
}

async function loadCatalogFrameSet(normalizedKey: string): Promise<OriginalSceneFrameSet | null> {
  if (!frameSetCatalogPromise) {
    frameSetCatalogPromise = fetch("/original-ui/frame-sets.generated.json")
      .then(async (response) => response.ok ? await response.json() as OriginalSceneFrameSetCatalog : null)
      .catch(() => null);
  }
  const catalog = await frameSetCatalogPromise;
  const entry = catalog?.libraries?.[normalizedKey];
  if (!entry || !Array.isArray(entry.actions) || entry.actions.length === 0) return null;
  return { count: entry.actions.length, actions: entry.actions };
}
