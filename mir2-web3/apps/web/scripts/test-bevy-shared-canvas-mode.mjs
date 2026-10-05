import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

function load(path) {
  const module = { exports: {} };
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  const output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("exports", "module", "require", output)(module.exports, module, () => ({}));
  return module.exports;
}

const mode = load("../lib/bevy-shared-canvas-mode.ts");
const quest = load("../lib/bevy-quest-ui.ts");

test("only exact first raw startup values opt WebGL2 into the primary shared canvas", () => {
  const valid = "?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1";
  assert.equal(mode.isWebGl2SharedCanvasPrototype(valid), true);
  assert.equal(mode.isWebGl2SharedCanvasPrototype(`${valid}&bevyQuestUi=1`), true);
  assert.equal(mode.isWebGl2SharedCanvasPrototype("?bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=1"), true);
  for (const search of [
    "?bevyBackend=webgpu&bevySharedCanvas=1&bevyBagUi=1",
    "?bevySharedCanvas=1&bevyBagUi=1",
    "?bevyBackend=webgl2&bevyBagUi=1",
    "?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=0",
    "?bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=%31",
    "??bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1",
    "?bevyBackend=web%67l2&bevySharedCanvas=1&bevyBagUi=1",
    "?bevyBackend=webgl2&bevySharedCanvas=%31&bevyBagUi=1",
    "?%62evyBackend=webgl2&bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1",
    "?bevyBackend=webgpu&bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1",
    "?bevyBackend=webgl2&bevySharedCanvas=0&bevySharedCanvas=1&bevyBagUi=1",
    "?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=0&bevyBagUi=1",
  ]) assert.equal(mode.isWebGl2SharedCanvasPrototype(search), false, search);
  assert.equal(mode.exactFirstRawQueryValue("?bevyBagUi=0&bevyBagUi=1", "bevyBagUi", "1"), false);
});

test("surface and backend selection stay fixed to the explicit prototype", () => {
  assert.equal(mode.sharedUiCanvasId(false), "mir2-quest-ui-canvas");
  assert.equal(mode.sharedUiCanvasId(true), "mir2-web3-canvas");
  assert.equal(mode.sharedCanvasUsesWebGl2(true, "webgl2"), true);
  assert.equal(mode.sharedCanvasUsesWebGl2(true, "webgpu"), false);
  assert.equal(mode.sharedCanvasUsesWebGl2(false, "webgl2"), false);
});

test("the selected primary canvas supplies existing stage presentation geometry", () => {
  const frame = { dataset: { viewportSceneWidth: "1368", viewportSceneHeight: "768" },
    clientWidth: 1368, clientHeight: 768,
    getBoundingClientRect: () => ({ left: 0, top: 1, width: 640, height: 359.3 }) };
  const canvas = { id: mode.sharedUiCanvasId(true), parentElement: frame,
    clientWidth: 1368, clientHeight: 768,
    getBoundingClientRect: () => ({ left: 0, top: 1, width: 640, height: 359.3 }) };
  assert.deepEqual(quest.readBevyQuestPresentation(frame, canvas, true), {
    logicalWidth: 1368, logicalHeight: 768, stageCssScale: 640 / 1368, touch: true,
  });
  assert.equal(quest.readBevyQuestPresentation(frame, { ...canvas, parentElement: {} }, true), null);
});
