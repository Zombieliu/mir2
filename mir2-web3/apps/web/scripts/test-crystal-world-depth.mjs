import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTs(path, dependencies = {}) {
  const url = new URL(path, import.meta.url);
  const compiled = ts.transpileModule(readFileSync(url, "utf8"), {
    fileName: fileURLToPath(url),
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.CommonJS,
      jsx: ts.JsxEmit.ReactJSX,
      strict: true,
    },
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (Object.hasOwn(dependencies, specifier)) return dependencies[specifier];
    throw new Error(`Unexpected test import ${specifier} in ${path}`);
  };
  new Function("exports", "module", "require", compiled.outputText)(module.exports, module, require);
  return module.exports;
}

const depth = loadTs("../lib/crystal-world-depth.ts");
const originalUi = { ORIGINAL_UI: { game: { sceneWidth: 1024, sceneHeight: 768 } } };
const layout = loadTs("../app/components/original-client-scene-layout.ts", {
  "../../lib/original-ui": originalUi,
  "../../lib/crystal-world-depth": depth,
});
const motion = loadTs("../app/components/original-client-scene-motion.ts", {
  "./original-client-scene-layout": layout,
});
const map = loadTs("../app/components/original-client-scene-map-rendering.tsx", {
  "react": { memo: (component) => component },
  "react/jsx-runtime": {},
  "../../lib/original-ui": originalUi,
  "../../lib/crystal-world-depth": depth,
  "../../public/generated/original-map-blend/blend-frames.json": { default: { lib: "WemadeMir2/Objects", frames: [] } },
  "../../lib/map-atlas-manifest": {
    mapAtlasPathRequiresAlphaKey: () => false,
    mapAtlasRectKeyForPath: (path) => path,
  },
  "../../lib/scene-alpha-key": {},
  "../../lib/scene-asset-retry-policy": {},
  "./original-client-scene-layout": layout,
});
const center = { x: 289, y: 615 };
const actor = { objectId: "npc", kind: "npc", name: "Merchant", ...center };
const effect = loadTs("../lib/scene-effect-runtime.ts", { "./crystal-magic-effects": {} });

function mapFixture(anchor = center) {
  const cells = [
    { x: anchor.x, y: anchor.y, back: "floor", middle: "middle", front: "front" },
    { x: anchor.x + 2, y: anchor.y, front: "front" },
    { x: anchor.x - 2, y: anchor.y + 1, front: "front" },
  ];
  const sprite = (kind, drawMode, height) => ({
    kind, drawMode,
    frames: [{ path: `/original-map/WemadeMir2/Objects/${kind}.png`, width: 48, height }],
  });
  return {
    originalMapRegion: {
      regionBounds: {
        minX: anchor.x - 2, maxX: anchor.x + 2,
        minY: anchor.y, maxY: anchor.y + 1,
      },
      cells,
      sprites: {
        floor: sprite("back", "floor", 32),
        middle: sprite("middle", "object", 128),
        front: sprite("front", "object", 128),
      },
    },
  };
}

test("complete map row precedes all same-row objects and the next row still occludes", () => {
  for (const anchor of [center, { x: 65_400, y: 65_400 }]) {
    for (const mapDx of [-128, -70, -1, 0, 1, 70, 128]) {
      for (const actorDx of [-128, -70, -1, 0, 1, 70, 128]) {
        const mapFront = depth.crystalMapDepth(anchor.x + mapDx, anchor.y, anchor, 1);
        const live = depth.crystalObjectDepth(anchor.x + actorDx, anchor.y, anchor);
        const nextMap = depth.crystalMapDepth(anchor.x - 128, anchor.y + 1, anchor, -2);
        assert.ok(mapFront < live + 1, "even the rightmost map column precedes the leftmost item");
        assert.ok(live + 14 < nextMap, "even the rightmost actor overlay remains below the next row");
      }
    }
  }
});

test("Web matches the native Bichon source-cell depth receipt", () => {
  assert.equal(depth.crystalObjectDepth(289, 610, { x: 289, y: 611 }), 1_532_560);
  assert.equal(depth.crystalEntityLayerDepth(1_532_560, "body"), 1_532_565);
});

test("camera translation and u16 map edges preserve composite f32 ordering", () => {
  const base = depth.crystalObjectDepth(290, 614, center);
  assert.equal(base, depth.crystalObjectDepth(65_401, 65_399, { x: 65_400, y: 65_400 }));
  const layers = ["ground", "mount", "rearWeapon", "body", "hair", "frontWeapon"]
    .map((role) => Math.fround(depth.crystalEntityLayerDepth(base, role)));
  for (let i = 1; i < layers.length; i += 1) assert.ok(layers[i - 1] < layers[i]);
  assert.ok(layers.at(-1) < depth.crystalEntityLayerDepth(depth.crystalObjectDepth(291, 614, center), "ground"));
});

