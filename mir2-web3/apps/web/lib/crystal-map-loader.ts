import "server-only";

import { closeSync, existsSync, openSync, readFileSync, readSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { deflateSync, gunzipSync } from "node:zlib";

import { normalizeCrystalMiniMapFileName } from "./crystal-minimap-transform";
import { CRYSTAL_MINI_MAP_TRANSFORMS } from "./generated/crystal-minimap-transforms";
import type { OriginalMapRegion, OriginalMapSpriteFrame, SceneBlueprint } from "./scene-types";

const WORKSPACE_ROOT = path.resolve(/* turbopackIgnore: true */ process.cwd());
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
const PUBLIC_ORIGINAL_MAP_DIR = path.join(WORKSPACE_ROOT, "public", "original-map");
const PUBLIC_ORIGINAL_ASSET_MANIFEST_PATH = path.join(
  WORKSPACE_ROOT,
  "public",
  "original-asset-manifest.generated.json",
);
const GENERATED_DATA_DIR = path.join(WORKSPACE_ROOT, "lib", "generated");
const LOCAL_CRYSTAL_CLIENT_ROOT = path.join(MIR2_ROOT, "downloads", "crystal-client-full");
const DEFAULT_CRYSTAL_CLIENT_ROOT = "E:\\mir2\\Crystal\\Build\\Client\\Debug";
const CRYSTAL_CLIENT_ROOT = process.env.CRYSTAL_CLIENT_ROOT
  ? path.resolve(process.env.CRYSTAL_CLIENT_ROOT)
  : existsSync(LOCAL_CRYSTAL_CLIENT_ROOT)
    ? LOCAL_CRYSTAL_CLIENT_ROOT
    : DEFAULT_CRYSTAL_CLIENT_ROOT;
const MAP_DIR = path.join(CRYSTAL_CLIENT_ROOT, "Map");
const DATA_MAP_DIR = path.join(CRYSTAL_CLIENT_ROOT, "Data", "Map");
const RESPAWN_MANIFEST_PATH = path.join(
  GENERATED_DATA_DIR,
  "crystal_respawn_manifest.json",
);
const PACKAGED_MAP_DIR = path.join(GENERATED_DATA_DIR, "crystal-map-pack");
const PACKAGED_MAP_LIBRARY_META_DIR = path.join(GENERATED_DATA_DIR, "crystal-map-library-meta");
const PACKAGED_STARTER_MAP_REGION_PATH = path.join(
  GENERATED_DATA_DIR,
  "crystal_starter_map_region.json",
);
const PACKAGED_STARTER_MAP_COLLISION_PATH = path.join(
  GENERATED_DATA_DIR,
  "crystal_starter_map_collision.json",
);
const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const CELL_WIDTH = 48;
const CELL_HEIGHT = 32;
const DEFAULT_SCENE_WIDTH = 24;
const DEFAULT_SCENE_HEIGHT = 18;
const IS_DEVELOPMENT = process.env.NODE_ENV === "development";
const REQUEST_FILE_WRITES_ENABLED =
  process.env.MIR2_ENABLE_REQUEST_FILE_WRITES === "1" ||
  (IS_DEVELOPMENT && process.env.MIR2_DISABLE_REQUEST_FILE_WRITES !== "1");
const SYNTHETIC_MAP_FALLBACK_ENABLED = process.env.MIR2_ALLOW_SYNTHETIC_MAP_FALLBACK === "1";
const SERVER_MAP_CACHE_MAX_BYTES = parseByteBudget(process.env.MIR2_MAP_CACHE_MAX_BYTES, 64 * 1024 * 1024);
const SERVER_LIBRARY_CACHE_MAX_BYTES = parseByteBudget(process.env.MIR2_LIBRARY_CACHE_MAX_BYTES, 24 * 1024 * 1024);
const DECODED_FRAME_CACHE_MAX_BYTES = parseByteBudget(process.env.MIR2_DECODED_FRAME_CACHE_MAX_BYTES, 64 * 1024 * 1024);

type CrystalMapManifest = {
  maps?: CrystalMapManifestEntry[];
};

type CrystalMapManifestEntry = {
  light?: number;
  map_file_name?: string;
  map_title?: string;
  mini_map?: number;
  big_map?: number;
  movements?: Array<{
    source?: { x?: number; y?: number };
    destination?: { x?: number; y?: number };
  }>;
  respawns?: Array<{ location?: { x?: number; y?: number } }>;
  safe_zones?: Array<{ location?: { x?: number; y?: number } }>;
};

type PackagedMapCollision = {
  blockedCells?: Array<{ x?: number; y?: number }>;
  doors?: Array<{ x?: number; y?: number; closed?: boolean }>;
};

type ParsedMap = {
  fileName: string;
  width: number;
  height: number;
  type: number;
  cells: ParsedMapCell[] | null;
  fallbackOriginalMapRegion?: OriginalMapRegion | null;
};

type ParsedMapCell = {
  x: number;
  y: number;
  backIndex: number;
  backImage: number;
  middleIndex: number;
  middleImage: number;
  frontIndex: number;
  frontImage: number;
  doorIndex: number;
  doorOffset: number;
  frontAnimationFrame: number;
  frontAnimationTick: number;
  middleAnimationFrame: number;
  middleAnimationTick: number;
  tileAnimationImage: number;
  tileAnimationOffset: number;
  tileAnimationFrames: number;
  light: number;
};

type ParsedLibrary = {
  version: number;
  count: number;
  filePath?: string;
  frameOffsets?: number[];
  frames: Array<ParsedFrame | null>;
};

type ParsedFrame = {
  index: number;
  width: number;
  height: number;
  x: number;
  y: number;
  dataOffset?: number;
  dataLength?: number;
};

type ExportRegionOptions = {
  mapFileName?: string | null;
  centerX?: number | null;
  centerY?: number | null;
  width?: number | null;
  height?: number | null;
};

type SizedCacheEntry<T> = {
  value: T;
  bytes: number;
};

type DecodedFrameCacheEntry = {
  rgba: Buffer;
  bytes: number;
};

type OriginalAssetManifestPayload = {
  schemaVersion?: number;
  assets?: Record<string, unknown>;
};

const mapCache = new Map<string, SizedCacheEntry<ParsedMap>>();
const libraryCache = new Map<string, SizedCacheEntry<ParsedLibrary | null>>();
const decodedFrameCache = new Map<string, DecodedFrameCacheEntry>();
const exportedFrames = new Set<string>();
let mapCacheBytes = 0;
let libraryCacheBytes = 0;
let decodedFrameCacheBytes = 0;
let originalAssetManifestPaths: Set<string> | null | undefined;
const resourceStats = {
  libraryParseCount: 0,
  frameHeaderReadCount: 0,
  frameDecodeCount: 0,
  decodedFrameCacheHits: 0,
  decodedFrameCacheMisses: 0,
};

export class CrystalResourceMissingError extends Error {
  readonly code = "resource_missing";
  readonly resourceType: "map" | "library" | "frame" | "png";
  readonly resourcePath: string;
  readonly mapFileName?: string;
  readonly libraryKey?: string;
  readonly frameIndex?: number;

  constructor({
    message,
    resourceType,
    resourcePath,
    mapFileName,
    libraryKey,
    frameIndex,
  }: {
    message: string;
    resourceType: "map" | "library" | "frame" | "png";
    resourcePath: string;
    mapFileName?: string;
    libraryKey?: string;
    frameIndex?: number;
  }) {
    super(message);
    this.name = "CrystalResourceMissingError";
    this.resourceType = resourceType;
    this.resourcePath = resourcePath;
    this.mapFileName = mapFileName;
    this.libraryKey = libraryKey;
    this.frameIndex = frameIndex;
  }
}

export function isCrystalResourceMissingError(error: unknown): error is CrystalResourceMissingError {
  return error instanceof CrystalResourceMissingError || (
    typeof error === "object" &&
    error !== null &&
    (error as { code?: unknown }).code === "resource_missing"
  );
}

function originalAssetManifestHasPublicPath(publicPath: string) {
  const normalizedPath = normalizeOriginalAssetPublicPath(publicPath);
  if (!normalizedPath) {
    return false;
  }
  const manifestPaths = loadOriginalAssetManifestPaths();
  return manifestPaths?.has(normalizedPath) ?? false;
}

function loadOriginalAssetManifestPaths() {
  if (originalAssetManifestPaths !== undefined) {
    return originalAssetManifestPaths;
  }

  try {
    const payload = JSON.parse(
      readFileSync(/* turbopackIgnore: true */ PUBLIC_ORIGINAL_ASSET_MANIFEST_PATH, "utf8"),
    ) as OriginalAssetManifestPayload;
    originalAssetManifestPaths = new Set(
      Object.keys(payload.assets ?? {})
        .map(normalizeOriginalAssetPublicPath)
        .filter((value): value is string => Boolean(value)),
    );
    return originalAssetManifestPaths;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
    originalAssetManifestPaths = null;
    if (originalAssetManifestRequired()) {
      throw new CrystalResourceMissingError({
        message: `Original asset manifest is missing: ${PUBLIC_ORIGINAL_ASSET_MANIFEST_PATH}`,
        resourceType: "png",
        resourcePath: PUBLIC_ORIGINAL_ASSET_MANIFEST_PATH,
      });
    }
    return null;
  }
}

function originalAssetManifestRequired() {
  return !IS_DEVELOPMENT && process.env.MIR2_DISABLE_ORIGINAL_ASSET_MANIFEST !== "1";
}

function normalizeOriginalAssetPublicPath(publicPath: string | null | undefined) {
  if (typeof publicPath !== "string" || !publicPath.trim()) {
    return null;
  }
  let pathname = publicPath.trim();
  try {
    pathname = new URL(pathname, "https://mir2.local").pathname;
  } catch {
    pathname = pathname.split("?")[0]?.split("#")[0] ?? pathname;
  }
  pathname = `/${pathname.replace(/^\/+/, "")}`;
  if (!pathname.startsWith("/original-map/") && !pathname.startsWith("/original-ui/")) {
    return null;
  }
  return pathname;
}

export async function loadCrystalSceneBlueprint(options: ExportRegionOptions = {}): Promise<SceneBlueprint> {
  const mapFileName = normalizeMapFileName(options.mapFileName ?? "0");
  const mapInfo = mapManifestEntry(mapFileName);
  const parsedMap = loadParsedMap(mapFileName);
  const fallbackCenter = fallbackCenterForMap(mapFileName, parsedMap);
  const center = {
    x: clampInt(options.centerX ?? fallbackCenter.x, 0, Math.max(parsedMap.width - 1, 0)),
    y: clampInt(options.centerY ?? fallbackCenter.y, 0, Math.max(parsedMap.height - 1, 0)),
  };
  const width = clampInt(options.width ?? DEFAULT_SCENE_WIDTH, 8, 48);
  const height = clampInt(options.height ?? DEFAULT_SCENE_HEIGHT, 8, 36);
  const sceneView = { center, width, height };

  return {
    mapTitle: mapInfo?.map_title ?? null,
    miniMapIndex: miniMapIndexForMapFileName(mapFileName, mapInfo),
    bigMapIndex: bigMapIndexForMapFileName(mapFileName, mapInfo),
    sceneView,
    terrainPatches: terrainPatchesForMap(mapFileName, parsedMap, mapInfo),
    decorObjects: [],
    originalMapRegion: await exportMapRegion(parsedMap, sceneView),
  };
}

function loadParsedMap(mapFileName: string): ParsedMap {
  const normalized = normalizeMapFileName(mapFileName);
  const cached = mapCache.get(normalized);
  if (cached) {
    touchSizedCacheEntry(mapCache, normalized, cached);
    return cached.value;
  }

  const mapPath = path.join(MAP_DIR, `${normalized}.map`);
  if (!existsSync(mapPath)) {
    const packaged = loadPackagedMap(normalized);
    if (packaged) {
      rememberMap(normalized, packaged);
      return packaged;
    }
    if (!SYNTHETIC_MAP_FALLBACK_ENABLED) {
      throw new CrystalResourceMissingError({
        message: `Crystal map ${normalized}.map is missing and synthetic fallback is disabled`,
        resourceType: "map",
        resourcePath: mapPath,
        mapFileName: normalized,
      });
    }
    const parsed = normalized === "0" ? loadPackagedFallbackMap("0") : loadMissingMapFallback(normalized);
    rememberMap(normalized, parsed);
    return parsed;
  }

  const bytes = readFileSync(mapPath);
  const parsed = parseMapBytes(`${normalized}.map`, bytes);
  rememberMap(normalized, parsed);
  return parsed;
}

function parseMapBytes(fileName: string, bytes: Buffer): ParsedMap {
  const type = detectMapType(bytes);
  switch (type) {
    case 100:
      return parseType100Map(fileName, bytes);
    case 0:
      return parseType0Map(fileName, bytes);
    case 1:
      return parseType1Map(fileName, bytes);
    case 2:
      return parseType2Map(fileName, bytes);
    case 3:
      return parseType3Map(fileName, bytes);
    case 4:
      return parseType4Map(fileName, bytes);
    case 5:
      return parseType5Map(fileName, bytes);
    case 6:
      return parseType6Map(fileName, bytes);
    case 7:
      return parseType7Map(fileName, bytes);
  }

  return {
    fileName,
    width: detectMapWidth(bytes, type),
    height: detectMapHeight(bytes, type),
    type,
    cells: null,
  };
}

function parseType0Map(fileName: string, bytes: Buffer): ParsedMap {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells: ParsedMapCell[] = [];
  let offset = 52;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 12 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backIndex: 0,
        backImage: normalizeBackImage(bytes.readInt16LE(offset)),
        middleIndex: 1,
        middleImage: bytes.readInt16LE(offset + 2),
        frontImage: bytes.readInt16LE(offset + 4),
        doorIndex: bytes.readUInt8(offset + 6) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 7),
        frontAnimationFrame: bytes.readUInt8(offset + 8),
        frontAnimationTick: bytes.readUInt8(offset + 9),
        frontIndex: bytes.readUInt8(offset + 10) + 2,
        light: bytes.readUInt8(offset + 11),
      });
      offset += 12;
    }
  }
  return { fileName, width, height, type: 0, cells };
}

