import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { gzipSync } from "node:zlib";

import sharp from "sharp";

import { compileEntityPack, validateEntityPack } from "./compile-entity-pack.mjs";

test("direct .Lib entity pack is deterministic, semantic, masked, and guttered", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "mir2-entity-pack-"));
  const dataDir = path.join(root, "Data");
  const libraryDir = path.join(dataDir, "NPC");
  await mkdir(libraryDir, { recursive: true });
  const libraryPath = path.join(libraryDir, "00.Lib");
  await writeFile(libraryPath, syntheticLibrary(-2));

  try {
    const firstDir = path.join(root, "first");
    const secondDir = path.join(root, "second");
    const input = { dataDir, packId: "test", libraries: ["NPC/00"], urlRoot: "/test" };
    const first = await compileEntityPack({ ...input, outputDir: firstDir });
    const second = await compileEntityPack({ ...input, outputDir: secondDir });
    assert.deepEqual(first, second);
    assert.equal(validateEntityPack(first), true);
    assert.equal(first.summary.frameCount, 1);
    assert.equal(first.summary.maskCount, 1);
    assert.equal(first.summary.actionCount, 1);
    assert.equal(first.libraries["NPC/00"].frameSet.actions[0].actionName, "Standing");
    assert.equal(first.libraries["NPC/00"].frames[0].x, -2);
    assert.ok(first.libraries["NPC/00"].frames[0].maskRectKey);

    const page = first.pages[0];
    const pageFile = path.join(firstDir, "pages", `${page.sha256}.png`);
    const secondPageFile = path.join(secondDir, "pages", `${page.sha256}.png`);
    assert.deepEqual(await readFile(pageFile), await readFile(secondPageFile));
    const rect = page.rects.find((candidate) => candidate.key === "NPC/00#0");
    const { data, info } = await sharp(pageFile).raw().toBuffer({ resolveWithObject: true });
    assert.deepEqual(pixel(data, info.width, rect.x - 1, rect.y), pixel(data, info.width, rect.x, rect.y));
    assert.deepEqual(pixel(data, info.width, rect.x, rect.y - 1), pixel(data, info.width, rect.x, rect.y));

    await writeFile(libraryPath, syntheticLibrary(-1));
    const changed = await compileEntityPack({ ...input, outputDir: path.join(root, "changed") });
    assert.notEqual(changed.contentHash, first.contentHash, "frame offsets participate in the semantic hash");
    assert.equal(changed.pages[0].sha256, first.pages[0].sha256, "unchanged pixels retain their CAS page hash");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

function syntheticLibrary(x) {
  const rgba = Buffer.from([
    1, 2, 3, 255, 11, 12, 13, 255,
    21, 22, 23, 255, 31, 32, 33, 255,
  ]);
  const maskRgba = Buffer.from([
    101, 102, 103, 255, 111, 112, 113, 255,
    121, 122, 123, 255, 131, 132, 133, 255,
  ]);
  const image = gzipSync(rgbaToBgra(rgba));
  const mask = gzipSync(rgbaToBgra(maskRgba));
  const imageHeader = Buffer.alloc(17);
  let offset = 0;
  imageHeader.writeInt16LE(2, offset); offset += 2;
  imageHeader.writeInt16LE(2, offset); offset += 2;
  imageHeader.writeInt16LE(x, offset); offset += 2;
  imageHeader.writeInt16LE(-3, offset); offset += 2;
  imageHeader.writeInt16LE(4, offset); offset += 2;
  imageHeader.writeInt16LE(5, offset); offset += 2;
  imageHeader.writeUInt8(0x80, offset); offset += 1;
  imageHeader.writeInt32LE(image.length, offset);
  const maskHeader = Buffer.alloc(12);
  maskHeader.writeInt16LE(2, 0);
  maskHeader.writeInt16LE(2, 2);
  maskHeader.writeInt16LE(0, 4);
  maskHeader.writeInt16LE(0, 6);
  maskHeader.writeInt32LE(mask.length, 8);
  const imageBlock = Buffer.concat([imageHeader, image, maskHeader, mask]);
  const headerSize = 16;
  const frameSeek = headerSize + imageBlock.length;
  const header = Buffer.alloc(headerSize);
  header.writeInt32LE(3, 0);
  header.writeInt32LE(1, 4);
  header.writeInt32LE(frameSeek, 8);
  header.writeInt32LE(headerSize, 12);
  const frameSet = Buffer.alloc(39);
  frameSet.writeInt32LE(1, 0);
  frameSet.writeUInt8(0, 4);
  frameSet.writeInt32LE(0, 5);
  frameSet.writeInt32LE(1, 9);
  frameSet.writeInt32LE(-1, 13);
  frameSet.writeInt32LE(500, 17);
  return Buffer.concat([header, imageBlock, frameSet]);
}

function rgbaToBgra(rgba) {
  const bgra = Buffer.alloc(rgba.length);
  for (let offset = 0; offset < rgba.length; offset += 4) {
    bgra[offset] = rgba[offset + 2];
    bgra[offset + 1] = rgba[offset + 1];
    bgra[offset + 2] = rgba[offset];
    bgra[offset + 3] = rgba[offset + 3];
  }
  return bgra;
}

function pixel(data, width, x, y) {
  const offset = (y * width + x) * 4;
  return [...data.subarray(offset, offset + 4)];
}
