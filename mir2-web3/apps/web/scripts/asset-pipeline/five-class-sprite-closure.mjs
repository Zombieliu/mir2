import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";
import { parseLibrary, decodeFrameRgba, encodePng } from "../crystal-library.mjs";
import { CLIENT_EFFECTS, normalizeAdditiveRgba } from "../export-crystal-magic-effects.mjs";
import { sha256 } from "./item-icon-closure.mjs";

const PROJECT_ROOT = path.resolve(import.meta.dirname, "../../../..");
const DEFAULT_PROFILE = path.join(PROJECT_ROOT, "packages/game-data/data/content_profiles/platinum_fiveclass.json");
const DEFAULT_CATALOGUE = path.join(PROJECT_ROOT, "packages/game-data/data/generated/crystal_item_manifest.json");
export const DEFAULT_FIVE_CLASS_REQUIREMENTS = path.join(import.meta.dirname, "five-class-sprite-requirements.json");
const CLASSES = ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"];
const SOUNDS = [
  { id: 21210, fileName: "M121-0.wav" },
  { id: 21212, fileName: "M121-2.wav" },
  { id: 21215, fileName: "M121-5.wav" },
];
export const ARCHER_FRAMES = [
  ...Array.from({ length: 16 }, (_, direction) => Array.from({ length: 5 }, (_, frame) => 1030 + direction * 10 + frame)).flat(),
  ...Array.from({ length: 10 }, (_, frame) => 2730 + frame),
];

// PlayerObject.SetLibraries switches to C bodies for ordinary actions, to A
// bodies for dual-blade actions and to AR bodies for bow movement/attacks.
// Female offsets are C=808/416, A=512, AR=352. Export the complete two-gender
// action blocks, including standing, combat, death, mining and fishing.
export function collectFiveClassSpriteRequirements(profile, catalogue) {
  assert.equal(profile?.profileId, "platinum_fiveclass", "five-class profile identity");
  assert.deepEqual(profile.allowedClasses, CLASSES, "five-class class list");
  assert.ok(Array.isArray(profile.itemWhitelist) && profile.itemWhitelist.length > 0);
  assert.ok(Array.isArray(catalogue?.items) && catalogue.items.length > 0);
  const names = new Set(profile.itemWhitelist);
  const selected = catalogue.items.filter((item) => names.has(item.name)
    && [1, 2].includes(item.item_type) && (item.required_class & 24));
  assert.ok(selected.length > 0, "five-class equipment must not be empty");
  const libraries = new Map();
  const add = (library, frameCount, usage) => {
    if (!libraries.has(library)) libraries.set(library, { library, frameCount, usage: new Set() });
    assert.equal(libraries.get(library).frameCount, frameCount);
    libraries.get(library).usage.add(usage);
  };
  const shape = (value) => {
    assert.ok(Number.isSafeInteger(value) && value >= 0 && value < 300, "valid source item shape");
    return String(value).padStart(2, "0");
  };
  for (const [className, mask, family, alternateFrames] of [
    ["Assassin", 8, "A", 1024], ["Archer", 16, "AR", 704],
  ]) {
    add("CArmour/00", 1616, className);
    add(`${family}Armour/00`, alternateFrames, className);
    for (let hair = 0; hair < 9; hair++) {
      add(`CHair/${shape(hair)}`, 1616, className);
      add(`${family}Hair/${shape(hair)}`, alternateFrames, className);
    }
    for (const item of selected.filter((item) => item.required_class & mask)) {
      assert.ok(Number.isSafeInteger(item.item_index) && item.item_index > 0);
      assert.ok([1, 2, 3].includes(item.required_gender), "source gender mask");
      if (item.item_type === 2) {
        add(`CArmour/${shape(item.shape)}`, 1616, className);
        add(`${family}Armour/${shape(item.shape)}`, alternateFrames, className);
      } else if (item.shape < 100) {
        add(`CWeapon/${shape(item.shape)}`, 832, className);
      } else if (item.shape < 200) {
        assert.equal(className, "Assassin", "only Assassin uses dual blades");
        for (const side of ["L", "R"]) add(`AWeapon/${shape(item.shape - 100)} ${side}`, 1024, className);
      } else {
        assert.equal(className, "Archer", "only Archer uses bows");
        add(`ARWeapon/${shape(item.shape - 200)}`, 832, className);
        add(`ARWeapon/${shape(item.shape - 200)} S`, 704, className);
      }
    }
  }
  return {
    equipment: selected.map((item) => ({ itemIndex: item.item_index, name: item.name, type: item.item_type,
      shape: item.shape, classMask: item.required_class, genderMask: item.required_gender, image: item.image }))
      .sort((a, b) => a.itemIndex - b.itemIndex),
    libraries: [...libraries.values()].map((entry) => ({ ...entry, usage: [...entry.usage].sort() }))
      .sort((a, b) => a.library.localeCompare(b.library, "en")),
  };
}

