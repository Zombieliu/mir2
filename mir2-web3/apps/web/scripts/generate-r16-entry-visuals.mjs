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
const playerMetadataFile = new URL("r16-entry-player-meta.json", generated);
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const media = "data:image/png;base64,";
const pngSignature = Buffer.from("89504e470d0a1a0a", "hex");
const [mode, manifestFile, ...extra] = process.argv.slice(2);
const check = mode === "--check" && manifestFile === undefined;
const split = mode === "--split" && manifestFile === undefined;
const fromAssets = mode === "--from-assets" && manifestFile && extra.length === 0;
assert(check || split || fromAssets,
  "Usage: node scripts/generate-r16-entry-visuals.mjs --check | --split | --from-assets <pinned release manifest>");
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
if (fromAssets) {
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
const monsterLibraries = ["Monster/000", "Monster/003", "Monster/004", "Monster/005", "Monster/139"];
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

function monsterFrameSelector(actionName) {
  return (_library, index, frameSet) => {
    const action = frameSet?.actions.find(entry => entry.actionName === actionName);
    if (!action || !Number.isSafeInteger(index) || index < action.start) return false;
    const stride = action.count + action.skip;
    const relative = index - action.start;
    return relative < 8 * stride && relative % stride < action.count;
  };
}

function actorData(keys = actorLibraries, includeFrame = isEntryActorFrame, useCatalog = true) {
  const catalog = useCatalog ? JSON.parse(fromRelease("original-ui/frame-sets.generated.json", "application/json")) : null;
  const libraries = {};
  const images = {};
  for (const library of keys) {
    const { version, count, frames, frameSet } = JSON.parse(fromRelease("original-ui/" + library + "/meta.json", "application/json"));
    const actions = frameSet?.actions?.length ? frameSet.actions : catalog?.libraries[library]?.actions;
    const frameSetForRender = actions?.length ? { count: actions.length, actions } : null;
    const selected = frames.filter(frame =>
      includeFrame(library, frame.index, frameSetForRender) && frame.width > 0 && frame.height > 0);
    const present = selected.map(({ index, width, height, x, y, shadowX, shadowY, path, maskPath }) =>
      ({ index, width, height, x, y, shadowX, shadowY, path, maskPath }));
    libraries[library] = { version, count, frameSet: frameSetForRender, frames: present };
    for (const frame of present) images[frame.path] = media + fromRelease(frame.path.slice(1), "image/png").toString("base64");
  }
  return { libraries, images };
}

function checkActorPack(pack, keys = actorLibraries, includeFrame = isEntryActorFrame, monsterFrameCount) {
  assert.deepEqual(Object.keys(pack.libraries), keys);
  const paths = [];
  for (const [library, meta] of Object.entries(pack.libraries)) {
    assert(Number.isSafeInteger(meta.version) && Number.isSafeInteger(meta.count) && meta.frames.length > 0);
    if (playerLibraries.includes(library)) assert.equal(meta.frames.length, 160);
    if (monsterFrameCount) assert.equal(meta.frames.length, monsterFrameCount);
    for (const frame of meta.frames) {
      assert(includeFrame(library, frame.index, meta.frameSet) &&
        frame.path === "/original-ui/" + library + "/" + frame.index + ".png");
      for (const key of ["width", "height", "x", "y", "shadowX", "shadowY"]) assert(Number.isSafeInteger(frame[key]));
      checkImage(pack.images[frame.path], frame);
      paths.push(frame.path);
    }
  }
  assert.deepEqual(Object.keys(pack.images), [...new Set(paths)]);
}

const old = fromAssets ? null : readJson(metadataFile);
if (fromAssets) fromRelease(blendFile, "application/json");
const objectData = () => Object.fromEntries(objectPaths.map(path => [
  path, media + fromRelease(path.slice(1), "image/png").toString("base64"),
]));
function verifyObjects(pack) {
  assert.deepEqual(Object.keys(pack), objectPaths);
  for (const path of objectPaths) checkImage(pack[path], objectFrames.get(path));
}
function assetPin(bytes) {
  const sha256 = digest(bytes);
  return { path: "/bootstrap/r16-visuals/" + sha256 + ".json", bytes: bytes.length, sha256 };
}
function writeAsset(pin, bytes) {
  const output = new URL(pin.path.slice(1), publicRoot);
  mkdirSync(new URL(".", output), { recursive: true });
  writeFileSync(output, bytes);
}
function packAsset(kind, maxBytes, make, verify) {
  let bytes;
  if (old) {
    const path = old[kind].path;
    assert.match(path, /^\/bootstrap\/r16-visuals\/[a-f0-9]{64}\.json$/);
    bytes = readFileSync(new URL(path.slice(1), publicRoot));
  } else {
    bytes = Buffer.from(JSON.stringify(make()) + "\n");
  }
  assert(bytes.length <= maxBytes, "Entry " + kind + " bytes exceed budget");
  const data = JSON.parse(bytes);
  verify(data);
  const pin = assetPin(bytes);
  if (old) assert.deepEqual(pin, old[kind]);
  else writeAsset(pin, bytes);
  return { pin, data };
}

function derivedAsset(data, maxBytes) {
  const bytes = Buffer.from(JSON.stringify(data) + "\n");
  assert(bytes.length <= maxBytes, "Split entry asset exceeds byte budget");
  const pin = assetPin(bytes);
  if (check) assert.deepEqual(readFileSync(new URL(pin.path.slice(1), publicRoot)), bytes);
  else writeAsset(pin, bytes);
  return pin;
}

function selectActorFrames(pack, keys, included) {
  const libraries = {};
  const images = {};
  for (const key of keys) {
    const source = pack.libraries[key];
    const frames = source.frames.filter(included);
    libraries[key] = { ...source, frames };
    for (const frame of frames) images[frame.path] = pack.images[frame.path];
  }
  return { libraries, images };
}

function splitObjects(data) {
  // Balance decoded bytes so large drawings do not postpone any one batch.
  const parts = Array.from({ length: 3 }, () => ({ paths: [], bytes: 0 }));
  const ordered = [...objectPaths].sort((a, b) => data[b].length - data[a].length);
  for (const path of ordered) {
    const part = parts.reduce((smallest, candidate) => candidate.bytes < smallest.bytes ? candidate : smallest);
    part.paths.push(path);
    part.bytes += path.length + data[path].length;
  }
  return parts.map(({ paths }) => derivedAsset(Object.fromEntries(paths.sort().map(path => [path, data[path]])), 420_000));
}
function monsterPack(kind, action, framesPerLibrary, maxBytes) {
  const select = monsterFrameSelector(action);
  return packAsset(kind, maxBytes, () => actorData(monsterLibraries, select, false),
    (pack) => checkActorPack(pack, monsterLibraries, select, framesPerLibrary)).pin;
}

const objects = packAsset("objects", 1_200_000, objectData, verifyObjects);
const actors = packAsset("actors", 980_000, actorData, checkActorPack);
const male = selectActorFrames(actors.data, playerLibraries, frame => frame.index < 80);
const female = selectActorFrames(actors.data, playerLibraries, frame => frame.index >= 808);
assert.equal(Object.keys(male.images).length, 160);
assert.equal(Object.keys(female.images).length, 160);
const npcs = selectActorFrames(actors.data, npcLibraries, () => true);

const metadata = {
  assetReleaseId: entry.assetReleaseId,
  sceneSha256: entry.sha256,
  mapFileName: region.mapFileName,
  objects: objects.pin,
  actors: actors.pin,
  objectParts: splitObjects(objects.data),
  players: { male: derivedAsset(male, 330_000), female: derivedAsset(female, 330_000) },
  npcs: derivedAsset(npcs, 350_000),
  monsterBounds: region.playBounds,
  monsterStanding: monsterPack("monsterStanding", "Standing", 32, 550_000),
  monsterWalking: monsterPack("monsterWalking", "Walking", 48, 820_000),
};
const text = JSON.stringify(metadata, null, 2) + "\n";
// The first visible player frame must not wait for either the complete motion
// pack or a remote library index. Keep only its exact, pinned frame directory
// in the client bundle; PNGs still use their ordinary asset paths.
const playerSourceMetas = {};
const playerDirectories = {};
for (const key of playerLibraries) {
  const path = "original-ui/" + key + "/meta.json";
  const bytes = fromAssets ? fromRelease(path, "application/json") : readFileSync(new URL(path, publicRoot));
  const source = JSON.parse(bytes);
  assert.equal(source.count, 1616);
  assert.equal(source.frames.length, source.count);
  assert(source.frames.every(frame => frame.maskPath === null), "Default player frames must retain their unmasked source");
  playerSourceMetas[key] = { path, bytes: bytes.length, sha256: digest(bytes) };
  playerDirectories[key] = source;
}
const playerMetadata = JSON.stringify({
  schemaVersion: 2,
  assetReleaseId: entry.assetReleaseId,
  sceneSha256: entry.sha256,
  releaseManifestSha256: release.remoteReleaseSha256,
  sourceMetas: playerSourceMetas,
  players: Object.fromEntries([["male", male], ["female", female]].map(([gender, pack]) => [gender, {
    sourceSha256: metadata.players[gender].sha256,
    // All actions need geometry before the slow full index arrives, including
    // run/attack/cast/hit/death/fishing. Keep the original count and exact frames;
    // paths follow the source library and index. No PNG bytes enter this bundle.
    libraries: Object.fromEntries(playerLibraries.map(key => {
      const source = playerDirectories[key];
      const frames = source.frames.filter(frame => gender === "male" ? frame.index < 808 : frame.index >= 808);
      assert.equal(frames.length, 808);
      assert.deepEqual(frames.filter(frame => isEntryActorFrame(key, frame.index))
        .map(({ index, width, height, x, y, shadowX, shadowY, path, maskPath }) =>
          ({ index, width, height, x, y, shadowX, shadowY, path, maskPath })), pack.libraries[key].frames);
      return [key, { version: source.version, count: source.count, frameSet: pack.libraries[key].frameSet,
        frames: frames.map(frame => [frame.index, frame.width, frame.height, frame.x, frame.y, frame.shadowX, frame.shadowY]) }];
    })),
  }])),
}) + "\n";
assert(Buffer.byteLength(playerMetadata) <= 80_000, "Player frame directory exceeds its bundle budget");
if (check) {
  assert.equal(readFileSync(metadataFile, "utf8"), text, "Entry visuals metadata is stale");
  assert.equal(readFileSync(playerMetadataFile, "utf8"), playerMetadata, "Player frame directory is stale");
} else {
  writeFileSync(metadataFile, text);
  writeFileSync(playerMetadataFile, playerMetadata);
}
console.log(JSON.stringify({ checked: check, ...metadata }));
