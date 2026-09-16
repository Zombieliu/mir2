import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

import sharp from "sharp";
import ts from "typescript";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const PUBLIC_ROOT = path.join(WEB_ROOT, "public");
const ORIGINAL_MAP_ROOT = path.join(PUBLIC_ROOT, "original-map");
const PACKAGED_MAP_ROOT = path.join(
  WEB_ROOT,
  "lib",
  "generated",
  "crystal-map-pack",
);
const STARTER_MAP_REGION_PATH = path.join(
  WEB_ROOT,
  "lib",
  "generated",
  "crystal_starter_map_region.json",
);
const OUTPUT_ROOT = path.join(PUBLIC_ROOT, "generated", "native-map-keyed");
const FULL_CRYSTAL_PACK_ROOT = path.join(
  PUBLIC_ROOT,
  "generated",
  "crystal-packs",
  "full",
);
const PRODUCTION_ASSET_CONFIG_PATH = path.resolve(
  WEB_ROOT,
  "..",
  "..",
  "config",
  "production-web-assets.json",
);
const SAFE_TEMP_OUTPUT_PREFIXES = ["native-keyed-map-", "native-map-keyed-"];

export const DEFAULT_MAP_FILE_NAME = "0";
export const DEFAULT_FULL_PACK_FALLBACK_MAP_FILE_NAMES = ["0141"];
export const NATIVE_KEYED_MANIFEST_KIND = "mir2-native-map-keyed-manifest";
// Map 0 still references Crystal frames that have not been legally exported.
// This is the tracked clean-checkout baseline; local untracked exports must not
// make CI's budget artificially stricter or looser.
export const NATIVE_KEYED_MAX_MISSING_SOURCES = 2969;

function loadTypeScriptModule(url, requireMap = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    throw new Error(`Unexpected require(${specifier}) while loading ${url}`);
  };
  const load = new Function(
    "exports",
    "module",
    "require",
    compiled.outputText,
  );
  load(module.exports, module, require);
  return module.exports;
}

export const { alphaKeyMapObjectPixels } = loadTypeScriptModule(
  new URL("../lib/scene-alpha-key.ts", import.meta.url),
);

function mapMir3LibraryKey(index, baseIndex, root) {
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

export function mapLibraryKeyForIndex(index) {
  const wemadeMir3 = mapMir3LibraryKey(index, 200, "WemadeMir3");
  if (wemadeMir3) return wemadeMir3;
  const shandaMir3 = mapMir3LibraryKey(index, 300, "ShandaMir3");
  if (shandaMir3) return shandaMir3;

  switch (index) {
    case 0:
      return "WemadeMir2/Tiles";
    case 1:
      return "WemadeMir2/SmTiles";
    case 2:
      return "WemadeMir2/Objects";
    case 90:
      return "WemadeMir2/Objects_32bit";
    case 100:
      return "ShandaMir2/Tiles";
    case 110:
      return "ShandaMir2/SmTiles";
    case 120:
      return "ShandaMir2/Objects";
    case 190:
      return "ShandaMir2/AniTiles1";
    default:
      if (index >= 3 && index <= 29) return `WemadeMir2/Objects${index - 1}`;
      if (index >= 101 && index <= 109) return `ShandaMir2/Tiles${index - 99}`;
      if (index >= 111 && index <= 119)
        return `ShandaMir2/SmTiles${index - 109}`;
      if (index >= 121 && index <= 150)
        return `ShandaMir2/Objects${index - 119}`;
      return "WemadeMir2/Tiles";
  }
}

export function decodeCrystalMiddleAnimationCount(animationFrame) {
  return animationFrame <= 0 || animationFrame >= 255
    ? 0
    : animationFrame & 0x0f;
}

export function decodeCrystalFrontAnimationCount(animationFrame) {
  return animationFrame > 0 ? animationFrame & 0x7f : 0;
}

export function crystalMiddleMapBlendMode(animationFrame) {
  const count = decodeCrystalMiddleAnimationCount(animationFrame);
  return count === 8 || count === 10 || (animationFrame & 0x80) !== 0
    ? "additive"
    : "normal";
}

export function crystalFrontMapBlendMode(animationFrame) {
  return (animationFrame & 0x80) !== 0 ? "additive" : "normal";
}

export function mapAtlasPathRequiresAlphaKey(value) {
  try {
    const normalized = new URL(value, "https://mir2.invalid/").pathname;
    return (
      normalized.startsWith("/original-map/") &&
      /\/(?:objects(?:_32bit|\d*)?|smobjects\d*|furnitures?c?|walls?c?|animations?c?|houses?c?|cliffs?c?|dungeons?c?|inners?c?|object[12]c)\//i.test(
        normalized,
      )
    );
  } catch {
    return false;
  }
}

