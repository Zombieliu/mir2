import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import ts from "typescript";

const indexExports = transpileModule(new URL("../lib/crystal-full-pack-index.ts", import.meta.url));
const bevyExports = transpileModule(
  new URL("../lib/crystal-full-pack-bevy.ts", import.meta.url),
  (request) => {
    if (request === "./crystal-full-pack-index") return indexExports;
    throw new Error(`Unexpected test require ${request}`);
  },
);

const { buildCrystalFullPackAtlasSnapshot, crystalFullPackFramePath } = bevyExports;

test("maps exported asset URLs back to Crystal library/frame identities", () => {
  assert.deepEqual(crystalFullPackFramePath("/original-ui/AWeapon/00%20L/42.png?rev=1"), {
    libraryKey: "AWeapon/00 L",
    frameIndex: 42,
  });
  assert.deepEqual(crystalFullPackFramePath("https://cdn.test/x/original-effects/Magic2/18.png"), {
    libraryKey: "Magic2",
    frameIndex: 18,
  });
  assert.deepEqual(crystalFullPackFramePath("/original-map/WemadeMir2/Tiles/901.png"), {
    libraryKey: "Map/WemadeMir2/Tiles",
    frameIndex: 901,
  });
  assert.equal(crystalFullPackFramePath("/original-ui/NPC/00/1.mask.png"), null);
});

test("builds a lazy multi-page Bevy snapshot with stable CAS page keys", async () => {
  const pageA = page("a", 256, 128);
  const pageB = page("b", 128, 128);
  const frames = new Map([
    ["NPC/00#1", resolved(pageA, "NPC/00#1", 2, 3, 20, 30)],
    ["Monster/010#2", resolved(pageB, "Monster/010#2", 4, 5, 24, 36)],
  ]);
  const loads = [];
  const runtime = {
    document: { sourceContentHash: "c".repeat(64) },
    loadLibrary: async (libraryKey) => {
      loads.push(libraryKey);
      return {
        resolveFrame(frameIndex) {
          return frames.get(`${libraryKey}#${frameIndex}`) ?? null;
        },
      };
    },
  };
  const sources = [
    source("/original-ui/NPC/00/1.png", 20, 30),
    source("/original-ui/Monster/010/2.png", 24, 36),
  ];
  const snapshot = await buildCrystalFullPackAtlasSnapshot(runtime, sources, "scene-key");
  assert.equal(snapshot.key, "scene-key");
  assert.equal(snapshot.sourceKey, `crystal-full:${"c".repeat(64)}`);
  assert.equal(snapshot.pages.length, 2);
  assert.deepEqual(loads.sort(), ["Monster/010", "NPC/00"]);
  assert.equal(snapshot.pages[0].key, `crystal-full:${"a".repeat(64)}`);
  assert.equal(snapshot.rects[sources[0].key].pageIndex, undefined);
  assert.equal(snapshot.rects[sources[1].key].pageIndex, 1);
});

test("returns null instead of a partial atlas for dimension or frame misses", async () => {
  const assetPage = page("d", 64, 64);
  const runtime = {
    document: { sourceContentHash: "e".repeat(64) },
    loadLibrary: async () => ({ resolveFrame: () => resolved(assetPage, "NPC/00#1", 0, 0, 16, 20) }),
  };
  assert.equal(
    await buildCrystalFullPackAtlasSnapshot(runtime, [source("/original-ui/NPC/00/1.png", 17, 20)], "bad"),
    null,
  );
  assert.equal(
    await buildCrystalFullPackAtlasSnapshot(runtime, [source("/not-a-crystal-export/1.png", 16, 20)], "bad"),
    null,
  );
});

function source(path, width, height) {
  return { key: `${path}|${width}x${height}`, path, width, height };
}

function page(character, width, height) {
  const sha256 = character.repeat(64);
  return {
    key: `sha256:${sha256}`,
    sha256,
    width,
    height,
    imageUrl: `/generated/crystal-packs/full/pages/${character}/${sha256}.png`,
    rects: [],
  };
}

function resolved(assetPage, key, x, y, width, height) {
  const rect = { key, x, y, width, height, sourceKind: "image" };
  return {
    noDraw: false,
    image: { page: assetPage, rect, imageUrl: assetPage.imageUrl },
    mask: null,
  };
}

function transpileModule(url, requireFn = () => {
  throw new Error("Unexpected require in test module");
}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
      skipLibCheck: true,
    },
    fileName: url.pathname,
    reportDiagnostics: true,
  });
  const diagnostics = (compiled.diagnostics ?? []).filter(
    (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
  );
  assert.deepEqual(diagnostics, []);
  const loaded = { exports: {} };
  new Function("exports", "module", "require", compiled.outputText)(loaded.exports, loaded, requireFn);
  return loaded.exports;
}
