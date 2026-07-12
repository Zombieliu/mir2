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
};

export type MapAtlasIndex = {
  pages: Map<string, MapAtlasPage>; // atlasKey -> page
  rectToAtlas: Map<string, string>; // rectKey ("<library>#<frame>") -> atlasKey
  rect: Map<string, MapAtlasRect>; // rectKey -> rect (deduped; first wins)
};

export const MAP_ATLAS_MANIFEST_URL = "/generated/map-atlas/manifest.json";

let manifestPromise: Promise<MapAtlasIndex | null> | null = null;

export function buildMapAtlasIndex(manifest: MapAtlasManifest | null): MapAtlasIndex | null {
  if (!manifest?.atlases?.length) {
    return null;
  }
  const pages = new Map<string, MapAtlasPage>();
  const rectToAtlas = new Map<string, string>();
  const rect = new Map<string, MapAtlasRect>();
  for (const page of manifest.atlases) {
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

/** Fetch + index the map-atlas manifest once. Returns null if absent/unparseable (DOM fallback). */
export function loadMapAtlasIndex(): Promise<MapAtlasIndex | null> {
  if (manifestPromise) {
    return manifestPromise;
  }
  manifestPromise = (async () => {
    try {
      const response = await fetch(MAP_ATLAS_MANIFEST_URL, { cache: "force-cache" });
      if (!response.ok) {
        return null;
      }
      const manifest = (await response.json()) as MapAtlasManifest;
      return buildMapAtlasIndex(manifest);
    } catch {
      return null;
    }
  })();
  return manifestPromise;
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
