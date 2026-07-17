import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

import { decodeFrameRgba, parseLibrary } from "./crystal-library.mjs";

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
const mapImageResidencyPath = new URL("../lib/bevy-map-image-residency.ts", import.meta.url);
const sceneCachePath = new URL("../lib/scene-blueprint-cache.ts", import.meta.url);
const packagedRegionPath = new URL("../lib/generated/crystal_starter_map_region.json", import.meta.url);
const mapExporterPath = new URL("./export-crystal-starter-map.mjs", import.meta.url);
const shellPath = new URL("../app/original-client-shell.tsx", import.meta.url);
const pagePath = new URL("../app/page.tsx", import.meta.url);
const bevyRuntimePath = new URL("../../game-client/runtime/src/lib.rs", import.meta.url);

const { mapAtlasPathRequiresAlphaKey } = loadTypeScriptModule(atlasPath);
const {
  crystalFrontMapBlendMode,
  crystalMiddleMapBlendMode,
  decodeCrystalFrontAnimationCount,
  decodeCrystalMiddleAnimationCount,
} = loadTypeScriptModule(blendPath);
const {
  isCompleteBevyMapImageFamilyResident,
  reconcileBevyMapImageResidency,
  shouldUploadBevyMapImage,
} = loadTypeScriptModule(mapImageResidencyPath);

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

const uploadedMapImages = new Set(["standalone-additive:frame-a", "standalone-additive:frame-b"]);
const residencyAfterFrameB = reconcileBevyMapImageResidency(
  uploadedMapImages,
  new Set(uploadedMapImages),
  ["standalone-additive:frame-a", "standalone-additive:frame-b"],
);
assert.deepEqual(
  Array.from(uploadedMapImages),
  ["standalone-additive:frame-a", "standalone-additive:frame-b"],
  "a map ACK must preserve every image retained by a visible animation family",
);
assert.deepEqual(Array.from(residencyAfterFrameB.presented), [
  "standalone-additive:frame-a",
  "standalone-additive:frame-b",
]);
assert.equal(residencyAfterFrameB.presentedChanged, false);
assert.equal(
  shouldUploadBevyMapImage(uploadedMapImages, "standalone-additive:frame-a"),
  false,
  "a recurring animation frame must not upload again while its family is visible",
);
assert.equal(
  isCompleteBevyMapImageFamilyResident(
    new Set(["standalone-additive:frame-a"]),
    ["standalone-additive:frame-a", "standalone-additive:frame-b"],
  ),
  false,
  "DOM ownership must remain while an animation family is only partly resident",
);
assert.equal(
  isCompleteBevyMapImageFamilyResident(
    new Set(["standalone-additive:frame-a", "standalone-additive:frame-b"]),
    ["standalone-additive:frame-a", "standalone-additive:frame-b"],
  ),
  true,
  "DOM ownership may transfer only after the complete animation family is resident",
);

const animatedFamily = Array.from(
  { length: 10 },
  (_, index) => `standalone-additive:Objects/${2723 + index}`,
);
const uploadedAnimatedFamily = new Set(animatedFamily);
let presentedAnimatedFamily = new Set(animatedFamily);
for (let phase = 0; phase < 20; phase += 1) {
  const currentFrame = animatedFamily[phase % animatedFamily.length];
  const residency = reconcileBevyMapImageResidency(
    uploadedAnimatedFamily,
    presentedAnimatedFamily,
    animatedFamily,
  );
  assert.deepEqual(
    residency.releasedUploadKeys,
    [],
    `phase ${phase} must not evict a visible family frame`,
  );
  assert.equal(
    shouldUploadBevyMapImage(uploadedAnimatedFamily, currentFrame),
    false,
    `phase ${phase} must reuse its retained upload`,
  );
  presentedAnimatedFamily = residency.presented;
}

const mapRenderingSource = readFileSync(mapRenderingPath, "utf8");
assert.match(
  mapRenderingSource,
  /resolvedMapSpriteBlendMode\(sprite\) \|\| mapAtlasPathRequiresAlphaKey\(sprite\.path\)/,
  "black-keyed or additive map cells must bypass the raw packed atlas",
);

