import { existsSync, readFileSync } from "node:fs";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { deflateSync, gunzipSync } from "node:zlib";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const PUBLIC_DIR = path.join(WORKSPACE_ROOT, "public", "original-map");
const STARTER_SCENE_PATH = path.resolve(
  WORKSPACE_ROOT,
  "..",
  "..",
  "packages",
  "game-data",
  "data",
  "starter_scene.json",
);
const OUTPUT_JSON_PATH = path.resolve(
  WORKSPACE_ROOT,
  "..",
  "..",
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_starter_map_region.json",
);
const WHOLEMAP_SUMMARY_PATH = path.resolve(
  WORKSPACE_ROOT,
  "..",
  "..",
  "docs",
  "generated",
  "map",
  "wholemap",
  "_grand-summary.json",
);
const DEFAULT_CLIENT_ROOT = "E:\\mir2\\Crystal\\Build\\Client\\Debug";

const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const CELL_WIDTH = 48;
const CELL_HEIGHT = 32;
const args = parseArgs(process.argv.slice(2));

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

async function main() {
  const clientRoot = args.clientRoot ?? args._[0] ?? DEFAULT_CLIENT_ROOT;
  if (args.frames.length > 0) {
    await exportExplicitFrames(clientRoot, args.frames);
    return;
  }

  if (args.allMaps) {
    await exportAllMaps(clientRoot, { overwrite: Boolean(args.overwrite) });
    return;
  }

  if (args.full) {
    await exportFullMap(clientRoot, args.map, { overwrite: Boolean(args.overwrite) });
    return;
  }

  const starterScene = JSON.parse(await readFile(STARTER_SCENE_PATH, "utf8"));
  const mapFileName = String(starterScene.map?.file_name ?? "0");
  const mapPath = path.join(clientRoot, "Map", `${mapFileName}.map`);
  const dataDir = path.join(clientRoot, "Data");
  const sceneView = starterScene.scene_view;

  if (!sceneView?.center) {
    throw new Error("starter scene is missing scene_view.center");
  }

  await mkdir(PUBLIC_DIR, { recursive: true });
  await mkdir(path.dirname(OUTPUT_JSON_PATH), { recursive: true });

  const bounds = exportBoundsForScene(sceneView);
  const sprites = {};
  const cells = [];
  const spriteIds = new Map();
  const exportedFrames = new Set();
  const libraryCache = new Map();
  const pendingWrites = [];

  forEachMapCell(mapPath, ({ x, y, cell }) => {
    if (!cellInBounds(bounds, x, y)) {
      return;
    }

    const outputCell = { x, y };

    if (x % 2 === 0 && y % 2 === 0) {
      const backLayer = backLayerForCell(cell);
      if (backLayer) {
        outputCell.back = registerSprite(backLayer, "back");
      }
    }

    const middleLayer = middleLayerForCell(cell);
    if (middleLayer) {
      outputCell.middle = registerSprite(middleLayer, "middle");
    }

    const frontLayer = frontLayerForCell(cell);
    if (frontLayer) {
      outputCell.front = registerSprite(frontLayer, "front");
    }

    const tileAnimationLayer = tileAnimationLayerForCell(cell);
    if (tileAnimationLayer) {
      outputCell.tileAnimation = registerSprite(tileAnimationLayer, "tileAnimation");
    }

    if (Object.keys(outputCell).length > 2) {
      cells.push(outputCell);
    }
  });

  const output = {
    mapFileName: `${mapFileName}.map`,
    cellWidth: CELL_WIDTH,
    cellHeight: CELL_HEIGHT,
    regionBounds: bounds,
    playBounds: playBoundsForScene(sceneView),
    sprites,
    cells,
  };

  await Promise.all(pendingWrites);
  await writeFile(OUTPUT_JSON_PATH, `${JSON.stringify(output, null, 2)}\n`, "utf8");
  console.log(`Exported starter map region to ${OUTPUT_JSON_PATH}`);

  function registerSprite(layer, kind) {
    const library = ensureLibrary(layer.libraryKey);
    const drawMode = resolveDrawMode(layer, library);
    const spriteKey = `${kind}|${layer.libraryKey}|${drawMode}|${layer.frames.join(",")}`;
    const existingId = spriteIds.get(spriteKey);
    if (existingId) {
      return existingId;
    }

    const id = `sprite-${spriteIds.size + 1}`;
    const frames = layer.frames
      .map((frameIndex) => exportFrame(layer.libraryKey, library, frameIndex))
      .filter((frame) => frame !== null);

    if (!frames.length) {
      return null;
    }

    sprites[id] = {
      kind,
      drawMode,
      frames,
    };
    spriteIds.set(spriteKey, id);
    return id;
  }

  function ensureLibrary(libraryKey) {
    const cached = libraryCache.get(libraryKey);
    if (cached) {
      return cached;
    }

    const libraryPath = path.join(dataDir, "Map", ...libraryKey.split("/")) + ".Lib";
    const parsed = parseLibrary(libraryPath);
    libraryCache.set(libraryKey, parsed);
    return parsed;
  }

  function exportFrame(libraryKey, library, frameIndex) {
    const frame = library.frames[frameIndex];
    if (!frame || frame.width <= 0 || frame.height <= 0) {
      return null;
    }

    const normalizedKey = normalizeLibraryName(libraryKey);
    const frameKey = `${normalizedKey}:${frameIndex}`;
    const exportDir = path.join(PUBLIC_DIR, ...normalizedKey.split("/"));
    const pngPath = path.join(exportDir, `${frameIndex}.png`);
    const rgba = postProcessFrameRgba(normalizedKey, frameIndex, frame.rgba);

    if (!exportedFrames.has(frameKey)) {
      exportedFrames.add(frameKey);
      pendingWrites.push(
        mkdir(exportDir, { recursive: true }).then(() =>
          writeFile(pngPath, encodePng(frame.width, frame.height, rgba)),
        ),
      );
    }

    return {
      path: `/original-map/${normalizedKey}/${frameIndex}.png`,
      width: frame.width,
      height: frame.height,
      offsetX: frame.x,
      offsetY: frame.y,
    };
  }
}

