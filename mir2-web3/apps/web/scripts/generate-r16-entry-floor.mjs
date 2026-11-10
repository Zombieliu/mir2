import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";

const publicRoot = new URL("../public/", import.meta.url);
const entryScene = JSON.parse(readFileSync(new URL("../lib/generated/r16-entry-scene.json", import.meta.url)));
const releaseConfig = JSON.parse(readFileSync(new URL("../../../config/numeron-r16-web-assets.json", import.meta.url)));
const metadataFile = new URL("../lib/generated/r16-entry-floor.json", import.meta.url);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const dataPrefix = "data:image/png;base64,";
const sizeLimit = 450_000;
const pngSignature = Buffer.from("89504e470d0a1a0a", "hex");

const [mode, manifestPath, ...extra] = process.argv.slice(2);
const check = mode === "--check" && manifestPath === undefined;
assert(check || (mode === "--from-assets" && manifestPath && extra.length === 0),
  "Usage: node scripts/generate-r16-entry-floor.mjs --check | --from-assets <pinned release manifest>");
assert.equal(entryScene.assetReleaseId, releaseConfig.version);

const sceneBytes = readFileSync(new URL(entryScene.path.slice(1), publicRoot));
assert.equal(sha256(sceneBytes), entryScene.sha256, "The floor pack must use the exact R16 starter scene");
const region = JSON.parse(sceneBytes).originalMapRegion;
const frames = new Map();
// The ordinary R16 spawn is (288,616). Keep a margin for widescreen and the
// first steps in town. Surrounding tiles continue to load as normal images.
const bounds = { minX: 270, minY: 595, maxX: 306, maxY: 637 };
for (const cell of region.cells) {
  if (cell.x < bounds.minX || cell.x > bounds.maxX || cell.y < bounds.minY || cell.y > bounds.maxY) continue;
  for (const key of [cell.back, cell.middle, cell.front, cell.tileAnimation]) {
    const sprite = region.sprites[key];
    if (sprite?.drawMode !== "floor") continue;
    for (const frame of sprite.frames) {
      assert(/^\/original-map\/WemadeMir2\/(?:[\w.-]+\/)*\d+\.png$/.test(frame.path), "Unexpected floor path");
      frames.set(frame.path, frame);
    }
  }
}
const paths = Array.from(frames.keys()).sort();
assert(paths.length > 0, "The R16 floor pack is empty");

function checkPng(raw, frame) {
  assert(raw.length >= 24 && raw.subarray(0, 8).equals(pngSignature) && raw.toString("ascii", 12, 16) === "IHDR",
    "Invalid PNG for " + frame.path);
  assert.equal(raw.readUInt32BE(16), frame.width, "Width for " + frame.path);
  assert.equal(raw.readUInt32BE(20), frame.height, "Height for " + frame.path);
}

let bytes;
if (check) {
  const metadata = JSON.parse(readFileSync(metadataFile));
  assert.match(metadata.path, /^\/bootstrap\/r16-floor\/[a-f0-9]{64}\.json$/);
  bytes = readFileSync(new URL(metadata.path.slice(1), publicRoot));
} else {
  const manifestBytes = readFileSync(manifestPath);
  assert.equal(sha256(manifestBytes), releaseConfig.remoteReleaseSha256, "R16 release manifest hash changed");
  const manifest = JSON.parse(manifestBytes);
  assert.equal(manifest.version, entryScene.assetReleaseId);
  const published = new Map(manifest.files.map((file) => ["/" + file.p, file]));
  const entries = paths.map((path) => {
    const png = readFileSync(new URL(path.slice(1), publicRoot));
    const pin = published.get(path);
    assert(pin && pin.c === "image/png" && pin.s === png.length && pin.h === sha256(png),
      "Local PNG does not match the pinned release for " + path);
    return [path, dataPrefix + png.toString("base64")];
  });
  bytes = Buffer.from(JSON.stringify(Object.fromEntries(entries), null, 2) + "\n");
}

assert(bytes.length <= sizeLimit, "R16 floor pack exceeds its decoded byte budget");
const images = JSON.parse(bytes);
assert.deepEqual(Object.keys(images), paths, "R16 floor pack must cover its exact ordered set of frames");
for (const path of paths) {
  const data = images[path];
  assert.equal(typeof data, "string");
  assert(data.startsWith(dataPrefix), "Invalid image source for " + path);
  const raw = Buffer.from(data.slice(dataPrefix.length), "base64");
  assert.equal(data, dataPrefix + raw.toString("base64"), "Invalid base64 for " + path);
  checkPng(raw, frames.get(path));
}
const digest = sha256(bytes);
const path = "/bootstrap/r16-floor/" + digest + ".json";
const metadata = {
  assetReleaseId: entryScene.assetReleaseId,
  sceneSha256: entryScene.sha256,
  mapFileName: region.mapFileName,
  path,
  sha256: digest,
  bytes: bytes.length,
  frameCount: paths.length,
};
const text = JSON.stringify(metadata, null, 2) + "\n";
if (check) {
  assert.equal(readFileSync(metadataFile, "utf8"), text, "Generated R16 floor pack metadata is stale");
} else {
  const output = new URL(path.slice(1), publicRoot);
  mkdirSync(new URL(".", output), { recursive: true });
  writeFileSync(output, bytes);
  writeFileSync(metadataFile, text);
}
console.log(JSON.stringify({ checked: check, ...metadata }));
