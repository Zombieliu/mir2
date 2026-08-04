import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  DEFAULT_MAX_PAGE_PIXELS,
  MAX_SIZE,
  mapAtlasLibrarySupportsRawUpload,
  mapAtlasManifestFitsBudget,
  packIntoPages,
  removeStaleMapAtlasArtifacts,
} from "./build-map-atlas-pack.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const MANIFEST_PATH = path.resolve(
  SCRIPT_DIR,
  "../public/generated/map-atlas/manifest.json",
);
const requireManifest = process.argv.includes("--requireManifest");

test("floor atlases are split into bounded streaming pages", () => {
  const sources = Array.from({ length: 630 }, (_, index) => ({
    filePath: `fixture-${index}.png`,
    frame: String(index),
    width: 96,
    height: 64,
  }));

  const pages = packIntoPages(sources);
  assert.ok(pages.length > 1);
  assert.ok(
    pages.every(
      (page) =>
        page.width <= MAX_SIZE &&
        page.height <= MAX_SIZE &&
        page.width * page.height <= DEFAULT_MAX_PAGE_PIXELS,
    ),
  );
});

test("dev ensure rejects stale oversized manifests", () => {
  assert.equal(
    mapAtlasManifestFitsBudget({
      schemaVersion: 2,
      pages: [{ w: 1024, h: 256, b: 100, l: "WemadeMir2/Tiles", p: 0, u: "/x", r: [] }],
    }),
    true,
  );
  assert.equal(
    mapAtlasManifestFitsBudget({
      schemaVersion: 2,
      pages: [{ w: 1024, h: 8192, b: 100, l: "WemadeMir2/Tiles", p: 0, u: "/x", r: [] }],
    }),
    false,
  );
  assert.equal(mapAtlasManifestFitsBudget({ schemaVersion: 1, atlases: [] }), false);
  assert.equal(mapAtlasManifestFitsBudget(null), false);
});

test("the packed index contains only raw-upload-safe floor libraries", () => {
  assert.equal(mapAtlasLibrarySupportsRawUpload("WemadeMir2/Tiles"), true);
  assert.equal(mapAtlasLibrarySupportsRawUpload("WemadeMir2/SmTiles"), true);
  assert.equal(mapAtlasLibrarySupportsRawUpload("WemadeMir3/Sand/Tiles5c"), true);
  assert.equal(mapAtlasLibrarySupportsRawUpload("WemadeMir2/Objects"), false);
  assert.equal(mapAtlasLibrarySupportsRawUpload("WemadeMir3/Sand/Dungeonsc"), false);
});

test("repeat builds remove only stale content-addressed atlas artifacts", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-map-atlas-clean-"));
  const libraryDir = path.join(root, "WemadeMir2-Tiles");
  await fs.mkdir(libraryDir, { recursive: true });
  const staleManifest = path.join(root, `manifest.${"a".repeat(64)}.json`);
  const stalePage = path.join(libraryDir, `p0.${"b".repeat(16)}.png`);
  const stableManifest = path.join(root, "manifest.json");
  const unrelated = path.join(libraryDir, "notes.txt");
  await Promise.all([
    fs.writeFile(staleManifest, "{}"),
    fs.writeFile(stalePage, "page"),
    fs.writeFile(stableManifest, "{}"),
    fs.writeFile(unrelated, "keep"),
  ]);

  try {
    assert.equal(await removeStaleMapAtlasArtifacts(root), 2);
    await assert.rejects(fs.access(staleManifest), { code: "ENOENT" });
    await assert.rejects(fs.access(stalePage), { code: "ENOENT" });
    await fs.access(stableManifest);
    await fs.access(unrelated);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
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

  const oversized = manifest.pages.filter(
    (page) => page.w > MAX_SIZE || page.h > MAX_SIZE || page.b <= 0,
  );
  assert.deepEqual(
    oversized.map(({ l, p, w, h, b }) => ({ l, p, w, h, b })),
    [],
  );
});
