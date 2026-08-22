import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { existsSync, readFileSync } from "node:fs";
import fs from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

import sharp from "sharp";

import {
  alphaKeyMapObjectPixels,
  assertSafeNativeKeyedOutputRoot,
  buildNativeKeyedMapPack,
  collectStandaloneMapReferences,
  crystalFrontMapBlendMode,
  crystalMiddleMapBlendMode,
  decodeCrystalMiddleAnimationCount,
  mapAtlasPathRequiresAlphaKey,
  mapLibraryKeyForIndex,
  parseType100Map,
  resolveCrystalMapPlacement,
} from "./build-native-keyed-map-pack.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));

function makePixels(width, height, at) {
  const pixels = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const [r, g, b, a] = at(x, y);
      const offset = (y * width + x) * 4;
      pixels[offset] = r;
      pixels[offset + 1] = g;
      pixels[offset + 2] = b;
      pixels[offset + 3] = a;
    }
  }
  return pixels;
}

function makeType100MapBytes(cells) {
  const bytes = Buffer.alloc(8 + cells.length * 26);
  bytes[2] = 0x43;
  bytes[3] = 0x23;
  bytes.writeUInt16LE(cells.length, 4);
  bytes.writeUInt16LE(1, 6);
  for (let index = 0; index < cells.length; index += 1) {
    cells[index](bytes, 8 + index * 26);
  }
  return bytes;
}

