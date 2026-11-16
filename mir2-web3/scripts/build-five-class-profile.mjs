import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const profilePath = "packages/game-data/data/content_profiles/platinum_fiveclass.json";
const readJson = async (file) => JSON.parse(await fs.readFile(path.join(root, file), "utf8"));

// Extend the pinned classic world. Original book Shape, not its display name or
// MagicInfo.Level1, identifies the skill and the level at which it can be learnt.
export async function buildFiveClassProfile() {
  const base = await readJson("packages/game-data/data/content_profiles/platinum_176.json");
  const { items } = await readJson("packages/game-data/data/generated/crystal_item_manifest.json");
  const { magics } = await readJson("packages/game-data/data/generated/crystal_magic_manifest.json");
  const source = await fs.readFile(path.join(root, "packages/protocol/src/types.rs"), "utf8");
  const enumBody = source.match(/pub enum Spell\s*\{([\s\S]*?)\n\}/)?.[1];
  if (!enumBody) throw new Error("Canonical Spell enum is missing");
  const spells = new Map([...enumBody.matchAll(/^\s*(\w+)\s*=\s*(\d+)\s*,/gm)]
    .map(([, name, id]) => [Number(id), name]));
  const magicNames = new Set(magics.map((magic) => magic.spell));
  const classes = new Map([[8, "Assassin"], [16, "Archer"]]);
  const books = items.filter((item) => item.item_type === 20
    && classes.has(item.required_class) && item.required_type === 0
    && item.required_amount > 0 && item.required_amount <= base.acceptanceLevel);
  const extraSkills = books.map((book) => {
    const spell = spells.get(book.shape);
    if (!spell || !magicNames.has(spell)) throw new Error(`Unresolved source book ${book.name} (${book.shape})`);
    return { spell, class: classes.get(book.required_class), requiredLevel: book.required_amount };
  });
  if (new Set(extraSkills.map((skill) => skill.spell)).size !== extraSkills.length) {
    throw new Error("Duplicate five-class source book identity");
  }
  // Omit broken source/event placeholders and future-level items. Normal shops
  // and source drop tables remain authoritative; this only admits their items.
  const equipment = items.filter((item) => classes.has(item.required_class)
    && item.item_type !== 20 && item.required_type === 0
    && item.required_amount <= base.acceptanceLevel && item.price > 0
    && /^[\x20-\x7e]+$/.test(item.name));
  // These eleven source books have no table behind an admitted monster's
  // actual DropPath. Use the project's existing same-tier book override rates;
  // the other 23 books keep their original Crystal drop paths and probabilities.
  const missingBookDrops = [
    ["MentalState", "Ghoul", 10], ["Concentration", "WhiteBoar", 3],
    ["BindingShot", "EvilCentipede", 3], ["ElementalBarrier", "EvilCentipede", 3],
    ["StoneTrap", "ZumaTaurus", 3], ["VampireShot", "ZumaTaurus", 3],
    ["CatTognue", "ZumaTaurus", 3], ["CresentSlash", "RedMoonEvil", 3],
    ["PoisonShot", "RedMoonEvil", 3], ["CrippleShot", "RedMoonEvil", 3],
    ["SummonSnakes", "RedMoonEvil", 3],
  ].map(([item, monster, denominator]) => ({
    monster, item, chanceNumerator: 1, chanceDenominator: denominator, questRequired: false,
    sourceNote: "Five-class normal acquisition policy; same-tier classic skill-book override rate. Source book name/Shape retained; not an original Crystal drop probability.",
  }));
  return {
    ...base,
    profileId: "platinum_fiveclass",
    version: 1,
    source: `${base.source}; Crystal source Assassin/Archer books and equipment through level ${base.acceptanceLevel}`,
    allowedClasses: [...base.allowedClasses, "Assassin", "Archer"],
    itemWhitelist: [...new Set([...base.itemWhitelist, ...books.map((book) => book.name), ...equipment.map((item) => item.name)])],
    skills: [...base.skills, ...extraSkills],
    dropOverrides: [...base.dropOverrides, ...missingBookDrops],
  };
}

if (path.resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  const profile = await buildFiveClassProfile();
  const output = path.join(root, profilePath);
  const text = `${JSON.stringify(profile, null, 2)}\n`;
  if (process.argv.includes("--check")) {
    if (await fs.readFile(output, "utf8") !== text) throw new Error("Five-class profile is stale; regenerate it with this script");
  } else {
    await fs.mkdir(path.dirname(output), { recursive: true });
    await fs.writeFile(output, text);
  }
  console.log(JSON.stringify({ profileId: profile.profileId, version: profile.version,
    classes: profile.allowedClasses, items: profile.itemWhitelist.length, skills: profile.skills.length }));
}