export function json(bytes) { return JSON.parse(bytes.toString("utf8")); }
export function jsonBytes(value) { return Buffer.from(`${JSON.stringify(value, null, 2)}\n`); }
function pngDimensions(bytes) {
  assert.ok(bytes.length >= 24 && bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])), "PNG signature/header");
  assert.equal(bytes.toString("ascii", 12, 16), "IHDR");
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
}
export function sourceIdentity(library, bytes) { return { path: `${library}.Lib`, sha256: sha256(bytes), bytes: bytes.length }; }
export function assertSourceFrame(frame, index, library) {
  assert.ok(frame && frame.width > 0 && frame.height > 0, `${library} source frame ${index} is missing`);
  assert.ok(!frame.maskRgba && !frame.hasMask, `${library} player frame ${index} unexpectedly requires a mask`);
}
export async function writeExactFrame(assetRoot, prefix, library, index, frame, expectedRgba) {
  const relative = `${prefix}/${library}/${index}.png`;
  const output = path.join(assetRoot, relative);
  let png;
  if (existsSync(output)) {
    png = await readFile(output);
    const actual = await sharp(png).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    assert.equal(actual.info.width, frame.width, `${relative} source width`);
    assert.equal(actual.info.height, frame.height, `${relative} source height`);
    assert.ok(actual.data.equals(expectedRgba), `${relative} does not match actual Crystal pixels`);
  } else {
    png = encodePng(frame.width, frame.height, expectedRgba, 1);
    await writeFile(output, png);
  }
  return { index, width: frame.width, height: frame.height, x: frame.x, y: frame.y,
    shadowX: frame.shadowX, shadowY: frame.shadowY, hasMask: false, maskWidth: null, maskHeight: null,
    rgbaSha256: sha256(expectedRgba), pngSha256: sha256(png), path: `/${relative}`, maskPath: null };
}

