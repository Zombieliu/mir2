import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { createHash } from "node:crypto";
import * as vocabulary from "./native-i18n-translations.mjs";
import { questRows, questCompletion } from "./native-i18n-quest-translations.mjs";
import { tooltipCopy } from "./native-i18n-tooltip-translations.mjs";
import { sharedPortuguese, sharedTraditional } from "./native-i18n-shared-overrides.mjs";
import { assertReviewedSources } from "./native-i18n-source-guard.mjs";

export const root = resolve(import.meta.dirname, "../../..");
const generated = resolve(root, "packages/game-data/data/generated");
const destination = resolve(root, "packages/game-data/data/native-i18n");
const read = (name) => JSON.parse(readFileSync(resolve(generated, name), "utf8"));
export const plainText = (text) => text.replace(/\{([^{}]+)\/[^{}]+\}/g, "$1");
const humanize = (value) => value.replace(/_/g, " ").replace(/([a-z])([A-Z])/g, "$1 $2").replace(/([A-Z])([A-Z][a-z])/g, "$1 $2").trim();
const dictionary = (list = []) => new Map(list.map(([en, tw, pt]) => [en, { en: humanize(en), "zh-TW": tw, "pt-BR": pt }]));
const names = dictionary(vocabulary.exactNames);
const npcRoles = dictionary(vocabulary.npcRoles);
const itemNouns = dictionary(vocabulary.itemNouns);
const itemModifiers = dictionary(vocabulary.itemModifiers);
const mapNames = dictionary(vocabulary.mapNames);
const questNames = dictionary(vocabulary.questNames);
const prose = dictionary(vocabulary.proseRows);
const hash = (value) => createHash("sha256").update(value).digest("hex").slice(0, 12);
const suffix = (translation, en, tw, pt) => ({ en: translation.en + en, "zh-TW": translation["zh-TW"] + tw, "pt-BR": translation["pt-BR"] + pt });

export function decodeQuestText(hex) {
  const bytes = Buffer.from(hex, "hex");
  let cursor = 0;
  const i32 = () => { if (cursor + 4 > bytes.length) throw new Error("Truncated quest integer"); const value = bytes.readInt32LE(cursor); cursor += 4; return value; };
  const string = () => {
    let length = 0, shift = 0, part;
    do { if (cursor >= bytes.length || shift > 28) throw new Error("Invalid quest string length"); part = bytes[cursor++]; length += (part & 127) * 2 ** shift; shift += 7; } while (part & 128);
    if (length > 1024 * 1024 || cursor + length > bytes.length) throw new Error("Truncated quest string");
    const result = bytes.subarray(cursor, cursor + length).toString("utf8"); cursor += length; return result;
  };
  const index = i32(); i32(); const name = string(), group = string(); const output = { index, name, group };
  for (const field of ["description", "task", "return", "completion"]) {
    const count = i32(); if (count < 0 || count > 4096) throw new Error("Invalid quest text count");
    output[field] = Array.from({ length: count }, string);
  }
  return output;
}

