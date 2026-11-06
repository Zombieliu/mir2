import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { allPresentFrameIndices, decodeFrameRgba, parseLibrary } from "../crystal-library.mjs";
import { CLIENT_EFFECTS, OBJECT_EFFECTS, SPELL_EFFECTS, WORLD_SPELL_EFFECTS, normalizeAdditiveRgba, validateEffectDefinitions } from "../export-crystal-magic-effects.mjs";
import { ARCHER_FRAMES, DEFAULT_FIVE_CLASS_REQUIREMENTS, assertSourceFrame, inspectFrame, json, jsonBytes, sourceIdentity, writeExactFrame } from "./five-class-sprite-closure.mjs";
import { FIVE_CLASS_SKILL_ACTORS, FIVE_CLASS_SKILL_SOUNDS, FIVE_CLASS_SKILL_SOURCE } from "./five-class-skill-source.mjs";
import { sha256 } from "./item-icon-closure.mjs";

const PROJECT_ROOT = path.resolve(import.meta.dirname, "../../../..");
const PROFILE = path.join(PROJECT_ROOT, "packages/game-data/data/content_profiles/platinum_fiveclass.json");
export const DEFAULT_FIVE_CLASS_SKILL_REQUIREMENTS = path.join(import.meta.dirname, "five-class-skill-requirements.json");
const SOURCE_FILES = ["Client/MirObjects/PlayerObject.cs", "Client/MirObjects/Frames.cs", "Client/MirObjects/Effect.cs", "Client/MirObjects/SpellObject.cs", "Client/MirObjects/MonsterObject.cs", "Client/MirScenes/GameScene.cs", "Server/MirObjects/HumanObject.cs", "Shared/Enums.cs"];

export function fiveClassSkillDefinitions() {
  validateEffectDefinitions();
  const sections = { spell_effects: SPELL_EFFECTS, client_effects: CLIENT_EFFECTS, object_effects: OBJECT_EFFECTS, ground_effects: WORLD_SPELL_EFFECTS };
  const refs = [...new Set(FIVE_CLASS_SKILL_SOURCE.flatMap((entry) => entry.refs))].sort();
  return refs.map((ref) => {
    const [section, name] = ref.split(":");
    const matching = sections[section]?.filter((spec) => (spec.spell ?? spec.effect) === name);
    assert.equal(matching?.length, 1, `${ref} must resolve to one source effect contract`);
    return { ref, section, definition: matching[0] };
  });
}
export function collectFiveClassSkillRequirements(profile) {
  assert.equal(profile?.profileId, "platinum_fiveclass");
  const skills = profile.skills.filter((entry) => ["Assassin", "Archer"].includes(entry.class))
    .sort((a, b) => a.spell.localeCompare(b.spell, "en"));
  assert.deepEqual(skills.map(({ spell, class: className }) => ({ spell, class: className })),
    FIVE_CLASS_SKILL_SOURCE.map(({ spell, class: className }) => ({ spell, class: className }))
      .sort((a, b) => a.spell.localeCompare(b.spell, "en")), "all 34 added skills must be audited, with no silent omission");
  for (const skill of skills) assert.ok(skill.requiredLevel > 0 && skill.requiredLevel <= 50);
  const definitions = fiveClassSkillDefinitions();
  const byLibrary = new Map();
  for (const { definition } of definitions) {
    for (const spec of [definition, definition.projectile, definition.impact, definition.returnEffect].filter(Boolean)) {
      if (!byLibrary.has(spec.library)) byLibrary.set(spec.library, new Map());
      for (let value = 0; value < (spec.valueCount ?? 1); value++) {
        for (let direction = 0; direction < (spec.directionCount ?? 1); direction++) {
          for (let frame = 0; frame < spec.count; frame++) {
            const index = spec.base + value * (spec.valueStride ?? 0) + direction * (spec.directionStride ?? 0) + frame;
            const previous = byLibrary.get(spec.library).get(index);
            assert.ok(previous === undefined || previous === spec.blend, `${spec.library}/${index} mixes raw and additive source pixel modes`);
            byLibrary.get(spec.library).set(index, spec.blend);
          }
        }
      }
    }
  }
  return { skills, matrix: FIVE_CLASS_SKILL_SOURCE, definitions,
    effects: [...byLibrary].sort(([a], [b]) => a.localeCompare(b, "en"))
      .map(([library, frames]) => ({ library, frames: [...frames].sort(([a], [b]) => a - b).map(([index, normalize]) => ({ index, normalize })) })),
    actors: FIVE_CLASS_SKILL_ACTORS, sounds: FIVE_CLASS_SKILL_SOUNDS };
}
async function sourceLibrary(sourceDataDir, library) {
  const bytes = await readFile(path.join(sourceDataDir, `${library}.Lib`));
  return { original: parseLibrary(bytes), identity: sourceIdentity(library, bytes) };
}
function frameLookup(meta, index) { return Array.isArray(meta.frames) ? meta.frames.find((frame) => frame.index === index) : meta.frames?.[String(index)]; }
function effectEntryKey(entry) { return entry.spell ?? entry.effect; }