async function exportExplicitFrames(clientRoot, specs) {
  const dataDir = path.join(clientRoot, "Data");
  const grouped = new Map();
  for (const spec of specs) {
    const parsed = parseFrameSpec(spec);
    if (!parsed) {
      throw new Error(`Invalid frame spec: ${spec}. Expected Library/Key:1,2,3`);
    }
    const frames = grouped.get(parsed.libraryKey) ?? new Set();
    for (const frame of parsed.frames) frames.add(frame);
    grouped.set(parsed.libraryKey, frames);
  }

  const exported = [];
  for (const [libraryKey, frames] of grouped) {
    const libraryPath = path.join(dataDir, "Map", ...libraryKey.split("/")) + ".Lib";
    const library = parseLibrary(libraryPath);
    for (const frameIndex of [...frames].sort((a, b) => a - b)) {
      const frame = library.frames[frameIndex];
      if (!frame || frame.width <= 0 || frame.height <= 0) {
        throw new Error(`Missing or empty frame ${libraryKey}:${frameIndex}`);
      }
      const normalizedKey = normalizeLibraryName(libraryKey);
      const exportDir = path.join(PUBLIC_DIR, ...normalizedKey.split("/"));
      const pngPath = path.join(exportDir, `${frameIndex}.png`);
      const rgba = postProcessFrameRgba(normalizedKey, frameIndex, frame.rgba);
      await mkdir(exportDir, { recursive: true });
      await writeFile(pngPath, encodePng(frame.width, frame.height, rgba));
      exported.push({
        libraryKey: normalizedKey,
        frameIndex,
        path: `/original-map/${normalizedKey}/${frameIndex}.png`,
        width: frame.width,
        height: frame.height,
        offsetX: frame.x,
        offsetY: frame.y,
      });
    }
  }

  console.log(JSON.stringify({ ok: true, clientRoot, exported }, null, 2));
}

