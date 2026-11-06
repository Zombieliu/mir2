#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../../../..");
const webRoot = path.join(repoRoot, "apps/web");

const mapFileName = process.argv[2] ?? "0";
const centerX = numberArg(process.argv[3], 288);
const centerY = numberArg(process.argv[4], 616);
const points = (process.argv.slice(5).length > 0
  ? process.argv.slice(5)
  : ["365,335", "540,580"]
).map(parsePoint);

const mapBytes = zlib.gunzipSync(
  fs.readFileSync(path.join(webRoot, `lib/generated/crystal-map-pack/${mapFileName}.map.gz`)),
);
const mapWidth = mapBytes.readUInt16LE(4);
const mapHeight = mapBytes.readUInt16LE(6);
const atlasManifest = JSON.parse(
  fs.readFileSync(path.join(webRoot, "public/generated/map-atlas/manifest.json"), "utf8"),
);
const keyedManifest = JSON.parse(
  fs.readFileSync(
    path.join(webRoot, "public/generated/native-map-keyed/manifest.json"),
    "utf8",
  ),
);

const atlasRects = new Map();
for (const page of atlasManifest.pages ?? []) {
  for (const rect of page.r ?? []) {
    atlasRects.set(`${page.l}#${rect[0]}`, {
      width: rect[3],
      height: rect[4],
      imageUrl: page.u,
      atlasKey: `map:${page.l}#p${page.p}`,
    });
  }
}
const standaloneEntries = new Map(
  (keyedManifest.entries ?? []).map((entry) => [entry.key, entry]),
);

const draws = [];
const marginX = 19 / 2 + 6;
const marginY = 15 / 2 + 6;
for (let x = Math.floor(centerX - marginX); x <= Math.ceil(centerX + marginX); x += 1) {
  for (let y = Math.floor(centerY - marginY); y <= Math.ceil(centerY + marginY); y += 1) {
    if (x < 0 || y < 0 || x >= mapWidth || y >= mapHeight) continue;
    const offset = 8 + (x * mapHeight + y) * 26;
    const cellLayers = resolveCellLayers(mapBytes, offset, x, y);
    for (const layer of cellLayers) {
      const library = libraryKeyForIndex(layer.libraryIndex);
      const rectKey = `${library}#${layer.frameIndex}`;
      const standalone = standaloneEntries.get(rectKey);
      const atlas = atlasRects.get(rectKey);
      const asset = standalone ?? atlas;
      if (!asset) continue;

      const screenRect = screenRectForDraw({
        centerX,
        centerY,
        x,
        y,
        layer,
        asset,
        standalone: Boolean(standalone),
      });
      const overlaps = points.map((point) => pointInRect(point, screenRect));
      if (!overlaps.some(Boolean)) continue;

      draws.push({
        map: [x, y],
        screen: [screenRect.left, screenRect.top, screenRect.width, screenRect.height],
        layer: layer.name,
        rectKey,
        route: standalone ? "standalone" : "atlas",
        additive: layer.additive,
        frameCount: layer.frameCount,
        overlaps,
        imageUrl: asset.imageUrl,
        placementMode: standalone?.placementMode ?? null,
        offset: standalone ? [standalone.offsetX ?? 0, standalone.offsetY ?? 0] : null,
      });
    }
  }
}

console.log(
  JSON.stringify(
    {
      mapFileName,
      mapSize: [mapWidth, mapHeight],
      center: [centerX, centerY],
      points,
      draws,
    },
    null,
    2,
  ),
);

function resolveCellLayers(bytes, offset, x, y) {
  const layers = [];
  const backIndex = bytes.readInt16LE(offset);
  const backFrame = (bytes.readInt32LE(offset + 2) & 0x1fffffff) - 1;
  if (backIndex >= 0 && backFrame >= 0 && x % 2 === 0 && y % 2 === 0) {
    layers.push(layer("back", backIndex, backFrame, 1, false));
  }

  const middleIndex = bytes.readInt16LE(offset + 6);
  const middleFrame = bytes.readInt16LE(offset + 8) - 1;
  const middleAnimation = bytes[offset + 18];
  const middleCount =
    middleAnimation === 0 || middleAnimation >= 0xff ? 0 : middleAnimation & 0x0f;
  if (middleFrame >= 0) {
    layers.push(
      layer(
        "mid",
        middleIndex,
        middleFrame,
        Math.max(1, middleCount),
        middleCount === 8 || middleCount === 10 || (middleAnimation & 0x80) !== 0,
      ),
    );
  }

  const frontIndex = bytes.readInt16LE(offset + 10);
  const frontFrame = (bytes.readInt16LE(offset + 12) & 0x7fff) - 1;
  const frontAnimation = bytes[offset + 16];
  const frontCount = frontAnimation > 0 ? frontAnimation & 0x7f : 0;
  if (frontIndex >= 0 && frontFrame >= 0) {
    layers.push(
      layer(
        "front",
        frontIndex,
        frontFrame,
        Math.max(1, frontCount),
        (frontAnimation & 0x80) !== 0,
      ),
    );
  }
  return layers;
}

function layer(name, libraryIndex, frameIndex, frameCount, additive) {
  return { name, libraryIndex, frameIndex, frameCount, additive };
}

function screenRectForDraw({ centerX, centerY, x, y, layer, asset, standalone }) {
  const cellLeft = 470 + (x - centerX) * 48;
  const cellTop = 352 + (y - centerY) * 32;
  const width = asset.width;
  const height = asset.height;
  if (standalone) {
    const sourceOffset = asset.placementMode === "source-offset";
    return {
      left: cellLeft + (sourceOffset ? asset.offsetX ?? 0 : 0),
      top: cellTop + 32 - height + (sourceOffset ? asset.offsetY ?? 0 : 0),
      width,
      height,
    };
  }

  const floorSized =
    (width === 48 && height === 32) || (width === 96 && height === 64);
  const drawAsFloor = layer.name === "back" || (layer.frameCount === 1 && floorSized);
  return {
    left: drawAsFloor ? cellLeft : cellLeft + (48 - width) / 2,
    top: drawAsFloor ? cellTop : cellTop + 32 - height,
    width,
    height,
  };
}

function pointInRect(point, rect) {
  return (
    point.x >= rect.left &&
    point.x < rect.left + rect.width &&
    point.y >= rect.top &&
    point.y < rect.top + rect.height
  );
}

function parsePoint(value) {
  const [x, y] = String(value).split(",").map(Number);
  if (!Number.isFinite(x) || !Number.isFinite(y)) {
    throw new Error(`invalid screen point: ${value}`);
  }
  return { x, y };
}

function numberArg(value, fallback) {
  if (value === undefined) return fallback;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) throw new Error(`invalid numeric argument: ${value}`);
  return parsed;
}

function libraryKeyForIndex(index) {
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
