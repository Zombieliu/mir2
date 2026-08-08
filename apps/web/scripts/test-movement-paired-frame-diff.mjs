import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.dirname(scriptDir);
const reporterPath = path.join(scriptDir, "report-movement-temporal-parity.mjs");
const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-movement-paired-frame-diff-"));

try {
  await testBoundedPairedFrameEvidence();
  await testExplicitPreActionClipWindow();
  await testOutputByteLimit();
  await testBackwardCompatibleOptOut();
  await testGeometryMismatchFailsClosed();
  console.log("Movement paired-frame diff tests passed");
} finally {
  await fs.rm(tempRoot, { recursive: true, force: true });
}

async function testExplicitPreActionClipWindow() {
  const fixture = await createFixture("pre-action", { width: 8, height: 6 });
  const outputDir = path.join(tempRoot, "pre-action-output");
  const result = runReporter(fixture, outputDir, "pre-action", [
    "--emitPairedFrameDiffs",
    "true",
    "--pairedFrameClipStartMs",
    "-20",
    "--pairedFrameClipEndMs",
    "-1",
    "--pairedFrameMaxDeltaMs",
    "10",
    "--pairedFrameWidth",
    "8",
  ]);
  assert.equal(result.status, 0, reporterFailure(result));

  const report = await readJson(path.join(outputDir, "pre-action.json"));
  const evidence = report.pairedFrameEvidence;
  assert.equal(evidence.candidatePairCount, 1);
  assert.equal(evidence.emittedPairCount, 1);
  assert.deepEqual(evidence.clipWindow.native, { startMs: -20, endMs: -1 });
  assert.deepEqual(evidence.clipWindow.web, { startMs: -20, endMs: -1 });
  assert.equal(evidence.pairs[0].native.actionAlignedElapsedMs, -10);
  assert.equal(evidence.pairs[0].web.actionAlignedElapsedMs, -9);
}