function parseType1Map(fileName: string, bytes: Buffer): ParsedMap {
  const xor = bytes.readInt16LE(23);
  const width = bytes.readInt16LE(21) ^ xor;
  const height = bytes.readInt16LE(25) ^ xor;
  const cells: ParsedMapCell[] = [];
  let offset = 54;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 15 > bytes.length) break;
      let frontIndex = bytes.readUInt8(offset + 12) + 2;
      if (frontIndex === 102) frontIndex = 90;
      if (frontIndex >= 255) frontIndex = -1;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backIndex: 0,
        backImage: (bytes.readInt32LE(offset) ^ 0xaa38aa38) | 0,
        middleIndex: 1,
        middleImage: signed16(bytes.readInt16LE(offset + 4) ^ xor),
        frontImage: signed16(bytes.readInt16LE(offset + 6) ^ xor),
        doorIndex: bytes.readUInt8(offset + 8) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 9),
        frontAnimationFrame: bytes.readUInt8(offset + 10),
        frontAnimationTick: bytes.readUInt8(offset + 11),
        frontIndex,
        light: bytes.readUInt8(offset + 13),
      });
      offset += 15;
    }
  }
  return { fileName, width, height, type: 1, cells };
}

function parseType2Map(fileName: string, bytes: Buffer): ParsedMap {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells: ParsedMapCell[] = [];
  let offset = 52;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 14 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backImage: normalizeBackImage(bytes.readInt16LE(offset)),
        middleImage: bytes.readInt16LE(offset + 2),
        frontImage: bytes.readInt16LE(offset + 4),
        doorIndex: bytes.readUInt8(offset + 6) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 7),
        frontAnimationFrame: bytes.readUInt8(offset + 8),
        frontAnimationTick: bytes.readUInt8(offset + 9),
        frontIndex: bytes.readUInt8(offset + 10) + 120,
        light: bytes.readUInt8(offset + 11),
        backIndex: bytes.readUInt8(offset + 12) + 100,
        middleIndex: bytes.readUInt8(offset + 13) + 110,
      });
      offset += 14;
    }
  }
  return { fileName, width, height, type: 2, cells };
}