export function originalMapFramePath(libraryKey, frameIndex) {
  return `/original-map/${libraryKey}/${frameIndex}.png`;
}

export function packagedMapFilePath(mapFileName = DEFAULT_MAP_FILE_NAME) {
  return path.join(PACKAGED_MAP_ROOT, `${mapFileName}.map.gz`);
}

function nativeKeyedPageRoot(outputRoot) {
  return path.join(outputRoot, "pages");
}

export function parseType100Map(bytes) {
  if (bytes.length < 8 || bytes[2] !== 0x43 || bytes[3] !== 0x23) {
    return null;
  }
  const width = bytes.readUInt16LE(4);
  const height = bytes.readUInt16LE(6);
  const cellBytes = 8 + width * height * 26;
  if (bytes.length < cellBytes) {
    return null;
  }

  const cells = [];
  let offset = 8;
  for (let index = 0; index < width * height; index += 1) {
    cells.push({
      backIndex: bytes.readInt16LE(offset),
      backImage: bytes.readInt32LE(offset + 2),
      middleIndex: bytes.readInt16LE(offset + 6),
      middleImage: bytes.readInt16LE(offset + 8),
      frontIndex: bytes.readInt16LE(offset + 10),
      frontImage: bytes.readInt16LE(offset + 12),
      frontAnimationFrame: bytes[offset + 16],
      frontAnimationTick: bytes[offset + 17],
      middleAnimationFrame: bytes[offset + 18],
      middleAnimationTick: bytes[offset + 19],
    });
    offset += 26;
  }

  return { width, height, cells };
}

/** Parse Wemade's `Map 2010 Ver 1.0` / Crystal LoadMapType1 format. */
export function parseType1Map(bytes) {
  const headerBytes = 54;
  const cellBytes = 15;
  if (
    bytes.length < headerBytes ||
    bytes[0] !== 0x10 ||
    bytes[2] !== 0x61 ||
    bytes[7] !== 0x31 ||
    bytes[14] !== 0x31
  ) {
    return null;
  }

  const xor = bytes.readInt16LE(23);
  const width = bytes.readInt16LE(21) ^ xor;
  const height = bytes.readInt16LE(25) ^ xor;
  if (
    width <= 0 ||
    height <= 0 ||
    bytes.length < headerBytes + width * height * cellBytes
  ) {
    return null;
  }

  const cells = [];
  let offset = headerBytes;
  for (let index = 0; index < width * height; index += 1) {
    let frontIndex = bytes[offset + 12] + 2;
    if (frontIndex === 102) frontIndex = 90;
    if (frontIndex >= 255) frontIndex = -1;
    cells.push({
      backIndex: 0,
      backImage: (bytes.readInt32LE(offset) ^ 0xaa38aa38) | 0,
      middleIndex: 1,
      middleImage: signed16(bytes.readInt16LE(offset + 4) ^ xor),
      frontIndex,
      frontImage: signed16(bytes.readInt16LE(offset + 6) ^ xor),
      frontAnimationFrame: bytes[offset + 10],
      frontAnimationTick: bytes[offset + 11],
      middleAnimationFrame: 0,
      middleAnimationTick: 0,
    });
    offset += cellBytes;
  }

  return { width, height, cells };
}

function signed16(value) {
  return value & 0x8000 ? value - 0x10000 : value;
}

export function parsePackagedMap(bytes) {
  return parseType100Map(bytes) ?? parseType1Map(bytes);
}

export function crystalMapFrameHasLegacyOffsetFallback(sourcePath) {
  return /\/original-map\/WemadeMir2\/Objects\/27(2[3-9]|3[0-2])\.png$/i.test(
    sourcePath,
  );
}

export function crystalMapFrameUsesSourceOffset(reference, offsetMeta) {
  if (crystalMapFrameHasLegacyOffsetFallback(reference.sourcePath)) {
    return true;
  }
  if (
    /^WemadeMir2\/Objects27$/i.test(reference.libraryKey) &&
    offsetMeta &&
    ((typeof offsetMeta.offsetX === "number" && offsetMeta.offsetX !== 0) ||
      (typeof offsetMeta.offsetY === "number" && offsetMeta.offsetY !== 0))
  ) {
    return true;
  }
  return false;
}

export function fallbackCrystalMapOffset(sourcePath) {
  if (crystalMapFrameHasLegacyOffsetFallback(sourcePath)) {
    return { offsetX: -50, offsetY: -100 };
  }
  return null;
}

