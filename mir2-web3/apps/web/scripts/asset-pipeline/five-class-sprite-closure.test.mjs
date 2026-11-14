import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { sha256 } from "./item-icon-closure.mjs";
import { ARCHER_FRAMES, collectFiveClassSpriteRequirements, inspectFiveClassSpriteClosure } from "./five-class-sprite-closure.mjs";

const profile = { profileId: "platinum_fiveclass", allowedClasses: ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"],
  itemWhitelist: ["DualBlade", "Bow", "AssassinArmour", "ArcherArmour", "PickAxe"] };
const catalogue = { items: [
  { item_index: 1, name: "DualBlade", item_type: 1, shape: 104, required_class: 8, required_gender: 3, image: 100 },
  { item_index: 2, name: "Bow", item_type: 1, shape: 213, required_class: 16, required_gender: 3, image: 101 },
  { item_index: 3, name: "AssassinArmour", item_type: 2, shape: 29, required_class: 8, required_gender: 2, image: 102 },
  { item_index: 4, name: "ArcherArmour", item_type: 2, shape: 39, required_class: 16, required_gender: 1, image: 103 },
  { item_index: 5, name: "PickAxe", item_type: 1, shape: 42, required_class: 31, required_gender: 3, image: 104 },
  { item_index: 6, name: "ExcludedBow", item_type: 1, shape: 222, required_class: 16, required_gender: 3, image: 105 },
] };
const asBytes = (value) => Buffer.from(`${JSON.stringify(value, null, 2)}\n`);

test("high-tier equipment closes normal and alternate bodies plus both weapon render modes", () => {
  const requirements = collectFiveClassSpriteRequirements(profile, catalogue);
  const library = new Map(requirements.libraries.map((entry) => [entry.library, entry]));
  assert.equal(requirements.equipment.length, 5);
  for (const required of ["AArmour/29", "CArmour/29", "ARArmour/39", "CArmour/39", "AWeapon/04 L", "AWeapon/04 R",
    "ARWeapon/13", "ARWeapon/13 S", "CWeapon/42", "AHair/08", "ARHair/08", "CHair/08"]) assert.ok(library.has(required), required);
  assert.equal(library.get("AWeapon/04 L").frameCount, 1024, "female Sneek tail must be exported");
  assert.equal(library.get("ARWeapon/13").frameCount, 832, "female ordinary bow tail must be exported");
  assert.equal(library.get("ARWeapon/13 S").frameCount, 704);
  assert.equal(library.get("CArmour/29").frameCount, 1616, "does not export unused 8080-frame extensions");
  assert.deepEqual(library.get("CWeapon/42").usage, ["Archer", "Assassin"]);
  assert.ok(!library.has("ARWeapon/22"), "non-whitelisted source gear is excluded");
});

test("classic three-class profile cannot be mistaken for five-class resource closure", () => {
  assert.throws(() => collectFiveClassSpriteRequirements({ ...profile, profileId: "platinum_176" }, catalogue), /identity/);
  assert.throws(() => collectFiveClassSpriteRequirements({ ...profile, allowedClasses: profile.allowedClasses.slice(0, 3) }, catalogue), /class list/);
});

test("normal arrows include all sixteen source directions, not the gaps between them", () => {
  assert.equal(new Set(ARCHER_FRAMES).size, 90);
  assert.ok(ARCHER_FRAMES.includes(1030) && ARCHER_FRAMES.includes(1184) && ARCHER_FRAMES.includes(2739));
  assert.ok(!ARCHER_FRAMES.includes(1035) && !ARCHER_FRAMES.includes(1189));
});

async function fixture() {
  const root = await mkdtemp(path.join(tmpdir(), "mir2-five-class-closure-"));
  const profilePath = path.join(root, "profile.json");
  const cataloguePath = path.join(root, "catalogue.json");
  const requirementsPath = path.join(root, "requirements.json");
  await writeFile(profilePath, asBytes(profile));
  await writeFile(cataloguePath, asBytes(catalogue));
  const derived = collectFiveClassSpriteRequirements(profile, catalogue);
  const required = { schemaVersion: 1, kind: "mir2-five-class-sprite-closure", profileId: "platinum_fiveclass",
    profileSha256: sha256(await readFile(profilePath)), catalogueSha256: sha256(await readFile(cataloguePath)),
    equipment: derived.equipment, libraries: derived.libraries };
  await writeFile(requirementsPath, asBytes(required));
  return { root, profilePath, cataloguePath, requirementsPath, required, assetRoot: path.join(root, "assets") };
}

test("removing an entire library from a package denominator still fails closed", async () => {
  const options = await fixture();
  options.required.libraries.pop();
  await writeFile(options.requirementsPath, asBytes(options.required));
  await assert.rejects(inspectFiveClassSpriteClosure(options), /required library denominator/);
});

test("metadata with a missing female frame fails even if its own source hashes were updated", async () => {
  const options = await fixture();
  const library = options.required.libraries[0];
  const sourceLibrary = { path: `${library.library}.Lib`, sha256: "1".repeat(64), bytes: 1000 };
  const frameSet = { seek: null, count: 0, actions: [] };
  const meta = asBytes({ count: library.frameCount, sourceLibrary, frameSet,
    frames: Array.from({ length: library.frameCount - 1 }, (_, index) => ({ index })) });
  Object.assign(library, { sourceLibrary, metaSha256: sha256(meta), frameSetSha256: sha256(asBytes(frameSet)), sourceFrameCount: library.frameCount });
  const directory = path.join(options.assetRoot, "original-ui", library.library);
  await mkdir(directory, { recursive: true });
  await writeFile(path.join(directory, "meta.json"), meta);
  await writeFile(options.requirementsPath, asBytes(options.required));
  await assert.rejects(inspectFiveClassSpriteClosure(options), /full frame count/);
});

test("a changed profile invalidates a stale resource snapshot before inspecting asset files", async () => {
  const options = await fixture();
  await writeFile(options.profilePath, asBytes({ ...profile, version: 2 }));
  await assert.rejects(inspectFiveClassSpriteClosure(options), /profile changed/);
});
