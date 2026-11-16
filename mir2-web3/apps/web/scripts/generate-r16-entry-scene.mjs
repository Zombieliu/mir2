import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

// The decoded anonymous GET /api/scene/crystal response from numeron.obelisk.build
// at e3e6f3f06f83bb13a10334062b4d25bb50d6a44b. Use the pins below as v,
// assetRelease, map, x, y, width and height, then pass the raw response to --from-response.
const assetReleaseId = "20261108-r16-08ec86d5105c";
const schemaVersion = "2026-08-14-v7-door-interaction";
const request = { mapFileName: "0", centerX: 296, centerY: 620, width: 56, height: 72 };
const sha256 = "5c08d1fd4e81d92a7be20eb98c4c6288515749d5d459978dcf78fe2b86bdf325";
const path = "/bootstrap/r16-scenes/" + sha256 + ".json";
const sceneFile = new URL("../public" + path, import.meta.url);
const metadataFile = new URL("../lib/generated/r16-entry-scene.json", import.meta.url);

const [mode, input, ...rest] = process.argv.slice(2);
const check = mode === "--check" && input === undefined;
assert(check || (mode === "--from-response" && input && rest.length === 0),
  "Usage: node scripts/generate-r16-entry-scene.mjs --check | --from-response <raw JSON file>");
const raw = readFileSync(check ? sceneFile : resolve(input));
assert.equal(createHash("sha256").update(raw).digest("hex"), sha256, "R16 Crystal response hash changed");

const scene = JSON.parse(raw.toString("utf8"));
const region = scene.originalMapRegion;
assert.equal(region.mapFileName, request.mapFileName + ".map");
assert.deepEqual(scene.sceneView, {
  center: { x: request.centerX, y: request.centerY }, width: 48, height: 36,
}, "R16 scene view must match the Crystal loader's clamped viewport");
assert.equal(region.missingAssets?.length ?? 0, 0, "R16 scene has unresolved assets");

// The authoritative (288, 616) spawn normalizes to this 16×17 chunk. Sparse
// Crystal cells need no records when empty, so coverage comes from the bounds.
const spawnChunk = { minX: 288, maxX: 303, minY: 612, maxY: 628 };
const mapBounds = { minX: 0, maxX: region.mapWidth - 1, minY: 0, maxY: region.mapHeight - 1 };
function covers(outer, inner) {
  return outer.minX <= inner.minX && outer.maxX >= inner.maxX &&
    outer.minY <= inner.minY && outer.maxY >= inner.maxY;
}
assert(covers(mapBounds, region.regionBounds) && covers(region.regionBounds, region.playBounds) &&
  covers(region.playBounds, spawnChunk), "R16 scene must cover the entire normalized spawn chunk");

const framePaths = new Set();
for (const [spriteId, sprite] of Object.entries(region.sprites)) {
  assert(Array.isArray(sprite.frames) && sprite.frames.length > 0, "Missing frames for " + spriteId);
  for (const frame of sprite.frames) {
    assert(/^\/original-map\/WemadeMir2\/(?:[\w.-]+\/)*\d+\.png$/.test(frame.path) &&
      Number.isSafeInteger(frame.width) && frame.width > 0 &&
      Number.isSafeInteger(frame.height) && frame.height > 0, "Invalid R16 sprite frame for " + spriteId);
    framePaths.add(frame.path);
  }
}

const seenCells = new Set(), usedSprites = new Set();
for (const cell of region.cells) {
  const key = cell.x + "," + cell.y;
  assert(Number.isSafeInteger(cell.x) && Number.isSafeInteger(cell.y) && !seenCells.has(key) &&
    covers(region.regionBounds, { minX: cell.x, maxX: cell.x, minY: cell.y, maxY: cell.y }),
  "Duplicate or out-of-bounds R16 cell " + key);
  seenCells.add(key);
  for (const layer of ["back", "middle", "front", "tileAnimation"]) {
    if (!Object.hasOwn(cell, layer) || (cell[layer] === null && layer !== "back")) continue;
    const spriteId = cell[layer];
    assert(typeof spriteId === "string" && Object.hasOwn(region.sprites, spriteId) &&
      region.sprites[spriteId].kind === layer, "Unresolved R16 " + layer + " layer at " + key);
    usedSprites.add(spriteId);
  }
}
assert(usedSprites.size > 0 && usedSprites.size === Object.keys(region.sprites).length,
  "R16 scene sprite table must match all referenced layers");

const metadata = { assetReleaseId, schemaVersion, request, path, sha256, bytes: raw.length };
const metadataText = JSON.stringify(metadata, null, 2) + "\n";
if (check) {
  assert.equal(readFileSync(metadataFile, "utf8"), metadataText, "Generated R16 entry scene metadata is stale");
} else {
  mkdirSync(new URL(".", sceneFile), { recursive: true });
  writeFileSync(sceneFile, raw);
  writeFileSync(metadataFile, metadataText);
}
console.log(JSON.stringify({ checked: check, ...metadata, cells: seenCells.size, sprites: usedSprites.size, frames: framePaths.size }));
