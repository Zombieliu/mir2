/**
 * mir2 offline asset pipeline — atlas packer CLI
 *
 * Formalises the ad-hoc atlas work (build-bevy-entity-atlas-pack.mjs) into a
 * repeatable offline build step.  The output is a deterministic manifest
 * (manifest.json) plus one PNG atlas page per atlas entry — ready to be served
 * by the runtime or pushed to R2 alongside the other asset releases.
 *
 * The schema of the emitted manifest is the backward-compatible superset
 * defined in schema.ts (schemaVersion=2).  The legacy runtime at
 * /bevy-entity-atlases/manifest.json (schemaVersion=1) is a strict subset so
 * the runtime needs no changes.
 *
 * Usage
 * ─────
 *   node scripts/asset-pipeline/pack.mjs [options]
 *
 * Options
 *   --category   entities|map|ui          (default: entities)
 *   --roots      comma-separated sub-dirs under public/original-ui/
 *                                         (default: entity starter set)
 *   --atlasKey   name for the atlas entry (default: starter-bichon-base)
 *   --outDir     output directory          (default: public/bevy-entity-atlases)
 *   --publicBase public URL prefix for imageUrl fields
 *                                         (default: /bevy-entity-atlases)
 *   --padding    inter-frame padding px    (default: 1)
 *   --pageSize   atlas page dimension px   (default: 2048) — frames spill onto a
 *                                          new page once a page fills up
 *   --maxSize    absolute page ceiling px  (default: 4096) — a single frame may
 *                                          not exceed this
 *   --dry-run    print plan, skip writing  (default: false)
 *
 * Multi-page
 * ──────────
 *   Frames are shelf-packed into fixed `pageSize`×`pageSize` pages; when a page
 *   fills, packing spills onto a new page.  Page 0 keeps the `<atlasKey>.png`
 *   name for backward compatibility; spill pages are `<atlasKey>-p<i>.png`.  Each
 *   rect records its `pageIndex` (absent ⇒ 0).  See schema.ts `AtlasPage`.
 *
 * KTX2/Basis compression
 * ──────────────────────
 *   Not yet wired.  The pipeline emits PNG atlas pages.  GPU compression
 *   (UASTC+zstd via bevy_basisu_loader) is a gated follow-up — see
 *   docs/ASSET-ATLAS-MIGRATION-PLAN.md §4.  Encode the emitted PNG pages with
 *   `toktx`/`ktx create` once the WebGL2 transcode path is verified.
 */

import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// We import sharp from the web app's node_modules because the pipeline lives
// inside the monorepo and doesn't have its own node_modules yet.
const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const PIPELINE_ROOT = SCRIPT_DIR;
const WEB_ROOT = path.resolve(PIPELINE_ROOT, "..", "..", "apps", "web");
const PUBLIC_ROOT = path.join(WEB_ROOT, "public");
const ORIGINAL_UI_ROOT = path.join(PUBLIC_ROOT, "original-ui");

// Sharp is resolved lazily inside main() so the helper function is available
// before the top-level code runs (ESM top-level await ordering).

// ── defaults ─────────────────────────────────────────────────────────────────

const PIPELINE_VERSION = "0.3.0";

const DEFAULT_ENTITY_ROOTS = [
  "CArmour/00",
  "CHair/00",
  "CWeapon/00",
  "AArmour/00",
  "AHair/00",
  "AWeapon/00 L",
  "AWeapon/00 R",
  "NPC",
  "Monster/000",
  "Monster/003",
  "Monster/004",
  "Monster/005",
  "Monster/007",
  "Monster/010",
  "Monster/012",
  "Monster/139",
];