// Full-map export: sweep EVERY cell of the map and export every referenced
// sprite frame (back/middle/front/tile-animation) so a whole town like Bichon
// has no missing PNG "holes" beyond the starter viewport. Runs where the
// Crystal client .Lib libraries already live (no client needed in the cloud
// container); the resulting small PNGs are committed via git.
async function exportFullMap(clientRoot, mapFileNameArg, options = {}) {
  const overwrite = Boolean(options.overwrite);
  const mapFileName =
    mapFileNameArg ??
    String(JSON.parse(readFileSync(STARTER_SCENE_PATH, "utf8")).map?.file_name ?? "0");
  const mapPath = path.join(clientRoot, "Map", `${mapFileName}.map`);
  const dataDir = path.join(clientRoot, "Data");

  const warnings = new Set();
  const wanted = new Map(); // libraryKey -> Set<frameIndex>
  let cellsScanned = 0;

  function want(layer) {
    if (!layer) {
      return;
    }
    let frames = wanted.get(layer.libraryKey);
    if (!frames) {
      frames = new Set();
      wanted.set(layer.libraryKey, frames);
    }
    for (const frameIndex of layer.frames) {
      if (Number.isInteger(frameIndex) && frameIndex >= 0) {
        frames.add(frameIndex);
      }
    }
  }

  function safeLayer(fn, cell) {
    try {
      return fn(cell);
    } catch (error) {
      warnings.add(error.message);
      return null;
    }
  }

  forEachMapCell(mapPath, ({ x, y, cell }) => {
    cellsScanned += 1;
    if (x % 2 === 0 && y % 2 === 0) {
      want(safeLayer(backLayerForCell, cell));
    }
    want(safeLayer(middleLayerForCell, cell));
    want(safeLayer(frontLayerForCell, cell));
    want(safeLayer(tileAnimationLayerForCell, cell));
  });

  await mkdir(PUBLIC_DIR, { recursive: true });

  const CONCURRENCY = 64;
  let framesExported = 0;
  let framesSkipped = 0;
  let alreadyPresent = 0;

  // Process one library at a time so only a single (potentially large) .Lib
  // buffer is held in memory, and write PNGs in bounded batches to avoid EMFILE.
  for (const [libraryKey, frameSet] of wanted) {
    const libraryPath = path.join(dataDir, "Map", ...libraryKey.split("/")) + ".Lib";
    let library = null;
    try {
      library = parseLibraryLazy(libraryPath);
    } catch (error) {
      warnings.add(`library ${libraryKey}: ${error.message}`);
      continue;
    }

    const normalizedKey = normalizeLibraryName(libraryKey);
    const exportDir = path.join(PUBLIC_DIR, ...normalizedKey.split("/"));
    await mkdir(exportDir, { recursive: true });

    const indices = [...frameSet].sort((a, b) => a - b);
    for (let start = 0; start < indices.length; start += CONCURRENCY) {
      const batch = indices.slice(start, start + CONCURRENCY);
      await Promise.all(
        batch.map(async (frameIndex) => {
          const pngPath = path.join(exportDir, `${frameIndex}.png`);
          if (!overwrite && existsSync(pngPath)) {
            alreadyPresent += 1;
            return;
          }
          const frame = libraryFrame(library, frameIndex);
          if (!frame || frame.width <= 0 || frame.height <= 0) {
            framesSkipped += 1;
            return;
          }
          const rgba = postProcessFrameRgba(normalizedKey, frameIndex, frame.rgba);
          await writeFile(pngPath, encodePng(frame.width, frame.height, rgba));
          framesExported += 1;
        }),
      );
    }

    library = null;
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        mode: "full-map",
        clientRoot,
        mapFileName: `${mapFileName}.map`,
        cellsScanned,
        librariesReferenced: wanted.size,
        framesExported,
        alreadyPresent,
        framesSkipped,
        warnings: [...warnings],
      },
      null,
      2,
    ),
  );
}

