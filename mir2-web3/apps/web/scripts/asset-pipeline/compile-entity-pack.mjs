import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

import {
  allPresentFrameIndices,
  decodeFrameRgba,
  decodeMaskFrameRgba,
  normalizeLibraryName,
  parseLibrary,
} from "../crystal-library.mjs";

export const ENTITY_PACK_SCHEMA_VERSION = 1;

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const WEB_ROOT = path.resolve(import.meta.dirname, "..", "..");
const MIR2_ROOT = path.resolve(WEB_ROOT, "..", "..", "..");
const DEFAULT_DATA_DIR = path.join(MIR2_ROOT, "Crystal", "Build", "Client", "Debug", "Data");
const DEFAULT_OUTPUT_DIR = path.join(WEB_ROOT, "public", "generated", "crystal-packs", "entities-starter");
const DEFAULT_LIBRARIES = ["NPC/00", "Monster/000", "Monster/010"];
const PAGE_SIZE = 2048;
const PADDING = 1;

if (process.argv[1] && path.resolve(process.argv[1]) === SCRIPT_PATH) {
  runEntityPackCompiler().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}

export async function runEntityPackCompiler(argv = process.argv.slice(2)) {
  const args = parseArgs(argv);
  const dataDir = path.resolve(args.dataDir ?? DEFAULT_DATA_DIR);
  const outputDir = path.resolve(args.output ?? DEFAULT_OUTPUT_DIR);
  const packId = String(args.packId ?? "entities-starter");
  const libraries = parseList(args.libraries, DEFAULT_LIBRARIES);
  const urlRoot = String(args.urlRoot ?? `/generated/crystal-packs/${packId}`).replace(/\/$/, "");
  const manifest = await compileEntityPack({ dataDir, outputDir, packId, libraries, urlRoot });
  console.log(JSON.stringify({
    outputDir,
    packId,
    contentHash: manifest.contentHash,
    libraryCount: manifest.summary.libraryCount,
    frameCount: manifest.summary.frameCount,
    maskCount: manifest.summary.maskCount,
    pageCount: manifest.summary.pageCount,
    networkBytes: manifest.summary.networkBytes,
    gpuBytes: manifest.summary.gpuBytes,
  }, null, 2));
  return manifest;
}