export async function planFiveClassSkills({ assetRoot, profilePath = PROFILE, sourceDataDir }) {
  const derived = collectFiveClassSkillRequirements(json(await readFile(profilePath)));
  let missingPng = 0;
  const libraries = [];
  for (const required of derived.effects) {
    const source = sourceDataDir ? await sourceLibrary(sourceDataDir, required.library) : null;
    let absent = 0, rawRgbaBytes = 0;
    for (const { index } of required.frames) {
      if (!existsSync(path.join(assetRoot, "original-effects", required.library, `${index}.png`))) absent++;
      if (source) {
        const frame = source.original.frames[index];
        assertSourceFrame(frame, index, required.library);
        rawRgbaBytes += frame.width * frame.height * 4;
      }
    }
    missingPng += absent;
    libraries.push({ library: required.library, frames: required.frames.length, missingPng: absent, rawRgbaBytes });
  }
  for (const actor of derived.actors) {
    let absent = 0;
    const source = sourceDataDir ? await sourceLibrary(sourceDataDir, actor.library) : null;
    if (source) {
      assert.equal(source.original.count, actor.frameCount);
      assert.equal(allPresentFrameIndices(source.original).length, actor.frameCount);
      for (const index of allPresentFrameIndices(source.original)) assertSourceFrame(source.original.frames[index], index, actor.library);
    }
    for (let i = 0; i < actor.frameCount; i++) if (!existsSync(path.join(assetRoot, "original-ui", actor.library, `${i}.png`))) absent++;
    missingPng += absent;
    libraries.push({ library: actor.library, frames: actor.frameCount, missingPng: absent, prefix: "original-ui" });
  }
  return { skillCount: derived.skills.length, effectDefinitionCount: derived.definitions.length, missingPng, libraries,
    requiredWavCount: derived.sounds.length, knownUnavailableWavs: derived.sounds.filter((sound) => sound.expectedSourceAbsent).map((sound) => sound.fileName) };
}