const CATEGORY_DEFAULTS = {
  entities: {
    roots: DEFAULT_ENTITY_ROOTS,
    atlasKey: "starter-bichon-base",
    outDir: path.join(PUBLIC_ROOT, "bevy-entity-atlases"),
    publicBase: "/bevy-entity-atlases",
  },
  map: {
    roots: [],
    atlasKey: "map-tiles",
    outDir: path.join(PUBLIC_ROOT, "bevy-map-atlases"),
    publicBase: "/bevy-map-atlases",
  },
  ui: {
    roots: [],
    atlasKey: "ui-base",
    outDir: path.join(PUBLIC_ROOT, "bevy-ui-atlases"),
    publicBase: "/bevy-ui-atlases",
  },
};

const DEFAULT_PADDING = 1;
const DEFAULT_PAGE_SIZE = 2048;
const DEFAULT_MAX_SIZE = 4096;

// ── entry point ──────────────────────────────────────────────────────────────

const cliArgs = parseArgs(process.argv.slice(2));
main(cliArgs).catch((error) => {
  console.error("asset-pipeline/pack:", error.message ?? error);
  process.exitCode = 1;
});

async function main(args) {
  const t0 = Date.now();

  const category = String(args.category ?? "entities");
  if (!["entities", "map", "ui"].includes(category)) {
    throw new Error(`Unknown --category "${category}"; expected entities | map | ui`);
  }

  const defaults = CATEGORY_DEFAULTS[category];
  const roots = parseListArg(args.roots, defaults.roots);
  const atlasKey = String(args.atlasKey ?? defaults.atlasKey);
  const outDir = path.resolve(args.outDir ?? defaults.outDir);
  const publicBase = String(args.publicBase ?? defaults.publicBase).replace(/\/+$/, "");
  const padding = positiveInteger(args.padding, DEFAULT_PADDING);
  const maxSize = positiveInteger(args.maxSize, DEFAULT_MAX_SIZE);
  const pageSize = Math.min(positiveInteger(args.pageSize, DEFAULT_PAGE_SIZE), maxSize);
  const dryRun = parseBoolean(args["dry-run"] ?? args.dryRun ?? false);

  console.log("asset-pipeline/pack", PIPELINE_VERSION);
  console.log("  category:", category);
  console.log("  atlasKey:", atlasKey);
  console.log("  roots:   ", roots.length ? roots.join(", ") : "(none — will produce empty atlas)");
  console.log("  outDir:  ", outDir);
  console.log("  pageSize:", pageSize);
  console.log("  dryRun:  ", dryRun);

  // 1. Collect source frames (PNGs + optional meta.json offsets).
  const sources = await collectSources(roots, category);
  console.log(`\n  ${sources.length} source frame(s) found`);

  if (!sources.length && !dryRun) {
    console.warn("  Warning: no source frames found — emitting empty manifest.");
  }

  // 2. Pack into atlas pages (multi-page: spill to a new page when one fills).
  const pages = sources.length
    ? packSourcesMultiPage(sources, padding, pageSize)
    : [{ pageIndex: 0, width: 1, height: 1, sources: [] }];

  // Page 0 keeps the `<atlasKey>.png` name (backward-compatible with the
  // single-page runtime); spill pages get a `-p<i>` suffix.
  const pageFileName = (i) => (i === 0 ? `${atlasKey}.png` : `${atlasKey}-p${i}.png`);
  const manifestPath = path.join(outDir, "manifest.json");

  if (dryRun) {
    const totalFrames = pages.reduce((n, pg) => n + pg.sources.length, 0);
    console.log("\n  [dry-run] would write:", manifestPath);
    console.log(`  ${pages.length} page(s), ${totalFrames} frame(s):`);
    for (const pg of pages) {
      console.log(
        `     ${pageFileName(pg.pageIndex).padEnd(28)} ${pg.width}×${pg.height}  ${pg.sources.length} frames`,
      );
    }
    printDryRunTable(pages.flatMap((pg) => pg.sources).slice(0, 8));
    return;
  }

  // 3. Render one PNG per page.
  await fs.mkdir(outDir, { recursive: true });
  const sharpPath = await resolveSharp();
  const sharp = (await import(pathToFileURL(sharpPath).href)).default;

  /** @type {import("./schema.ts").AtlasPage[]} */
  const pageDescriptors = [];
  for (const pg of pages) {
    const imageFileName = pageFileName(pg.pageIndex);
    const imagePath = path.join(outDir, imageFileName);
    await sharp({
      create: {
        width: pg.width,
        height: pg.height,
        channels: 4,
        background: { r: 0, g: 0, b: 0, alpha: 0 },
      },
    })
      .composite(
        pg.sources.map((source) => ({
          input: source.filePath,
          left: source.x,
          top: source.y,
        })),
      )
      .png({ compressionLevel: 9, adaptiveFiltering: true })
      .toFile(imagePath);

    const bytes = await readFileBytes(imagePath);
    const sha256 = createHash("sha256").update(bytes).digest("hex");
    const stat = await fs.stat(imagePath);
    pageDescriptors.push({
      imageFile: imageFileName,
      imageUrl: `${publicBase}/${imageFileName}`,
      width: pg.width,
      height: pg.height,
      sha256,
      imageBytes: stat.size,
      rgbaBytes: pg.width * pg.height * 4,
    });
  }

  // 4. Build the flat per-frame rect list (sorted by key for determinism; each
  //    rect carries its pageIndex when > 0).
  const allSources = pages
    .flatMap((pg) => pg.sources)
    .sort((a, b) => a.key.localeCompare(b.key));
  const rects = allSources.map((source) => {
    /** @type {import("./schema.ts").AtlasRect} */
    const rect = {
      key: source.key,
      x: source.x,
      y: source.y,
      width: source.width,
      height: source.height,
    };
    // Carry Crystal frame offsets into the manifest when present.
    if (source.offsetX !== undefined) rect.offsetX = source.offsetX;
    if (source.offsetY !== undefined) rect.offsetY = source.offsetY;
    if (source.shadowX !== undefined) rect.shadowX = source.shadowX;
    if (source.shadowY !== undefined) rect.shadowY = source.shadowY;
    if (source.frameIndex !== undefined) rect.frameIndex = source.frameIndex;
    if (source.pageIndex) rect.pageIndex = source.pageIndex;
    return rect;
  });

  // 5. Content hash (deterministic over frame keys/positions + all page hashes).
  const contentHash = buildContentHash(
    atlasKey,
    rects,
    pageDescriptors.map((p) => p.sha256),
  );

  // 6. Aggregate byte totals; page 0 mirrors the legacy top-level fields.
  const page0 = pageDescriptors[0];
  const totalImageBytes = pageDescriptors.reduce((n, p) => n + p.imageBytes, 0);
  const totalRgbaBytes = pageDescriptors.reduce((n, p) => n + p.rgbaBytes, 0);

  // 7. Assemble atlas entry (backward-compatible: top-level width/height/imageUrl
  //    describe page 0; full per-page detail lives in pages[]).
  /** @type {import("./schema.ts").AtlasEntry} */
  const atlasEntry = {
    // ── legacy fields (mirror page 0) ──
    key: atlasKey,
    label: labelForCategory(category, atlasKey),
    width: page0.width,
    height: page0.height,
    sourceCount: sources.length,
    imageBytes: page0.imageBytes,
    rgbaBytes: page0.rgbaBytes,
    roots,
    imageUrl: page0.imageUrl,
    rects,
    // ── new fields ──
    pages: pageDescriptors,
    contentHash,
    category,
    padding,
  };

  // 8. Assemble top-level manifest.
  const durationMs = Date.now() - t0;
  /** @type {import("./schema.ts").AtlasManifest} */
  const manifest = {
    schemaVersion: 2,
    kind: "mir2-bevy-entity-atlas-manifest",
    generatedAt: new Date().toISOString(),
    atlases: [atlasEntry],
    stats: {
      sourceCount: sources.length,
      roots,
      imageBytes: totalImageBytes,
      rgbaBytes: totalRgbaBytes,
      pageCount: pageDescriptors.length,
      durationMs,
    },
    pipeline: {
      version: PIPELINE_VERSION,
      category,
      padding,
      maxSize,
      pageSize,
      dryRun: false,
      roots,
    },
  };

  await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");

  console.log(
    "\n  Done:",
    JSON.stringify(
      {
        ok: true,
        atlasKey,
        sourceCount: sources.length,
        pageCount: pageDescriptors.length,
        page0: `${page0.width}×${page0.height}`,
        imageBytes: totalImageBytes,
        sha256: page0.sha256.slice(0, 12) + "…",
        contentHash: contentHash.slice(0, 12) + "…",
        durationMs,
        manifestPath,
        outDir,
      },
      null,
      2,
    ),
  );
}

