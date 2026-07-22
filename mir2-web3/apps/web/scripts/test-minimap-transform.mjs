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

const {
  createLinearMiniMapTransform,
  crystalMiniMapRadarColor,
  findCrystalMiniMapTransform,
  normalizeCrystalMiniMapFileName,
  worldToCrystalMiniMapRadarPoint,
  worldToMiniMapImagePoint,
} = helperExports;
const { CRYSTAL_MINI_MAP_TRANSFORMS } = generatedExports;

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

  const miniViewWidth = 120;
  const miniViewHeight = 108;
  const miniRasterLeft = Math.max(Math.min(Math.trunc(playerImagePoint.x) - Math.floor(miniViewWidth / 2), 1052 - miniViewWidth), 0);
  const miniRasterTop = Math.max(Math.min(Math.trunc(playerImagePoint.y) - Math.floor(miniViewHeight / 2), 700 - miniViewHeight), 0);
  const miniViewportPoint = {
    x: playerImagePoint.x - miniRasterLeft,
    y: playerImagePoint.y - miniRasterTop,
  };
  closeTo(miniViewportPoint.x, 60.94285714285718, 0.0001, "map 0 mini viewport x should use Crystal's truncated source rectangle");
  closeTo(miniViewportPoint.y, 54, 0.0001, "map 0 mini viewport y should stay centered on player");

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

  const maxViewportWidth = 568;
  const maxViewportHeight = 380;
  const contentScale = Math.min(maxViewportWidth / 1052, maxViewportHeight / 700);
  const bigContentWidth = Math.max(1, Math.round(1052 * contentScale));
  const bigContentHeight = Math.max(1, Math.round(700 * contentScale));
  const bigViewport = {
    left: 14 + Math.round((maxViewportWidth - bigContentWidth) / 2),
    top: 52 + Math.round((maxViewportHeight - bigContentHeight) / 2),
    scale: contentScale,
  };
  const bigViewportPoint = {
    x: bigViewport.left + playerImagePoint.x * bigViewport.scale,
    y: bigViewport.top + playerImagePoint.y * bigViewport.scale,
  };
  closeTo(bigViewportPoint.x, 281.77142857142854, 0.0001, "map 0 big viewport x");
  closeTo(bigViewportPoint.y, 198.77946768060835, 0.0001, "map 0 big viewport y");

  closeTo((bigViewportPoint.x - bigViewport.left) / bigViewport.scale, playerImagePoint.x, 0.0001, "big viewport X should decode back to image space");
  closeTo((bigViewportPoint.y - bigViewport.top) / bigViewport.scale, playerImagePoint.y, 0.0001, "big viewport Y should decode back to image space");
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

console.log("minimap transform tests passed");
