import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { buildOriginalAssetManifest } from "./build-crystal-original-asset-manifest.mjs";
import {
  parseOriginalEvidenceArgs,
  prepareOriginalVisualEvidence,
} from "./prepare-original-visual-evidence.mjs";

async function fixture(t) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-original-evidence-"));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  const assetRoot = path.join(directory, "CrystalClient");
  const manifestPath = path.join(directory, "crystal-assets.json");
  const executablePath = path.join(directory, "Client.exe");
  const outputDir = path.join(directory, "evidence");
  await fs.mkdir(path.join(assetRoot, "Data"), { recursive: true });
  await fs.writeFile(path.join(assetRoot, "Data", "items.dat"), "items");
  await fs.writeFile(path.join(assetRoot, "Map.bin"), "map");
  await fs.writeFile(executablePath, Buffer.from("Crystal executable fixture"));
  await buildOriginalAssetManifest({
    assetRoot,
    output: manifestPath,
    generatedAt: "2026-08-24T00:00:00.000Z",
  });
  return { directory, assetRoot, manifestPath, executablePath, outputDir };
}

async function hashAndSize(filePath) {
  const bytes = await fs.readFile(filePath);
  return {
    bytes: bytes.length,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
  };
}

function relative(from, target) {
  return path.relative(from, target).split(path.sep).join("/");
}

test("evidence binds actual executable/manifest bytes and content-addressed sourceRevision", async (t) => {
  const fixtureData = await fixture(t);
  const observedAt = "2026-08-24T00:00:01.000Z";
  const executable = await hashAndSize(fixtureData.executablePath);
  const manifest = await hashAndSize(fixtureData.manifestPath);
  // The evidence tool must not walk the original asset root after the manifest exists.
  await fs.rm(fixtureData.assetRoot, { recursive: true, force: true });

  const result = await prepareOriginalVisualEvidence({
    runId: "original-visual-001",
    executable: fixtureData.executablePath,
    assetManifest: fixtureData.manifestPath,
    outputDir: fixtureData.outputDir,
    observedAt,
  });
  const build = JSON.parse(await fs.readFile(result.buildEvidencePath, "utf8"));
  const asset = JSON.parse(await fs.readFile(result.assetEvidencePath, "utf8"));

  assert.deepEqual(Object.keys(build).sort(), ["executable", "observedAt", "producer", "runId", "schemaVersion", "sourceRevision"]);
  assert.deepEqual(Object.keys(build.executable).sort(), ["bytes", "path", "sha256"]);
  assert.deepEqual(Object.keys(asset).sort(), ["assetManifest", "observedAt", "producer", "runId", "schemaVersion"]);
  assert.deepEqual(Object.keys(asset.assetManifest).sort(), ["bytes", "path", "sha256"]);
  assert.equal(build.schemaVersion, "mir2-native-build-evidence-v1");
  assert.equal(build.producer, "crystal-original-build-evidence");
  assert.equal(asset.schemaVersion, "mir2-native-asset-evidence-v1");
  assert.equal(asset.producer, "crystal-original-asset-evidence");
  assert.equal(build.runId, "original-visual-001");
  assert.equal(asset.runId, "original-visual-001");
  assert.equal(build.observedAt, observedAt);
  assert.equal(asset.observedAt, observedAt);
  assert.equal(build.sourceRevision, `crystal-original-artifact-${executable.sha256}`);
  assert.equal(build.executable.sha256, executable.sha256);
  assert.equal(build.executable.bytes, executable.bytes);
  assert.equal(asset.assetManifest.sha256, manifest.sha256);
  assert.equal(asset.assetManifest.bytes, manifest.bytes);
  assert.equal(build.executable.path, relative(fixtureData.outputDir, fixtureData.executablePath));
  assert.equal(asset.assetManifest.path, relative(fixtureData.outputDir, fixtureData.manifestPath));
  assert.equal(path.isAbsolute(build.executable.path), false);
  assert.equal(path.isAbsolute(asset.assetManifest.path), false);
});

test("evidence rejects a tampered manifest and a rootSha256 mismatch without rewalking assets", async (t) => {
  const fixtureData = await fixture(t);
  const original = JSON.parse(await fs.readFile(fixtureData.manifestPath, "utf8"));
  original.rootSha256 = "0".repeat(64);
  const tamperedPath = path.join(fixtureData.directory, "tampered-manifest.json");
  await fs.writeFile(tamperedPath, `${JSON.stringify(original, null, 2)}\n`, "utf8");
  await assert.rejects(
    prepareOriginalVisualEvidence({
      runId: "original-visual-002",
      executable: fixtureData.executablePath,
      assetManifest: tamperedPath,
      outputDir: fixtureData.outputDir,
    }),
    /rootSha256 does not match canonical entries/,
  );
  assert.equal(await fs.stat(fixtureData.outputDir).then(() => true).catch(() => false), false);

  original.rootSha256 = (await import("./build-crystal-original-asset-manifest.mjs")).computeRootSha256(original.files);
  original.extra = "closed schema violation";
  const closedPath = path.join(fixtureData.directory, "closed-manifest.json");
  await fs.writeFile(closedPath, `${JSON.stringify(original, null, 2)}\n`, "utf8");
  await assert.rejects(
    prepareOriginalVisualEvidence({
      runId: "original-visual-003",
      executable: fixtureData.executablePath,
      assetManifest: closedPath,
      outputDir: path.join(fixtureData.directory, "closed-evidence"),
    }),
    /unknown field/,
  );
});

test("evidence rejects hand-filled digest/source revision arguments and invalid run IDs", () => {
  assert.throws(
    () => parseOriginalEvidenceArgs([
      "--run-id", "x",
      "--executable", "Client.exe",
      "--asset-manifest", "manifest.json",
      "--output-dir", "evidence",
      "--source-revision", "hand-filled",
    ]),
    /Unknown argument: --source-revision/,
  );
  assert.throws(
    () => parseOriginalEvidenceArgs([
      "--run-id", "../private",
      "--executable", "Client.exe",
      "--asset-manifest", "manifest.json",
      "--output-dir", "evidence",
    ]),
    /run-id must match/,
  );
});

test("evidence refuses to overwrite either existing artifact", async (t) => {
  const fixtureData = await fixture(t);
  const result = await prepareOriginalVisualEvidence({
    runId: "original-visual-004",
    executable: fixtureData.executablePath,
    assetManifest: fixtureData.manifestPath,
    outputDir: fixtureData.outputDir,
  });
  const beforeBuild = await fs.readFile(result.buildEvidencePath, "utf8");
  const beforeAsset = await fs.readFile(result.assetEvidencePath, "utf8");
  await assert.rejects(
    prepareOriginalVisualEvidence({
      runId: "original-visual-005",
      executable: fixtureData.executablePath,
      assetManifest: fixtureData.manifestPath,
      outputDir: fixtureData.outputDir,
    }),
    /refusing to overwrite existing/,
  );
  assert.equal(await fs.readFile(result.buildEvidencePath, "utf8"), beforeBuild);
  assert.equal(await fs.readFile(result.assetEvidencePath, "utf8"), beforeAsset);
});
