import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourceUrl = new URL("../lib/crystal-full-pack-index.ts", import.meta.url);
const sourcePath = fileURLToPath(sourceUrl);
const source = readFileSync(sourceUrl, "utf8");
const compilerOptions = {
  module: ts.ModuleKind.CommonJS,
  target: ts.ScriptTarget.ES2022,
  strict: true,
  skipLibCheck: true,
};

const compiled = ts.transpileModule(source, { compilerOptions, fileName: sourcePath, reportDiagnostics: true });
const transpileErrors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(transpileErrors, []);

const loadedModule = { exports: {} };
new Function("exports", "module", compiled.outputText)(loadedModule.exports, loadedModule);

const {
  CRYSTAL_FULL_PACK_INDEX_URL,
  loadCrystalFullPackIndex,
  normalizeCrystalFullPackLibraryKey,
  resetCrystalFullPackIndexForTests,
  resolveCrystalFullPackFrame,
  validateCrystalFullPackIndex,
  validateCrystalLibraryPack,
} = loadedModule.exports;

const SOURCE_CONTENT_HASH = "a".repeat(64);
const LIBRARY_SOURCE_HASH = "b".repeat(64);
const SHARD_URL = "/generated/crystal-packs/full/libraries/NPC/00.json";

test.afterEach(() => resetCrystalFullPackIndexForTests());

test("normalizes a library key and resolves a drawable image page plus rect", async () => {
  const { root, shard } = makeFixture();
  const { fetcher, calls } = makeFetcher(root, shard);
  const runtime = await loadCrystalFullPackIndex({ fetcher });

  assert.equal(normalizeCrystalFullPackLibraryKey(" /NPC\\00.Lib/ "), "NPC/00");
  assert.equal(runtime.getLibraryRecord("npc\\00.lib")?.libraryKey, "NPC/00");

  const resolved = await runtime.resolveFrame("NPC\\00.Lib", 0);
  assert.equal(resolved.noDraw, false);
  assert.equal(resolved.image.page.key, "page:image");
  assert.equal(resolved.image.rect.key, "NPC/00#0");
  assert.equal(resolved.image.imageUrl, "/generated/crystal-packs/full/pages/image.png");
  assert.equal(resolved.mask, null);
  assert.deepEqual(calls, [CRYSTAL_FULL_PACK_INDEX_URL, SHARD_URL]);
});

test("preserves explicit noDraw frame slots without resolving a texture", async () => {
  const { root, shard } = makeFixture();
  const { fetcher } = makeFetcher(root, shard);

  const resolved = await resolveCrystalFullPackFrame("NPC/00", 1, { fetcher });
  assert.equal(resolved.noDraw, true);
  assert.equal(resolved.frame.index, 1);
  assert.equal(resolved.image, null);
  assert.equal(resolved.mask, null);
});

test("resolves an image and mask from independent pages", async () => {
  const { root, shard } = makeFixture();
  const { fetcher } = makeFetcher(root, shard);

  const resolved = await resolveCrystalFullPackFrame("NPC/00", 2, { fetcher });
  assert.equal(resolved.noDraw, false);
  assert.equal(resolved.image.page.key, "page:image");
  assert.equal(resolved.image.rect.key, "NPC/00#2");
  assert.equal(resolved.mask.page.key, "page:mask");
  assert.equal(resolved.mask.rect.key, "NPC/00#2:mask");
  assert.equal(resolved.mask.imageUrl, "/generated/crystal-packs/full/pages/mask.png");
});

test("rejects root and shard count corruption", () => {
  const { root, shard } = makeFixture();
  root.summary.noDrawFrameCount += 1;
  assert.throws(() => validateCrystalFullPackIndex(root), /frame counts do not add up|noDrawFrameCount mismatch/);

  const clean = makeFixture();
  clean.shard.summary.maskFrameCount = 0;
  clean.shard.summary.rectCount = 2;
  assert.throws(() => validateCrystalLibraryPack(clean.shard), /maskFrameCount mismatch/);
});

test("rejects source hash and shard URL mismatches against the root record", async () => {
  for (const mutate of [
    (shard) => { shard.sourceSha256 = "c".repeat(64); },
    (shard) => { shard.shardUrl = "/generated/crystal-packs/full/libraries/NPC/01.json"; },
  ]) {
    resetCrystalFullPackIndexForTests();
    const { root, shard } = makeFixture();
    mutate(shard);
    const { fetcher } = makeFetcher(root, shard);
    const runtime = await loadCrystalFullPackIndex({ fetcher });
    await assert.rejects(runtime.loadLibrary("NPC/00"), /sourceSha256 mismatch|shardUrl mismatch/);
  }
});

