import type {
  CrystalFullPackRuntimeIndex,
  CrystalLibraryPackPage,
  CrystalLibraryPackRuntimeIndex,
} from "./crystal-full-pack-index";

export type CrystalFullPackAtlasSource = {
  key: string;
  path: string;
  width: number;
  height: number;
};

export type CrystalFullPackAtlasRect = {
  key: string;
  x: number;
  y: number;
  width: number;
  height: number;
  pageIndex?: number;
};

export type CrystalFullPackAtlasPage = {
  key: string;
  width: number;
  height: number;
  imageUrl: string;
  rectList: CrystalFullPackAtlasRect[];
};

export type CrystalFullPackAtlasSnapshot = {
  key: string;
  sourceKey: string;
  width: number;
  height: number;
  imageUrl: string;
  rects: Record<string, CrystalFullPackAtlasRect>;
  rectList: CrystalFullPackAtlasRect[];
  pages: CrystalFullPackAtlasPage[];
};

export type CrystalFullPackFramePath = {
  libraryKey: string;
  frameIndex: number;
};

type PendingRect = {
  source: CrystalFullPackAtlasSource;
  page: CrystalLibraryPackPage;
  x: number;
  y: number;
  width: number;
  height: number;
};

/**
 * Convert the existing exported-PNG URL convention back to its Crystal .Lib
 * identity. CDN prefixes and encoded spaces are accepted; mask files are not.
 */
export function crystalFullPackFramePath(path: string): CrystalFullPackFramePath | null {
  let pathname: string;
  try {
    pathname = decodeURIComponent(new URL(path, "https://mir2.invalid/").pathname);
  } catch {
    return null;
  }
  const segments = pathname.split("/").filter(Boolean);
  const fileName = segments.at(-1) ?? "";
  const frameMatch = fileName.match(/^(\d+)\.png$/i);
  if (!frameMatch) return null;

  const roots = ["original-ui", "original-effects", "original-map"] as const;
  let root: (typeof roots)[number] | null = null;
  let rootIndex = -1;
  for (const candidate of roots) {
    const index = segments.lastIndexOf(candidate);
    if (index > rootIndex) {
      root = candidate;
      rootIndex = index;
    }
  }
  if (!root || rootIndex < 0 || rootIndex >= segments.length - 1) return null;
  const librarySegments = segments.slice(rootIndex + 1, -1);
  if (!librarySegments.length) return null;
  if (root === "original-map") librarySegments.unshift("Map");
  return {
    libraryKey: librarySegments.join("/"),
    frameIndex: Number(frameMatch[1]),
  };
}

/**
 * Resolve only the pages referenced by the current scene. The full root index
 * is tiny; library manifests and texture pages remain lazy and cacheable.
 */
export async function buildCrystalFullPackAtlasSnapshot(
  runtime: CrystalFullPackRuntimeIndex,
  sources: CrystalFullPackAtlasSource[],
  key: string,
): Promise<CrystalFullPackAtlasSnapshot | null> {
  if (!sources.length) return null;
  const parsed = sources.map((source) => ({ source, identity: crystalFullPackFramePath(source.path) }));
  if (parsed.some(({ identity }) => !identity)) return null;

  const libraryKeys = [...new Set(parsed.map(({ identity }) => identity!.libraryKey))].sort(compareCodePoints);
  const loaded = await Promise.all(
    libraryKeys.map(async (libraryKey) => [libraryKey, await runtime.loadLibrary(libraryKey)] as const),
  );
  const libraries = new Map<string, CrystalLibraryPackRuntimeIndex>();
  for (const [libraryKey, library] of loaded) {
    if (!library) return null;
    libraries.set(libraryKey, library);
  }

  const pending: PendingRect[] = [];
  for (const { source, identity } of parsed) {
    const library = libraries.get(identity!.libraryKey);
    const resolved = library?.resolveFrame(identity!.frameIndex);
    if (!resolved || resolved.noDraw || !resolved.image) return null;
    const rect = resolved.image.rect;
    if (rect.width !== source.width || rect.height !== source.height) return null;
    pending.push({
      source,
      page: resolved.image.page,
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height,
    });
  }

  const referencedPages = new Map<string, CrystalLibraryPackPage>();
  for (const entry of pending) referencedPages.set(stablePageKey(entry.page), entry.page);
  const sortedPages = [...referencedPages.entries()].sort(([left], [right]) => compareCodePoints(left, right));
  const pageIndexByKey = new Map(sortedPages.map(([pageKey], index) => [pageKey, index]));
  const rectsByPage = new Map<number, CrystalFullPackAtlasRect[]>();
  const rectList: CrystalFullPackAtlasRect[] = [];

  for (const entry of pending.sort((left, right) => compareCodePoints(left.source.key, right.source.key))) {
    const pageIndex = pageIndexByKey.get(stablePageKey(entry.page));
    if (pageIndex === undefined) return null;
    const rect: CrystalFullPackAtlasRect = {
      key: entry.source.key,
      x: entry.x,
      y: entry.y,
      width: entry.width,
      height: entry.height,
      ...(pageIndex > 0 ? { pageIndex } : {}),
    };
    rectList.push(rect);
    const pageRects = rectsByPage.get(pageIndex) ?? [];
    pageRects.push(rect);
    rectsByPage.set(pageIndex, pageRects);
  }

  const pages = sortedPages.map(([pageKey, page], pageIndex) => ({
    key: pageKey,
    width: page.width,
    height: page.height,
    imageUrl: page.imageUrl,
    rectList: rectsByPage.get(pageIndex) ?? [],
  }));
  const first = pages[0];
  if (!first) return null;
  return {
    key,
    sourceKey: `crystal-full:${runtime.document.sourceContentHash}`,
    width: first.width,
    height: first.height,
    imageUrl: first.imageUrl,
    rects: Object.fromEntries(rectList.map((rect) => [rect.key, rect])),
    rectList,
    pages,
  };
}

function stablePageKey(page: CrystalLibraryPackPage): string {
  return `crystal-full:${page.sha256 ?? page.key}`;
}

function compareCodePoints(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}
