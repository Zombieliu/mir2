import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { SPELL_EFFECTS } from "../export-crystal-magic-effects.mjs";
import { collectFiveClassSkillRequirements, inspectFiveClassSkillClosure } from "./five-class-skill-closure.mjs";
import { FIVE_CLASS_SKILL_SOURCE } from "./five-class-skill-source.mjs";
import { sha256 } from "./item-icon-closure.mjs";

const profile = () => ({ profileId: "platinum_fiveclass", skills: FIVE_CLASS_SKILL_SOURCE.map(({ spell, class: className }) => ({ spell, class: className, requiredLevel: 30 })) });

test("all 34 source skills include both directions, phases and all four summon actors", () => {
  const derived = collectFiveClassSkillRequirements(profile());
  assert.equal(derived.skills.length, 34);
  assert.equal(derived.definitions.length, 61);
  assert.equal(derived.effects.reduce((n, entry) => n + entry.frames.length, 0), 1508);
  assert.deepEqual(derived.actors.map((entry) => entry.image), [130, 359, 361, 362]);
  assert.equal(derived.actors.reduce((n, entry) => n + entry.frameCount, 0), 401);
  const magic = new Set(derived.effects.find((entry) => entry.library === "Magic3").frames.map((entry) => entry.index));
  assert.ok(magic.has(1210) && magic.has(1364), "StraightShot direction0 and15");
  assert.ok(!magic.has(1215), "source directional padding must not become a fabricated frame");
  assert.ok(magic.has(1617), "DelayedExplosion final-stage frame7");
  assert.ok(magic.has(1905), "16-frame persistent barrier differs from ten-frame packet-up");
  assert.ok(magic.has(2516), "CrippleShot Vampire buff last impact frame");
  const disappearance = derived.definitions.find((entry) => entry.ref === "client_effects:AssassinDarkBodyDisappear").definition;
  assert.deepEqual([disappearance.library, disappearance.base, disappearance.count, disappearance.interval, disappearance.kind],
    ["Magic2", 2600, 10, 120, "ground"]);
  assert.equal(disappearance.blend, true);
  assert.equal(disappearance.repeat, false);
  assert.match(disappearance.provenance.source, /ObjectDied\/Type=2/);
  const darkBody = derived.matrix.find((entry) => entry.spell === "DarkBody");
  assert.ok(darkBody.refs.includes("client_effects:AssassinDarkBodyDisappear"));
  assert.match(darkBody.trigger, /ObjectDied\.Type=2/);
});

test("omitting a skill or accepting a level outside the audited 1-50 slice fails", () => {
  const omitted = profile(); omitted.skills = omitted.skills.filter((entry) => entry.spell !== "CatTongue");
  assert.throws(() => collectFiveClassSkillRequirements(omitted), /34 added skills/);
  const highLevel = profile(); highLevel.skills[0].requiredLevel = 51;
  assert.throws(() => collectFiveClassSkillRequirements(highLevel));
});

test("stateful elements, concentration and net are client contracts, never an unconditional cast", () => {
  const derived = collectFiveClassSkillRequirements(profile());
  assert.equal(SPELL_EFFECTS.some((entry) => entry.spell === "ElementalShot"), false);
  const elemental = derived.matrix.find((entry) => entry.spell === "ElementalShot");
  assert.match(elemental.trigger, /HasElements && !ElementCasted/);
  const concentration = derived.matrix.find((entry) => entry.spell === "Concentration");
  assert.match(concentration.trigger, /Enabled && !Interrupted/);
  const cripple = derived.matrix.find((entry) => entry.spell === "CrippleShot");
  assert.match(cripple.trigger, /PoisonShot overrides VampireShot/);
  for (const spell of ["FlashDash", "BackStep", "MentalState"]) assert.deepEqual(derived.matrix.find((entry) => entry.spell === spell).refs, []);
});

test("source WAV gaps are explicit and never substituted with a neighbouring spell", () => {
  const derived = collectFiveClassSkillRequirements(profile());
  assert.equal(derived.sounds.length, 57);
  assert.deepEqual(derived.sounds.filter((entry) => entry.expectedSourceAbsent).map((entry) => entry.fileName),
    ["M94-0.wav", "M94-1.wav", "M107-0.wav", "M107-1.wav", "M137-7.wav", "M140-7.wav"]);
  assert.deepEqual(derived.actors.find((entry) => entry.image === 362).sourceMissingFrames, [52, 53, 54, 55, 56, 57, 58, 59]);
  assert.match(derived.actors.find((entry) => entry.image === 362).sourceLifecycle, /Dead removes/);
});

async function rejectedSnapshot(mutate, pattern) {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "mir2-fiveclass-skill-"));
  try {
    const p = profile(), bytes = Buffer.from(JSON.stringify(p)), derived = collectFiveClassSkillRequirements(p);
    const required = { schemaVersion: 1, kind: "mir2-five-class-skill-closure", profileId: p.profileId, profileSha256: sha256(bytes), ...derived };
    const changed = structuredClone(required); mutate(changed);
    const profilePath = path.join(tempRoot, "profile.json"), requirementsPath = path.join(tempRoot, "requirements.json");
    await writeFile(profilePath, bytes); await writeFile(requirementsPath, JSON.stringify(changed));
    await assert.rejects(inspectFiveClassSkillClosure({ assetRoot: tempRoot, profilePath, requirementsPath }), pattern);
  } finally {
    assert.ok(path.resolve(tempRoot).startsWith(path.resolve(os.tmpdir()) + path.sep));
    await rm(tempRoot, { recursive: true, force: true });
  }
}

test("removing a conditional direction from the required union cannot shrink package scope", async () => {
  await rejectedSnapshot((entry) => { entry.effects.find((library) => library.library === "Magic3").frames.pop(); }, /effect frame union denominator/);
});
test("dropping a summoned actor or an absent sound entry cannot pass the package denominator", async () => {
  await rejectedSnapshot((entry) => { entry.actors.pop(); }, /summoned actor denominator/);
  await rejectedSnapshot((entry) => { entry.sounds = entry.sounds.filter((sound) => sound.fileName !== "M94-0.wav"); }, /source sound denominator/);
});
test("a modified projectile stride is rejected even before opening packaged PNGs", async () => {
  await rejectedSnapshot((entry) => { entry.definitions.find((definition) => definition.ref === "spell_effects:StraightShot").definition.projectile.directionCount = 8; }, /definitions denominator\/contract/);
});
test("a stale profile hash cannot claim the current resource matrix", async () => {
  await rejectedSnapshot((entry) => { entry.profileSha256 = "0".repeat(64); }, /profile changed/);
});
