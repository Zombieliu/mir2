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
const PACKAGED_MAP_ROOT = path.join(WEB_ROOT, "lib", "generated", "crystal-map-pack");
const STARTER_MAP_REGION_PATH = path.join(WEB_ROOT, "lib", "generated", "crystal_starter_map_region.json");
const OUTPUT_ROOT = path.join(PUBLIC_ROOT, "generated", "native-map-keyed");
const SAFE_TEMP_OUTPUT_PREFIXES = ["native-keyed-map-", "native-map-keyed-"];

export const DEFAULT_MAP_FILE_NAME = "0";
export const NATIVE_KEYED_MANIFEST_KIND = "mir2-native-map-keyed-manifest";
// Map 0 still references Crystal frames that have not been legally exported.
// This is the tracked clean-checkout baseline; local untracked exports must not
// make CI's budget artificially stricter or looser.
export const NATIVE_KEYED_MAX_MISSING_SOURCES = 2508;

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
  const load = new Function("exports", "module", "require", compiled.outputText);
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
      if (index >= 111 && index <= 119) return `ShandaMir2/SmTiles${index - 109}`;
      if (index >= 121 && index <= 150) return `ShandaMir2/Objects${index - 119}`;
      return "WemadeMir2/Tiles";
  }
}

export function decodeCrystalMiddleAnimationCount(animationFrame) {
  return animationFrame <= 0 || animationFrame >= 255 ? 0 : animationFrame & 0x0f;
}

export function decodeCrystalFrontAnimationCount(animationFrame) {
  return animationFrame > 0 ? animationFrame & 0x7f : 0;
}

