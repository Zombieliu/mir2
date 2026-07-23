import { existsSync } from "node:fs";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import {
  PNG_SIGNATURE,
  allPresentFrameIndices,
  decodeFrameRgba,
  decodeMaskFrameRgba,
  encodePng,
  normalizeLibraryName,
  parseLibrary as parseLibraryBuffer,
} from "./crystal-library.mjs";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
const LOCAL_CRYSTAL_CLIENT_ROOT = path.join(MIR2_ROOT, "downloads", "crystal-client-full");
const DEFAULT_PUBLIC_DIR = path.join(WORKSPACE_ROOT, "public", "original-ui");
const MANIFEST_PATH = path.join(WORKSPACE_ROOT, "scripts", "crystal-ui-export-manifest.json");
const RESPAWN_MANIFEST_PATH = path.join(
  WORKSPACE_ROOT,
  "..",
  "..",
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_respawn_manifest.json",
);
const DEFAULT_DATA_DIR = "E:\\mir2\\Crystal\\Build\\Client\\Debug\\Data";
const PNG_DEFLATE_LEVEL = clampInt(process.env.MIR2_CRYSTAL_UI_PNG_DEFLATE_LEVEL ?? "1", 0, 9);

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const publicDir = path.resolve(args.outputDir ?? process.env.MIR2_CRYSTAL_UI_OUTPUT_DIR ?? DEFAULT_PUBLIC_DIR);
  const onlyLibrariesInput = args.libraries ?? process.env.MIR2_CRYSTAL_UI_LIBRARIES;
  const fullLibrariesInput = args.fullLibraries ?? process.env.MIR2_CRYSTAL_UI_FULL_LIBRARIES;
  const onlyLibraries = parseLibraryFilter(onlyLibrariesInput);
  const fullLibraries = parseLibraryFilter(fullLibrariesInput);
  const exportAllLibraries =
    hasAllLibrarySentinel(onlyLibrariesInput) || hasAllLibrarySentinel(fullLibrariesInput);
  const skipExisting = parseBoolean(args.skipExisting ?? process.env.MIR2_CRYSTAL_UI_SKIP_EXISTING);
  const writeConcurrency = clampInt(args.concurrency ?? process.env.MIR2_CRYSTAL_UI_WRITE_CONCURRENCY ?? "32", 1, 256);
  const manifestText = await readFile(MANIFEST_PATH, "utf8");
  const manifest = JSON.parse(manifestText);
  const dataDir =
    args.dataDir ??
    args._[0] ??
    process.env.CRYSTAL_CLIENT_DATA_DIR ??
    dataDirFromClientRoot(process.env.CRYSTAL_CLIENT_ROOT) ??
    (existsSync(LOCAL_CRYSTAL_CLIENT_ROOT) ? path.join(LOCAL_CRYSTAL_CLIENT_ROOT, "Data") : null) ??
    manifest.dataDir ??
    DEFAULT_DATA_DIR;

  await mkdir(publicDir, { recursive: true });

  const summaryPath = path.join(publicDir, "manifest.generated.json");
  const existingSummary = onlyLibraries && !exportAllLibraries
    ? await loadJsonIfPresent(summaryPath)
    : null;
  const summary = {
    dataDir,
    exportedAt: new Date().toISOString(),
    libraries: existingSummary?.libraries ?? {},
  };
  const crystalMiniMapIndices = await loadCrystalMiniMapIndices();

  const libraryEntries = exportAllLibraries
    ? (await discoverCrystalUiLibraries(dataDir)).map((libraryName) => [libraryName, {}])
    : Object.entries(manifest.libraries);
  let libraryOrdinal = 0;

  for (const [libraryName, config] of libraryEntries) {
    const normalizedLibraryName = normalizeLibraryName(libraryName);
    if (onlyLibraries && !onlyLibraries.has(normalizedLibraryName) && !exportAllLibraries) {
      continue;
    }
    libraryOrdinal += 1;

    const inputPath = path.join(dataDir, ...normalizedLibraryName.split("/")) + ".Lib";
    const library = await parseLibrary(inputPath);
    const exportDir = path.join(publicDir, ...normalizedLibraryName.split("/"));
    const indices = exportAllLibraries || fullLibraries?.has(normalizedLibraryName)
      ? allPresentFrameIndices(library)
      : expandIndices(
          config,
          normalizedLibraryName.toLowerCase() === "mmap" ? crystalMiniMapIndices : [],
        );

    await mkdir(exportDir, { recursive: true });
    console.log(
      `[crystal-ui] ${libraryOrdinal}/${libraryEntries.length} ${normalizedLibraryName}: ` +
        `${indices.length}/${library.count} present frame(s)`,
    );

    const frames = [];
    let written = 0;
    let reused = 0;
    for (let start = 0; start < indices.length; start += writeConcurrency) {
      const batch = indices.slice(start, start + writeConcurrency);
      const batchFrames = await Promise.all(batch.map(exportFrameIndex));
      for (const frameMeta of batchFrames) {
        if (frameMeta) frames.push(frameMeta);
      }
    }
    console.log(
      `[crystal-ui] ${normalizedLibraryName}: wrote ${written}, reused ${reused}, meta ${frames.length}`,
    );

    const libraryMeta = {
      version: library.version,
      count: Math.max(
        library.count,
        frames.reduce((maxIndex, frame) => Math.max(maxIndex, Number(frame.index)), -1) + 1,
      ),
      frameSet: library.frameSet,
      frames,
    };

    await writeFile(
      path.join(exportDir, "meta.json"),
      `${JSON.stringify(libraryMeta, null, 2)}\n`,
      "utf8",
    );

    summary.libraries[normalizedLibraryName] = libraryMeta;

    async function exportFrameIndex(index) {
      const frame = library.frames[index];
      if (!frame) {
        return loadExistingPngFrame(exportDir, normalizedLibraryName, index);
      }

      const basename = `${index}`;
      const pngPath = path.join(exportDir, `${basename}.png`);
      const pngExists = skipExisting && existsSync(pngPath);
      if (!pngExists) {
        await writeFile(
          pngPath,
          encodePng(frame.width, frame.height, decodeFrameRgba(library, frame), PNG_DEFLATE_LEVEL),
        );
        written += 1;
      } else {
        reused += 1;
      }

      if (frame.maskRgba) {
        const maskPath = path.join(exportDir, `${basename}.mask.png`);
        if (!skipExisting || !existsSync(maskPath)) {
          await writeFile(
            maskPath,
            encodePng(
              frame.maskWidth,
              frame.maskHeight,
              decodeMaskFrameRgba(library, frame),
              PNG_DEFLATE_LEVEL,
            ),
          );
        }
      }

      return {
        index,
        width: frame.width,
        height: frame.height,
        x: frame.x,
        y: frame.y,
        shadowX: frame.shadowX,
        shadowY: frame.shadowY,
        hasMask: Boolean(frame.maskRgba),
        maskWidth: frame.maskWidth ?? null,
        maskHeight: frame.maskHeight ?? null,
        path: `/original-ui/${normalizedLibraryName}/${basename}.png`,
        maskPath: frame.maskRgba ? `/original-ui/${normalizedLibraryName}/${basename}.mask.png` : null,
      };
    }
  }

  await writeFile(
    summaryPath,
    `${JSON.stringify(summary, null, 2)}\n`,
    "utf8",
  );

  console.log(`Exported UI assets to ${publicDir}`);
}

