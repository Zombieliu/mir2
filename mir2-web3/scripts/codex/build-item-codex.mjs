#!/usr/bin/env node
// Build the Item Codex dataset for the in-app game database (资料库).
//
// Reads the Crystal-extracted manifests under packages/game-data/data/generated
// and emits a single enriched, cross-linked JSON consumed by /codex/items.
//
// All enum-decode tables below are transcribed from the Rust crates, which are
// the authoritative mirror of Crystal (Crystal/ is a read-only submodule):
//   - Stat ids  -> packages/protocol/src/types.rs:1745 (crystal_stat_label)
//   - ItemType  -> apps/simulation/src/runtime/crystal_compat.rs:178-198
//                  + apps/simulation/src/runtime/fishing.rs:40-43
//   - ItemGrade -> apps/simulation/src/config.rs:29
//   - MirClass  -> packages/protocol/src/types.rs:29 (bitmask in required_class)
// Unknown numeric values are kept verbatim (e.g. "Type38") rather than guessed,
// matching the project's "code is authoritative" rule.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO = join(__dirname, "..", "..");
const GEN = join(REPO, "packages", "game-data", "data", "generated");
const OUT = join(REPO, "apps", "web", "public", "codex", "items.json");

const readJson = (p) => JSON.parse(readFileSync(p, "utf8"));

// --- enum decode tables (see header for source citations) ---
const STAT_LABELS = {
  0: "MinAC", 1: "MaxAC", 2: "MinMAC", 3: "MaxMAC", 4: "MinDC", 5: "MaxDC",
  6: "MinMC", 7: "MaxMC", 8: "MinSC", 9: "MaxSC", 10: "Accuracy", 11: "Agility",
  12: "HP", 13: "MP", 14: "AttackSpeed", 15: "Luck", 16: "BagWeight",
  17: "HandWeight", 18: "WearWeight", 19: "Reflect", 20: "Strong", 21: "Holy",
  22: "Freezing", 23: "PoisonAttack", 30: "MagicResist", 31: "PoisonResist",
  32: "HealthRecovery", 33: "SpellRecovery", 34: "PoisonRecovery",
  35: "CriticalRate", 36: "CriticalDamage", 40: "MaxACRatePercent",
  41: "MaxMACRatePercent", 42: "MaxDCRatePercent", 43: "MaxMCRatePercent",
  44: "MaxSCRatePercent", 45: "AttackSpeedRatePercent", 46: "HPRatePercent",
  47: "MPRatePercent", 48: "HPDrainRatePercent", 100: "ExpRatePercent",
  101: "ItemDropRatePercent", 102: "GoldDropRatePercent", 103: "MineRatePercent",
  104: "GemRatePercent", 105: "FishRatePercent", 106: "CraftRatePercent",
  107: "SkillGainMultiplier", 108: "AttackBonus", 120: "LoverExpRatePercent",
  121: "MentorDamageRatePercent", 123: "MentorExpRatePercent",
  124: "DamageReductionPercent", 125: "EnergyShieldPercent",
  126: "EnergyShieldHPGain", 127: "ManaPenaltyPercent",
  128: "TeleportManaPenaltyPercent", 129: "Hero",
};

// Only the values verified in the Rust source are named; gaps stay numeric.
const ITEM_TYPES = {
  1: "Weapon", 2: "Armour", 4: "Helmet", 5: "Necklace", 6: "Bracelet",
  7: "Ring", 8: "Amulet", 9: "Belt", 10: "Boots", 11: "Stone", 12: "Torch",
  13: "Potion", 14: "Ore", 15: "Meat", 17: "Scroll", 18: "Gem", 19: "Mount",
  20: "Book", 27: "Food", 28: "Hook", 29: "Float", 30: "Bait", 31: "Finder",
  32: "Reel", 39: "Socket",
};

const GRADES = { 0: "None", 1: "Common", 2: "Rare", 3: "Legendary", 4: "Mythical", 5: "Heroic" };

const CLASS_BITS = [
  [1 << 0, "Warrior"], [1 << 1, "Wizard"], [1 << 2, "Taoist"],
  [1 << 3, "Assassin"], [1 << 4, "Archer"],
];

// Crystal RequiredType { Level, MaxAC, MaxMAC, MaxDC, MaxMC, MaxSC }
const REQUIRED_TYPES = { 0: "Level", 1: "MaxAC", 2: "MaxMAC", 3: "MaxDC", 4: "MaxMC", 5: "MaxSC" };
// Crystal RequiredGender flags: Male=1, Female=2
const GENDER_FLAGS = [[1, "Male"], [2, "Female"]];

function decodeClasses(mask) {
  if (!mask) return [];
  const out = CLASS_BITS.filter(([bit]) => (mask & bit) !== 0).map(([, n]) => n);
  // 31 == all five classes -> render as "All classes" upstream when out.length === 5
  return out;
}
function decodeGenders(mask) {
  if (!mask) return [];
  return GENDER_FLAGS.filter(([bit]) => (mask & bit) !== 0).map(([, n]) => n);
}
function decodeStats(stats) {
  return (stats || []).map((s) => ({
    id: s.stat,
    label: STAT_LABELS[s.stat] ?? `Stat${s.stat}`,
    value: s.value,
  }));
}

