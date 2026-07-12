import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
let ts;
try {
  ts = require("typescript");
} catch {
  ts = require("../node_modules/.ignored/typescript/lib/typescript.js");
}

function loadTypeScriptModule(url) {
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
  const load = new Function("exports", "module", compiled.outputText);
  load(module.exports, module);
  return module.exports;
}

const atlasPath = new URL("../lib/map-atlas-manifest.ts", import.meta.url);
const blendPath = new URL("../lib/crystal-map-blend.ts", import.meta.url);
const mapRenderingPath = new URL("../app/components/original-client-scene-map-rendering.tsx", import.meta.url);
const mapLoaderPath = new URL("../lib/crystal-map-loader.ts", import.meta.url);
const sceneCachePath = new URL("../lib/scene-blueprint-cache.ts", import.meta.url);
const packagedRegionPath = new URL("../lib/generated/crystal_starter_map_region.json", import.meta.url);
const mapExporterPath = new URL("./export-crystal-starter-map.mjs", import.meta.url);

const { mapAtlasPathRequiresAlphaKey } = loadTypeScriptModule(atlasPath);
const {
  crystalFrontMapBlendMode,
  crystalMiddleMapBlendMode,
  decodeCrystalFrontAnimationCount,
  decodeCrystalMiddleAnimationCount,
} = loadTypeScriptModule(blendPath);

assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir2/Objects21/1704.png"), true);
assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir2/Objects/5174.png"), true);
assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/ShandaMir2/SmObjects/22.png"), true);
assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir2/Tiles/0.png"), false);
assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir2/SmTiles/0.png"), false);
assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir3/Sand/Dungeonsc/1659.png"), true);
assert.equal(mapAtlasPathRequiresAlphaKey("/original-map/WemadeMir3/Sand/Tilesc/1659.png"), false);

assert.equal(crystalFrontMapBlendMode(0x88), "additive");
assert.equal(decodeCrystalFrontAnimationCount(0x88), 8);
assert.equal(crystalFrontMapBlendMode(0x08), "normal");
assert.equal(crystalMiddleMapBlendMode(8), "additive");
assert.equal(crystalMiddleMapBlendMode(10), "additive");
assert.equal(crystalMiddleMapBlendMode(6), "normal");
assert.equal(decodeCrystalMiddleAnimationCount(255), 0);

const mapRenderingSource = readFileSync(mapRenderingPath, "utf8");
assert.match(
  mapRenderingSource,
  /resolvedMapSpriteBlendMode\(sprite\) \|\| mapAtlasPathRequiresAlphaKey\(sprite\.path\)/,
  "black-keyed or additive map cells must bypass the raw packed atlas",
);
assert.match(
  mapRenderingSource,
  /const alphaKeyMapObject = !additive && mapAtlasPathRequiresAlphaKey\(fetchUrl\);/,
  "standalone black-keying must follow the asset path for floor-sized object frames too",
);
assert.doesNotMatch(
  mapRenderingSource,
  /alphaKeyMapObject = [^;]*mapObject/,
  "a floor render bucket must not suppress object-library alpha keying",
);
assert.match(
  mapRenderingSource,
  /viewportFloorDepthForCell\(cell\.x, cell\.y, player, FLOOR_LAYER_ORDERS\[sprite\.kind\]\)/,
  "all Crystal floor layers must stay in the floor depth band below objects and entities",
);
assert.doesNotMatch(
  mapRenderingSource,
  /FLOOR_LAYER_Z_STRIDE/,
  "floor layer offsets must not escape into the shared object/entity depth band",
);

const mapLoaderSource = readFileSync(mapLoaderPath, "utf8");
assert.match(
  mapLoaderSource,
  /blendMode: crystalFrontMapBlendMode\(cell\.frontAnimationFrame\)/,
  "front-cell high-bit blending must survive scene export",
);
assert.match(
  mapLoaderSource,
  /blendMode: crystalMiddleMapBlendMode\(cell\.middleAnimationFrame\)/,
  "middle 8/10-frame additive blending must survive scene export",
);
assert.doesNotMatch(
  mapLoaderSource,
  /frontAnimationFrame\s*&=\s*0x0f/,
  "map decoding must not erase the front additive high bit",
);

const mapExporterSource = readFileSync(mapExporterPath, "utf8");
assert.match(mapExporterSource, /blendMode: layer\.blendMode/);
assert.doesNotMatch(
  mapExporterSource,
  /frontAnimationFrame\s*&=\s*0x0f/,
  "offline map export must preserve the front additive high bit",
);

const packagedRegion = JSON.parse(readFileSync(packagedRegionPath, "utf8"));
const packagedSprites = Object.values(packagedRegion.sprites ?? {});
assert.ok(packagedSprites.length > 0, "packaged starter map must contain sprites");
assert.equal(
  packagedSprites.every((sprite) => sprite.blendMode === "normal" || sprite.blendMode === "additive"),
  true,
  "packaged starter sprites must carry explicit per-cell blend metadata",
);
assert.ok(
  packagedSprites.some((sprite) => sprite.blendMode === "additive"),
  "packaged starter map must retain its additive Crystal cell",
);

const sceneCacheSource = readFileSync(sceneCachePath, "utf8");
assert.match(
  sceneCacheSource,
  /SCENE_CACHE_SCHEMA_VERSION = "[^"]*map-blend"/,
  "cached blueprints without per-cell blend metadata must be invalidated",
);

console.log("map render routing tests passed");
