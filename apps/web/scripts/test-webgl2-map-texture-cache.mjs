import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTsModule(url) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const mod = { exports: {} };
  const require = (specifier) => {
    if (specifier === "./render-tier") return {};
    throw new Error(`Unexpected require(${JSON.stringify(specifier)})`);
  };
  new Function("exports", "module", "require", compiled.outputText)(
    mod.exports,
    mod,
    require,
  );
  return mod.exports;
}

const {
  estimateRgba8TextureBytes,
  mapTextureResidencyBytes,
  planMapTextureEvictions,
  resolveMapTextureByteBudget,
} = loadTsModule(
  new URL("../lib/webgl2-map-texture-cache.ts", import.meta.url),
);
const componentSource = readFileSync(
  new URL("../app/components/webgl2-map-atlas-layer.tsx", import.meta.url),
  "utf8",
);

const MiB = 1024 * 1024;
const record = (key, mib, lastUsedAt) => ({
  key,
  byteSize: mib * MiB,
  lastUsedAt,
});

assert.equal(estimateRgba8TextureBytes(1024, 4096), 16 * MiB);
assert.equal(estimateRgba8TextureBytes(-1, 4096), 0);
assert.equal(
  mapTextureResidencyBytes([record("a", 16, 1), record("b", 32, 2)]),
  48 * MiB,
);

assert.equal(
  resolveMapTextureByteBudget({
    tier: "low",
    deviceMemoryGiB: 2,
    maxTextureSize: 4096,
  }),
  64 * MiB,
);
assert.equal(
  resolveMapTextureByteBudget({
    tier: "low",
    deviceMemoryGiB: 4,
    maxTextureSize: 4096,
  }),
  96 * MiB,
);
assert.equal(
  resolveMapTextureByteBudget({
    tier: "medium",
    deviceMemoryGiB: 6,
    maxTextureSize: 4096,
  }),
  144 * MiB,
);
assert.equal(
  resolveMapTextureByteBudget({
    tier: "medium",
    deviceMemoryGiB: 6,
    maxTextureSize: 8192,
  }),
  160 * MiB,
);
assert.equal(
  resolveMapTextureByteBudget({
    tier: "high",
    deviceMemoryGiB: 12,
    maxTextureSize: 8192,
  }),
  256 * MiB,
);

const underBudget = planMapTextureEvictions(
  [record("old", 32, 1), record("visible", 32, 2)],
  new Set(["visible"]),
  96 * MiB,
);
assert.deepEqual(underBudget.evictKeys, []);
assert.equal(underBudget.bytesAfter, 64 * MiB);

const lru = planMapTextureEvictions(
  [
    record("b", 32, 1),
    record("a", 32, 1),
    record("recent", 32, 3),
    record("visible", 32, 4),
  ],
  new Set(["visible"]),
  96 * MiB,
);
assert.deepEqual(lru.evictKeys, ["a", "b"]);
assert.equal(lru.bytesBefore, 128 * MiB);
assert.equal(lru.bytesAfter, 64 * MiB);
assert.equal(lru.pinnedBytes, 32 * MiB);

const pinsCanExceedBudget = planMapTextureEvictions(
  [
    record("old", 16, 1),
    record("visible-a", 64, 2),
    record("visible-b", 64, 3),
  ],
  new Set(["visible-a", "visible-b"]),
  96 * MiB,
);
assert.deepEqual(pinsCanExceedBudget.evictKeys, ["old"]);
assert.equal(pinsCanExceedBudget.bytesAfter, 128 * MiB);
assert.equal(pinsCanExceedBudget.pinnedBytes, 128 * MiB);

const noPins = planMapTextureEvictions(
  [record("old", 64, 1), record("new", 64, 2)],
  new Set(),
  96 * MiB,
);
assert.deepEqual(noPins.evictKeys, ["old"]);
assert.equal(noPins.bytesAfter, 64 * MiB);

assert.match(
  componentSource,
  /if \(!enabled\) \{[\s\S]{0,240}releaseTextureCache\(gl, texturesRef\.current\)/,
  "Bevy ownership must release the disabled WebGL2 map texture cache",
);
const loadBoundary = componentSource.indexOf("const loadedTextures = await Promise.all");
const clearAfterLoad = componentSource.indexOf("gl.clearColor(0, 0, 0, 0)", loadBoundary);
assert.ok(loadBoundary >= 0 && clearAfterLoad > loadBoundary, "new atlas pages must load before clearing the previous frame");

console.log("webgl2 map texture cache tests passed");