export function normalizeCrystalMapOffset(offsetMeta, sourcePath) {
  if (
    offsetMeta &&
    Number.isInteger(offsetMeta.offsetX) &&
    Number.isInteger(offsetMeta.offsetY)
  ) {
    return { offsetX: offsetMeta.offsetX, offsetY: offsetMeta.offsetY };
  }
  return fallbackCrystalMapOffset(sourcePath);
}

export async function loadStarterMapOffsetIndex(
  regionPath = STARTER_MAP_REGION_PATH,
) {
  if (!existsSync(regionPath)) {
    return new Map();
  }
  const payload = JSON.parse(await fs.readFile(regionPath, "utf8"));
  const offsets = new Map();
  for (const spriteGroup of Object.values(payload?.sprites ?? {})) {
    if (!Array.isArray(spriteGroup?.frames)) continue;
    for (const frame of spriteGroup.frames) {
      if (typeof frame?.path !== "string") continue;
      if (
        !Number.isInteger(frame?.offsetX) ||
        !Number.isInteger(frame?.offsetY)
      )
        continue;
      offsets.set(frame.path, {
        offsetX: frame.offsetX,
        offsetY: frame.offsetY,
      });
    }
  }
  return offsets;
}

export function resolveCrystalMapPlacement(reference, offsetIndex) {
  const offsetMeta = offsetIndex?.get(reference.sourcePath) ?? null;
  if (!crystalMapFrameUsesSourceOffset(reference, offsetMeta)) {
    return null;
  }
  const offset = normalizeCrystalMapOffset(offsetMeta, reference.sourcePath);
  if (!offset) {
    return null;
  }
  return {
    placementMode: "source-offset",
    offsetX: offset.offsetX,
    offsetY: offset.offsetY,
  };
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (!value.startsWith("--")) continue;
    const [rawKey, inlineValue] = value.slice(2).split("=", 2);
    const key = rawKey.trim();
    if (!key) continue;
    parsed[key] =
      inlineValue !== undefined
        ? inlineValue
        : argv[index + 1] && !argv[index + 1].startsWith("--")
          ? argv[++index]
          : "true";
  }
  return parsed;
}

function positiveInteger(value) {
  return Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0;
}

export function assertNativeKeyedMapMissingSourceBudget(
  result,
  maxMissingSources = NATIVE_KEYED_MAX_MISSING_SOURCES,
) {
  if (!Number.isSafeInteger(maxMissingSources) || maxMissingSources < 0) {
    throw new Error(
      `Invalid native keyed map missing-source budget: ${maxMissingSources}`,
    );
  }
  if (
    !Number.isSafeInteger(result?.missingSourceCount) ||
    result.missingSourceCount < 0
  ) {
    throw new Error(
      "Native keyed map build returned an invalid missingSourceCount",
    );
  }
  if (result.missingSourceCount > maxMissingSources) {
    throw new Error(
      `Native keyed map source coverage regressed: ${result.missingSourceCount} missing, ` +
        `budget ${maxMissingSources}`,
    );
  }
}

function parsedCellAt(map, x, y) {
  return map.cells[x * map.height + y];
}

export function collectStandaloneMapReferences(parsedMap) {
  const references = new Map();
  for (let x = 0; x < parsedMap.width; x += 1) {
    for (let y = 0; y < parsedMap.height; y += 1) {
      const cell = parsedCellAt(parsedMap, x, y);
      if (!cell) continue;

      const add = (libraryKey, frameIndex, additive, layer) => {
        if (frameIndex < 0) {
          return;
        }
        const sourcePath = originalMapFramePath(libraryKey, frameIndex);
        const requiresAlphaKey =
          !additive && mapAtlasPathRequiresAlphaKey(sourcePath);
        if (!additive && !requiresAlphaKey) {
          return;
        }
        const key = `${libraryKey}#${frameIndex}`;
        const existing = references.get(key);
        if (!existing || (additive && !existing.additive)) {
          references.set(key, {
            key,
            libraryKey,
            frameIndex,
            sourcePath,
            additive,
            layer,
          });
        }
      };
      const addFamily = (
        libraryKey,
        baseFrameIndex,
        frameCount,
        additive,
        layer,
      ) => {
        const count = Math.max(1, positiveInteger(frameCount));
        for (let phase = 0; phase < count; phase += 1) {
          add(libraryKey, baseFrameIndex + phase, additive, layer);
        }
      };

      const backFrame =
        cell.backImage === 0 ? -1 : (cell.backImage & 0x1fffffff) - 1;
      if (cell.backIndex >= 0 && backFrame >= 0) {
        add(mapLibraryKeyForIndex(cell.backIndex), backFrame, false, "back");
      }

      const middleFrame = cell.middleImage - 1;
      if (cell.middleIndex >= 0 && middleFrame >= 0) {
        addFamily(
          mapLibraryKeyForIndex(cell.middleIndex),
          middleFrame,
          decodeCrystalMiddleAnimationCount(cell.middleAnimationFrame),
          crystalMiddleMapBlendMode(cell.middleAnimationFrame) === "additive",
          "middle",
        );
      }

      const frontFrame = (cell.frontImage & 0x7fff) - 1;
      if (cell.frontIndex >= 0 && frontFrame >= 0) {
        addFamily(
          mapLibraryKeyForIndex(cell.frontIndex),
          frontFrame,
          decodeCrystalFrontAnimationCount(cell.frontAnimationFrame),
          crystalFrontMapBlendMode(cell.frontAnimationFrame) === "additive",
          "front",
        );
      }
    }
  }

  return [...references.values()].sort((left, right) =>
    left.key.localeCompare(right.key),
  );
}