function parseType3Map(fileName: string, bytes: Buffer): ParsedMap {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells: ParsedMapCell[] = [];
  let offset = 52;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 36 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backImage: normalizeBackImage(bytes.readInt16LE(offset)),
        middleImage: bytes.readInt16LE(offset + 2),
        frontImage: bytes.readInt16LE(offset + 4),
        doorIndex: bytes.readUInt8(offset + 6) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 7),
        frontAnimationFrame: bytes.readUInt8(offset + 8),
        frontAnimationTick: bytes.readUInt8(offset + 9),
        frontIndex: bytes.readUInt8(offset + 10) + 120,
        light: bytes.readUInt8(offset + 11),
        backIndex: bytes.readUInt8(offset + 12) + 100,
        middleIndex: bytes.readUInt8(offset + 13) + 110,
        tileAnimationImage: bytes.readInt16LE(offset + 14),
        tileAnimationFrames: bytes.readUInt8(offset + 21),
        tileAnimationOffset: bytes.readInt16LE(offset + 22),
      });
      offset += 36;
    }
  }
  return { fileName, width, height, type: 3, cells };
}

function parseType4Map(fileName: string, bytes: Buffer): ParsedMap {
  const xor = bytes.readInt16LE(33);
  const width = bytes.readInt16LE(31) ^ xor;
  const height = bytes.readInt16LE(35) ^ xor;
  const cells: ParsedMapCell[] = [];
  let offset = 64;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 12 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backIndex: 0,
        backImage: normalizeBackImage(signed16(bytes.readInt16LE(offset) ^ xor)),
        middleIndex: 1,
        middleImage: signed16(bytes.readInt16LE(offset + 2) ^ xor),
        frontImage: signed16(bytes.readInt16LE(offset + 4) ^ xor),
        doorIndex: bytes.readUInt8(offset + 6) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 7),
        frontAnimationFrame: bytes.readUInt8(offset + 8),
        frontAnimationTick: bytes.readUInt8(offset + 9),
        frontIndex: bytes.readUInt8(offset + 10) + 2,
        light: bytes.readUInt8(offset + 11),
      });
      offset += 12;
    }
  }
  return { fileName, width, height, type: 4, cells };
}

function parseType5Map(fileName: string, bytes: Buffer): ParsedMap {
  const width = bytes.readInt16LE(22);
  const height = bytes.readInt16LE(24);
  const cells = createEmptyCellGrid(width, height);
  let offset = 28;

  for (let x = 0; x < Math.floor(width / 2); x += 1) {
    for (let y = 0; y < Math.floor(height / 2); y += 1) {
      if (offset + 3 > bytes.length) break;
      const backIndex = bytes.readUInt8(offset) !== 255 ? bytes.readUInt8(offset) + 200 : -1;
      const backImage = bytes.readUInt16LE(offset + 1) + 1;
      for (let index = 0; index < 4; index += 1) {
        const cell = cells[(x * 2 + (index % 2)) * height + (y * 2 + Math.floor(index / 2))];
        if (!cell) continue;
        cell.backIndex = backIndex;
        cell.backImage = backImage;
      }
      offset += 3;
    }
  }

  offset = 28 + 3 * (Math.floor(width / 2) + (width % 2)) * Math.floor(height / 2);
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 14 > bytes.length) break;
      const cell = cells[x * height + y];
      const flag = bytes.readUInt8(offset);
      cell.middleAnimationFrame = bytes.readUInt8(offset + 1);
      cell.frontAnimationFrame = bytes.readUInt8(offset + 2) === 255 ? 0 : bytes.readUInt8(offset + 2) & 0x8f;
      cell.middleAnimationTick = 0;
      cell.frontAnimationTick = 0;
      cell.frontIndex = bytes.readUInt8(offset + 3) !== 255 ? bytes.readUInt8(offset + 3) + 200 : -1;
      cell.middleIndex = bytes.readUInt8(offset + 4) !== 255 ? bytes.readUInt8(offset + 4) + 200 : -1;
      cell.middleImage = bytes.readUInt16LE(offset + 5) + 1;
      cell.frontImage = bytes.readUInt16LE(offset + 7) + 1;
      if (cell.frontImage === 1 && cell.frontIndex === 200) cell.frontIndex = -1;
      cell.light = (bytes.readUInt8(offset + 12) & 0x0f) * 2;
      if ((flag & 0x01) !== 1) cell.backImage |= 0x20000000;
      if ((flag & 0x02) !== 2) cell.frontImage |= 0x8000;
      offset += 14;
    }
  }

  return { fileName, width, height, type: 5, cells };
}

function parseType6Map(fileName: string, bytes: Buffer): ParsedMap {
  const width = bytes.readInt16LE(16);
  const height = bytes.readInt16LE(18);
  const cells: ParsedMapCell[] = [];
  let offset = 40;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 20 > bytes.length) break;
      const flag = bytes.readUInt8(offset);
      let frontAnimationFrame = bytes.readUInt8(offset + 11) === 255 ? 0 : bytes.readUInt8(offset + 11);
      if (frontAnimationFrame > 0x0f) frontAnimationFrame &= 0x0f;
      let frontIndex = bytes.readUInt8(offset + 3) !== 255 ? bytes.readUInt8(offset + 3) + 300 : -1;
      const baseFrontImage = bytes.readInt16LE(offset + 8) + 1;
      if (baseFrontImage === 1 && frontIndex === 200) frontIndex = -1;
      const cell = {
        ...emptyParsedMapCell(x, y),
        backIndex: bytes.readUInt8(offset + 1) !== 255 ? bytes.readUInt8(offset + 1) + 300 : -1,
        middleIndex: bytes.readUInt8(offset + 2) !== 255 ? bytes.readUInt8(offset + 2) + 300 : -1,
        frontIndex,
        backImage: bytes.readInt16LE(offset + 4) + 1,
        middleImage: bytes.readInt16LE(offset + 6) + 1,
        frontImage: (flag & 0x02) !== 2 ? baseFrontImage | 0x8000 : baseFrontImage,
        middleAnimationFrame: bytes.readUInt8(offset + 10),
        frontAnimationFrame,
        middleAnimationTick: 1,
        frontAnimationTick: 1,
        light: (bytes.readUInt8(offset + 12) & 0x0f) * 4,
      };
      if ((flag & 0x01) !== 1) cell.backImage |= 0x20000000;
      cells.push(cell);
      offset += 20;
    }
  }
  return { fileName, width, height, type: 6, cells };
}

function parseType7Map(fileName: string, bytes: Buffer): ParsedMap {
  const width = bytes.readInt16LE(21);
  const height = bytes.readInt16LE(25);
  const cells: ParsedMapCell[] = [];
  let offset = 54;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 15 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backIndex: 0,
        backImage: normalizeBackImage(bytes.readInt32LE(offset)),
        middleIndex: 1,
        middleImage: bytes.readInt16LE(offset + 4),
        frontImage: bytes.readInt16LE(offset + 6),
        doorIndex: bytes.readUInt8(offset + 8) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 9),
        frontAnimationFrame: bytes.readUInt8(offset + 10),
        frontAnimationTick: bytes.readUInt8(offset + 11),
        frontIndex: bytes.readUInt8(offset + 12) + 2,
        light: bytes.readUInt8(offset + 13),
      });
      offset += 15;
    }
  }
  return { fileName, width, height, type: 7, cells };
}

