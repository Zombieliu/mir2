#!/usr/bin/env node

import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { verifyMonsterFrameClosure } from "./verify-monster-frame-closure.mjs";

async function writeLibrary(root, { frameCount, pngCount, actions }) {
  const libraryDir = path.join(root, "Monster", "900");
  await mkdir(libraryDir, { recursive: true });
  await writeFile(
    path.join(libraryDir, "meta.json"),
    JSON.stringify({
      count: frameCount,
      frames: Array.from({ length: frameCount }, () => ({})),
      frameSet: { actions },
    }),
  );
  await Promise.all(
    Array.from({ length: pngCount }, (_, index) =>
      writeFile(path.join(libraryDir, `${index}.png`), "png"),
    ),
  );
}

const completeActions = [
  { actionName: "Standing", start: 0, count: 1 },
  { actionName: "Die", start: 1, count: 2 },
  { actionName: "Dead", start: 2, count: 1 },
];

test("accepts a contiguous library with complete actions", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "monster-closure-ok-"));
  await writeLibrary(root, { frameCount: 3, pngCount: 3, actions: completeActions });

  const result = await verifyMonsterFrameClosure({
    assetRoot: root,
    libraries: ["Monster/900"],
  });

  assert.deepEqual(result, [{ library: "Monster/900", frameCount: 3, actionCount: 3 }]);
});

test("rejects truncated PNGs and an empty frameSet", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "monster-closure-bad-"));
  await writeLibrary(root, { frameCount: 3, pngCount: 1, actions: [] });

  await assert.rejects(
    verifyMonsterFrameClosure({ assetRoot: root, libraries: ["Monster/900"] }),
    /PNG count 1 does not match meta\.count 3[\s\S]*frameSet\.actions is empty/u,
  );
});