export async function exportFiveClassSpriteAssets({
  sourceDataDir, assetRoot, profilePath = DEFAULT_PROFILE, cataloguePath = DEFAULT_CATALOGUE,
  requirementsPath = DEFAULT_FIVE_CLASS_REQUIREMENTS, log = console.log,
}) {
  assert.ok(sourceDataDir, "--source-data-dir is required for export");
  const profileBytes = await readFile(profilePath);
  const catalogueBytes = await readFile(cataloguePath);
  const derived = collectFiveClassSpriteRequirements(json(profileBytes), json(catalogueBytes));
  const records = [];
  for (const [ordinal, required] of derived.libraries.entries()) {
    const bytes = await readFile(path.join(sourceDataDir, `${required.library}.Lib`));
    const original = parseLibrary(bytes);
    const frames = [];
    await mkdir(path.join(assetRoot, "original-ui", required.library), { recursive: true });
    for (let start = 0; start < required.frameCount; start += 32) {
      const batch = Array.from({ length: Math.min(32, required.frameCount - start) }, (_, offset) => start + offset);
      frames.push(...await Promise.all(batch.map(async (index) => {
        const frame = original.frames[index];
        assertSourceFrame(frame, index, required.library);
        return writeExactFrame(assetRoot, "original-ui", required.library, index, frame, decodeFrameRgba(original, frame));
      })));
    }
    const sourceLibrary = sourceIdentity(required.library, bytes);
    const meta = jsonBytes({ version: original.version, sourceLibrary, count: original.count,
      frameSet: original.frameSet, frames });
    await writeFile(path.join(assetRoot, "original-ui", required.library, "meta.json"), meta);
    records.push({ ...required, sourceLibrary, metaSha256: sha256(meta), frameSetSha256: sha256(jsonBytes(original.frameSet)), sourceFrameCount: original.count });
    log(`[five-class-assets] ${ordinal + 1}/${derived.libraries.length} ${required.library}: ${frames.length} exact frames`);
  }
  const magicBytes = await readFile(path.join(sourceDataDir, "Magic3.Lib"));
  const magic = parseLibrary(magicBytes);
  const magicDir = path.join(assetRoot, "original-effects/Magic3");
  await mkdir(magicDir, { recursive: true });
  const magicMetaPath = path.join(magicDir, "meta.json");
  const magicMeta = existsSync(magicMetaPath) ? json(await readFile(magicMetaPath)) : { frames: {} };
  assert.ok(!Array.isArray(magicMeta.frames) && magicMeta.frames && typeof magicMeta.frames === "object");
  for (const index of ARCHER_FRAMES) {
    const frame = magic.frames[index];
    assertSourceFrame(frame, index, "Magic3");
    const exported = await writeExactFrame(assetRoot, "original-effects", "Magic3", index, frame, normalizeAdditiveRgba(decodeFrameRgba(magic, frame)));
    magicMeta.frames[String(index)] = exported;
  }
  magicMeta.count = magic.count;
  magicMeta.sourceLibrary = sourceIdentity("Magic3", magicBytes);
  const magicMetaBytes = jsonBytes(magicMeta);
  await writeFile(magicMetaPath, magicMetaBytes);
  const effectsPath = path.join(assetRoot, "original-effects/effects.generated.json");
  const effects = json(await readFile(effectsPath));
  const archerEffects = CLIENT_EFFECTS.filter((entry) => ["ArcherNormalArrow", "ArcherFocusProc"].includes(entry.effect));
  assert.equal(archerEffects.length, 2);
  const effectNames = new Set(archerEffects.map((entry) => entry.effect));
  effects.client_effects = [...effects.client_effects.filter((entry) => !effectNames.has(entry.effect)), ...archerEffects]
    .sort((a, b) => a.effect.localeCompare(b.effect));
  await writeFile(effectsPath, jsonBytes(effects));
  const soundIndexPath = path.join(assetRoot, "original-ui/sound-index.generated.json");
  const soundIndex = json(await readFile(soundIndexPath));
  const soundRecords = [];
  await mkdir(path.join(assetRoot, "original-ui/Sound"), { recursive: true });
  for (const sound of SOUNDS) {
    const bytes = await readFile(path.join(sourceDataDir, "../Sound", sound.fileName));
    assert.equal(bytes.toString("ascii", 0, 4), "RIFF", `${sound.fileName} is not actual WAV`);
    assert.equal(bytes.toString("ascii", 8, 12), "WAVE");
    await writeFile(path.join(assetRoot, "original-ui/Sound", sound.fileName), bytes);
    const record = { ...sound, sourcePath: `Sound/${sound.fileName}`, path: `/original-ui/Sound/${sound.fileName}`, bytes: bytes.length, sha256: sha256(bytes) };
    soundRecords.push(record);
    soundIndex.sounds[String(sound.id)] = { fileName: sound.fileName, path: record.path,
      sourceExists: true, exactSourceExists: true, fallback: false, sourceBytes: bytes.length };
  }
  soundIndex.soundListEntryCount = Object.keys(soundIndex.sounds).length;
  soundIndex.exportedSoundCount = Object.values(soundIndex.sounds).filter((entry) => entry.sourceExists).length;
  await writeFile(soundIndexPath, jsonBytes(soundIndex));
  const requirements = {
    schemaVersion: 1, kind: "mir2-five-class-sprite-closure", profileId: "platinum_fiveclass",
    profileSha256: sha256(profileBytes), catalogueSha256: sha256(catalogueBytes),
    provenance: { libraries: "Crystal/Client/MirObjects/PlayerObject.cs::SetLibraries",
      actions: "Crystal/Client/MirObjects/Frames.cs::Player", effects: "Crystal/Client/MirObjects/PlayerObject.cs::MirAction.AttackRange1" },
    equipment: derived.equipment, libraries: records,
    effects: { library: "Magic3", sourceLibrary: magicMeta.sourceLibrary, metaSha256: sha256(magicMetaBytes), frames: ARCHER_FRAMES, definitions: archerEffects },
    sounds: soundRecords,
  };
  await mkdir(path.dirname(requirementsPath), { recursive: true });
  await writeFile(requirementsPath, jsonBytes(requirements));
  return { libraryCount: records.length, playerFrameCount: records.reduce((sum, record) => sum + record.frameCount, 0), effectFrameCount: ARCHER_FRAMES.length, soundCount: soundRecords.length };
}

