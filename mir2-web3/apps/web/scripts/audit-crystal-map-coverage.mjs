import fs from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const DEFAULT_CRYSTAL_CLIENT_ROOT = "E:\\mir2\\Crystal\\Build\\Client\\Debug";
const CRYSTAL_CLIENT_ROOT = process.env.CRYSTAL_CLIENT_ROOT ?? DEFAULT_CRYSTAL_CLIENT_ROOT;
const MAP_DIR = path.join(CRYSTAL_CLIENT_ROOT, "Map");
const DATA_MAP_DIR = path.join(CRYSTAL_CLIENT_ROOT, "Data", "Map");
const PUBLIC_ORIGINAL_MAP_DIR = path.join(WORKSPACE_ROOT, "public", "original-map");
const RESPAWN_MANIFEST_PATH = path.join(
  REPO_ROOT,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_respawn_manifest.json",
);
const MMAP_META_PATH = path.join(WORKSPACE_ROOT, "public", "original-ui", "MMap", "meta.json");
const OUTPUT_PATH = path.resolve(
  REPO_ROOT,
  process.env.MIR2_MAP_COVERAGE_OUT ?? "docs/generated/map/latest-crystal-map-coverage.json",
);
const SAMPLE_WIDTH = positiveInt(process.env.MIR2_MAP_COVERAGE_SAMPLE_WIDTH, 24);
const SAMPLE_HEIGHT = positiveInt(process.env.MIR2_MAP_COVERAGE_SAMPLE_HEIGHT, 18);
const CELL_WIDTH = 48;
const CELL_HEIGHT = 32;
const SUPPORTED_TYPES = new Set([0, 1, 2, 3, 4, 5, 6, 7, 100]);

const respawnManifest = readJson(RESPAWN_MANIFEST_PATH);
const mmapMeta = readJson(MMAP_META_PATH);
const maps = Array.isArray(respawnManifest.maps) ? respawnManifest.maps : [];
const exportedMiniMaps = new Map(
  (Array.isArray(mmapMeta.frames) ? mmapMeta.frames : []).map((frame) => [Number(frame.index), frame]),
);
const mapFilesByName = indexFilesByNormalizedStem(MAP_DIR, ".map", false);
const libraryMetaCache = new Map();
const failures = [];
const mapResults = [];
const aggregateMissingLibraries = new Map();
const aggregateOutOfRangeFrames = new Map();
const aggregateEmptyFrames = new Map();

for (const map of maps) {
  const result = auditMap(map);
  mapResults.push(result);
  for (const key of result.sampleCoverage.missingLibraries) {
    aggregateMissingLibraries.set(key, (aggregateMissingLibraries.get(key) ?? 0) + 1);
  }
  for (const frame of result.sampleCoverage.outOfRangeFrames) {
    const key = `${frame.libraryKey}:${frame.frameIndex}`;
    aggregateOutOfRangeFrames.set(key, (aggregateOutOfRangeFrames.get(key) ?? 0) + 1);
  }
  for (const frame of result.sampleCoverage.emptyFrames) {
    const key = `${frame.libraryKey}:${frame.frameIndex}`;
    aggregateEmptyFrames.set(key, (aggregateEmptyFrames.get(key) ?? 0) + 1);
  }
}

const neededMiniMaps = [
  ...new Set(maps.map((map) => Number(map.mini_map)).filter((index) => Number.isFinite(index) && index > 0)),
].sort((left, right) => left - right);
const missingMiniMapIndices = neededMiniMaps.filter((index) => !exportedMiniMaps.has(index));
const mapsMissingMiniMapAssets = maps
  .filter((map) => Number(map.mini_map) > 0 && !exportedMiniMaps.has(Number(map.mini_map)))
  .map((map) => mapSummary(map));
const mapsWithoutMiniMap = maps
  .filter((map) => !(Number(map.mini_map) > 0))
  .map((map) => mapSummary(map));