function normalizeMapFileNames(values, fallback = [DEFAULT_MAP_FILE_NAME]) {
  const source = Array.isArray(values)
    ? values
    : String(values ?? "").split(",");
  const normalized = [
    ...new Set(source.map((value) => String(value).trim()).filter(Boolean)),
  ];
  return normalized.length > 0 ? normalized : [...fallback];
}

function collectMapSetReferences(parsedMaps) {
  const references = new Map();
  for (const { mapFileName, parsedMap } of parsedMaps) {
    for (const reference of collectStandaloneMapReferences(parsedMap)) {
      const existing = references.get(reference.key);
      if (!existing) {
        references.set(reference.key, {
          ...reference,
          mapFileNames: new Set([mapFileName]),
        });
        continue;
      }
      existing.mapFileNames.add(mapFileName);
      if (reference.additive && !existing.additive) {
        Object.assign(existing, reference, {
          mapFileNames: existing.mapFileNames,
        });
      }
    }
  }
  return [...references.values()].sort((left, right) =>
    left.key.localeCompare(right.key),
  );
}

function pathIsInside(root, candidate) {
  const relative = path.relative(root, candidate);
  return (
    relative === "" ||
    (!relative.startsWith("..") && !path.isAbsolute(relative))
  );
}

export function assertSafeNativeKeyedOutputRoot(outputRoot) {
  const resolvedOutputRoot = path.resolve(outputRoot);
  const resolvedDefaultRoot = path.resolve(OUTPUT_ROOT);
  if (resolvedOutputRoot === resolvedDefaultRoot) {
    return resolvedOutputRoot;
  }

  const resolvedTempRoot = path.resolve(os.tmpdir());
  if (!pathIsInside(resolvedTempRoot, resolvedOutputRoot)) {
    throw new Error(
      `Refusing to mutate native keyed output outside ${resolvedDefaultRoot} or a dedicated temp root: ${resolvedOutputRoot}`,
    );
  }

  const baseName = path.basename(resolvedOutputRoot).toLowerCase();
  if (
    !SAFE_TEMP_OUTPUT_PREFIXES.some((prefix) => baseName.startsWith(prefix))
  ) {
    throw new Error(
      `Refusing temp output root without dedicated native-keyed prefix: ${resolvedOutputRoot}`,
    );
  }

  return resolvedOutputRoot;
}

async function assertNoReparseTree(root) {
  let current = path.resolve(root);
  const filesystemRoot = path.parse(current).root;
  while (true) {
    if (existsSync(current)) {
      const stat = await fs.lstat(current);
      if (stat.isSymbolicLink()) {
        throw new Error(
          `Refusing native keyed output through a symlink/junction: ${current}`,
        );
      }
    }
    if (current === filesystemRoot) break;
    const parent = path.dirname(current);
    if (parent === current) break;
    current = parent;
  }

  const walk = async (candidate) => {
    if (!existsSync(candidate)) return;
    const stat = await fs.lstat(candidate);
    if (stat.isSymbolicLink()) {
      throw new Error(
        `Refusing symlink/junction inside native keyed output: ${candidate}`,
      );
    }
    if (!stat.isDirectory()) return;
    for (const entry of await fs.readdir(candidate)) {
      await walk(path.join(candidate, entry));
    }
  };
  await walk(path.resolve(root));
}