test("rejects missing cross-references and duplicate rect definitions", () => {
  const brokenReference = makeFixture().shard;
  brokenReference.frames[2].maskRectKey = "NPC/00#missing:mask";
  assert.throws(() => validateCrystalLibraryPack(brokenReference), /references missing rect/);

  const duplicateRect = makeFixture().shard;
  duplicateRect.pages[1].rects[0].key = "NPC/00#0";
  assert.throws(() => validateCrystalLibraryPack(duplicateRect), /Duplicate library rect/);
});

test("caches root and shard fetch promises and reset starts a clean generation", async () => {
  const { root, shard } = makeFixture();
  const firstFetcher = makeFetcher(root, shard);

  const firstRootPromise = loadCrystalFullPackIndex({ fetcher: firstFetcher.fetcher });
  const secondRootPromise = loadCrystalFullPackIndex({ fetcher: firstFetcher.fetcher });
  assert.equal(firstRootPromise, secondRootPromise);
  const runtime = await firstRootPromise;

  const firstShardPromise = runtime.loadLibrary("NPC/00");
  const secondShardPromise = runtime.loadLibrary("npc\\00.lib");
  assert.equal(firstShardPromise, secondShardPromise);
  await Promise.all([firstShardPromise, secondShardPromise]);
  assert.deepEqual(firstFetcher.calls, [CRYSTAL_FULL_PACK_INDEX_URL, SHARD_URL]);

  resetCrystalFullPackIndexForTests();
  const secondFetcher = makeFetcher(root, shard);
  await loadCrystalFullPackIndex({ fetcher: secondFetcher.fetcher });
  assert.deepEqual(secondFetcher.calls, [CRYSTAL_FULL_PACK_INDEX_URL]);
});

function makeFixture() {
  const counts = {
    frameSlotCount: 3,
    drawableFrameCount: 2,
    noDrawFrameCount: 1,
    maskFrameCount: 1,
    pageCount: 2,
    rectCount: 3,
  };
  const root = {
    schemaVersion: 1,
    kind: "mir2-crystal-full-pack-index",
    sourceContentHash: SOURCE_CONTENT_HASH,
    libraries: [
      {
        libraryKey: "NPC/00",
        sourceSha256: LIBRARY_SOURCE_HASH,
        shardUrl: SHARD_URL,
        ...counts,
      },
    ],
    summary: { libraryCount: 1, ...counts },
  };
  const shard = {
    schemaVersion: 1,
    kind: "mir2-crystal-library-pack",
    sourceContentHash: SOURCE_CONTENT_HASH,
    libraryKey: "NPC/00",
    sourceSha256: LIBRARY_SOURCE_HASH,
    shardUrl: SHARD_URL,
    frameSlotCount: 3,
    pages: [
      {
        key: "page:image",
        width: 64,
        height: 64,
        imageUrl: "/generated/crystal-packs/full/pages/image.png",
        rects: [
          { key: "NPC/00#0", x: 0, y: 0, width: 16, height: 20, sourceKind: "image" },
          { key: "NPC/00#2", x: 16, y: 0, width: 20, height: 24, sourceKind: "image" },
        ],
      },
      {
        key: "page:mask",
        width: 32,
        height: 32,
        imageUrl: "/generated/crystal-packs/full/pages/mask.png",
        rects: [
          { key: "NPC/00#2:mask", x: 0, y: 0, width: 20, height: 24, sourceKind: "mask" },
        ],
      },
    ],
    frames: [
      {
        index: 0,
        noDraw: false,
        pageKey: "page:image",
        rectKey: "NPC/00#0",
        imageUrl: "/generated/crystal-packs/full/pages/image.png",
        width: 16,
        height: 20,
        x: -8,
        y: -18,
      },
      { index: 1, noDraw: true },
      {
        index: 2,
        noDraw: false,
        pageKey: "page:image",
        rectKey: "NPC/00#2",
        imageUrl: "/generated/crystal-packs/full/pages/image.png",
        maskPageKey: "page:mask",
        maskRectKey: "NPC/00#2:mask",
        maskImageUrl: "/generated/crystal-packs/full/pages/mask.png",
        width: 20,
        height: 24,
        maskWidth: 20,
        maskHeight: 24,
      },
    ],
    summary: { ...counts },
  };
  return structuredClone({ root, shard });
}

function makeFetcher(root, shard) {
  const calls = [];
  const responses = new Map([
    [CRYSTAL_FULL_PACK_INDEX_URL, root],
    [SHARD_URL, shard],
  ]);
  return {
    calls,
    fetcher: async (url, init) => {
      calls.push(url);
      assert.deepEqual(init, {
        cache: url === CRYSTAL_FULL_PACK_INDEX_URL ? "no-cache" : "force-cache",
      });
      const payload = responses.get(url);
      return {
        ok: payload !== undefined,
        status: payload === undefined ? 404 : 200,
        json: async () => structuredClone(payload),
      };
    },
  };
}
