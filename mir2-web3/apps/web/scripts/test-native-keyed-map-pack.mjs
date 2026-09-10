import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import os from "node:os";
import path from "node:path";
import { existsSync, readFileSync } from "node:fs";
import fs from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

import sharp from "sharp";

import {
  alphaKeyMapObjectPixels,
  assertNativeKeyedMapMissingSourceBudget,
  assertSafeNativeKeyedOutputRoot,
  buildNativeKeyedMapPack,
  collectStandaloneMapReferences,
  crystalFrontMapBlendMode,
  crystalMiddleMapBlendMode,
  decodeCrystalMiddleAnimationCount,
  mapAtlasPathRequiresAlphaKey,
  mapLibraryKeyForIndex,
  NATIVE_KEYED_MAX_MISSING_SOURCES,
  parseType1Map,
  parseType100Map,
  resolveCrystalMapPlacement,
} from "./build-native-keyed-map-pack.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));

{
  assert.doesNotThrow(() =>
    assertNativeKeyedMapMissingSourceBudget({
      missingSourceCount: NATIVE_KEYED_MAX_MISSING_SOURCES,
    }),
  );
  assert.throws(
    () =>
      assertNativeKeyedMapMissingSourceBudget({
        missingSourceCount: NATIVE_KEYED_MAX_MISSING_SOURCES + 1,
      }),
    /source coverage regressed/,
  );
}

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

function makeType1MapBytes(cells, width = cells.length, height = 1) {
  const xor = 0x1357;
  const bytes = Buffer.alloc(54 + cells.length * 15);
  bytes[0] = 0x10;
  bytes[2] = 0x61;
  bytes[7] = 0x31;
  bytes[14] = 0x31;
  bytes.writeInt16LE(width ^ xor, 21);
  bytes.writeInt16LE(xor, 23);
  bytes.writeInt16LE(height ^ xor, 25);
  for (let index = 0; index < cells.length; index += 1) {
    cells[index](bytes, 54 + index * 15, xor);
  }
  return bytes;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
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
        name === "Object1c" || name === "Object2c"
          ? ""
          : wemadeMir3Folders[state];
      const wemadePrefix = wemadeFolder
        ? `WemadeMir3/${wemadeFolder}/`
        : "WemadeMir3/";
      assert.equal(
        mapLibraryKeyForIndex(200 + state * 15 + slot),
        `${wemadePrefix}${name}`,
      );
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
  assert.equal(
    mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir2/Tiles/1.png"),
    false,
  );
  assert.equal(
    mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir2/Objects/1.png"),
    true,
  );
  assert.equal(
    mapAtlasPathRequiresAlphaKey(
      "/original-map/WemadeMir3/Sand/Dungeonsc/99.png",
    ),
    true,
  );
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
    x === 0 || y === 0 || x === 4 || y === 4
      ? [0, 0, 0, 255]
      : [200, 200, 200, 255],
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
  assert.equal(
    refs[0].additive,
    true,
    "additive reference must win during dedupe",
  );
}

{
  const bytes = makeType100MapBytes([
    (target, base) => {
      target.writeInt16LE(2, base + 10);
      target.writeInt16LE(2724, base + 12); // front base frame 2723
      target[base + 16] = 0x8a; // ten-frame additive Crystal lamp flame
    },
  ]);
  const parsed = parseType100Map(bytes);
  assert.ok(parsed);
  const refs = collectStandaloneMapReferences(parsed);
  assert.deepEqual(
    refs.map((reference) => reference.key),
    Array.from(
      { length: 10 },
      (_, phase) => `WemadeMir2/Objects#${2723 + phase}`,
    ),
    "native keyed pack must close every phase in a Crystal animation family",
  );
  assert.ok(refs.every((reference) => reference.additive));
}

