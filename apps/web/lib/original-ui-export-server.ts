import "server-only";

import { existsSync } from "node:fs";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { deflateSync, gunzipSync } from "node:zlib";

import sourceLibraries from "../public/original-ui/source-libraries.generated.json";

const WORKSPACE_ROOT = path.resolve(/* turbopackIgnore: true */ process.cwd());
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
// Keep the writable development export cache out of Turbopack's static
// directory-reference graph. A literal `public/original-ui` join makes Next
// enumerate tens of thousands of generated PNGs while compiling this API route.
const ORIGINAL_UI_OUTPUT_RELATIVE = Buffer.from(
  "cHVibGljL29yaWdpbmFsLXVp",
  "base64",
).toString("utf8");
const PUBLIC_ORIGINAL_UI_DIR = process.env.MIR2_ORIGINAL_UI_OUTPUT_DIR
  ? path.resolve(process.env.MIR2_ORIGINAL_UI_OUTPUT_DIR)
  : path.resolve(WORKSPACE_ROOT, ORIGINAL_UI_OUTPUT_RELATIVE);
const LOCAL_CRYSTAL_CLIENT_ROOT = path.join(MIR2_ROOT, "downloads", "crystal-client-full");
const DEFAULT_CRYSTAL_CLIENT_ROOT = "E:\\mir2\\Crystal\\Build\\Client\\Debug";
const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const MAX_ON_DEMAND_LIBRARY_FRAMES = Number(process.env.MIR2_MAX_ON_DEMAND_LIBRARY_FRAMES ?? 15000);

type OriginalUiFrameMeta = {
  index: number;
  width: number;
  height: number;
  x: number;
  y: number;
  shadowX: number;
  shadowY: number;
  hasMask: boolean;
  maskWidth: number | null;
  maskHeight: number | null;
  path: string;
  maskPath: string | null;
};

export type OriginalUiLibraryMeta = {
  version: number;
  count: number;
  frames: OriginalUiFrameMeta[];
};

type ParsedFrame = {
  index: number;
  width: number;
  height: number;
  x: number;
  y: number;
  shadowX: number;
  shadowY: number;
  rgba: Buffer;
  maskWidth?: number;
  maskHeight?: number;
  maskRgba: Buffer | null;
};

type ParsedLibrary = {
  version: number;
  count: number;
  frames: Array<ParsedFrame | null>;
};

type SourceLibraryEntry = {
  frames?: number;
};

type SourceLibraryIndex = {
  libraries?: Record<string, SourceLibraryEntry>;
};

export class OriginalUiExportError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly code: string,
  ) {
    super(message);
  }
}

const pendingExports = new Map<string, Promise<OriginalUiLibraryMeta>>();

export async function readDeployedOriginalUiLibraryMeta(libraryKey: string): Promise<OriginalUiLibraryMeta | null> {
  const normalizedKey = normalizeLibraryKey(libraryKey);
  validateOriginalUiLibraryKey(normalizedKey);

  const outputDir = path.join(
    /* turbopackIgnore: true */ PUBLIC_ORIGINAL_UI_DIR,
    ...normalizedKey.split("/"),
  );
  return readExistingMeta(path.join(outputDir, "meta.json"));
}

export async function ensureOriginalUiLibraryExport(libraryKey: string): Promise<OriginalUiLibraryMeta> {
  const normalizedKey = normalizeLibraryKey(libraryKey);
  const pending = pendingExports.get(normalizedKey);
  if (pending) {
    return pending;
  }

  const promise = exportOriginalUiLibrary(normalizedKey).finally(() => {
    pendingExports.delete(normalizedKey);
  });
  pendingExports.set(normalizedKey, promise);
  return promise;
}

