import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/scene-asset-preloader.ts", import.meta.url));
const compiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: sourcePath,
  reportDiagnostics: true,
});
const errors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(errors, []);
const loadedModule = { exports: {} };
new Function("exports", "module", compiled.outputText)(loadedModule.exports, loadedModule);

const { preloadSceneAssetUrls } = loadedModule.exports;

test("the actual scene collector stops raw entity requests only after the complete GPU atlas is presented", () => {
  const shellText = readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8");
  const file = ts.createSourceFile("shell.tsx", shellText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const functions = [], flags = [];
  const visit = node => {
    if (ts.isFunctionDeclaration(node) && ["collectVisibleSceneAssetUrls", "sceneMapSpritePreloadPriority"].includes(node.name?.text)) functions.push(node.getText(file));
    if (ts.isPropertyAssignment(node) && node.name.getText(file) === "includeEntitySprites") flags.push(node.initializer.getText(file));
    ts.forEachChild(node, visit);
  };
  visit(file); assert.equal(functions.length, 2); assert.equal(flags.length, 1);
  const code = ts.transpileModule(functions.join("\n"), { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  const collect = new Function("SCENE_INTERACTION_PRELOAD_URL_LIMIT", "SCENE_INTERACTION_ENTITY_PRELOAD_PATHS_PER_SPRITE",
    "SCENE_INTERACTION_ENTITY_PRELOAD_URL_LIMIT", "mapSpriteRenderPath", code + "\nreturn collectVisibleSceneAssetUrls;")(64, 8, 32, path => path);
  const shouldInclude = new Function("hideDomEntitySpritesForBevy", "useBevyEntityAtlas", "activeBevyEntityAtlasCoversAllSources", "webGl2EntityTextureReady", "return (" + flags[0] + ");");
  const map = { floor: [{ path: "/map-floor.png", left: 0, top: 0, width: 48, height: 32 }], objects: [] };
  const entities = [{ sprite: { body: { path: "/body.png" }, hair: { path: "/hair.png" },
    rearWeapons: [{ path: "/rear.png" }], frontWeapons: [{ path: "/front.png" }], preloadPaths: ["/next-body.png", "/body.png"] } }];
  const viewportLayout = { stageWidth: 1024, stageHeight: 768 };
  for (let mask = 0; mask < 16; mask++) {
    const values = [0, 1, 2, 3].map(bit => !!(mask & 1 << bit));
    const includeEntitySprites = shouldInclude(...values);
    const urls = collect(map, entities, { includeEntitySprites, includeEntityPreloadPaths: true, viewportLayout });
    assert.ok(urls.includes("/map-floor.png"), "GPU entity ownership never drops map dependencies");
    if (mask === 15) assert.deepEqual(urls, ["/map-floor.png"], "the presented, complete atlas owns all entity imagery");
    else for (const path of ["/body.png", "/hair.png", "/rear.png", "/front.png", "/next-body.png"]) assert.ok(urls.includes(path), "warming, uncovered or fallback imagery stays available");
    assert.equal(new Set(urls).size, urls.length);
  }
  assert.ok(collect(map, entities, { viewportLayout }).includes("/body.png"), "other callers retain the DOM default");
});

test("scene preload caps concurrency and stops queued work after interaction readiness", async () => {
  let active = 0;
  let maxActive = 0;
  let started = 0;
  const result = await preloadSceneAssetUrls(
    Array.from({ length: 100 }, (_, index) => `/tile-${index}.png`),
    2_000,
    {
      allowPartialReady: true,
      minLoaded: 6,
      concurrency: 4,
      loadCandidate: async () => {
        started += 1;
        active += 1;
        maxActive = Math.max(maxActive, active);
        await new Promise((resolve) => setTimeout(resolve, 2));
        active -= 1;
        return true;
      },
    },
  );

  assert.equal(result.ready, true);
  assert.equal(result.loaded, 6);
  assert.equal(maxActive, 4);
  assert.ok(started <= 9, `only a small bounded head should start, got ${started}`);
  assert.ok(result.pending >= 90, "the speculative tail must remain unstarted");
});

test("scene preload deduplicates URLs and uses candidate fallbacks", async () => {
  const attempted = [];
  const result = await preloadSceneAssetUrls(["/a.png", "/a.png"], 1_000, {
    minLoaded: 1,
    concurrency: 1,
    resolveCandidates: () => ["/primary.png", "/fallback.png"],
    loadCandidate: async (url) => {
      attempted.push(url);
      return url === "/fallback.png";
    },
  });

  assert.equal(result.total, 1);
  assert.equal(result.loaded, 1);
  assert.equal(result.visualReady, true);
  assert.deepEqual(attempted, ["/primary.png", "/fallback.png"]);
});

test("a timeout with no loaded image cannot masquerade as interaction-ready", async () => {
  const result = await preloadSceneAssetUrls(["/slow.png"], 5, {
    allowPartialReady: true,
    minLoaded: 1,
    loadCandidate: async () => await new Promise(() => undefined),
  });
  assert.equal(result.ready, false);
  assert.equal(result.interactionReady, false);
  assert.equal(result.status, "timeout");
  assert.equal(result.pending, 1);
});

test("default image loading coalesces concurrent requests for the same stable URL", async () => {
  const previousWindow = globalThis.window;
  const previousImage = globalThis.Image;
  let imageCount = 0;

  class FakeImage {
    complete = false;
    naturalWidth = 0;
    onload = null;
    onerror = null;
    decoding = "auto";

    set src(_url) {
      imageCount += 1;
      setTimeout(() => {
        this.complete = true;
        this.naturalWidth = 32;
        this.onload?.();
      }, 5);
    }
  }

  globalThis.window = { setTimeout, clearTimeout };
  globalThis.Image = FakeImage;
  try {
    const [first, second] = await Promise.all([
      preloadSceneAssetUrls(["/shared-stable.png"], 500),
      preloadSceneAssetUrls(["/shared-stable.png"], 500),
    ]);
    assert.equal(first.loaded, 1);
    assert.equal(second.loaded, 1);
    assert.equal(imageCount, 1);
  } finally {
    if (previousWindow === undefined) delete globalThis.window;
    else globalThis.window = previousWindow;
    if (previousImage === undefined) delete globalThis.Image;
    else globalThis.Image = previousImage;
  }
});
