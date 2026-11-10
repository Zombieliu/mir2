import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";

const publicRoot = new URL("../public/", import.meta.url);
const generated = new URL("../lib/generated/", import.meta.url);
const readJson = (file) => JSON.parse(readFileSync(file));
const release = readJson(new URL("../../../config/numeron-r16-web-assets.json", import.meta.url));
const entry = readJson(new URL("r16-entry-scene.json", generated));
const sources = readJson(new URL("r16-entry-object-paths.json", generated));
const metadataFile = new URL("r16-entry-visuals.json", generated);
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const media = "data:image/png;base64,";
const pngSignature = Buffer.from("89504e470d0a1a0a", "hex");
const [mode, manifestFile, ...extra] = process.argv.slice(2);
const check = mode === "--check" && manifestFile === undefined;
assert(check || (mode === "--from-assets" && manifestFile && extra.length === 0),
  "Usage: node scripts/generate-r16-entry-visuals.mjs --check | --from-assets <pinned release manifest>");
assert.equal(entry.assetReleaseId, release.version);
assert.equal(sources.sceneSha256, entry.sha256);

const sceneBytes = readFileSync(new URL(entry.path.slice(1), publicRoot));
assert.equal(digest(sceneBytes), entry.sha256);
const region = JSON.parse(sceneBytes).originalMapRegion;
const blendFile = "generated/original-map-blend/blend-frames.json";
const blends = readJson(new URL(blendFile, publicRoot));
const objectFrames = new Map();
for (const sprite of Object.values(region.sprites)) {
  if (sprite.drawMode !== "object") continue;
  for (const frame of sprite.frames) {
    const key = frame.path.match(/^\/original-map\/WemadeMir2\/(Objects[0-9]*\/[0-9]+)\.png$/)?.[1];
    if (!key) continue;
    const path = blends.frames.includes(key) ? "/generated/original-map-blend/WemadeMir2/" + key + ".png" : frame.path;
    objectFrames.set(path, frame);
  }
}
const objectPaths = sources.paths;
assert.equal(objectPaths.length, 90, "The default viewport and first steps must retain their audited coverage");
assert.deepEqual(objectPaths, [...new Set(objectPaths)].sort());
assert(objectPaths.every(path => objectFrames.has(path)), "Entry object paths must come from the pinned scene");

let manifest;
if (!check) {
  const manifestBytes = readFileSync(manifestFile);
  assert.equal(digest(manifestBytes), release.remoteReleaseSha256, "Pinned R16 release manifest changed");
  const value = JSON.parse(manifestBytes);
  assert.equal(value.version, entry.assetReleaseId);
  manifest = new Map(value.files.map(file => [file.p, file]));
}

function fromRelease(path, contentType) {
  const bytes = readFileSync(new URL(path, publicRoot));
  const pin = manifest.get(path);
  assert(pin?.c?.split(";")[0] === contentType && pin.s === bytes.length && pin.h === digest(bytes), "Published asset mismatch for " + path);
  return bytes;
}

function checkImage(data, frame) {
  assert.equal(typeof data, "string");
  assert(data.startsWith(media), "Unexpected image source for " + frame.path);
  const raw = Buffer.from(data.slice(media.length), "base64");
  assert.equal(data, media + raw.toString("base64"), "Invalid base64 for " + frame.path);
  assert(raw.length >= 24 && raw.subarray(0, 8).equals(pngSignature) && raw.toString("ascii", 12, 16) === "IHDR",
    "Invalid PNG for " + frame.path);
  assert.equal(raw.readUInt32BE(16), frame.width, "PNG width for " + frame.path);
  assert.equal(raw.readUInt32BE(20), frame.height, "PNG height for " + frame.path);
}

const playerLibraries = ["CArmour/00", "CHair/00"];
const npcLibraries = ["NPC/00", "NPC/01", "NPC/03", "NPC/05", "NPC/07", "NPC/11", "NPC/15", "NPC/45"];
const actorLibraries = [...playerLibraries, ...npcLibraries];
// The shared Crystal table uses four standing and six walking frames per direction.
// R16 starts the female body and hair bank at 808. Spawn NPC packets face up.
function isEntryActorFrame(library, index, frameSet) {
  if (!Number.isSafeInteger(index)) return false;
  if (playerLibraries.includes(library)) {
    return (index >= 0 && index < 80) || (index >= 808 && index < 888);
  }
  return frameSet?.actions.some(action => ["Standing", "Harvest"].includes(action.actionName) &&
    index >= action.start && index < action.start + action.count);
}

