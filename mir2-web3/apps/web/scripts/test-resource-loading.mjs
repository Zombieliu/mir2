import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import ts from "typescript";

const nodeRequire = createRequire(import.meta.url);

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

const loaderExports = loadTypeScriptModule(new URL("../lib/crystal-map-loader.ts", import.meta.url), {
  "server-only": {},
});
const sceneCacheExports = loadTypeScriptModule(new URL("../lib/scene-blueprint-cache.ts", import.meta.url), {
  "./crystal-map-loader": { loadCrystalSceneBlueprint: async () => ({}) },
  "./scene-types": {},
});

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
  assert.match(a, /2026-05-27-v3-0-cx\d+-cy\d+-w40-h40-/);
}

console.log("resource loading tests passed");
