import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

// Packs each Mir2 MAP LIBRARY's exported per-frame PNGs (public/original-map/<Lib>/<frame>.png)
// into texture-atlas PAGES, mirroring Crystal's one-indexed-.Lib-per-library model. Collapses
// the ~450-510 per-frame R2 GETs/viewport into a handful of per-library atlas page fetches that
// are immutable-cached forever. Only frames already exported to disk (= frames the maps use) are
// packed, so atlas size stays bounded. Rect keys are "<library>#<frame>" so the runtime can look
// up a (library, frame) — derived from the existing per-tile PNG path — directly.

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const PUBLIC_ROOT = path.join(WEB_ROOT, "public");
const ORIGINAL_MAP_ROOT = path.join(PUBLIC_ROOT, "original-map");
const DEFAULT_OUT_DIR = path.join(PUBLIC_ROOT, "generated", "map-atlas");

const ATLAS_PADDING = 1;
const INITIAL_WIDTH = 1024;
export const MAX_SIZE = 4096;

const args = parseArgs(process.argv.slice(2));
const outDir = path.resolve(args.outDir ?? DEFAULT_OUT_DIR);
// Optional comma-separated allowlist of library keys (e.g. "WemadeMir2/Tiles,WemadeMir2/Objects").
// Default: every leaf library directory under original-map that contains PNGs.
const onlyLibraries = parseListArg(args.libraries, null);

async function main() {
  // `--skipIfPresent` (used by the `dev` hook) short-circuits when a manifest already exists, so a
  // warm `npm run dev` doesn't pay the ~48s repack every start. CI/`build` calls it without the flag
  // to always produce a fresh atlas from the committed source PNGs.
  if (args.skipIfPresent) {
    const existingManifest = path.join(outDir, "manifest.json");
    if (await exists(existingManifest)) {
      try {
        const manifest = JSON.parse(await fs.readFile(existingManifest, "utf8"));
        if (mapAtlasManifestFitsBudget(manifest)) {
          console.log(JSON.stringify({ ok: true, skipped: true, reason: "compatible manifest already present", manifestPath: existingManifest }));
          return;
        }
        console.warn(`[mir2-map-atlas] rebuilding incompatible manifest: ${existingManifest}`);
      } catch {
        console.warn(`[mir2-map-atlas] rebuilding unreadable manifest: ${existingManifest}`);
      }
    }
  }

  const libraries = await discoverLibraries();
  const selected = onlyLibraries
    ? libraries.filter((lib) => onlyLibraries.includes(lib.libraryKey))
    : libraries;
  if (!selected.length) {
    throw new Error("No map-library PNG directories found under public/original-map");
  }

  await fs.mkdir(outDir, { recursive: true });
  const atlases = [];
  let totalSources = 0;
  let totalImageBytes = 0;

  for (const lib of selected) {
    const sources = await collectLibrarySources(lib);
    if (!sources.length) continue;
    totalSources += sources.length;

    // One library may need multiple pages if its frames exceed the texture budget.
    const pages = packIntoPages(sources);
    for (let pageIndex = 0; pageIndex < pages.length; pageIndex += 1) {
      const page = pages[pageIndex];
      const atlasKey = `map:${lib.libraryKey}#p${pageIndex}`;
      const safeName = `${lib.libraryKey.replace(/[^a-z0-9]+/gi, "-")}-p${pageIndex}.png`;
      const imageRelDir = path.join("generated", "map-atlas", lib.dirParts.join("-"));
      const imageAbsDir = path.join(PUBLIC_ROOT, imageRelDir);
      await fs.mkdir(imageAbsDir, { recursive: true });
      const imageAbsPath = path.join(imageAbsDir, `p${pageIndex}.png`);
      const imageUrl = `/${path.join(imageRelDir, `p${pageIndex}.png`).split(path.sep).join("/")}`;

      await sharp({
        create: { width: page.width, height: page.height, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } },
      })
        .composite(page.sources.map((s) => ({ input: s.filePath, left: s.x, top: s.y })))
        .png({ compressionLevel: 9, adaptiveFiltering: true })
        .toFile(imageAbsPath);

      const imageStat = await fs.stat(imageAbsPath);
      totalImageBytes += imageStat.size;
      void safeName;
      atlases.push({
        key: atlasKey,
        library: lib.libraryKey,
        page: pageIndex,
        width: page.width,
        height: page.height,
        sourceCount: page.sources.length,
        imageBytes: imageStat.size,
        imageUrl,
        rects: page.sources
          .map((s) => ({ key: `${lib.libraryKey}#${s.frame}`, x: s.x, y: s.y, width: s.width, height: s.height }))
          .sort((a, b) => a.key.localeCompare(b.key)),
      });
    }
  }

  const manifestPath = path.join(outDir, "manifest.json");
  const manifest = {
    schemaVersion: 1,
    kind: "mir2-map-atlas-manifest",
    generatedAt: args.generatedAt ?? null, // stamped by caller/CI; null keeps the file deterministic
    atlases: atlases.sort((a, b) => a.key.localeCompare(b.key)),
    stats: {
      libraryCount: new Set(atlases.map((a) => a.library)).size,
      atlasPageCount: atlases.length,
      sourceCount: totalSources,
      imageBytes: totalImageBytes,
    },
  };
  await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");

  console.log(
    JSON.stringify(
      {
        ok: true,
        libraryCount: manifest.stats.libraryCount,
        atlasPageCount: manifest.stats.atlasPageCount,
        sourceCount: totalSources,
        imageBytes: totalImageBytes,
        manifestPath,
      },
      null,
      2,
    ),
  );
}