export async function exportFiveClassSkills({ assetRoot, profilePath = PROFILE, requirementsPath = DEFAULT_FIVE_CLASS_SKILL_REQUIREMENTS,
  spriteRequirementsPath = DEFAULT_FIVE_CLASS_REQUIREMENTS, sourceDataDir, sourceRoot, evidenceDir, log = console.log }) {
  assert.ok(sourceDataDir, "--source-data-dir required for export");
  const profileBytes = await readFile(profilePath), derived = collectFiveClassSkillRequirements(json(profileBytes));
  // Validate all library/frame prerequisites before creating resources.
  await planFiveClassSkills({ assetRoot, profilePath, sourceDataDir });
  const csharpRoot = sourceRoot ?? path.resolve(sourceDataDir, "../../../..");
  const sourceFiles = await Promise.all(SOURCE_FILES.map(async (file) => {
    const bytes = await readFile(path.join(csharpRoot, file));
    return { path: file, bytes: bytes.length, sha256: sha256(bytes) };
  }));
  const spriteRequirementsBytes = await readFile(spriteRequirementsPath), spriteRequirements = json(spriteRequirementsBytes);
  const beforeMagic3Bytes = await readFile(path.join(assetRoot, "original-effects/Magic3/meta.json")), beforeMagic3 = json(beforeMagic3Bytes);
  assert.equal(sha256(beforeMagic3Bytes), spriteRequirements.effects.metaSha256, "independent basic-arrow checkpoint must still match before extension");
  const checkpointDir = evidenceDir ?? path.join(PROJECT_ROOT, "artifacts/qa-evidence/five-class-assets-20261010");
  await mkdir(checkpointDir, { recursive: true });
  const savedCheckpoint = path.join(checkpointDir, "body-and-basic-archer-checkpoint-requirements.json");
  if (!existsSync(savedCheckpoint)) await writeFile(savedCheckpoint, spriteRequirementsBytes);
  const effectRecords = [];
  for (const required of derived.effects) {
    const { original, identity } = await sourceLibrary(sourceDataDir, required.library);
    const dir = path.join(assetRoot, "original-effects", required.library), metaPath = path.join(dir, "meta.json");
    await mkdir(dir, { recursive: true });
    const meta = existsSync(metaPath) ? json(await readFile(metaPath)) : { frames: {} };
    assert.ok(meta.frames && !Array.isArray(meta.frames));
    for (const { index, normalize } of required.frames) {
      const frame = original.frames[index];
      let rgba = decodeFrameRgba(original, frame);
      if (normalize) rgba = normalizeAdditiveRgba(rgba);
      meta.frames[String(index)] = await writeExactFrame(assetRoot, "original-effects", required.library, index, frame, rgba);
    }
    meta.count = original.count; meta.sourceLibrary = identity; meta.frameSet = original.frameSet;
    const metaBytes = jsonBytes(meta);
    await writeFile(metaPath, metaBytes);
    effectRecords.push({ ...required, sourceLibrary: identity, sourceFrameCount: original.count, metaSha256: sha256(metaBytes), frameSetSha256: sha256(jsonBytes(original.frameSet)) });
    log(`[five-class-skills] ${required.library}: ${required.frames.length} exact source effect frames`);
  }
  const actorRecords = [];
  for (const actor of derived.actors) {
    const { original, identity } = await sourceLibrary(sourceDataDir, actor.library);
    await mkdir(path.join(assetRoot, "original-ui", actor.library), { recursive: true });
    const frames = [];
    for (const index of allPresentFrameIndices(original)) frames.push(await writeExactFrame(assetRoot, "original-ui", actor.library, index, original.frames[index], decodeFrameRgba(original, original.frames[index])));
    const metaBytes = jsonBytes({ version: original.version, sourceLibrary: identity, count: original.count, frameSet: original.frameSet, frames });
    await writeFile(path.join(assetRoot, "original-ui", actor.library, "meta.json"), metaBytes);
    actorRecords.push({ ...actor, sourceLibrary: identity, metaSha256: sha256(metaBytes), frameSetSha256: sha256(jsonBytes(original.frameSet)) });
    log(`[five-class-skills] ${actor.library}: ${frames.length} exact source actor frames`);
  }
  const manifestPath = path.join(assetRoot, "original-effects/effects.generated.json"), manifest = json(await readFile(manifestPath));
  for (const { section, definition } of derived.definitions) {
    manifest[section] = [...manifest[section].filter((entry) => effectEntryKey(entry) !== effectEntryKey(definition)), definition]
      .sort((a, b) => effectEntryKey(a).localeCompare(effectEntryKey(b), "en"));
  }
  manifest.available = [...new Set([...manifest.available, ...derived.effects.map((entry) => entry.library)])].sort();
  await writeFile(manifestPath, jsonBytes(manifest));
  const soundIndexPath = path.join(assetRoot, "original-ui/sound-index.generated.json"), soundIndex = json(await readFile(soundIndexPath));
  const sounds = [];
  await mkdir(path.join(assetRoot, "original-ui/Sound"), { recursive: true });
  for (const sound of derived.sounds) {
    const sourcePath = path.join(sourceDataDir, "../Sound", sound.fileName);
    assert.equal(existsSync(sourcePath), !sound.expectedSourceAbsent, `${sound.fileName} exact source availability changed: re-audit`);
    if (sound.expectedSourceAbsent) {
      assert.ok(!existsSync(path.join(assetRoot, "original-ui/Sound", sound.fileName)), `${sound.fileName} must not contain an unverified substitute`);
      soundIndex.sounds[sound.id] = { fileName: sound.fileName, path: null, sourceExists: false, exactSourceExists: false, fallback: false, sourceBytes: 0 };
      sounds.push({ ...sound, sourcePath: `Sound/${sound.fileName}`, path: null, bytes: 0, sha256: null });
      continue;
    }
    const bytes = await readFile(sourcePath);
    assert.equal(bytes.toString("ascii", 0, 4), "RIFF"); assert.equal(bytes.toString("ascii", 8, 12), "WAVE");
    await writeFile(path.join(assetRoot, "original-ui/Sound", sound.fileName), bytes);
    const soundPath = `/original-ui/Sound/${sound.fileName}`;
    soundIndex.sounds[sound.id] = { fileName: sound.fileName, path: soundPath, sourceExists: true, exactSourceExists: true, fallback: false, sourceBytes: bytes.length };
    sounds.push({ ...sound, sourcePath: `Sound/${sound.fileName}`, path: soundPath, bytes: bytes.length, sha256: sha256(bytes) });
  }
  soundIndex.soundListEntryCount = Object.keys(soundIndex.sounds).length;
  soundIndex.exportedSoundCount = Object.values(soundIndex.sounds).filter((entry) => entry.sourceExists).length;
  soundIndex.missingSoundCount = Object.values(soundIndex.sounds).filter((entry) => !entry.sourceExists).length;
  await writeFile(soundIndexPath, jsonBytes(soundIndex));
  const afterMagic3Bytes = await readFile(path.join(assetRoot, "original-effects/Magic3/meta.json")), afterMagic3 = json(afterMagic3Bytes);
  assert.deepEqual(afterMagic3.sourceLibrary, beforeMagic3.sourceLibrary, "original arrow source identity remains pinned");
  for (const index of ARCHER_FRAMES) assert.deepEqual(frameLookup(afterMagic3, index), frameLookup(beforeMagic3, index), `independent original arrow/Focus frame ${index} changed`);
  spriteRequirements.effects.metaSha256 = sha256(afterMagic3Bytes);
  await writeFile(spriteRequirementsPath, jsonBytes(spriteRequirements));
  const requirements = { schemaVersion: 1, kind: "mir2-five-class-skill-closure", profileId: "platinum_fiveclass", profileSha256: sha256(profileBytes),
    skills: derived.skills, matrix: derived.matrix, definitions: derived.definitions, sourceFiles, effects: effectRecords, actors: actorRecords, sounds,
    independentArrowCheckpoint: { sha256: sha256(spriteRequirementsBytes), preservedFrames: ARCHER_FRAMES, oldMagic3MetaSha256: sha256(beforeMagic3Bytes), extendedMagic3MetaSha256: sha256(afterMagic3Bytes) } };
  await writeFile(requirementsPath, jsonBytes(requirements));
  return { skillCount: derived.skills.length, effectDefinitionCount: derived.definitions.length, effectFrameCount: effectRecords.reduce((n, entry) => n + entry.frames.length, 0),
    actorFrameCount: actorRecords.reduce((n, entry) => n + entry.frameCount, 0), availableWavCount: sounds.filter((entry) => !entry.expectedSourceAbsent).length,
    unavailableWavs: sounds.filter((entry) => entry.expectedSourceAbsent).map((entry) => entry.fileName) };
}