{
  const bytes = makeType1MapBytes([
    (target, base, xor) => {
      target.writeInt32LE(0xaa38aa38 | 0, base);
      target.writeInt16LE(xor, base + 4);
      target.writeInt16LE(2 ^ xor, base + 6);
      target[base + 12] = 1; // library index 3 -> WemadeMir2/Objects2
    },
  ]);
  const parsed = parseType1Map(bytes);
  assert.ok(parsed);
  assert.equal(parsed.width, 1);
  assert.equal(parsed.height, 1);
  assert.equal(parsed.cells[0].frontIndex, 3);
  assert.equal(parsed.cells[0].frontImage, 2);
  assert.equal(
    collectStandaloneMapReferences(parsed)[0].key,
    "WemadeMir2/Objects2#1",
  );
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
      { key: "WemadeMir3/Tilesc#3", additive: true, layer: "middle" },
      { key: "WemadeMir3/Tilesc#4", additive: true, layer: "middle" },
      { key: "WemadeMir3/Tilesc#5", additive: true, layer: "middle" },
      { key: "WemadeMir3/Tilesc#6", additive: true, layer: "middle" },
      { key: "WemadeMir3/Tilesc#7", additive: true, layer: "middle" },
      { key: "WemadeMir3/Tilesc#8", additive: true, layer: "middle" },
      { key: "WemadeMir3/Tilesc#9", additive: true, layer: "middle" },
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
    assertSafeNativeKeyedOutputRoot(
      path.join(os.tmpdir(), "plain-temp-output"),
    );
  } catch (error) {
    unsafeError = error;
  }
  assert.ok(unsafeError instanceof Error);
}

{
  const tempRoot = await fs.mkdtemp(
    path.join(os.tmpdir(), "native-keyed-map-"),
  );
  const packagedMapRoot = path.join(tempRoot, "packaged");
  const originalMapRoot = path.join(tempRoot, "original-map");
  const outputRoot = path.join(tempRoot, "native-keyed-map-output");
  const starterMapRegionPath = path.join(
    tempRoot,
    "crystal_starter_map_region.json",
  );
  await fs.mkdir(packagedMapRoot, { recursive: true });
  await fs.mkdir(path.join(originalMapRoot, "WemadeMir2", "Objects"), {
    recursive: true,
  });
  await fs.mkdir(path.join(originalMapRoot, "WemadeMir2", "Objects27"), {
    recursive: true,
  });
  const mapBytes = makeType100MapBytes([
    (target, base) => {
      target.writeInt16LE(2, base + 6);
      target.writeInt16LE(2, base + 8); // frame 1 -> authoritative Crystal RGBA
    },
    (target, base) => {
      target.writeInt16LE(2, base + 10);
      target.writeInt16LE(2724, base + 12); // frame 2723 -> additive legacy offset
      target[base + 16] = 0x81;
    },
  ]);
  await fs.writeFile(
    path.join(packagedMapRoot, "0.map.gz"),
    gzipSync(mapBytes),
  );
  // Crystal exports ordinary map objects with authoritative alpha. In particular,
  // dark opaque art is allowed to touch a narrow frame's edge; treating that art
  // as a second black-key background is the regression that made buildings pale.
  const keyedSource = await sharp(
    Buffer.from([
      8, 8, 8, 255,
      0, 0, 0, 0,
      32, 24, 16, 255,
      200, 180, 120, 255,
    ]),
    { raw: { width: 2, height: 2, channels: 4 } },
  )
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
  await fs.writeFile(
    path.join(originalMapRoot, "WemadeMir2", "Objects", "1.png"),
    keyedSource,
  );
  await fs.writeFile(
    path.join(originalMapRoot, "WemadeMir2", "Objects", "2723.png"),
    additiveSource,
  );
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
  const keyedEntry = manifest.entries.find(
    (entry) => entry.key === "WemadeMir2/Objects#1",
  );
  const additiveEntry = manifest.entries.find(
    (entry) => entry.key === "WemadeMir2/Objects#2723",
  );
  assert.ok(keyedEntry);
  assert.ok(additiveEntry);
  assert.equal(additiveEntry.placementMode, "source-offset");
  assert.equal(additiveEntry.offsetX, -51);
  assert.equal(additiveEntry.offsetY, -113);
  const keyedPagePath = path.join(
    outputRoot,
    "pages",
    path.basename(keyedEntry.imageUrl),
  );
  const additivePagePath = path.join(
    outputRoot,
    "pages",
    path.basename(additiveEntry.imageUrl),
  );
  assert.ok(
    existsSync(keyedPagePath),
    "keyed page must be emitted in custom output root",
  );
  assert.ok(
    existsSync(additivePagePath),
    "additive page must be emitted in custom output root",
  );
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
    readFileSync(keyedPagePath),
    keyedSource,
    "normal staged PNG must preserve authoritative Crystal RGBA byte-for-byte",
  );
  assert.deepEqual(
    readFileSync(additivePagePath),
    additiveSource,
    "additive staged PNG must stay byte-identical to source",
  );
  const stagedMeta = await sharp(readFileSync(additivePagePath)).metadata();
  assert.equal(
    stagedMeta.hasAlpha,
    true,
    "additive staged PNG must preserve source alpha",
  );
}