export async function compileEntityPack({ dataDir, outputDir, packId, libraries, urlRoot }) {
  const normalizedLibraries = [...new Set(libraries.map(normalizeLibraryName))].sort(compareCodePoints);
  if (normalizedLibraries.length === 0) throw new Error("Entity pack requires at least one library");

  const libraryRecords = [];
  const sources = [];
  for (const libraryKey of normalizedLibraries) {
    const filePath = path.join(dataDir, ...libraryKey.split("/")) + ".Lib";
    const buffer = await readFile(filePath);
    const library = parseLibrary(buffer);
    const sourceSha256 = sha256(buffer);
    const frames = [];
    for (const index of allPresentFrameIndices(library)) {
      const frame = library.frames[index];
      if (!frame || frame.width <= 0 || frame.height <= 0) continue;
      const rectKey = `${libraryKey}#${index}`;
      sources.push({
        key: rectKey,
        libraryKey,
        frameIndex: index,
        role: "image",
        width: frame.width,
        height: frame.height,
        rgba: decodeFrameRgba(library, frame),
      });
      let maskRectKey = null;
      if (frame.maskRgba && frame.maskWidth > 0 && frame.maskHeight > 0) {
        maskRectKey = `${rectKey}:mask`;
        sources.push({
          key: maskRectKey,
          libraryKey,
          frameIndex: index,
          role: "mask",
          width: frame.maskWidth,
          height: frame.maskHeight,
          rgba: decodeMaskFrameRgba(library, frame),
        });
      }
      frames.push({
        index,
        rectKey,
        maskRectKey,
        width: frame.width,
        height: frame.height,
        x: frame.x,
        y: frame.y,
        shadowX: frame.shadowX,
        shadowY: frame.shadowY,
        shadow: frame.shadow,
        maskWidth: frame.maskWidth ?? null,
        maskHeight: frame.maskHeight ?? null,
        maskX: frame.maskX ?? null,
        maskY: frame.maskY ?? null,
      });
    }
    libraryRecords.push({
      key: libraryKey,
      sourceSha256,
      sourceBytes: buffer.byteLength,
      version: library.version,
      frameSlotCount: library.count,
      frameSet: { count: library.frameSet.count, actions: library.frameSet.actions },
      frames,
    });
  }

  const packedPages = packSourcesIntoPages(sources);
  const pagesDir = path.join(outputDir, "pages");
  await mkdir(pagesDir, { recursive: true });
  const pages = [];
  const rectLocations = new Map();
  for (let pageIndex = 0; pageIndex < packedPages.length; pageIndex += 1) {
    const page = packedPages[pageIndex];
    const png = await renderPage(page);
    const pageHash = sha256(png);
    const fileName = `${pageHash}.png`;
    await writeFile(path.join(pagesDir, fileName), png);
    const pageKey = `sha256:${pageHash}`;
    const rects = page.sources
      .map((source) => {
        const rect = {
          key: source.key,
          x: source.x + PADDING,
          y: source.y + PADDING,
          width: source.width,
          height: source.height,
          sourceKind: source.role,
        };
        rectLocations.set(source.key, { pageKey, ...rect });
        return rect;
      })
      .sort((left, right) => compareCodePoints(left.key, right.key));
    pages.push({
      key: pageKey,
      sha256: pageHash,
      width: page.width,
      height: page.height,
      networkBytes: png.byteLength,
      gpuBytes: page.width * page.height * 4,
      imageUrl: `${urlRoot}/pages/${fileName}`,
      rects,
    });
  }

  const librariesByKey = {};
  for (const library of libraryRecords.sort((left, right) => compareCodePoints(left.key, right.key))) {
    librariesByKey[library.key] = {
      ...library,
      frames: library.frames.map((frame) => ({
        ...frame,
        pageKey: rectLocations.get(frame.rectKey)?.pageKey ?? null,
        maskPageKey: frame.maskRectKey ? rectLocations.get(frame.maskRectKey)?.pageKey ?? null : null,
      })),
    };
  }

  const body = {
    schemaVersion: ENTITY_PACK_SCHEMA_VERSION,
    kind: "mir2-crystal-entity-pack",
    id: packId,
    textureFormat: "png-rgba8-srgb",
    sampler: "nearest",
    padding: PADDING,
    pages,
    libraries: librariesByKey,
    summary: {
      libraryCount: libraryRecords.length,
      frameCount: sources.filter((source) => source.role === "image").length,
      maskCount: sources.filter((source) => source.role === "mask").length,
      actionCount: libraryRecords.reduce((sum, library) => sum + library.frameSet.count, 0),
      pageCount: pages.length,
      networkBytes: pages.reduce((sum, page) => sum + page.networkBytes, 0),
      gpuBytes: pages.reduce((sum, page) => sum + page.gpuBytes, 0),
    },
  };
  const manifest = { ...body, contentHash: semanticHash(body) };
  validateEntityPack(manifest);
  await mkdir(outputDir, { recursive: true });
  await writeFile(path.join(outputDir, "manifest.json"), `${canonicalJson(manifest)}\n`, "utf8");
  return manifest;
}

export function validateEntityPack(manifest) {
  if (manifest?.schemaVersion !== ENTITY_PACK_SCHEMA_VERSION || manifest.kind !== "mir2-crystal-entity-pack") {
    throw new Error("Invalid Crystal entity pack schema");
  }
  const body = { ...manifest };
  delete body.contentHash;
  const expectedHash = semanticHash(body);
  if (manifest.contentHash !== expectedHash) throw new Error("Crystal entity pack contentHash mismatch");
  const pageKeys = new Set();
  const rectKeys = new Set();
  for (const page of manifest.pages) {
    if (!/^sha256:[a-f0-9]{64}$/.test(page.key) || page.sha256 !== page.key.slice(7)) {
      throw new Error(`Invalid entity page hash ${page.key}`);
    }
    if (pageKeys.has(page.key)) throw new Error(`Duplicate entity page ${page.key}`);
    pageKeys.add(page.key);
    for (const rect of page.rects) {
      if (rectKeys.has(rect.key)) throw new Error(`Duplicate entity rect ${rect.key}`);
      rectKeys.add(rect.key);
      if (rect.x < PADDING || rect.y < PADDING || rect.x + rect.width >= page.width || rect.y + rect.height >= page.height) {
        throw new Error(`Entity rect ${rect.key} has no complete extruded gutter`);
      }
    }
  }
  for (const library of Object.values(manifest.libraries)) {
    if (library.frameSet.count !== library.frameSet.actions.length) {
      throw new Error(`FrameSet count mismatch for ${library.key}`);
    }
    for (const frame of library.frames) {
      if (!rectKeys.has(frame.rectKey) || !pageKeys.has(frame.pageKey)) {
        throw new Error(`Missing image rect for ${library.key}#${frame.index}`);
      }
      if (frame.maskRectKey && (!rectKeys.has(frame.maskRectKey) || !pageKeys.has(frame.maskPageKey))) {
        throw new Error(`Missing mask rect for ${library.key}#${frame.index}`);
      }
    }
  }
  return true;
}