const dropBasename = (s) =>
  (s || "").replace(/\\/g, "/").split("/").pop().replace(/\.[^.]+$/, "").toLowerCase();

function main() {
  const itemManifest = readJson(join(GEN, "crystal_item_manifest.json"));
  const monsterManifest = readJson(join(GEN, "crystal_monster_manifest.json"));
  const dropManifest = readJson(join(GEN, "crystal_drop_manifest.json"));

  // index drop tables by basename + table_key for monster lookup
  const tableByKey = new Map();
  for (const t of dropManifest.tables) {
    const bn = dropBasename(t.relative_path);
    if (bn && !tableByKey.has(bn)) tableByKey.set(bn, t);
    if (t.table_key) {
      const k = t.table_key.toLowerCase();
      if (!tableByKey.has(k)) tableByKey.set(k, t);
    }
  }

  // build item_name -> [{ monster, level, chanceRaw, chance }]
  const droppedBy = new Map();
  for (const mon of monsterManifest.monsters) {
    const dp = mon.drop_path;
    if (!dp) continue;
    const t = tableByKey.get(dropBasename(dp)) || tableByKey.get(dp.toLowerCase());
    if (!t) continue;
    for (const section of t.sections || []) {
      for (const e of section.entries || []) {
        const name = e.item_name;
        if (!name) continue;
        const chance =
          e.chance_denominator > 0 ? e.chance_numerator / e.chance_denominator : null;
        if (!droppedBy.has(name)) droppedBy.set(name, []);
        droppedBy.get(name).push({
          monster: mon.name,
          monsterLevel: mon.level || 0,
          isBoss: !!mon.is_boss,
          chanceRaw: e.chance_raw || null,
          chance,
        });
      }
    }
  }
  // de-dup (same monster + same chance) + sort each item's drop list by best chance
  for (const [name, list] of droppedBy) {
    const seen = new Set();
    const deduped = list.filter((d) => {
      const k = d.monster + "|" + d.chanceRaw;
      if (seen.has(k)) return false;
      seen.add(k);
      return true;
    });
    deduped.sort((a, b) => (b.chance ?? 0) - (a.chance ?? 0));
    droppedBy.set(name, deduped);
  }

  const items = itemManifest.items.map((it) => {
    const classes = decodeClasses(it.required_class);
    const sources = (droppedBy.get(it.name) || []);
    return {
      index: it.item_index,
      name: it.name,
      type: ITEM_TYPES[it.item_type] ?? `Type${it.item_type}`,
      typeId: it.item_type,
      grade: GRADES[it.grade] ?? `Grade${it.grade}`,
      gradeId: it.grade,
      requirement:
        it.required_amount > 0
          ? { type: REQUIRED_TYPES[it.required_type] ?? `Req${it.required_type}`, amount: it.required_amount }
          : null,
      classes: classes.length === 5 ? ["All"] : classes,
      genders: (() => {
        const g = decodeGenders(it.required_gender);
        return g.length === 2 ? ["Any"] : g;
      })(),
      stats: decodeStats(it.stats),
      price: it.price,
      weight: it.weight,
      durability: it.durability,
      stackSize: it.stack_size,
      shape: it.shape,
      image: it.image,
      light: it.light,
      bind: it.bind,
      unique: it.unique,
      slots: it.slots,
      needIdentify: !!it.need_identify,
      canMine: !!it.can_mine,
      canAwakening: !!it.can_awakening,
      randomStatsId: it.random_stats_id,
      droppedBy: sources,
      droppedByCount: sources.length,
      // developer view: raw Crystal record + provenance
      raw: it,
    };
  });

  const out = {
    schemaVersion: 1,
    generatedFrom: {
      itemManifest: "packages/game-data/data/generated/crystal_item_manifest.json",
      dropManifest: "packages/game-data/data/generated/crystal_drop_manifest.json",
      monsterManifest: "packages/game-data/data/generated/crystal_monster_manifest.json",
    },
    crystalDbVersion: itemManifest.crystal_db_version ?? null,
    crystalDbCustomVersion: itemManifest.crystal_db_custom_version ?? null,
    decodeSources: {
      stat: "packages/protocol/src/types.rs:1745 crystal_stat_label",
      itemType: "apps/simulation/src/runtime/crystal_compat.rs:178-198",
      itemGrade: "apps/simulation/src/config.rs:29 ItemGrade",
      mirClass: "packages/protocol/src/types.rs:29 MirClass (required_class bitmask)",
    },
    totalItems: items.length,
    itemsWithDropSource: items.filter((i) => i.droppedByCount > 0).length,
    types: [...new Set(items.map((i) => i.type))].sort(),
    grades: [...new Set(items.map((i) => i.grade))].sort(),
    items,
  };

  mkdirSync(dirname(OUT), { recursive: true });
  writeFileSync(OUT, JSON.stringify(out));
  console.log(
    `[codex] wrote ${items.length} items (${out.itemsWithDropSource} with drop sources) -> ${OUT}`
  );
}

main();
