import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import sharp from "sharp";

const publicRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../public");
const manifestPath = path.join(publicRoot, "generated/map-atlas/manifest.json");
const manifestBytes = await fs.readFile(manifestPath);
const manifest = JSON.parse(manifestBytes);
assert.equal(manifest.edgeExtrusion, 1, "manifest must declare owned one-pixel borders");
let frames = 0;
let sourcePixels = 0;
let gutterPixels = 0;
for (const page of manifest.pages) {
  const imagePath = path.resolve(publicRoot, page.u.replace(/^\/+/, ""));
  assert.ok(imagePath.startsWith(`${publicRoot}${path.sep}`));
  const encoded = await fs.readFile(imagePath);
  assert.equal(encoded.length, page.b);
  assert.ok(path.basename(imagePath).includes(createHash("sha256").update(encoded).digest("hex").slice(0, 16)));
  const atlas = await sharp(encoded).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  assert.equal(atlas.info.width, page.w);
  assert.equal(atlas.info.height, page.h);
  for (const [frame, x, y, width, height] of page.r) {
    const sourcePath = path.resolve(publicRoot, "original-map", page.l, `${frame}.png`);
    assert.ok(sourcePath.startsWith(`${publicRoot}${path.sep}`));
    const source = await sharp(sourcePath).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    assert.equal(source.info.width, width);
    assert.equal(source.info.height, height);
    for (let row = -1; row <= height; row += 1) {
      const sourceY = Math.max(0, Math.min(height - 1, row));
      const destination = ((y + row) * page.w + x) * 4;
      const sourceStart = sourceY * width * 4;
      assert.ok(atlas.data.subarray(destination, destination + width * 4)
        .equals(source.data.subarray(sourceStart, sourceStart + width * 4)), `${page.l}#${frame} row ${row}`);
      assert.ok(atlas.data.subarray(destination - 4, destination)
        .equals(source.data.subarray(sourceStart, sourceStart + 4)), `${page.l}#${frame} left ${row}`);
      assert.ok(atlas.data.subarray(destination + width * 4, destination + (width + 1) * 4)
        .equals(source.data.subarray(sourceStart + (width - 1) * 4, sourceStart + width * 4)), `${page.l}#${frame} right ${row}`);
    }
    frames += 1;
    sourcePixels += width * height;
    gutterPixels += 2 * width + 2 * height + 4;
  }
}
console.log(JSON.stringify({
  passed: true,
  manifestSha256: createHash("sha256").update(manifestBytes).digest("hex"),
  pages: manifest.pages.length,
  frames,
  sourcePixelsPreserved: sourcePixels,
  edgeAndCornerPixelsVerified: gutterPixels,
  sourceImagesModified: false,
  liveVisualAccepted: false,
}, null, 2));