async function removeStaleOutputs(root, allowedRoot = root) {
  const resolvedRoot = path.resolve(root);
  const resolvedAllowedRoot = path.resolve(allowedRoot);
  if (!pathIsInside(resolvedAllowedRoot, resolvedRoot)) {
    throw new Error(
      `Refusing to delete outside guarded native keyed root: ${resolvedRoot}`,
    );
  }
  if (!existsSync(resolvedRoot)) {
    return 0;
  }
  let removed = 0;
  const entries = await fs.readdir(resolvedRoot, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = path.resolve(resolvedRoot, entry.name);
    if (!pathIsInside(resolvedAllowedRoot, entryPath)) {
      throw new Error(
        `Refusing to delete escaped path outside guarded root: ${entryPath}`,
      );
    }
    if (entry.isDirectory()) {
      removed += await removeStaleOutputs(entryPath, resolvedAllowedRoot);
      const leftovers = await fs.readdir(entryPath).catch(() => []);
      if (!leftovers.length) {
        await fs.rm(entryPath, { recursive: true, force: true });
      }
      continue;
    }
    if (
      entry.name === "manifest.json" ||
      /^manifest\.[0-9a-f]{64}\.json$/i.test(entry.name) ||
      /^[0-9a-f]{64}\.png$/i.test(entry.name)
    ) {
      await fs.rm(entryPath, { force: true });
      removed += 1;
    }
  }
  return removed;
}

async function rawImageForSource(absolutePath) {
  const encoded = await fs.readFile(absolutePath);
  const metadata = await sharp(encoded).metadata();
  const width = positiveInteger(metadata.width);
  const height = positiveInteger(metadata.height);
  if (!width || !height) {
    return null;
  }
  return { encoded, width, height };
}

function sha256Hex(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function fullPackRelativePath(assetPath) {
  const pathname = new URL(assetPath, "https://mir2.invalid/").pathname;
  const prefix = "/generated/crystal-packs/full/";
  if (!pathname.startsWith(prefix)) {
    throw new Error(
      `Full Crystal asset escaped its immutable root: ${assetPath}`,
    );
  }
  return pathname.slice(prefix.length);
}

function expectedPageSha256(frameImage) {
  const key = String(frameImage?.pageKey ?? "");
  if (/^sha256:[0-9a-f]{64}$/i.test(key))
    return key.slice("sha256:".length).toLowerCase();
  const fileName = path.basename(
    new URL(frameImage?.imageUrl, "https://mir2.invalid/").pathname,
  );
  const match = /^([0-9a-f]{64})\.png$/i.exec(fileName);
  return match?.[1]?.toLowerCase() ?? null;
}

async function fetchImmutableAsset(url, attempts = 3) {
  let lastError = null;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const response = await fetch(url, { cache: "no-store" });
      if (!response.ok)
        throw new Error(`${response.status} ${response.statusText}`);
      return Buffer.from(await response.arrayBuffer());
    } catch (error) {
      lastError = error;
      if (attempt < attempts) {
        await new Promise((resolve) => setTimeout(resolve, attempt * 250));
      }
    }
  }
  throw new Error(
    `Unable to fetch immutable Full Crystal asset ${url}: ${lastError}`,
  );
}

async function loadProductionFullPackConfig(
  configPath = PRODUCTION_ASSET_CONFIG_PATH,
) {
  const config = JSON.parse(await fs.readFile(configPath, "utf8"));
  const assetBaseUrl = String(config?.assetBaseUrl ?? "").replace(/\/+$/, "");
  const indexPath = String(
    config?.fullCrystalPack?.path ?? "/generated/crystal-packs/full/index.json",
  );
  const contentHash = String(
    config?.fullCrystalPack?.contentHash ?? "",
  ).toLowerCase();
  if (
    !assetBaseUrl ||
    !indexPath.startsWith("/generated/crystal-packs/full/")
  ) {
    throw new Error(
      `Invalid Full Crystal production asset config: ${configPath}`,
    );
  }
  if (!/^[0-9a-f]{64}$/.test(contentHash)) {
    throw new Error(`Invalid Full Crystal content hash in ${configPath}`);
  }
  return { assetBaseUrl, indexPath, contentHash };
}

class FullCrystalMapFrameSource {
  constructor({ fullPackRoot, assetBaseUrl, indexPath, expectedContentHash }) {
    this.fullPackRoot = fullPackRoot ? path.resolve(fullPackRoot) : null;
    this.assetBaseUrl = String(assetBaseUrl ?? "").replace(/\/+$/, "");
    this.indexPath = indexPath;
    this.expectedContentHash = String(expectedContentHash ?? "").toLowerCase();
    this.index = null;
    this.libraryByKey = new Map();
    this.libraryManifestByKey = new Map();
    this.pageBytesByUrl = new Map();
  }