function actorData() {
  const catalog = JSON.parse(fromRelease("original-ui/frame-sets.generated.json", "application/json"));
  const libraries = {};
  const images = {};
  for (const library of actorLibraries) {
    const { version, count, frames, frameSet } = JSON.parse(fromRelease("original-ui/" + library + "/meta.json", "application/json"));
    const actions = frameSet?.actions?.length ? frameSet.actions : catalog.libraries[library]?.actions;
    const frameSetForRender = actions?.length ? { count: actions.length, actions } : null;
    const selected = frames.filter(frame =>
      isEntryActorFrame(library, frame.index, frameSetForRender) && frame.width > 0 && frame.height > 0);
    const present = selected.map(({ index, width, height, x, y, shadowX, shadowY, path, maskPath }) =>
      ({ index, width, height, x, y, shadowX, shadowY, path, maskPath }));
    libraries[library] = { version, count, frameSet: frameSetForRender, frames: present };
    for (const frame of present) images[frame.path] = media + fromRelease(frame.path.slice(1), "image/png").toString("base64");
  }
  return { libraries, images };
}

function checkActorPack(pack) {
  assert.deepEqual(Object.keys(pack.libraries), actorLibraries);
  const paths = [];
  for (const [library, meta] of Object.entries(pack.libraries)) {
    assert(Number.isSafeInteger(meta.version) && Number.isSafeInteger(meta.count) && meta.frames.length > 0);
    if (playerLibraries.includes(library)) assert.equal(meta.frames.length, 160);
    for (const frame of meta.frames) {
      assert(isEntryActorFrame(library, frame.index, meta.frameSet) &&
        frame.path === "/original-ui/" + library + "/" + frame.index + ".png");
      for (const key of ["width", "height", "x", "y", "shadowX", "shadowY"]) assert(Number.isSafeInteger(frame[key]));
      checkImage(pack.images[frame.path], frame);
      paths.push(frame.path);
    }
  }
  assert.deepEqual(Object.keys(pack.images), [...new Set(paths)]);
}

const old = check ? readJson(metadataFile) : null;
if (!check) fromRelease(blendFile, "application/json");
const objectData = () => Object.fromEntries(objectPaths.map(path => [
  path, media + fromRelease(path.slice(1), "image/png").toString("base64"),
]));
function verifyObjects(pack) {
  assert.deepEqual(Object.keys(pack), objectPaths);
  for (const path of objectPaths) checkImage(pack[path], objectFrames.get(path));
}
function packAsset(kind, maxBytes, make, verify) {
  let bytes;
  if (check) {
    const path = old[kind].path;
    assert.match(path, /^\/bootstrap\/r16-visuals\/[a-f0-9]{64}\.json$/);
    bytes = readFileSync(new URL(path.slice(1), publicRoot));
  } else {
    bytes = Buffer.from(JSON.stringify(make()) + "\n");
  }
  assert(bytes.length <= maxBytes, "Entry " + kind + " bytes exceed budget");
  verify(JSON.parse(bytes));
  const sha256 = digest(bytes);
  const path = "/bootstrap/r16-visuals/" + sha256 + ".json";
  if (!check) {
    const output = new URL(path.slice(1), publicRoot);
    mkdirSync(new URL(".", output), { recursive: true });
    writeFileSync(output, bytes);
  }
  return { path, bytes: bytes.length, sha256 };
}
const metadata = {
  assetReleaseId: entry.assetReleaseId,
  sceneSha256: entry.sha256,
  mapFileName: region.mapFileName,
  objects: packAsset("objects", 1_200_000, objectData, verifyObjects),
  actors: packAsset("actors", 980_000, actorData, checkActorPack),
};
const text = JSON.stringify(metadata, null, 2) + "\n";
if (check) {
  assert.equal(readFileSync(metadataFile, "utf8"), text, "Entry visuals metadata is stale");
} else {
  writeFileSync(metadataFile, text);
}
console.log(JSON.stringify({ checked: check, ...metadata }));
