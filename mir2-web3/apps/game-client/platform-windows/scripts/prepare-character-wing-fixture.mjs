#!/usr/bin/env node
// Create a NEW loopback QA store; never change the source store or ItemInfo data.
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const { values } = parseArgs({ options: {
  seed: { type: "string" }, account: { type: "string" }, output: { type: "string" },
} });
if (!values.seed || !values.account || !values.output) {
  throw new Error("Usage: --seed accounts.json --account seed-account --output NEW-accounts.json");
}
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
const seedPath = path.resolve(values.seed);
const outputPath = path.resolve(values.output);
if (seedPath.toLowerCase() === outputPath.toLowerCase()) throw new Error("Refuse to replace seed");
const seed = JSON.parse(await fs.readFile(seedPath, "utf8"));
const original = seed.accounts?.[values.account];
const baseline = original?.saves?.[original.characters?.[0]?.index];
if (!baseline) throw new Error("Selected seed account must have a saved character");
const catalogue = JSON.parse(await fs.readFile(path.join(root,
  "packages/game-data/data/generated/crystal_item_manifest.json"), "utf8"));
const sourceEquipment = baseline.equipment_items_json.map(JSON.parse);
const sourceArmour = sourceEquipment.find(item => item.slot === "armour");
const sourceWeapon = sourceEquipment.find(item => item.slot === "weapon");
if (!sourceArmour || !sourceWeapon) throw new Error("Seed must retain exact armour and weapon carriers");

const accounts = {};
const cases = [
  ["wing-one-m", "WingOneM", "Male", "HeavenArmour(M)", 1, 1202],
  ["wing-one-f", "WingOneF", "Female", "HeavenArmour(F)", 1, 1203],
  ["wing-two-m", "WingTwoM", "Male", "MirArmour(M)", 2, 1204],
  ["wing-two-f", "WingTwoF", "Female", "MirArmour(F)", 2, 1205],
];
const report = [];
for (const [index, [accountId, name, gender, armourName, effect, frame]] of cases.entries()) {
  const candidates = catalogue.items.filter(item => item.name === armourName);
  if (candidates.length !== 1 || candidates[0].effect !== effect) {
    throw new Error(`Source catalogue does not uniquely match ${armourName}`);
  }
  const info = candidates[0];
  const item = structuredClone(sourceArmour);
  Object.assign(item, {
    key: `crystal-item-${info.item_index}`, name: info.name, icon: info.image,
    shape: info.shape, description: "Source-catalogue character wing QA fixture",
    durability_current: info.durability, durability_max: info.durability,
    attack: 0, defence: 0,
  });
  item.user_item_metadata.item_index = info.item_index;
  const save = structuredClone(baseline);
  save.character = { index, name, gender, class: "Warrior", level: 50 };
  save.revision = 0;
  save.map_file_name = "0";
  save.map_title = "BichonProvince";
  save.position = { x: 290, y: 620 };
  save.direction = "Right";
  save.equipment_items_json = [JSON.stringify(sourceWeapon), JSON.stringify(item)];
  save.equipment_items_explicit_empty = true;
  for (const field of ["inventory_items_json", "belt_items_json", "storage_items_json",
    "hero_inventory_items_json", "buff_states_json", "quest_states_json", "skill_states_json"]) {
    save[field] = [];
  }
  const stage5 = JSON.parse(save.stage5_systems_json);
  stage5.appearance.hair = 0;
  save.stage5_systems_json = JSON.stringify(stage5);
  accounts[accountId] = {
    password: "wing-qa-local", storage_size: 80, gm_level: 0,
    characters: [save.character], saves: { [index]: save },
  };
  report.push({ accountId, characterIndex: index, character: name, gender,
    armour: armourName, itemIndex: info.item_index, effect, frame,
    position: save.position, map: "0" });
}
await fs.mkdir(path.dirname(outputPath), { recursive: true });
await fs.writeFile(outputPath, JSON.stringify({ schemaVersion: 2, nextCharacterIndex: 4,
  gameShopGlobalPurchases: {}, accounts }, null, 2) + "\n", { flag: "wx" });
console.log(JSON.stringify({ output: outputPath, sourceUnchanged: true,
  acceptance: { visualAccepted: false, sameStateCrystalPair: false }, cases: report }, null, 2));