export async function inspectFrame({ assetRoot, prefix, library, index, frame, original, normalize, verifyPixels }) {
  for (const field of ["width", "height", "x", "y", "shadowX", "shadowY"]) assert.ok(Number.isSafeInteger(frame?.[field]), `${library}/${index}.${field}`);
  assert.ok(frame.width > 0 && frame.height > 0);
  assert.equal(frame.path, `/${prefix}/${library}/${index}.png`);
  assert.match(frame.pngSha256 ?? "", /^[a-f0-9]{64}$/);
  assert.match(frame.rgbaSha256 ?? "", /^[a-f0-9]{64}$/);
  const bytes = await readFile(path.join(assetRoot, prefix, library, `${index}.png`));
  assert.equal(sha256(bytes), frame.pngSha256, `${library}/${index} PNG identity`);
  assert.deepEqual(pngDimensions(bytes), { width: frame.width, height: frame.height }, `${library}/${index} PNG dimensions`);
  if (original) {
    const source = original.frames[index];
    assertSourceFrame(source, index, library);
    for (const field of ["width", "height", "x", "y", "shadowX", "shadowY"]) assert.equal(frame[field], source[field], `${library}/${index}.${field} source`);
    let expected = decodeFrameRgba(original, source);
    if (normalize) expected = normalizeAdditiveRgba(expected);
    assert.equal(sha256(expected), frame.rgbaSha256, `${library}/${index} source pixels hash`);
    if (verifyPixels) {
      const actual = await sharp(bytes).ensureAlpha().raw().toBuffer();
      assert.ok(actual.equals(expected), `${library}/${index} actual source pixels`);
    }
  }
}

