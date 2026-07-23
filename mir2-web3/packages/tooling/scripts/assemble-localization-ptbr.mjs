// Assembles the pt-BR language + es-ES overrides into the localization bundle from
// the AI-translation batch files under apps/web/docs/generated/i18n-work/.
//
// What it does (one-shot, idempotent):
//   1. Reads apps/web/lib/generated/localization_bundle.json and ALL batch-*.json.
//   2. Merges every batch row {key, ptBR, esES} into two flat maps.
//   3. Writes packages/game-data/data/i18n-overrides.json  — the COMMITTED source of
//      truth the generator (import-crystal-localization.mjs) reads so a regeneration
//      does NOT wipe pt-BR / es overrides.
//   4. Adds a "pt-BR" language to the bundle (nativeName "Português (Brasil)",
//      locale "pt-BR") with texts = en baseline overlaid by the merged ptBR map, and
//      overlays the merged esES map onto the existing "es" texts (only where given).
//   5. Writes apps/web/lib/generated/localization_bundle.json (and the game-data
//      mirror, keeping both bundle outputs in sync with the generator).
//   6. Writes apps/web/lib/generated/i18n-native-review.json from flagged.jsonl and
//      moves the glossary to apps/web/lib/generated/i18n-glossary.json.
import { mkdirSync, readFileSync, readdirSync, writeFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
const webRoot = resolve(repoRoot, "apps", "web");
const i18nWorkDir = resolve(webRoot, "docs", "generated", "i18n-work");
const webGeneratedDir = resolve(webRoot, "lib", "generated");

const webBundlePath = resolve(webGeneratedDir, "localization_bundle.json");
const gameDataBundlePath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "localization_bundle.json",
);
const overridesPath = resolve(repoRoot, "packages", "game-data", "data", "i18n-overrides.json");
const nativeReviewPath = resolve(webGeneratedDir, "i18n-native-review.json");
const glossaryOutPath = resolve(webGeneratedDir, "i18n-glossary.json");

const PT_BR_NATIVE_NAME = "Português (Brasil)";
const PT_BR_LOCALE = "pt-BR";

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function sortEntries(entries) {
  return Object.fromEntries(
    Object.entries(entries).sort(([left], [right]) => left.localeCompare(right)),
  );
}

// 1 + 2: read bundle + merge batch rows.
const bundle = readJson(webBundlePath);
if (!bundle.languages || !bundle.languages.en) {
  throw new Error("localization_bundle.json missing languages.en");
}

const batchFiles = readdirSync(i18nWorkDir)
  .filter((name) => /^batch-\d+\.json$/.test(name))
  .sort((a, b) => {
    const na = Number(a.match(/\d+/)[0]);
    const nb = Number(b.match(/\d+/)[0]);
    return na - nb;
  });

if (batchFiles.length === 0) {
  throw new Error(`no batch-*.json files found in ${i18nWorkDir}`);
}

const ptBR = {};
const esES = {};
let rowCount = 0;
for (const file of batchFiles) {
  const rows = readJson(resolve(i18nWorkDir, file));
  if (!Array.isArray(rows)) {
    throw new Error(`expected array in ${file}`);
  }
  for (const row of rows) {
    if (!row || typeof row.key !== "string") continue;
    rowCount += 1;
    if (typeof row.ptBR === "string" && row.ptBR.length > 0) {
      ptBR[row.key] = row.ptBR;
    }
    if (typeof row.esES === "string" && row.esES.length > 0) {
      esES[row.key] = row.esES;
    }
  }
}

const ptBrKeys = Object.keys(ptBR).length;
const esKeys = Object.keys(esES).length;

