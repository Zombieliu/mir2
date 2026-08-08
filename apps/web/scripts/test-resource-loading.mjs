import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import ts from "typescript";

const nodeRequire = createRequire(import.meta.url);
const manifestPath = fileURLToPath(new URL("../public/original-asset-manifest.generated.json", import.meta.url));

function loadTypeScriptModule(url, requireMap = {}) {
  const source = readFileSyncCompat(url);
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      esModuleInterop: true,
      allowSyntheticDefaultImports: true,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    if (specifier.startsWith("node:")) return require(specifier.slice(5));
    return requireModule(specifier);
  };
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, require);
  return module.exports;
}

function requireModule(specifier) {
  return nodeRequire(specifier);
}

function readFileSyncCompat(url) {
  return requireModule("node:fs").readFileSync(url, "utf8");
}

function buildFakeLib(frameCount) {
  const headerLength = 8 + frameCount * 4;
  const chunks = [];
  const offsets = [];
  let cursor = headerLength;

  for (let index = 0; index < frameCount; index += 1) {
    const width = 2 + (index % 3);
    const height = 2 + (index % 2);
    const bgra = Buffer.alloc(width * height * 4, index + 1);
    for (let pixel = 0; pixel < bgra.length; pixel += 4) {
      bgra[pixel + 3] = 255;
    }
    const compressed = gzipSync(bgra);
    const frame = Buffer.alloc(17 + compressed.length);
    frame.writeInt16LE(width, 0);
    frame.writeInt16LE(height, 2);
    frame.writeInt16LE(index, 4);
    frame.writeInt16LE(-index, 6);
    frame.writeInt32LE(compressed.length, 13);
    compressed.copy(frame, 17);
    offsets.push(cursor);
    chunks.push(frame);
    cursor += frame.length;
  }

  const header = Buffer.alloc(headerLength);
  header.writeInt32LE(2, 0);
  header.writeInt32LE(frameCount, 4);
  for (let index = 0; index < offsets.length; index += 1) {
    header.writeInt32LE(offsets[index], 8 + index * 4);
  }
  return Buffer.concat([header, ...chunks]);
}

const minimapHelperExports = loadTypeScriptModule(new URL("../lib/crystal-minimap-transform.ts", import.meta.url));
const mapBlendExports = loadTypeScriptModule(new URL("../lib/crystal-map-blend.ts", import.meta.url));
const minimapTransformExports = loadTypeScriptModule(
  new URL("../lib/generated/crystal-minimap-transforms.ts", import.meta.url),
  { "../crystal-minimap-transform": minimapHelperExports },
);
const loaderExports = loadTypeScriptModule(new URL("../lib/crystal-map-loader.ts", import.meta.url), {
  "server-only": {},
  "./crystal-minimap-transform": minimapHelperExports,
  "./crystal-map-blend": mapBlendExports,
  "./generated/crystal-minimap-transforms": minimapTransformExports,
});
const sceneRequestExports = loadTypeScriptModule(
  new URL("../lib/scene-blueprint-request.ts", import.meta.url),
);
const sceneCacheExports = loadTypeScriptModule(new URL("../lib/scene-blueprint-cache.ts", import.meta.url), {
  "./crystal-map-loader": { loadCrystalSceneBlueprint: async () => ({}) },
  "./scene-blueprint-request": sceneRequestExports,
  "./scene-types": {},
});
const originalAssetManifest = readOriginalAssetManifest();
const originalAssetPaths = new Set(Object.keys(originalAssetManifest.assets ?? {}));
const localOriginalMapPngCount = [...originalAssetPaths].filter((assetPath) =>
  assetPath.startsWith("/original-map/"),
).length;

// The full Bichon map "0" cell data (which references WemadeMir2/Objects23 sprites) only
// exists when the raw Crystal client map or its packaged form is present. In reduced
// environments (e.g. web sandboxes without CRYSTAL_CLIENT_ROOT) map "0" falls back to the
// committed starter region, which is valid but does not include Objects23. Detect that here
// so the strong region assertion still runs wherever the data exists, without failing the
// reduced environments where it cannot.
const packagedMap0Path = fileURLToPath(new URL("../lib/generated/crystal-map-pack/0.map.gz", import.meta.url));
const crystalClientRoot = process.env.CRYSTAL_CLIENT_ROOT;
const crystalClientMap0Path = crystalClientRoot ? path.join(crystalClientRoot, "Map", "0.map") : null;
const fullBichonMapDataAvailable =
  existsSync(packagedMap0Path) || (crystalClientMap0Path ? existsSync(crystalClientMap0Path) : false);

