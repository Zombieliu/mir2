import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import {
  assertItemIconClosure, collectItemIconRequirements, DEFAULT_ITEM_MANIFEST_PATH,
  inspectItemIconClosure, sha256,
} from "./asset-pipeline/item-icon-closure.mjs";
import { encodePng } from "./crystal-library.mjs";

function item(overrides = {}) {
  return { item_index: 45, image: 595, item_type: 2, shape: 0, stack_size: 1, ...overrides };
}

function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), "mir2-catalogue-item-icons-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const options = { itemManifestPath: path.join(root, "items.json"), itemIconRoot: path.join(root, "Items") };
  mkdirSync(options.itemIconRoot);
  const catalogue = { items: [
    item({ item_index: 45, name: "MirArmour(M)", image: 595 }),
    item({ item_index: 46, name: "MirArmour(M)1", image: 595 }),
    item({ item_index: 51, name: "MirArmour(F)", image: 605 }),
  ] };
  const catalogueBytes = Buffer.from(JSON.stringify(catalogue));
  writeFileSync(options.itemManifestPath, catalogueBytes);
  const rgba = Buffer.from([170, 90, 30, 255, 0, 0, 0, 0]);
  const png = encodePng(2, 1, rgba);
  const meta = {
    sourceLibrary: { path: "Items.Lib", sha256: sha256(Buffer.from("fixture")), bytes: 7 },
    itemCatalogue: {
      sha256: sha256(catalogueBytes), itemCount: 3,
      catalogueImageCount: 2, stackImageCount: 0, uniqueImageCount: 2,
    },
    frames: [595, 605].map((index) => ({
      index, width: 2, height: 1, x: 0, y: 0,
      path: `/original-ui/Items/${index}.png`, rgbaSha256: sha256(rgba), pngSha256: sha256(png),
    })),
  };
  const writeMeta = () => writeFileSync(path.join(options.itemIconRoot, "meta.json"), JSON.stringify(meta));
  for (const index of [595, 605]) writeFileSync(path.join(options.itemIconRoot, `${index}.png`), png);
  writeMeta();
  return { options, catalogue, meta, writeMeta };
}

test("all catalogue variants use Image, never item index or a name guess", () => {
  const result = collectItemIconRequirements({ items: [
    item({ item_index: 51, name: "same name", image: 605 }),
    item({ item_index: 45, name: "same name", image: 595 }),
    item({ item_index: 46, name: "level/class variant", image: 595 }),
  ] });
  assert.deepEqual(result.uniqueImages, [595, 605]);
  assert.deepEqual(result.requirements.map((item) => item.itemIndex), [45, 46, 51]);
});

test("UserItem.Image includes every Amulet and Poison count threshold without name guesses", () => {
  const result = collectItemIconRequirements({ items: [
    item({ item_index: 710, name: "arbitrary", item_type: 8, shape: 1, stack_size: 500, image: 259 }),
    item({ item_index: 711, name: "arbitrary", item_type: 8, shape: 2, stack_size: 500, image: 258 }),
    item({ item_index: 712, name: "arbitrary", item_type: 8, shape: 0, stack_size: 500, image: 270 }),
  ] });
  assert.deepEqual(result.uniqueCatalogueImages, [258, 259, 270]);
  assert.deepEqual(result.uniqueStackImages, [2960, 2961, 3660, 3661, 3662, 3670, 3671, 3672, 3673, 3674, 3675]);
  assert.equal(result.uniqueImages.length, 14);
  assert.deepEqual(result.requirements.find((entry) => entry.itemIndex === 710).stackImages, [3673, 3674, 2960, 3675]);
  for (const source of [
    item({ name: "Amulet Poison", item_type: 7, shape: 0, stack_size: 500 }),
    item({ name: "Amulet", item_type: 8, shape: 0, stack_size: 0 }),
    item({ name: "AmuletOfRevival", item_type: 8, shape: 3, stack_size: 5 }),
  ]) {
    assert.deepEqual(collectItemIconRequirements({ items: [source] }).uniqueImages, [595]);
  }
});

test("missing, duplicate or invalid catalogue fields fail closed", () => {
  for (const catalogue of [
    {}, { items: [] }, { items: [{ item_index: 45 }] },
    { items: [item({ image: "595" })] },
    { items: [item({ image: -1 })] },
    { items: [item({ image: 65536 })] },
    { items: [item({ item_type: undefined })] },
    { items: [item({ item_type: "8" })] },
    { items: [item({ shape: undefined })] },
    { items: [item({ shape: 32768 })] },
    { items: [item({ stack_size: undefined })] },
    { items: [item({ stack_size: -1 })] },
    { items: [item(), item({ image: 605 })] },
  ]) assert.throws(() => collectItemIconRequirements(catalogue));
});