test("malformed far outliers cannot escape their bounded row passes", () => {
  const farMap = depth.crystalMapDepth(Number.MAX_VALUE, center.y, center, 1);
  const farActor = depth.crystalObjectDepth(-Number.MAX_VALUE, center.y, center);
  assert.ok(farMap < farActor);
  assert.ok(Number.isFinite(depth.crystalObjectDepth(Number.NaN, Number.POSITIVE_INFINITY, center)));
});

test("floating drop labels stay above the largest actor hit region inside the isolated world", () => {
  const css = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");
  const rule = (selector) => {
    const start = css.indexOf(`${selector} {`);
    assert.notEqual(start, -1, selector);
    return css.slice(start, css.indexOf("}", start));
  };
  const drop = rule(".viewport-drop-overlay"), world = rule(".game-world-composite");
  const dropZ = Number(drop.match(/z-index:\s*(\d+)/)?.[1]);
  const largestActor = depth.crystalObjectDepth(Number.MAX_VALUE, Number.MAX_VALUE, center);
  assert.ok(dropZ > largestActor + 20, "a corpse's transparent hit region cannot cover the visible drop label");
  assert.match(drop, /pointer-events:\s*none/, "the space between labels remains pass-through");
  assert.match(world, /isolation:\s*isolate/, "the high label depth cannot escape above sibling game UI");
  assert.match(world, /z-index:\s*1\s*;/);
});

test("walking sorts by its source tile until completion while grid position stays at the target", () => {
  const entity = { ...actor, kind: "player", x: 290, y: 616 };
  const segment = { fromX: 289, fromY: 615, toX: 290, toY: 616, animationState: "walking", startedAt: 1000, expiresAt: 1600 };
  assert.deepEqual(depth.crystalEntityDepth(entity, center, segment, 1599), {
    sortZ: depth.crystalObjectDepth(289, 615, center), motionSortX: 289, motionSortY: 615,
  });
  assert.equal(depth.crystalEntityDepth(entity, center, segment, 1600).motionSortY, 616);
  assert.equal(entity.y, 616);
});

test("death, NPCs, stale target and malformed timing never borrow an old source tile", () => {
  const entity = { ...actor, kind: "player", x: 290, y: 616 };
  const segment = { fromX: 289, fromY: 615, toX: 290, toY: 616, animationState: "running", startedAt: 1000, expiresAt: 1600 };
  for (const [candidate, motionWindow, now] of [
    [{ ...entity, dead: true }, segment, 1200],
    [{ ...entity, kind: "npc" }, segment, 1200],
    [entity, { ...segment, toY: 617 }, 1200],
    [entity, { ...segment, expiresAt: Number.NaN }, 1200],
    [entity, { ...segment, startedAt: 1700 }, 1200],
    [entity, segment, Number.NaN],
  ]) assert.equal(depth.crystalEntityDepth(candidate, center, motionWindow, now).motionSortY, 616);
});

test("fractional interpolation anchors cannot invent a source cell", () => {
  const entity = { ...actor, kind: "player", x: 291, y: 616 };
  const segment = { fromX: 289.75, fromY: 615.75, toX: 291, toY: 616, animationState: "running", startedAt: 1000, expiresAt: 1600 };
  assert.equal(depth.crystalEntityDepth(entity, center, segment, 1200).motionSortX, 291);
  assert.equal(depth.crystalEntityDepth(entity, center, { ...segment, sortX: 290, sortY: 616 }, 1200).motionSortX, 290);
});

test("real motion refresh records the previous authoritative target during a mid-glide retarget", () => {
  const entity = { ...actor, kind: "player", x: 291, y: 616 };
  const previous = { fromX: 289, fromY: 615, toX: 290, toY: 616, animationState: "walking", startedAt: 1000, expiresAt: 1600 };
  const refreshed = motion.refreshEntityMotionSnapshots("game", [entity], null, { npc: previous }, 1250).npc;
  assert.ok(refreshed.fromX > 289 && refreshed.fromX < 290, "pixel movement still starts at the interpolated position");
  assert.equal(refreshed.sortX, 290);
  assert.equal(refreshed.sortY, 616);
  assert.equal(depth.crystalEntityDepth(entity, center, refreshed, 1250).motionSortX, 290);
});

