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
  findCrystalMiniMapTransform,
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
  const transform = findCrystalMiniMapTransform(CRYSTAL_MINI_MAP_TRANSFORMS, {
    mapFileName: "0",
    miniMapIndex: 101,
    bigMapIndex: 101,
    kind: "mini",
  });
  assert.ok(transform, "Bichon mini map transform should exist");
  const player = worldToMiniMapImagePoint(transform, { x: 347, y: 285 });
  assert.ok(player.x > 560 && player.x < 585, "Bichon 347,285 should land in the calibrated image x range");
  assert.ok(player.y > 305 && player.y < 325, "Bichon 347,285 should land in the calibrated image y range");

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

console.log("minimap transform tests passed");
