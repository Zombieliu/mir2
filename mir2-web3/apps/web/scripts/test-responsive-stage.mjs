import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourcePath = new URL("../app/components/original-client-stage-presentation.ts", import.meta.url);
const source = readFileSync(sourcePath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(sourcePath),
});
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);

const {
  calculateMir2StagePresentation,
  calculateMir2TouchControlDeck,
  calculateMir2TouchControlMetrics,
  MIR2_TOUCH_GAME_RAIL_CSS_PX,
} = module.exports;

function assertContained(input, presentation) {
  const stageWidth = 1024 * presentation.scale;
  const stageHeight = 768 * presentation.scale;
  assert.ok(presentation.left >= 0);
  assert.ok(presentation.top >= 0);
  assert.ok(presentation.left + stageWidth <= input.cssWidth + 1);
  assert.ok(presentation.top + stageHeight <= input.cssHeight + 1);
}

const iphoneSelect = {
  cssWidth: 932,
  cssHeight: 430,
  devicePixelRatio: 2,
  layout: "touch",
  input: "touch",
  screen: "select",
};
const iphoneSelectPresentation = calculateMir2StagePresentation(iphoneSelect);
assertContained(iphoneSelect, iphoneSelectPresentation);
assert.equal(iphoneSelectPresentation.scale, 1144 / 2 / 1024);
assert.equal(iphoneSelectPresentation.left, 180);
assert.equal(iphoneSelectPresentation.top, 0.5);

const iphoneGame = { ...iphoneSelect, screen: "game" };
const iphoneGamePresentation = calculateMir2StagePresentation(iphoneGame);
assertContained(iphoneGame, iphoneGamePresentation);
assert.ok(iphoneGamePresentation.left >= MIR2_TOUCH_GAME_RAIL_CSS_PX);
assert.deepEqual(calculateMir2TouchControlDeck(iphoneGamePresentation), {
  left: 52,
  width: 828,
});

const narrowAndroidGame = {
  cssWidth: 667,
  cssHeight: 375,
  devicePixelRatio: 2,
  layout: "touch",
  input: "touch",
  screen: "game",
};
const narrowAndroidPresentation = calculateMir2StagePresentation(narrowAndroidGame);
assertContained(narrowAndroidGame, narrowAndroidPresentation);
assert.ok(narrowAndroidPresentation.left >= MIR2_TOUCH_GAME_RAIL_CSS_PX - 1);

const ultraWideTouchGame = {
  cssWidth: 995,
  cssHeight: 289.5,
  devicePixelRatio: 2,
  layout: "touch",
  input: "touch",
  screen: "game",
};
const ultraWideTouchPresentation = calculateMir2StagePresentation(ultraWideTouchGame);
const ultraWideTouchDeck = calculateMir2TouchControlDeck(ultraWideTouchPresentation);
const ultraWideTouchControls = calculateMir2TouchControlMetrics(ultraWideTouchGame.cssHeight);
assertContained(ultraWideTouchGame, ultraWideTouchPresentation);
assert.ok(ultraWideTouchDeck.left > 170);
assert.ok(ultraWideTouchDeck.left + ultraWideTouchDeck.width < ultraWideTouchGame.cssWidth - 170);
assert.equal(
  ultraWideTouchDeck.left + MIR2_TOUCH_GAME_RAIL_CSS_PX,
  ultraWideTouchPresentation.left,
);
assert.equal(ultraWideTouchControls.actionSize, 36);
const shortActionPadHeight = ultraWideTouchGame.cssHeight - 16;
const shortQuickSlotBottom =
  ultraWideTouchControls.quickRowTops[2] + ultraWideTouchControls.actionSize;
const shortRunTop =
  shortActionPadHeight -
  ultraWideTouchControls.runBottom -
  ultraWideTouchControls.actionSize;
const shortRunBottom = shortActionPadHeight - ultraWideTouchControls.runBottom;
const shortPrimaryTop = shortActionPadHeight - ultraWideTouchControls.primarySize;
assert.ok(shortQuickSlotBottom < shortRunTop);
assert.ok(shortRunBottom < shortPrimaryTop);

const regularTouchControls = calculateMir2TouchControlMetrics(iphoneGame.cssHeight);
assert.equal(regularTouchControls.actionSize, 44);

const desktop = {
  cssWidth: 1440,
  cssHeight: 900,
  devicePixelRatio: 1,
  layout: "desktop",
  input: "keyboardMouse",
  screen: "game",
};
assert.deepEqual(calculateMir2StagePresentation(desktop), {
  scale: 1,
  left: 208,
  top: 66,
});

const tv = {
  cssWidth: 1920,
  cssHeight: 1080,
  devicePixelRatio: 1,
  layout: "tv",
  input: "gamepad",
  screen: "game",
};
assert.deepEqual(calculateMir2StagePresentation(tv), {
  scale: 1.40625,
  left: 240,
  top: 0,
});

console.log("responsive stage tests passed");