function emptyParsedMapCell(x: number, y: number): ParsedMapCell {
  return {
    x,
    y,
    backIndex: -1,
    backImage: 0,
    middleIndex: -1,
    middleImage: 0,
    frontIndex: -1,
    frontImage: 0,
    doorIndex: 0,
    doorOffset: 0,
    frontAnimationFrame: 0,
    frontAnimationTick: 0,
    middleAnimationFrame: 0,
    middleAnimationTick: 0,
    tileAnimationImage: 0,
    tileAnimationOffset: 0,
    tileAnimationFrames: 0,
    light: 0,
  };
}

function createEmptyCellGrid(width: number, height: number) {
  const cells: ParsedMapCell[] = [];
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      cells.push(emptyParsedMapCell(x, y));
    }
  }
  return cells;
}

function normalizeBackImage(image: number) {
  return (image & 0x8000) !== 0 ? (image & 0x7fff) | 0x20000000 : image;
}

function signed16(value: number) {
  return (value << 16) >> 16;
}

function parseType100Map(fileName: string, bytes: Buffer): ParsedMap {
  const width = bytes.readInt16LE(4);
  const height = bytes.readInt16LE(6);
  const cells: ParsedMapCell[] = [];
  let offset = 8;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 26 > bytes.length) break;
      cells.push({
        x,
        y,
        backIndex: bytes.readInt16LE(offset),
        backImage: bytes.readInt32LE(offset + 2),
        middleIndex: bytes.readInt16LE(offset + 6),
        middleImage: bytes.readInt16LE(offset + 8),
        frontIndex: bytes.readInt16LE(offset + 10),
        frontImage: bytes.readInt16LE(offset + 12),
        doorIndex: bytes.readUInt8(offset + 14) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 15),
        frontAnimationFrame: bytes.readUInt8(offset + 16),
        frontAnimationTick: bytes.readUInt8(offset + 17),
        middleAnimationFrame: bytes.readUInt8(offset + 18),
        middleAnimationTick: bytes.readUInt8(offset + 19),
        tileAnimationImage: bytes.readInt16LE(offset + 20),
        tileAnimationOffset: bytes.readInt16LE(offset + 22),
        tileAnimationFrames: bytes.readUInt8(offset + 24),
        light: bytes.readUInt8(offset + 25),
      });
      offset += 26;
    }
  }
  return { fileName, width, height, type: 100, cells };
}

async function exportMapRegion(
  parsedMap: ParsedMap,
  sceneView: { center: { x: number; y: number }; width: number; height: number },
): Promise<OriginalMapRegion | null> {
  if (parsedMap.fallbackOriginalMapRegion) {
    return {
      ...parsedMap.fallbackOriginalMapRegion,
      mapFileName: parsedMap.fileName,
    };
  }

  if (!parsedMap.cells) {
    return fallbackMapRegion(parsedMap, sceneView);
  }

  const bounds = exportBoundsForScene(sceneView, parsedMap);
  const sprites: OriginalMapRegion["sprites"] = {};
  const cells: OriginalMapRegion["cells"] = [];
  const spriteIds = new Map<string, string>();
  const pendingWrites: Array<Promise<void>> = [];

  for (let x = bounds.minX; x <= bounds.maxX; x += 1) {
    for (let y = bounds.minY; y <= bounds.maxY; y += 1) {
      const cell = parsedCellAt(parsedMap, x, y);
      if (!cell) continue;

      const outputCell: OriginalMapRegion["cells"][number] = { x: cell.x, y: cell.y };
      if (parsedCellBlocksMovement(cell)) {
        outputCell.blocked = true;
      }
      if (cell.x % 2 === 0 && cell.y % 2 === 0) {
        const backLayer = backLayerForCell(cell);
        if (backLayer) outputCell.back = registerSprite(backLayer, "back");
      }

      const middleLayer = middleLayerForCell(cell);
      if (middleLayer) outputCell.middle = registerSprite(middleLayer, "middle");

      const frontLayer = frontLayerForCell(cell);
      if (frontLayer) outputCell.front = registerSprite(frontLayer, "front");

      const tileAnimationLayer = tileAnimationLayerForCell(cell);
      if (tileAnimationLayer) outputCell.tileAnimation = registerSprite(tileAnimationLayer, "tileAnimation");

      if (Object.keys(outputCell).length > 2) {
        cells.push(outputCell);
      }
    }
  }

  await Promise.all(pendingWrites);
  return {
    mapFileName: parsedMap.fileName,
    mapWidth: parsedMap.width,
    mapHeight: parsedMap.height,
    cellWidth: CELL_WIDTH,
    cellHeight: CELL_HEIGHT,
    regionBounds: bounds,
    playBounds: playBoundsForScene(sceneView, parsedMap),
    sprites,
    cells,
  };

  function registerSprite(layer: { libraryKey: string; drawMode: "auto" | "floor" | "object"; frames: number[] }, kind: "back" | "middle" | "front" | "tileAnimation") {
    const library = ensureLibrary(layer.libraryKey);
    if (!library) return null;

    const drawMode = resolveDrawMode(layer, library);
    const spriteKey = `${kind}|${layer.libraryKey}|${drawMode}|${layer.frames.join(",")}`;
    const existingId = spriteIds.get(spriteKey);
    if (existingId) return existingId;

    const frames = layer.frames
      .map((frameIndex) => exportFrame(layer.libraryKey, library, frameIndex, pendingWrites))
      .filter((frame): frame is OriginalMapSpriteFrame => frame !== null);
    if (!frames.length) return null;

    const id = `sprite-${spriteIds.size + 1}`;
    sprites[id] = { kind, drawMode, frames };
    spriteIds.set(spriteKey, id);
    return id;
  }
}

function parsedCellAt(parsedMap: ParsedMap, x: number, y: number) {
  if (!parsedMap.cells || x < 0 || y < 0 || x >= parsedMap.width || y >= parsedMap.height) {
    return null;
  }

  const cell = parsedMap.cells[x * parsedMap.height + y];
  return cell?.x === x && cell.y === y ? cell : null;
}

function exportFrame(
  libraryKey: string,
  library: ParsedLibrary,
  frameIndex: number,
  pendingWrites: Array<Promise<void>>,
): OriginalMapSpriteFrame | null {
  const frame = readLibraryFrameMeta(library, frameIndex);
  if (!frame || frame.width <= 0 || frame.height <= 0) return null;

  const normalizedKey = normalizeLibraryName(libraryKey);
  const frameKey = `${normalizedKey}:${frameIndex}`;
  const exportDir = path.join(/* turbopackIgnore: true */ PUBLIC_ORIGINAL_MAP_DIR, ...normalizedKey.split("/"));
  const pngPath = path.join(/* turbopackIgnore: true */ exportDir, `${frameIndex}.png`);
  const publicPath = `/original-map/${normalizedKey}/${frameIndex}.png`;
  const pngExists = existsSync(/* turbopackIgnore: true */ pngPath);
  if (!pngExists && !REQUEST_FILE_WRITES_ENABLED && !originalAssetManifestHasPublicPath(publicPath)) {
    throw new CrystalResourceMissingError({
      message: `Pre-exported map frame PNG is missing from local public assets and original asset manifest: ${publicPath}`,
      resourceType: "png",
      resourcePath: publicPath,
      libraryKey: normalizedKey,
      frameIndex,
    });
  }
  if (!exportedFrames.has(frameKey) && !pngExists && REQUEST_FILE_WRITES_ENABLED) {
    exportedFrames.add(frameKey);
    const decoded = decodeLibraryFrameRgba(library, frameIndex);
    if (!decoded) return null;
    const rgba = postProcessFrameRgba(normalizedKey, frameIndex, decoded);
    pendingWrites.push(
      mkdir(exportDir, { recursive: true }).then(() => writeFile(pngPath, encodePng(frame.width, frame.height, rgba))),
    );
  }

  return {
    path: publicPath,
    width: frame.width,
    height: frame.height,
    offsetX: frame.x,
    offsetY: frame.y,
  };
}

