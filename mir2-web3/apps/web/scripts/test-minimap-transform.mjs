import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

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

const helperUrl = new URL("../lib/crystal-minimap-transform.ts", import.meta.url);
const helperExports = loadTypeScriptModule(helperUrl);
const generatedExports = loadTypeScriptModule(
  new URL("../lib/generated/crystal-minimap-transforms.ts", import.meta.url),
  { "../crystal-minimap-transform": helperExports },
);
const mapInputExports = loadTypeScriptModule(new URL("../lib/client-map-input.ts", import.meta.url));

const {
  createLinearMiniMapTransform,
  crystalMiniMapRadarColor,
  findCrystalMiniMapTransform,
  normalizeCrystalMiniMapFileName,
  worldToCrystalMiniMapRadarPoint,
  worldToMiniMapImagePoint,
} = helperExports;
const { CRYSTAL_MINI_MAP_TRANSFORMS } = generatedExports;
const {
  buildMapRouteEdges,
  mapRouteSourceMatchesWorld,
  nativeBigMapImagePointToTile,
  nativeBigMapImageRect,
  nativeMiniMapCrop,
  nativeMiniMapViewportPointToTile,
} = mapInputExports;

const closeTo = (actual, expected, tolerance, label) => {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${label}: expected ${actual} to be within ${tolerance} of ${expected}`,
  );
};

{
  assert.equal(normalizeCrystalMiniMapFileName("0"), "0");
  assert.equal(normalizeCrystalMiniMapFileName("0.map"), "0");
  assert.equal(normalizeCrystalMiniMapFileName("Map/0.MAP"), "0");
  assert.equal(normalizeCrystalMiniMapFileName("WemadeMir2\\Map\\BICHON.MAP"), "bichon");

  const transform = findCrystalMiniMapTransform(CRYSTAL_MINI_MAP_TRANSFORMS, {
    mapFileName: "WemadeMir2\\Map\\0.MAP",
    miniMapIndex: 101,
    bigMapIndex: 101,
    kind: "mini",
  });
  assert.ok(transform, "Bichon mini map transform should exist");
  const player = worldToMiniMapImagePoint(transform, { x: 347, y: 285 });
  // Crystal's minimap is a LINEAR projection (MainDialogs.DrawMiniMap:
  // scaleX = image.Width/map.Width): 347 * 1052/700 = 521.49, 285 * 700/700 = 285.
  assert.ok(player.x > 515 && player.x < 528, "Bichon 347,285 should land in the linear image x range");
  assert.ok(player.y > 280 && player.y < 290, "Bichon 347,285 should land in the linear image y range");

  const stableGirlMary = worldToMiniMapImagePoint(transform, { x: 353, y: 278 });
  assert.ok(stableGirlMary.x > player.x, "StableGirl Mary should be slightly east/right of 347,285 on MMap 101");
  closeTo(stableGirlMary.y, player.y, 8, "StableGirl Mary should sit on the same Bichon town minimap band");
}

{
  const transforms = [
    {
      mapFileName: "0",
      miniMapIndex: 101,
      bigMapIndex: 201,
      worldMinX: 0,
      worldMinY: 0,
      worldMaxX: 100,
      worldMaxY: 100,
      imageMinX: 0,
      imageMinY: 0,
      imageMaxX: 100,
      imageMaxY: 100,
    },
    {
      mapFileName: "0",
      miniMapIndex: 102,
      bigMapIndex: 202,
      worldMinX: 0,
      worldMinY: 0,
      worldMaxX: 100,
      worldMaxY: 100,
      imageMinX: 0,
      imageMinY: 0,
      imageMaxX: 100,
      imageMaxY: 100,
    },
  ];
  assert.equal(
    findCrystalMiniMapTransform(transforms, {
      mapFileName: "0",
      miniMapIndex: 101,
      bigMapIndex: 202,
      kind: "mini",
    }),
    transforms[0],
    "mini lookup must use miniMapIndex",
  );
  assert.equal(
    findCrystalMiniMapTransform(transforms, {
      mapFileName: "0",
      miniMapIndex: 101,
      bigMapIndex: 202,
      kind: "big",
    }),
    transforms[1],
    "big lookup must use bigMapIndex",
  );
}

{
  const linear = createLinearMiniMapTransform({
    mapFileName: "0",
    miniMapIndex: 101,
    bigMapIndex: 101,
    worldWidth: 700,
    worldHeight: 700,
    imageWidth: 1052,
    imageHeight: 700,
  });
  const point = worldToMiniMapImagePoint(linear, { x: 347, y: 285 });
  closeTo(point.x, 347 * (1052 / 700), 0.000001, "linear fallback x should match the legacy ratio");
  closeTo(point.y, 285, 0.000001, "linear fallback y should match the legacy ratio");
}

{
  const miniTransform = findCrystalMiniMapTransform(CRYSTAL_MINI_MAP_TRANSFORMS, {
    mapFileName: "0",
    miniMapIndex: 101,
    bigMapIndex: 101,
    kind: "mini",
  });
  assert.ok(miniTransform, "Bichon mini transform should exist for map 0");

  const player = { x: 330, y: 270 };
  const playerImagePoint = worldToMiniMapImagePoint(miniTransform, player);
  closeTo(playerImagePoint.x, 495.9428571428572, 0.0001, "map 0 player image x should be 330*1052/700");
  closeTo(playerImagePoint.y, 270, 0.0001, "map 0 player image y should be 270 (linear, scaleY=1)");

  const miniCrop = nativeMiniMapCrop(player.x, player.y, 700, 700, 1052, 700);
  assert.ok(miniCrop);
  const miniViewportPoint = {
    x: (Math.fround(Math.fround(player.x) * Math.fround(1052)) / Math.fround(700) - miniCrop.left) * 120 / miniCrop.width,
    y: (Math.fround(Math.fround(player.y) * Math.fround(700)) / Math.fround(700) - miniCrop.top) * 108 / miniCrop.height,
  };
  closeTo(miniViewportPoint.x, 60, 0.02, "Native mini viewport projects from its floating-point source crop");
  closeTo(miniViewportPoint.y, 54, 0.0001, "Native mini viewport centers the player vertically");

  const bigTransform = findCrystalMiniMapTransform(CRYSTAL_MINI_MAP_TRANSFORMS, {
    mapFileName: "0",
    miniMapIndex: 101,
    bigMapIndex: 101,
    kind: "big",
  });
  assert.ok(bigTransform, "Bichon big transform should exist for map 0");
  assert.notEqual(
    miniTransform,
    bigTransform,
    "map 0 mini and big transform entries should be intentionally separated",
  );
  closeTo(worldToMiniMapImagePoint(bigTransform, player).x, playerImagePoint.x, 0.0001, "big transform should land on the same image X for map 0");
  closeTo(worldToMiniMapImagePoint(bigTransform, player).y, playerImagePoint.y, 0.0001, "big transform should land on the same image Y for map 0");

  const bigRect = nativeBigMapImageRect(1052, 700);
  const bigViewportPoint = {
    x: bigRect.left + (player.x / 700) * bigRect.width,
    y: bigRect.top + (player.y / 700) * bigRect.height,
  };
  closeTo(bigViewportPoint.x, 281.77142857142854, 0.0001, "Native big map projects player position into its clipped image rectangle");
  closeTo(bigViewportPoint.y, 198.57142857142856, 0.0001, "Native big map keeps the independently clipped Y axis");
}

{
  const transform = findCrystalMiniMapTransform(CRYSTAL_MINI_MAP_TRANSFORMS, {
    mapFileName: "0",
    miniMapIndex: 101,
    bigMapIndex: 101,
    kind: "mini",
  });
  assert.ok(transform, "Bichon mini transform should exist for native radar projection");
  const point = worldToCrystalMiniMapRadarPoint(
    transform,
    { imageLeft: 438, imageTop: 221 },
    { x: 332, y: 275 },
  );
  closeTo(point.x, 61.61714285714285, 0.0001, "native radar X should re-project from world start 291");
  closeTo(point.y, 54, 0.0001, "native radar Y should re-project from world start 221");
  assert.equal(Math.floor(point.x - 0.5) + 3, 64, "player radar rectangle should start at panel X 64");
  assert.equal(Math.floor(point.y - 0.5) + 22, 75, "player radar rectangle should start at panel Y 75");

  assert.equal(crystalMiniMapRadarColor({ kind: "selfPlayer" }), "#ffffff");
  assert.equal(crystalMiniMapRadarColor({ kind: "npc" }), "#00ff32");
  assert.equal(crystalMiniMapRadarColor({ kind: "monster", ai: 6 }), "#00ff32");
  assert.equal(crystalMiniMapRadarColor({ kind: "monster", ai: 57 }), "#ff0000");
  assert.equal(
    crystalMiniMapRadarColor({ kind: "monster", ai: 57, ownedByPlayer: true }),
    "#0000ff",
  );
}

{
  assert.deepEqual(nativeBigMapImageRect(300, 199), { left: 148, top: 142, width: 300, height: 199 });
  assert.deepEqual(nativeBigMapImageRect(700, 700), { left: 14, top: 52, width: 568, height: 380 });
  assert.deepEqual(nativeBigMapImagePointToTile(0, 0, 700, 700, 700, 700), { x: 0, y: 0 });
  assert.deepEqual(nativeBigMapImagePointToTile(567.99, 379.99, 700, 700, 700, 700), { x: 699, y: 699 });
  assert.equal(nativeBigMapImagePointToTile(568, 10, 700, 700, 700, 700), null,
    "BigMap image hit testing is half-open at the right edge");
  assert.equal(nativeBigMapImagePointToTile(-0.01, 10, 700, 700, 700, 700), null);

  const crop = nativeMiniMapCrop(330, 270, 700, 700, 1052, 700);
  assert.ok(crop);
  closeTo(crop.left, (330 * 1052 / 700) - 60, 0.01, "mini source crop centers on the source image point");
  assert.equal(crop.top, 216);
  assert.equal(crop.width, 120);
  assert.equal(crop.height, 108);
  assert.deepEqual(nativeMiniMapViewportPointToTile(0, 0, crop, 700, 700, 1052, 700), { x: 290, y: 216 });
  assert.equal(nativeMiniMapViewportPointToTile(120, 54, crop, 700, 700, 1052, 700), null,
    "MiniMap viewport hit testing is half-open at the right edge");

  const source = { owner: {}, mapIndex: 4, mapFileName: "0.map", playerObjectId: "self",
    mapWidth: 700, mapHeight: 700, miniMapIndex: 101, bigMapIndex: 201 };
  const world = { mapFileName: "Map/0.MAP", miniMapIndex: 101, bigMapIndex: 201,
    originalMapRegion: { mapWidth: 700, mapHeight: 700 } };
  assert.equal(mapRouteSourceMatchesWorld(source, world, "self", "mini"), true);
  assert.equal(mapRouteSourceMatchesWorld({ ...source, owner: {} }, world, "self", "big"), true,
    "new wrapper objects remain eligible only when Page retains the opaque owner token");
  assert.equal(mapRouteSourceMatchesWorld({ ...source, mapWidth: 701 }, world, "self", "mini"), false);
  assert.equal(mapRouteSourceMatchesWorld(source, { ...world, miniMapIndex: 102 }, "self", "mini"), false);
  assert.equal(mapRouteSourceMatchesWorld(source, world, "other", "mini"), false);
}

await (async () => {
  const fingerprint = "a".repeat(64);
  const blockedBits = new Uint8Array(1);
  blockedBits[0] = 1 << 2; // Quest cache is column-major: (x=1,y=0) => 1*height+0.
  const edges = await buildMapRouteEdges({ mapFileName: "fixture.map", width: 3, height: 2,
    fingerprint, blockedBits }, [], [], () => true);
  assert.ok(edges, "valid collision geometry should create a complete edge mask");
  assert.equal(edges[0] & (1 << 2), 0, "blocked eastward destination must clear the source edge");
  assert.equal(edges[2] & (1 << 6), 0, "blocked destination must clear its reciprocal incoming edge");
  assert.notEqual(edges[0] & (1 << 3), 0, "unblocked diagonal destinations remain available");

  const occupied = await buildMapRouteEdges({ mapFileName: "fixture.map", width: 3, height: 2,
    fingerprint, blockedBits: new Uint8Array(1) }, [{ x: 1, y: 0 }], [], () => true);
  assert.equal(occupied[0] & (1 << 2), 0, "occupied tiles clear incoming player edges");
  const rejected = await buildMapRouteEdges({ mapFileName: "fixture.map", width: 3, height: 2,
    fingerprint, blockedBits: new Uint8Array(1) }, [], [{ from: { x: 0, y: 1 }, to: { x: 1, y: 1 } }], () => true);
  assert.equal(rejected[3] & (1 << 2), 0, "a rejected directed step clears only that source direction");
  assert.equal(await buildMapRouteEdges({ mapFileName: "fixture.map", width: 3, height: 2,
    fingerprint, blockedBits: new Uint8Array(1) }, [], [], () => false), null,
  "stale action leases reject before producing a mask");
  assert.equal(await buildMapRouteEdges({ mapFileName: "fixture.map", width: 4097, height: 4097,
    fingerprint, blockedBits: new Uint8Array(1) }, [], [], () => true), null,
  "route masks respect the existing 16M-cell ceiling");
  let currentChecks = 0;
  const staleDuringYield = await buildMapRouteEdges({ mapFileName: "fixture.map", width: 300, height: 300,
    fingerprint, blockedBits: new Uint8Array(Math.ceil(300 * 300 / 8)) }, [], [], () => ++currentChecks < 3);
  assert.equal(staleDuringYield, null, "an action lease invalidated at a chunk yield cannot return a partial mask");
})();

console.log("minimap transform tests passed");