async function exportOriginalUiLibrary(normalizedKey: string): Promise<OriginalUiLibraryMeta> {
  const sourceEntry = validateOriginalUiLibraryKey(normalizedKey);
  if (
    Number.isFinite(MAX_ON_DEMAND_LIBRARY_FRAMES) &&
    MAX_ON_DEMAND_LIBRARY_FRAMES > 0 &&
    Number(sourceEntry.frames ?? 0) > MAX_ON_DEMAND_LIBRARY_FRAMES
  ) {
    throw new OriginalUiExportError(
      `Crystal library is too large for on-demand export: ${normalizedKey}`,
      413,
      "library_too_large",
    );
  }

  const outputDir = path.join(
    /* turbopackIgnore: true */ PUBLIC_ORIGINAL_UI_DIR,
    ...normalizedKey.split("/"),
  );
  const metaPath = path.join(outputDir, "meta.json");
  const existing = await readExistingMeta(metaPath);
  if (existing) {
    return existing;
  }

  const dataDir = crystalDataDir();
  if (!existsSync(dataDir)) {
    throw new OriginalUiExportError(`Crystal data directory is missing: ${dataDir}`, 503, "crystal_data_missing");
  }
  const sourcePath = path.join(dataDir, ...normalizedKey.split("/")) + ".Lib";
  const resolvedSourcePath = path.resolve(sourcePath);
  const resolvedDataDir = path.resolve(dataDir);
  if (!resolvedSourcePath.startsWith(`${resolvedDataDir}${path.sep}`)) {
    throw new OriginalUiExportError(`Invalid original UI library path: ${normalizedKey}`, 400, "invalid_library_path");
  }
  if (!existsSync(resolvedSourcePath)) {
    throw new OriginalUiExportError(`Crystal library is not available: ${normalizedKey}`, 404, "library_missing");
  }

  const tempOutputDir = `${outputDir}.tmp-${process.pid}-${Date.now()}`;
  await rm(tempOutputDir, { recursive: true, force: true });
  await mkdir(tempOutputDir, { recursive: true });
  const library = await parseLibrary(resolvedSourcePath);
  const frames: OriginalUiFrameMeta[] = [];

  try {
    for (const frame of library.frames) {
      if (!frame) {
        continue;
      }

      const pngPath = path.join(tempOutputDir, `${frame.index}.png`);
      await writeFile(pngPath, encodePng(frame.width, frame.height, frame.rgba));

      let maskPath: string | null = null;
      if (frame.maskRgba && frame.maskWidth && frame.maskHeight) {
        await writeFile(
          path.join(tempOutputDir, `${frame.index}.mask.png`),
          encodePng(frame.maskWidth, frame.maskHeight, frame.maskRgba),
        );
        maskPath = `/original-ui/${normalizedKey}/${frame.index}.mask.png`;
      }

      frames.push({
        index: frame.index,
        width: frame.width,
        height: frame.height,
        x: frame.x,
        y: frame.y,
        shadowX: frame.shadowX,
        shadowY: frame.shadowY,
        hasMask: Boolean(maskPath),
        maskWidth: frame.maskWidth ?? null,
        maskHeight: frame.maskHeight ?? null,
        path: `/original-ui/${normalizedKey}/${frame.index}.png`,
        maskPath,
      });
    }

    const meta = {
      version: library.version,
      count: library.count,
      frames,
    };
    await writeFile(path.join(tempOutputDir, "meta.json"), `${JSON.stringify(meta, null, 2)}\n`, "utf8");
    await rm(outputDir, { recursive: true, force: true });
    await mkdir(path.dirname(outputDir), { recursive: true });
    await rename(tempOutputDir, outputDir);
    return meta;
  } catch (error) {
    await rm(tempOutputDir, { recursive: true, force: true });
    throw error;
  }
}

function normalizeLibraryKey(libraryKey: string) {
  const normalized = libraryKey
    .replaceAll("\\", "/")
    .split("/")
    .filter(Boolean)
    .join("/");

  if (
    normalized.startsWith("/") ||
    normalized.split("/").some((segment) => segment === "." || segment === "..")
  ) {
    throw new OriginalUiExportError(`Invalid original UI library: ${libraryKey}`, 400, "invalid_library_path");
  }

  return normalized;
}

function sourceLibraryEntry(normalizedKey: string): SourceLibraryEntry | null {
  const libraries = (sourceLibraries as SourceLibraryIndex).libraries ?? {};
  return libraries[normalizedKey] ?? libraries[normalizedKey.replaceAll("\\", "/")] ?? null;
}

function validateOriginalUiLibraryKey(normalizedKey: string): SourceLibraryEntry {
  if (!normalizedKey || normalizedKey.startsWith("Map/")) {
    throw new OriginalUiExportError(`Unsupported original UI library: ${normalizedKey}`, 400, "unsupported_library");
  }

  const sourceEntry = sourceLibraryEntry(normalizedKey);
  if (!sourceEntry) {
    throw new OriginalUiExportError(`Crystal library is not indexed: ${normalizedKey}`, 403, "library_not_indexed");
  }

  return sourceEntry;
}