test("actual map producer orders right-hand stalls behind same-row NPCs but preserves next-row occlusion", () => {
  const sprites = map.buildViewportMapSprites(mapFixture(), actor, 0);
  const live = depth.crystalEntityLayerDepth(depth.crystalEntityDepth(actor, center, null, 0).sortZ, "body");
  const sameRow = sprites.objects.filter((sprite) => sprite.cellY === center.y);
  const nextRow = sprites.objects.filter((sprite) => sprite.cellY === center.y + 1);
  assert.ok(sameRow.some((sprite) => sprite.cellX > center.x));
  assert.ok(sameRow.every((sprite) => sprite.zIndex < live));
  assert.ok(nextRow.length && nextRow.every((sprite) => sprite.zIndex > live));
  assert.ok(sprites.floor.every((sprite) => sprite.zIndex < Math.min(...sprites.objects.map((entry) => entry.zIndex))));
  assert.ok(sprites.objects.every((sprite) => Number.isInteger(sprite.zIndex)), "DOM z-index remains an integer");
});

test("atlas and standalone GPU producers apply the runtime map gain exactly once", () => {
  const sprites = map.buildViewportMapSprites(mapFixture(), actor, 0);
  const paths = [...sprites.floor, ...sprites.objects].map((sprite) => [sprite.path, "fixture-atlas"]);
  const atlas = map.buildMapTileDrawList(sprites, { rectToAtlas: new Map(paths) }, { x: 0, y: 0 });
  const standalone = map.buildStandaloneMapTiles(sprites, { x: 0, y: 0 });
  assert.equal(atlas.tiles.length, paths.length);
  assert.equal(standalone.tiles.length, paths.length);
  for (const sprite of [...sprites.floor, ...sprites.objects]) {
    const atlasTile = atlas.tiles.find((tile) => tile.key === sprite.key);
    const standaloneTile = standalone.tiles.find((tile) => tile.key.endsWith(sprite.key));
    assert.equal(atlasTile.z * depth.CRYSTAL_MAP_DEPTH_GAIN, sprite.zIndex);
    assert.equal(standaloneTile.z * depth.CRYSTAL_MAP_DEPTH_GAIN, sprite.zIndex);
  }
});

test("actual map draw positions and depths are invariant under a large camera/map translation", () => {
  const low = map.buildViewportMapSprites(mapFixture(), actor, 0);
  const anchor = { x: 65_400, y: 65_400 };
  const high = map.buildViewportMapSprites(mapFixture(anchor), { ...actor, ...anchor }, 0);
  const drawFacts = (sprites) => sprites.objects.map(({ left, top, zIndex }) => ({ left, top, zIndex }));
  assert.deepEqual(drawFacts(high), drawFacts(low));
});

test("DOM entity and item producers use the same raw depth contract", () => {
  const source = readFileSync(new URL("../app/components/original-client-scene-visual-layers.tsx", import.meta.url), "utf8");
  assert.match(source, /crystalEntityDepth\(\s*entity, viewportDepthPlayer, entityMotionSnapshots\[entity\.objectId\], motionNow/);
  assert.match(source, /zIndex: entityDepth\.sortZ \+ 5/);
  assert.match(source, /viewportDepthForCell\(drop\.x, drop\.y, viewportDepthPlayer, 1\)/);
});

test("persistent spell and mask remain below bodies while transient effects stay within the same column", () => {
  const live = depth.crystalEntityLayerDepth(0, "body");
  const front = depth.crystalEntityLayerDepth(0, "frontWeapon");
  assert.ok(effect.crystalSceneEffectLayerOffset("objectSpell") < live);
  assert.ok(effect.crystalSceneEffectLayerOffset("objectSpell", true) < live);
  assert.equal(effect.crystalSceneEffectLayerOffset("objectSpell", true), effect.crystalSceneEffectLayerOffset("objectSpell") + 1);
  for (const source of ["spell", "attackOverlay", "actorEffect"]) {
    assert.ok(effect.crystalSceneEffectLayerOffset(source) > front);
    assert.ok(effect.crystalSceneEffectLayerOffset(source, true) < 20);
  }
});

test("standalone depth policy passes strict focused TypeScript", () => {
  const path = fileURLToPath(new URL("../lib/crystal-world-depth.ts", import.meta.url));
  const program = ts.createProgram([path], {
    noEmit: true, strict: true, skipLibCheck: true,
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler, types: [],
  });
  const errors = ts.getPreEmitDiagnostics(program).filter((diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error);
  assert.equal(errors.length, 0, ts.formatDiagnosticsWithColorAndContext(errors, {
    getCanonicalFileName: (name) => name,
    getCurrentDirectory: () => process.cwd(),
    getNewLine: () => "\n",
  }));
});
