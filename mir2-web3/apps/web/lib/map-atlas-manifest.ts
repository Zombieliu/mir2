// Runtime loader for the packed map-library texture atlases produced by
// scripts/build-map-atlas-pack.mjs. Mirrors loadBevyEntityAtlasManifest: fetch the
// manifest once (memoised), and expose a (library#frame) -> atlas-page index so the
// GPU map renderer can resolve any tile — whose (library, frame) is derived from the
// existing per-tile PNG path — to its atlas page + UV rect in O(1).

export type MapAtlasRect = {
  key: string; // "<library>#<frame>", e.g. "WemadeMir2/Tiles#901"
  x: number;
  y: number;
  width: number;
  height: number;
};

export type MapAtlasPage = {
  key: string; // unique atlas-page key, e.g. "map:WemadeMir2/Tiles#p0"
  library: string;
  page: number;
  width: number;
  height: number;
  imageUrl: string;
  rects: MapAtlasRect[];
};

export type MapAtlasManifest = {
  schemaVersion?: number;
  kind?: string;
  atlases?: MapAtlasPage[];
  pages?: MapAtlasCompactPage[];
};

export type MapAtlasCompactPage = {
  l: string;
  p: number;
  w: number;
  h: number;
  b?: number;
  u: string;
  r: Array<[number | string, number, number, number, number]>;
};

export type MapAtlasIndex = {
  pages: Map<string, MapAtlasPage>; // atlasKey -> page
  rectToAtlas: Map<string, string>; // rectKey ("<library>#<frame>") -> atlasKey
  rect: Map<string, MapAtlasRect>; // rectKey -> rect (deduped; first wins)
};

export const MAP_ATLAS_MANIFEST_URL = "/generated/map-atlas/manifest.json";

const manifestPromises = new Map<string, Promise<MapAtlasIndex | null>>();

export function buildMapAtlasIndex(manifest: MapAtlasManifest | null): MapAtlasIndex | null {
  const atlasPages = normalizeMapAtlasPages(manifest);
  if (!atlasPages.length) {
    return null;
  }
  const pages = new Map<string, MapAtlasPage>();
  const rectToAtlas = new Map<string, string>();
  const rect = new Map<string, MapAtlasRect>();
  for (const page of atlasPages) {
    if (!page?.key || !page.imageUrl || !Array.isArray(page.rects)) {
      continue;
    }
    pages.set(page.key, page);
    for (const r of page.rects) {
      if (!r?.key || rectToAtlas.has(r.key)) {
        continue;
      }
      rectToAtlas.set(r.key, page.key);
      rect.set(r.key, r);
    }
  }
  return pages.size ? { pages, rectToAtlas, rect } : null;
}

function normalizeMapAtlasPages(manifest: MapAtlasManifest | null): MapAtlasPage[] {
  if (manifest?.schemaVersion === 2 && Array.isArray(manifest.pages)) {
    return manifest.pages.flatMap((page) => {
      if (
        !page ||
        typeof page.l !== "string" ||
        !page.l ||
        !Number.isInteger(page.p) ||
        page.p < 0 ||
        !Number.isFinite(page.w) ||
        page.w <= 0 ||
        !Number.isFinite(page.h) ||
        page.h <= 0 ||
        typeof page.u !== "string" ||
        !page.u.startsWith("/generated/map-atlas/") ||
        !Array.isArray(page.r)
      ) {
        return [];
      }
      const rects = page.r.flatMap((rect): MapAtlasRect[] => {
        if (
          !Array.isArray(rect) ||
          rect.length !== 5 ||
          !Number.isFinite(Number(rect[0])) ||
          !rect.slice(1).every((value) => Number.isFinite(value) && Number(value) >= 0)
        ) {
          return [];
        }
        return [{
          key: `${page.l}#${Number(rect[0])}`,
          x: Number(rect[1]),
          y: Number(rect[2]),
          width: Number(rect[3]),
          height: Number(rect[4]),
        }];
      });
      return [{
        key: `map:${page.l}#p${page.p}`,
        library: page.l,
        page: page.p,
        width: page.w,
        height: page.h,
        imageUrl: page.u,
        rects,
      }];
    });
  }
  return Array.isArray(manifest?.atlases) ? manifest.atlases : [];
}

/** Fetch + index the map-atlas manifest once. Returns null if absent/unparseable (DOM fallback). */
export function loadMapAtlasIndex(
  manifestUrl = MAP_ATLAS_MANIFEST_URL,
): Promise<MapAtlasIndex | null> {
  const safeManifestUrl = normalizeMapAtlasManifestUrl(manifestUrl);
  const existing = manifestPromises.get(safeManifestUrl);
  if (existing) return existing;

  const pending = (async () => {
    try {
      // The URL is stable while atlas coordinates change whenever the pack is
      // regenerated. Revalidate it so rects cannot outlive their matching PNG.
      const response = await fetch(safeManifestUrl, { cache: "no-cache" });
      if (!response.ok) {
        return null;
      }
      const manifest = (await response.json()) as MapAtlasManifest;
      return buildMapAtlasIndex(manifest);
    } catch {
      return null;
    }
  })();
  manifestPromises.set(safeManifestUrl, pending);
  return pending;
}

function normalizeMapAtlasManifestUrl(value: string) {
  const normalized = value.trim();
  return normalized.startsWith("/generated/map-atlas/") && normalized.endsWith(".json")
    ? normalized
    : MAP_ATLAS_MANIFEST_URL;
}

/** "/original-map/WemadeMir2/Tiles/901.png" -> rect key "WemadeMir2/Tiles#901" (matches the packer). */
export function mapAtlasRectKeyForPath(path: string): string | null {
  const match = path.match(/\/((?:Wemade|Shanda)Mir[23])\/(.+)\/(\d+)\.[a-z0-9]+(?:[?#].*)?$/i);
  if (!match) {
    return null;
  }
  return `${match[1]}/${match[2]}#${match[3]}`;
}

export function mapAtlasPathRequiresAlphaKey(path: string): boolean {
  try {
    const normalized = new URL(path, "https://mir2.invalid/").pathname;
    return (
      normalized.startsWith("/original-map/") &&
      /\/(?:objects(?:_32bit|\d*)?|smobjects\d*|furnitures?c?|walls?c?|animations?c?|houses?c?|cliffs?c?|dungeons?c?|inners?c?|object[12]c)\//i.test(
        normalized,
      )
    );
  } catch {
    return false;
  }
}
