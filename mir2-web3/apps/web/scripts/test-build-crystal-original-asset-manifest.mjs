import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  buildOriginalAssetManifest,
  canonicalManifestEntries,
  computeRootSha256,
} from "./build-crystal-original-asset-manifest.mjs";

async function tempDirectory(prefix) {
  return fs.mkdtemp(path.join(os.tmpdir(), prefix));
}

async function removeLater(t, directory) {
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
}

test("manifest uses stable slash paths, sorted entries, and content root hash", async (t) => {
  const directory = await tempDirectory("mir2-original-manifest-");
  await removeLater(t, directory);
  const assetRoot = path.join(directory, "CrystalAssets");
  const output = path.join(directory, "evidence", "manifest.json");
  await fs.mkdir(path.join(assetRoot, "zeta"), { recursive: true });
  await fs.mkdir(path.join(assetRoot, "alpha"), { recursive: true });
  await fs.writeFile(path.join(assetRoot, "zeta", "two.bin"), Buffer.from("two"));
  await fs.writeFile(path.join(assetRoot, "alpha", "one.bin"), Buffer.from("one"));
  await fs.writeFile(path.join(assetRoot, "empty.bin"), Buffer.alloc(0));

  const first = await buildOriginalAssetManifest({
    assetRoot,
    output,
    generatedAt: "2026-08-24T00:00:00.000Z",
  });
  const files = first.manifest.files;
  assert.deepEqual(files.map((file) => file.path), ["alpha/one.bin", "empty.bin", "zeta/two.bin"]);
  assert.equal(first.manifest.rootName, "CrystalAssets");
  assert.equal(first.manifest.fileCount, 3);
  assert.equal(first.manifest.totalBytes, 6);
  assert.equal(first.manifest.rootSha256, computeRootSha256(files));
  const oneHash = crypto.createHash("sha256").update("one").digest("hex");
  const emptyHash = crypto.createHash("sha256").update(Buffer.alloc(0)).digest("hex");
  const twoHash = crypto.createHash("sha256").update("two").digest("hex");
  assert.equal(
    canonicalManifestEntries(files),
    `alpha/one.bin\t3\t${oneHash}\nempty.bin\t0\t${emptyHash}\nzeta/two.bin\t3\t${twoHash}\n`,
  );

  const second = await buildOriginalAssetManifest({
    assetRoot,
    output: path.join(directory, "evidence", "manifest-2.json"),
    generatedAt: "2026-08-25T00:00:00.000Z",
  });
  assert.equal(second.manifest.rootSha256, first.manifest.rootSha256);
  assert.notEqual(second.manifest.generatedAt, first.manifest.generatedAt);
});

test("manifest rejects an output inside the asset root", async (t) => {
  const directory = await tempDirectory("mir2-original-manifest-path-");
  await removeLater(t, directory);
  const assetRoot = path.join(directory, "assets");
  await fs.mkdir(assetRoot);
  await assert.rejects(
    buildOriginalAssetManifest({ assetRoot, output: path.join(assetRoot, "manifest.json") }),
    /outside the asset root/,
  );
});

test("include filters traversal to selected non-overlapping relative subdirectories", async (t) => {
  const directory = await tempDirectory("mir2-original-manifest-include-");
  await removeLater(t, directory);
  const assetRoot = path.join(directory, "CrystalClient");
  const output = path.join(directory, "manifest.json");
  await fs.mkdir(path.join(assetRoot, "Data", "nested"), { recursive: true });
  await fs.mkdir(path.join(assetRoot, "Map"), { recursive: true });
  await fs.mkdir(path.join(assetRoot, "Localization"), { recursive: true });
  await fs.mkdir(path.join(assetRoot, "Sound"), { recursive: true });
  await fs.writeFile(path.join(assetRoot, "Data", "items.dat"), "items");
  await fs.writeFile(path.join(assetRoot, "Data", "nested", "frames.dat"), "frames");
  await fs.writeFile(path.join(assetRoot, "Map", "b.map"), "map");
  await fs.writeFile(path.join(assetRoot, "Localization", "zh.txt"), "zh");
  await fs.writeFile(path.join(assetRoot, "Sound", "town.wav"), "sound");
  await fs.writeFile(path.join(assetRoot, "Error.txt"), "private log");
  await fs.writeFile(path.join(assetRoot, "Mir2Test.ini"), "private config");

  const result = await buildOriginalAssetManifest({
    assetRoot,
    output,
    includes: ["Sound", "Data", "Localization", "Map"],
  });
  assert.deepEqual(result.manifest.files.map((file) => file.path), [
    "Data/items.dat",
    "Data/nested/frames.dat",
    "Localization/zh.txt",
    "Map/b.map",
    "Sound/town.wav",
  ]);
  assert.equal(result.manifest.files.some((file) => file.path.includes("Error.txt")), false);
  assert.equal(result.manifest.files.some((file) => file.path.includes("Mir2Test.ini")), false);
});

test("include rejects duplicates, ancestor overlap, absolute paths, and backslashes", async (t) => {
  const directory = await tempDirectory("mir2-original-manifest-include-invalid-");
  await removeLater(t, directory);
  const assetRoot = path.join(directory, "assets");
  await fs.mkdir(path.join(assetRoot, "Data", "nested"), { recursive: true });
  const cases = [
    ["duplicate", ["Data", "Data"]],
    ["ancestor", ["Data", "Data/nested"]],
    ["absolute", [path.resolve(assetRoot, "Data")]],
    ["backslash", ["Data\\nested"]],
  ];
  for (const [label, includes] of cases) {
    await assert.rejects(
      buildOriginalAssetManifest({ assetRoot, output: path.join(directory, `${label}.json`), includes }),
      /include|relative path|overlap|unsafe/,
    );
  }
});

test("manifest rejects symlink or reparse entries", async (t) => {
  const directory = await tempDirectory("mir2-original-manifest-link-");
  await removeLater(t, directory);
  const assetRoot = path.join(directory, "assets");
  const target = path.join(directory, "outside.bin");
  const link = path.join(assetRoot, "linked.bin");
  await fs.mkdir(assetRoot);
  await fs.writeFile(target, "outside");
  try {
    await fs.symlink(target, link, "file");
  } catch (error) {
    t.skip(`symlink creation is unavailable in this environment: ${error.code ?? error.message}`);
    return;
  }
  await assert.rejects(
    buildOriginalAssetManifest({ assetRoot, output: path.join(directory, "manifest.json") }),
    /symlink\/reparse point/,
  );
});

test("manifest refuses to overwrite an existing output", async (t) => {
  const directory = await tempDirectory("mir2-original-manifest-overwrite-");
  await removeLater(t, directory);
  const assetRoot = path.join(directory, "assets");
  const output = path.join(directory, "manifest.json");
  await fs.mkdir(assetRoot);
  await fs.writeFile(output, "do not replace");
  await assert.rejects(
    buildOriginalAssetManifest({ assetRoot, output }),
    /refusing to overwrite existing output/,
  );
  assert.equal(await fs.readFile(output, "utf8"), "do not replace");
});
