import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  canonicalJson,
  compilePackCatalog,
  computeSourceSnapshotContentHash,
  runPackCatalogCli,
  validatePackCatalogBundle,
  validateSourceSnapshot,
} from "./pack-catalog.mjs";

function action(overrides = {}) {
  return {
    actionId: 0,
    actionName: "Standing",
    start: 0,
    count: 4,
    skip: 0,
    interval: 100,
    effectStart: 0,
    effectCount: 0,
    effectSkip: 0,
    effectInterval: 0,
    reverse: false,
    blend: false,
    ...overrides,
  };
}

function library(libraryPath, overrides = {}) {
  return {
    path: libraryPath,
    status: "ok",
    byteLength: 100,
    sha256: "a".repeat(64),
    version: 3,
    frameSlotCount: 10,
    presentFrameCount: 8,
    emptyFrameCount: 2,
    invalidFrameOffsetCount: 0,
    frameSeek: 80,
    frameSet: { seek: 80, count: 1, actions: [action()] },
    issues: [],
    ...overrides,
  };
}

function snapshot(libraries = [library("Monster/001.Lib"), library("Map/2.Lib")]) {
  const body = {
    schemaVersion: 1,
    sourceKind: "crystal-client-data",
    sourceLayout: "Crystal/Build/Client/Debug/Data",
    hashAlgorithm: "sha256",
    summary: {
      libraryCount: libraries.length,
      parsedLibraryCount: libraries.length,
      failedLibraryCount: 0,
      sourceBytes: libraries.reduce((sum, item) => sum + item.byteLength, 0),
      frameSlotCount: libraries.reduce((sum, item) => sum + item.frameSlotCount, 0),
      presentFrameCount: libraries.reduce((sum, item) => sum + item.presentFrameCount, 0),
      emptyFrameCount: libraries.reduce((sum, item) => sum + item.emptyFrameCount, 0),
      invalidFrameOffsetCount: libraries.reduce((sum, item) => sum + item.invalidFrameOffsetCount, 0),
      versionCounts: { "3": libraries.length },
      frameSetLibraryCount: libraries.filter((item) => item.frameSet.count > 0).length,
      actionCount: libraries.reduce((sum, item) => sum + item.frameSet.count, 0),
      unknownActionCount: 0,
      duplicateActionCount: 0,
      issueCount: 0,
    },
    libraries,
  };
  return { ...body, contentHash: computeSourceSnapshotContentHash(body) };
}

function rehash(value) {
  const copy = structuredClone(value);
  copy.contentHash = computeSourceSnapshotContentHash(copy);
  return copy;
}

test("byte-identical compiler and CLI reruns", async () => {
  const source = snapshot();
  assert.equal(canonicalJson(compilePackCatalog(source)), canonicalJson(compilePackCatalog(source)));

  const directory = await mkdtemp(path.join(os.tmpdir(), "mir2-pack-catalog-"));
  try {
    const input = path.join(directory, "snapshot.json");
    const first = path.join(directory, "first.json");
    const second = path.join(directory, "second.json");
    await writeFile(input, JSON.stringify(source), "utf8");
    await runPackCatalogCli(["--input", input, "--output", first]);
    await runPackCatalogCli(["--input", input, "--output", second]);
    assert.deepEqual(await readFile(first), await readFile(second));
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("source metadata and complete FrameSet actions affect hashes", () => {
  const baseline = compilePackCatalog(snapshot());
  const changedLibrary = library("Monster/001.Lib", {
    byteLength: 101,
    sha256: "b".repeat(64),
    frameSet: { seek: 80, count: 1, actions: [action({ interval: 101, blend: true })] },
  });
  const changed = compilePackCatalog(snapshot([changedLibrary, library("Map/2.Lib")]));
  assert.notEqual(baseline.packs.find((pack) => pack.category === "entities").contentHash,
    changed.packs.find((pack) => pack.category === "entities").contentHash);
  assert.notEqual(baseline.catalog.contentHash, changed.catalog.contentHash);
  assert.notEqual(baseline.release.contentHash, changed.release.contentHash);
  assert.deepEqual(changed.packs.find((pack) => pack.category === "entities").libraries[0].frameSet.actions[0],
    changedLibrary.frameSet.actions[0]);
});

test("input path ordering does not affect output bytes", () => {
  const libraries = [library("NPC/010.Lib"), library("Map/2.Lib"), library("Monster/001.Lib")];
  const forward = compilePackCatalog(snapshot(libraries));
  const reverse = compilePackCatalog(snapshot([...libraries].reverse()));
  assert.equal(canonicalJson(forward), canonicalJson(reverse));
  for (const pack of forward.packs) {
    assert.deepEqual(pack.libraries.map((item) => item.path), [...pack.libraries.map((item) => item.path)].sort());
  }
});

test("missing and corrupt snapshot fields are rejected", () => {
  const missingHash = snapshot();
  delete missingHash.libraries[0].sha256;
  assert.throws(() => validateSourceSnapshot(missingHash), /sha256/);

  const invalidCounts = snapshot();
  invalidCounts.libraries[0].presentFrameCount = 9;
  assert.throws(() => validateSourceSnapshot(rehash(invalidCounts)), /frame counts/);

  const missingActions = snapshot();
  delete missingActions.libraries[0].frameSet.actions;
  assert.throws(() => validateSourceSnapshot(rehash(missingActions)), /actions/);

  const corruptSnapshotHash = snapshot();
  corruptSnapshotHash.contentHash = "0".repeat(64);
  assert.throws(() => compilePackCatalog(corruptSnapshotHash), /contentHash mismatch/);
});

test("all generated content hashes and dependency hashes are validated", () => {
  const bundle = compilePackCatalog(snapshot());
  assert.equal(validatePackCatalogBundle(bundle), true);

  const corruptPack = structuredClone(bundle);
  corruptPack.packs[0].libraries[0].byteLength += 1;
  assert.throws(() => validatePackCatalogBundle(corruptPack), /contentHash mismatch/);

  const corruptDependency = structuredClone(bundle);
  corruptDependency.catalog.dependencies[0].contentHash = "f".repeat(64);
  assert.throws(() => validatePackCatalogBundle(corruptDependency), /contentHash mismatch/);
});
