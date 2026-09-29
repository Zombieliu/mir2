#!/usr/bin/env node
// Read-only verification of the fixed prepare-item-stack-image-fixture layout.
// This is a sampled icon/geometry check, never a visual-acceptance promotion.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
const sharp = createRequire(path.join(root, "apps/web/package.json"))("sharp");
const { values } = parseArgs({ options: { capture: { type: "string" } } });
if (!values.capture) throw new Error("Usage: --capture inventory.png (adjacent .json required)");
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const capturePath = path.resolve(values.capture);
assert.equal(path.extname(capturePath).toLowerCase(), ".png");
const png = await fs.readFile(capturePath);
const sidecarBytes = await fs.readFile(capturePath.slice(0, -4) + ".json");
const sidecar = JSON.parse(sidecarBytes);
assert.equal(sidecar.imageSha256, sha256(png), "capture hash differs from original sidecar");
assert.equal(sidecar.imagePath, path.basename(capturePath));
assert.equal(sidecar.dpiScale, 1, "this fixture measures only native 100% DPI");
assert.deepEqual(sidecar.logicalSize, { width: 1024, height: 768 });
for (const state of ["panel=Inventory", "inventoryPage=0", "inventoryLocation=0.00,0.00"]) {
  assert.ok(sidecar.uiState.split(";").includes(state), `wrong capture state: ${state}`);
}
const capture = await sharp(png).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
assert.equal(capture.info.width, 1024);
assert.equal(capture.info.height, 768);
assert.equal(capture.info.channels, 4);

// Independent expected bands from Crystal Shared/Data/ItemData.cs:641-681.
// Do not import the production selector or infer the expected frame from pixels.
const families = [
  { index: 712, type: 8, shape: 0, base: 270, slots: 0, counts: [300, 1, 199, 200, 299, 500],
    frames: [3662, 3660, 3660, 3661, 3661, 3662], alternatives: [3660, 3661, 3662, 270] },
  { index: 710, type: 8, shape: 1, base: 259, slots: 8, counts: [1, 49, 50, 99, 100, 149, 150, 500],
    frames: [3673, 3673, 3674, 3674, 2960, 2960, 3675, 3675], alternatives: [3673, 3674, 2960, 3675, 259] },
  { index: 711, type: 8, shape: 2, base: 258, slots: 16, counts: [1, 49, 50, 99, 100, 149, 150, 500],
    frames: [3670, 3670, 3671, 3671, 2961, 2961, 3672, 3672], alternatives: [3670, 3671, 2961, 3672, 258] },
  { index: 714, type: 8, shape: 3, base: 277, slots: 24, counts: [5], frames: [277], alternatives: [3660, 3662] },
  { index: 713, type: 21, shape: 10, base: 466, slots: 25, counts: [1], frames: [466], alternatives: [3660, 3662] },
];
const cataloguePath = path.join(root, "packages/game-data/data/generated/crystal_item_manifest.json");
const catalogueBytes = await fs.readFile(cataloguePath);
const catalogue = JSON.parse(catalogueBytes);
const sources = new Map();
async function sourceImage(frame) {
  if (!sources.has(frame)) {
    const bytes = await fs.readFile(path.join(root, `apps/web/public/original-ui/Items/${frame}.png`));
    const decoded = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    sources.set(frame, { ...decoded, pngSha256: sha256(bytes) });
  }
  return sources.get(frame);
}