export function translateName(source, family, depth = 0) {
  if (depth > 8 || !source) return null;
  if (names.has(source)) return { ...names.get(source), method: "reviewed-name" };
  if (family === "map" && mapNames.has(source)) return { ...mapNames.get(source), method: "reviewed-map" };
  if (vocabulary.properNames.has(source)) return { en: humanize(source), "zh-TW": humanize(source), "pt-BR": humanize(source), method: "explicit-proper-name" };
  const variant = source.match(/^(.*?)(\d+)$/);
  if (variant && variant[1]) {
    const base = translateName(variant[1], family, depth + 1);
    if (base) return { ...suffix(base, ` ${variant[2]}`, ` ${variant[2]}`, ` ${variant[2]}`), method: base.method };
  }
  if (family === "map") {
    const floor = source.match(/^(.*?)_?(\d+)F$/);
    if (floor) { const base = translateName(floor[1], family, depth + 1); if (base) return { ...suffix(base, ` ${floor[2]}F`, ` ${floor[2]}層`, ` · ${floor[2]}º andar`), method: base.method }; }
    const direction = source.match(/^(.*?)\(([NSEW])\)$/);
    if (direction) { const base = translateName(direction[1], family, depth + 1); const dirs = { N: ["北", "norte"], S: ["南", "sul"], E: ["東", "leste"], W: ["西", "oeste"] }; if (base) return { ...suffix(base, ` (${direction[2]})`, `（${dirs[direction[2]][0]}）`, ` (${dirs[direction[2]][1]})`), method: base.method }; }
  }
  if (family === "npc") {
    const split = source.indexOf("_");
    if (split > 0 && npcRoles.has(source.slice(0, split))) {
      const role = npcRoles.get(source.slice(0, split)), person = source.slice(split + 1);
      return { en: `${role.en} ${person}`, "zh-TW": `${role["zh-TW"]} ${person}`, "pt-BR": `${role["pt-BR"]} ${person}`, method: "reviewed-role-preserved-personal-name" };
    }
  }
  for (const [prefix, tw, pt] of [["Ancient_", "遠古", "ancestral"], ["Frozen_", "冰封", "congelado"]]) {
    if (source.startsWith(prefix)) { const base = translateName(source.slice(prefix.length), family, depth + 1); if (base) return { en: `${prefix.replace("_", "")} ${base.en}`, "zh-TW": tw + base["zh-TW"], "pt-BR": `${base["pt-BR"]} (${pt})`, method: "reviewed-name-composition" }; }
  }
  if (family === "item") {
    const tag = source.match(/^(\[H\]|\(金龍\)|\(幻影當籤\)|\(新\)|\(慶\)|\(진\))(.*)$/);
    if (tag) { const base = translateName(tag[2], family, depth + 1); const tags = { "[H]": ["高級", "Superior", "Superior"], "(金龍)": ["金龍", "Golden Dragon", "Dragão Dourado"], "(幻影當籤)": ["幻影獎勵", "Phantom Prize", "Prêmio Fantasma"], "(新)": ["新", "New", "Novo"], "(慶)": ["慶典", "Celebration", "Celebração"], "(진)": ["真", "True", "Verdadeiro"] }; if (base) return { ...suffix(base, ` (${tags[tag[1]][1]})`, `（${tags[tag[1]][0]}）`, ` (${tags[tag[1]][2]})`), method: base.method }; }
    const duration = source.match(/^(.*?)\[(\d+)([dhM])\]$/);
    if (duration) { const base = translateName(duration[1], family, depth + 1); const units = { d: ["天", "dias"], h: ["小時", "horas"], M: ["個月", "meses"] }; if (base) return { ...suffix(base, ` [${duration[2]}${duration[3]}]`, `（${duration[2]}${units[duration[3]][0]}）`, ` (${duration[2]} ${units[duration[3]][1]})`), method: base.method }; }
    const grade = source.match(/^(.*?)\((寶|聖|神|寶物|聖物|神物|\+?\d+)\)$/);
    if (grade) { const base = translateName(grade[1], family, depth + 1); const grades = { 寶: ["Treasure", "Tesouro"], 聖: ["Holy", "Sagrado"], 神: ["Divine", "Divino"], 寶物: ["Treasure", "Tesouro"], 聖物: ["Holy Relic", "Relíquia Sagrada"], 神物: ["Divine Relic", "Relíquia Divina"] }; const label = grades[grade[2]] ?? [grade[2], grade[2]]; if (base) return { ...suffix(base, ` (${label[0]})`, `（${grade[2]}）`, ` (${label[1]})`), method: base.method }; }
    const experience = source.match(/^EXP(\d+)%$/);
    if (experience) return { en: `Experience +${experience[1]}%`, "zh-TW": `經驗提升 ${experience[1]}%`, "pt-BR": `Experiência +${experience[1]}%`, method: "reviewed-item-pattern" };
    // Consumables use (M) for medium. Armour uses M/F as a gender pair.
    const gender = source.match(/^(.*?)\((M|F)\)$/);
    if (gender && (gender[2] === "F" || !/(Potion|Drug|Stone|Amulet|Bundle|Ball|Aid|Torch|Box|Knapsack)$/.test(gender[1]))) { const base = translateName(gender[1], family, depth + 1); if (base) return { ...suffix(base, ` (${gender[2]})`, gender[2] === "M" ? "（男）" : "（女）", gender[2] === "M" ? " (masculino)" : " (feminino)"), method: base.method }; }
    const size = source.match(/^(.*?)\((S|M|L|XL|H|WAR|WIZ|TAO|ASSA|ARCH)\)$/);
    if (size) { const base = translateName(size[1], family, depth + 1); const labels = { S: ["小", "pequeno"], M: ["中", "médio"], L: ["大", "grande"], XL: ["特大", "extragrande"], H: ["高級", "superior"], WAR: ["戰士", "guerreiro"], WIZ: ["法師", "mago"], TAO: ["道士", "taoísta"], ASSA: ["刺客", "assassino"], ARCH: ["弓箭手", "arqueiro"] }; if (base) return { ...suffix(base, ` (${size[2]})`, `（${labels[size[2]][0]}）`, ` (${labels[size[2]][1]})`), method: base.method }; }
    if (itemNouns.has(source)) return { ...itemNouns.get(source), method: "reviewed-item-term" };
    for (const noun of [...itemNouns.keys()].sort((a, b) => b.length - a.length)) {
      let modifier = null;
      if (source.endsWith(noun) && source.length > noun.length) modifier = source.slice(0, -noun.length);
      if (source.startsWith(noun + "Of")) modifier = source.slice(noun.length + 2);
      if (!modifier || !itemModifiers.has(modifier)) continue;
      const a = itemNouns.get(noun), b = itemModifiers.get(modifier);
      const feminine = /^(Espada|Lâmina|Faca|Vara|Baioneta|Botas|Armadura|Luva|Manopla|Coleira|Gema|Pedra|Água|Adaga|Maça|Foice|Lança|Varinha|Tiara|Máscara|Tocha|Fita|Rédea|Boia|Isca|Carretilha|Carne|Castanha|Fruta|Poção|Sopa|Pílula|Caixa|Escama|Perna|Carapaça|Linha|Carta|Corda|Tira|Pulseira|Fivela|Conta|Flauta|Roupa|Garra)/.test(a["pt-BR"]);
      let attribute = b["pt-BR"];
      if (attribute.startsWith("^")) { attribute = attribute.slice(1); if (feminine) attribute = attribute.replace(/o\b/g, "a"); if (["Boots", "Shoes", "Blades", "Teeth"].includes(noun)) attribute = attribute.endsWith("l") ? attribute.slice(0, -1) + "is" : attribute + "s"; }
      else attribute = `de ${attribute}`;
      return { en: humanize(source), "zh-TW": b["zh-TW"] + a["zh-TW"], "pt-BR": `${a["pt-BR"]} ${attribute}`, method: "reviewed-item-composition" };
    }
  }
  return null;
}

