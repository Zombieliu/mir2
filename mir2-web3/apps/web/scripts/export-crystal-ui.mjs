import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { deflateSync, gunzipSync } from "node:zlib";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const PUBLIC_DIR = path.join(WORKSPACE_ROOT, "public", "original-ui");
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

const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

async function main() {
  const manifestText = await readFile(MANIFEST_PATH, "utf8");
  const manifest = JSON.parse(manifestText);
  const dataDir = process.argv[2] ?? manifest.dataDir ?? DEFAULT_DATA_DIR;

  await mkdir(PUBLIC_DIR, { recursive: true });

  const summary = {
    dataDir,
    exportedAt: new Date().toISOString(),
    libraries: {},
  };
  const crystalMiniMapIndices = await loadCrystalMiniMapIndices();

  for (const [libraryName, config] of Object.entries(manifest.libraries)) {
    const normalizedLibraryName = normalizeLibraryName(libraryName);
    const inputPath = path.join(dataDir, ...normalizedLibraryName.split("/")) + ".Lib";
    const library = await parseLibrary(inputPath);
    const exportDir = path.join(PUBLIC_DIR, ...normalizedLibraryName.split("/"));
    const indices = expandIndices(
      config,
      normalizedLibraryName.toLowerCase() === "mmap" ? crystalMiniMapIndices : [],
    );

    await mkdir(exportDir, { recursive: true });

    const frames = [];
    for (const index of indices) {
      const frame = library.frames[index];
      if (!frame) {
        continue;
      }

      const basename = `${index}`;
      const pngPath = path.join(exportDir, `${basename}.png`);
      await writeFile(pngPath, encodePng(frame.width, frame.height, frame.rgba));

      if (frame.maskRgba) {
        await writeFile(
          path.join(exportDir, `${basename}.mask.png`),
          encodePng(frame.maskWidth, frame.maskHeight, frame.maskRgba),
        );
      }

      frames.push({
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
      });
    }

    const libraryMeta = {
      version: library.version,
      count: library.count,
      frames,
    };

    await writeFile(
      path.join(exportDir, "meta.json"),
      `${JSON.stringify(libraryMeta, null, 2)}\n`,
      "utf8",
    );

    summary.libraries[normalizedLibraryName] = libraryMeta;
  }

  await writeFile(
    path.join(PUBLIC_DIR, "manifest.generated.json"),
    `${JSON.stringify(summary, null, 2)}\n`,
    "utf8",
  );

  console.log(`Exported UI assets to ${PUBLIC_DIR}`);
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

    frames[index] = parseFrame(buffer, frameOffset, index);
  }

  return { version, count, frames };
}

function parseFrame(buffer, offset, index) {
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

  const raw = buffer.subarray(offset, offset + length);
  offset += length;

  const hasMask = (shadow >> 7) === 1;
  let maskWidth;
  let maskHeight;
  let maskX;
  let maskY;
  let maskLength;
  let maskRaw;

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
    maskRaw = buffer.subarray(offset, offset + maskLength);
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
    rgba: decodeFrame(width, height, raw),
    maskWidth,
    maskHeight,
    maskX,
    maskY,
    maskLength,
    maskRgba: hasMask ? decodeFrame(maskWidth, maskHeight, maskRaw) : null,
  };
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

  const idat = deflateSync(raw);
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
