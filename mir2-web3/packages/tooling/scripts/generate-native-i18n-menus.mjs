import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { translateName, root } from "./generate-native-i18n-content.mjs";
import { menuRows } from "./native-i18n-menu-translations.mjs";

const catalogDir = resolve(root, "packages/game-data/data/native-i18n");
const sourceFile = resolve(root, "packages/tooling/data/native-i18n/npc-menu-source.json");
const manual = new Map(menuRows.map(([en, tw, pt]) => [en, { en, "zh-TW": tw, "pt-BR": pt }]));
const sources = ["common.json", "content.json"].flatMap((file) => JSON.parse(readFileSync(resolve(catalogDir, file), "utf8")).entries);
const compact = (source) => source.replace(/[\s_]/g, "").toLowerCase();
const known = new Map();
for (const entry of sources) for (const alias of [entry.en, ...entry.aliases]) if (!known.has(compact(alias))) known.set(compact(alias), entry);

export function translateMenu(source) {
  if (manual.has(source)) return { ...manual.get(source), method: "reviewed-menu" };
  if (known.has(compact(source))) return { ...known.get(compact(source)), en: source, method: "existing-display-name-or-label" };
  const skillLevel = source.match(/^(\w+) (\d+)$/);
  if (skillLevel && known.has(compact(skillLevel[1]))) {
    const base = known.get(compact(skillLevel[1]));
    return { en: source, "zh-TW": `${base["zh-TW"]} ${skillLevel[2]}`, "pt-BR": `${base["pt-BR"]} ${skillLevel[2]}`, method: "reviewed-skill-level-template" };
  }
  const numbered = source.match(/^(Guild Territory|Rebirth) (\d+)$/);
  if (numbered) { const base = manual.get(numbered[1]); return { en: source, "zh-TW": `${base["zh-TW"]} ${numbered[2]}`, "pt-BR": `${base["pt-BR"]} ${numbered[2]}`, method: "reviewed-numbered-menu" }; }
  const level = source.match(/^(?:LV\s*|[Ll]evel )(\d+)([~>\-])(\d+)$/);
  if (level) return { en: source, "zh-TW": `等級 ${level[1]}–${level[3]}`, "pt-BR": `Níveis ${level[1]}–${level[3]}`, method: "reviewed-level-range" };
  const minimumLevel = source.match(/^[Ll]evel (\d+)\+$/);
  if (minimumLevel) return { en: source, "zh-TW": `等級 ${minimumLevel[1]} 以上`, "pt-BR": `Nível ${minimumLevel[1]} ou superior`, method: "reviewed-level-range" };
  const tax = source.match(/^Tax to (\d+)%$/);
  if (tax) return { en: source, "zh-TW": `稅率設為 ${tax[1]}%`, "pt-BR": `Definir imposto em ${tax[1]}%`, method: "reviewed-tax-percentage" };
  const map = translateName(source.replaceAll(" ", ""), "map");
  if (map) return { ...map, en: source, method: "existing-map-display-name" };
  return null;
}

export function generateMenus() {
  const source = JSON.parse(readFileSync(sourceFile, "utf8")).entries;
  const missing = [], entries = [], methods = {};
  for (const row of source) {
    const translation = translateMenu(row.en);
    if (!translation) { missing.push({ key: row.key, en: row.en }); continue; }
    methods[translation.method] = (methods[translation.method] ?? 0) + 1;
    entries.push({ key: row.key, en: translation.en, "zh-TW": translation["zh-TW"], "pt-BR": translation["pt-BR"], aliases: row.aliases });
  }
  const report = { sourceLabels: source.length, translatedLabels: entries.length, missing, methods };
  writeFileSync(resolve(catalogDir, "npc-menus.json"), JSON.stringify({ entries }, null, 2) + "\n");
  writeFileSync(resolve(catalogDir, "npc-menu-coverage.json"), JSON.stringify(report, null, 2) + "\n");
  return report;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) console.log(JSON.stringify(generateMenus(), null, 2));