// ── source collection ─────────────────────────────────────────────────────────

/**
 * Collect PNGs under each root dir and attempt to load per-frame offset
 * metadata from the adjacent meta.json (written by export-crystal-ui.mjs).
 */
async function collectSources(roots, _category) {
  const sources = [];

  // Pre-load all available meta.json files so we can resolve offsets.
  // meta.json files are small (<< 1 MB each) and there are typically O(10-100)
  // of them for a typical atlas build, so loading them all eagerly is fine.
  const metaCache = new Map(); // dirPath → { frameMap: Map<index, frameMeta> }

  for (const root of roots) {
    const rootDir = path.join(ORIGINAL_UI_ROOT, root);
    if (!(await pathExists(rootDir))) {
      console.warn(`  Warning: source root not found, skipping: ${rootDir}`);
      continue;
    }
    const pngFiles = await walkPngFiles(rootDir);
    for (const filePath of pngFiles) {
      const frameDir = path.dirname(filePath);
      // Load (and cache) the meta.json for this directory.
      if (!metaCache.has(frameDir)) {
        metaCache.set(frameDir, await loadFrameMeta(frameDir));
      }
      const meta = metaCache.get(frameDir);
      const frameIndex = parseFrameIndex(path.basename(filePath));
      const frameMeta = meta?.frameMap.get(frameIndex) ?? null;

      // Read actual pixel dimensions from the PNG.
      const dims = await readPngDimensions(filePath);
      if (!dims) continue;

      const urlPath = publicUrlForFile(filePath);
      const key = `${urlPath}|${dims.width}x${dims.height}`;

      /** @type {SourceFrame} */
      const source = {
        filePath,
        path: urlPath,
        key,
        width: dims.width,
        height: dims.height,
      };

      // Attach Crystal offsets when available.
      if (frameMeta) {
        if (typeof frameMeta.x === "number") source.offsetX = frameMeta.x;
        if (typeof frameMeta.y === "number") source.offsetY = frameMeta.y;
        if (typeof frameMeta.shadowX === "number") source.shadowX = frameMeta.shadowX;
        if (typeof frameMeta.shadowY === "number") source.shadowY = frameMeta.shadowY;
        if (typeof frameMeta.index === "number") source.frameIndex = frameMeta.index;
      }

      sources.push(source);
    }
  }

  // Sort deterministically by key so repeated runs produce the same output.
  return sources.sort((a, b) => a.key.localeCompare(b.key));
}

