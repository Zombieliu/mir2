import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { encodePng } from "./crystal-library.mjs";
import { sha256 } from "./asset-pipeline/item-icon-closure.mjs";
import { assertEquippedSpriteClosure, collectEquippedSpriteRequirements, DEFAULT_EQUIPMENT_ITEM_CATALOGUE,
  DEFAULT_EQUIPMENT_REQUIREMENTS, inspectEquippedSpriteClosure } from "./asset-pipeline/equipment-sprite-closure.mjs";

const catalogue = { items: [
  { item_index: 1221, name: "arbitrary armour", item_type: 2, shape: 3 },
  { item_index: 1216, name: "arbitrary weapon", item_type: 1, shape: 9 },
] };
const profile = { class: "Warrior", gender: "Male", hair: 0, equipment: [
  { slot: "armour", key: "crystal-item-1221" },
  { slot: "weapon", key: "crystal-item-1216" },
] };

function fixture(t, { atlasHair = true, body = true, weapon = true } = {}) {
  const assetRoot = mkdtempSync(path.join(tmpdir(), "mir2-equipped-sprites-"));
  t.after(() => rmSync(assetRoot, { recursive: true, force: true }));
  const options = { assetRoot, requirementsPath: path.join(assetRoot, "requirements.json"),
    itemCataloguePath: path.join(assetRoot, "items.json"), sourceCataloguePath: path.join(assetRoot, "sources.json") };
  writeFileSync(options.requirementsPath, JSON.stringify({ schemaVersion: 1,
    kind: "mir2-natural-equipment-sprite-requirements", profiles: [profile] }));
  writeFileSync(options.itemCataloguePath, JSON.stringify(catalogue));
  const rgba = Buffer.from([170, 90, 30, 255, 0, 0, 0, 0]);
  const png = encodePng(2, 1, rgba);
  const libraries = ["CArmour/03", "CHair/00", "CWeapon/09"];
  const origins = libraries.map((library) => ({ path: `${library}.Lib`, sha256: sha256(Buffer.from(library)), byteLength: library.length }));
  writeFileSync(options.sourceCataloguePath, JSON.stringify({ libraries: origins }));
  const metas = new Map();
  for (const library of libraries) {
    if ((library === "CHair/00" && atlasHair) || (library === "CArmour/03" && !body)
        || (library === "CWeapon/09" && !weapon)) continue;
    const root = path.join(assetRoot, "original-ui", library);
    mkdirSync(root, { recursive: true });
    const origin = origins.find((entry) => entry.path === `${library}.Lib`);
    const meta = { sourceLibrary: { path: origin.path, sha256: origin.sha256, bytes: origin.byteLength }, frames: [] };
    for (let index = 0; index < 416; index++) {
      writeFileSync(path.join(root, `${index}.png`), png);
      meta.frames.push({ index, width: 2, height: 1, x: 2, y: -9,
        path: `/original-ui/${library}/${index}.png`, pngSha256: sha256(png), rgbaSha256: sha256(rgba) });
    }
    metas.set(library, meta);
    writeFileSync(path.join(root, "meta.json"), JSON.stringify(meta));
  }
  if (atlasHair) {
    const root = path.join(assetRoot, "bevy-entity-atlases");
    mkdirSync(root);
    writeFileSync(path.join(root, "hair.png"), png);
    writeFileSync(path.join(root, "manifest.json"), JSON.stringify({ schemaVersion: 2, atlases: [{
      pages: [{ imageUrl: "/bevy-entity-atlases/hair.png", sha256: sha256(png), imageBytes: png.length, width: 2, height: 1 }],
      rects: Array.from({ length: 416 }, (_, index) => ({ key: `/original-ui/CHair/00/${index}.png|2x1`,
        pageIndex: 0, x: 0, y: 0, width: 2, height: 1, offsetX: 2, offsetY: -9 })),
    }] }));
  }
  const writeMeta = (library) => writeFileSync(path.join(assetRoot, "original-ui", library, "meta.json"), JSON.stringify(metas.get(library)));
  return { options, png, metas, writeMeta };
}

function cli(options, extra = []) {
  return spawnSync(process.execPath, [path.join(import.meta.dirname, "build-bevy-entity-atlas-pack.mjs"),
    "--assetRoot", options.assetRoot, "--equipmentRequirements", options.requirementsPath,
    "--equipmentItemCatalogue", options.itemCataloguePath, "--equipmentSourceCatalogue", options.sourceCataloguePath,
    ...extra], { encoding: "utf8" });
}

test("equipment item types/shapes derive libraries, and gender selects complete action blocks", () => {
  const male = collectEquippedSpriteRequirements([profile, profile], catalogue);
  assert.deepEqual(male.map(({ library }) => library), ["CArmour/03", "CHair/00", "CWeapon/09"]);
  for (const { frames } of male) assert.deepEqual([frames.length, frames[0], frames.at(-1)], [416, 0, 415]);
  const female = collectEquippedSpriteRequirements([{ ...profile, gender: "Female" }], catalogue);
  for (const { library, frames } of female) {
    assert.deepEqual([frames.length, frames[0], frames.at(-1)], library.startsWith("CWeapon") ? [416, 416, 831] : [416, 808, 1223]);
  }
  assert.throws(() => collectEquippedSpriteRequirements([profile], { items: [{ ...catalogue.items[0], item_type: 1 }, catalogue.items[1]] }), /Invalid equipped armour/);
  assert.throws(() => collectEquippedSpriteRequirements([profile], { items: [catalogue.items[0]] }), /Invalid equipped weapon/);
  assert.throws(() => collectEquippedSpriteRequirements([{ ...profile, class: "Archer" }], catalogue), /Unsupported/);
});

