import { existsSync } from "node:fs";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { deflateSync, gunzipSync } from "node:zlib";

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

const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

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

  const summary = {
    dataDir,
    exportedAt: new Date().toISOString(),
    libraries: {},
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
        await writeFile(pngPath, encodePng(frame.width, frame.height, decodeFrameRgba(library, frame)));
        written += 1;
      } else {
        reused += 1;
      }

      if (frame.maskRgba) {
        const maskPath = path.join(exportDir, `${basename}.mask.png`);
        if (!skipExisting || !existsSync(maskPath)) {
          await writeFile(maskPath, encodePng(frame.maskWidth, frame.maskHeight, decodeMaskFrameRgba(library, frame)));
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
    path.join(publicDir, "manifest.generated.json"),
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

function normalizeLibraryName(libraryName) {
  return String(libraryName)
    .replaceAll("\\", "/")
    .split("/")
    .filter(Boolean)
    .join("/");
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

function allPresentFrameIndices(library) {
  return library.frames
    .map((frame, index) => (frame ? index : null))
    .filter((index) => index !== null);
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

async function parseLibrary(filePath) {
  const buffer = await readFile(filePath);
  let offset = 0;

  const version = buffer.readInt32LE(offset);
  offset += 4;

  if (version < 2) {
    throw new Error(`Unsupported lib version ${version}: ${filePath}`);
  }

  const count = buffer.readInt32LE(offset);
  offset += 4;

  if (version >= 3) {
    offset += 4;
  }

  const frameOffsets = [];
  for (let index = 0; index < count; index += 1) {
    frameOffsets.push(buffer.readInt32LE(offset));
    offset += 4;
  }

  const frames = new Array(count).fill(null);
  for (let index = 0; index < frameOffsets.length; index += 1) {
    const frameOffset = frameOffsets[index];
    if (frameOffset <= 0 || frameOffset >= buffer.length) {
      continue;
    }

    frames[index] = parseFrameHeader(buffer, frameOffset, index);
  }

  return { version, count, frames, buffer };
}

function parseFrameHeader(buffer, offset, index) {
  const width = buffer.readInt16LE(offset);
  offset += 2;
  const height = buffer.readInt16LE(offset);
  offset += 2;
  const x = buffer.readInt16LE(offset);
  offset += 2;
  const y = buffer.readInt16LE(offset);
  offset += 2;
  const shadowX = buffer.readInt16LE(offset);
  offset += 2;
  const shadowY = buffer.readInt16LE(offset);
  offset += 2;
  const shadow = buffer.readUInt8(offset);
  offset += 1;
  const length = buffer.readInt32LE(offset);
  offset += 4;

  const dataOffset = offset;
  offset += length;

  const hasMask = (shadow >> 7) === 1;
  let maskWidth;
  let maskHeight;
  let maskX;
  let maskY;
  let maskLength;
  let maskDataOffset;

  if (hasMask) {
    maskWidth = buffer.readInt16LE(offset);
    offset += 2;
    maskHeight = buffer.readInt16LE(offset);
    offset += 2;
    maskX = buffer.readInt16LE(offset);
    offset += 2;
    maskY = buffer.readInt16LE(offset);
    offset += 2;
    maskLength = buffer.readInt32LE(offset);
    offset += 4;
    maskDataOffset = offset;
    offset += maskLength;
  }

  return {
    index,
    width,
    height,
    x,
    y,
    shadowX,
    shadowY,
    shadow,
    dataOffset,
    dataLength: length,
    maskWidth,
    maskHeight,
    maskX,
    maskY,
    maskLength,
    maskDataOffset,
    maskRgba: hasMask,
  };
}

function decodeFrameRgba(library, frame) {
  return decodeFrame(
    frame.width,
    frame.height,
    library.buffer.subarray(frame.dataOffset, frame.dataOffset + frame.dataLength),
  );
}

function decodeMaskFrameRgba(library, frame) {
  return decodeFrame(
    frame.maskWidth,
    frame.maskHeight,
    library.buffer.subarray(frame.maskDataOffset, frame.maskDataOffset + frame.maskLength),
  );
}

function decodeFrame(width, height, compressed) {
  if (!width || !height) {
    return Buffer.alloc(0);
  }

  const bgra = gunzipSync(compressed);
  const rgba = Buffer.allocUnsafe(width * height * 4);

  for (let source = 0; source < bgra.length; source += 4) {
    const dest = source;
    rgba[dest] = bgra[source + 2];
    rgba[dest + 1] = bgra[source + 1];
    rgba[dest + 2] = bgra[source];
    rgba[dest + 3] = bgra[source + 3];
  }

  return rgba;
}

function encodePng(width, height, rgba) {
  const raw = Buffer.allocUnsafe((width * 4 + 1) * height);

  for (let row = 0; row < height; row += 1) {
    const rawOffset = row * (width * 4 + 1);
    raw[rawOffset] = 0;
    rgba.copy(raw, rawOffset + 1, row * width * 4, (row + 1) * width * 4);
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  const idat = deflateSync(raw, { level: PNG_DEFLATE_LEVEL });
  return Buffer.concat([PNG_SIGNATURE, chunk("IHDR", ihdr), chunk("IDAT", idat), chunk("IEND", Buffer.alloc(0))]);
}

function chunk(type, data) {
  const typeBuffer = Buffer.from(type, "ascii");
  const lengthBuffer = Buffer.alloc(4);
  lengthBuffer.writeUInt32BE(data.length, 0);

  const crcBuffer = Buffer.concat([typeBuffer, data]);
  const checksum = crc32(crcBuffer);
  const crcOutput = Buffer.alloc(4);
  crcOutput.writeUInt32BE(checksum >>> 0, 0);

  return Buffer.concat([lengthBuffer, typeBuffer, data, crcOutput]);
}

function crc32(buffer) {
  let crc = 0xffffffff;

  for (let index = 0; index < buffer.length; index += 1) {
    crc ^= buffer[index];

    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1) === 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
    }
  }

  return (crc ^ 0xffffffff) >>> 0;
}