/** Walk a directory recursively and return all .png paths. */
async function walkPngFiles(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await walkPngFiles(entryPath)));
    } else if (entry.isFile() && entry.name.toLowerCase().endsWith(".png")) {
      files.push(entryPath);
    }
  }
  return files;
}

/**
 * Load frame metadata from a meta.json file in the given directory.
 * Returns null when the file does not exist or cannot be parsed.
 */
async function loadFrameMeta(dir) {
  const metaPath = path.join(dir, "meta.json");
  try {
    const text = await fs.readFile(metaPath, "utf8");
    const payload = JSON.parse(text);
    if (!Array.isArray(payload.frames)) return null;
    const frameMap = new Map();
    for (const frame of payload.frames) {
      if (typeof frame.index === "number") {
        frameMap.set(frame.index, frame);
      }
    }
    return { frameMap };
  } catch {
    return null;
  }
}

/**
 * Parse the 0-based frame index from a filename like "42.png" → 42.
 * Returns null for filenames that don't match the pattern.
 */
function parseFrameIndex(filename) {
  const base = path.basename(filename, ".png");
  const n = Number(base);
  return Number.isInteger(n) && n >= 0 ? n : null;
}

// ── multi-page shelf packer ─────────────────────────────────────────────────
// Sort by height descending, then shelf-pack into fixed pageSize×pageSize pages.
// When a frame won't fit the current page, spill onto a new page.  Each packed
// source records its pageIndex; each page's height is trimmed to a power of two.

