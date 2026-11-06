import fs from "node:fs/promises";
import { createHash } from "node:crypto";
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
const PRODUCTION_ASSET_CONFIG = path.resolve(
  WEB_ROOT,
  "..",
  "..",
  "config",
  "production-web-assets.json",
);

const ATLAS_PADDING = 1;
const INITIAL_WIDTH = 1024;
export const MAX_SIZE = 4096;
export const DEFAULT_MAX_PAGE_PIXELS = 1024 * 256;
export const NEAR_BLACK_CHANNEL_MAX = 16;

const args = parseArgs(process.argv.slice(2));
const outDir = path.resolve(args.outDir ?? DEFAULT_OUT_DIR);
const maxPagePixels = positiveInteger(args.maxPagePixels) || DEFAULT_MAX_PAGE_PIXELS;
// Optional comma-separated allowlist of library keys (e.g. "WemadeMir2/Tiles,WemadeMir2/Objects").
// Default: every leaf library directory under original-map that contains PNGs.
const onlyLibraries = parseListArg(args.libraries, null);

async function main() {
  if (isRemoteReleaseBuild()) {
    const pinned = await readVerifiedRemoteMapAtlasRelease();
    console.log(
      JSON.stringify({
        ok: true,
        skipped: true,
        reason: "verified remote map-atlas release is pinned",
        ...pinned,
      }),
    );
    return;
  }

  // `--skipIfPresent` (used by the `dev` hook) short-circuits when a manifest already exists, so a
  // warm `npm run dev` doesn't pay the ~48s repack or source scan every start. CI/`build` calls it
  // without the flag to always inspect and produce a fresh atlas from the committed source PNGs.
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
    : libraries.filter((lib) => mapAtlasLibrarySupportsRawUpload(lib.libraryKey));
  if (!selected.length) {
    throw new Error("No map-library PNG directories found under public/original-map");
  }

  const selectedLibraries = [];
  const qualityFindings = [];
  for (const lib of selected) {
    const sources = await collectLibrarySources(lib);
    if (!sources.length) continue;
    selectedLibraries.push({ lib, sources });
    qualityFindings.push(
      ...(await findUniformOpaqueNearBlackPlaceholderSources(sources)).map((finding) => ({
        ...finding,
        libraryKey: lib.libraryKey,
      })),
    );
  }
  reportMapAtlasSourceQuality(
    qualityFindings,
    path.resolve(WEB_ROOT, "..", ".."),
    args.strictSourceQuality === "true",
  );

  await fs.mkdir(outDir, { recursive: true });
  await removeStaleMapAtlasArtifacts(outDir);
  const atlases = [];
  let totalSources = 0;
  let totalImageBytes = 0;

  for (const { lib, sources } of selectedLibraries) {
    totalSources += sources.length;

    // One library may need multiple pages if its frames exceed the texture budget.
    const pages = packIntoPages(sources, maxPagePixels);
    for (let pageIndex = 0; pageIndex < pages.length; pageIndex += 1) {
      const page = pages[pageIndex];
      const imageAbsDir = path.join(outDir, lib.dirParts.join("-"));
      await fs.mkdir(imageAbsDir, { recursive: true });
      const imageBuffer = await sharp({
        create: { width: page.width, height: page.height, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } },
      })
        .composite(page.sources.map((s) => ({ input: s.filePath, left: s.x, top: s.y })))
        .png({ compressionLevel: 9, adaptiveFiltering: true })
        .toBuffer();
      const imageHash = createHash("sha256").update(imageBuffer).digest("hex");
      const imageFileName = `p${pageIndex}.${imageHash.slice(0, 16)}.png`;
      const imageAbsPath = path.join(imageAbsDir, imageFileName);
      await fs.writeFile(imageAbsPath, imageBuffer);
      const imageRelativePath = path.relative(PUBLIC_ROOT, imageAbsPath);
      if (imageRelativePath.startsWith("..") || path.isAbsolute(imageRelativePath)) {
        throw new Error(`Map atlas output must stay under ${PUBLIC_ROOT}: ${imageAbsPath}`);
      }
      const imageUrl = `/${imageRelativePath.split(path.sep).join("/")}`;

      totalImageBytes += imageBuffer.length;
      atlases.push({
        l: lib.libraryKey,
        p: pageIndex,
        w: page.width,
        h: page.height,
        b: imageBuffer.length,
        u: imageUrl,
        r: page.sources
          .map((s) => [Number(s.frame), s.x, s.y, s.width, s.height])
          .sort((a, b) => a[0] - b[0]),
      });
    }
  }

  const manifestPath = path.join(outDir, "manifest.json");
  const manifest = {
    schemaVersion: 2,
    kind: "mir2-map-atlas-manifest",
    pages: atlases.sort((a, b) => a.l.localeCompare(b.l) || a.p - b.p),
    stats: {
      libraryCount: new Set(atlases.map((a) => a.l)).size,
      atlasPageCount: atlases.length,
      sourceCount: totalSources,
      imageBytes: totalImageBytes,
      maxPageBytes: Math.max(...atlases.map((a) => a.b)),
      maxPagePixels,
    },
  };
  const manifestJson = `${JSON.stringify(manifest)}\n`;
  const contentHash = createHash("sha256").update(manifestJson).digest("hex");
  const releaseManifestPath = path.join(outDir, `manifest.${contentHash}.json`);
  await fs.writeFile(manifestPath, manifestJson, "utf8");
  await fs.writeFile(releaseManifestPath, manifestJson, "utf8");

  console.log(
    JSON.stringify(
      {
        ok: true,
        libraryCount: manifest.stats.libraryCount,
        atlasPageCount: manifest.stats.atlasPageCount,
        sourceCount: totalSources,
        imageBytes: totalImageBytes,
        maxPageBytes: manifest.stats.maxPageBytes,
        maxPagePixels,
        contentHash,
        manifestPath,
        releaseManifestPath,
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

export async function findUniformOpaqueNearBlackPlaceholderSources(sources) {
  const findings = [];
  for (const source of sources) {
    const stats = await sharp(source.filePath).ensureAlpha().stats();
    if (!isUniformOpaqueNearBlackPlaceholder(stats)) continue;
    findings.push({
      filePath: source.filePath,
      frame: source.frame,
      width: source.width,
      height: source.height,
      rgba: stats.channels.slice(0, 4).map((channel) => channel.min),
    });
  }
  return findings;
}

export function isUniformOpaqueNearBlackPlaceholder(stats) {
  const channels = stats?.channels;
  if (!Array.isArray(channels) || channels.length < 4) return false;

  const rgb = channels.slice(0, 3);
  const alpha = channels[3];
  return (
    rgb.every(
      (channel) =>
        Number.isFinite(channel?.min) &&
        Number.isFinite(channel?.max) &&
        channel.min === channel.max &&
        channel.min >= 0 &&
        channel.max <= NEAR_BLACK_CHANNEL_MAX,
    ) &&
    Number.isFinite(alpha?.min) &&
    Number.isFinite(alpha?.max) &&
    alpha.min === 255 &&
    alpha.max === 255
  );
}

export function reportMapAtlasSourceQuality(
  findings,
  repoRoot = path.resolve(WEB_ROOT, "..", ".."),
  strictGroundTiles = false,
) {
  if (!findings.length) return;

  const groundTileFindings = findings.filter((finding) =>
    mapAtlasLibrarySupportsRawUpload(finding.libraryKey),
  );
  const otherFindings = findings.filter(
    (finding) => !mapAtlasLibrarySupportsRawUpload(finding.libraryKey),
  );
  if (otherFindings.length) {
    console.warn(
      [
        `[mir2-map-atlas] warning: found ${otherFindings.length} uniform opaque near-black source image(s) outside ground-tile libraries; continuing without making these known placeholder objects fatal.`,
        ...formatMapAtlasQualityFindingPaths(otherFindings, repoRoot),
      ].join("\n"),
    );
  }
  if (groundTileFindings.length && strictGroundTiles) {
    throw new Error(
      [
        `[mir2-map-atlas] ground-tile quality check failed: found ${groundTileFindings.length} uniform opaque near-black source image(s). Replace or re-export these source PNGs before building the map atlas:`,
        ...formatMapAtlasQualityFindingPaths(groundTileFindings, repoRoot),
      ].join("\n"),
    );
  }
  if (groundTileFindings.length) {
    console.warn(
      [
        `[mir2-map-atlas] warning: found ${groundTileFindings.length} uniform opaque near-black ground-tile source image(s). Use --strictSourceQuality to make this release-blocking:`,
        ...formatMapAtlasQualityFindingPaths(groundTileFindings, repoRoot),
      ].join("\n"),
    );
  }
}

function formatMapAtlasQualityFindingPaths(findings, repoRoot) {
  return findings
    .map((finding) => {
      const relativePath =
        (path.relative(repoRoot, finding.filePath) || finding.filePath).split(path.sep).join("/");
      const rgba = Array.isArray(finding.rgba) ? ` (RGBA ${finding.rgba.join(",")})` : "";
      return `  - ${relativePath}${rgba}`;
    })
    .sort((a, b) => a.localeCompare(b));
}

// Shelf-pack into one or more pages (each <= MAX_SIZE). Tall/wide frames that don't fit a row
// wrap to the next row; when the page height would exceed MAX_SIZE, start a new page.
export function packIntoPages(sources, pagePixelBudget = DEFAULT_MAX_PAGE_PIXELS) {
  const sorted = [...sources].sort((a, b) => b.height - a.height || b.width - a.width || a.frame.localeCompare(b.frame));
  const widest = sorted.reduce((max, s) => Math.max(max, s.width + ATLAS_PADDING * 2), 1);
  const width = Math.min(MAX_SIZE, Math.max(INITIAL_WIDTH, nextPowerOfTwo(widest)));
  const tallest = sorted.reduce((max, s) => Math.max(max, s.height + ATLAS_PADDING * 2), 1);
  const budgetHeight = previousPowerOfTwo(Math.max(1, Math.floor(pagePixelBudget / width)));
  const maxPageHeight = Math.min(
    MAX_SIZE,
    Math.max(nextPowerOfTwo(tallest), budgetHeight),
  );

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
    if (s.width + ATLAS_PADDING * 2 > width || s.height + ATLAS_PADDING * 2 > maxPageHeight) {
      throw new Error(`Frame ${s.filePath} (${s.width}x${s.height}) exceeds ${MAX_SIZE}px atlas budget`);
    }
    if (current.cursorX + s.width + ATLAS_PADDING > width) {
      current.cursorX = ATLAS_PADDING;
      current.cursorY += current.rowHeight + ATLAS_PADDING;
      current.rowHeight = 0;
    }
    // Would this row overflow the page? Roll to a new page.
    if (current.cursorY + s.height + ATLAS_PADDING > maxPageHeight) {
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
  const pages = manifest?.schemaVersion === 2 ? manifest.pages : null;
  return Boolean(
    Array.isArray(pages) &&
      pages.length > 0 &&
      pages.every(
        (page) =>
          Number.isFinite(page?.w) &&
          Number.isFinite(page?.h) &&
          page.w > 0 &&
          page.h > 0 &&
          page.w <= maxSize &&
          page.h <= maxSize &&
          Number.isFinite(page?.b) &&
          page.b > 0,
      ),
  );
}

export function mapAtlasLibrarySupportsRawUpload(libraryKey) {
  const leaf = String(libraryKey).split("/").filter(Boolean).at(-1) ?? "";
  return /^(?:sm)?tiles\d*c?$/i.test(leaf);
}

export function isRemoteReleaseBuild(environment = process.env) {
  return environment.MIR2_ORIGINAL_ASSET_MANIFEST_MODE === "remote-release";
}

export async function readVerifiedRemoteMapAtlasRelease(
  configPath = PRODUCTION_ASSET_CONFIG,
) {
  let release;
  try {
    release = JSON.parse(await fs.readFile(configPath, "utf8"));
  } catch (error) {
    throw new Error(
      `Unable to read production asset release ${configPath}: ${error?.message ?? error}`,
    );
  }

  const mapAtlas = release?.mapAtlas;
  const contentHash = String(mapAtlas?.contentHash ?? "").toLowerCase();
  const manifestPath = String(mapAtlas?.manifestPath ?? "");
  const manifestMatch =
    /^\/generated\/map-atlas\/manifest\.([a-f0-9]{64})\.json$/i.exec(manifestPath);
  const pageCount = positiveInteger(mapAtlas?.pageCount);
  const maxPageBytes = positiveInteger(mapAtlas?.maxPageBytes);
  if (
    mapAtlas?.enabled !== true ||
    mapAtlas?.verified !== true ||
    !/^[a-f0-9]{64}$/.test(contentHash) ||
    manifestMatch?.[1]?.toLowerCase() !== contentHash ||
    !pageCount ||
    !maxPageBytes
  ) {
    throw new Error(
      `Production map-atlas release is not complete or verified: ${configPath}`,
    );
  }

  return {
    releaseVersion: String(release.version ?? ""),
    manifestPath,
    contentHash,
    pageCount,
    maxPageBytes,
  };
}

export async function removeStaleMapAtlasArtifacts(root) {
  let removed = 0;
  async function walk(directory, isRoot = false) {
    let entries;
    try {
      entries = await fs.readdir(directory, { withFileTypes: true });
    } catch (error) {
      if (error?.code === "ENOENT") return;
      throw error;
    }
    for (const entry of entries) {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        await walk(entryPath);
        continue;
      }
      const staleManifest = isRoot && /^manifest\.[0-9a-f]{64}\.json$/.test(entry.name);
      const stalePage = /^p\d+\.[0-9a-f]{16}\.png$/.test(entry.name);
      if (!entry.isFile() || (!staleManifest && !stalePage)) continue;
      await fs.rm(entryPath, { force: true });
      removed += 1;
    }
  }
  await walk(path.resolve(root), true);
  return removed;
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
function previousPowerOfTwo(value) {
  return 2 ** Math.floor(Math.log2(Math.max(1, value)));
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