function parseArgs(argv) {
  const parsed = { _: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) {
      parsed._.push(arg);
      continue;
    }

    const equals = arg.indexOf("=");
    if (equals !== -1) {
      parsed[arg.slice(2, equals)] = arg.slice(equals + 1);
      continue;
    }

    const key = arg.slice(2);
    const next = argv[index + 1];
    if (next && !next.startsWith("--")) {
      parsed[key] = next;
      index += 1;
    } else {
      parsed[key] = "true";
    }
  }
  return parsed;
}

function parseLibraryFilter(value) {
  const libraries = String(value ?? "")
    .split(",")
    .map((library) => normalizeLibraryName(library))
    .filter(Boolean);
  return libraries.length ? new Set(libraries) : null;
}

function hasAllLibrarySentinel(value) {
  return String(value ?? "")
    .split(",")
    .map((library) => library.trim().toUpperCase())
    .includes("ALL");
}

function parseBoolean(value) {
  return /^(1|true|yes|on)$/i.test(String(value ?? ""));
}

function clampInt(value, min, max) {
  const parsed = Number.parseInt(String(value), 10);
  if (!Number.isFinite(parsed)) return min;
  return Math.max(min, Math.min(max, parsed));
}

async function discoverCrystalUiLibraries(dataDir) {
  const files = await listFilesRecursive(dataDir);
  return files
    .filter((filePath) => /\.lib$/i.test(filePath))
    .map((filePath) => path.relative(dataDir, filePath).split(path.sep).join("/").replace(/\.lib$/i, ""))
    .filter((libraryName) => !libraryName.toLowerCase().startsWith("map/"))
    .map((libraryName) => normalizeLibraryName(libraryName))
    .sort((left, right) => left.localeCompare(right, "en"));
}