export function crystalMiddleMapBlendMode(animationFrame) {
  const count = decodeCrystalMiddleAnimationCount(animationFrame);
  return count === 8 || count === 10 || (animationFrame & 0x80) !== 0 ? "additive" : "normal";
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

export function crystalMapFrameHasLegacyOffsetFallback(sourcePath) {
  return /\/original-map\/WemadeMir2\/Objects\/27(2[3-9]|3[0-2])\.png$/i.test(sourcePath);
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

export async function loadStarterMapOffsetIndex(regionPath = STARTER_MAP_REGION_PATH) {
  if (!existsSync(regionPath)) {
    return new Map();
  }
  const payload = JSON.parse(await fs.readFile(regionPath, "utf8"));
  const offsets = new Map();
  for (const spriteGroup of Object.values(payload?.sprites ?? {})) {
    if (!Array.isArray(spriteGroup?.frames)) continue;
    for (const frame of spriteGroup.frames) {
      if (typeof frame?.path !== "string") continue;
      if (!Number.isInteger(frame?.offsetX) || !Number.isInteger(frame?.offsetY)) continue;
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
    throw new Error(`Invalid native keyed map missing-source budget: ${maxMissingSources}`);
  }
  if (!Number.isSafeInteger(result?.missingSourceCount) || result.missingSourceCount < 0) {
    throw new Error("Native keyed map build returned an invalid missingSourceCount");
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
        const requiresAlphaKey = !additive && mapAtlasPathRequiresAlphaKey(sourcePath);
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

      const backFrame = cell.backImage === 0 ? -1 : (cell.backImage & 0x1fffffff) - 1;
      if (cell.backIndex >= 0 && backFrame >= 0) {
        add(mapLibraryKeyForIndex(cell.backIndex), backFrame, false, "back");
      }

      const middleFrame = cell.middleImage - 1;
      if (cell.middleIndex >= 0 && middleFrame >= 0) {
        add(
          mapLibraryKeyForIndex(cell.middleIndex),
          middleFrame,
          crystalMiddleMapBlendMode(cell.middleAnimationFrame) === "additive",
          "middle",
        );
      }

      const frontFrame = (cell.frontImage & 0x7fff) - 1;
      if (cell.frontIndex >= 0 && frontFrame >= 0) {
        add(
          mapLibraryKeyForIndex(cell.frontIndex),
          frontFrame,
          crystalFrontMapBlendMode(cell.frontAnimationFrame) === "additive",
          "front",
        );
      }
    }
  }

  return [...references.values()].sort((left, right) => left.key.localeCompare(right.key));
}

function pathIsInside(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
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
  if (!SAFE_TEMP_OUTPUT_PREFIXES.some((prefix) => baseName.startsWith(prefix))) {
    throw new Error(
      `Refusing temp output root without dedicated native-keyed prefix: ${resolvedOutputRoot}`,
    );
  }

  return resolvedOutputRoot;
}

async function removeStaleOutputs(root, allowedRoot = root) {
  const resolvedRoot = path.resolve(root);
  const resolvedAllowedRoot = path.resolve(allowedRoot);
  if (!pathIsInside(resolvedAllowedRoot, resolvedRoot)) {
    throw new Error(`Refusing to delete outside guarded native keyed root: ${resolvedRoot}`);
  }
  if (!existsSync(resolvedRoot)) {
    return 0;
  }
  let removed = 0;
  const entries = await fs.readdir(resolvedRoot, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = path.resolve(resolvedRoot, entry.name);
    if (!pathIsInside(resolvedAllowedRoot, entryPath)) {
      throw new Error(`Refusing to delete escaped path outside guarded root: ${entryPath}`);
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

async function keyedImageForSource(absolutePath) {
  const { data, info } = await sharp(absolutePath)
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });
  const pixels = new Uint8ClampedArray(data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength));
  alphaKeyMapObjectPixels(pixels, info.width, info.height);
  const encoded = await sharp(Buffer.from(pixels), {
    raw: {
      width: info.width,
      height: info.height,
      channels: 4,
    },
  })
    .png({ compressionLevel: 9, adaptiveFiltering: true })
    .toBuffer();
  return { encoded, width: info.width, height: info.height };
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

export async function buildNativeKeyedMapPack({
  mapFileName = DEFAULT_MAP_FILE_NAME,
  outputRoot = OUTPUT_ROOT,
  originalMapRoot = ORIGINAL_MAP_ROOT,
  packagedMapRoot = PACKAGED_MAP_ROOT,
  starterMapRegionPath = STARTER_MAP_REGION_PATH,
} = {}) {
  const resolvedOutputRoot = assertSafeNativeKeyedOutputRoot(outputRoot);
  const outputPageRoot = nativeKeyedPageRoot(resolvedOutputRoot);
  const mapPath = path.join(packagedMapRoot, `${mapFileName}.map.gz`);
  const compressed = await fs.readFile(mapPath);
  const parsedMap = parseType100Map(gunzipSync(compressed));
  if (!parsedMap) {
    throw new Error(`Unable to parse packaged map ${mapPath}`);
  }

  const removedArtifacts = await removeStaleOutputs(resolvedOutputRoot, resolvedOutputRoot);
  await fs.mkdir(outputPageRoot, { recursive: true });

  const references = collectStandaloneMapReferences(parsedMap);
  const offsetIndex = await loadStarterMapOffsetIndex(starterMapRegionPath);
  const entries = [];
  let keyedEntryCount = 0;
  let additiveEntryCount = 0;
  let missingSourceCount = 0;
  let imageBytes = 0;

  for (const reference of references) {
    const absoluteSourcePath = path.join(
      originalMapRoot,
      reference.libraryKey.split("/").join(path.sep),
      `${reference.frameIndex}.png`,
    );
    if (!existsSync(absoluteSourcePath)) {
      missingSourceCount += 1;
      continue;
    }

    if (reference.additive) {
      const raw = await rawImageForSource(absoluteSourcePath);
      if (!raw) {
        missingSourceCount += 1;
        continue;
      }
      const hash = createHash("sha256").update(raw.encoded).digest("hex");
      const pageFileName = `${hash}.png`;
      const pageAbsolutePath = path.join(outputPageRoot, pageFileName);
      await fs.writeFile(pageAbsolutePath, raw.encoded);
      const placement = resolveCrystalMapPlacement(reference, offsetIndex);
      additiveEntryCount += 1;
      imageBytes += raw.encoded.length;
      entries.push({
        key: reference.key,
        imageUrl: `/generated/native-map-keyed/pages/${pageFileName}`,
        width: raw.width,
        height: raw.height,
        ...(placement ?? {}),
      });
      continue;
    }

    const keyed = await keyedImageForSource(absoluteSourcePath);
    const hash = createHash("sha256").update(keyed.encoded).digest("hex");
    const pageFileName = `${hash}.png`;
    const pageAbsolutePath = path.join(outputPageRoot, pageFileName);
    await fs.writeFile(pageAbsolutePath, keyed.encoded);
    keyedEntryCount += 1;
    imageBytes += keyed.encoded.length;
    const placement = resolveCrystalMapPlacement(reference, offsetIndex);
    entries.push({
      key: reference.key,
      imageUrl: `/generated/native-map-keyed/pages/${pageFileName}`,
      width: keyed.width,
      height: keyed.height,
      ...(placement ?? {}),
    });
  }

  const manifest = {
    schemaVersion: 1,
    kind: NATIVE_KEYED_MANIFEST_KIND,
    mapFileName,
    entries,
    stats: {
      referenceCount: references.length,
      emittedEntryCount: entries.length,
      keyedEntryCount,
      additiveEntryCount,
      missingSourceCount,
      removedArtifacts,
      imageBytes,
    },
  };
  const manifestJson = `${JSON.stringify(manifest, null, 2)}\n`;
  const manifestHash = createHash("sha256").update(manifestJson).digest("hex");
  await fs.writeFile(path.join(resolvedOutputRoot, "manifest.json"), manifestJson, "utf8");
  await fs.writeFile(
    path.join(resolvedOutputRoot, `manifest.${manifestHash}.json`),
    manifestJson,
    "utf8",
  );

  return {
    ...manifest.stats,
    manifestHash,
    manifestPath: path.join(resolvedOutputRoot, "manifest.json"),
    releaseManifestPath: path.join(resolvedOutputRoot, `manifest.${manifestHash}.json`),
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const result = await buildNativeKeyedMapPack({
    mapFileName: String(args.map ?? DEFAULT_MAP_FILE_NAME),
  });
  const maxMissingSources =
    args.maxMissingSources === undefined
      ? NATIVE_KEYED_MAX_MISSING_SOURCES
      : Number(args.maxMissingSources);
  assertNativeKeyedMapMissingSourceBudget(result, maxMissingSources);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