function packSourcesIntoPages(sources) {
  const sorted = [...sources].sort(
    (left, right) =>
      right.height - left.height ||
      right.width - left.width ||
      compareCodePoints(left.key, right.key),
  );
  const pages = [];
  let page = newPage();
  const flush = () => {
    if (page.sources.length === 0) return;
    page.height = nextPowerOfTwo(page.cursorY + page.rowHeight + PADDING);
    pages.push(page);
  };
  for (const source of sorted) {
    const outerWidth = source.width + PADDING * 2;
    const outerHeight = source.height + PADDING * 2;
    if (outerWidth > PAGE_SIZE || outerHeight > PAGE_SIZE) {
      throw new Error(`Entity source ${source.key} exceeds ${PAGE_SIZE}px page budget`);
    }
    if (page.cursorX + outerWidth > PAGE_SIZE) {
      page.cursorX = 0;
      page.cursorY += page.rowHeight;
      page.rowHeight = 0;
    }
    if (page.cursorY + outerHeight > PAGE_SIZE) {
      flush();
      page = newPage();
    }
    page.sources.push({ ...source, x: page.cursorX, y: page.cursorY });
    page.cursorX += outerWidth;
    page.rowHeight = Math.max(page.rowHeight, outerHeight);
  }
  flush();
  return pages;
}

function newPage() {
  return { width: PAGE_SIZE, height: 0, cursorX: 0, cursorY: 0, rowHeight: 0, sources: [] };
}

async function renderPage(page) {
  const composites = page.sources.map((source) => ({
    input: extrudeRgba(source.rgba, source.width, source.height, PADDING),
    raw: {
      width: source.width + PADDING * 2,
      height: source.height + PADDING * 2,
      channels: 4,
    },
    left: source.x,
    top: source.y,
  }));
  return sharp({
    create: {
      width: page.width,
      height: page.height,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite(composites)
    .png({ compressionLevel: 9, adaptiveFiltering: false, palette: false })
    .toBuffer();
}

function extrudeRgba(rgba, width, height, padding) {
  const outputWidth = width + padding * 2;
  const outputHeight = height + padding * 2;
  const output = Buffer.allocUnsafe(outputWidth * outputHeight * 4);
  for (let y = 0; y < outputHeight; y += 1) {
    const sourceY = Math.min(height - 1, Math.max(0, y - padding));
    for (let x = 0; x < outputWidth; x += 1) {
      const sourceX = Math.min(width - 1, Math.max(0, x - padding));
      const sourceOffset = (sourceY * width + sourceX) * 4;
      const outputOffset = (y * outputWidth + x) * 4;
      rgba.copy(output, outputOffset, sourceOffset, sourceOffset + 4);
    }
  }
  return output;
}

function semanticHash(value) {
  return sha256(Buffer.from(canonicalJson(value), "utf8"));
}

function canonicalJson(value) {
  return JSON.stringify(canonicalize(value), null, 2);
}

function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort(compareCodePoints).map((key) => [key, canonicalize(value[key])]));
  }
  return value;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function nextPowerOfTwo(value) {
  return 2 ** Math.ceil(Math.log2(Math.max(1, value)));
}

function compareCodePoints(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function parseList(value, fallback) {
  return String(value ?? fallback.join(","))
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) continue;
    const equals = argument.indexOf("=");
    if (equals >= 0) parsed[argument.slice(2, equals)] = argument.slice(equals + 1);
    else if (argv[index + 1] && !argv[index + 1].startsWith("--")) parsed[argument.slice(2)] = argv[++index];
    else throw new Error(`Missing value for ${argument}`);
  }
  return parsed;
}
