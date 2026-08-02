import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourcePath = new URL("../app/components/original-client-stage-presentation.ts", import.meta.url);
const source = readFileSync(sourcePath, "utf8");
const layoutSourcePath = new URL("../app/components/original-client-scene-layout.ts", import.meta.url);
const layoutSource = readFileSync(layoutSourcePath, "utf8");
const mobileControlsSource = readFileSync(
  new URL("../app/components/original-client-mobile-controls.tsx", import.meta.url),
  "utf8",
);
const globalCss = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");
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

const compiledLayout = ts.transpileModule(layoutSource, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(layoutSourcePath),
});
const layoutModule = { exports: {} };
const layoutRequire = (specifier) => {
  if (specifier === "../../lib/original-ui") {
    return { ORIGINAL_UI: { game: { sceneWidth: 1024, sceneHeight: 768 } } };
  }
  throw new Error(`unexpected layout dependency: ${specifier}`);
};
new Function("exports", "module", "require", compiledLayout.outputText)(
  layoutModule.exports,
  layoutModule,
  layoutRequire,
);

const {
  calculateMir2StagePresentation,
  calculateMir2TouchControlDeck,
  calculateMir2TouchControlMetrics,
  MIR2_TOUCH_GAME_RAIL_CSS_PX,
} = module.exports;
const { DEFAULT_VIEWPORT_LAYOUT, viewportLayoutForStage } = layoutModule.exports;

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
assert.equal(iphoneSelectPresentation.virtualWidth, 1024);
assert.equal(iphoneSelectPresentation.virtualHeight, 768);
assert.equal(iphoneSelectPresentation.wideMobile, false);

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

const wideMobile = {
  cssWidth: 844,
  cssHeight: 390,
  devicePixelRatio: 1,
  layout: "touch",
  input: "touch",
  screen: "game",
  wideMobile: true,
};
const wideMobilePresentation = calculateMir2StagePresentation(wideMobile);
assert.equal(wideMobilePresentation.wideMobile, true);
assert.equal(wideMobilePresentation.virtualWidth, 1664);
assert.equal(wideMobilePresentation.virtualHeight, 768);
assert.equal(wideMobilePresentation.left, 0);
assert.equal(wideMobilePresentation.top, 1);
assert.equal(wideMobilePresentation.scale, 844 / 1664);
assert.deepEqual(calculateMir2TouchControlDeck(wideMobilePresentation), {
  left: 0,
  width: wideMobilePresentation.virtualWidth * wideMobilePresentation.scale,
});
const wideViewportLayout = viewportLayoutForStage(
  wideMobilePresentation.virtualWidth,
  wideMobilePresentation.virtualHeight,
);
assert.equal(wideViewportLayout.stageWidth, 1664);
assert.equal(wideViewportLayout.rangeX, 23);
assert.equal(wideViewportLayout.rangeY, DEFAULT_VIEWPORT_LAYOUT.rangeY);
assert.equal(wideViewportLayout.entityLeftOrigin, 816);
assert.equal(wideViewportLayout.mouseTileCenterX, 840);
assert.equal(wideViewportLayout.tileLeftOrigin, 799);

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
  virtualWidth: 1024,
  virtualHeight: 768,
  wideMobile: false,
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
  virtualWidth: 1024,
  virtualHeight: 768,
  wideMobile: false,
});

assert.match(mobileControlsSource, /data-testid="mobile-orientation-gate"/);
assert.match(mobileControlsSource, /data-secondary-open=/);
assert.match(mobileControlsSource, /TUTORIAL_STEP_EVENT/);
assert.match(globalCss, /@media \(orientation: portrait\)/);
assert.match(globalCss, /mir-stage::before/);
assert.match(globalCss, /data-secondary-open="false"/);

console.log("responsive stage tests passed");