  async readAsset(assetPath, expectedHash = null) {
    const relativePath = fullPackRelativePath(assetPath);
    let bytes = null;
    if (this.fullPackRoot) {
      const localPath = path.join(
        this.fullPackRoot,
        ...relativePath.split("/"),
      );
      if (existsSync(localPath)) bytes = await fs.readFile(localPath);
    }
    if (!bytes) {
      if (!this.assetBaseUrl) {
        throw new Error(
          `Full Crystal asset is unavailable locally and no immutable base URL is set: ${assetPath}`,
        );
      }
      bytes = await fetchImmutableAsset(`${this.assetBaseUrl}${assetPath}`);
    }
    if (expectedHash && sha256Hex(bytes) !== expectedHash.toLowerCase()) {
      throw new Error(`Full Crystal SHA-256 mismatch for ${assetPath}`);
    }
    return bytes;
  }

  async initialize() {
    if (this.index) return;
    const bytes = await this.readAsset(this.indexPath);
    const index = JSON.parse(bytes.toString("utf8"));
    if (
      String(index?.contentHash ?? "").toLowerCase() !==
      this.expectedContentHash
    ) {
      throw new Error(
        `Full Crystal content hash mismatch: expected ${this.expectedContentHash}, got ${index?.contentHash}`,
      );
    }
    for (const library of index?.libraries ?? []) {
      if (typeof library?.libraryKey === "string")
        this.libraryByKey.set(library.libraryKey, library);
    }
    this.index = index;
  }

  async libraryManifest(libraryKey) {
    await this.initialize();
    const fullLibraryKey = `Map/${libraryKey}`;
    if (this.libraryManifestByKey.has(fullLibraryKey)) {
      return this.libraryManifestByKey.get(fullLibraryKey);
    }
    const library = this.libraryByKey.get(fullLibraryKey);
    if (!library) return null;
    const manifestPath = String(library.manifestUrl ?? library.shardUrl ?? "");
    const manifestHash = String(library.manifestSha256 ?? "").toLowerCase();
    if (!manifestPath || !/^[0-9a-f]{64}$/.test(manifestHash)) {
      throw new Error(
        `Full Crystal library descriptor is invalid: ${fullLibraryKey}`,
      );
    }
    const bytes = await this.readAsset(manifestPath, manifestHash);
    const manifest = JSON.parse(bytes.toString("utf8"));
    if (manifest?.libraryKey !== fullLibraryKey) {
      throw new Error(
        `Full Crystal library identity mismatch for ${fullLibraryKey}`,
      );
    }
    this.libraryManifestByKey.set(fullLibraryKey, manifest);
    return manifest;
  }

  async frame(reference) {
    const manifest = await this.libraryManifest(reference.libraryKey);
    if (!manifest) return null;
    const direct = manifest.frames?.[reference.frameIndex];
    const frame =
      direct?.index === reference.frameIndex
        ? direct
        : manifest.frames?.find(
            (candidate) => candidate?.index === reference.frameIndex,
          );
    if (!frame) return null;
    if (frame.noDraw || frame.status === "no-draw")
      return { noDraw: true, frame };
    const image = frame.image ?? frame;
    if (
      frame.status !== "packed" ||
      typeof image?.imageUrl !== "string" ||
      !Number.isInteger(image.x) ||
      !Number.isInteger(image.y) ||
      !positiveInteger(image.width) ||
      !positiveInteger(image.height)
    ) {
      return null;
    }
    return { noDraw: false, frame, image };
  }

  async render(reference, resolvedFrame = null) {
    const resolved = resolvedFrame ?? (await this.frame(reference));
    if (!resolved || resolved.noDraw) return resolved;
    const { frame, image } = resolved;
    let pageBytes = this.pageBytesByUrl.get(image.imageUrl);
    if (!pageBytes) {
      pageBytes = await this.readAsset(
        image.imageUrl,
        expectedPageSha256(image),
      );
      this.pageBytesByUrl.set(image.imageUrl, pageBytes);
    }
    const encoded = await sharp(pageBytes)
      .extract({
        left: image.x,
        top: image.y,
        width: image.width,
        height: image.height,
      })
      .png({ compressionLevel: 9, adaptiveFiltering: true })
      .toBuffer();
    const placement =
      Number.isInteger(frame.x) && Number.isInteger(frame.y)
        ? {
            placementMode: "source-offset",
            offsetX: frame.x,
            offsetY: frame.y,
          }
        : null;
    return {
      noDraw: false,
      encoded,
      width: image.width,
      height: image.height,
      placement,
    };
  }
}