// 3: write the committed override source the generator consumes.
const overridesDoc = {
  _meta: {
    purpose:
      "Native-translation overrides consumed by import-crystal-localization.mjs so a " +
      "regeneration preserves pt-BR + es-ES. esES overlays the English-derived Spanish " +
      "baseline; ptBR defines the pt-BR language (English baseline where a key is absent).",
    source: "apps/web/docs/generated/i18n-work/batch-*.json",
    languages: { ptBR: { nativeName: PT_BR_NATIVE_NAME, locale: PT_BR_LOCALE } },
    ptBrKeys,
    esKeys,
    generatedBy: "packages/tooling/scripts/assemble-localization-ptbr.mjs",
  },
  esES: sortEntries(esES),
  ptBR: sortEntries(ptBR),
};
mkdirSync(resolve(overridesPath, ".."), { recursive: true });
writeFileSync(overridesPath, `${JSON.stringify(overridesDoc, null, 2)}\n`, "utf8");

// 4: apply overrides to the in-memory bundle.
const enTexts = bundle.languages.en.texts ?? {};

// es: overlay esES onto existing es texts (only where provided).
const esEntry = bundle.languages.es;
if (esEntry && esEntry.texts) {
  let updated = 0;
  for (const [key, value] of Object.entries(esES)) {
    if (esEntry.texts[key] !== value) updated += 1;
    esEntry.texts[key] = value;
  }
  esEntry.texts = sortEntries(esEntry.texts);
  console.log(`es: overlaid ${esKeys} translated keys (${updated} changed values)`);
}

// pt-BR: English baseline, then overlay ptBR translations.
const ptTexts = { ...enTexts, ...ptBR };
bundle.languages["pt-BR"] = {
  nativeName: PT_BR_NATIVE_NAME,
  locale: PT_BR_LOCALE,
  texts: sortEntries(ptTexts),
};

// Keep the languages map ordered en, zh-CN, es, pt-BR for readable diffs.
const orderedLanguages = {};
for (const code of ["en", "zh-CN", "es", "pt-BR"]) {
  if (bundle.languages[code]) orderedLanguages[code] = bundle.languages[code];
}
for (const [code, value] of Object.entries(bundle.languages)) {
  if (!orderedLanguages[code]) orderedLanguages[code] = value;
}
bundle.languages = orderedLanguages;

// 5: write the bundle to both consumed paths.
const bundleJson = `${JSON.stringify(bundle, null, 2)}\n`;
mkdirSync(webGeneratedDir, { recursive: true });
writeFileSync(webBundlePath, bundleJson, "utf8");
if (existsSync(resolve(gameDataBundlePath, ".."))) {
  // Mirror so the game-data copy matches what the web app ships.
  writeFileSync(gameDataBundlePath, bundleJson, "utf8");
  console.log(`wrote ${gameDataBundlePath}`);
}

// 6: native-review list from flagged.jsonl + move glossary.
const flaggedPath = resolve(i18nWorkDir, "flagged.jsonl");
const flaggedLines = readFileSync(flaggedPath, "utf8").split(/\r?\n/).filter((l) => l.trim().length > 0);
const flaggedItems = flaggedLines.map((line, idx) => {
  try {
    return JSON.parse(line);
  } catch (err) {
    throw new Error(`flagged.jsonl line ${idx + 1} is not valid JSON: ${err.message}`);
  }
});
const nativeReviewDoc = {
  _meta: {
    purpose: "Human native-speaker review queue for AI-translated localization keys.",
    source: "apps/web/docs/generated/i18n-work/flagged.jsonl",
    count: flaggedItems.length,
    fields: ["key", "lang", "reason", "suggestion"],
    generatedBy: "packages/tooling/scripts/assemble-localization-ptbr.mjs",
  },
  items: flaggedItems,
};
writeFileSync(nativeReviewPath, `${JSON.stringify(nativeReviewDoc, null, 2)}\n`, "utf8");

const glossaryDoc = readJson(resolve(i18nWorkDir, "glossary.json"));
writeFileSync(glossaryOutPath, `${JSON.stringify(glossaryDoc, null, 2)}\n`, "utf8");

console.log(`wrote ${overridesPath}`);
console.log(`wrote ${webBundlePath}`);
console.log(`wrote ${nativeReviewPath}`);
console.log(`wrote ${glossaryOutPath}`);
console.log(
  JSON.stringify({
    ptBrKeys,
    esUpdated: esKeys,
    flaggedTotal: flaggedItems.length,
    batchRows: rowCount,
    languages: Object.keys(bundle.languages),
  }),
);
