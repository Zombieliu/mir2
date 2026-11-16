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

test("shared UI requests default on and allow independent first-value opt-outs", () => {
  for (const strictRaw of [false, true]) {
    assert.deepEqual(mode.resolveSharedUiRequests("", strictRaw), { quest: true, bag: true });
    assert.deepEqual(mode.resolveSharedUiRequests("?bevyBackend=webgpu", strictRaw), { quest: true, bag: true });
    assert.deepEqual(mode.resolveSharedUiRequests("?bevyQuestUi=0", strictRaw), { quest: false, bag: true });
    assert.deepEqual(mode.resolveSharedUiRequests("?bevyBagUi=0", strictRaw), { quest: true, bag: false });
    assert.deepEqual(mode.resolveSharedUiRequests("?bevyQuestUi=1&bevyBagUi=1", strictRaw), { quest: true, bag: true });
    assert.deepEqual(mode.resolveSharedUiRequests("?bevyQuestUi=0&bevyQuestUi=1&bevyBagUi=1&bevyBagUi=0", strictRaw), { quest: false, bag: true });
    assert.deepEqual(mode.resolveSharedUiRequests("?bevyQuestUi=1&bevyQuestUi=0&bevyBagUi=0&bevyBagUi=1", strictRaw), { quest: true, bag: false });
    assert.deepEqual(mode.resolveSharedUiRequests("?bevyQuestUi=&bevyBagUi=2", strictRaw), { quest: false, bag: false });
  }
});

test("strict shared UI requests reject encoded aliases and values", () => {
  assert.deepEqual(mode.resolveSharedUiRequests("?%62evyQuestUi=1&bevyBagUi=%31", true), { quest: false, bag: false });
  assert.deepEqual(mode.resolveSharedUiRequests("?%62evyQuestUi=1&bevyQuestUi=1&bevyBagUi=1", true), { quest: false, bag: true });
  assert.deepEqual(mode.resolveSharedUiRequests("?bevyQuestUi=%31&bevyQuestUi=1&bevyBagUi=1", true), { quest: false, bag: true });
  assert.deepEqual(mode.resolveSharedUiRequests("?bevyQuestUi=1&bevyBagUi=0&%62evyBagUi=1", true), { quest: true, bag: false });
});

test("ordinary shared UI requests retain URLSearchParams decoded first values", () => {
  assert.deepEqual(mode.resolveSharedUiRequests("?%62evyQuestUi=%31&bevyBagUi=%31"), { quest: true, bag: true });
  assert.deepEqual(mode.resolveSharedUiRequests("?bevyQuestUi=%31&bevyQuestUi=0&bevyBagUi=0&bevyBagUi=1"), { quest: true, bag: false });
  assert.deepEqual(mode.resolveSharedUiRequests("?bevyQuestUi=0&%62evyQuestUi=1"), { quest: false, bag: true });
});

test("default shared canvas intent keeps every explicit override exact", () => {
  assert.equal(mode.wantsPrimarySharedCanvas(""), true);
  assert.equal(mode.wantsPrimarySharedCanvas("?bevyBackend=webgpu"), true);
  assert.equal(mode.wantsPrimarySharedCanvas("?bevyQuestUi=0"), true);
  assert.equal(mode.wantsPrimarySharedCanvas("?bevyBagUi=0"), true);
  assert.equal(mode.wantsPrimarySharedCanvas("?bevyQuestUi=0&bevyBagUi=0"), false);
  assert.equal(mode.wantsPrimarySharedCanvas("?bevyQuestUi=%31&bevyBagUi=0"), false);
  assert.equal(mode.wantsPrimarySharedCanvas("?%62evyQuestUi=1&bevyBagUi=0"), false);
  assert.equal(mode.wantsPrimarySharedCanvas("?bevySharedCanvas=0"), false);
  assert.equal(mode.wantsPrimarySharedCanvas("?%62evySharedCanvas=1"), false);
  assert.equal(mode.wantsPrimarySharedCanvas("?bevySharedCanvas=0&bevySharedCanvas=1"), false);
  assert.equal(mode.wantsPrimarySharedCanvas("?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1"), true);
  assert.equal(mode.wantsPrimarySharedCanvas("?bevyBackend=webgl2&bevySharedCanvas=%31&bevyBagUi=1"), false);
  assert.notEqual(mode.sharedCanvasQuerySignature("?bevyQuestUi=0&bevyBagUi=1"),
    mode.sharedCanvasQuerySignature("?bevyQuestUi=1&bevyBagUi=0"));
  assert.equal(mode.sharedCanvasQuerySignature("?bevyBagUi=1&mapAtlas=0"),
    mode.sharedCanvasQuerySignature("?bevyBagUi=1&mapAtlas=1"));
});

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

test("shared UI uses the fixed stage canvas while GL2 activation follows actual backend", () => {
  assert.equal(mode.sharedUiCanvasId(false), "mir2-quest-ui-canvas");
  assert.equal(mode.sharedUiCanvasId(true), "mir2-quest-ui-canvas");
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
