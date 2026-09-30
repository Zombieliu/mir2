// Build six keyed overlays without changing the accepted three-language source.
// Drafts are offline inputs, never a runtime network service or English fallback.
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { createHash } from "node:crypto";
import { catalogueFiles, placeholders, sameValueReason } from "./verify-native-i18n.mjs";

export const expansionLocales = ["ru", "hi", "id", "vi", "th", "ar"];
const root = resolve(import.meta.dirname, "../../..");
const data = resolve(root, "packages/game-data/data/native-i18n");
const reviewed = resolve(root, "packages/tooling/data/native-i18n/expansion");
const overrideFiles = ["ui-overrides.json", "content-overrides.json", "review-overrides.json", "npc-overrides.json", "names-overrides.json", "other-names-overrides.json", "runtime-overrides.json"];
const hash = text => createHash("sha256").update(text).digest("hex");
// Git stores these JSON catalogues with eol=lf. Fingerprint that canonical
// text form so a Windows editor/checkout does not invalidate equal content.
const textHash = bytes => hash(bytes.toString("utf8").replaceAll("\r\n", "\n"));
const read = path => JSON.parse(readFileSync(path, "utf8"));
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const commands = text => [...text.matchAll(/(?<![\w.])@[A-Za-z][A-Za-z0-9_]*(?:\([^\r\n)]*\))?/g)].map(m => m[0]).sort();
const cleanSlots = text => text.replace(/\{[A-Za-z0-9_]+\}|\{\d+:[0-9#,.*%+; PpFfNnDdXxGgEeCcRr\-]+\}/g, "");
// Localized decimal glyphs are presentation; the value must stay exact.
const asciiDigits = text => text.replace(/[٠-٩۰-۹०-९๐-๙]/g, ch => {
  const code = ch.codePointAt(0);
  const start = [0x660, 0x6f0, 0x966, 0xe50].find(n => code >= n && code <= n + 9);
  return String(code - start);
});
export const numbers = text => [...asciiDigits(cleanSlots(text)).matchAll(/\d+(?:[.,]\d+)*/g)].map(m => m[0]).sort();

export function validateValue(entry, locale, text) {
  const errors = [];
  if (typeof text !== "string" || !text.trim()) return ["missing/empty translation"];
  if (!equal(placeholders(entry.en), placeholders(text))) errors.push("interpolation parameters changed");
  if (!equal(commands(entry.en), commands(text))) errors.push("displayed command changed");
  if (!equal(numbers(entry.en), numbers(text))) errors.push("authored numeric values changed");
  if (text.includes("�")) errors.push("replacement character");
  if (/\[X\d+\]/i.test(text) && !/\[X\d+\]/i.test(entry.en)) errors.push("unrestored draft marker");
  if (/\p{L}/u.test(cleanSlots(entry.en)) && !/\p{L}/u.test(cleanSlots(text))) errors.push("authored words disappeared around parameters");
  return errors;
}

export function loadSources() {
  return catalogueFiles.flatMap(file => read(resolve(data, file)).entries.map(entry => ({ ...entry, file })));
}

function loadOverrides(entries) {
  const keys = new Set(entries.map(e => e.key));
  const result = {};
  for (const file of overrideFiles) {
    const path = resolve(reviewed, file);
    if (!existsSync(path)) continue;
    for (const [key, values] of Object.entries(read(path))) {
      if (!keys.has(key)) throw new Error(`Unknown override key ${key} in ${file}`);
      for (const [locale, text] of Object.entries(values)) {
        if (!expansionLocales.includes(locale) || typeof text !== "string" || !text.trim()) throw new Error(`Invalid ${file} ${key}/${locale}`);
        result[key] ??= {};
        if (result[key][locale] && result[key][locale] !== text) throw new Error(`Conflicting reviewed override ${key}/${locale}`);
        result[key][locale] = text;
      }
    }
  }
  return result;
}

// Reuse a reviewed exact source only when all owning contexts agree. A menu
// "Return" and quest "Return" with different translations stay independent.
function unambiguousShared(entries, overrides, locale) {
  const variants = new Map();
  for (const entry of entries) {
    const value = overrides[entry.key]?.[locale];
    if (!value) continue;
    if (!variants.has(entry.en)) variants.set(entry.en, new Set());
    variants.get(entry.en).add(value);
  }
  return new Map([...variants].filter(([,values]) => values.size === 1).map(([key,values]) => [key, [...values][0]]));
}

export function buildExpansion(draftsDirectory, { write = false } = {}) {
  const entries = loadSources(), overrides = loadOverrides(entries);
  const nameDraftPath = resolve(draftsDirectory, "name-drafts.json");
  const nameDrafts = existsSync(nameDraftPath) ? read(nameDraftPath) : {};
  const report = { schema: 1, locales: expansionLocales, entriesPerLocale: entries.length,
    sourceSha256: Object.fromEntries(catalogueFiles.map(file => [file, textHash(readFileSync(resolve(data, file)))])),
    reviewedOverrideSha256: Object.fromEntries(overrideFiles.filter(file => existsSync(resolve(reviewed, file))).map(file => [file, textHash(readFileSync(resolve(reviewed, file)))])),
    methods: {}, errors: [], identicalValues: [], draftSha256: {}, outputSha256: {},
    review: "Keyed overrides were authored/reviewed by the development team. Remaining prose uses offline translation drafts and requires native-speaker acceptance; no full linguistic acceptance is claimed." };
  const outputs = {};
  for (const locale of expansionLocales) {
    const path = resolve(draftsDirectory, `${locale}.json`), raw = readFileSync(path), draft = JSON.parse(raw);
    report.textHashNormalization = "UTF-8, CRLF normalized to LF for source and reviewed JSON";
    report.draftSha256[locale] = hash(raw);
    const failed = new Set(draft.problems.map(problem => problem.source));
    const shared = unambiguousShared(entries, overrides, locale);
    const values = {}, methods = { keyedReview: 0, exactSourceReview: 0, offlineDraft: 0, offlineNameDraft: 0 };
    for (const entry of entries) {
      let text = overrides[entry.key]?.[locale], method = "keyedReview";
      if (text === undefined) { text = shared.get(entry.en); method = "exactSourceReview"; }
      if (text === undefined) { text = draft.entries[entry.en]; method = "offlineDraft"; }
      const nameDraft = nameDrafts[entry.key]?.[locale];
      if (method === "offlineDraft" && text === entry.en && nameDraft && !validateValue(entry, locale, nameDraft).length) {
        text = nameDraft; method = "offlineNameDraft";
      }
      const errors = validateValue(entry, locale, text);
      if (method === "offlineDraft" && failed.has(entry.en)) errors.push("draft reported unresolved protected token");
      for (const reason of errors) report.errors.push({ key: entry.key, file: entry.file, locale, reason, source: entry.en, text });
      values[entry.key] = text;
      methods[method]++;
      if (text === entry.en) {
        const reason = method === "keyedReview" || method === "exactSourceReview" ? "explicit keyed/exact-source review" : sameValueReason({ ...entry, [locale]: text }, locale);
        report.identicalValues.push({ key: entry.key, locale, source: text, reason: reason ?? "requires review" });
      }
    }
    report.methods[locale] = methods;
    outputs[locale] = JSON.stringify(values, null, 2) + "\n";
    report.outputSha256[locale] = hash(outputs[locale]);
  }
  report.identicalNeedsReview = report.identicalValues.filter(item => item.reason === "requires review");
  if (existsSync(nameDraftPath)) report.nameDraftSha256 = hash(readFileSync(nameDraftPath));
  if (write && !report.errors.length && !report.identicalNeedsReview.length) {
    mkdirSync(resolve(data, "extra"), { recursive: true });
    for (const [locale, text] of Object.entries(outputs)) writeFileSync(resolve(data, "extra", `${locale}.json`), text);
    writeFileSync(resolve(data, "extra", "provenance.json"), JSON.stringify(report, null, 2) + "\n");
  }
  return report;
}

export function verifyExpansionFiles() {
  const entries = loadSources(), errors = [];
  const provenancePath = resolve(data, "extra", "provenance.json");
  const provenance = existsSync(provenancePath) ? read(provenancePath) : null;
  if (!provenance) errors.push({ reason: "missing overlay provenance" });
  if (provenance && (provenance.errors?.length !== 0 || provenance.identicalNeedsReview?.length !== 0)) errors.push({ reason: "overlay generation has unresolved review findings" });
  for (const file of catalogueFiles) if (provenance?.sourceSha256?.[file] !== textHash(readFileSync(resolve(data, file)))) {
    errors.push({ file, reason: "source changed since overlay review" });
  }
  for (const file of overrideFiles) if (!existsSync(resolve(reviewed, file)) || provenance?.reviewedOverrideSha256?.[file] !== textHash(readFileSync(resolve(reviewed, file)))) {
    errors.push({ file, reason: "keyed review changed since overlay generation" });
  }
  for (const locale of expansionLocales) {
    const path = resolve(data, "extra", `${locale}.json`);
    if (!existsSync(path)) { errors.push({ locale, reason: "missing keyed overlay" }); continue; }
    const values = read(path);
    if (provenance?.outputSha256?.[locale] !== hash(readFileSync(path))) errors.push({ locale, reason: "overlay bytes differ from reviewed generation" });
    if (!equal(Object.keys(values).sort(), entries.map(e => e.key).sort())) errors.push({ locale, reason: "key closure differs from source" });
    for (const entry of entries) for (const reason of validateValue(entry, locale, values[entry.key])) errors.push({ key: entry.key, locale, reason });
  }
  return { locales: expansionLocales, entriesPerLocale: entries.length, errors };
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  const draftIndex = process.argv.indexOf("--drafts"), reportIndex = process.argv.indexOf("--report");
  const report = draftIndex < 0 ? verifyExpansionFiles() : buildExpansion(process.argv[draftIndex + 1], { write: process.argv.includes("--write") });
  if (reportIndex >= 0) writeFileSync(resolve(process.argv[reportIndex + 1]), JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify({ locales: report.locales, entriesPerLocale: report.entriesPerLocale, methods: report.methods,
    errors: report.errors.length, identicalNeedsReview: report.identicalNeedsReview?.length ?? 0 }, null, 2));
  if (report.errors.length || report.identicalNeedsReview?.length) process.exitCode = 1;
}