{
  const mir3Names = [
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
  const wemadeMir3Folders = ["", "Wood", "Sand", "Snow", "Forest"];
  const shandaMir3Suffixes = ["", "wood", "sand", "snow", "forest"];
  for (let state = 0; state < 5; state += 1) {
    for (let slot = 0; slot < mir3Names.length; slot += 1) {
      const name = mir3Names[slot];
      const wemadeFolder =
        name === "Object1c" || name === "Object2c" ? "" : wemadeMir3Folders[state];
      const wemadePrefix = wemadeFolder ? `WemadeMir3/${wemadeFolder}/` : "WemadeMir3/";
      assert.equal(mapLibraryKeyForIndex(200 + state * 15 + slot), `${wemadePrefix}${name}`);
      assert.equal(
        mapLibraryKeyForIndex(300 + state * 15 + slot),
        `ShandaMir3/${name}${shandaMir3Suffixes[state]}`,
      );
    }
  }
  assert.equal(mapLibraryKeyForIndex(214), "WemadeMir2/Tiles");
  assert.equal(mapLibraryKeyForIndex(299), "WemadeMir2/Tiles");
  assert.equal(mapLibraryKeyForIndex(373), "ShandaMir3/Object2cforest");
  assert.equal(mapLibraryKeyForIndex(374), "WemadeMir2/Tiles");
  assert.equal(mapLibraryKeyForIndex(375), "WemadeMir2/Tiles");

  assert.equal(mapLibraryKeyForIndex(0), "WemadeMir2/Tiles");
  assert.equal(mapLibraryKeyForIndex(2), "WemadeMir2/Objects");
  assert.equal(mapLibraryKeyForIndex(5), "WemadeMir2/Objects4");
  assert.equal(mapLibraryKeyForIndex(120), "ShandaMir2/Objects");
}

{
  assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir2/Tiles/1.png"), false);
  assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir2/Objects/1.png"), true);
  assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir3/Sand/Dungeonsc/99.png"), true);
}

{
  assert.equal(decodeCrystalMiddleAnimationCount(0), 0);
  assert.equal(decodeCrystalMiddleAnimationCount(8), 8);
  assert.equal(decodeCrystalMiddleAnimationCount(0x88), 8);
  assert.equal(decodeCrystalMiddleAnimationCount(0xff), 0);
  assert.equal(crystalMiddleMapBlendMode(8), "additive");
  assert.equal(crystalMiddleMapBlendMode(10), "additive");
  assert.equal(crystalMiddleMapBlendMode(0x88), "additive");
  assert.equal(crystalMiddleMapBlendMode(2), "normal");
  assert.equal(crystalFrontMapBlendMode(0x81), "additive");
  assert.equal(crystalFrontMapBlendMode(1), "normal");
}

{
  const pixels = makePixels(5, 5, (x, y) =>
    x === 0 || y === 0 || x === 4 || y === 4 ? [0, 0, 0, 255] : [200, 200, 200, 255],
  );
  const changed = alphaKeyMapObjectPixels(pixels, 5, 5);
  assert.ok(changed > 0);
  assert.equal(pixels[3], 0);
  assert.equal(pixels[(2 * 5 + 2) * 4 + 3], 255);
}

{
  const bytes = makeType100MapBytes([
    (target, base) => {
      target.writeInt16LE(2, base + 6);
      target.writeInt16LE(2, base + 8); // middle frame 1
    },
    (target, base) => {
      target.writeInt16LE(2, base + 10);
      target.writeInt16LE(2, base + 12); // front frame 1
      target[base + 16] = 0x81;
    },
  ]);
  const parsed = parseType100Map(bytes);
  assert.ok(parsed);
  const refs = collectStandaloneMapReferences(parsed);
  assert.equal(refs.length, 1, "same frame should dedupe");
  assert.equal(refs[0].key, "WemadeMir2/Objects#1");
  assert.equal(refs[0].additive, true, "additive reference must win during dedupe");
}

{
  const bytes = makeType100MapBytes([
    (target, base) => {
      target.writeInt16LE(206, base);
      target.writeInt32LE(2, base + 2); // WemadeMir3/Dungeonsc frame 1 -> alpha-key
    },
    (target, base) => {
      target.writeInt16LE(200, base + 6);
      target.writeInt16LE(3, base + 8); // WemadeMir3/Tilesc frame 2 -> additive
      target[base + 18] = 8;
    },
    (target, base) => {
      target.writeInt16LE(212, base + 10);
      target.writeInt16LE(4, base + 12); // WemadeMir3/Object1c frame 3 -> additive
      target[base + 16] = 0x81;
    },
  ]);
  const parsed = parseType100Map(bytes);
  assert.ok(parsed);
  const refs = collectStandaloneMapReferences(parsed);
  assert.deepEqual(
    refs.map(({ key, additive, layer }) => ({ key, additive, layer })),
    [
      { key: "WemadeMir3/Dungeonsc#1", additive: false, layer: "back" },
      { key: "WemadeMir3/Object1c#3", additive: true, layer: "front" },
      { key: "WemadeMir3/Tilesc#2", additive: true, layer: "middle" },
    ],
  );
}

{
  const additivePlacement = resolveCrystalMapPlacement(
    {
      libraryKey: "WemadeMir2/Objects",
      sourcePath: "/original-map/WemadeMir2/Objects/2723.png",
    },
    new Map([
      [
        "/original-map/WemadeMir2/Objects/2723.png",
        { offsetX: -51, offsetY: -113 },
      ],
    ]),
  );
  assert.deepEqual(additivePlacement, {
    placementMode: "source-offset",
    offsetX: -51,
    offsetY: -113,
  });
  const objects27Placement = resolveCrystalMapPlacement(
    {
      libraryKey: "WemadeMir2/Objects27",
      sourcePath: "/original-map/WemadeMir2/Objects27/42.png",
    },
    new Map([
      [
        "/original-map/WemadeMir2/Objects27/42.png",
        { offsetX: 3, offsetY: -9 },
      ],
    ]),
  );
  assert.deepEqual(objects27Placement, {
    placementMode: "source-offset",
    offsetX: 3,
    offsetY: -9,
  });
  assert.equal(
    resolveCrystalMapPlacement(
      {
        libraryKey: "WemadeMir2/Objects",
        sourcePath: "/original-map/WemadeMir2/Objects/102.png",
      },
      new Map([
        [
          "/original-map/WemadeMir2/Objects/102.png",
          { offsetX: 7, offsetY: -44 },
        ],
      ]),
    ),
    null,
  );
}

{
  let unsafeError = null;
  try {
    assertSafeNativeKeyedOutputRoot(path.join(os.tmpdir(), "plain-temp-output"));
  } catch (error) {
    unsafeError = error;
  }
  assert.ok(unsafeError instanceof Error);
}

{
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "native-keyed-map-"));
  const packagedMapRoot = path.join(tempRoot, "packaged");
  const originalMapRoot = path.join(tempRoot, "original-map");
  const outputRoot = path.join(tempRoot, "native-keyed-map-output");
  const starterMapRegionPath = path.join(tempRoot, "crystal_starter_map_region.json");
  await fs.mkdir(packagedMapRoot, { recursive: true });
  await fs.mkdir(path.join(originalMapRoot, "WemadeMir2", "Objects"), { recursive: true });
  await fs.mkdir(path.join(originalMapRoot, "WemadeMir2", "Objects27"), { recursive: true });
  const mapBytes = makeType100MapBytes([
    (target, base) => {
      target.writeInt16LE(2, base + 6);
      target.writeInt16LE(2, base + 8); // frame 1 -> alpha-key
    },
    (target, base) => {
      target.writeInt16LE(2, base + 10);
      target.writeInt16LE(2724, base + 12); // frame 2723 -> additive legacy offset
      target[base + 16] = 0x81;
    },
  ]);
  await fs.writeFile(path.join(packagedMapRoot, "0.map.gz"), gzipSync(mapBytes));
  const keyedSource = await sharp({
    create: {
      width: 2,
      height: 2,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 1 },
    },
  })
    .png()
    .toBuffer();
  const additiveSource = await sharp({
    create: {
      width: 3,
      height: 2,
      channels: 4,
      background: { r: 15, g: 120, b: 240, alpha: 0.5 },
    },
  })
    .png()
    .toBuffer();
  await fs.writeFile(path.join(originalMapRoot, "WemadeMir2", "Objects", "1.png"), keyedSource);
  await fs.writeFile(path.join(originalMapRoot, "WemadeMir2", "Objects", "2723.png"), additiveSource);
  await fs.writeFile(
    starterMapRegionPath,
    JSON.stringify({
      sprites: {
        legacyTorch: {
          frames: [
            {
              path: "/original-map/WemadeMir2/Objects/2723.png",
              offsetX: -51,
              offsetY: -113,
            },
          ],
        },
      },
    }),
  );

  const result = await buildNativeKeyedMapPack({
    mapFileName: "0",
    packagedMapRoot,
    originalMapRoot,
    outputRoot,
    starterMapRegionPath,
  });
  assert.equal(result.referenceCount, 2);
  assert.equal(result.keyedEntryCount, 1);
  assert.equal(result.additiveEntryCount, 1);
  assert.equal(result.missingSourceCount, 0);

  const manifest = JSON.parse(
    await fs.readFile(path.join(outputRoot, "manifest.json"), "utf8"),
  );
  const keyedEntry = manifest.entries.find((entry) => entry.key === "WemadeMir2/Objects#1");
  const additiveEntry = manifest.entries.find(
    (entry) => entry.key === "WemadeMir2/Objects#2723",
  );
  assert.ok(keyedEntry);
  assert.ok(additiveEntry);
  assert.equal(additiveEntry.placementMode, "source-offset");
  assert.equal(additiveEntry.offsetX, -51);
  assert.equal(additiveEntry.offsetY, -113);
  const keyedPagePath = path.join(outputRoot, "pages", path.basename(keyedEntry.imageUrl));
  const additivePagePath = path.join(outputRoot, "pages", path.basename(additiveEntry.imageUrl));
  assert.ok(existsSync(keyedPagePath), "keyed page must be emitted in custom output root");
  assert.ok(existsSync(additivePagePath), "additive page must be emitted in custom output root");
  assert.equal(
    existsSync(
      path.resolve(
        SCRIPT_DIR,
        "..",
        "public",
        "generated",
        "native-map-keyed",
        "pages",
        path.basename(additiveEntry.imageUrl),
      ),
    ),
    false,
    "custom output must not leak additive pages into the default generated tree",
  );
  assert.deepEqual(
    readFileSync(additivePagePath),
    additiveSource,
    "additive staged PNG must stay byte-identical to source",
  );
  const stagedMeta = await sharp(readFileSync(additivePagePath)).metadata();
  assert.equal(stagedMeta.hasAlpha, true, "additive staged PNG must preserve source alpha");
}

console.log("native keyed map pack tests passed");