export function generate() {
  assertReviewedSources();
  const bundle = read("localization_bundle.json"); const english = bundle.languages.en.texts;
  if (!bundle.languages["zh-TW"]) throw new Error("Run generate-native-i18n-traditional.py first");
  const commonOverrides = dictionary(vocabulary.commonRows);
  const commonEntries = Object.entries(english).map(([key, en]) => {
    const overrides = commonOverrides.get(en);
    return { key, en, "zh-TW": sharedTraditional[en] ?? overrides?.["zh-TW"] ?? bundle.languages["zh-TW"].texts[key] ?? en, "pt-BR": sharedPortuguese[en] ?? overrides?.["pt-BR"] ?? bundle.languages["pt-BR"].texts[key] ?? en, aliases: [...new Set([en, bundle.languages["zh-CN"].texts[key]].filter(Boolean))] };
  });
  for (const [spell, title] of Object.entries(vocabulary.skillTitleOverrides ?? {})) {
    const entry = commonEntries.find((row) => row.key === `client.${spell}SkillDescription`);
    if (entry) entry["pt-BR"] = title + entry["pt-BR"].slice(entry["pt-BR"].indexOf("\n"));
  }
  const snakes = commonEntries.find((entry) => entry.key === "client.SummonSnakesSkillDescription");
  if (snakes && !snakes["pt-BR"].includes("{0}")) snakes["pt-BR"] += "\n\nNível atual da habilidade {0}\nPróximo nível {1}";
  // Four old zh-CN labels embedded a shortcut absent from the English key.
  // Keep those dynamic forms under their own complete parameterized entries.
  for (const key of ["client.Exit", "client.Fishing", "client.Groups", "client.LogOut", "client.Guild", "client.Quests", "client.Skills", "client.Trade"]) {
    const entry = commonEntries.find((row) => row.key === key);
    entry.aliases = entry.aliases.filter((alias) => !alias.includes("{0}"));
  }
  for (const [en, tw, pt] of vocabulary.commonRows) if (!commonEntries.some((entry) => entry.en === en)) commonEntries.push({ key: `common.${hash(en)}`, en, "zh-TW": tw, "pt-BR": pt, aliases: [en, ...(vocabulary.commonLegacyAliases[en] ?? [])] });
  for (const entry of commonEntries) entry.aliases = [...new Set([...entry.aliases, ...(vocabulary.commonLegacyAliases[entry.en] ?? [])])];
  for (const entry of commonEntries) if (/^content\.(entity|item|map)\..*\.name$/.test(entry.key)) {
    const family = entry.key.split(".")[1];
    const canonical = entry.en.replaceAll(" ", "");
    const translation = translateName(canonical, family === "entity" ? "npc" : family) ?? translateName(canonical, "monster");
    if (translation) for (const locale of ["zh-TW", "pt-BR"]) entry[locale] = translation[locale];
  }
  const knownShared = new Map(commonEntries.map((entry) => [entry.en, entry]));
  for (const entry of commonEntries) if (entry.key.endsWith("SkillDescription")) {
    const spell = entry.key.slice(7, -16); const first = (text) => text.split(/\r?\n/)[0];
    if (!names.has(spell)) names.set(spell, { en: first(entry.en), "zh-TW": first(entry["zh-TW"]), "pt-BR": first(entry["pt-BR"]) });
  }
  const entries = [], coverage = { schemaVersion: 1, generatedBy: "generate-native-i18n-content.mjs", sourceHashAlgorithm: "sha256", catalogues: {}, missingTranslations: [], sameAsEnglish: [], absentSourceFields: [], npcDialogue: { sourceAudit: "packages/tooling/data/native-i18n/npc-source-audit.json", menuCatalogue: "npc-menus.json", proseCatalogue: "npc-prose.json", validation: "validation-report.json" }, notes: ["Name and text catalogue presence does not prove native call-site coverage.", "Proper names are retained only with an explicit classification; unknown prose is a missing translation.", "Source descriptions are translated as written, not a gameplay balance/specification claim.", "The original placeholder monster name 00 and invalid item tooltip wont remain identified source defects, not newly invented gameplay explanations."] };
  const add = (key, source, translated, aliases = [], family = "") => {
    if (!source?.trim()) return;
    const clean = plainText(source); translated ??= prose.get(clean) ?? knownShared.get(clean);
    const method = translated?.method ?? (translated ? "reviewed-text-or-existing-shared" : "missing-source-fallback");
    const entry = { key, en: translated?.en ?? clean, "zh-TW": translated?.["zh-TW"] ?? clean, "pt-BR": translated?.["pt-BR"] ?? clean, aliases: [...new Set([source, clean, ...aliases].filter(Boolean))] };
    entries.push(entry);
    if (!translated) coverage.missingTranslations.push({ key, family, source: clean });
    if (entry["zh-TW"] === entry.en || entry["pt-BR"] === entry.en) coverage.sameAsEnglish.push({ key, method, locales: ["zh-TW", "pt-BR"].filter((locale) => entry[locale] === entry.en) });
  };
  const manifests = [
    ["item", "crystal_item_manifest.json", "items", "item_index", "name"],
    ["monster", "crystal_monster_manifest.json", "monsters", "monster_index", "name"],
    ["npc", "crystal_npc_info_manifest.json", "npcs", "npc_index", "name"],
    ["magic", "crystal_magic_manifest.json", "magics", "spell", "name"],
    ["map", "crystal_respawn_manifest.json", "maps", "map_index", "map_title"],
  ];
  for (const [family, file, field, id, display] of manifests) {
    const data = read(file)[field];
    coverage.catalogues[family] = { source: `packages/game-data/data/generated/${file}`, sha256: createHash("sha256").update(readFileSync(resolve(generated, file))).digest("hex"), records: data.length, emptyNames: data.filter((row) => !row[display]).length };
    for (const row of data) {
      add(`content.${family}.${row[id]}.name`, row[display], translateName(row[display], family), [], family);
      if (family === "item" && row.tooltip) add(`content.item.${row[id]}.description`, row.tooltip, tooltipCopy(row.tooltip) ?? translateName(row.tooltip.trim().replace(/\s+/g, ""), "item"), [], family);
      if (family === "magic") {
        const legacyDescriptionNames = { DoubleSlash: "DoubleSlashToggle", FatalSword: "FatalSwordPassive", Haste: "HasteBuff" };
        const descriptionName = legacyDescriptionNames[row.spell] ?? row.spell;
        const description = commonEntries.find((entry) => entry.key.toLowerCase() === `client.${descriptionName}SkillDescription`.toLowerCase());
        if (description) add(`content.magic.${row[id]}.description`, description.en, description, [], family);
        else coverage.absentSourceFields.push({ key: `content.magic.${row[id]}.description`, reason: "No matching shared skill description key" });
      }
    }
  }
  const quests = read("crystal_quest_packet_manifest.json").quests;
  coverage.catalogues.quest = { source: "packages/game-data/data/generated/crystal_quest_packet_manifest.json", sha256: createHash("sha256").update(readFileSync(resolve(generated, "crystal_quest_packet_manifest.json"))).digest("hex"), records: quests.length };
  for (const row of quests) {
    const parsed = decodeQuestText(row.payload_hex);
    if (parsed.index !== row.index || parsed.name !== row.name) throw new Error(`Quest identity mismatch ${row.index}`);
    const copy = questRows.find((entry) => entry.id === row.index);
    const fieldCopy = (source, values) => values && values[0] !== "-" ? { en: plainText(source), "zh-TW": values[0], "pt-BR": values[1], method: "reviewed-original-quest" } : null;
    add(`content.quest.${row.index}.name`, row.name, fieldCopy(row.name, copy?.title) ?? questNames.get(row.name), [], "quest");
    for (const field of ["description", "task", "return", "completion"]) {
      const source = parsed[field].join("\n"); if (!source.trim()) continue;
      const translated = fieldCopy(source, field === "completion" ? questCompletion[row.index] : copy?.[field]);
      add(`content.quest.${row.index}.${field}`, source, translated, [parsed[field].map(plainText).join(" ")], "quest");
      // A paragraph is not duplicated as the translation of each wrapped line.
      // Clients render this stable field key before applying their own wrapping.
      for (const [index, line] of parsed[field].entries()) if (prose.has(plainText(line))) add(`content.quest.${row.index}.${field}.${index}`, line, prose.get(plainText(line)), [], "quest");
    }
  }
  const output = { entries: entries.sort((a, b) => a.key.localeCompare(b.key, "en")) };
  coverage.sharedBaseKeys = Object.keys(english).length;
  coverage.commonEntries = commonEntries.length;
  for (const [family, details] of Object.entries(coverage.catalogues)) {
    const prefix = `content.${family}.`;
    details.translatedNames = entries.filter(({ key }) => key.startsWith(prefix) && key.endsWith(".name")).length;
    details.translatedDescriptions = entries.filter(({ key }) => key.startsWith(prefix) && key.endsWith(".description")).length;
    if (family === "quest") for (const field of ["task", "return", "completion"]) details[`translated${field[0].toUpperCase()}${field.slice(1)}Fields`] = entries.filter(({ key }) => key.startsWith(prefix) && key.endsWith(`.${field}`)).length;
  }
  // Keep the shared catalogue and its web mirror reproducible too. Existing
  // language selectors and English/zh-CN/es gameplay content are unchanged.
  const overridesPath = resolve(root, "packages/game-data/data/i18n-overrides.json");
  const overrides = JSON.parse(readFileSync(overridesPath, "utf8"));
  for (const entry of commonEntries) if (Object.hasOwn(english, entry.key)) for (const [locale, overrideKey] of [["zh-TW", "zhTW"], ["pt-BR", "ptBR"]]) {
    bundle.languages[locale].texts[entry.key] = entry[locale];
    overrides[overrideKey][entry.key] = entry[locale];
  }
  for (const path of [resolve(generated, "localization_bundle.json"), resolve(root, "apps/web/lib/generated/localization_bundle.json")]) writeFileSync(path, JSON.stringify(bundle, null, 2) + "\n");
  writeFileSync(overridesPath, JSON.stringify(overrides, null, 2) + "\n");
  mkdirSync(destination, { recursive: true });
  for (const [file, value] of [["common.json", { entries: commonEntries }], ["content.json", output], ["content-coverage.json", coverage]]) writeFileSync(resolve(destination, file), JSON.stringify(value, null, 2) + "\n");
  console.log(JSON.stringify({ common: commonEntries.length, content: entries.length, gaps: coverage.missingTranslations.length, properOrSame: coverage.sameAsEnglish.length }));
  return { commonEntries, ...output, coverage };
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) generate();
