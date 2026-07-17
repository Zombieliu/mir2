export const CRYSTAL_NATIVE_CLIENT_WIDTH = 1024;
export const CRYSTAL_NATIVE_CLIENT_HEIGHT = 768;

export function assertCanonicalNativeCaptureReport(report, options = {}) {
  const expectedWidth = expectedDimension(
    options.expectedWidth,
    CRYSTAL_NATIVE_CLIENT_WIDTH,
    "expectedWidth",
  );
  const expectedHeight = expectedDimension(
    options.expectedHeight,
    CRYSTAL_NATIVE_CLIENT_HEIGHT,
    "expectedHeight",
  );

  if (!report || typeof report !== "object") {
    throw new Error("Native capture report is missing or invalid.");
  }
  if (report.ok !== true) {
    throw new Error("Native capture report did not complete successfully.");
  }

  assertNativeFrameDimensions(report.captureArea, {
    expectedWidth,
    expectedHeight,
    label: "capture area",
  });

  const samples = Array.isArray(report.samples) ? report.samples : [];
  if (samples.length === 0) {
    throw new Error("Native capture report did not contain any samples.");
  }
  samples.forEach((sample, index) => {
    assertNativeFrameDimensions(sample?.capture, {
      expectedWidth,
      expectedHeight,
      label: `sample ${index}`,
    });
  });

  const declaredSampleCount = Number(report.sampleCount);
  if (Number.isInteger(declaredSampleCount) && declaredSampleCount !== samples.length) {
    throw new Error(
      `Native capture report sample count mismatch: declared ${declaredSampleCount}, found ${samples.length}.`,
    );
  }

  return { expectedWidth, expectedHeight, sampleCount: samples.length };
}

export function assertNativeFrameDimensions(frame, options = {}) {
  const expectedWidth = expectedDimension(
    options.expectedWidth,
    CRYSTAL_NATIVE_CLIENT_WIDTH,
    "expectedWidth",
  );
  const expectedHeight = expectedDimension(
    options.expectedHeight,
    CRYSTAL_NATIVE_CLIENT_HEIGHT,
    "expectedHeight",
  );
  const label = options.label ?? "native frame";
  const width = Number(frame?.width);
  const height = Number(frame?.height);

  if (width !== expectedWidth || height !== expectedHeight) {
    throw new Error(
      `Expected ${label} to be ${expectedWidth}x${expectedHeight}, got ${displayDimension(width)}x${displayDimension(height)}.`,
    );
  }

  return { width, height };
}

function expectedDimension(value, fallback, label) {
  const parsed = value === undefined ? fallback : Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer.`);
  }
  return parsed;
}

function displayDimension(value) {
  return Number.isFinite(value) ? String(value) : "missing";
}