/**
 * @param {SourceFrame[]} sources
 * @param {number} padding
 * @param {number} pageSize
 * @returns {{ pageIndex: number, width: number, height: number, sources: any[] }[]}
 */
function packSourcesMultiPage(sources, padding, pageSize) {
  const sorted = [...sources].sort(
    (a, b) => b.height - a.height || b.width - a.width || a.key.localeCompare(b.key),
  );

  for (const s of sorted) {
    if (s.width + padding * 2 > pageSize || s.height + padding * 2 > pageSize) {
      throw new Error(
        `Frame ${s.key} (${s.width}×${s.height}) does not fit a ` +
          `${pageSize}×${pageSize} page (padding ${padding}); raise --pageSize.`,
      );
    }
  }

  /** @type {{ used: number, sources: any[] }[]} */
  const pages = [];
  let cursorX = padding;
  let cursorY = padding;
  let rowHeight = 0;
  let current = [];

  const flush = () => {
    pages.push({ used: cursorY + rowHeight + padding, sources: current });
    current = [];
    cursorX = padding;
    cursorY = padding;
    rowHeight = 0;
  };

  for (const source of sorted) {
    // Wrap to a new row when the frame overflows the page width.
    if (cursorX + source.width + padding > pageSize) {
      cursorX = padding;
      cursorY += rowHeight + padding;
      rowHeight = 0;
    }
    // Spill to a new page when the row overflows the page height.
    if (cursorY + source.height + padding > pageSize) {
      flush();
    }
    current.push({ ...source, x: cursorX, y: cursorY, pageIndex: pages.length });
    cursorX += source.width + padding;
    rowHeight = Math.max(rowHeight, source.height);
  }
  if (current.length) flush();

  // Re-sort each page's frames by key for a deterministic manifest.
  return pages.map((page, idx) => ({
    pageIndex: idx,
    width: pageSize,
    height: Math.min(pageSize, nextPowerOfTwo(page.used)),
    sources: page.sources.sort((a, b) => a.key.localeCompare(b.key)),
  }));
}

// ── helpers ───────────────────────────────────────────────────────────────────

/**
 * Read only the 8-byte PNG signature + IHDR chunk to get width/height without
 * decoding the full image.  Falls back to sharp when the header can't be read.
 */
async function readPngDimensions(filePath) {
  try {
    // PNG IHDR: signature(8) + chunk-length(4) + "IHDR"(4) + width(4) + height(4)
    const fd = await fs.open(filePath, "r");
    const buf = Buffer.alloc(24);
    await fd.read(buf, 0, 24, 0);
    await fd.close();
    // Verify PNG signature
    if (
      buf[0] !== 0x89 ||
      buf[1] !== 0x50 ||
      buf[2] !== 0x4e ||
      buf[3] !== 0x47
    ) {
      return null;
    }
    const width = buf.readUInt32BE(16);
    const height = buf.readUInt32BE(20);
    if (!width || !height) return null;
    return { width, height };
  } catch {
    return null;
  }
}

function publicUrlForFile(filePath) {
  const relative = path.relative(PUBLIC_ROOT, filePath).split(path.sep);
  return `/${relative.map((part) => encodeURIComponent(part)).join("/")}`;
}