const sourceMapMissing = mapResults.filter((map) => !map.source.mapFileExists);
const unsupportedTypes = mapResults.filter((map) => map.source.mapFileExists && !map.source.supportedType);
const parseErrors = mapResults.filter((map) => map.source.parseError);
const sampleMissingSourceAssets = mapResults.filter((map) => map.sampleCoverage.missingLibraries.length > 0);
const sampleCrystalIgnoredFrames = mapResults.filter(
  (map) => map.sampleCoverage.emptyFrames.length > 0 || map.sampleCoverage.outOfRangeFrames.length > 0,
);
const sampleNoSpriteMaps = mapResults.filter(
  (map) => map.source.mapFileExists && map.source.supportedType && map.sampleCoverage.requiredFrameCount === 0,
);
const visualFallbackRiskMaps = mapResults.filter(
  (map) =>
    !map.source.mapFileExists ||
    !map.source.supportedType ||
    Boolean(map.source.parseError) ||
    map.sampleCoverage.missingLibraries.length > 0 ||
    map.sampleCoverage.requiredFrameCount === 0,
);

const summary = {
  generatedAt: new Date().toISOString(),
  crystalClientRoot: CRYSTAL_CLIENT_ROOT,
  mapSourceDir: MAP_DIR,
  dataMapSourceDir: DATA_MAP_DIR,
  sample: {
    width: SAMPLE_WIDTH,
    height: SAMPLE_HEIGHT,
    note: "Static audit samples one Crystal-style viewport per manifest map. It does not replace human full-map visual acceptance.",
  },
  manifest: {
    totalMaps: maps.length,
    sourceGeneratedAt: respawnManifest.generated_at ?? null,
    sourceFile: respawnManifest.source_file ?? null,
    sourceRoutesDir: respawnManifest.source_routes_dir ?? null,
  },
  sourceMapCoverage: {
    availableMapFileCount: mapFilesByName.size,
    mapFilePresentCount: maps.length - sourceMapMissing.length,
    mapFileMissingCount: sourceMapMissing.length,
    unsupportedMapTypeCount: unsupportedTypes.length,
    parseErrorCount: parseErrors.length,
    missingMaps: sourceMapMissing.map((map) => map.identity),
    unsupportedMaps: unsupportedTypes.map((map) => ({
      ...map.identity,
      type: map.source.type,
    })),
    parseErrors: parseErrors.map((map) => ({
      ...map.identity,
      error: map.source.parseError,
    })),
  },
  sampledSpriteCoverage: {
    mapsWithSampleFrames: mapResults.filter((map) => map.sampleCoverage.requiredFrameCount > 0).length,
    mapsWithNoSampleFrames: sampleNoSpriteMaps.length,
    sourceRequiredFrameCount: sum(mapResults, (map) => map.sampleCoverage.requiredFrameCount),
    sourcePresentFrameCount: sum(mapResults, (map) => map.sampleCoverage.presentSourceFrameCount),
    sourceEmptyFrameCount: sum(mapResults, (map) => map.sampleCoverage.emptySourceFrameCount),
    sourceOutOfRangeFrameCount: sum(mapResults, (map) => map.sampleCoverage.outOfRangeFrameCount),
    crystalIgnoredFrameCount: sum(
      mapResults,
      (map) => map.sampleCoverage.emptySourceFrameCount + map.sampleCoverage.outOfRangeFrameCount,
    ),
    alreadyExportedFrameCount: sum(mapResults, (map) => map.sampleCoverage.alreadyExportedFrameCount),
    mapsWithMissingSourceAssets: sampleMissingSourceAssets.length,
    uniqueMissingLibraryCount: aggregateMissingLibraries.size,
    uniqueOutOfRangeFrameCount: aggregateOutOfRangeFrames.size,
    uniqueEmptyFrameCount: aggregateEmptyFrames.size,
    topMissingLibraries: topCounts(aggregateMissingLibraries),
    topOutOfRangeFrames: topCounts(aggregateOutOfRangeFrames),
    topEmptyFrames: topCounts(aggregateEmptyFrames),
    mapsWithNoSampleFrames: sampleNoSpriteMaps.map((map) => map.identity),
    mapsWithMissingSourceAssets: sampleMissingSourceAssets.map((map) => ({
      ...map.identity,
      missingLibraries: map.sampleCoverage.missingLibraries,
    })),
    mapsWithCrystalIgnoredFrames: sampleCrystalIgnoredFrames.map((map) => ({
      ...map.identity,
      emptyFrames: map.sampleCoverage.emptyFrames,
      outOfRangeFrames: map.sampleCoverage.outOfRangeFrames,
      note: "Crystal MLibrary.GetSize/Draw treats empty or out-of-range frame indices as Size.Empty/no draw; these are tracked but not fallback risk.",
    })),
  },
  miniMapCoverage: {
    neededMiniMapCount: neededMiniMaps.length,
    exportedMiniMapCount: exportedMiniMaps.size,
    missingMiniMapIndices,
    mapsMissingMiniMapAssets,
    mapsWithoutMiniMap,
  },
  visualFallbackRisk: {
    mapCount: visualFallbackRiskMaps.length,
    maps: visualFallbackRiskMaps.map((map) => ({
      ...map.identity,
      reasons: fallbackReasons(map),
    })),
  },
  failures,
  maps: mapResults,
};

