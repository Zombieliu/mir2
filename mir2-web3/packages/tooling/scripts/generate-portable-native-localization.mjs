import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { buildR16NativeLocalization } from "../../../apps/web/scripts/generate-r16-native-localization.mjs";

const ROOT = resolve(import.meta.dirname, "../../..");
const OUTPUT = resolve(ROOT, "packages/game-data/data/generated/portable-native-i18n.json");
export const PORTABLE_NATIVE_LANGUAGES = Object.freeze(["en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar", "zh-CN"]);
export const PORTABLE_NATIVE_MAX_BYTES = 1.25 * 1024 * 1024;
// Only shared presentation fields. NPC prose/menus use the existing browser
// catalog at their authoritative display edge; account/chat text is never input.
const graduationDisplayKeys = ["content.item.244.name", "content.item.259.name", "content.item.272.name", "content.map.186.name"];
export const isPortableNativeKey = key => graduationDisplayKeys.includes(key)
  || /^(?:quest\.|ui\.quest|ui\.(?:previous|next|close)$|game\.|periodic\.|shell\.|common\.|content\.magic\.|content\.(?:monster|npc)\..*\.name$)/.test(key);

export function buildPortableNativeLocalization({ native = buildR16NativeLocalization() } = {}) {
  assert.equal(native.schemaVersion, 1);
  assert(Array.isArray(native.languages) && native.languages.length === 10);
  const codes = native.languages.map(row => row.code);
  assert.deepEqual([...codes].sort(), [...PORTABLE_NATIVE_LANGUAGES].sort());
  assert.equal(new Set(codes).size, 10);
  assert(Array.isArray(native.entries));
  const seen = new Set();
  const entries = [];
  for (const row of native.entries) {
    assert(typeof row.key === "string" && !seen.has(row.key), "duplicate/invalid Native key");
    seen.add(row.key);
    if (!isPortableNativeKey(row.key)) continue;
    assert(Array.isArray(row.values) && row.values.length === 10
      && row.values.every(value => typeof value === "string" && value.trim() && !value.includes("\uFFFD")), row.key);
    assert(Array.isArray(row.aliases) && row.aliases.every(alias => typeof alias === "string"));
    assert(["system", "npc", "both", "keyed", "billing"].includes(row.domain));
    const values = PORTABLE_NATIVE_LANGUAGES.map(code => row.values[codes.indexOf(code)]);
    entries.push({ key: row.key, values,
      aliases: [...new Set(row.aliases.filter(alias => !values.includes(alias)))], domain: row.domain,
      ...(row.systemMatch === false ? { systemMatch: false } : {}) });
  }
  entries.sort((a, b) => a.key < b.key ? -1 : a.key > b.key ? 1 : 0);
  assert(entries.length > 0 && entries.length <= 2500, "bounded shared Native vocabulary changed");
  for (const key of ["quest.action.talk", "quest.feedback.76", "quest.feedback.79", "quest.feedback.32",
    "periodic.copy.0", "periodic.copy.1", "game.060", "game.061", "game.151", "content.magic.Fencing.name",
    "ui.previous", "ui.next", "ui.close", ...graduationDisplayKeys])
    assert(seen.has(key) && entries.some(entry => entry.key === key), `required display key missing: ${key}`);
  assert(native.sourcePins && Object.values(native.sourcePins).every(value => /^[a-f0-9]{64}$/.test(value)));
  const result = { schema: 1, languages: [...PORTABLE_NATIVE_LANGUAGES], entries, sourcePins: native.sourcePins };
  assert(Buffer.byteLength(encodePortableNativeLocalization(result)) <= PORTABLE_NATIVE_MAX_BYTES,
    "shared display vocabulary exceeds the unchanged 1.25 MiB producer cap");
  return result;
}

export function encodePortableNativeLocalization(value) { return JSON.stringify(value) + "\n"; }

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert(process.argv.length <= 3 && process.argv.slice(2).every(arg => arg === "--check"));
  const result = buildPortableNativeLocalization(), bytes = encodePortableNativeLocalization(result);
  if (process.argv.includes("--check")) assert.equal(readFileSync(OUTPUT, "utf8"), bytes, "portable Native display projection is stale");
  else { mkdirSync(dirname(OUTPUT), { recursive: true }); writeFileSync(OUTPUT, bytes); }
  console.log(JSON.stringify({ checked: process.argv.includes("--check"), keys: result.entries.length,
    languages: result.languages, bytes: Buffer.byteLength(bytes), budgetBytes: PORTABLE_NATIVE_MAX_BYTES }));
}