async function testBoundedPairedFrameEvidence() {
  const fixture = await createFixture("bounded", { width: 8, height: 6 });
  const outputDir = path.join(tempRoot, "bounded-output");
  const result = runReporter(fixture, outputDir, "bounded", [
    "--emitPairedFrameDiffs",
    "true",
    "--pairedFrameMaxPairs",
    "2",
    "--pairedFrameMaxOutputBytes",
    "50000",
    "--pairedFrameMaxDeltaMs",
    "10",
    "--pairedFrameWidth",
    "8",
  ]);
  assert.equal(result.status, 0, reporterFailure(result));

  const report = await readJson(path.join(outputDir, "bounded.json"));
  const evidence = report.pairedFrameEvidence;
  assert.equal(report.ok, true);
  assert.equal(evidence.enabled, true);
  assert.equal(evidence.ok, true);
  assert.equal(evidence.candidatePairCount, 4);
  assert.equal(evidence.selectedPairCount, 2);
  assert.equal(evidence.emittedPairCount, 2);
  assert.equal(evidence.geometryValidatedPairCount, 2);
  assert.equal(evidence.truncated, true);
  assert.deepEqual(evidence.truncationReasons, ["pair-count-limit"]);
  assert.ok(evidence.outputBytes > 0 && evidence.outputBytes <= evidence.maxOutputBytes);
  assert.equal(evidence.pairs.length, 2);
  assert.equal(evidence.regionalSummary.world.pairCount, 2);
  assert.ok(Number.isFinite(evidence.regionalSummary.world.meanAbsDeltaAverage));
  assert.ok(Number.isFinite(evidence.regionalSummary.hudMid.changedPixelRatioAverage));
  assert.deepEqual(
    evidence.pairs.map((pair) => pair.absoluteDeltaMs),
    [1, 0],
    "the bounded sample should retain nearest pairs across the aligned window",
  );

  for (const pair of evidence.pairs) {
    assert.equal(pair.geometry.sourceWidth, 8);
    assert.equal(pair.geometry.sourceHeight, 6);
    assert.equal(pair.geometry.evidenceWidth, 8);
    assert.equal(pair.geometry.evidenceHeight, 6);
    assert.equal(pair.geometry.channels, 3);
    assert.equal(pair.metrics.pixelCount, 48);
    assert.ok(Number.isFinite(pair.metrics.meanAbsDelta));
    assert.ok(Number.isFinite(pair.metrics.rootMeanSquareDelta));
    assert.ok(pair.metrics.changedPixelCount >= 0 && pair.metrics.changedPixelCount <= 48);
    assert.ok(pair.regionalMetrics.world.pixelCount > 0);
    assert.ok(Number.isFinite(pair.regionalMetrics.world.nativeMeanChannel));
    assert.ok(Number.isFinite(pair.regionalMetrics.hudRight.webMeanChannel));

    const overlayMetadata = await sharp(pair.artifacts.overlayPng.path).metadata();
    const heatmapMetadata = await sharp(pair.artifacts.heatmapDiffPng.path).metadata();
    assert.deepEqual(
      { format: overlayMetadata.format, width: overlayMetadata.width, height: overlayMetadata.height },
      { format: "png", width: 8, height: 6 },
    );
    assert.deepEqual(
      { format: heatmapMetadata.format, width: heatmapMetadata.width, height: heatmapMetadata.height },
      { format: "png", width: 8, height: 6 },
    );
    assert.equal(
      pair.artifacts.totalBytes,
      pair.artifacts.overlayPng.bytes + pair.artifacts.heatmapDiffPng.bytes,
    );
  }
  assert.equal(
    evidence.outputBytes,
    evidence.pairs.reduce((sum, pair) => sum + pair.artifacts.totalBytes, 0),
  );

  const markdown = await fs.readFile(path.join(outputDir, "bounded.md"), "utf8");
  assert.match(markdown, /## Paired Frame Evidence/);
  assert.match(markdown, /candidates=4, selected=2, emitted=2/);
}

async function testOutputByteLimit() {
  const fixture = await createFixture("byte-limit", { width: 8, height: 6 });
  const outputDir = path.join(tempRoot, "byte-limit-output");
  const result = runReporter(fixture, outputDir, "byte-limit", [
    "--emitPairedFrameDiffs",
    "true",
    "--pairedFrameMaxOutputBytes",
    "1",
    "--pairedFrameMaxDeltaMs",
    "10",
    "--pairedFrameWidth",
    "8",
  ]);
  assert.equal(result.status, 0, reporterFailure(result));

  const report = await readJson(path.join(outputDir, "byte-limit.json"));
  const evidence = report.pairedFrameEvidence;
  assert.equal(report.ok, false);
  assert.equal(evidence.enabled, true);
  assert.equal(evidence.ok, false);
  assert.equal(evidence.emittedPairCount, 0);
  assert.equal(evidence.outputBytes, 0);
  assert.equal(evidence.maxOutputBytes, 1);
  assert.equal(evidence.outputDir, null);
  assert.ok(evidence.truncationReasons.includes("output-byte-limit"));
  assert.deepEqual(await findPngs(outputDir), []);
}

async function testBackwardCompatibleOptOut() {
  const fixture = await createFixture("disabled", { width: 8, height: 6 });
  const outputDir = path.join(tempRoot, "disabled-output");
  const result = runReporter(fixture, outputDir, "disabled");
  assert.equal(result.status, 0, reporterFailure(result));

  const report = await readJson(path.join(outputDir, "disabled.json"));
  assert.equal(report.ok, true);
  assert.equal(report.pairedFrameEvidence.enabled, false);
  assert.equal(report.pairedFrameEvidence.emittedPairCount, 0);
  assert.deepEqual(await findPngs(outputDir), []);
}

async function testGeometryMismatchFailsClosed() {
  const fixture = await createFixture("geometry", {
    width: 8,
    height: 6,
    webWidth: 9,
  });
  const outputDir = path.join(tempRoot, "geometry-output");
  const result = runReporter(fixture, outputDir, "geometry", [
    "--emitPairedFrameDiffs",
    "true",
    "--pairedFrameMaxPairs",
    "1",
    "--pairedFrameMaxDeltaMs",
    "10",
    "--pairedFrameWidth",
    "8",
  ]);
  assert.notEqual(result.status, 0, "geometry mismatch must fail the reporter process");
  assert.match(`${result.stdout}\n${result.stderr}`, /Paired frame geometry differs/);
  await assert.rejects(fs.access(path.join(outputDir, "geometry.json")));
  assert.deepEqual(await findPngs(outputDir), []);
}

async function createFixture(name, { width, height, webWidth = width }) {
  const fixtureDir = path.join(tempRoot, name);
  await fs.mkdir(fixtureDir, { recursive: true });
  const nativeTimes = [90, 110, 210, 310];
  const webTimes = [96, 116, 218, 315];
  const colors = [
    [0, 0, 0],
    [40, 20, 10],
    [80, 40, 20],
    [120, 60, 30],
  ];
  const nativePaths = [];
  const webPaths = [];
  for (let index = 0; index < nativeTimes.length; index += 1) {
    const nativePath = path.join(fixtureDir, `native-${index}.png`);
    const webPath = path.join(fixtureDir, `web-${index}.png`);
    await writeRgbPng(nativePath, width, height, colors[index]);
    await writeRgbPng(
      webPath,
      webWidth,
      height,
      index === 0 ? colors[index] : colors[index].map((channel) => channel + 6),
    );
    nativePaths.push(nativePath);
    webPaths.push(webPath);
  }

  const originalPath = path.join(fixtureDir, "original.json");
  const webPath = path.join(fixtureDir, "web.json");
  await fs.writeFile(
    originalPath,
    `${JSON.stringify({
      ok: true,
      sampleMs: 50,
      actions: [{ label: "step1", performedAtCaptureMs: 100 }],
      samples: nativeTimes.map((elapsedMs, index) => ({
        label: "native",
        elapsedMs,
        index,
        capture: { path: nativePaths[index], width, height },
      })),
    })}\n`,
    "utf8",
  );
  await fs.writeFile(
    webPath,
    `${JSON.stringify({
      ok: true,
      sampleMs: 50,
      frameImageCount: webPaths.length,
      actions: [
        {
          dispatch: {
            clicks: [{ label: "step1", performedAtCaptureMs: 105 }],
          },
        },
      ],
      samples: webTimes.map((elapsedMs, index) => ({
        label: "web",
        t: elapsedMs,
        elapsedMs,
        index,
        frameImage: webPaths[index],
      })),
    })}\n`,
    "utf8",
  );
  return { originalPath, webPath };
}

async function writeRgbPng(filePath, width, height, color) {
  const data = Buffer.alloc(width * height * 3);
  for (let offset = 0; offset < data.length; offset += 3) {
    data[offset] = color[0];
    data[offset + 1] = color[1];
    data[offset + 2] = color[2];
  }
  await sharp(data, { raw: { width, height, channels: 3 } }).png().toFile(filePath);
}

function runReporter(fixture, outputDir, prefix, extraArgs = []) {
  return spawnSync(
    process.execPath,
    [
      reporterPath,
      "--original",
      fixture.originalPath,
      "--web",
      fixture.webPath,
      "--output",
      outputDir,
      "--prefix",
      prefix,
      "--frameDiffWidth",
      "8",
      ...extraArgs,
    ],
    {
      cwd: webDir,
      encoding: "utf8",
      timeout: 20_000,
    },
  );
}

function reporterFailure(result) {
  return `Reporter failed with status ${result.status}:\n${result.stdout}\n${result.stderr}`;
}

async function readJson(filePath) {
  return JSON.parse(await fs.readFile(filePath, "utf8"));
}

async function findPngs(root) {
  try {
    const entries = await fs.readdir(root, { recursive: true, withFileTypes: true });
    return entries
      .filter((entry) => entry.isFile() && entry.name.toLowerCase().endsWith(".png"))
      .map((entry) => path.join(entry.parentPath, entry.name))
      .sort();
  } catch (error) {
    if (error?.code === "ENOENT") return [];
    throw error;
  }
}
