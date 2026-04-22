import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
const clientRoot = process.argv[2] ?? "E:\\mir2\\Crystal\\Build\\Client\\Debug";
const regionPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_starter_map_region.json",
);
const outputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_starter_map_collision.json",
);

main();

function main() {
  const region = JSON.parse(readFileSync(regionPath, "utf8"));
  const mapPath = resolve(clientRoot, "Map", region.mapFileName);
  const bytes = readFileSync(mapPath);

  if (bytes[2] !== 0x43 || bytes[3] !== 0x23) {
    throw new Error(`Unsupported map format for ${mapPath}. Expected Crystal type100 map.`);
  }

  const mapWidth = bytes.readInt16LE(4);
  const mapHeight = bytes.readInt16LE(6);
  const blockedCells = [];
  const doors = [];

  let offset = 8;
  for (let x = 0; x < mapWidth; x += 1) {
    for (let y = 0; y < mapHeight; y += 1) {
      offset += 2;
      const highWall = (bytes.readInt32LE(offset) & 0x20000000) !== 0;
      offset += 10;
      const lowWall = (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      const doorIndex = bytes[offset];
      offset += 11;
      const light = bytes[offset++];

      if (!pointInBounds(region.regionBounds, x, y)) {
        continue;
      }

      if (highWall || lowWall) {
        blockedCells.push({
          x,
          y,
          attribute: lowWall ? "low_wall" : "high_wall",
        });
      }

      if (doorIndex > 0) {
        doors.push({
          x,
          y,
          index: doorIndex & 0x7f,
          closed: true,
        });
      }

      void light;
    }
  }

  const output = {
    mapFileName: region.mapFileName,
    mapWidth,
    mapHeight,
    regionBounds: region.regionBounds,
    playBounds: region.playBounds,
    blockedCells,
    doors,
  };

  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, `${JSON.stringify(output, null, 2)}\n`, "utf8");
  console.log(
    `Wrote ${blockedCells.length} blocked cells and ${doors.length} doors to ${outputPath}`,
  );
}

function pointInBounds(bounds, x, y) {
  return (
    x >= bounds.minX &&
    x <= bounds.maxX &&
    y >= bounds.minY &&
    y <= bounds.maxY
  );
}