const crystalObjectsLibPath = fileURLToPath(
  new URL("../../../../Crystal/Build/Client/Debug/Data/Map/WemadeMir2/Objects.Lib", import.meta.url),
);
const crystalObjectsLibrary = existsSync(crystalObjectsLibPath)
  ? parseLibrary(readFileSync(crystalObjectsLibPath))
  : null;
for (let frameIndex = 2723; frameIndex <= 2732; frameIndex += 1) {
  const pngPath = fileURLToPath(
    new URL(`../public/original-map/WemadeMir2/Objects/${frameIndex}.png`, import.meta.url),
  );
  const decodedPng = await sharp(readFileSync(pngPath))
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });
  const alphaValues = new Set();
  for (let offset = 3; offset < decodedPng.data.length; offset += 4) {
    alphaValues.add(decodedPng.data[offset]);
  }
  assert.deepEqual(
    [...alphaValues].sort((left, right) => left - right),
    [0, 255],
    `original additive frame ${frameIndex} must preserve Crystal's binary source alpha`,
  );
  if (crystalObjectsLibrary) {
    const nativeFrame = crystalObjectsLibrary.frames[frameIndex];
    assert.ok(nativeFrame, `Crystal Objects.Lib must contain frame ${frameIndex}`);
    assert.deepEqual(
      decodedPng.data,
      decodeFrameRgba(crystalObjectsLibrary, nativeFrame),
      `original additive frame ${frameIndex} must be byte-identical to Objects.Lib RGBA`,
    );
  }
}
assert.match(
  mapRenderingSource,
  /const alphaKeyMapObject = !additive && mapAtlasPathRequiresAlphaKey\(fetchUrl\);/,
  "standalone black-keying must follow the asset path for floor-sized object frames too",
);
assert.match(
  mapRenderingSource,
  /additive && sprite\.animationFramePaths\?\.length/,
  "visible additive animations must advertise their complete frame family",
);
assert.match(
  mapRenderingSource,
  /requiredImageKeysByTileKey/,
  "standalone draw tiles must carry an atomic image-family requirement",
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
assert.doesNotMatch(
  mapLoaderSource,
  /postProcessFrameRgba/,
  "runtime original-map export must not rewrite native additive alpha",
);
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
assert.doesNotMatch(
  mapExporterSource,
  /postProcessFrameRgba/,
  "offline original-map export must preserve native RGBA bytes",
);
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

const shellSource = readFileSync(shellPath, "utf8");
assert.doesNotMatch(
  shellSource,
  /decodeMapAtlasPagePixels|decodedMapAtlasPagesRef|getImageData\([^)]*map-atlas/,
  "packed map pages must not cross Canvas RGBA readback into WASM",
);
assert.match(shellSource, /packedPageTransport: "bevy-asset-server-url"/);
assert.match(shellSource, /ackKey = `g\$\{bevyMapRuntimeGeneration\}:r\$\{revision\}`/);
assert.match(
  shellSource,
  /requiredImageKeys\.every\(imageReady\)/,
  "DOM-to-Bevy handoff must wait for the whole visible animation family",
);
assert.match(
  shellSource,
  /retainedImageKeys,/,
  "the Bevy map transaction must retain non-current family frames",
);

const pageSource = readFileSync(pagePath, "utf8");
assert.match(
  pageSource,
  /status\.ackKey !== pendingBevyMapAckKeyRef\.current/,
  "stale map ACKs must not transfer renderer ownership",
);
assert.match(pageSource, /reconcileBevyMapImageResidency\(/);
assert.match(
  pageSource,
  /state\.retainedImageKeys \?\? \[\]/,
  "the runtime upload/ACK boundary must include retained animation images",
);

const bevyRuntimeSource = readFileSync(bevyRuntimePath, "utf8");
assert.match(bevyRuntimeSource, /asset_server\.load\(asset_path\)/);
assert.match(bevyRuntimeSource, /is_loaded_with_dependencies\(image\.id\(\)\)/);
assert.match(bevyRuntimeSource, /publish_map_status\([\s\S]*"map-render-synced"/);
assert.match(bevyRuntimeSource, /layouts[\s\S]*\.retain\(\|key, _\| active_atlas_keys\.contains\(key\)\)/);
assert.match(bevyRuntimeSource, /retained_image_keys/);

console.log("map render routing tests passed");