async function exportAllMaps(clientRoot, options = {}) {
  const mapDir = path.join(clientRoot, "Map");
  const dataDir = path.join(clientRoot, "Data");
  const mapFiles = (await listFilesRecursive(mapDir))
    .filter((filePath) => /\.map$/i.test(filePath))
    .sort((left, right) => left.localeCompare(right, "en"));
  const wanted = new Map();
  const maps = [];
  const skippedMaps = [];
  const warnings = new Set();

  for (const mapPath of mapFiles) {
    const relativePath = path.relative(mapDir, mapPath).split(path.sep).join("/");
    const mapWarnings = new Set();
    const before = snapshotWantedCounts(wanted);
    let cellsScanned = 0;

    function want(layer) {
      if (!layer) return;
      let frames = wanted.get(layer.libraryKey);
      if (!frames) {
        frames = new Set();
        wanted.set(layer.libraryKey, frames);
      }
      for (const frameIndex of layer.frames) {
        if (Number.isInteger(frameIndex) && frameIndex >= 0) frames.add(frameIndex);
      }
    }

    function safeLayer(fn, cell) {
      try {
        return fn(cell);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        mapWarnings.add(message);
        warnings.add(`${relativePath}: ${message}`);
        return null;
      }
    }

    try {
      forEachMapCell(mapPath, ({ x, y, cell }) => {
        cellsScanned += 1;
        if (x % 2 === 0 && y % 2 === 0) {
          want(safeLayer(backLayerForCell, cell));
        }
        want(safeLayer(middleLayerForCell, cell));
        want(safeLayer(frontLayerForCell, cell));
        want(safeLayer(tileAnimationLayerForCell, cell));
      });
    } catch (error) {
      skippedMaps.push({
        path: relativePath,
        error: error instanceof Error ? error.message : String(error),
      });
      continue;
    }

    const after = snapshotWantedCounts(wanted);
    maps.push({
      path: relativePath,
      fileName: relativePath.replace(/\.map$/i, ""),
      cellsScanned,
      librariesReferenced: countMapLibrariesDelta(before.libraries, wanted),
      framesReferenced: after.frames - before.frames,
      warnings: [...mapWarnings],
    });
  }

  const summary = {
    generatedAt: new Date().toISOString(),
    ok: skippedMaps.length === 0,
    mode: "all-maps",
    clientRoot,
    mapDir,
    outputDir: PUBLIC_DIR,
    mapCount: maps.length,
    mapFileCount: mapFiles.length,
    skippedMaps,
    maps,
    warnings: [...warnings],
    ...snapshotWantedCounts(wanted),
  };

  if (skippedMaps.length === 0) {
    Object.assign(summary, await exportWantedMapFrames(dataDir, wanted, options));
  }

  await mkdir(path.dirname(WHOLEMAP_SUMMARY_PATH), { recursive: true });
  await writeFile(WHOLEMAP_SUMMARY_PATH, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  console.log(JSON.stringify(summary, null, 2));

  if (skippedMaps.length > 0) {
    throw new Error(`Failed to parse ${skippedMaps.length}/${mapFiles.length} map(s); see ${WHOLEMAP_SUMMARY_PATH}`);
  }
}

async function exportWantedMapFrames(dataDir, wanted, options = {}) {
  const overwrite = Boolean(options.overwrite);
  const CONCURRENCY = 64;
  let framesExported = 0;
  let framesSkipped = 0;
  let alreadyPresent = 0;
  const warnings = [];

  await mkdir(PUBLIC_DIR, { recursive: true });

  for (const [libraryKey, frameSet] of [...wanted.entries()].sort(([left], [right]) =>
    left.localeCompare(right, "en"),
  )) {
    const libraryPath = path.join(dataDir, "Map", ...libraryKey.split("/")) + ".Lib";
    let library = null;
    try {
      library = parseLibraryLazy(libraryPath);
    } catch (error) {
      warnings.push(`library ${libraryKey}: ${error instanceof Error ? error.message : String(error)}`);
      continue;
    }

    const normalizedKey = normalizeLibraryName(libraryKey);
    const exportDir = path.join(PUBLIC_DIR, ...normalizedKey.split("/"));
    await mkdir(exportDir, { recursive: true });

    const indices = [...frameSet].sort((a, b) => a - b);
    for (let start = 0; start < indices.length; start += CONCURRENCY) {
      const batch = indices.slice(start, start + CONCURRENCY);
      await Promise.all(
        batch.map(async (frameIndex) => {
          const pngPath = path.join(exportDir, `${frameIndex}.png`);
          if (!overwrite && existsSync(pngPath)) {
            alreadyPresent += 1;
            return;
          }
          const frame = libraryFrame(library, frameIndex);
          if (!frame || frame.width <= 0 || frame.height <= 0) {
            framesSkipped += 1;
            return;
          }
          const rgba = postProcessFrameRgba(normalizedKey, frameIndex, frame.rgba);
          await writeFile(pngPath, encodePng(frame.width, frame.height, rgba));
          framesExported += 1;
        }),
      );
    }

    library = null;
  }

  return {
    framesExported,
    alreadyPresent,
    framesSkipped,
    exportWarnings: warnings,
  };
}

function snapshotWantedCounts(wanted) {
  let frames = 0;
  for (const frameSet of wanted.values()) frames += frameSet.size;
  return { libraries: wanted.size, frames };
}

function countMapLibrariesDelta(previousLibraryCount, wanted) {
  return Math.max(0, wanted.size - previousLibraryCount);
}

function exportBoundsForScene(sceneView) {
  const play = playBoundsForScene(sceneView);
  return {
    minX: play.minX - Math.floor(sceneView.width / 2) - 4,
    maxX: play.maxX + Math.floor(sceneView.width / 2) + 4,
    minY: play.minY - Math.floor(sceneView.height / 2) - 4,
    maxY: play.maxY + Math.floor(sceneView.height / 2) + 25,
  };
}

function playBoundsForScene(sceneView) {
  const halfWidth = Math.floor(sceneView.width / 2);
  const halfHeight = Math.floor(sceneView.height / 2);
  return {
    minX: sceneView.center.x - halfWidth,
    maxX: sceneView.center.x + halfWidth,
    minY: sceneView.center.y - halfHeight,
    maxY: sceneView.center.y + halfHeight,
  };
}

function cellInBounds(bounds, x, y) {
  return x >= bounds.minX && x <= bounds.maxX && y >= bounds.minY && y <= bounds.maxY;
}

function backLayerForCell(cell) {
  if (cell.backIndex < 0 || cell.backImage === 0) {
    return null;
  }

  const frameIndex = (cell.backImage & 0x1fffffff) - 1;
  if (frameIndex < 0) {
    return null;
  }

  return {
    libraryKey: mapLibraryKeyForIndex(cell.backIndex),
    drawMode: "floor",
    frames: [frameIndex],
  };
}

function middleLayerForCell(cell) {
  if (cell.middleIndex < 0) {
    return null;
  }

  const baseFrameIndex = cell.middleImage - 1;
  if (baseFrameIndex < 0) {
    return null;
  }

  const frames = repeatedAnimationFrames(
    baseFrameIndex,
    decodeMiddleAnimationCount(cell.middleAnimationFrame),
    cell.middleAnimationTick,
  );
  return {
    libraryKey: mapLibraryKeyForIndex(cell.middleIndex),
    drawMode: "auto",
    frames,
  };
}

function frontLayerForCell(cell) {
  if (cell.frontIndex < 0) {
    return null;
  }

  const baseFrameIndex = (cell.frontImage & 0x7fff) - 1;
  if (baseFrameIndex < 0) {
    return null;
  }

  const animationCount = decodeFrontAnimationCount(cell.frontAnimationFrame);
  const frames = repeatedAnimationFrames(baseFrameIndex, animationCount, cell.frontAnimationTick);
  return {
    libraryKey: mapLibraryKeyForIndex(cell.frontIndex),
    drawMode: "auto",
    frames,
  };
}

function tileAnimationLayerForCell(cell) {
  if (cell.tileAnimationImage <= 0 || cell.tileAnimationFrames <= 0) {
    return null;
  }

  const stride = cell.tileAnimationOffset ^ 0x2000;
  const frames = [];
  for (let frame = 0; frame < cell.tileAnimationFrames; frame += 1) {
    const index = cell.tileAnimationImage - 1 + stride * frame;
    if (index >= 0) {
      frames.push(index);
    }
  }

  if (!frames.length) {
    return null;
  }

  return {
    libraryKey: mapLibraryKeyForIndex(190),
    drawMode: "object",
    frames,
  };
}

function decodeMiddleAnimationCount(animationFrame) {
  if (animationFrame <= 0 || animationFrame >= 255) {
    return 0;
  }

  return animationFrame & 0x0f;
}

function decodeFrontAnimationCount(animationFrame) {
  return animationFrame > 0 ? animationFrame & 0x7f : 0;
}

function repeatedAnimationFrames(baseFrameIndex, animationCount, animationTick) {
  if (animationCount <= 0) {
    return [baseFrameIndex];
  }

  const repeat = 1 + Math.max(animationTick, 0);
  const frames = [];

  for (let frame = 0; frame < animationCount; frame += 1) {
    for (let tick = 0; tick < repeat; tick += 1) {
      frames.push(baseFrameIndex + frame);
    }
  }

  return frames;
}

function resolveDrawMode(layer, library) {
  if (layer.drawMode !== "auto") {
    return layer.drawMode;
  }

  const frame = library.frames[layer.frames[0]];
  if (!frame) {
    return "floor";
  }

  const floorSized =
    (frame.width === CELL_WIDTH && frame.height === CELL_HEIGHT) ||
    (frame.width === CELL_WIDTH * 2 && frame.height === CELL_HEIGHT * 2);

  return layer.frames.length === 1 && floorSized ? "floor" : "object";
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

  const suffixes = ["", "2", "3", "4", "5"];
  return `${root}/${name}${suffixes[stateIndex] ?? ""}`;
}

function normalizeLibraryName(libraryName) {
  return String(libraryName)
    .replaceAll("\\", "/")
    .split("/")
    .filter(Boolean)
    .join("/");
}

function parseFrameSpec(spec) {
  const [libraryKey, rawFrames] = String(spec).split(":");
  if (!libraryKey || !rawFrames) return null;
  const frames = rawFrames
    .split(",")
    .map((value) => Number.parseInt(value.trim(), 10))
    .filter((value) => Number.isInteger(value) && value >= 0);
  if (frames.length === 0) return null;
  return { libraryKey: normalizeLibraryName(libraryKey), frames };
}

function parseArgs(values) {
  const parsed = { _: [], frames: [] };
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === "--clientRoot") {
      parsed.clientRoot = requireValue(values, index, value);
      index += 1;
      continue;
    }
    if (value === "--full" || value === "--full-map") {
      parsed.full = true;
      continue;
    }
    if (value === "--all-maps") {
      parsed.allMaps = true;
      continue;
    }
    if (value === "--overwrite") {
      parsed.overwrite = true;
      continue;
    }
    if (value === "--map") {
      parsed.map = requireValue(values, index, value);
      index += 1;
      continue;
    }
    if (value === "--frame") {
      parsed.frames.push(requireValue(values, index, value));
      index += 1;
      continue;
    }
    if (value === "--frames") {
      parsed.frames.push(...requireValue(values, index, value).split(";").filter(Boolean));
      index += 1;
      continue;
    }
    if (!value.startsWith("--")) {
      parsed._.push(value);
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }
  return parsed;
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
    if (entry.isFile()) files.push(childPath);
  }
  return files;
}