{
  const tempRoot = await fs.mkdtemp(
    path.join(os.tmpdir(), "native-keyed-map-"),
  );
  const packagedMapRoot = path.join(tempRoot, "packaged");
  const originalMapRoot = path.join(tempRoot, "original-map");
  const fullPackRoot = path.join(tempRoot, "full");
  const outputRoot = path.join(tempRoot, "native-keyed-map-full-pack-output");
  const productionAssetConfigPath = path.join(
    tempRoot,
    "production-web-assets.json",
  );
  const starterMapRegionPath = path.join(
    tempRoot,
    "crystal_starter_map_region.json",
  );
  await fs.mkdir(packagedMapRoot, { recursive: true });
  await fs.mkdir(originalMapRoot, { recursive: true });
  const mapBytes = makeType1MapBytes([
    (target, base, xor) => {
      target.writeInt32LE(0xaa38aa38 | 0, base);
      target.writeInt16LE(xor, base + 4);
      target.writeInt16LE(2 ^ xor, base + 6);
      target[base + 12] = 1;
    },
  ]);
  await fs.writeFile(
    path.join(packagedMapRoot, "0141.map.gz"),
    gzipSync(mapBytes),
  );
  await fs.writeFile(starterMapRegionPath, JSON.stringify({ sprites: {} }));

  const pageBytes = await sharp({
    create: {
      width: 4,
      height: 4,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite([
      {
        input: await sharp({
          create: {
            width: 2,
            height: 2,
            channels: 4,
            background: { r: 25, g: 120, b: 240, alpha: 1 },
          },
        })
          .png()
          .toBuffer(),
        left: 1,
        top: 1,
      },
    ])
    .png()
    .toBuffer();
  const pageHash = sha256(pageBytes);
  const pageUrl = `/generated/crystal-packs/full/pages/${pageHash.slice(0, 2)}/${pageHash}.png`;
  const libraryManifest = {
    libraryKey: "Map/WemadeMir2/Objects2",
    frames: [
      { index: 0, noDraw: true, status: "no-draw" },
      {
        index: 1,
        status: "packed",
        noDraw: false,
        x: 7,
        y: -44,
        image: {
          imageUrl: pageUrl,
          pageKey: `sha256:${pageHash}`,
          x: 1,
          y: 1,
          width: 2,
          height: 2,
        },
      },
    ],
  };
  const libraryBytes = Buffer.from(`${JSON.stringify(libraryManifest)}\n`);
  const libraryHash = sha256(libraryBytes);
  const libraryUrl =
    "/generated/crystal-packs/full/libraries/maps/objects2.json";
  const contentHash = "a".repeat(64);
  const indexBytes = Buffer.from(
    `${JSON.stringify({
      contentHash,
      libraries: [
        {
          libraryKey: "Map/WemadeMir2/Objects2",
          manifestUrl: libraryUrl,
          manifestSha256: libraryHash,
        },
      ],
    })}\n`,
  );
  await fs.mkdir(path.join(fullPackRoot, "libraries", "maps"), {
    recursive: true,
  });
  await fs.mkdir(path.join(fullPackRoot, "pages", pageHash.slice(0, 2)), {
    recursive: true,
  });
  await fs.writeFile(path.join(fullPackRoot, "index.json"), indexBytes);
  await fs.writeFile(
    path.join(fullPackRoot, "libraries", "maps", "objects2.json"),
    libraryBytes,
  );
  await fs.writeFile(
    path.join(fullPackRoot, "pages", pageHash.slice(0, 2), `${pageHash}.png`),
    pageBytes,
  );
  await fs.writeFile(
    productionAssetConfigPath,
    JSON.stringify({
      assetBaseUrl: "https://assets.invalid/release",
      fullCrystalPack: {
        path: "/generated/crystal-packs/full/index.json",
        contentHash,
      },
    }),
  );

  const result = await buildNativeKeyedMapPack({
    mapFileNames: ["0141"],
    packagedMapRoot,
    originalMapRoot,
    fullPackRoot,
    outputRoot,
    starterMapRegionPath,
    productionAssetConfigPath,
    fullPackFallbackMapFileNames: ["0141"],
    maxMissingSources: 0,
  });
  assert.equal(result.fullPackEntryCount, 1);
  assert.equal(result.missingSourceCount, 0);
  const manifest = JSON.parse(
    await fs.readFile(path.join(outputRoot, "manifest.json"), "utf8"),
  );
  assert.deepEqual(manifest.mapFileNames, ["0141"]);
  assert.equal(manifest.entries[0].key, "WemadeMir2/Objects2#1");
  assert.equal(manifest.entries[0].placementMode, "source-offset");
  assert.equal(manifest.entries[0].offsetX, 7);
  assert.equal(manifest.entries[0].offsetY, -44);
  const extracted = await sharp(
    path.join(outputRoot, "pages", path.basename(manifest.entries[0].imageUrl)),
  )
    .raw()
    .toBuffer({ resolveWithObject: true });
  assert.equal(extracted.info.width, 2);
  assert.equal(extracted.info.height, 2);
  assert.deepEqual([...extracted.data.subarray(0, 4)], [25, 120, 240, 255]);
}

{
  const tempRoot = await fs.mkdtemp(
    path.join(os.tmpdir(), "native-keyed-map-"),
  );
  const packagedMapRoot = path.join(tempRoot, "packaged");
  const originalMapRoot = path.join(tempRoot, "original-map");
  const outputRoot = path.join(tempRoot, "native-keyed-map-budget-output");
  const starterMapRegionPath = path.join(
    tempRoot,
    "crystal_starter_map_region.json",
  );
  await fs.mkdir(packagedMapRoot, { recursive: true });
  await fs.mkdir(outputRoot, { recursive: true });
  const mapBytes = makeType100MapBytes([
    (target, base) => {
      target.writeInt16LE(2, base + 6);
      target.writeInt16LE(2, base + 8);
    },
  ]);
  await fs.writeFile(
    path.join(packagedMapRoot, "0.map.gz"),
    gzipSync(mapBytes),
  );
  await fs.writeFile(starterMapRegionPath, JSON.stringify({ sprites: {} }));
  const sentinelManifest = '{"sentinel":true}\n';
  await fs.writeFile(path.join(outputRoot, "manifest.json"), sentinelManifest);

  await assert.rejects(
    buildNativeKeyedMapPack({
      mapFileName: "0",
      packagedMapRoot,
      originalMapRoot,
      outputRoot,
      starterMapRegionPath,
      maxMissingSources: 0,
    }),
    /source coverage regressed/,
  );
  assert.equal(
    await fs.readFile(path.join(outputRoot, "manifest.json"), "utf8"),
    sentinelManifest,
    "budget rejection must leave the previous generated output untouched",
  );
}

console.log("native keyed map pack tests passed");