function fallbackMapRegion(
  parsedMap: ParsedMap,
  sceneView: { center: { x: number; y: number }; width: number; height: number },
): OriginalMapRegion {
  const bounds = exportBoundsForScene(sceneView, parsedMap);
  return {
    mapFileName: parsedMap.fileName,
    mapWidth: parsedMap.width,
    mapHeight: parsedMap.height,
    cellWidth: CELL_WIDTH,
    cellHeight: CELL_HEIGHT,
    regionBounds: bounds,
    playBounds: playBoundsForScene(sceneView, parsedMap),
    sprites: {},
    cells: [],
  };
}

function loadPackagedFallbackMap(fileName: string): ParsedMap {
  const region = loadPackagedStarterMapRegion();
  return {
    fileName,
    width: region?.mapWidth ?? 400,
    height: region?.mapHeight ?? 400,
    type: -1,
    cells: null,
    fallbackOriginalMapRegion: region,
  };
}

function loadMissingMapFallback(fileName: string): ParsedMap {
  const dimensions = missingMapDimensionsForMap(fileName);
  return {
    fileName,
    width: dimensions.width,
    height: dimensions.height,
    type: -1,
    cells: null,
    fallbackOriginalMapRegion: null,
  };
}

function missingMapDimensionsForMap(fileName: string) {
  const mapInfo = mapManifestEntry(fileName);
  const points: Array<{ x?: number; y?: number }> = [];

  for (const zone of mapInfo?.safe_zones ?? []) {
    if (zone.location) points.push(zone.location);
  }
  for (const respawn of mapInfo?.respawns ?? []) {
    if (respawn.location) points.push(respawn.location);
  }
  for (const movement of mapInfo?.movements ?? []) {
    if (movement.source) points.push(movement.source);
    if (movement.destination) points.push(movement.destination);
  }

  const maxX = Math.max(0, ...points.map((point) => Math.trunc(point.x ?? 0)));
  const maxY = Math.max(0, ...points.map((point) => Math.trunc(point.y ?? 0)));
  return {
    width: Math.max(96, maxX + 64),
    height: Math.max(96, maxY + 64),
  };
}

function loadPackagedStarterMapRegion(): OriginalMapRegion | null {
  try {
    const source = JSON.parse(readFileSync(PACKAGED_STARTER_MAP_REGION_PATH, "utf8")) as Partial<OriginalMapRegion>;
    const maxCellX = Math.max(...(source.cells ?? []).map((cell) => cell.x), source.regionBounds?.maxX ?? 0);
    const maxCellY = Math.max(...(source.cells ?? []).map((cell) => cell.y), source.regionBounds?.maxY ?? 0);
    return withPackagedStarterCollision({
      mapFileName: source.mapFileName ?? "0",
      mapWidth: source.mapWidth ?? maxCellX + 1,
      mapHeight: source.mapHeight ?? maxCellY + 1,
      cellWidth: source.cellWidth ?? CELL_WIDTH,
      cellHeight: source.cellHeight ?? CELL_HEIGHT,
      regionBounds: source.regionBounds ?? { minX: 0, maxX: maxCellX, minY: 0, maxY: maxCellY },
      playBounds: source.playBounds ?? { minX: 0, maxX: maxCellX, minY: 0, maxY: maxCellY },
      sprites: source.sprites ?? {},
      cells: source.cells ?? [],
    });
  } catch {
    return null;
  }
}

function withPackagedStarterCollision(region: OriginalMapRegion): OriginalMapRegion {
  try {
    const collision = JSON.parse(readFileSync(PACKAGED_STARTER_MAP_COLLISION_PATH, "utf8")) as PackagedMapCollision;
    const cellsByKey = new Map(region.cells.map((cell) => [`${cell.x}:${cell.y}`, { ...cell }]));

    for (const point of collision.blockedCells ?? []) {
      const x = Math.trunc(point.x ?? Number.NaN);
      const y = Math.trunc(point.y ?? Number.NaN);
      if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
      if (!pointInRegion(region.regionBounds, x, y)) continue;
      const key = `${x}:${y}`;
      cellsByKey.set(key, { ...(cellsByKey.get(key) ?? { x, y }), blocked: true });
    }

    for (const door of collision.doors ?? []) {
      if (!door.closed) continue;
      const x = Math.trunc(door.x ?? Number.NaN);
      const y = Math.trunc(door.y ?? Number.NaN);
      if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
      if (!pointInRegion(region.regionBounds, x, y)) continue;
      const key = `${x}:${y}`;
      cellsByKey.set(key, { ...(cellsByKey.get(key) ?? { x, y }), closedDoor: true });
    }

    return { ...region, cells: [...cellsByKey.values()] };
  } catch {
    return region;
  }
}

function pointInRegion(
  bounds: { minX: number; maxX: number; minY: number; maxY: number },
  x: number,
  y: number,
) {
  return x >= bounds.minX && x <= bounds.maxX && y >= bounds.minY && y <= bounds.maxY;
}

function backLayerForCell(cell: ParsedMapCell) {
  if (cell.backIndex < 0 || cell.backImage === 0) return null;
  const frameIndex = (cell.backImage & 0x1fffffff) - 1;
  if (frameIndex < 0) return null;
  return { libraryKey: mapLibraryKeyForIndex(cell.backIndex), drawMode: "floor" as const, frames: [frameIndex] };
}

function parsedCellBlocksMovement(cell: ParsedMapCell) {
  return (cell.backImage & 0x20000000) !== 0 || (cell.frontImage & 0x8000) !== 0;
}

function middleLayerForCell(cell: ParsedMapCell) {
  if (cell.middleIndex < 0) return null;
  const baseFrameIndex = cell.middleImage - 1;
  if (baseFrameIndex < 0) return null;
  return {
    libraryKey: mapLibraryKeyForIndex(cell.middleIndex),
    drawMode: "auto" as const,
    frames: repeatedAnimationFrames(
      baseFrameIndex,
      decodeMiddleAnimationCount(cell.middleAnimationFrame),
      cell.middleAnimationTick,
    ),
  };
}

function frontLayerForCell(cell: ParsedMapCell) {
  if (cell.frontIndex < 0) return null;
  const baseFrameIndex = (cell.frontImage & 0x7fff) - 1;
  if (baseFrameIndex < 0) return null;
  return {
    libraryKey: mapLibraryKeyForIndex(cell.frontIndex),
    drawMode: "auto" as const,
    frames: repeatedAnimationFrames(baseFrameIndex, decodeFrontAnimationCount(cell.frontAnimationFrame), cell.frontAnimationTick),
  };
}

function tileAnimationLayerForCell(cell: ParsedMapCell) {
  if (cell.tileAnimationImage <= 0 || cell.tileAnimationFrames <= 0) return null;
  const stride = cell.tileAnimationOffset ^ 0x2000;
  const frames = Array.from({ length: cell.tileAnimationFrames }, (_, index) => cell.tileAnimationImage - 1 + stride * index).filter((index) => index >= 0);
  if (!frames.length) return null;
  return { libraryKey: mapLibraryKeyForIndex(190), drawMode: "object" as const, frames };
}