function crystalDataDir() {
  const root = process.env.CRYSTAL_CLIENT_ROOT
    ? path.resolve(process.env.CRYSTAL_CLIENT_ROOT)
    : existsSync(LOCAL_CRYSTAL_CLIENT_ROOT)
      ? LOCAL_CRYSTAL_CLIENT_ROOT
      : DEFAULT_CRYSTAL_CLIENT_ROOT;
  return path.basename(root).toLowerCase() === "data" ? root : path.join(root, "Data");
}

async function readExistingMeta(metaPath: string) {
  try {
    const meta = JSON.parse(await readFile(metaPath, "utf8")) as OriginalUiLibraryMeta;
    if (await existingFramesArePresent(meta)) {
      return meta;
    }
    return null;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

async function existingFramesArePresent(meta: OriginalUiLibraryMeta) {
  for (const frame of meta.frames) {
    const relativeFramePath = frame.path
      .replace(/^[/\\]+/, "")
      .replace(/^original-ui[/\\]/, "");
    const filePath = path.join(PUBLIC_ORIGINAL_UI_DIR, relativeFramePath);
    try {
      const info = await stat(/* turbopackIgnore: true */ filePath);
      if (!info.isFile() || info.size === 0) {
        return false;
      }
    } catch {
      return false;
    }
  }
  return true;
}

async function parseLibrary(filePath: string): Promise<ParsedLibrary> {
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

  const frameOffsets: number[] = [];
  for (let index = 0; index < count; index += 1) {
    frameOffsets.push(buffer.readInt32LE(offset));
    offset += 4;
  }

  const frames = new Array<ParsedFrame | null>(count).fill(null);
  for (let index = 0; index < frameOffsets.length; index += 1) {
    const frameOffset = frameOffsets[index];
    if (frameOffset <= 0 || frameOffset >= buffer.length) {
      continue;
    }

    frames[index] = parseFrame(buffer, frameOffset, index);
  }

  return { version, count, frames };
}

function parseFrame(buffer: Buffer, offset: number, index: number): ParsedFrame {
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
  let maskWidth: number | undefined;
  let maskHeight: number | undefined;
  let maskRaw: Buffer | undefined;

  if (hasMask) {
    maskWidth = buffer.readInt16LE(offset);
    offset += 2;
    maskHeight = buffer.readInt16LE(offset);
    offset += 2;
    offset += 4;
    const maskLength = buffer.readInt32LE(offset);
    offset += 4;
    maskRaw = buffer.subarray(offset, offset + maskLength);
  }

  return {
    index,
    width,
    height,
    x,
    y,
    shadowX,
    shadowY,
    rgba: decodeFrame(width, height, raw),
    maskWidth,
    maskHeight,
    maskRgba: hasMask && maskWidth && maskHeight && maskRaw ? decodeFrame(maskWidth, maskHeight, maskRaw) : null,
  };
}

function decodeFrame(width: number, height: number, compressed: Buffer) {
  if (!width || !height) {
    return Buffer.alloc(0);
  }

  const bgra = gunzipSync(compressed);
  const rgba = Buffer.allocUnsafe(width * height * 4);

  for (let source = 0; source < bgra.length; source += 4) {
    rgba[source] = bgra[source + 2];
    rgba[source + 1] = bgra[source + 1];
    rgba[source + 2] = bgra[source];
    rgba[source + 3] = bgra[source + 3];
  }

  return rgba;
}

function encodePng(width: number, height: number, rgba: Buffer) {
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

function chunk(type: string, data: Buffer) {
  const typeBuffer = Buffer.from(type, "ascii");
  const lengthBuffer = Buffer.alloc(4);
  lengthBuffer.writeUInt32BE(data.length, 0);

  const checksum = crc32(Buffer.concat([typeBuffer, data]));
  const crcOutput = Buffer.alloc(4);
  crcOutput.writeUInt32BE(checksum >>> 0, 0);

  return Buffer.concat([lengthBuffer, typeBuffer, data, crcOutput]);
}

function crc32(buffer: Buffer) {
  let crc = 0xffffffff;

  for (let index = 0; index < buffer.length; index += 1) {
    crc ^= buffer[index];

    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1) === 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
    }
  }

  return (crc ^ 0xffffffff) >>> 0;
}