export async function inspectFiveClassSpriteClosure({
  assetRoot, profilePath = DEFAULT_PROFILE, cataloguePath = DEFAULT_CATALOGUE,
  requirementsPath = DEFAULT_FIVE_CLASS_REQUIREMENTS, sourceDataDir, verifyPixels = false,
}) {
  assert.ok(!verifyPixels || sourceDataDir, "--verify-pixels requires --source-data-dir");
  const profileBytes = await readFile(profilePath);
  const catalogueBytes = await readFile(cataloguePath);
  const derived = collectFiveClassSpriteRequirements(json(profileBytes), json(catalogueBytes));
  const required = json(await readFile(requirementsPath));
  assert.equal(required.schemaVersion, 1);
  assert.equal(required.kind, "mir2-five-class-sprite-closure");
  assert.equal(required.profileId, "platinum_fiveclass");
  assert.equal(required.profileSha256, sha256(profileBytes), "five-class profile changed: regenerate resource closure");
  assert.equal(required.catalogueSha256, sha256(catalogueBytes), "source item catalogue changed");
  assert.deepEqual(required.equipment, derived.equipment, "equipment denominator");
  assert.deepEqual(required.libraries.map(({ library, frameCount, usage }) => ({ library, frameCount, usage })), derived.libraries, "required library denominator");
  let playerFrameCount = 0;
  for (const library of required.libraries) {
    const bytes = await readFile(path.join(assetRoot, "original-ui", library.library, "meta.json"));
    assert.equal(sha256(bytes), library.metaSha256, `${library.library} metadata identity`);
    const meta = json(bytes);
    assert.deepEqual(meta.sourceLibrary, library.sourceLibrary, `${library.library} source identity`);
    assert.equal(meta.sourceLibrary.path, `${library.library}.Lib`);
    assert.match(meta.sourceLibrary.sha256, /^[a-f0-9]{64}$/);
    assert.ok(Number.isSafeInteger(meta.sourceLibrary.bytes) && meta.sourceLibrary.bytes > 0);
    assert.equal(meta.count, library.sourceFrameCount);
    assert.ok(meta.count >= library.frameCount);
    assert.equal(sha256(jsonBytes(meta.frameSet)), library.frameSetSha256);
    assert.ok(Array.isArray(meta.frames) && meta.frames.length === library.frameCount, `${library.library} full frame count`);
    const sourceBytes = sourceDataDir ? await readFile(path.join(sourceDataDir, `${library.library}.Lib`)) : null;
    if (sourceBytes) assert.deepEqual(sourceIdentity(library.library, sourceBytes), library.sourceLibrary);
    const original = sourceBytes ? parseLibrary(sourceBytes) : null;
    if (original) assert.deepEqual(meta.frameSet, original.frameSet, `${library.library} source frame set`);
    for (let start = 0; start < library.frameCount; start += 32) {
      const batch = Array.from({ length: Math.min(32, library.frameCount - start) }, (_, offset) => start + offset);
      await Promise.all(batch.map((index) => {
        const frame = meta.frames[index];
        assert.equal(frame.index, index, `${library.library} frame identity/order`);
        return inspectFrame({ assetRoot, prefix: "original-ui", library: library.library, index, frame, original, verifyPixels });
      }));
    }
    playerFrameCount += library.frameCount;
  }
  const effectRequired = required.effects;
  assert.equal(effectRequired.library, "Magic3");
  assert.deepEqual(effectRequired.frames, ARCHER_FRAMES);
  const definitions = CLIENT_EFFECTS.filter((entry) => ["ArcherNormalArrow", "ArcherFocusProc"].includes(entry.effect));
  assert.deepEqual(effectRequired.definitions, definitions);
  const effects = json(await readFile(path.join(assetRoot, "original-effects/effects.generated.json")));
  for (const definition of definitions) assert.deepEqual(effects.client_effects.find((entry) => entry.effect === definition.effect), definition);
  const effectMetaBytes = await readFile(path.join(assetRoot, "original-effects/Magic3/meta.json"));
  assert.equal(sha256(effectMetaBytes), effectRequired.metaSha256, "Magic3 metadata identity");
  const effectMeta = json(effectMetaBytes);
  assert.deepEqual(effectMeta.sourceLibrary, effectRequired.sourceLibrary);
  const magicSource = sourceDataDir ? await readFile(path.join(sourceDataDir, "Magic3.Lib")) : null;
  if (magicSource) assert.deepEqual(sourceIdentity("Magic3", magicSource), effectRequired.sourceLibrary);
  const original = magicSource ? parseLibrary(magicSource) : null;
  await Promise.all(ARCHER_FRAMES.map((index) => inspectFrame({ assetRoot, prefix: "original-effects", library: "Magic3", index,
    frame: effectMeta.frames[String(index)], original, normalize: true, verifyPixels })));
  const soundIndex = json(await readFile(path.join(assetRoot, "original-ui/sound-index.generated.json")));
  assert.deepEqual(required.sounds.map(({ id, fileName }) => ({ id, fileName })), SOUNDS);
  for (const sound of required.sounds) {
    const bytes = await readFile(path.join(assetRoot, "original-ui/Sound", sound.fileName));
    assert.equal(sha256(bytes), sound.sha256, `${sound.fileName} exact source`);
    assert.equal(bytes.length, sound.bytes);
    assert.equal(sound.path, `/original-ui/Sound/${sound.fileName}`);
    const indexed = soundIndex.sounds[String(sound.id)];
    assert.ok(indexed.sourceExists && indexed.exactSourceExists && !indexed.fallback);
    assert.equal(indexed.path, sound.path);
    assert.equal(indexed.sourceBytes, sound.bytes);
    if (sourceDataDir) assert.ok(bytes.equals(await readFile(path.join(sourceDataDir, "../Sound", sound.fileName))));
  }
  // Inventory icons and full-sized paperdoll art use source item.Image.
  for (const library of ["Items", "StateItem"]) {
    const meta = json(await readFile(path.join(assetRoot, "original-ui", library, "meta.json")));
    const byIndex = new Map(meta.frames.map((frame) => [frame.index, frame]));
    for (const index of new Set(derived.equipment.map((item) => item.image))) {
      const frame = byIndex.get(index);
      assert.ok(frame, `${library} missing required equipment image ${index}`);
      const png = await readFile(path.join(assetRoot, "original-ui", library, `${index}.png`));
      assert.deepEqual(pngDimensions(png), { width: frame.width, height: frame.height });
    }
  }
  return { profileId: required.profileId, equipmentCount: derived.equipment.length, libraryCount: required.libraries.length,
    playerFrameCount, effectFrameCount: ARCHER_FRAMES.length, soundCount: SOUNDS.length,
    sourceVerified: Boolean(sourceDataDir), sourcePixelsVerified: verifyPixels };
}