function ensureLibrary(libraryKey: string) {
  const cached = libraryCache.get(libraryKey);
  if (cached !== undefined) {
    touchSizedCacheEntry(libraryCache, libraryKey, cached);
    return cached.value;
  }

  const libraryPath = path.join(DATA_MAP_DIR, ...libraryKey.split("/")) + ".Lib";
  const alternatePath = path.join(DATA_MAP_DIR, ...libraryKey.split("/")) + ".lib";
  const filePath = existsSync(libraryPath) ? libraryPath : alternatePath;
  if (!existsSync(filePath)) {
    const packaged = loadPackagedMapLibraryMeta(libraryKey);
    if (!packaged) {
      throw new CrystalResourceMissingError({
        message: `Crystal map library is missing: ${libraryKey}`,
        resourceType: "library",
        resourcePath: filePath,
        libraryKey,
      });
    }
    rememberLibrary(libraryKey, packaged);
    return packaged;
  }

  const parsed = parseLibrary(filePath);
  rememberLibrary(libraryKey, parsed);
  return parsed;
}

function loadPackagedMap(mapFileName: string): ParsedMap | null {
  const packagedPath = path.join(PACKAGED_MAP_DIR, `${packagedMapFileStem(mapFileName)}.map.gz`);
  if (!existsSync(packagedPath)) {
    return null;
  }

  const bytes = gunzipSync(readFileSync(packagedPath));
  return parseMapBytes(`${mapFileName}.map`, bytes);
}

function loadPackagedMapLibraryMeta(libraryKey: string): ParsedLibrary | null {
  const normalizedKey = normalizeLibraryName(libraryKey);
  const packagedPath = path.join(PACKAGED_MAP_LIBRARY_META_DIR, ...normalizedKey.split("/")) + ".json.gz";
  if (!existsSync(packagedPath)) {
    return null;
  }

  const source = JSON.parse(gunzipSync(readFileSync(packagedPath)).toString("utf8")) as {
    version?: number;
    count?: number;
    frames?: Array<[number, number, number, number] | null>;
  };
  const count = Math.max(0, Math.trunc(source.count ?? source.frames?.length ?? 0));
  const frames = new Array<ParsedFrame | null>(count).fill(null);
  for (let index = 0; index < Math.min(count, source.frames?.length ?? 0); index += 1) {
    const frame = source.frames?.[index];
    if (!frame) continue;
    frames[index] = {
      index,
      width: frame[0],
      height: frame[1],
      x: frame[2],
      y: frame[3],
    };
  }

  return {
    version: Math.trunc(source.version ?? 0),
    count,
    frames,
  };
}

export function parseLibrary(filePath: string): ParsedLibrary {
  const header = readFileSlice(filePath, 0, 12);
  let offset = 0;
  const version = header.readInt32LE(offset);
  offset += 4;
  if (version < 2) throw new Error(`Unsupported lib version ${version}: ${filePath}`);
  const count = header.readInt32LE(offset);
  offset += 4;
  if (version >= 3) offset += 4;

  const offsetTable = readFileSlice(filePath, offset, count * 4);
  const frameOffsets: number[] = [];
  for (let index = 0; index < count; index += 1) {
    const tableOffset = index * 4;
    frameOffsets.push(tableOffset + 4 <= offsetTable.length ? offsetTable.readInt32LE(tableOffset) : 0);
  }

  const frames = new Array<ParsedFrame | null>(count).fill(null);
  resourceStats.libraryParseCount += 1;
  return { version, count, filePath, frameOffsets, frames };
}

function readLibraryFrameMeta(library: ParsedLibrary, index: number): ParsedFrame | null {
  const cached = library.frames[index];
  if (cached !== undefined && cached !== null) return cached;

  const frameOffset = library.frameOffsets?.[index];
  if (!library.filePath || !frameOffset || frameOffset <= 0) return library.frames[index] = null;

  const header = readFileSlice(library.filePath, frameOffset, 17);
  if (header.length < 17) return library.frames[index] = null;
  const frame = parseFrameHeader(header, frameOffset, index);
  library.frames[index] = frame;
  resourceStats.frameHeaderReadCount += 1;
  return frame;
}

function parseFrameHeader(buffer: Buffer, absoluteOffset: number, index: number): ParsedFrame {
  const width = buffer.readInt16LE(0);
  const height = buffer.readInt16LE(2);
  const x = buffer.readInt16LE(4);
  const y = buffer.readInt16LE(6);
  const length = buffer.readInt32LE(13);
  return {
    index,
    width,
    height,
    x,
    y,
    dataOffset: absoluteOffset + 17,
    dataLength: length,
  };
}

export function decodeLibraryFrameRgba(library: ParsedLibrary, frameIndex: number): Buffer | null {
  const frame = readLibraryFrameMeta(library, frameIndex);
  if (!frame || !library.filePath || frame.dataOffset === undefined || frame.dataLength === undefined) {
    return null;
  }

  const cacheKey = `${library.filePath}:${frameIndex}`;
  const cached = decodedFrameCache.get(cacheKey);
  if (cached) {
    decodedFrameCache.delete(cacheKey);
    decodedFrameCache.set(cacheKey, cached);
    resourceStats.decodedFrameCacheHits += 1;
    return cached.rgba;
  }

  resourceStats.decodedFrameCacheMisses += 1;
  const raw = readFileSlice(library.filePath, frame.dataOffset, frame.dataLength);
  const rgba = decodeFrame(frame.width, frame.height, raw);
  rememberDecodedFrame(cacheKey, rgba);
  return rgba;
}

function detectMapType(bytes: Buffer) {
  if (bytes[2] === 0x43 && bytes[3] === 0x23) return 100;
  if (bytes[0] === 0) return 5;
  if (bytes[0] === 0x0f && bytes[5] === 0x53 && bytes[14] === 0x33) return 6;
  if (bytes[0] === 0x15 && bytes[4] === 0x32 && bytes[6] === 0x41 && bytes[19] === 0x31) return 4;
  if (bytes[0] === 0x10 && bytes[2] === 0x61 && bytes[7] === 0x31 && bytes[14] === 0x31) return 1;
  if (bytes[4] === 0x0f || (bytes[4] === 0x03 && bytes[18] === 0x0d && bytes[19] === 0x0a)) {
    const width = bytes[0] + (bytes[1] << 8);
    const height = bytes[2] + (bytes[3] << 8);
    return bytes.length > 52 + width * height * 14 ? 3 : 2;
  }
  if (bytes[0] === 0x0d && bytes[1] === 0x4c && bytes[7] === 0x20 && bytes[11] === 0x6d) return 7;
  return 0;
}

function detectMapWidth(bytes: Buffer, type: number) {
  if (type === 1) return bytes.readInt16LE(21) ^ bytes.readInt16LE(23);
  if (type === 4) return bytes.readInt16LE(31) ^ bytes.readInt16LE(33);
  if (type === 5) return bytes.readInt16LE(22);
  if (type === 6) return bytes.readInt16LE(16);
  if (type === 7) return bytes.readInt16LE(21);
  return bytes.readInt16LE(0);
}

function detectMapHeight(bytes: Buffer, type: number) {
  if (type === 1) return bytes.readInt16LE(25) ^ bytes.readInt16LE(23);
  if (type === 4) return bytes.readInt16LE(35) ^ bytes.readInt16LE(33);
  if (type === 5) return bytes.readInt16LE(24);
  if (type === 6) return bytes.readInt16LE(18);
  if (type === 7) return bytes.readInt16LE(25);
  return bytes.readInt16LE(2);
}

function mapManifestEntry(mapFileName: string) {
  try {
    const manifest = JSON.parse(readFileSync(RESPAWN_MANIFEST_PATH, "utf8")) as CrystalMapManifest;
    const normalized = normalizeMapFileName(mapFileName);
    return manifest.maps?.find((entry) => normalizeMapFileName(entry.map_file_name ?? "") === normalized) ?? null;
  } catch {
    return null;
  }
}