function nextPowerOfTwo(value) {
  return 2 ** Math.ceil(Math.log2(Math.max(1, value)));
}

function positiveInteger(value, fallback) {
  const n = Number(value);
  return Number.isFinite(n) && n > 0 ? Math.trunc(n) : fallback;
}

async function pathExists(p) {
  try {
    await fs.access(p);
    return true;
  } catch {
    return false;
  }
}

async function readFileBytes(filePath) {
  return fs.readFile(filePath);
}

function buildContentHash(atlasKey, rects, pageSha256s) {
  const hash = createHash("sha256");
  hash.update("mir2-atlas-content-v1");
  hash.update(atlasKey);
  for (const rect of rects) {
    hash.update(rect.key);
    hash.update(`${rect.x},${rect.y},${rect.width},${rect.height}`);
    if (rect.pageIndex) hash.update(`p${rect.pageIndex}`);
  }
  for (const sha of pageSha256s) hash.update(sha);
  return hash.digest("hex");
}

function labelForCategory(category, atlasKey) {
  switch (category) {
    case "entities":
      return `Entity atlas: ${atlasKey}`;
    case "map":
      return `Map tile atlas: ${atlasKey}`;
    case "ui":
      return `UI atlas: ${atlasKey}`;
    default:
      return atlasKey;
  }
}

function printDryRunTable(sources) {
  console.log("  Sample packed frames:");
  for (const s of sources) {
    const offsetStr =
      s.offsetX !== undefined ? ` offset=(${s.offsetX},${s.offsetY})` : "";
    console.log(`    [${s.x},${s.y}] ${s.width}x${s.height}${offsetStr}  ${s.key}`);
  }
}

// ── sharp resolution ──────────────────────────────────────────────────────────

/**
 * Find the sharp package entry point by searching node_modules starting at
 * WEB_ROOT and walking up to the filesystem root.  This handles:
 *   - Normal installs: node_modules inside WEB_ROOT
 *   - pnpm hoisting: node_modules in an ancestor dir
 *   - Worktrees without a local node_modules (main checkout has the modules)
 */
async function resolveSharp() {
  // Sharp 0.35 moved its public entry point from lib/index.js to dist/index.cjs.
  // Resolve through the package exports map first so both layouts work.
  try {
    return createRequire(path.join(WEB_ROOT, "package.json")).resolve("sharp");
  } catch {
    // Keep the ancestor walk for hoisted/worktree installations that do not expose
    // a resolver-visible package from the web app's package boundary.
  }
  let dir = WEB_ROOT;
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const candidate = path.join(dir, "node_modules", "sharp", "lib", "index.js");
    if (await pathExists(candidate)) {
      return candidate;
    }
    // Also try the .cjs variant.
    const candidateCjs = candidate.replace(/\.js$/, ".cjs");
    if (await pathExists(candidateCjs)) {
      return candidateCjs;
    }
    const parent = path.dirname(dir);
    if (parent === dir) {
      throw new Error(
        "Could not find sharp in any node_modules directory up from " + WEB_ROOT +
          ". Run `npm install` in apps/web first.",
      );
    }
    dir = parent;
  }
}

// ── arg parsing ───────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const parsed = {};
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith("--")) continue;
    const [rawKey, inlineValue] = token.slice(2).split("=", 2);
    const key = rawKey.trim();
    if (!key) continue;
    if (inlineValue !== undefined) {
      parsed[key] = inlineValue;
    } else {
      parsed[key] =
        argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[++i] : "true";
    }
  }
  return parsed;
}

function parseListArg(value, fallback) {
  if (!value) return fallback;
  return String(value)
    .split(",")
    .map((v) => v.trim())
    .filter(Boolean);
}

function parseBoolean(value) {
  if (typeof value === "boolean") return value;
  return String(value).toLowerCase() === "true" || value === "1";
}
