import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  MAX_SIZE,
  mapAtlasManifestFitsBudget,
  packIntoPages,
} from "./build-map-atlas-pack.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const MANIFEST_PATH = path.resolve(
  SCRIPT_DIR,
  "../public/generated/map-atlas/manifest.json",
);
const requireManifest = process.argv.includes("--requireManifest");

test("an exactly full final shelf remains a 4096px atlas page", () => {
  const sources = Array.from({ length: 630 }, (_, index) => ({
    filePath: `fixture-${index}.png`,
    frame: String(index),
    width: 96,
    height: 64,
  }));

  const pages = packIntoPages(sources);
  assert.ok(pages.length >= 1);
  assert.equal(pages[0].height, MAX_SIZE);
  assert.ok(
    pages.every((page) => page.width <= MAX_SIZE && page.height <= MAX_SIZE),
  );
});

test("dev ensure rejects stale oversized manifests", () => {
  assert.equal(
    mapAtlasManifestFitsBudget({ atlases: [{ width: 1024, height: 4096 }] }),
    true,
  );
  assert.equal(
    mapAtlasManifestFitsBudget({ atlases: [{ width: 1024, height: 8192 }] }),
    false,
  );
  assert.equal(mapAtlasManifestFitsBudget({ atlases: [] }), false);
  assert.equal(mapAtlasManifestFitsBudget(null), false);
});

test("the generated map atlas never exceeds the WebGL2 compatibility budget", async (context) => {
  let manifest;
  try {
    manifest = JSON.parse(await fs.readFile(MANIFEST_PATH, "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT" && !requireManifest) {
      context.skip(
        "generated map atlas is absent; build creates it before release verification",
      );
      return;
    }
    throw error;
  }

  const oversized = manifest.atlases.filter(
    (atlas) => atlas.width > MAX_SIZE || atlas.height > MAX_SIZE,
  );
  assert.deepEqual(
    oversized.map(({ key, width, height }) => ({ key, width, height })),
    [],
  );
});