// A library = the deepest directory under original-map that directly contains <number>.png files,
// e.g. original-map/WemadeMir2/Tiles -> libraryKey "WemadeMir2/Tiles".
async function discoverLibraries() {
  const libraries = [];
  async function walk(absDir, relParts) {
    const entries = await fs.readdir(absDir, { withFileTypes: true });
    const hasPng = entries.some((e) => e.isFile() && e.name.toLowerCase().endsWith(".png"));
    if (hasPng) {
      libraries.push({ libraryKey: relParts.join("/"), dir: absDir, dirParts: relParts });
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        await walk(path.join(absDir, entry.name), [...relParts, entry.name]);
      }
    }
  }
  if (await exists(ORIGINAL_MAP_ROOT)) {
    const roots = await fs.readdir(ORIGINAL_MAP_ROOT, { withFileTypes: true });
    for (const root of roots) {
      if (root.isDirectory()) await walk(path.join(ORIGINAL_MAP_ROOT, root.name), [root.name]);
    }
  }
  return libraries.sort((a, b) => a.libraryKey.localeCompare(b.libraryKey));
}

async function collectLibrarySources(lib) {
  const entries = await fs.readdir(lib.dir, { withFileTypes: true });
  const sources = [];
  for (const entry of entries) {
    if (!entry.isFile() || !entry.name.toLowerCase().endsWith(".png")) continue;
    const frame = entry.name.replace(/\.png$/i, "");
    if (!/^\d+$/.test(frame)) continue; // map frames are numeric; skip anything else
    const filePath = path.join(lib.dir, entry.name);
    const metadata = await sharp(filePath).metadata();
    const width = positiveInteger(metadata.width);
    const height = positiveInteger(metadata.height);
    if (!width || !height) continue;
    sources.push({ filePath, frame, width, height });
  }
  return sources;
}

// Shelf-pack into one or more pages (each <= MAX_SIZE). Tall/wide frames that don't fit a row
// wrap to the next row; when the page height would exceed MAX_SIZE, start a new page.
export function packIntoPages(sources) {
  const sorted = [...sources].sort((a, b) => b.height - a.height || b.width - a.width || a.frame.localeCompare(b.frame));
  const widest = sorted.reduce((max, s) => Math.max(max, s.width + ATLAS_PADDING * 2), 1);
  const width = Math.min(MAX_SIZE, Math.max(INITIAL_WIDTH, nextPowerOfTwo(widest)));

  const pages = [];
  let current = newPage();
  const flush = () => {
    if (!current.sources.length) return;
    // cursorY can already point at the next empty row when the previous row
    // exactly fills a 4096px page. Size from the actual used extent instead.
    current.height = nextPowerOfTwo(current.contentBottom + ATLAS_PADDING);
    current.width = width;
    pages.push(current);
  };

  for (const s of sorted) {
    if (s.width + ATLAS_PADDING * 2 > width || s.height + ATLAS_PADDING * 2 > MAX_SIZE) {
      throw new Error(`Frame ${s.filePath} (${s.width}x${s.height}) exceeds ${MAX_SIZE}px atlas budget`);
    }
    if (current.cursorX + s.width + ATLAS_PADDING > width) {
      current.cursorX = ATLAS_PADDING;
      current.cursorY += current.rowHeight + ATLAS_PADDING;
      current.rowHeight = 0;
    }
    // Would this row overflow the page? Roll to a new page.
    if (current.cursorY + s.height + ATLAS_PADDING > MAX_SIZE) {
      flush();
      current = newPage();
    }
    current.sources.push({ ...s, x: current.cursorX, y: current.cursorY });
    current.contentBottom = Math.max(current.contentBottom, current.cursorY + s.height);
    current.cursorX += s.width + ATLAS_PADDING;
    current.rowHeight = Math.max(current.rowHeight, s.height);
  }
  flush();
  return pages;
}

export function mapAtlasManifestFitsBudget(manifest, maxSize = MAX_SIZE) {
  return Boolean(
    manifest &&
      Array.isArray(manifest.atlases) &&
      manifest.atlases.length > 0 &&
      manifest.atlases.every(
        (atlas) =>
          Number.isFinite(atlas?.width) &&
          Number.isFinite(atlas?.height) &&
          atlas.width > 0 &&
          atlas.height > 0 &&
          atlas.width <= maxSize &&
          atlas.height <= maxSize,
      ),
  );
}

function newPage() {
  return {
    sources: [],
    cursorX: ATLAS_PADDING,
    cursorY: ATLAS_PADDING,
    rowHeight: 0,
    contentBottom: 0,
    width: 0,
    height: 0,
  };
}

function nextPowerOfTwo(value) {
  return 2 ** Math.ceil(Math.log2(Math.max(1, value)));
}
function positiveInteger(value) {
  return Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0;
}
async function exists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}
function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (!value.startsWith("--")) continue;
    const [rawKey, inlineValue] = value.slice(2).split("=", 2);
    const key = rawKey.trim();
    if (!key) continue;
    parsed[key] = inlineValue !== undefined ? inlineValue : argv[index + 1] && !argv[index + 1].startsWith("--") ? argv[++index] : "true";
  }
  return parsed;
}
function parseListArg(value, fallback) {
  if (!value || value === "true") return fallback;
  return String(value).split(",").map((e) => e.trim()).filter(Boolean);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