async function listFilesRecursive(root) {
  const entries = await readdir(root, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const childPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFilesRecursive(childPath)));
      continue;
    }
    if (entry.isFile()) {
      files.push(childPath);
    }
  }
  return files;
}

function dataDirFromClientRoot(clientRoot) {
  if (!clientRoot) {
    return null;
  }
  const root = path.resolve(clientRoot);
  return path.basename(root).toLowerCase() === "data" ? root : path.join(root, "Data");
}

function expandIndices(config, extraIndices = []) {
  const indices = new Set();

  if (Array.isArray(config.indices)) {
    for (const index of config.indices) {
      indices.add(Number(index));
    }
  }

  if (Array.isArray(config.ranges)) {
    for (const [start, end] of config.ranges) {
      const from = Number(start);
      const to = Number(end);
      const step = from <= to ? 1 : -1;

      for (let index = from; step > 0 ? index <= to : index >= to; index += step) {
        indices.add(index);
      }
    }
  }

  for (const index of extraIndices) {
    indices.add(Number(index));
  }

  return [...indices].sort((left, right) => left - right);
}

async function loadExistingPngFrame(exportDir, normalizedLibraryName, index) {
  try {
    const pngPath = path.join(exportDir, `${index}.png`);
    const png = await readFile(pngPath);
    if (!png.subarray(0, PNG_SIGNATURE.length).equals(PNG_SIGNATURE)) {
      return null;
    }

    return {
      index,
      width: png.readUInt32BE(16),
      height: png.readUInt32BE(20),
      x: 0,
      y: 0,
      shadowX: 0,
      shadowY: 0,
      hasMask: false,
      maskWidth: null,
      maskHeight: null,
      path: `/original-ui/${normalizedLibraryName}/${index}.png`,
      maskPath: null,
    };
  } catch (error) {
    if (error.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

async function loadCrystalMiniMapIndices() {
  try {
    const manifest = JSON.parse(await readFile(RESPAWN_MANIFEST_PATH, "utf8"));
    return [
      ...new Set(
        (manifest.maps ?? [])
          .map((map) => Number(map.mini_map))
          .filter((index) => Number.isFinite(index) && index > 0),
      ),
    ];
  } catch {
    return [];
  }
}

async function loadJsonIfPresent(filePath) {
  try {
    return JSON.parse(await readFile(filePath, "utf8"));
  } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

async function parseLibrary(filePath) {
  const buffer = await readFile(filePath);
  try {
    return parseLibraryBuffer(buffer);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to parse Crystal library ${filePath}: ${message}`, { cause: error });
  }
}
