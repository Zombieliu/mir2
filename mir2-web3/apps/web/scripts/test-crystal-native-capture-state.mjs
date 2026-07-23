import assert from "node:assert/strict";

import {
  assertCanonicalNativeCaptureReport,
  assertNativeCursorParking,
  assertNativeFrameDimensions,
} from "./crystal-native-capture-state.mjs";

const validReport = {
  ok: true,
  captureArea: { width: 1024, height: 768 },
  cursorParking: { requestedScreenX: 12, requestedScreenY: 384, succeeded: true },
  sampleCount: 2,
  samples: [
    { capture: { width: 1024, height: 768, path: "frame-0.png" } },
    { capture: { width: 1024, height: 768, path: "frame-1.png" } },
  ],
};

assert.deepEqual(assertCanonicalNativeCaptureReport(validReport), {
  expectedWidth: 1024,
  expectedHeight: 768,
  sampleCount: 2,
});
assert.deepEqual(assertNativeFrameDimensions({ width: 1024, height: 768 }), {
  width: 1024,
  height: 768,
});
assert.deepEqual(assertNativeCursorParking(validReport), {
  requestedScreenX: 12,
  requestedScreenY: 384,
});

assert.throws(
  () =>
    assertCanonicalNativeCaptureReport({
      ...validReport,
      captureArea: { width: 160, height: 28 },
    }),
  /Expected capture area to be 1024x768, got 160x28/,
);
assert.throws(
  () =>
    assertCanonicalNativeCaptureReport({
      ...validReport,
      samples: [{ capture: { width: 160, height: 28 } }],
      sampleCount: 1,
    }),
  /Expected sample 0 to be 1024x768, got 160x28/,
);
assert.throws(
  () => assertCanonicalNativeCaptureReport({ ...validReport, sampleCount: 3 }),
  /sample count mismatch/,
);
assert.throws(
  () => assertNativeFrameDimensions({ width: 1024 }),
  /got 1024xmissing/,
);
assert.throws(
  () => assertNativeCursorParking({ ...validReport, cursorParking: { succeeded: false } }),
  /did not confirm that the cursor was parked/,
);

console.log("crystal-native-capture-state assertions passed");