console.log(JSON.stringify(summaryBrief(summary), null, 2));
await mkdir(path.dirname(OUTPUT_PATH), { recursive: true });
await writeFile(OUTPUT_PATH, `${JSON.stringify(summary, null, 2)}\n`);
console.log(`Wrote ${OUTPUT_PATH}`);

if (process.env.MIR2_MAP_COVERAGE_STRICT === "1" && visualFallbackRiskMaps.length > 0) {
  process.exitCode = 1;
}

function auditMap(map) {
  const normalized = normalizeMapFileName(map.map_file_name ?? "");
  const mapPath = mapFilesByName.get(normalized) ?? path.join(MAP_DIR, `${normalized}.map`);
  const identity = mapSummary(map);
  const miniMapIndex = Number(map.mini_map);
  const source = {
    mapFileExists: fs.existsSync(mapPath),
    path: mapPath,
    type: null,
    supportedType: false,
    width: null,
    height: null,
    parseError: null,
  };
  const sample = {
    center: null,
    regionBounds: null,
  };
  const sampleCoverage = {
    requiredFrameCount: 0,
    presentSourceFrameCount: 0,
    alreadyExportedFrameCount: 0,
    libraries: [],
    missingLibraries: [],
    emptySourceFrameCount: 0,
    outOfRangeFrameCount: 0,
    emptyFrames: [],
    outOfRangeFrames: [],
  };

  if (!source.mapFileExists) {
    return {
      identity,
      source,
      miniMap: miniMapStatus(miniMapIndex),
      sample,
      sampleCoverage,
    };
  }

  try {
    const bytes = fs.readFileSync(mapPath);
    const parsedMap = parseMapBytes(`${normalized}.map`, bytes);
    source.type = parsedMap.type;
    source.supportedType = SUPPORTED_TYPES.has(parsedMap.type) && Array.isArray(parsedMap.cells);
    source.width = parsedMap.width;
    source.height = parsedMap.height;

    const center = sampleCenterForMap(map, parsedMap);
    const regionBounds = exportBoundsForScene(
      {
        center,
        width: SAMPLE_WIDTH,
        height: SAMPLE_HEIGHT,
      },
      parsedMap,
    );
    sample.center = center;
    sample.regionBounds = regionBounds;

    if (!source.supportedType || !Array.isArray(parsedMap.cells)) {
      return {
        identity,
        source,
        miniMap: miniMapStatus(miniMapIndex),
        sample,
        sampleCoverage,
      };
    }

    const requiredFrames = requiredFramesForRegion(parsedMap, regionBounds);
    const libraryKeys = [...new Set([...requiredFrames.values()].map((frame) => frame.libraryKey))].sort();
    sampleCoverage.requiredFrameCount = requiredFrames.size;
    sampleCoverage.libraries = libraryKeys;

    for (const frame of requiredFrames.values()) {
      const library = loadLibraryMeta(frame.libraryKey);
      if (!library.exists) {
        pushUnique(sampleCoverage.missingLibraries, frame.libraryKey);
        continue;
      }
      if (frame.frameIndex >= library.count) {
        sampleCoverage.outOfRangeFrameCount += 1;
        sampleCoverage.outOfRangeFrames.push({ ...frame, libraryCount: library.count });
        continue;
      }

      const frameMeta = library.frames.get(frame.frameIndex);
      if (!frameMeta || frameMeta.width <= 0 || frameMeta.height <= 0) {
        sampleCoverage.emptySourceFrameCount += 1;
        sampleCoverage.emptyFrames.push(frame);
        continue;
      }
      sampleCoverage.presentSourceFrameCount += 1;
      if (fs.existsSync(publicFramePath(frame.libraryKey, frame.frameIndex))) {
        sampleCoverage.alreadyExportedFrameCount += 1;
      }
    }
  } catch (error) {
    source.parseError = error instanceof Error ? error.message : String(error);
    failures.push(`${identity.mapFileName}: ${source.parseError}`);
  }

  return {
    identity,
    source,
    miniMap: miniMapStatus(miniMapIndex),
    sample,
    sampleCoverage: {
      ...sampleCoverage,
      missingLibraries: sampleCoverage.missingLibraries.sort(),
      emptyFrames: sampleCoverage.emptyFrames.sort((left, right) =>
        `${left.libraryKey}:${left.frameIndex}`.localeCompare(`${right.libraryKey}:${right.frameIndex}`),
      ),
      outOfRangeFrames: sampleCoverage.outOfRangeFrames.sort((left, right) =>
        `${left.libraryKey}:${left.frameIndex}`.localeCompare(`${right.libraryKey}:${right.frameIndex}`),
      ),
    },
  };
}

