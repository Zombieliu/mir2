import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { gzipSync } from "node:zlib";

import sharp from "sharp";

import {
  buildFullAssetPacks,
  planFullAssetPacks,
  pruneFullAssetPacks,
  validateFullPackIndex,
  validateLibraryPack,
  verifyFullAssetPacks,
} from "./compile-full-asset-packs.mjs";

test("full Crystal pack classifies every slot, preserves masks, resumes, and verifies CAS", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "mir2-full-pack-"));
  try {
    const dataDir = path.join(root, "Data");
    await mkdir(path.join(dataDir, "NPC"), { recursive: true });
    const source = syntheticLibrary();
    const sourcePath = path.join(dataDir, "NPC", "00.Lib");
    await writeFile(sourcePath, source);
    const catalogPath = path.join(root, "catalog.json");
    await writeFile(catalogPath, JSON.stringify(syntheticCatalog(source)));

    const outputDir = path.join(root, "output");
    const reportPath = path.join(root, "report.json");
    const options = {
      dataDir,
      catalogPath,
      outputDir,
      reportPath,
      urlRoot: "/test/full",
      pageSize: 64,
      padding: 1,
      compressionLevel: 1,
      jobs: 1,
      resume: true,
      verifyPages: true,
    };

    const planned = await planFullAssetPacks(options);
    assert.equal(planned.plan.summary.libraryCount, 1);
    assert.equal(planned.plan.summary.frameSlotCount, 3);
    assert.equal(planned.plan.summary.packedFrameCount, 1);
    assert.equal(planned.plan.summary.noDrawFrameCount, 2);
    assert.equal(planned.plan.summary.packedMaskCount, 1);
    assert.equal(planned.plan.summary.noDrawMaskCount, 1);
    assert.equal(planned.plan.summary.pageCount, 1);

    const first = await buildFullAssetPacks(options);
    assert.equal(validateFullPackIndex(first.index), true);
    assert.equal(first.consoleSummary.builtLibraryCount, 1);
    assert.equal(first.consoleSummary.resumedLibraryCount, 0);

    const record = first.index.librariesByKey["NPC/00"];
    const manifestPath = fileForUrl(outputDir, options.urlRoot, record.manifestUrl);
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    assert.equal(validateLibraryPack(manifest), true);
    assert.deepEqual(manifest.frames.map((frame) => frame.status), ["packed", "noDraw", "noDraw"]);
    assert.equal(manifest.frames[0].mask.status, "packed");
    assert.equal(manifest.frames[1].reason, "empty-offset");
    assert.equal(manifest.frames[2].reason, "non-positive-dimensions");
    assert.equal(manifest.frames[2].mask.status, "noDraw");
    assert.equal(manifest.frameSet.actions[0].actionName, "Standing");

    const page = manifest.pages[0];
    const pagePath = fileForUrl(outputDir, options.urlRoot, page.imageUrl);
    const decoded = await sharp(pagePath).raw().toBuffer({ resolveWithObject: true });
    const rect = manifest.frames[0].image;
    assert.deepEqual(pixel(decoded.data, decoded.info.width, rect.x, rect.y), [1, 2, 3, 255]);
    assert.deepEqual(
      pixel(decoded.data, decoded.info.width, rect.x - 1, rect.y),
      pixel(decoded.data, decoded.info.width, rect.x, rect.y),
    );
    assert.deepEqual(
      pixel(decoded.data, decoded.info.width, rect.x, rect.y - 1),
      pixel(decoded.data, decoded.info.width, rect.x, rect.y),
    );

    const resumed = await buildFullAssetPacks(options);
    assert.equal(resumed.consoleSummary.resumedLibraryCount, 1);
    assert.equal(resumed.index.contentHash, first.index.contentHash);

    const second = await buildFullAssetPacks({ ...options, outputDir: path.join(root, "second") });
    assert.equal(second.index.contentHash, first.index.contentHash);
    assert.deepEqual(await readFile(fileForUrl(path.join(root, "second"), options.urlRoot, page.imageUrl)), await readFile(pagePath));

    const verified = await verifyFullAssetPacks(options);
    assert.equal(verified.consoleSummary.verifiedLibraryCount, 1);
    assert.equal(verified.consoleSummary.verifiedUniquePageCount, 1);

    const orphanManifestPath = path.join(outputDir, "libraries", "entities", "orphan.json");
    const orphanPagePath = path.join(outputDir, "pages", "ff", "orphan.png");
    await mkdir(path.dirname(orphanManifestPath), { recursive: true });
    await mkdir(path.dirname(orphanPagePath), { recursive: true });
    await writeFile(orphanManifestPath, "orphan manifest");
    await writeFile(orphanPagePath, "orphan page");

    const pruneDryRun = await pruneFullAssetPacks({ ...options, apply: false });
    assert.equal(pruneDryRun.consoleSummary.applied, false);
    assert.equal(pruneDryRun.consoleSummary.orphanManifestCount, 1);
    assert.equal(pruneDryRun.consoleSummary.orphanPageCount, 1);
    assert.equal(await readFile(orphanManifestPath, "utf8"), "orphan manifest");

    const pruned = await pruneFullAssetPacks({ ...options, apply: true });
    assert.equal(pruned.consoleSummary.applied, true);
    assert.equal(pruned.consoleSummary.reclaimedBytes, 26);
    await assert.rejects(() => readFile(orphanManifestPath), { code: "ENOENT" });
    await assert.rejects(() => readFile(orphanPagePath), { code: "ENOENT" });
    assert.ok((await readFile(manifestPath)).byteLength > 0);
    assert.ok((await readFile(pagePath)).byteLength > 0);

    const corrupt = Buffer.from(await readFile(pagePath));
    corrupt[corrupt.length - 1] ^= 0xff;
    await writeFile(pagePath, corrupt);
    await assert.rejects(() => verifyFullAssetPacks(options), /CAS page hash mismatch/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

function syntheticCatalog(source) {
  return {
    schemaVersion: 1,
    catalog: { contentHash: "catalog-test" },
    packs: [
      {
        id: "crystal-entities",
        category: "entities",
        libraries: [
          {
            path: "NPC/00.Lib",
            status: "ok",
            byteLength: source.byteLength,
            sha256: sha256(source),
            version: 3,
            frameSlotCount: 3,
            presentFrameCount: 2,
          },
        ],
      },
    ],
  };
}

function syntheticLibrary() {
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
  const drawableHeader = imageHeader({ width: 2, height: 2, x: -2, y: -3, shadow: 0x80, length: image.length });
  const maskHeader = Buffer.alloc(12);
  maskHeader.writeInt16LE(2, 0);
  maskHeader.writeInt16LE(2, 2);
  maskHeader.writeInt16LE(4, 4);
  maskHeader.writeInt16LE(5, 6);
  maskHeader.writeInt32LE(mask.length, 8);
  const drawable = Buffer.concat([drawableHeader, image, maskHeader, mask]);

  const noDrawHeader = imageHeader({ width: 0, height: 2, x: 7, y: 8, shadow: 0x80, length: 0 });
  const noDrawMaskHeader = Buffer.alloc(12);
  noDrawMaskHeader.writeInt16LE(2, 0);
  noDrawMaskHeader.writeInt16LE(2, 2);
  noDrawMaskHeader.writeInt16LE(0, 4);
  noDrawMaskHeader.writeInt16LE(0, 6);
  noDrawMaskHeader.writeInt32LE(0, 8);
  const noDraw = Buffer.concat([noDrawHeader, noDrawMaskHeader]);

  const headerBytes = 12 + 3 * 4;
  const drawableOffset = headerBytes;
  const noDrawOffset = drawableOffset + drawable.length;
  const frameSeek = noDrawOffset + noDraw.length;
  const header = Buffer.alloc(headerBytes);
  header.writeInt32LE(3, 0);
  header.writeInt32LE(3, 4);
  header.writeInt32LE(frameSeek, 8);
  header.writeInt32LE(drawableOffset, 12);
  header.writeInt32LE(0, 16);
  header.writeInt32LE(noDrawOffset, 20);

  const frameSet = Buffer.alloc(39);
  frameSet.writeInt32LE(1, 0);
  frameSet.writeUInt8(0, 4);
  frameSet.writeInt32LE(0, 5);
  frameSet.writeInt32LE(1, 9);
  frameSet.writeInt32LE(-1, 13);
  frameSet.writeInt32LE(500, 17);
  return Buffer.concat([header, drawable, noDraw, frameSet]);
}

function imageHeader({ width, height, x, y, shadow, length }) {
  const header = Buffer.alloc(17);
  let offset = 0;
  header.writeInt16LE(width, offset); offset += 2;
  header.writeInt16LE(height, offset); offset += 2;
  header.writeInt16LE(x, offset); offset += 2;
  header.writeInt16LE(y, offset); offset += 2;
  header.writeInt16LE(4, offset); offset += 2;
  header.writeInt16LE(5, offset); offset += 2;
  header.writeUInt8(shadow, offset); offset += 1;
  header.writeInt32LE(length, offset);
  return header;
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

function fileForUrl(outputDir, urlRoot, url) {
  return path.join(outputDir, ...url.slice(`${urlRoot}/`.length).split("/"));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function pixel(data, width, x, y) {
  const offset = (y * width + x) * 4;
  return [...data.subarray(offset, offset + 4)];
}