async function createFullCrystalMapFrameSource({
  fullPackRoot = process.env.MIR2_FULL_PACK_ROOT ?? FULL_CRYSTAL_PACK_ROOT,
  assetBaseUrl = process.env.MIR2_ASSET_BASE_URL ?? "",
  productionAssetConfigPath = PRODUCTION_ASSET_CONFIG_PATH,
} = {}) {
  const production = await loadProductionFullPackConfig(
    productionAssetConfigPath,
  );
  return new FullCrystalMapFrameSource({
    fullPackRoot,
    assetBaseUrl: assetBaseUrl || production.assetBaseUrl,
    indexPath: production.indexPath,
    expectedContentHash: production.contentHash,
  });
}

export async function buildNativeKeyedMapPack({
  mapFileName = DEFAULT_MAP_FILE_NAME,
  mapFileNames = null,
  outputRoot = OUTPUT_ROOT,
  originalMapRoot = ORIGINAL_MAP_ROOT,
  packagedMapRoot = PACKAGED_MAP_ROOT,
  starterMapRegionPath = STARTER_MAP_REGION_PATH,
  fullPackFallbackMapFileNames = DEFAULT_FULL_PACK_FALLBACK_MAP_FILE_NAMES,
  fullPackRoot = process.env.MIR2_FULL_PACK_ROOT ?? FULL_CRYSTAL_PACK_ROOT,
  assetBaseUrl = process.env.MIR2_ASSET_BASE_URL ?? "",
  productionAssetConfigPath = PRODUCTION_ASSET_CONFIG_PATH,
  maxMissingSources = NATIVE_KEYED_MAX_MISSING_SOURCES,
} = {}) {
  const resolvedOutputRoot = assertSafeNativeKeyedOutputRoot(outputRoot);
  await assertNoReparseTree(resolvedOutputRoot);
  const outputPageRoot = nativeKeyedPageRoot(resolvedOutputRoot);
  const requestedMapFileNames = normalizeMapFileNames(
    mapFileNames ?? [mapFileName],
  );
  const parsedMaps = [];
  for (const requestedMapFileName of requestedMapFileNames) {
    const mapPath = path.join(
      packagedMapRoot,
      `${requestedMapFileName}.map.gz`,
    );
    const compressed = await fs.readFile(mapPath);
    const parsedMap = parsePackagedMap(gunzipSync(compressed));
    if (!parsedMap) {
      throw new Error(`Unable to parse packaged map ${mapPath}`);
    }
    parsedMaps.push({ mapFileName: requestedMapFileName, parsedMap });
  }

  const references = collectMapSetReferences(parsedMaps);
  const fallbackMaps = new Set(
    normalizeMapFileNames(fullPackFallbackMapFileNames, []),
  );
  const sourcePlans = new Map();
  let fullPackSource = null;
  let preflightMissingSourceCount = 0;
  for (const reference of references) {
    const absoluteSourcePath = path.join(
      originalMapRoot,
      reference.libraryKey.split("/").join(path.sep),
      `${reference.frameIndex}.png`,
    );
    if (existsSync(absoluteSourcePath)) {
      if (
        reference.additive &&
        !(await rawImageForSource(absoluteSourcePath))
      ) {
        preflightMissingSourceCount += 1;
        sourcePlans.set(reference.key, { kind: "missing" });
      } else {
        sourcePlans.set(reference.key, { kind: "local", absoluteSourcePath });
      }
      continue;
    }

    const fallbackAllowed = [...reference.mapFileNames].some((name) =>
      fallbackMaps.has(name),
    );
    if (fallbackAllowed) {
      fullPackSource ??= await createFullCrystalMapFrameSource({
        fullPackRoot,
        assetBaseUrl,
        productionAssetConfigPath,
      });
      const frame = await fullPackSource.frame(reference);
      if (frame) {
        sourcePlans.set(
          reference.key,
          frame.noDraw ? { kind: "no-draw" } : { kind: "full", frame },
        );
        continue;
      }
    }

    preflightMissingSourceCount += 1;
    sourcePlans.set(reference.key, { kind: "missing" });
  }
  assertNativeKeyedMapMissingSourceBudget(
    { missingSourceCount: preflightMissingSourceCount },
    maxMissingSources,
  );

  const removedArtifacts = await removeStaleOutputs(
    resolvedOutputRoot,
    resolvedOutputRoot,
  );
  await fs.mkdir(outputPageRoot, { recursive: true });

  const offsetIndex = await loadStarterMapOffsetIndex(starterMapRegionPath);
  const entries = [];
  // Kept for schema/backward compatibility: this now counts normal standalone
  // entries, whose local Crystal PNG payload is authoritative RGBA passthrough.
  let keyedEntryCount = 0;
  let additiveEntryCount = 0;
  let fullPackEntryCount = 0;
  let noDrawReferenceCount = 0;
  let missingSourceCount = 0;
  let imageBytes = 0;

  for (const reference of references) {
    const sourcePlan = sourcePlans.get(reference.key);
    if (!sourcePlan || sourcePlan.kind === "missing") {
      missingSourceCount += 1;
      continue;
    }
    if (sourcePlan.kind === "no-draw") {
      noDrawReferenceCount += 1;
      continue;
    }

    let image;
    let placement = null;
    if (sourcePlan.kind === "full") {
      image = await fullPackSource.render(reference, sourcePlan.frame);
      if (!image || image.noDraw) {
        noDrawReferenceCount += 1;
        continue;
      }
      placement = image.placement;
      fullPackEntryCount += 1;
    } else {
      // `original-map` PNGs are direct Crystal `.Lib` exports: their RGBA,
      // including binary alpha on ordinary buildings, is already authoritative.
      // Re-running the legacy black-key/feather pass here turns edge-connected
      // dark roof and wall pixels partially transparent and lets the ground show
      // through. Stage both normal and additive local frames byte-for-byte; the
      // runtime still chooses their distinct blend modes from map metadata.
      image = await rawImageForSource(sourcePlan.absoluteSourcePath);
      placement = resolveCrystalMapPlacement(reference, offsetIndex);
    }

    if (!image) {
      missingSourceCount += 1;
      continue;
    }
    const hash = sha256Hex(image.encoded);
    const pageFileName = `${hash}.png`;
    const pageAbsolutePath = path.join(outputPageRoot, pageFileName);
    await fs.writeFile(pageAbsolutePath, image.encoded);
    if (reference.additive) additiveEntryCount += 1;
    else keyedEntryCount += 1;
    imageBytes += image.encoded.length;
    entries.push({
      key: reference.key,
      imageUrl: `/generated/native-map-keyed/pages/${pageFileName}`,
      width: image.width,
      height: image.height,
      ...(placement ?? {}),
    });
  }

  const manifest = {
    schemaVersion: 1,
    kind: NATIVE_KEYED_MANIFEST_KIND,
    mapFileName: requestedMapFileNames[0],
    mapFileNames: requestedMapFileNames,
    entries,
    stats: {
      mapCount: requestedMapFileNames.length,
      referenceCount: references.length,
      emittedEntryCount: entries.length,
      keyedEntryCount,
      additiveEntryCount,
      fullPackEntryCount,
      noDrawReferenceCount,
      missingSourceCount,
      removedArtifacts,
      imageBytes,
    },
  };
  const manifestJson = `${JSON.stringify(manifest, null, 2)}\n`;
  const manifestHash = createHash("sha256").update(manifestJson).digest("hex");
  await fs.writeFile(
    path.join(resolvedOutputRoot, "manifest.json"),
    manifestJson,
    "utf8",
  );
  await fs.writeFile(
    path.join(resolvedOutputRoot, `manifest.${manifestHash}.json`),
    manifestJson,
    "utf8",
  );

  return {
    ...manifest.stats,
    manifestHash,
    manifestPath: path.join(resolvedOutputRoot, "manifest.json"),
    releaseManifestPath: path.join(
      resolvedOutputRoot,
      `manifest.${manifestHash}.json`,
    ),
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const maxMissingSources =
    args.maxMissingSources === undefined
      ? NATIVE_KEYED_MAX_MISSING_SOURCES
      : Number(args.maxMissingSources);
  const result = await buildNativeKeyedMapPack({
    mapFileNames: normalizeMapFileNames(
      args.maps ?? args.map ?? DEFAULT_MAP_FILE_NAME,
    ),
    outputRoot:
      args.outputRoot === undefined ? OUTPUT_ROOT : String(args.outputRoot),
    fullPackFallbackMapFileNames: normalizeMapFileNames(
      args.fullPackFallbackMaps ?? DEFAULT_FULL_PACK_FALLBACK_MAP_FILE_NAMES,
      [],
    ),
    fullPackRoot:
      args.fullPackRoot === undefined ? undefined : String(args.fullPackRoot),
    assetBaseUrl:
      args.assetBaseUrl === undefined ? undefined : String(args.assetBaseUrl),
    maxMissingSources,
  });
  assertNativeKeyedMapMissingSourceBudget(result, maxMissingSources);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