test("normal Crystal export configuration includes every required natural-equipment action frame", () => {
  const requirements = JSON.parse(readFileSync(DEFAULT_EQUIPMENT_REQUIREMENTS));
  const items = JSON.parse(readFileSync(DEFAULT_EQUIPMENT_ITEM_CATALOGUE));
  const exports = JSON.parse(readFileSync(path.join(import.meta.dirname, "crystal-ui-export-manifest.json")));
  const libraries = collectEquippedSpriteRequirements(requirements.profiles, items);
  assert.deepEqual(libraries.map(({ library }) => library), ["CArmour/02", "CArmour/03", "CArmour/04", "CHair/00", "CWeapon/07", "CWeapon/09", "CWeapon/10"]);
  for (const { library, frames } of libraries) {
    const config = exports.libraries[library];
    assert(config, `Missing normal export library ${library}`);
    for (const frame of frames) assert(config.indices?.includes(frame)
      || config.ranges?.some(([from, to]) => frame >= from && frame <= to), `Missing normal export ${library}/${frame}`);
  }
});

test("the former hair-only package fails both library closure and the verify CLI", async (t) => {
  const { options } = fixture(t, { body: false, weapon: false });
  const report = await inspectEquippedSpriteClosure(options);
  assert.equal(report.ok, false);
  assert.equal(report.atlasFrameCount, 416);
  for (const library of ["CArmour/03", "CWeapon/09"]) assert(report.errors.some((entry) => entry.library === library));
  assert.throws(() => assertEquippedSpriteClosure(report), /Equipped sprite closure failed/);
  const result = cli(options, ["--verifyEquipmentClosure"]);
  assert.equal(result.status, 1, result.stdout);
  assert.match(result.stderr, /Equipped sprite closure failed/);
});

test("verified standalone body and weapon close an existing hair atlas without rebuilding it", async (t) => {
  const { options } = fixture(t);
  const manifestPath = path.join(options.assetRoot, "bevy-entity-atlases/manifest.json");
  const before = readFileSync(manifestPath);
  const report = assertEquippedSpriteClosure(await inspectEquippedSpriteClosure(options));
  assert.equal(report.atlasFrameCount, 416);
  assert.equal(report.standaloneFrameCount, 832);
  assert.equal(report.checkedAtlasPages, 1);
  assert.equal(cli(options, ["--verifyEquipmentClosure"]).status, 0);
  assert.deepEqual(readFileSync(manifestPath), before);
});

test("removing a movement frame cannot shrink the equipment denominator", async (t) => {
  const { options, metas, writeMeta } = fixture(t);
  rmSync(path.join(options.assetRoot, "original-ui/CArmour/03/64.png"));
  metas.get("CArmour/03").frames = metas.get("CArmour/03").frames.filter(({ index }) => index !== 64);
  writeMeta("CArmour/03");
  const report = await inspectEquippedSpriteClosure(options);
  assert.equal(report.ok, false);
  assert(report.errors.some((entry) => entry.library === "CArmour/03" && entry.frame === 64));
  assert.equal(report.libraries.find(({ library }) => library === "CArmour/03").requiredFrames, 416);
});

test("same-sized pixel substitutions and invalid original offsets fail integrity", async (t) => {
  const { options, metas, writeMeta } = fixture(t);
  writeFileSync(path.join(options.assetRoot, "original-ui/CWeapon/09/104.png"), encodePng(2, 1, Buffer.alloc(8)));
  metas.get("CArmour/03").frames[0].x = null;
  writeMeta("CArmour/03");
  const report = await inspectEquippedSpriteClosure(options);
  assert.equal(report.ok, false);
  assert(report.errors.some(({ library, frame, reason }) => library === "CWeapon/09" && frame === 104 && /hash/.test(reason)));
  assert(report.errors.some(({ library, frame, reason }) => library === "CArmour/03" && frame === 0 && /metadata/.test(reason)));
});

test("source identity changes and a corrupt atlas page fail closed", async (t) => {
  const { options, metas, writeMeta } = fixture(t);
  metas.get("CArmour/03").sourceLibrary.sha256 = "0".repeat(64);
  writeMeta("CArmour/03");
  writeFileSync(path.join(options.assetRoot, "bevy-entity-atlases/hair.png"), encodePng(2, 1, Buffer.alloc(8)));
  const report = await inspectEquippedSpriteClosure(options);
  assert.equal(report.ok, false);
  assert(report.errors.some(({ library, reason }) => library === "CArmour/03" && /identity/.test(reason)));
  assert(report.errors.some(({ library, reason }) => library === "CHair/00" && /atlas page hash/.test(reason)));
});

test("a standalone first build emits a hash-bound atlas with exact source offsets that passes closure", async (t) => {
  const { options } = fixture(t, { atlasHair: false });
  const initial = assertEquippedSpriteClosure(await inspectEquippedSpriteClosure(options));
  assert.equal(initial.standaloneFrameCount, 1248);
  const result = cli(options, ["--roots", "CArmour/03,CHair/00,CWeapon/09", "--atlasKey", "test-equipment"]);
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(readFileSync(path.join(options.assetRoot, "bevy-entity-atlases/manifest.json")));
  assert.equal(manifest.schemaVersion, 2);
  const family = manifest.atlases[0];
  assert.equal(family.pages[0].sha256, sha256(readFileSync(path.join(options.assetRoot, "bevy-entity-atlases/test-equipment.png"))));
  for (const rect of family.rects) assert.deepEqual([rect.pageIndex, rect.offsetX, rect.offsetY], [0, 2, -9]);
  const report = assertEquippedSpriteClosure(await inspectEquippedSpriteClosure(options));
  assert.equal(report.atlasFrameCount, 1248);
  assert.equal(report.standaloneFrameCount, 0);
});