function fallbackCenterForMap(mapFileName: string, parsedMap: ParsedMap) {
  const mapInfo = mapManifestEntry(mapFileName);
  if (mapInfo) {
    try {
      const manifest = JSON.parse(readFileSync(RESPAWN_MANIFEST_PATH, "utf8")) as { maps?: Array<CrystalMapManifestEntry & { respawns?: Array<{ location?: { x?: number; y?: number } }> }> };
      const normalized = normalizeMapFileName(mapFileName);
      const map = manifest.maps?.find((entry) => normalizeMapFileName(entry.map_file_name ?? "") === normalized);
      const spawn = map?.respawns?.find((entry) => typeof entry.location?.x === "number" && typeof entry.location?.y === "number");
      if (spawn?.location) {
        return { x: spawn.location.x ?? 0, y: spawn.location.y ?? 0 };
      }
    } catch {
      // Fall back below.
    }
  }

  return {
    x: Math.floor(parsedMap.width / 2),
    y: Math.floor(parsedMap.height / 2),
  };
}

function terrainPatchesForMap(
  mapFileName: string,
  parsedMap: ParsedMap,
  mapInfo: CrystalMapManifestEntry | null,
) {
  return [
    {
      x: 0,
      y: 0,
      width: Math.min(parsedMap.width, 2000),
      height: Math.min(parsedMap.height, 2000),
      kind: terrainKindForMap(mapFileName, mapInfo),
    },
  ];
}

function terrainKindForMap(
  mapFileName: string,
  mapInfo: CrystalMapManifestEntry | null,
): "grass" | "dirt" | "road" | "water" | "stone" {
  const normalized = normalizeMapFileName(mapFileName);
  const title = mapInfo?.map_title?.toLowerCase() ?? "";
  if (mapInfo?.light === 2 || /store|shop|house|room|inn|guild|palace|temple|weapon|armor/i.test(title)) {
    return "stone";
  }
  if (/^d|mine|cave|b\d/i.test(normalized)) return "stone";
  if (/desert|sand|3$|^3/i.test(normalized)) return "dirt";
  if (/river|water|sea/i.test(normalized)) return "water";
  return "grass";
}

function miniMapIndexForMapFileName(mapFileName: string, mapInfo: CrystalMapManifestEntry | null) {
  if (typeof mapInfo?.mini_map === "number" && mapInfo.mini_map > 0) {
    return mapInfo.mini_map;
  }

  const transformIndex = miniMapTransformIndexForMapFileName(mapFileName, "mini");
  if (typeof transformIndex === "number" && transformIndex > 0) {
    return transformIndex;
  }

  const normalized = normalizeMapFileName(mapFileName);
  const numeric = Number.parseInt(normalized, 10);
  return Number.isFinite(numeric) ? numeric : null;
}

function bigMapIndexForMapFileName(mapFileName: string, mapInfo: CrystalMapManifestEntry | null) {
  if (typeof mapInfo?.big_map === "number" && mapInfo.big_map > 0) {
    return mapInfo.big_map;
  }

  const transformIndex = miniMapTransformIndexForMapFileName(mapFileName, "big");
  return typeof transformIndex === "number" && transformIndex > 0 ? transformIndex : null;
}

function miniMapTransformIndexForMapFileName(mapFileName: string, kind: "mini" | "big") {
  const normalized = normalizeCrystalMiniMapFileName(mapFileName);
  const transform = CRYSTAL_MINI_MAP_TRANSFORMS.find(
    (entry) => normalizeCrystalMiniMapFileName(entry.mapFileName) === normalized,
  );
  if (!transform) return null;
  return kind === "mini" ? transform.miniMapIndex : transform.bigMapIndex ?? transform.miniMapIndex;
}

export function crystalMapMiniMapIndexForTests(mapFileName: string) {
  return miniMapIndexForMapFileName(mapFileName, null);
}

export function crystalMapBigMapIndexForTests(mapFileName: string) {
  return bigMapIndexForMapFileName(mapFileName, null);
}

export function crystalMapLibraryKeyForIndexForTests(index: number) {
  return mapLibraryKeyForIndex(index);
}

function exportBoundsForScene(
  sceneView: { center: { x: number; y: number }; width: number; height: number },
  parsedMap: ParsedMap,
) {
  const play = playBoundsForScene(sceneView, parsedMap);
  return {
    minX: clampInt(play.minX - Math.floor(sceneView.width / 2) - 4, 0, Math.max(parsedMap.width - 1, 0)),
    maxX: clampInt(play.maxX + Math.floor(sceneView.width / 2) + 4, 0, Math.max(parsedMap.width - 1, 0)),
    minY: clampInt(play.minY - Math.floor(sceneView.height / 2) - 4, 0, Math.max(parsedMap.height - 1, 0)),
    maxY: clampInt(play.maxY + Math.floor(sceneView.height / 2) + 25, 0, Math.max(parsedMap.height - 1, 0)),
  };
}

function playBoundsForScene(
  sceneView: { center: { x: number; y: number }; width: number; height: number },
  parsedMap: ParsedMap,
) {
  const halfWidth = Math.floor(sceneView.width / 2);
  const halfHeight = Math.floor(sceneView.height / 2);
  return {
    minX: clampInt(sceneView.center.x - halfWidth, 0, Math.max(parsedMap.width - 1, 0)),
    maxX: clampInt(sceneView.center.x + halfWidth, 0, Math.max(parsedMap.width - 1, 0)),
    minY: clampInt(sceneView.center.y - halfHeight, 0, Math.max(parsedMap.height - 1, 0)),
    maxY: clampInt(sceneView.center.y + halfHeight, 0, Math.max(parsedMap.height - 1, 0)),
  };
}

function decodeMiddleAnimationCount(animationFrame: number) {
  return animationFrame <= 0 || animationFrame >= 255 ? 0 : animationFrame & 0x0f;
}

function decodeFrontAnimationCount(animationFrame: number) {
  return animationFrame > 0 ? animationFrame & 0x7f : 0;
}

function repeatedAnimationFrames(baseFrameIndex: number, animationCount: number, animationTick: number) {
  if (animationCount <= 0) return [baseFrameIndex];
  const repeat = 1 + Math.max(animationTick, 0);
  const frames: number[] = [];
  for (let frame = 0; frame < animationCount; frame += 1) {
    for (let tick = 0; tick < repeat; tick += 1) frames.push(baseFrameIndex + frame);
  }
  return frames;
}

function resolveDrawMode(layer: { drawMode: "auto" | "floor" | "object"; frames: number[] }, library: ParsedLibrary) {
  if (layer.drawMode !== "auto") return layer.drawMode;
  const frame = readLibraryFrameMeta(library, layer.frames[0]);
  if (!frame) return "floor";
  const floorSized =
    (frame.width === CELL_WIDTH && frame.height === CELL_HEIGHT) ||
    (frame.width === CELL_WIDTH * 2 && frame.height === CELL_HEIGHT * 2);
  return layer.frames.length === 1 && floorSized ? "floor" : "object";
}

function mapLibraryKeyForIndex(index: number) {
  const wemadeMir3 = mapMir3LibraryKey(index, 200, "WemadeMir3");
  if (wemadeMir3) return wemadeMir3;
  const shandaMir3 = mapMir3LibraryKey(index, 300, "ShandaMir3");
  if (shandaMir3) return shandaMir3;

  if (index === 0) return "WemadeMir2/Tiles";
  if (index === 1) return "WemadeMir2/SmTiles";
  if (index === 2) return "WemadeMir2/Objects";
  if (index >= 3 && index <= 29) return `WemadeMir2/Objects${index - 1}`;
  if (index === 90) return "WemadeMir2/Objects_32bit";
  if (index === 100) return "ShandaMir2/Tiles";
  if (index >= 101 && index <= 109) return `ShandaMir2/Tiles${index - 99}`;
  if (index === 110) return "ShandaMir2/SmTiles";
  if (index >= 111 && index <= 119) return `ShandaMir2/SmTiles${index - 109}`;
  if (index === 120) return "ShandaMir2/Objects";
  if (index >= 121 && index <= 150) return `ShandaMir2/Objects${index - 119}`;
  if (index === 190) return "ShandaMir2/AniTiles1";
  return "WemadeMir2/Tiles";
}