export async function inspectFiveClassSkillClosure({ assetRoot, profilePath = PROFILE, requirementsPath = DEFAULT_FIVE_CLASS_SKILL_REQUIREMENTS, sourceDataDir, sourceRoot, verifyPixels = false }) {
  assert.ok(!verifyPixels || sourceDataDir, "--verify-pixels requires --source-data-dir");
  const profileBytes = await readFile(profilePath), derived = collectFiveClassSkillRequirements(json(profileBytes)), required = json(await readFile(requirementsPath));
  assert.equal(required.schemaVersion, 1); assert.equal(required.kind, "mir2-five-class-skill-closure");
  assert.equal(required.profileId, "platinum_fiveclass"); assert.equal(required.profileSha256, sha256(profileBytes), "five-class profile changed: re-audit skill closure");
  for (const key of ["skills", "matrix", "definitions"]) assert.deepEqual(required[key], derived[key], `${key} denominator/contract`);
  assert.deepEqual(required.effects.map(({ library, frames }) => ({ library, frames })), derived.effects, "effect frame union denominator");
  assert.deepEqual(required.actors.map(({ sourceLibrary, metaSha256, frameSetSha256, ...rest }) => rest), derived.actors, "summoned actor denominator");
  assert.deepEqual(required.sounds.map(({ id, fileName, expectedSourceAbsent }) => ({ id, fileName, expectedSourceAbsent })), derived.sounds, "source sound denominator");
  const manifest = json(await readFile(path.join(assetRoot, "original-effects/effects.generated.json")));
  for (const { section, definition } of derived.definitions) assert.deepEqual(manifest[section].find((entry) => effectEntryKey(entry) === effectEntryKey(definition)), definition, `${section}:${effectEntryKey(definition)} source contract`);
  const verifyLibrary = async (record, prefix, actor) => {
    const metaBytes = await readFile(path.join(assetRoot, prefix, record.library, "meta.json")), meta = json(metaBytes);
    assert.equal(sha256(metaBytes), record.metaSha256, `${record.library} metadata identity`);
    assert.deepEqual(meta.sourceLibrary, record.sourceLibrary); assert.equal(meta.sourceLibrary.path, `${record.library}.Lib`);
    assert.match(meta.sourceLibrary.sha256, /^[a-f0-9]{64}$/); assert.ok(meta.sourceLibrary.bytes > 0);
    assert.equal(meta.count, actor ? record.frameCount : record.sourceFrameCount);
    assert.equal(sha256(jsonBytes(meta.frameSet)), record.frameSetSha256, `${record.library} frame set identity`);
    const source = sourceDataDir ? await sourceLibrary(sourceDataDir, record.library) : null;
    if (source) { assert.deepEqual(source.identity, record.sourceLibrary); assert.deepEqual(source.original.frameSet, meta.frameSet); }
    const frames = actor ? Array.from({ length: record.frameCount }, (_, index) => ({ index, normalize: false })) : record.frames;
    if (actor) assert.equal(meta.frames.length, record.frameCount);
    for (let start = 0; start < frames.length; start += 24) await Promise.all(frames.slice(start, start + 24).map(({ index, normalize }) => inspectFrame({ assetRoot, prefix, library: record.library, index, frame: frameLookup(meta, index), original: source?.original, normalize, verifyPixels })));
    if (actor && source) for (const index of record.sourceMissingFrames ?? []) assert.ok(!source.original.frames[index] || !source.original.frames[index].width, `${record.library} source missing frame ${index} changed: re-audit lifecycle`);
  };
  for (const record of required.effects) await verifyLibrary(record, "original-effects", false);
  for (const record of required.actors) await verifyLibrary(record, "original-ui", true);
  const soundIndex = json(await readFile(path.join(assetRoot, "original-ui/sound-index.generated.json")));
  for (const sound of required.sounds) {
    const indexed = soundIndex.sounds[sound.id], soundPath = path.join(assetRoot, "original-ui/Sound", sound.fileName);
    assert.equal(indexed.fileName, sound.fileName); assert.equal(indexed.fallback, false);
    if (sound.expectedSourceAbsent) {
      assert.equal(sound.path, null); assert.equal(sound.sha256, null); assert.equal(sound.bytes, 0);
      assert.equal(indexed.path, null); assert.equal(indexed.sourceExists, false); assert.equal(indexed.exactSourceExists, false);
      assert.ok(!existsSync(soundPath), `${sound.fileName} unverified substitute`);
      if (sourceDataDir) assert.ok(!existsSync(path.join(sourceDataDir, "../Sound", sound.fileName)));
    } else {
      const bytes = await readFile(soundPath);
      assert.equal(sha256(bytes), sound.sha256); assert.equal(bytes.length, sound.bytes);
      assert.equal(indexed.path, `/original-ui/Sound/${sound.fileName}`); assert.equal(indexed.sourceBytes, bytes.length);
      assert.equal(indexed.sourceExists, true); assert.equal(indexed.exactSourceExists, true);
      if (sourceDataDir) assert.ok(bytes.equals(await readFile(path.join(sourceDataDir, "../Sound", sound.fileName))));
    }
  }
  const csharpRoot = sourceRoot ?? (sourceDataDir ? path.resolve(sourceDataDir, "../../../..") : null);
  assert.deepEqual(required.sourceFiles.map((entry) => entry.path), SOURCE_FILES);
  if (csharpRoot) for (const entry of required.sourceFiles) {
    const bytes = await readFile(path.join(csharpRoot, entry.path));
    assert.equal(sha256(bytes), entry.sha256, `${entry.path} source hash`); assert.equal(bytes.length, entry.bytes);
  }
  return { profileId: required.profileId, skillCount: derived.skills.length, effectDefinitionCount: derived.definitions.length,
    effectFrameCount: required.effects.reduce((n, entry) => n + entry.frames.length, 0), actorFrameCount: required.actors.reduce((n, entry) => n + entry.frameCount, 0),
    availableWavCount: required.sounds.filter((entry) => !entry.expectedSourceAbsent).length,
    knownUnavailableWavs: required.sounds.filter((entry) => entry.expectedSourceAbsent).map((entry) => entry.fileName),
    sourceVerified: Boolean(sourceDataDir), sourcePixelsVerified: verifyPixels };
}