function requireValue(values, index, flag) {
  const next = values[index + 1];
  if (!next || next.startsWith("--")) {
    throw new Error(`${flag} requires a value`);
  }
  return next;
}

function postProcessFrameRgba(libraryName, frameIndex, rgba) {
  if (libraryName !== "WemadeMir2/Objects" || frameIndex < 2723 || frameIndex > 2732) {
    return rgba;
  }

  const next = Buffer.from(rgba);

  for (let index = 0; index < next.length; index += 4) {
    const r = next[index];
    const g = next[index + 1];
    const b = next[index + 2];
    const alpha = next[index + 3];
    const brightness = Math.max(r, g, b);

    if (brightness <= 20) {
      next[index + 3] = 0;
      continue;
    }

    if (brightness < 72) {
      const scaledAlpha = Math.round(((brightness - 20) / 52) * alpha);
      next[index + 3] = Math.max(0, Math.min(alpha, scaledAlpha));
    }
  }

  return next;
}

function forEachMapCell(mapPath, visit) {
  const parsedMap = parseMapBytes(path.basename(mapPath), readFileSyncCompat(mapPath));
  if (!Array.isArray(parsedMap.cells)) {
    throw new Error(`Unsupported Crystal map type ${parsedMap.type}: ${mapPath}`);
  }

  for (const cell of parsedMap.cells) {
    visit({ x: cell.x, y: cell.y, cell });
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

function normalizeBackImage(image) {
  return (image & 0x8000) !== 0 ? (image & 0x7fff) | 0x20000000 : image;
}

function signed16(value) {
  return (value << 16) >> 16;
}

function readFileSyncCompat(filePath) {
  return readFileSync(filePath);
}

function parseLibrary(filePath) {
  const buffer = readFileSyncCompat(filePath);
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

// Lazy variant of parseLibrary: reads the frame offset table but defers frame
// decoding to libraryFrame(), so a full-map sweep only decodes the frames it
// actually needs instead of every frame in a large .Lib.
function parseLibraryLazy(filePath) {
  const buffer = readFileSyncCompat(filePath);
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

  return { version, count, frameOffsets, buffer };
}

function libraryFrame(library, index) {
  if (index < 0 || index >= library.count) {
    return null;
  }
  const frameOffset = library.frameOffsets[index];
  if (!frameOffset || frameOffset <= 0 || frameOffset >= library.buffer.length) {
    return null;
  }
  return parseFrame(library.buffer, frameOffset, index);
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