{
  const tempDir = mkdtempSync(path.join(tmpdir(), "mir2-lib-test-"));
  try {
    const libPath = path.join(tempDir, "Large.Lib");
    writeFileSync(libPath, buildFakeLib(64));

    loaderExports.resetCrystalMapLoaderResourceStatsForTests();
    const library = loaderExports.parseLibrary(libPath);
    assert.equal(library.count, 64);
    assert.equal(library.frames.filter(Boolean).length, 0, "parseLibrary must only parse the index");
    assert.equal(loaderExports.getCrystalMapLoaderResourceStats().frameDecodeCount, 0);

    const rgba = loaderExports.decodeLibraryFrameRgba(library, 17);
    assert.ok(Buffer.isBuffer(rgba));
    assert.equal(loaderExports.getCrystalMapLoaderResourceStats().frameDecodeCount, 1);

    loaderExports.decodeLibraryFrameRgba(library, 17);
    assert.equal(loaderExports.getCrystalMapLoaderResourceStats().frameDecodeCount, 1, "second decode should hit LRU cache");

    loaderExports.decodeLibraryFrameRgba(library, 18);
    assert.equal(loaderExports.getCrystalMapLoaderResourceStats().frameDecodeCount, 2);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

{
  assert.equal(loaderExports.crystalMapMiniMapIndexForTests("0"), 101, "Bichon map 0 mini index comes from transform metadata");
  assert.equal(loaderExports.crystalMapBigMapIndexForTests("0"), 101, "Bichon map 0 big map index comes from transform metadata");
  assert.equal(loaderExports.crystalMapMiniMapIndexForTests("Map/0.MAP"), 101);
  assert.equal(loaderExports.crystalMapLibraryKeyForIndexForTests(245), "WemadeMir3/Snow/Tilesc");
  assert.equal(loaderExports.crystalMapLibraryKeyForIndexForTests(227), "WemadeMir3/Object1c");
  assert.equal(loaderExports.crystalMapLibraryKeyForIndexForTests(242), "WemadeMir3/Object1c");
  assert.equal(loaderExports.crystalMapLibraryKeyForIndexForTests(257), "WemadeMir3/Object1c");
  assert.equal(loaderExports.crystalMapLibraryKeyForIndexForTests(258), "WemadeMir3/Object2c");
}

{
  const a = sceneCacheExports.createSceneCacheKeyForTests({
    mapFileName: "0",
    centerX: 347,
    centerY: 285,
    width: 33,
    height: 34,
  });
  const b = sceneCacheExports.createSceneCacheKeyForTests({
    mapFileName: "0.map",
    centerX: 351,
    centerY: 288,
    width: 34,
    height: 34,
  });
  assert.equal(a, b, "same scene chunk and size bucket should share one blueprint cache key");
  assert.match(a, /^2026-08-05-v5-canonical-request-0-cx\d+-cy\d+-w40-h40-default$/);
}

{
  const production404RegressionPaths = [
    "/original-map/WemadeMir2/Objects23/1422.png",
    "/original-map/WemadeMir2/Objects23/1426.png",
    "/original-map/WemadeMir2/Objects23/1427.png",
    "/original-map/WemadeMir2/Objects23/1428.png",
    "/original-ui/Monster/139/14.png",
    "/original-ui/NPC/27/0.png",
    "/original-ui/NPC/83/0.png",
    "/original-ui/NPC/16/0.png",
    "/original-ui/Monster/000/2.png",
  ];
  for (const publicPath of production404RegressionPaths) {
    assert.ok(originalAssetPaths.has(publicPath), `original asset manifest must include ${publicPath}`);
    assert.equal(
      loaderExports.crystalOriginalAssetManifestHasPathForTests(publicPath),
      true,
      `loader must accept manifest-published path ${publicPath}`,
    );
  }
}

{
  const blueprint = await loaderExports.loadCrystalSceneBlueprint({
    mapFileName: "0",
    centerX: 347,
    centerY: 285,
    width: 40,
    height: 36,
  });
  const framePaths = sceneBlueprintOriginalMapFramePaths(blueprint);
  assert.ok(framePaths.length > 0, "Bichon scene blueprint should include original-map frame paths");
  if (fullBichonMapDataAvailable) {
    assert.ok(
      framePaths.some((framePath) => framePath.startsWith("/original-map/WemadeMir2/Objects23/")),
      "Bichon production regression region should include Objects23 frames",
    );
  } else {
    console.warn(
      "resource loading tests: skipping Objects23 region assertion (full Crystal map data absent; starter fallback region in use)",
    );
  }
  for (const framePath of framePaths) {
    assert.ok(originalAssetPaths.has(framePath), `scene blueprint frame missing from manifest: ${framePath}`);
  }
  const sceneMissingAssets = blueprint.originalMapRegion?.missingAssets ?? [];
  if (fullBichonMapDataAvailable && localOriginalMapPngCount < 200_000) {
    assertOnlyRemoteOriginalMapMisses(sceneMissingAssets);
  } else {
    // Graceful degradation: with all referenced assets present the region must report no misses.
    assert.deepEqual(sceneMissingAssets, [], "fully-resolved scene must not report missing assets");
  }
}

{
  const actorMetaFramePaths = originalActorMetaFramePaths();
  assert.ok(actorMetaFramePaths.length > 0, "NPC/Monster meta should expose actor frame paths");
  for (const framePath of actorMetaFramePaths) {
    assert.ok(originalAssetPaths.has(framePath), `actor frame path missing from manifest: ${framePath}`);
  }
}

// Graceful asset degradation: a referenced frame whose PNG is absent locally and not published
// in the manifest must be skipped and recorded (not throw and abort the scene). Strict mode
// (MIR2_STRICT_ASSET_RESOLUTION=1) must instead propagate the CrystalResourceMissingError so
// release gates still catch missing assets.
{
  const tempDir = mkdtempSync(path.join(tmpdir(), "mir2-graceful-test-"));
  try {
    const libPath = path.join(tempDir, "Fake.Lib");
    writeFileSync(libPath, buildFakeLib(16));
    const missingLibraryKey = "ZzMir2GracefulDegradeTest/Fake";

    loaderExports.resetCrystalMapLoaderResourceStatsForTests();
    const gracefulLib = loaderExports.parseLibrary(libPath);
    const graceful = loaderExports.resolveLayerSpriteFramesForTests(missingLibraryKey, gracefulLib, [3, 5]);
    assert.equal(graceful.frames.length, 0, "missing frame PNGs must be skipped, not returned");
    assert.equal(graceful.missingAssets.length, 2, "each distinct missing frame PNG must be recorded once");
    assert.ok(
      graceful.missingAssets.every(
        (asset) => typeof asset.libraryKey === "string" && asset.path.startsWith("/original-map/"),
      ),
      "recorded misses must carry a libraryKey and an original-map public path",
    );
    assert.equal(
      loaderExports.getCrystalMapLoaderResourceStats().recoveredMissingAssetCount,
      2,
      "resource stats must count recovered asset misses",
    );

    const deduped = loaderExports.resolveLayerSpriteFramesForTests(missingLibraryKey, gracefulLib, [3, 3]);
    assert.equal(deduped.missingAssets.length, 1, "a duplicate frame index must dedupe to a single miss");

    process.env.MIR2_STRICT_ASSET_RESOLUTION = "1";
    let strictLoader;
    try {
      strictLoader = loadTypeScriptModule(new URL("../lib/crystal-map-loader.ts", import.meta.url), {
        "server-only": {},
        "./crystal-minimap-transform": minimapHelperExports,
        "./crystal-map-blend": mapBlendExports,
        "./generated/crystal-minimap-transforms": minimapTransformExports,
      });
    } finally {
      delete process.env.MIR2_STRICT_ASSET_RESOLUTION;
    }
    const strictLib = strictLoader.parseLibrary(libPath);
    assert.throws(
      () => strictLoader.resolveLayerSpriteFramesForTests(missingLibraryKey, strictLib, [3]),
      (error) => strictLoader.isCrystalResourceMissingError(error),
      "strict mode must propagate the missing-asset error instead of degrading",
    );
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

// A wholly-missing map degrades to a synthetic empty region (recorded in missingAssets) at
// runtime, and still hard-fails under strict resolution so missing maps are caught in CI.
{
  const missingMapName = "ZzMir2NonexistentMapForTest";
  const blueprint = await loaderExports.loadCrystalSceneBlueprint({ mapFileName: missingMapName });
  assert.ok(blueprint.originalMapRegion, "missing map should still yield a (synthetic) region");
  const mapMisses = (blueprint.originalMapRegion.missingAssets ?? []).filter(
    (asset) => asset.resourceType === "map",
  );
  assert.equal(mapMisses.length, 1, "synthetic map must be recorded as a missing asset");

  process.env.MIR2_STRICT_ASSET_RESOLUTION = "1";
  let strictLoader;
  try {
    strictLoader = loadTypeScriptModule(new URL("../lib/crystal-map-loader.ts", import.meta.url), {
      "server-only": {},
      "./crystal-minimap-transform": minimapHelperExports,
      "./crystal-map-blend": mapBlendExports,
      "./generated/crystal-minimap-transforms": minimapTransformExports,
    });
  } finally {
    delete process.env.MIR2_STRICT_ASSET_RESOLUTION;
  }
  await assert.rejects(
    () => strictLoader.loadCrystalSceneBlueprint({ mapFileName: missingMapName }),
    (error) => strictLoader.isCrystalResourceMissingError(error),
    "strict mode must reject a wholly-missing map",
  );
}

console.log("resource loading tests passed");

function readOriginalAssetManifest() {
  assert.ok(
    existsSync(manifestPath),
    `Run npm run generate:original-asset-manifest before resource tests; missing ${manifestPath}`,
  );
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.kind, "mir2-original-asset-manifest");
  assert.ok(manifest.assets && typeof manifest.assets === "object");
  return manifest;
}

function sceneBlueprintOriginalMapFramePaths(blueprint) {
  const paths = [];
  for (const sprite of Object.values(blueprint.originalMapRegion?.sprites ?? {})) {
    for (const frame of sprite?.frames ?? []) {
      if (typeof frame?.path === "string" && frame.path.startsWith("/original-map/")) {
        paths.push(frame.path);
      }
    }
  }
  return [...new Set(paths)].sort((left, right) => left.localeCompare(right));
}

function assertOnlyRemoteOriginalMapMisses(missingAssets) {
  for (const asset of missingAssets) {
    assert.equal(asset.resourceType, "png", "local subset misses must be PNG resources");
    assert.ok(
      typeof asset.path === "string" && asset.path.startsWith("/original-map/"),
      `local subset miss must be an original-map path: ${JSON.stringify(asset)}`,
    );
    assert.ok(!originalAssetPaths.has(asset.path), `reported missing path is unexpectedly local: ${asset.path}`);
    assert.ok(typeof asset.libraryKey === "string" && asset.libraryKey.length > 0, "missing PNG must name libraryKey");
    assert.ok(Number.isInteger(asset.frameIndex), "missing PNG must name frameIndex");
  }
}

function originalActorMetaFramePaths() {
  const roots = [
    fileURLToPath(new URL("../public/original-ui/NPC", import.meta.url)),
    fileURLToPath(new URL("../public/original-ui/Monster", import.meta.url)),
  ];
  const paths = [];
  for (const root of roots) {
    if (!existsSync(root)) continue;
    for (const metaPath of listMetaJsonFiles(root)) {
      const meta = JSON.parse(readFileSync(metaPath, "utf8"));
      for (const frame of meta.frames ?? []) {
        if (typeof frame?.path === "string") {
          paths.push(frame.path);
        }
      }
    }
  }
  return [...new Set(paths)].sort((left, right) => left.localeCompare(right));
}

function listMetaJsonFiles(root) {
  const entries = readdirSync(root, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const entryPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...listMetaJsonFiles(entryPath));
      continue;
    }
    if (entry.isFile() && entry.name === "meta.json" && statSync(entryPath).isFile()) {
      files.push(entryPath);
    }
  }
  return files;
}
