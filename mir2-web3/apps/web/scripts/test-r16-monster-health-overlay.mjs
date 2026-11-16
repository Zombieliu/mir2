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

const sceneOverlaysPath = new URL("../app/components/original-client-scene-overlays.tsx", import.meta.url);
const sceneOverlaysModule = { exports: {} };
const sceneOverlaysJs = ts.transpileModule(readFileSync(sceneOverlaysPath, "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
  fileName: fileURLToPath(sceneOverlaysPath),
}).outputText;
const offset = { x: 0, y: 0 };
new Function("exports", "module", "require", sceneOverlaysJs)(sceneOverlaysModule.exports, sceneOverlaysModule, (path) => {
  if (path === "./original-client-scene-rendering") return {
    EMPTY_VIEWPORT_OFFSET: offset, DEFAULT_VIEWPORT_LAYOUT: { entityLeftOrigin: 480, entityTopOrigin: 352 },
    VIEWPORT_CELL_WIDTH: 48, VIEWPORT_CELL_HEIGHT: 32,
    entityMotionOffsetForEntity: () => offset, entityNameplateColor: () => "#fff",
    entitySpriteHitBounds: () => ({ left: 0, right: 48, top: -48 }),
    ratio: (value, max) => value / max, viewportDepthForCell: () => 8,
  };
  if (path === "../../lib/crystal-content-localization") return { localizeCrystalEntityName: (name) => name };
  return require(path);
});
const { createElement } = require("react");
const { renderToStaticMarkup } = require("react-dom/server");
const self = { objectId: "self", kind: "selfPlayer", name: "Warrior", x: 0, y: 0, dx: 0, dy: 0 };
const hen = (objectId, hp, dx, dead = false) => ({ objectId, kind: "monster", name: "Hen", x: dx, y: 0, dx, dy: 0, hp, maxHp: 100, dead });
const fullHen = hen("full", 100, 4);
const merchant = { objectId: "merchant", kind: "npc", name: "Merchant", x: 3, y: 0, dx: 3, dy: 0 };
const makeSceneOverlays = (selectedEntity) => renderToStaticMarkup(createElement(sceneOverlaysModule.exports.OriginalClientSceneOverlays, {
  screen: "game", t: (_key, _params, fallback) => fallback, player: self, selectedEntity,
  viewportEntitySprites: [self, fullHen, hen("middle", 50, 5), hen("low", 25, 6), hen("dead", 0, 7, true), merchant]
    .map(entity => ({ entity, sprite: null })),
  playerCameraMotionOffset: offset, entityMotionSnapshots: {}, motionNow: 0, imperativeCamera: false,
  chatBubbles: [], damageFloaters: [], targetActionLabel: "Interact", entityKindClassName: (kind) => kind,
}));
const monsterOverlays = makeSceneOverlays(fullHen);
assert.doesNotMatch(monsterOverlays, /class="scene-target-readout"/);
assert.match(monsterOverlays, /class="scene-selection-ring hostile"/);
assert.match(monsterOverlays, /data-object-id="full" style="left:680px;top:288px;[^>]*><span style="width:32px;background:#00c000/);
assert.match(monsterOverlays, /data-object-id="middle"[^>]*><span style="width:16px;background:#e0a200/);
assert.match(monsterOverlays, /data-object-id="low"[^>]*><span style="width:8px;background:#d8352a/);
assert.doesNotMatch(monsterOverlays, /data-object-id="dead"/);
assert.match(makeSceneOverlays(merchant), /class="scene-target-readout"[\s\S]*?Interact/);
console.log("R16 monster health overlay checks passed");
