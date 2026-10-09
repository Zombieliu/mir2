import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import os from "node:os";
import path from "node:path";
import { existsSync, readFileSync } from "node:fs";
import fs from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

import sharp from "sharp";
import ts from "typescript";

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
  // Call the actual collision/parser functions with one bounded in-memory map
  // scope. No real map, private configuration, HTTP listener or resource loader
  // is available to this scope; these fixtures remain in this existing suite.
  const sourcePath = new URL("../lib/crystal-map-loader.ts", import.meta.url);
  const source = readFileSync(sourcePath, "utf8");
  const ast = ts.createSourceFile(String(sourcePath), source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  const declarations = new Map();
  for (const statement of ast.statements) {
    if ((ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement)) && statement.name) {
      declarations.set(statement.name.text, statement.getText(ast));
    } else if (ts.isVariableStatement(statement)) {
      for (const declaration of statement.declarationList.declarations) {
        if (ts.isIdentifier(declaration.name)) declarations.set(declaration.name.text, statement.getText(ast));
      }
    }
  }
  const names = [
    "QuestCollisionError", "MAX_QUEST_COLLISION_CELLS", "completeQuestCollisionMaps",
    "questMapFileName", "completeQuestCollisionMap", "strictQuestGeometry", "loadCrystalQuestCollisionRegion",
    "loadCrystalCollisionRegion", "parsedCellAt", "parsedCellBlocksMovement", "normalizeMapFileName", "clampInt",
    "parseMapBytes", "parseMapGeometry", "parseType100Map", "detectMapType", "detectMapWidth", "detectMapHeight",
  ];
  for (const name of names) assert.ok(declarations.has(name), `actual collision declaration ${name}`);
  const module = { exports: {} };
  const fixtureMaps = new Map();
  let fixtureManifest = { maps: [] };
  let fixtureMapReads = 0;
  let fixtureManifestReads = 0;
  const compiled = ts.transpileModule(names.map((name) => declarations.get(name)).join("\n"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const actual = new Function("exports", "module", "createHash", "readFileSync", "RESPAWN_MANIFEST_PATH", "loadParsedMap",
    `${compiled}\nreturn {loadCrystalQuestCollisionRegion,loadCrystalCollisionRegion,QuestCollisionError,parseMapBytes};`)(
    module.exports, module, createHash,
    (requested) => {
      assert.equal(requested, "fixture-manifest", "only the in-memory public manifest may be read");
      fixtureManifestReads++;
      return Buffer.from(JSON.stringify(fixtureManifest));
    }, "fixture-manifest",
    (file) => {
      fixtureMapReads++;
      assert.ok(fixtureMaps.has(file), `only an in-memory map may be loaded: ${file}`);
      return fixtureMaps.get(file);
    },
  );
  const sourceBytes = makeType100MapBytes(Array.from({ length: 6 }, () => (bytes, offset) => bytes.writeInt16LE(-32768, offset + 12)));
  const landingBytes = makeType100MapBytes([
    () => {}, () => {}, (bytes, offset) => bytes.writeInt16LE(-32768, offset + 12),
  ]);
  const parsedSource = actual.parseMapBytes("source.map", sourceBytes);
  fixtureMaps.set("source", parsedSource);
  fixtureMaps.set("landing", actual.parseMapBytes("landing.map", landingBytes));
  fixtureMaps.set("0", actual.parseMapBytes("0.map", sourceBytes));
  const movement = (x, destinationX, overrides = {}) => ({
    map_index: 2, source: { x, y: 0 }, destination: { x: destinationX, y: 0 },
    need_hole: false, need_move: false, conquest_index: 0, ...overrides,
  });
  fixtureManifest = { maps: [
    { map_index: 1, map_file_name: "SOURCE", movements: [
      movement(0, 1), movement(1, 1, { need_hole: true }), movement(2, 1, { need_move: true }),
      movement(3, 1, { conquest_index: 1 }), movement(4, 2), movement(5, 0),
    ] },
    { map_index: 2, map_file_name: "LANDING", movements: [] },
  ] };
  const region = { mapFileName: "source", minX: 0, maxX: 5, minY: 0, maxY: 0 };

  // strict_full_map_and_ordinary_entrance_keep_conditional_invalid_landings_blocked
  const initial = actual.loadCrystalQuestCollisionRegion(region);
  assert.equal(initial.schemaVersion, 1);
  assert.equal(initial.source, "crystalMap");
  assert.equal(initial.mapFileName, "source");
  assert.equal(initial.mapWidth, 6);
  assert.equal(initial.mapHeight, 1);
  assert.match(initial.geometryFingerprint, /^[0-9a-f]{64}$/);
  assert.equal(parsedSource.geometrySourceFingerprint, sha256(sourceBytes));
  assert.deepEqual(initial.blockedCells, [1, 2, 3, 4, 5].map((x) => ({ x, y: 0 })),
    "ordinary entrance is exempt; needHole/needMove/conquest/blocked landing/(0,0) are not");

  // strict_chunks_share_the_same_complete_geometry_fingerprint
  const first = actual.loadCrystalQuestCollisionRegion({ ...region, maxX: 2 });
  const last = actual.loadCrystalQuestCollisionRegion({ ...region, minX: 3 });
  assert.equal(first.geometryFingerprint, initial.geometryFingerprint);
  assert.equal(last.geometryFingerprint, initial.geometryFingerprint);
  assert.deepEqual([...first.blockedCells, ...last.blockedCells], initial.blockedCells);
  assert.deepEqual(actual.loadCrystalQuestCollisionRegion({ ...region, maxX: 255 }).bounds,
    { minX: 0, maxX: 5, minY: 0, maxY: 0 });

  // strict_geometry_rejects_synthetic_partial_and_sparse_maps
  const synthetic = { fileName: "synthetic", width: 6, height: 1, type: -1, cells: null,
    syntheticResourcePath: "fixture-unavailable.map" };
  const partial = actual.parseMapBytes("partial.map", sourceBytes.subarray(0, 8 + 26));
  const starter = { fileName: "starter", width: 6, height: 1, type: -1, cells: null,
    fallbackOriginalMapRegion: { regionBounds: { minX: 0, maxX: 1, minY: 0, maxY: 0 },
      cells: [{ x: 0, y: 0, blocked: true }] } };
  const sparse = { ...parsedSource, fileName: "sparse.map", cells: Array(6) };
  fixtureMaps.set("synthetic", synthetic);
  fixtureMaps.set("partial", partial);
  fixtureMaps.set("starter", starter);
  fixtureMaps.set("sparse", sparse);
  for (const file of ["synthetic", "partial", "starter", "sparse"]) {
    assert.throws(() => actual.loadCrystalQuestCollisionRegion({ ...region, mapFileName: file }),
      (error) => error instanceof actual.QuestCollisionError && error.code === "collisionUnavailable", file);
  }

  // strict_geometry_rejects_paths_devices_bad_bounds_and_oversized_chunks
  for (const file of ["../source", "source/other", "SOURCE", " source", "con", "nul.map", "source."]) {
    assert.throws(() => actual.loadCrystalQuestCollisionRegion({ ...region, mapFileName: file }),
      (error) => error instanceof actual.QuestCollisionError && error.code === "invalidInput", file);
  }
  for (const patch of [{ minX: -1 }, { minX: 6 }, { maxX: 256 }, { maxX: 0.5 }, { maxX: -1 }]) {
    assert.throws(() => actual.loadCrystalQuestCollisionRegion({ ...region, ...patch }),
      (error) => error instanceof actual.QuestCollisionError && error.code === "invalidInput");
  }

  // strict_fingerprint_changes_when_real_source_bytes_or_manifest_change
  const changedSourceBytes = Buffer.from(sourceBytes);
  changedSourceBytes[8 + 25] = 1; // Different real bytes; static walls intentionally unchanged.
  fixtureMaps.set("source", actual.parseMapBytes("source.map", changedSourceBytes));
  const changedSource = actual.loadCrystalQuestCollisionRegion(region);
  assert.notEqual(changedSource.geometryFingerprint, initial.geometryFingerprint);
  assert.deepEqual(changedSource.blockedCells, initial.blockedCells);
  fixtureManifest.maps[0].movements[1].need_hole = false;
  const changedManifest = actual.loadCrystalQuestCollisionRegion(region);
  assert.notEqual(changedManifest.geometryFingerprint, changedSource.geometryFingerprint);
  assert.deepEqual(changedManifest.blockedCells, [2, 3, 4, 5].map((x) => ({ x, y: 0 })));

  // strict_cached_source_observes_changed_landing_bytes_and_collision
  const sameSource = fixtureMaps.get("source");
  const changedLandingBytes = Buffer.from(landingBytes);
  changedLandingBytes.writeInt16LE(-32768, 8 + 26 + 12); // Ordinary landing is now a wall.
  fixtureMaps.set("landing", actual.parseMapBytes("landing.map", changedLandingBytes));
  const changedLanding = actual.loadCrystalQuestCollisionRegion(region);
  assert.equal(fixtureMaps.get("source"), sameSource, "source map remains cached");
  assert.notEqual(changedLanding.geometryFingerprint, changedManifest.geometryFingerprint);
  assert.deepEqual(changedLanding.blockedCells, [0, 1, 2, 3, 4, 5].map((x) => ({ x, y: 0 })));

  // Invoke the production GET function through a JSON response stub, never HTTP.
  const routePath = new URL("../app/api/scene/collision/route.ts", import.meta.url);
  const routeModule = { exports: {} };
  const routeCode = ts.transpileModule(readFileSync(routePath, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("exports", "module", "require", routeCode)(routeModule.exports, routeModule, (id) => {
    if (id === "next/server") return { NextResponse: { json: (body, options = {}) => ({
      body, status: options.status ?? 200, headers: options.headers ?? {},
    }) } };
    assert.equal(id, "../../../../lib/crystal-map-loader");
    return actual;
  });
  const completeParams = new URLSearchParams({ quest: "1", map: "source", minX: "0", maxX: "5", minY: "0", maxY: "0" });
  const strictResponse = await routeModule.exports.GET({ url: `https://fixture.invalid/api/scene/collision?${completeParams}` });
  assert.equal(strictResponse.status, 200);
  assert.deepEqual(strictResponse.body, changedLanding);

  // strict_api_requires_map_and_all_four_explicit_bounds_before_loading
  for (const key of ["map", "minX", "maxX", "minY", "maxY"]) {
    const missing = new URLSearchParams(completeParams);
    missing.delete(key);
    const beforeReads = [fixtureMapReads, fixtureManifestReads];
    const result = await routeModule.exports.GET({ url: `https://fixture.invalid/api/scene/collision?${missing}` });
    assert.equal(result.status, 400, `strict query missing ${key}`);
    assert.deepEqual(result.body, { schemaVersion: 1, error: "invalidInput" });
    assert.equal(result.headers["Cache-Control"], "no-store");
    assert.deepEqual([fixtureMapReads, fixtureManifestReads], beforeReads,
      `missing ${key} must be rejected before map or manifest access`);
  }
  for (const value of ["", "NaN", "1.5", "-1", "0oops"]) {
    const invalid = new URLSearchParams(completeParams);
    invalid.set("minX", value);
    const result = await routeModule.exports.GET({ url: `https://fixture.invalid/api/scene/collision?${invalid}` });
    assert.equal(result.status, 400, `invalid strict numeric bound ${JSON.stringify(value)}`);
    assert.deepEqual(result.body, { schemaVersion: 1, error: "invalidInput" });
  }
  const unavailableParams = new URLSearchParams(completeParams);
  unavailableParams.set("map", "synthetic");
  const unavailable = await routeModule.exports.GET({ url: `https://fixture.invalid/api/scene/collision?${unavailableParams}` });
  assert.equal(unavailable.status, 424);
  assert.deepEqual(unavailable.body, { schemaVersion: 1, error: "collisionUnavailable" });

  // legacy_api_without_quest_retains_clamping_raw_walls_and_default_map
  const legacy = await routeModule.exports.GET({ url: "https://fixture.invalid/api/scene/collision?map=source&minX=0&maxX=999&minY=0&maxY=0" });
  assert.equal(legacy.status, 200);
  assert.equal(legacy.body.schemaVersion, undefined);
  assert.equal(legacy.body.mapFileName, "source.map");
  assert.deepEqual(legacy.body.bounds, { minX: 0, maxX: 5, minY: 0, maxY: 0 });
  assert.deepEqual(legacy.body.blockedCells, [0, 1, 2, 3, 4, 5].map((x) => ({ x, y: 0 })));
  const defaultLegacy = await routeModule.exports.GET({ url: "https://fixture.invalid/api/scene/collision" });
  assert.equal(defaultLegacy.body.mapFileName, "0.map");
  assert.deepEqual(defaultLegacy.body.bounds, { minX: 0, maxX: 0, minY: 0, maxY: 0 });
  assert.deepEqual(defaultLegacy.body.blockedCells, [{ x: 0, y: 0 }]);
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
  for (const index of [210,225,240,255,270,310,325,340,355,370]) {
    const library = mapLibraryKeyForIndex(index);
    assert.equal(mapAtlasPathRequiresAlphaKey(`/original-map/${library}/2766.png`), true, library);
    const map = { width:1, height:1, cells:[{backIndex:-1,backImage:0,
      middleIndex:index,middleImage:2767,middleAnimationFrame:0,
      frontIndex:-1,frontImage:0,frontAnimationFrame:0}] };
    assert.ok(collectStandaloneMapReferences(map).some(r => r.key === `${library}#2766`));
  }
  for (const name of ["SmObjectsbad", "SmObjectsc2", "SmObjectscwoods"]) {
    assert.equal(mapAtlasPathRequiresAlphaKey(`/original-map/WemadeMir3/${name}/1.png`),false);
  }
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