async function main() {
  const args = {};
  for (let i = 2; i < process.argv.length; i++) {
    const key = process.argv[i];
    assert.ok(["--mode", "--profile", "--assets", "--requirements", "--source-data-dir", "--source-root", "--sprite-requirements", "--evidence-dir", "--verify-pixels"].includes(key), `unknown argument ${key}`);
    args[key] = key === "--verify-pixels" ? true : process.argv[++i];
  }
  const options = { assetRoot: path.resolve(args["--assets"] ?? path.join(PROJECT_ROOT, "apps/web/public")), profilePath: path.resolve(args["--profile"] ?? PROFILE),
    requirementsPath: path.resolve(args["--requirements"] ?? DEFAULT_FIVE_CLASS_SKILL_REQUIREMENTS), spriteRequirementsPath: path.resolve(args["--sprite-requirements"] ?? DEFAULT_FIVE_CLASS_REQUIREMENTS),
    sourceDataDir: args["--source-data-dir"] ? path.resolve(args["--source-data-dir"]) : undefined, sourceRoot: args["--source-root"] ? path.resolve(args["--source-root"]) : undefined,
    evidenceDir: args["--evidence-dir"] ? path.resolve(args["--evidence-dir"]) : undefined, verifyPixels: args["--verify-pixels"] === true };
  const mode = args["--mode"] ?? "verify";
  const summary = mode === "export" ? await exportFiveClassSkills(options) : mode === "verify" ? await inspectFiveClassSkillClosure(options)
    : mode === "plan" ? await planFiveClassSkills(options) : assert.fail(`invalid mode ${mode}`);
  console.log(JSON.stringify(summary, null, 2));
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().catch((error) => { console.error(error); process.exitCode = 1; });