function mapMir3LibraryKey(index: number, baseIndex: 200 | 300, root: "WemadeMir3" | "ShandaMir3") {
  const offset = index - baseIndex;
  if (offset < 0 || offset >= 75) return null;

  const stateIndex = Math.floor(offset / 15);
  const slot = offset % 15;
  const names = [
    "Tilesc",
    "Tiles30c",
    "Tiles5c",
    "SmTilesc",
    "Housesc",
    "Cliffsc",
    "Dungeonsc",
    "Innersc",
    "Furnituresc",
    "Wallsc",
    "SmObjectsc",
    "Animationsc",
    "Object1c",
    "Object2c",
  ];
  const name = names[slot];
  if (!name) return null;

  if (root === "WemadeMir3" && (name === "Object1c" || name === "Object2c")) {
    return `${root}/${name}`;
  }

  if (root === "WemadeMir3") {
    const folders = ["", "Wood", "Sand", "Snow", "Forest"];
    const folder = folders[stateIndex];
    return folder ? `${root}/${folder}/${name}` : `${root}/${name}`;
  }

  const suffixes = ["", "wood", "sand", "snow", "forest"];
  return `${root}/${name}${suffixes[stateIndex] ?? ""}`;
}

function rememberMap(key: string, parsedMap: ParsedMap) {
  const bytes = estimateParsedMapBytes(parsedMap);
  const previous = mapCache.get(key);
  if (previous) mapCacheBytes -= previous.bytes;
  mapCache.set(key, { value: parsedMap, bytes });
  mapCacheBytes += bytes;
  trimSizedCache(mapCache, SERVER_MAP_CACHE_MAX_BYTES, () => {
    mapCacheBytes = cacheByteTotal(mapCache);
  });
}

function rememberLibrary(key: string, library: ParsedLibrary | null) {
  const bytes = estimateParsedLibraryBytes(library);
  const previous = libraryCache.get(key);
  if (previous) libraryCacheBytes -= previous.bytes;
  libraryCache.set(key, { value: library, bytes });
  libraryCacheBytes += bytes;
  trimSizedCache(libraryCache, SERVER_LIBRARY_CACHE_MAX_BYTES, () => {
    libraryCacheBytes = cacheByteTotal(libraryCache);
  });
}

function rememberDecodedFrame(key: string, rgba: Buffer) {
  const previous = decodedFrameCache.get(key);
  if (previous) decodedFrameCacheBytes -= previous.bytes;
  const entry = { rgba, bytes: rgba.byteLength };
  decodedFrameCache.set(key, entry);
  decodedFrameCacheBytes += entry.bytes;
  while (decodedFrameCacheBytes > DECODED_FRAME_CACHE_MAX_BYTES && decodedFrameCache.size > 0) {
    const oldestKey = decodedFrameCache.keys().next().value as string | undefined;
    if (!oldestKey) break;
    const oldest = decodedFrameCache.get(oldestKey);
    decodedFrameCache.delete(oldestKey);
    decodedFrameCacheBytes -= oldest?.bytes ?? 0;
  }
}

function touchSizedCacheEntry<T>(cache: Map<string, SizedCacheEntry<T>>, key: string, entry: SizedCacheEntry<T>) {
  cache.delete(key);
  cache.set(key, entry);
}

function trimSizedCache<T>(
  cache: Map<string, SizedCacheEntry<T>>,
  maxBytes: number,
  afterTrim: () => void,
) {
  if (maxBytes <= 0) {
    cache.clear();
    afterTrim();
    return;
  }
  let total = cacheByteTotal(cache);
  while (total > maxBytes && cache.size > 1) {
    const oldestKey = cache.keys().next().value as string | undefined;
    if (!oldestKey) break;
    const oldest = cache.get(oldestKey);
    cache.delete(oldestKey);
    total -= oldest?.bytes ?? 0;
  }
  afterTrim();
}

function cacheByteTotal<T>(cache: Map<string, SizedCacheEntry<T>>) {
  let total = 0;
  for (const entry of cache.values()) total += entry.bytes;
  return total;
}

function estimateParsedMapBytes(parsedMap: ParsedMap) {
  return 512 + (parsedMap.cells?.length ?? 0) * 80 + Object.keys(parsedMap.fallbackOriginalMapRegion?.sprites ?? {}).length * 160;
}

function estimateParsedLibraryBytes(library: ParsedLibrary | null) {
  if (!library) return 64;
  const decodedHeaders = library.frames.reduce((count, frame) => count + (frame ? 1 : 0), 0);
  return 256 + library.count * 8 + decodedHeaders * 48;
}

function readFileSlice(filePath: string, offset: number, length: number) {
  const output = Buffer.allocUnsafe(Math.max(0, length));
  const fd = openSync(filePath, "r");
  try {
    const bytesRead = readSync(fd, output, 0, output.length, offset);
    return bytesRead === output.length ? output : output.subarray(0, bytesRead);
  } finally {
    closeSync(fd);
  }
}

function parseByteBudget(value: string | undefined, fallback: number) {
  if (!value) return fallback;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallback;
}

export function getCrystalMapLoaderResourceStats() {
  return {
    mapCacheEntries: mapCache.size,
    mapCacheBytes,
    libraryCacheEntries: libraryCache.size,
    libraryCacheBytes,
    decodedFrameCacheEntries: decodedFrameCache.size,
    decodedFrameCacheBytes,
    ...resourceStats,
  };
}

export function resetCrystalMapLoaderResourceStatsForTests() {
  resourceStats.libraryParseCount = 0;
  resourceStats.frameHeaderReadCount = 0;
  resourceStats.frameDecodeCount = 0;
  resourceStats.decodedFrameCacheHits = 0;
  resourceStats.decodedFrameCacheMisses = 0;
  decodedFrameCache.clear();
  decodedFrameCacheBytes = 0;
}

export function crystalOriginalAssetManifestHasPathForTests(publicPath: string) {
  return originalAssetManifestHasPublicPath(publicPath);
}

export function resetCrystalOriginalAssetManifestForTests() {
  originalAssetManifestPaths = undefined;
}

function normalizeLibraryName(libraryName: string) {
  return String(libraryName).replaceAll("\\", "/").split("/").filter(Boolean).join("/");
}

function normalizeMapFileName(mapFileName: string) {
  const normalized = String(mapFileName || "0").trim().replaceAll("\\", "/").split("/").pop() ?? "0";
  return normalized.replace(/\.map$/i, "") || "0";
}

function packagedMapFileStem(mapFileName: string) {
  return encodeURIComponent(normalizeMapFileName(mapFileName).toLowerCase());
}

function postProcessFrameRgba(libraryName: string, frameIndex: number, rgba: Buffer) {
  if (libraryName !== "WemadeMir2/Objects" || frameIndex < 2723 || frameIndex > 2732) return rgba;
  const next = Buffer.from(rgba);
  for (let index = 0; index < next.length; index += 4) {
    const brightness = Math.max(next[index], next[index + 1], next[index + 2]);
    const alpha = next[index + 3];
    if (brightness <= 20) {
      next[index + 3] = 0;
    } else if (brightness < 72) {
      next[index + 3] = Math.max(0, Math.min(alpha, Math.round(((brightness - 20) / 52) * alpha)));
    }
  }
  return next;
}

function decodeFrame(width: number, height: number, compressed: Buffer) {
  if (!width || !height) return Buffer.alloc(0);
  resourceStats.frameDecodeCount += 1;
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

function clampInt(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) return min;
  return Math.max(min, Math.min(max, Math.trunc(value)));
}
