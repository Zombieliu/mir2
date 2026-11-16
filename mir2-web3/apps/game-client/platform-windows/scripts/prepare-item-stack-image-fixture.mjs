#!/usr/bin/env node
// Isolated local QA only: source catalogue unchanged, output must not exist.
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
const source = path.resolve(values.seed);
const output = path.resolve(values.output);
if (source.toLowerCase() === output.toLowerCase()) throw new Error("Refuse to replace seed");
const seed = JSON.parse(await fs.readFile(source, "utf8"));
const account = seed.accounts?.[values.account];
const baseline = account?.saves?.[account.characters?.[0]?.index];
if (!baseline) throw new Error("Seed must contain a saved character");
const manifest = JSON.parse(await fs.readFile(path.join(root,
  "packages/game-data/data/generated/crystal_item_manifest.json"), "utf8"));
const catalogue = index => {
  const rows = manifest.items.filter(info => info.item_index === index);
  if (rows.length !== 1) throw new Error(`Ambiguous source index ${index}`);
  return rows[0];
};
const cases = [];
let uid = 720_001;
function stack(index, count, container, slot) {
  const info = catalogue(index);
  if (count < 1 || count > info.stack_size) throw new Error(`Illegal source count ${index}/${count}`);
  const item = {
    key: `crystal-item-${index}`, name: info.name, icon: info.image,
    slot, unique_id: uid++, container, quantity: count,
    description: "Original catalogue stack-image QA fixture",
    durability_current: info.durability || null, durability_max: info.durability || null,
    weight: info.weight, equip_slot: info.item_type === 8 ? "amulet" : null,
    grade: "common", attack: 0, defence: 0, heal_hp: 0, heal_mp: 0,
    user_item_metadata: { item_index: index },
  };
  cases.push({ uid: item.unique_id, itemIndex: index, name: info.name,
    count, container, slot, baseImage: info.image });
  return item;
}
const inventory = [];
for (const [slot, count] of [300, 1, 199, 200, 299, 500].entries()) {
  inventory.push(stack(712, count, "bag1", slot));
}
for (const [row, index] of [710, 711].entries()) {
  for (const [column, count] of [1, 49, 50, 99, 100, 149, 150, 500].entries()) {
    inventory.push(stack(index, count, "bag1", (row + 1) * 8 + column));
  }
}
inventory.push(stack(714, 5, "bag1", 24));
inventory.push(stack(713, 1, "bag1", 25));
const belt = [[712, 199], [712, 300], [710, 49], [710, 150], [711, 49], [711, 150]]
  .map(([index, count], slot) => stack(index, count, "belt", slot));
const storage = [[712, 199], [712, 300], [710, 49], [710, 150], [711, 49], [711, 150]]
  .map(([index, count], slot) => stack(index, count, "storage", slot));
const save = structuredClone(baseline);
save.character = { index: 0, name: "StackImageQA", gender: "Male", class: "Taoist", level: 50 };
save.revision = 0;
save.map_file_name = "0";
save.map_title = "BichonProvince";
save.position = { x: 290, y: 620 };
save.direction = "Right";
save.inventory_items_json = inventory.map(JSON.stringify);
save.belt_items_json = belt.map(JSON.stringify);
save.storage_items_json = storage.map(JSON.stringify);
save.equipment_items_json = [];
save.equipment_items_explicit_empty = true;
for (const field of ["hero_inventory_items_json", "buff_states_json", "quest_states_json", "skill_states_json"]) {
  save[field] = [];
}
const stage5 = JSON.parse(save.stage5_systems_json);
stage5.appearance.hair = 0;
save.stage5_systems_json = JSON.stringify(stage5);
await fs.mkdir(path.dirname(output), { recursive: true });
await fs.writeFile(output, JSON.stringify({ schemaVersion: 2, nextCharacterIndex: 1,
  gameShopGlobalPurchases: {}, accounts: { "stack-image-qa": {
    password: "stack-image-local-qa", storage_size: 80, gm_level: 0,
    characters: [save.character], saves: { 0: save },
  } },
}, null, 2) + "\n", { flag: "wx" });
console.log(JSON.stringify({ output, sourceUnchanged: true, catalogueUnchanged: true,
  accountId: "stack-image-qa", character: save.character, map: "0", position: save.position,
  visualAccepted: false, sameStateCrystalPair: false, cases,
}, null, 2));