function compare(source, slot, dx = 0, dy = 0) {
  // InventoryDialog.cs:161 and MirItemCell.cs:2521: fixed source coordinates
  // and C# integer truncation. No position search or best-fit alignment.
  const cellX = 9 + (slot % 8) * 37;
  const cellY = 37 + Math.floor(slot / 8) * 33;
  const x = cellX + Math.trunc((36 - source.info.width) / 2) + dx;
  const y = cellY + Math.trunc((32 - source.info.height) / 2) + dy;
  let checkedPixels = 0;
  let mismatchedPixels = 0;
  for (let iy = 0; iy < source.info.height; iy++) {
    for (let ix = 0; ix < source.info.width; ix++) {
      const sourceOffset = (iy * source.info.width + ix) * 4;
      // Exclude transparent/antialiased edges and the lower 14px count-label
      // region. The recorded claim is explicitly limited to these samples.
      if (source.data[sourceOffset + 3] !== 255 || y + iy >= cellY + 18) continue;
      assert.ok(x + ix >= 0 && x + ix < capture.info.width);
      assert.ok(y + iy >= 0 && y + iy < capture.info.height);
      const targetOffset = ((y + iy) * capture.info.width + x + ix) * 4;
      checkedPixels++;
      if ([0, 1, 2].some(channel =>
        source.data[sourceOffset + channel] !== capture.data[targetOffset + channel])) {
        mismatchedPixels++;
      }
    }
  }
  assert.ok(checkedPixels >= 64, "not enough unoccluded opaque source pixels");
  return { x, y, width: source.info.width, height: source.info.height,
    checkedPixels, mismatchedPixels };
}

const cases = [];
for (const family of families) {
  const rows = catalogue.items.filter(item => item.item_index === family.index);
  assert.equal(rows.length, 1, "fixture identity must have one exact catalogue row");
  assert.equal(rows[0].image, family.base, "source base image changed; re-audit fixture");
  assert.equal(rows[0].item_type, family.type);
  assert.equal(rows[0].shape, family.shape);
  assert.ok(family.counts.every(count => count >= 1 && count <= rows[0].stack_size));
  for (const [offset, frame] of family.frames.entries()) {
    const slot = family.slots + offset;
    const source = await sourceImage(frame);
    const measured = compare(source, slot);
    assert.equal(measured.mismatchedPixels, 0, `source icon or geometry mismatch in slot ${slot}`);
    // Negative controls prove the sample distinguishes all sibling bands and
    // the base preview. A one-pixel shift must also fail; no permissive score.
    const rejectedFrames = [];
    for (const other of family.alternatives.filter(candidate => candidate !== frame)) {
      const negative = compare(await sourceImage(other), slot);
      assert.ok(negative.mismatchedPixels > 0, `ambiguous icon ${frame}/${other} in slot ${slot}`);
      rejectedFrames.push({ frame: other, mismatchedPixels: negative.mismatchedPixels });
    }
    const rejectedTranslations = [];
    for (const [dx, dy] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
      const negative = compare(source, slot, dx, dy);
      assert.ok(negative.mismatchedPixels > 0, `ambiguous geometry in slot ${slot}`);
      rejectedTranslations.push({ dx, dy, mismatchedPixels: negative.mismatchedPixels });
    }
    cases.push({ slot, itemIndex: family.index, fixtureCount: family.counts[offset],
      expectedImage: frame, sourcePngSha256: source.pngSha256, ...measured,
      rejectedFrames, rejectedTranslations });
  }
}

console.log(JSON.stringify({
  schemaVersion: "mir2-native-stack-image-pixel-sample-v1",
  capture: path.basename(capturePath), captureSha256: sha256(png),
  sidecarSha256: sha256(sidecarBytes), catalogueSha256: sha256(catalogueBytes),
  sourceRule: "Crystal 92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d Shared/Data/ItemData.cs:641-681",
  matchedSlots: cases.length,
  checkedOpaquePixels: cases.reduce((sum, item) => sum + item.checkedPixels, 0),
  mismatchedPixels: cases.reduce((sum, item) => sum + item.mismatchedPixels, 0),
  rejectedWrongFrames: cases.reduce((sum, item) => sum + item.rejectedFrames.length, 0),
  rejectedOnePixelTranslations: cases.reduce((sum, item) => sum + item.rejectedTranslations.length, 0),
  scope: "Fixed 24 bag cells; exact RGB of opaque source pixels above the count region",
  notProven: ["count-label pixels", "transparent edges", "belt and other panels",
    "manual stack transitions", "current-source executable identity", "Crystal same-state pair",
    "other DPI", "trusted package and light provenance", "human visual acceptance"],
  visualAccepted: false, accepted: false, cases,
}, null, 2));