function requiredFramesForRegion(parsedMap, bounds) {
  const requiredFrames = new Map();
  for (let x = bounds.minX; x <= bounds.maxX; x += 1) {
    for (let y = bounds.minY; y <= bounds.maxY; y += 1) {
      const cell = parsedCellAt(parsedMap, x, y);
      if (!cell) continue;
      if (cell.x % 2 === 0 && cell.y % 2 === 0) registerLayer(backLayerForCell(cell));
      registerLayer(middleLayerForCell(cell));
      registerLayer(frontLayerForCell(cell));
      registerLayer(tileAnimationLayerForCell(cell));
    }
  }
  return requiredFrames;

  function registerLayer(layer) {
    if (!layer) return;
    for (const frameIndex of layer.frames) {
      const key = `${layer.libraryKey}:${frameIndex}`;
      requiredFrames.set(key, {
        libraryKey: layer.libraryKey,
        frameIndex,
      });
    }
  }
}

function parseMapBytes(fileName, bytes) {
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
    default:
      return {
        fileName,
        width: detectMapWidth(bytes, type),
        height: detectMapHeight(bytes, type),
        type,
        cells: null,
      };
  }
}

function parseType0Map(fileName, bytes) {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells = [];
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

function parseType1Map(fileName, bytes) {
  const xor = bytes.readInt16LE(23);
  const width = bytes.readInt16LE(21) ^ xor;
  const height = bytes.readInt16LE(25) ^ xor;
  const cells = [];
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

function parseType2Map(fileName, bytes) {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells = [];
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

function parseType3Map(fileName, bytes) {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells = [];
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

function parseType4Map(fileName, bytes) {
  const xor = bytes.readInt16LE(33);
  const width = bytes.readInt16LE(31) ^ xor;
  const height = bytes.readInt16LE(35) ^ xor;
  const cells = [];
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

function parseType5Map(fileName, bytes) {
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

function parseType6Map(fileName, bytes) {
  const width = bytes.readInt16LE(16);
  const height = bytes.readInt16LE(18);
  const cells = [];
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

function parseType7Map(fileName, bytes) {
  const width = bytes.readInt16LE(21);
  const height = bytes.readInt16LE(25);
  const cells = [];
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

function parseType100Map(fileName, bytes) {
  const width = bytes.readInt16LE(4);
  const height = bytes.readInt16LE(6);
  const cells = [];
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

function loadLibraryMeta(libraryKey) {
  const normalized = normalizeLibraryName(libraryKey);
  const cached = libraryMetaCache.get(normalized);
  if (cached) return cached;

  const libraryPath = path.join(DATA_MAP_DIR, ...normalized.split("/")) + ".Lib";
  const alternatePath = path.join(DATA_MAP_DIR, ...normalized.split("/")) + ".lib";
  const filePath = fs.existsSync(libraryPath) ? libraryPath : alternatePath;
  if (!fs.existsSync(filePath)) {
    const missing = { exists: false, path: filePath, version: null, count: 0, frames: new Map() };
    libraryMetaCache.set(normalized, missing);
    return missing;
  }

  try {
    const buffer = fs.readFileSync(filePath);
    let offset = 0;
    const version = buffer.readInt32LE(offset);
    offset += 4;
    const count = buffer.readInt32LE(offset);
    offset += 4;
    if (version >= 3) offset += 4;
    const frameOffsets = [];
    for (let index = 0; index < count && offset + 4 <= buffer.length; index += 1) {
      frameOffsets.push(buffer.readInt32LE(offset));
      offset += 4;
    }
    const frames = new Map();
    const emptyFrameIndices = [];
    for (let index = 0; index < frameOffsets.length; index += 1) {
      const frameOffset = frameOffsets[index];
      if (frameOffset <= 0 || frameOffset + 4 > buffer.length) {
        emptyFrameIndices.push(index);
        continue;
      }
      frames.set(index, {
        width: buffer.readInt16LE(frameOffset),
        height: buffer.readInt16LE(frameOffset + 2),
      });
    }
    const meta = { exists: true, path: filePath, version, count, frames, emptyFrameIndices };
    libraryMetaCache.set(normalized, meta);
    return meta;
  } catch (error) {
    failures.push(`${normalized}: ${error instanceof Error ? error.message : String(error)}`);
    const broken = { exists: false, path: filePath, version: null, count: 0, frames: new Map() };
    libraryMetaCache.set(normalized, broken);
    return broken;
  }
}

function detectMapType(bytes) {
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

function detectMapWidth(bytes, type) {
  if (type === 1) return bytes.readInt16LE(21) ^ bytes.readInt16LE(23);
  if (type === 4) return bytes.readInt16LE(31) ^ bytes.readInt16LE(33);
  if (type === 5) return bytes.readInt16LE(22);
  if (type === 6) return bytes.readInt16LE(16);
  if (type === 7) return bytes.readInt16LE(21);
  return bytes.readInt16LE(0);
}

function detectMapHeight(bytes, type) {
  if (type === 1) return bytes.readInt16LE(25) ^ bytes.readInt16LE(23);
  if (type === 4) return bytes.readInt16LE(35) ^ bytes.readInt16LE(33);
  if (type === 5) return bytes.readInt16LE(24);
  if (type === 6) return bytes.readInt16LE(18);
  if (type === 7) return bytes.readInt16LE(25);
  return bytes.readInt16LE(2);
}

function emptyParsedMapCell(x, y) {
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

function createEmptyCellGrid(width, height) {
  const cells = [];
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      cells.push(emptyParsedMapCell(x, y));
    }
  }
  return cells;
}

function parsedCellAt(parsedMap, x, y) {
  if (!Array.isArray(parsedMap.cells) || x < 0 || y < 0 || x >= parsedMap.width || y >= parsedMap.height) {
    return null;
  }
  const cell = parsedMap.cells[x * parsedMap.height + y];
  return cell?.x === x && cell.y === y ? cell : null;
}

function backLayerForCell(cell) {
  if (cell.backIndex < 0 || cell.backImage === 0) return null;
  const frameIndex = (cell.backImage & 0x1fffffff) - 1;
  if (frameIndex < 0) return null;
  return { libraryKey: mapLibraryKeyForIndex(cell.backIndex), frames: [frameIndex] };
}

function middleLayerForCell(cell) {
  if (cell.middleIndex < 0) return null;
  const baseFrameIndex = cell.middleImage - 1;
  if (baseFrameIndex < 0) return null;
  return {
    libraryKey: mapLibraryKeyForIndex(cell.middleIndex),
    frames: repeatedAnimationFrames(baseFrameIndex, decodeMiddleAnimationCount(cell.middleAnimationFrame), cell.middleAnimationTick),
  };
}

function frontLayerForCell(cell) {
  if (cell.frontIndex < 0) return null;
  const baseFrameIndex = (cell.frontImage & 0x7fff) - 1;
  if (baseFrameIndex < 0) return null;
  return {
    libraryKey: mapLibraryKeyForIndex(cell.frontIndex),
    frames: repeatedAnimationFrames(baseFrameIndex, decodeFrontAnimationCount(cell.frontAnimationFrame), cell.frontAnimationTick),
  };
}

function tileAnimationLayerForCell(cell) {
  if (cell.tileAnimationImage <= 0 || cell.tileAnimationFrames <= 0) return null;
  const stride = cell.tileAnimationOffset ^ 0x2000;
  const frames = Array.from({ length: cell.tileAnimationFrames }, (_, index) => cell.tileAnimationImage - 1 + stride * index).filter(
    (index) => index >= 0,
  );
  if (!frames.length) return null;
  return { libraryKey: mapLibraryKeyForIndex(190), frames };
}

function repeatedAnimationFrames(baseFrameIndex, animationCount, animationTick) {
  if (animationCount <= 0) return [baseFrameIndex];
  const repeat = 1 + Math.max(animationTick, 0);
  const frames = [];
  for (let frame = 0; frame < animationCount; frame += 1) {
    for (let tick = 0; tick < repeat; tick += 1) frames.push(baseFrameIndex + frame);
  }
  return frames;
}

function decodeMiddleAnimationCount(animationFrame) {
  return animationFrame <= 0 || animationFrame >= 255 ? 0 : animationFrame & 0x0f;
}

function decodeFrontAnimationCount(animationFrame) {
  return animationFrame > 0 ? animationFrame & 0x7f : 0;
}

function mapLibraryKeyForIndex(index) {
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

  if (root === "WemadeMir3") {
    const folders = ["", "Wood", "Sand", "Snow", "Forest"];
    const folder = folders[stateIndex];
    return folder ? `${root}/${folder}/${name}` : `${root}/${name}`;
  }

  const suffixes = ["", "wood", "sand", "snow", "forest"];
  return `${root}/${name}${suffixes[stateIndex] ?? ""}`;
}

function sampleCenterForMap(map, parsedMap) {
  const startZone = (map.safe_zones ?? []).find((zone) => zone.start_point && validPoint(zone.location));
  if (startZone?.location) return clampPoint(startZone.location, parsedMap);
  const safeZone = (map.safe_zones ?? []).find((zone) => validPoint(zone.location));
  if (safeZone?.location) return clampPoint(safeZone.location, parsedMap);
  const respawn = (map.respawns ?? []).find((entry) => validPoint(entry.location));
  if (respawn?.location) return clampPoint(respawn.location, parsedMap);
  const movementSource = (map.movements ?? []).find((entry) => validPoint(entry.source));
  if (movementSource?.source) return clampPoint(movementSource.source, parsedMap);
  const movementDestination = (map.movements ?? []).find((entry) => validPoint(entry.destination));
  if (movementDestination?.destination) return clampPoint(movementDestination.destination, parsedMap);
  return {
    x: Math.floor(Math.max(parsedMap.width - 1, 0) / 2),
    y: Math.floor(Math.max(parsedMap.height - 1, 0) / 2),
  };
}

function exportBoundsForScene(sceneView, parsedMap) {
  const play = playBoundsForScene(sceneView, parsedMap);
  return {
    minX: clampInt(play.minX - Math.floor(sceneView.width / 2) - 4, 0, Math.max(parsedMap.width - 1, 0)),
    maxX: clampInt(play.maxX + Math.floor(sceneView.width / 2) + 4, 0, Math.max(parsedMap.width - 1, 0)),
    minY: clampInt(play.minY - Math.floor(sceneView.height / 2) - 4, 0, Math.max(parsedMap.height - 1, 0)),
    maxY: clampInt(play.maxY + Math.floor(sceneView.height / 2) + 25, 0, Math.max(parsedMap.height - 1, 0)),
  };
}

function playBoundsForScene(sceneView, parsedMap) {
  const halfWidth = Math.floor(sceneView.width / 2);
  const halfHeight = Math.floor(sceneView.height / 2);
  return {
    minX: clampInt(sceneView.center.x - halfWidth, 0, Math.max(parsedMap.width - 1, 0)),
    maxX: clampInt(sceneView.center.x + halfWidth, 0, Math.max(parsedMap.width - 1, 0)),
    minY: clampInt(sceneView.center.y - halfHeight, 0, Math.max(parsedMap.height - 1, 0)),
    maxY: clampInt(sceneView.center.y + halfHeight, 0, Math.max(parsedMap.height - 1, 0)),
  };
}

function publicFramePath(libraryKey, frameIndex) {
  return path.join(PUBLIC_ORIGINAL_MAP_DIR, ...normalizeLibraryName(libraryKey).split("/"), `${frameIndex}.png`);
}

function miniMapStatus(index) {
  if (!(index > 0)) {
    return {
      index: Number.isFinite(index) ? index : null,
      required: false,
      exported: false,
    };
  }
  const frame = exportedMiniMaps.get(index);
  return {
    index,
    required: true,
    exported: Boolean(frame),
    width: frame?.width ?? null,
    height: frame?.height ?? null,
  };
}

function fallbackReasons(map) {
  const reasons = [];
  if (!map.source.mapFileExists) reasons.push("missing Crystal map file");
  if (map.source.mapFileExists && !map.source.supportedType) reasons.push(`unsupported or unparsed map type ${map.source.type}`);
  if (map.source.parseError) reasons.push(`parse error: ${map.source.parseError}`);
  if (map.sampleCoverage.requiredFrameCount === 0) reasons.push("sample viewport has no real source sprites");
  if (map.sampleCoverage.missingLibraries.length > 0) reasons.push("sample viewport references missing map libraries");
  return reasons;
}

function summaryBrief(summary) {
  return {
    generatedAt: summary.generatedAt,
    totalMaps: summary.manifest.totalMaps,
    sourceMapCoverage: summary.sourceMapCoverage,
    sampledSpriteCoverage: {
      mapsWithSampleFrames: summary.sampledSpriteCoverage.mapsWithSampleFrames,
      mapsWithNoSampleFrames: summary.sampledSpriteCoverage.mapsWithNoSampleFrames.length,
      sourceRequiredFrameCount: summary.sampledSpriteCoverage.sourceRequiredFrameCount,
      sourcePresentFrameCount: summary.sampledSpriteCoverage.sourcePresentFrameCount,
      sourceEmptyFrameCount: summary.sampledSpriteCoverage.sourceEmptyFrameCount,
      sourceOutOfRangeFrameCount: summary.sampledSpriteCoverage.sourceOutOfRangeFrameCount,
      crystalIgnoredFrameCount: summary.sampledSpriteCoverage.crystalIgnoredFrameCount,
      alreadyExportedFrameCount: summary.sampledSpriteCoverage.alreadyExportedFrameCount,
      mapsWithMissingSourceAssets: summary.sampledSpriteCoverage.mapsWithMissingSourceAssets.length,
      mapsWithCrystalIgnoredFrames: summary.sampledSpriteCoverage.mapsWithCrystalIgnoredFrames.length,
      uniqueMissingLibraryCount: summary.sampledSpriteCoverage.uniqueMissingLibraryCount,
      uniqueOutOfRangeFrameCount: summary.sampledSpriteCoverage.uniqueOutOfRangeFrameCount,
      uniqueEmptyFrameCount: summary.sampledSpriteCoverage.uniqueEmptyFrameCount,
    },
    miniMapCoverage: {
      neededMiniMapCount: summary.miniMapCoverage.neededMiniMapCount,
      exportedMiniMapCount: summary.miniMapCoverage.exportedMiniMapCount,
      missingMiniMapIndices: summary.miniMapCoverage.missingMiniMapIndices,
      mapsMissingMiniMapAssets: summary.miniMapCoverage.mapsMissingMiniMapAssets,
      mapsWithoutMiniMapCount: summary.miniMapCoverage.mapsWithoutMiniMap.length,
    },
    visualFallbackRisk: {
      mapCount: summary.visualFallbackRisk.mapCount,
    },
    failureCount: summary.failures.length,
  };
}

function mapSummary(map) {
  return {
    mapIndex: map.map_index ?? null,
    mapFileName: normalizeMapFileName(map.map_file_name ?? ""),
    title: map.map_title ?? null,
    miniMap: Number.isFinite(Number(map.mini_map)) ? Number(map.mini_map) : null,
    bigMap: Number.isFinite(Number(map.big_map)) ? Number(map.big_map) : null,
  };
}

function indexFilesByNormalizedStem(dir, extension, recursive) {
  const result = new Map();
  if (!fs.existsSync(dir)) return result;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (recursive) {
        for (const [key, value] of indexFilesByNormalizedStem(entryPath, extension, true)) {
          result.set(key, value);
        }
      }
      continue;
    }
    if (!entry.isFile() || !entry.name.toLowerCase().endsWith(extension.toLowerCase())) continue;
    const stem = entry.name.slice(0, -extension.length);
    result.set(normalizeMapFileName(stem), entryPath);
  }
  return result;
}

function normalizeMapFileName(mapFileName) {
  const normalized = String(mapFileName || "0").trim().replaceAll("\\", "/").split("/").pop() ?? "0";
  return normalized.replace(/\.map$/i, "") || "0";
}

function normalizeLibraryName(libraryName) {
  return String(libraryName).replaceAll("\\", "/").split("/").filter(Boolean).join("/");
}

function normalizeBackImage(image) {
  return (image & 0x8000) !== 0 ? (image & 0x7fff) | 0x20000000 : image;
}

function signed16(value) {
  return (value << 16) >> 16;
}

function validPoint(point) {
  return Number.isFinite(Number(point?.x)) && Number.isFinite(Number(point?.y));
}

function clampPoint(point, parsedMap) {
  return {
    x: clampInt(Number(point.x), 0, Math.max(parsedMap.width - 1, 0)),
    y: clampInt(Number(point.y), 0, Math.max(parsedMap.height - 1, 0)),
  };
}

function clampInt(value, min, max) {
  if (!Number.isFinite(value)) return min;
  return Math.max(min, Math.min(max, Math.trunc(value)));
}

function positiveInt(value, fallback) {
  const parsed = Number.parseInt(value ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function pushUnique(array, value) {
  if (!array.includes(value)) array.push(value);
}

function sum(values, selector) {
  return values.reduce((total, value) => total + selector(value), 0);
}

function topCounts(counts) {
  return [...counts.entries()]
    .map(([key, count]) => ({ key, count }))
    .sort((left, right) => right.count - left.count || left.key.localeCompare(right.key))
    .slice(0, 20);
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}
