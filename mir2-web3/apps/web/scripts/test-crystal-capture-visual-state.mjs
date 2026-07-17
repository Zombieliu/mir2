import assert from "node:assert/strict";

import {
  additiveEffectHasDirectWorldBackdrop,
  assertPairedCaptureLightSetting,
  crystalCaptureLightState,
  iconMatchesExpected,
  isDayCaptureLight,
  parseCaptureEffectFrame,
  parseCaptureLightSetting,
} from "./crystal-capture-visual-state.mjs";

assert.equal(parseCaptureLightSetting(undefined), null);
assert.equal(parseCaptureLightSetting(""), null);
assert.equal(parseCaptureLightSetting("   "), null);
assert.equal(parseCaptureLightSetting("day"), 2);
assert.equal(parseCaptureLightSetting("NIGHT"), 4);
assert.equal(parseCaptureLightSetting("0"), 0);
assert.equal(parseCaptureLightSetting(3), 3);
assert.throws(() => parseCaptureLightSetting("5"), /must be 0\.\.4/);
assert.throws(() => parseCaptureLightSetting("midday"), /must be 0\.\.4/);
assert.equal(parseCaptureEffectFrame(undefined), null);
assert.equal(parseCaptureEffectFrame("   "), null);
assert.equal(parseCaptureEffectFrame("0"), 0);
assert.equal(parseCaptureEffectFrame(9), 9);
assert.throws(() => parseCaptureEffectFrame(-1), /integer in 0\.\.9/);
assert.throws(() => parseCaptureEffectFrame(10), /integer in 0\.\.9/);
assert.throws(() => parseCaptureEffectFrame("1.5"), /integer in 0\.\.9/);

assert.deepEqual(crystalCaptureLightState(2), {
  setting: 2,
  label: "day",
  overlayClass: null,
  miniMapIcon: "/original-ui/Prguse/2093.png",
});
assert.equal(crystalCaptureLightState(4).overlayClass, "night");
assert.equal(isDayCaptureLight(0), true);
assert.equal(isDayCaptureLight(2), true);
assert.equal(isDayCaptureLight(1), false);
assert.deepEqual(assertPairedCaptureLightSetting(2, 2), {
  requestedSetting: 2,
  serverSetting: 2,
});
assert.deepEqual(assertPairedCaptureLightSetting(null, 4), {
  requestedSetting: null,
  serverSetting: 4,
});
assert.throws(
  () => assertPairedCaptureLightSetting(2, 4),
  /requires server lightSetting 2, got 4/,
);
assert.throws(
  () => assertPairedCaptureLightSetting(4, null),
  /got missing/,
);
assert.equal(iconMatchesExpected("http://127.0.0.1:3002/original-ui/Prguse/2093.png", "/original-ui/Prguse/2093.png"), true);
assert.equal(iconMatchesExpected("/original-ui/Prguse/2092.png", "/original-ui/Prguse/2093.png"), false);
assert.equal(
  additiveEffectHasDirectWorldBackdrop({
    blend: "additive",
    spriteOverlayZIndex: "auto",
    worldCompositeIsolation: "isolate",
    worldCompositeVisible: true,
  }),
  true,
);
assert.equal(
  additiveEffectHasDirectWorldBackdrop({
    blend: "additive",
    spriteOverlayZIndex: "5",
    worldCompositeIsolation: "isolate",
    worldCompositeVisible: true,
  }),
  false,
);
assert.equal(additiveEffectHasDirectWorldBackdrop({ blend: "alpha" }), true);

console.log("crystal capture visual state tests: ok");
