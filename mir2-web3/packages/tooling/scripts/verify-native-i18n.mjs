import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { createHash } from "node:crypto";
import { properNames } from "./native-i18n-translations.mjs";
import { assertReviewedSources } from "./native-i18n-source-guard.mjs";

export const locales = ["en", "zh-TW", "pt-BR"];
const root = resolve(import.meta.dirname, "../../..");
const directory = resolve(root, "packages/game-data/data/native-i18n");
export const catalogueFiles = ["common.json", "content.json", "shell.json", "quest.json", "game.json", "help.json", "npc-menus.json", "npc-prose.json"];
// Match native_i18n::is_slot: colour markup stays literal; a numeric .NET
// formatting suffix changes presentation, never the captured parameter name.
export const placeholders = (text) => [...text.matchAll(/\{([A-Za-z0-9_]+)\}|\{(\d+):[0-9#,.*%+; PpFfNnDdXxGgEeCcRr\-]+\}/g)].map((match) => match[1] ?? match[2]).sort();
const signature = (text) => JSON.stringify(placeholders(text));
const commandTokens = (text) => [...text.matchAll(/(?<![\w.])@[A-Za-z][A-Za-z0-9_]*(?:\([^\r\n)]*\))?/g)].map((match) => match[0]).sort();
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

export function classifyAliasConflict(alias, owners) {
  if (["Return", "New", "All", "返回", "全部"].includes(alias)) return "context-bound; use the UI/NPC/quest stable key or domain lookup";
  if (["Guild", "公會", "行會", "Status", "Estado"].includes(alias)) return "terminology/style difference; use one reviewed UI term";
  if (owners.some(({ file }) => file.startsWith("npc-"))) return "NPC-domain isolation required; menu/prose aliases must not override global UI";
  if (owners.some(({ file, key }) => file === "content.json" && /\.(name|description)$/.test(key)) && owners.every(({ file, key }) => file === "content.json" || (file === "common.json" && (key.startsWith("content.") || key.endsWith("SkillDescription"))))) return "canonical content priority; use stable content ID, preserving authored aliases";
  if (owners.every(({ values }) => values["zh-TW"] === owners[0].values["zh-TW"] && values["pt-BR"] === owners[0].values["pt-BR"])) return "English spelling/spacing or synonym only; localized outputs agree";
  return "context review required; all owner keys and values retained";
}

const autonymKeys = new Set(["ui.languageEnglish", "ui.languageChinese", "ui.languageSpanish"]);
const portugueseSameWords = new Set(["Menu", "MENU", "menu", "Mana", "Mentor", "MENTOR", "ITEM", "Item", "Status", "Normal", "OK", "Ok", "ONLINE", "Offline", "Online", "Portal", "Ginseng", "Clone", "legal", "Chat", "Auto", "mouse"]);
const proper = new Set([...properNames].map((value) => value.replace(/_/g, "").replace(/\s/g, "")));
export function sameValueReason(entry, locale) {
  const text = entry.en;
  if (entry[locale] !== text) return null;
  if (autonymKeys.has(entry.key)) return "language autonym in retained legacy catalogue; not a selectable native locale";
  if (text === "Legend of Mir 2") return "product proper name";
  if (text === "00") return "original authored placeholder name; not translated prose";
  if (entry.key === "npc.prose.8a30a8fdeadb9b8d" && text === "[@before1>") return "malformed original script marker emitted as body text; retained source defect, not English prose";
  if (entry.key === "npc.prose.e76d0bc0e98b2ad5" && text === "\u200e") return "original directionality control mark; no translatable words";
  if (entry.key === "npc.prose.f34a570db4e09470" && text === "SET [526] 1") return "original command mistakenly authored inside SAY; kept exact as text, never executed by catalogue";
  if (text === "Jack Sparrow") return "authored fictional character proper name";
  if (/^(?:Esc|NPC|WS|FPS|HP|MP|DC|MC|SC|EXP|AC|MAC)(?:\b.*)?$/.test(text) && !/[a-z]{3}/.test(text.replace(/\{[^{}]+\}/g, ""))) return "standard key/stat/protocol abbreviation with opaque parameters";
  if (!text.replace(/\{[^{}]+\}/g, "").replace(/[\p{P}\p{S}\d\s]/gu, "")) return "parameters, numerals or punctuation only";
  if (text === "*Hmmm*") return "nonverbal sound";
  if (proper.has(text.replace(/\s/g, "")) || proper.has(text.replace(/\s*\d+$/, "").replace(/\s/g, ""))) return "explicit authored proper name";
  if (locale === "pt-BR" && (portugueseSameWords.has(text.replace(/[.!?]+$/, "")) || /^(?:Mentor|Ranking) \{\d+\}$/.test(text) || /^(?:Mentor|Ranking) \(\{\d+\}\)$/.test(text))) return "same written form in Brazilian Portuguese";
  if (locale === "pt-BR" && /^(?:Oma|Shinsu)(?: \d+)?$/.test(text)) return "fictional creature name retained in Portuguese";
  if (locale === "pt-BR" && text === "HwanMaJin") return "authored fictional place name retained in Portuguese";
  if (locale === "pt-BR" && /^General [\w.]+$/.test(text)) return "General is the same Portuguese rank; personal name retained";
  return null;
}

export function validateCatalogues(catalogues) {
  const errors = [], keys = new Map(), aliases = new Map(), identicalValues = [];
  const counts = {};
  for (const [file, catalogue] of Object.entries(catalogues)) {
    if (!Array.isArray(catalogue.entries)) { errors.push({ file, reason: "entries must be an array" }); continue; }
    counts[file] = catalogue.entries.length;
    for (const entry of catalogue.entries) {
      const reference = { file, key: entry.key };
      if (typeof entry.key !== "string" || !entry.key.trim()) { errors.push({ ...reference, reason: "missing stable key" }); continue; }
      if (keys.has(entry.key)) errors.push({ ...reference, reason: "duplicate key", previous: keys.get(entry.key) });
      keys.set(entry.key, reference);
      for (const locale of locales) {
        if (typeof entry[locale] !== "string" || !entry[locale].trim()) errors.push({ ...reference, locale, reason: "missing or empty translation" });
        else if (typeof entry.en === "string") {
          if (signature(entry[locale]) !== signature(entry.en)) errors.push({ ...reference, locale, reason: "placeholder mismatch", expected: placeholders(entry.en), actual: placeholders(entry[locale]) });
          if (!same(commandTokens(entry[locale]), commandTokens(entry.en))) errors.push({ ...reference, locale, reason: "displayed command token changed", expected: commandTokens(entry.en), actual: commandTokens(entry[locale]) });
          if (locale !== "en" && entry[locale] === entry.en) identicalValues.push({ ...reference, locale, source: entry.en, reason: sameValueReason(entry, locale) ?? "requires review" });
        }
      }
      if (!Array.isArray(entry.aliases) || entry.aliases.some((alias) => typeof alias !== "string" || !alias.trim())) errors.push({ ...reference, reason: "aliases must be nonempty strings" });
      for (const alias of [...new Set([entry.en, entry["zh-TW"], entry["pt-BR"], ...(entry.aliases ?? [])].filter((value) => typeof value === "string" && value.trim()))]) {
        const list = aliases.get(alias) ?? [];
        list.push({ ...reference, values: Object.fromEntries(locales.map((locale) => [locale, entry[locale]])) }); aliases.set(alias, list);
      }
      for (const alias of entry.aliases ?? []) if (typeof alias === "string" && signature(alias) !== signature(entry.en)) errors.push({ ...reference, reason: "alias placeholder mismatch", alias, expected: placeholders(entry.en), actual: placeholders(alias) });
    }
  }
  const aliasConflicts = [], equivalentAliases = [];
  for (const [alias, owners] of aliases) if (owners.length > 1) {
    if (owners.every((owner) => same(owner.values, owners[0].values))) equivalentAliases.push({ alias, keys: owners.map(({ key }) => key) });
    else aliasConflicts.push({ alias, classification: classifyAliasConflict(alias, owners), owners });
  }
  return { schemaVersion: 1, locales, counts, totalEntries: keys.size, errors, equivalentAliasCount: equivalentAliases.length, aliasConflicts, identicalValues, unclassifiedIdenticalValues: identicalValues.filter((entry) => entry.reason === "requires review"), notes: ["Identical translations do not prove coverage: proper names, standard abbreviations and Portuguese same-form words are explicitly classified.", "Equivalent aliases across domains are allowed. Conflicting aliases list every owner/key/value; stable ID lookup avoids ambiguous display names.", "Catalogues do not translate player chat, player names, packet IDs, map filenames or NPC command targets.", "Numeric gameplay text and rendering/glyph/layout correctness still require native integration acceptance."] };
}

export function verifyFiles({ allowPending = false } = {}) {
  assertReviewedSources();
  const catalogues = {}, pendingFiles = [];
  for (const file of catalogueFiles) {
    const path = resolve(directory, file);
    if (!existsSync(path)) { pendingFiles.push(file); continue; }
    catalogues[file] = JSON.parse(readFileSync(path, "utf8"));
  }
  const report = validateCatalogues(catalogues);
  report.pendingFiles = pendingFiles;
  report.sha256 = Object.fromEntries(Object.keys(catalogues).map((file) => [file, createHash("sha256").update(readFileSync(resolve(directory, file))).digest("hex")]));
  if (pendingFiles.length && !allowPending) report.errors.push({ reason: "required catalogue files missing", files: pendingFiles });
  const bundle = JSON.parse(readFileSync(resolve(root, "packages/game-data/data/generated/localization_bundle.json"), "utf8"));
  const sourceKeys = Object.keys(bundle.languages.en.texts);
  report.sharedSourceKeys = sourceKeys.length;
  report.missingSharedKeys = sourceKeys.filter((key) => !catalogues["common.json"]?.entries.some((entry) => entry.key === key));
  if (report.missingSharedKeys.length) report.errors.push({ reason: "common catalogue omitted shared keys", keys: report.missingSharedKeys });
  const expectedHelpKeys = [
    ...Array.from({ length: 42 }, (_, page) => ["title", "body"].map(field => `help.page.${String(page).padStart(2, "0")}.${field}`)).flat(),
    "help.scroll_hint",
    ...Array.from({ length: 39 }, (_, index) => `help.shortcut.${String(index).padStart(2, "0")}`),
  ];
  const helpKeys = new Set((catalogues["help.json"]?.entries ?? []).map(entry => entry.key));
  const missingHelpKeys = expectedHelpKeys.filter(key => !helpKeys.has(key));
  report.helpCoverage = { pages: 42, shortcuts: 39, expected: expectedHelpKeys.length, present: expectedHelpKeys.length - missingHelpKeys.length, missing: missingHelpKeys };
  if (missingHelpKeys.length && !allowPending) report.errors.push({ file: "help.json", reason: "native help page or shortcut missing", keys: missingHelpKeys });
  report.sourceCoverage = {};
  for (const [sourceFile, catalogue] of [["npc-menu-source.json", "npc-menus.json"], ["npc-prose-source.json", "npc-prose.json"]]) {
    const baseSource = JSON.parse(readFileSync(resolve(root, "packages/tooling/data/native-i18n", sourceFile), "utf8")).entries;
    const source = baseSource.flatMap((entry) => [entry, ...(entry.runtimeVariants ?? [])]);
    const present = new Map((catalogues[catalogue]?.entries ?? []).map((entry) => [entry.key, entry]));
    const missing = source.filter(({ key }) => !present.has(key)).map(({ key }) => key);
    const stale = source.filter(({ key, en }) => present.has(key) && !(present.get(key).aliases ?? []).includes(en)).map(({ key }) => key);
    report.sourceCoverage[catalogue] = { originalSourceRows: baseSource.length, runtimeVariants: source.length - baseSource.length, expected: source.length, present: source.length - missing.length, missing, stale };
    if (missing.length && !allowPending) report.errors.push({ file: catalogue, reason: "visible source text missing", count: missing.length });
    if (stale.length) report.errors.push({ file: catalogue, reason: "source changed without retained exact alias", keys: stale });
  }
  report.aliasConflictClasses = Object.fromEntries([...new Set(report.aliasConflicts.map(({ classification }) => classification))].map((classification) => [classification, report.aliasConflicts.filter((entry) => entry.classification === classification).length]));
  if (report.unclassifiedIdenticalValues.length) report.errors.push({ reason: "identical English values require explicit review", keys: report.unclassifiedIdenticalValues.map(({ key, locale }) => `${key}/${locale}`) });
  writeFileSync(resolve(directory, "validation-report.json"), JSON.stringify(report, null, 2) + "\n");
  return report;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  const report = verifyFiles({ allowPending: process.argv.includes("--allow-pending") });
  console.log(JSON.stringify({ counts: report.counts, errors: report.errors, pendingFiles: report.pendingFiles, aliasConflicts: report.aliasConflicts.length, equivalentAliasCount: report.equivalentAliasCount, unclassifiedIdenticalValues: report.unclassifiedIdenticalValues }, null, 2));
  if (report.errors.length) process.exitCode = 1;
}