test("closure validates all images, metadata, decoded pixels and both identities", async (t) => {
  const { options } = fixture(t);
  const report = await assertItemIconClosure(options);
  assert.equal(report.itemCount, 3);
  assert.equal(report.uniqueImageCount, 2);
  assert.deepEqual(report.uniqueImages, [595, 605]);
});

test("dropping both file and meta cannot shrink the catalogue denominator", async (t) => {
  const { options, meta, writeMeta } = fixture(t);
  rmSync(path.join(options.itemIconRoot, "605.png"));
  meta.frames = meta.frames.filter((frame) => frame.index !== 605);
  writeMeta();
  const report = await inspectItemIconClosure(options);
  assert.deepEqual(report.missingFiles, [605]);
  assert.deepEqual(report.missingMetadata, [605]);
  await assert.rejects(assertItemIconClosure(options), /closure is incomplete/);
});

test("renamed, duplicate and wrong-sized metadata is rejected", async (t) => {
  const { options, meta, writeMeta } = fixture(t);
  meta.frames[0].path = "/original-ui/Items/605.png";
  meta.frames[1].width = 3;
  meta.frames.push({ ...meta.frames[1] });
  writeMeta();
  const report = await inspectItemIconClosure(options);
  assert(report.invalidMetadata.some((entry) => entry.reason === "invalid-frame-path-or-geometry"));
  assert(report.invalidMetadata.some((entry) => entry.reason === "invalid-or-duplicate-frame-index"));
  assert(report.invalidImages.some((entry) => entry.reason === "png-metadata-geometry-mismatch"));
});

test("a same-sized substitute image and corrupt PNG both fail", async (t) => {
  const { options } = fixture(t);
  writeFileSync(path.join(options.itemIconRoot, "595.png"), encodePng(2, 1, Buffer.alloc(8)));
  writeFileSync(path.join(options.itemIconRoot, "605.png"), "not a PNG");
  const report = await inspectItemIconClosure(options);
  assert(report.invalidImages.some((entry) => entry.index === 595 && entry.reason === "source-pixel-hash-mismatch"));
  assert(report.invalidImages.some((entry) => entry.index === 605 && entry.reason === "invalid-png"));
});

test("source fingerprints and exact catalogue identity cannot silently disappear", async (t) => {
  const { options, meta, writeMeta } = fixture(t);
  delete meta.sourceLibrary;
  delete meta.frames[0].rgbaSha256;
  meta.itemCatalogue.sha256 = "0".repeat(64);
  writeMeta();
  const report = await inspectItemIconClosure(options);
  assert(report.invalidMetadata.some((entry) => entry.reason === "missing-source-library-identity"));
  assert(report.invalidMetadata.some((entry) => entry.reason === "catalogue-identity-or-denominator-mismatch"));
  assert(report.invalidImages.some((entry) => entry.reason === "source-pixel-hash-mismatch"));
});

test("a new catalogue image requires a new export", async (t) => {
  const { options, catalogue } = fixture(t);
  catalogue.items.push(item({ item_index: 2000, name: "new variant", image: 700 }));
  writeFileSync(options.itemManifestPath, JSON.stringify(catalogue));
  const report = await inspectItemIconClosure(options);
  assert.deepEqual(report.missingFiles, [700]);
  assert.deepEqual(report.missingMetadata, [700]);
  assert(report.invalidMetadata.some((entry) => entry.reason === "catalogue-identity-or-denominator-mismatch"));
});

test("count-derived images cannot disappear just because every base image exists", async (t) => {
  const { options, catalogue, meta, writeMeta } = fixture(t);
  Object.assign(catalogue.items[0], { item_type: 8, shape: 0, stack_size: 500 });
  const catalogueBytes = Buffer.from(JSON.stringify(catalogue));
  writeFileSync(options.itemManifestPath, catalogueBytes);
  Object.assign(meta.itemCatalogue, {
    sha256: sha256(catalogueBytes), stackImageCount: 3, uniqueImageCount: 5,
  });
  writeMeta();
  const report = await inspectItemIconClosure(options);
  assert.deepEqual(report.invalidMetadata, []);
  assert.deepEqual(report.missingFiles, [3660, 3661, 3662]);
  assert.deepEqual(report.missingMetadata, [3660, 3661, 3662]);
  await assert.rejects(assertItemIconClosure(options), /closure is incomplete/);
});

test("checked-in catalogue has complete source-fingerprinted icon coverage", async () => {
  const report = await assertItemIconClosure();
  const catalogue = JSON.parse(readFileSync(DEFAULT_ITEM_MANIFEST_PATH, "utf8"));
  assert.equal(report.itemCount, catalogue.items.length);
  assert.equal(report.requirements.find((item) => item.itemIndex === 45).image, 595);
  assert.equal(report.requirements.find((item) => item.itemIndex === 51).image, 605);
  assert.equal(report.stackImageCount, 11);
});