async function main() {
  const args = {};
  for (let i = 2; i < process.argv.length; i++) {
    const key = process.argv[i];
    assert.ok(["--mode", "--profile", "--assets", "--requirements", "--source-data-dir", "--catalogue", "--verify-pixels"].includes(key), `unknown argument ${key}`);
    args[key] = key === "--verify-pixels" ? true : process.argv[++i];
  }
  const options = { assetRoot: path.resolve(args["--assets"] ?? path.join(PROJECT_ROOT, "apps/web/public")),
    profilePath: args["--profile"] ? path.resolve(args["--profile"]) : DEFAULT_PROFILE,
    cataloguePath: args["--catalogue"] ? path.resolve(args["--catalogue"]) : DEFAULT_CATALOGUE,
    requirementsPath: args["--requirements"] ? path.resolve(args["--requirements"]) : DEFAULT_FIVE_CLASS_REQUIREMENTS,
    sourceDataDir: args["--source-data-dir"] ? path.resolve(args["--source-data-dir"]) : undefined,
    verifyPixels: args["--verify-pixels"] === true };
  const mode = args["--mode"] ?? "verify";
  const summary = mode === "export" ? await exportFiveClassSpriteAssets(options)
    : mode === "verify" ? await inspectFiveClassSpriteClosure(options)
      : mode === "plan" ? collectFiveClassSpriteRequirements(json(await readFile(options.profilePath)), json(await readFile(options.cataloguePath)))
        : assert.fail(`invalid mode ${mode}`);
  console.log(JSON.stringify(summary, null, 2));
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().catch((error) => { console.error(error); process.exitCode = 1; });
