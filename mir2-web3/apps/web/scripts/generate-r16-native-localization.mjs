import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(import.meta.dirname, "../../..");
const NATIVE = "apps/game-client/client-bevy/src/native_i18n.rs";
const DIRECTORY = "packages/game-data/data/native-i18n";
const OUTPUT = resolve(ROOT, "apps/web/lib/generated/r16-native-localization.json");
const STARTUP_OUTPUT = resolve(ROOT, "apps/web/lib/generated/r16-native-localization-startup.json");
const GAMEPLAY_OUTPUT = resolve(ROOT, "apps/web/lib/generated/r16-native-localization-gameplay.json");
const GAMEPLAY_NPC_OUTPUT = resolve(ROOT, "apps/web/lib/generated/r16-native-localization-gameplay-npc.json");
const GAMEPLAY_DELIVERY_OUTPUT = resolve(ROOT, "apps/web/lib/generated/r16-native-localization-delivery.json");
// The fishing pane uses this authored NPC phrase as its accessible name.
const FISHING_EQUIPMENT_LABEL = "npc.prose.e2eed1b4d37624b7";
const BASE = ["common", "content", "shell", "quest", "game", "help", "npc-menus", "npc-prose"];
const AUTHORED = ["periodic", "conquest", "wallet", "procurement"];
const EXPANSION = ["ru", "hi", "id", "vi", "th", "ar"];
const BASE_LANGUAGES = ["en", "zh-TW", "pt-BR", ...EXPANSION];
const ORDER = ["zh-CN", "zh-TW", "en", "pt-BR", ...EXPANSION];
const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
const assert = (condition, message) => { if (!condition) throw Error(`R16 native localization: ${message}`); };
const hash = text => createHash("sha256").update(text).digest("hex");
const sorted = values => [...values].sort();
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const slots = text => sorted([...text.matchAll(/\{([A-Za-z0-9_]+)\}|\{(\d+):[0-9#,.*%+; PpFfNnDdXxGgEeCcRr\-]+\}/g)]
  .map(match => match[1] ?? match[2]));
const commands = text => sorted([...text.matchAll(/(?<![\w.])@[A-Za-z][A-Za-z0-9_]*(?:\([^\r\n)]*\))?/g)].map(match => match[0]));
// Existing native Simplified overrides retain shortcut placeholders on these
// eight otherwise plain captions. Preserve the literal projection explicitly;
// they are never eligible for alias/template matching. No new mismatch is safe.
export const R16_NATIVE_SLOT_EXCEPTIONS = Object.freeze([
  ["client.Exit", "Exit", "退出 ({0})"], ["client.Fishing", "Fishing", "钓鱼 ({0})"],
  ["client.Groups", "Groups", "队伍 ({0})"], ["client.Guild", "Guild", "行会（{0}）"],
  ["client.LogOut", "Log out", "退出登录 ({0})"], ["client.Quests", "Quests", "任务 ({0})"],
  ["client.Skills", "Skills", "技能 ({0})"], ["client.Trade", "Trade", "交易 ({0})"],
].map(([key, source, target]) => Object.freeze({ key, language: "zh-CN", source, target, sourceSlots: [], targetSlots: ["0"] })));

// A bounded literal reader, following the existing offline Simplified-Chinese
// generator. No eval, Rust compiler, network, OpenCC or model is required.
class RustLiterals {
  constructor(source) { this.source = source; this.at = 0; }
  skip() {
    for (;;) {
      while (/\s/.test(this.source[this.at] ?? "") && this.at < this.source.length) this.at++;
      if (this.source.startsWith("//", this.at)) {
        const end = this.source.indexOf("\n", this.at); this.at = end < 0 ? this.source.length : end + 1;
      } else if (this.source.startsWith("/*", this.at)) {
        const end = this.source.indexOf("*/", this.at + 2); assert(end >= 0, "unterminated Rust comment"); this.at = end + 2;
      } else break;
    }
  }
  expect(value) { this.skip(); assert(this.source.startsWith(value, this.at), `unsupported Rust literal near ${this.at}`); this.at += value.length; }
  string() {
    this.expect('"'); let result = "";
    const escapes = { n: "\n", r: "\r", t: "\t", 0: "\0", '"': '"', "\\": "\\" };
    while (this.at < this.source.length) {
      let value = this.source[this.at++];
      if (value === '"') return result;
      if (value !== "\\") { result += value; continue; }
      value = this.source[this.at++];
      if (Object.hasOwn(escapes, value)) result += escapes[value];
      else if (value === "u") {
        this.expect("{"); const end = this.source.indexOf("}", this.at);
        assert(end >= 0 && /^[a-fA-F0-9_]+$/.test(this.source.slice(this.at, end)), "invalid Rust Unicode escape");
        result += String.fromCodePoint(Number.parseInt(this.source.slice(this.at, end).replaceAll("_", ""), 16)); this.at = end + 1;
      } else if (value === "x") {
        const digits = this.source.slice(this.at, this.at + 2); assert(/^[a-fA-F0-9]{2}$/.test(digits), "invalid Rust byte escape");
        result += String.fromCharCode(Number.parseInt(digits, 16)); this.at += 2;
      } else throw Error("R16 native localization: unsupported Rust string escape");
    }
    throw Error("R16 native localization: unterminated Rust string");
  }
  constant(name) {
    assert(/^[A-Z][A-Z0-9_]*$/.test(name), "invalid Rust constant");
    const match = new RegExp(`const\\s+${name}\\s*:\\s*&str\\s*=`).exec(this.source);
    assert(match, `missing Rust string constant ${name}`);
    const reader = new RustLiterals(this.source); reader.at = match.index + match[0].length; return reader.string();
  }
  value() {
    this.skip(); if (this.source[this.at] === '"') return this.string();
    const match = /^[A-Z][A-Z0-9_]*/.exec(this.source.slice(this.at)); assert(match, "unsupported embedded Rust expression");
    this.at += match[0].length; return this.constant(match[0]);
  }
  billing() {
    const begin = this.source.indexOf("const BILLING_TRANSLATIONS:"); assert(begin >= 0, "missing embedded billing vocabulary");
    const array = this.source.indexOf("= &[", begin); assert(array >= 0, "unsupported embedded vocabulary declaration");
    this.at = array + "= &[".length; const entries = [];
    for (;;) {
      this.skip(); if (this.source[this.at] === "]") break;
      this.expect("("); const key = this.string(); this.expect(","); this.expect("["); const values = [];
      for (;;) {
        this.skip(); if (this.source[this.at] === "]") break;
        values.push(this.value()); this.skip();
        if (this.source[this.at] === ",") this.at++; else assert(this.source[this.at] === "]", `invalid embedded array ${key}`);
      }
      this.expect("]"); this.skip(); if (this.source[this.at] === ",") this.at++;
      this.expect(")"); this.expect(","); assert(values.length === 9, `expected nine embedded locale values: ${key}`);
      entries.push({ key, ...Object.fromEntries(BASE_LANGUAGES.map((code, index) => [code, values[index]])) });
    }
    this.expect("]"); this.expect(";"); return entries;
  }
}

function localeMetadata(source) {
  const all = /pub const ALL:[\s\S]*?=\s*\[([\s\S]*?)\];/.exec(source);
  assert(all, "missing native Locale::ALL");
  const variants = [...all[1].matchAll(/Self::([A-Za-z]+)/g)].map(match => match[1]);
  const method = name => {
    const begin = source.indexOf(`pub const fn ${name}(`); assert(begin >= 0, `missing locale ${name}`);
    const end = source.indexOf("\n    }", begin); assert(end >= 0, `invalid locale ${name}`); return source.slice(begin, end);
  };
  const values = name => new Map([...method(name).matchAll(/(Self::[A-Za-z]+|_)\s*=>\s*("(?:[^"\\]|\\.)*")/g)].map(match => {
    const reader = new RustLiterals(match[2]); return [match[1].replace("Self::", ""), reader.string()];
  }));
  const codes = values("code"), labels = values("label"), fonts = values("font_family");
  assert(method("is_rtl").includes("matches!(self, Self::Arabic)"), "review changed native directionality rules");
  const result = variants.map(variant => {
    const code = codes.get(variant), nativeName = labels.get(variant), fontFamily = fonts.get(variant) ?? fonts.get("_");
    assert(code && nativeName && fontFamily, `incomplete native locale metadata ${variant}`);
    return { code, nativeName, locale: code === "en" ? "en-US" : code, dir: variant === "Arabic" ? "rtl" : "ltr", fontFamily };
  });
  assert(equal(result.map(item => item.code), ORDER), "native language set/order changed; update the typed browser contract explicitly");
  return result;
}

/** Pure deterministic builder; the optional reader is trusted test injection. */
export function buildR16NativeLocalization({ readSource = relative => readFileSync(resolve(ROOT, relative), "utf8") } = {}) {
  const sourcePins = {}, counts = {}, source = relative => {
    const text = readSource(relative); assert(typeof text === "string", `missing source ${relative}`);
    const normalized = text.replaceAll("\r\n", "\n"); sourcePins[relative] = hash(normalized); return normalized;
  };
  const rust = source(NATIVE), reader = new RustLiterals(rust), languages = localeMetadata(rust);
  const simplified = JSON.parse(source(`${DIRECTORY}/extra/zh-CN.json`));
  const expansion = Object.fromEntries(EXPANSION.map(code => [code, JSON.parse(source(`${DIRECTORY}/extra/${code}.json`))]));
  assert(record(simplified) && Object.values(expansion).every(record), "locale overlays must be keyed objects");
  const entries = [], keys = new Set(), baseKeys = new Set(), usedExceptions = new Set();
  const exceptions = new Map(R16_NATIVE_SLOT_EXCEPTIONS.map(entry => [entry.key, entry]));
  const append = (entry, kind, file) => {
    const key = entry.key;
    assert(typeof key === "string" && key.trim() && !keys.has(key), `invalid/duplicate native key ${key}`); keys.add(key);
    if (kind === "base") baseKeys.add(key);
    const translations = Object.fromEntries(ORDER.map(code => [code, code === "zh-CN" ? simplified[key]
      : kind === "base" && EXPANSION.includes(code) ? expansion[code][key] : entry[code]]));
    assert(Array.isArray(entry.aliases ?? []) && (entry.aliases ?? []).every(alias => typeof alias === "string" && alias.trim()), `invalid aliases ${key}`);
    const reference = slots(entry.en ?? ""), displayedCommands = commands(entry.en ?? "");
    for (const code of ORDER) {
      const text = translations[code];
      assert(typeof text === "string" && text.trim(), `missing/empty ${code} translation ${key}`);
      if (!equal(slots(text), reference)) {
        const exception = exceptions.get(key);
        assert(exception && exception.language === code && exception.source === entry.en && exception.target === text
          && equal(exception.sourceSlots, reference) && equal(exception.targetSlots, slots(text)), `placeholder mismatch ${key}/${code}`);
        usedExceptions.add(key);
      }
      assert(equal(commands(text), displayedCommands), `displayed command changed ${key}/${code}`);
      assert(!text.includes("\uFFFD"), `replacement character ${key}/${code}`);
    }
    for (const alias of entry.aliases ?? []) assert(equal(slots(alias), reference), `alias placeholder mismatch ${key}`);
    if (entry["zh-CN"] !== undefined) assert(entry["zh-CN"] === translations["zh-CN"], `authored/Simplified overlay disagreement ${key}`);
    const domain = kind === "billing" ? "billing" : key.startsWith("procurement.") ? "keyed"
      : key.startsWith("npc.") ? "npc" : /^(?:periodic\.(?:accept|complete)\.|conquest\.)/.test(key) ? "both" : "system";
    entries.push({ key, values: ORDER.map(code => translations[code]), aliases: [...new Set(entry.aliases ?? [])], domain,
      ...(usedExceptions.has(key) ? { systemMatch: false } : {}) });
    counts[file] = (counts[file] ?? 0) + 1;
  };
  for (const name of [...BASE, ...AUTHORED]) {
    const file = `${name}.json`, document = JSON.parse(source(`${DIRECTORY}/${file}`));
    assert(Array.isArray(document.entries) && document.entries.length > 0, `empty catalog ${file}`);
    for (const entry of document.entries) { assert(record(entry), `invalid catalog entry ${file}`); append(entry, BASE.includes(name) ? "base" : "authored", file); }
  }
  for (const entry of reader.billing()) append(entry, "billing", "native_i18n.rs::BILLING_TRANSLATIONS");
  for (const [constant, key] of [["MONTHLY_CARD_ITEM_NAME", "billing.monthlyCardItemName"], ["MONTHLY_CARD_ITEM_TOOLTIP", "billing.monthlyCardItemTooltip"]]) {
    const entry = entries.find(entry => entry.key === key); assert(entry, `missing ${key}`); entry.aliases.push(reader.constant(constant));
  }
  assert(equal(sorted(Object.keys(simplified)), sorted(keys)), "Simplified overlay must exactly cover all native keys");
  for (const code of EXPANSION) assert(equal(sorted(Object.keys(expansion[code])), sorted(baseKeys)), `${code} overlay must exactly cover the base native keys`);
  assert(equal(sorted(usedExceptions), sorted(exceptions.keys())), "obsolete native slot exception; review/remove its fixed allowlist entry");
  return { schemaVersion: 1, languages, counts, sourcePins: Object.fromEntries(Object.entries(sourcePins).sort(([a], [b]) => a < b ? -1 : 1)),
    slotExceptions: R16_NATIVE_SLOT_EXCEPTIONS, entries };
}

export function encodeR16NativeLocalization(bundle) { return `${JSON.stringify(bundle)}\n`; }

export function partitionR16NativeLocalization(bundle) {
  const startup = { languages: bundle.languages, entries: [] };
  const gameplay = { entries: [] };
  // Keep the shell, account, character and system packet copy available before
  // entering the world. These namespaces are only used by world content.
  const gameplayKey = /^(?:content|npc|quest|help|procurement|periodic|conquest|billing|monthly)\./;
  for (const entry of bundle.entries) {
    (gameplayKey.test(entry.key) ? gameplay : startup).entries.push(entry);
  }
  return { startup, gameplay };
}

/** Keep complete locale vectors and record the NPC rows' original positions. */
export function partitionR16GameplayLocalization(gameplay) {
  const gameplaySha256 = hash(encodeR16NativeLocalization(gameplay));
  const core = { gameplaySha256, entries: [] };
  const npc = { gameplaySha256, spans: [], entries: [] };
  for (const [index, entry] of gameplay.entries.entries()) {
    if (entry.domain !== "npc" || entry.key === FISHING_EQUIPMENT_LABEL) {
      core.entries.push(entry);
      continue;
    }
    npc.entries.push(entry);
    const span = npc.spans.at(-1);
    if (span && span.start + span.count === index) span.count += 1;
    else npc.spans.push({ start: index, count: 1 });
  }
  return { core, npc };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert(process.argv.slice(2).every(arg => arg === "--check") && process.argv.length <= 3, "only --check is supported");
  const result = buildR16NativeLocalization(), bytes = encodeR16NativeLocalization(result);
  const { startup, gameplay } = partitionR16NativeLocalization(result);
  const { core, npc } = partitionR16GameplayLocalization(gameplay);
  const gameplayBytes = encodeR16NativeLocalization(core);
  const sha256 = hash(gameplayBytes);
  const path = "/bootstrap/r16-locales/" + sha256 + ".json";
  const delivery = { path, bytes: Buffer.byteLength(gameplayBytes), sha256 };
  const outputs = [[OUTPUT, bytes], [STARTUP_OUTPUT, encodeR16NativeLocalization(startup)],
    [GAMEPLAY_OUTPUT, gameplayBytes], [GAMEPLAY_NPC_OUTPUT, encodeR16NativeLocalization(npc)],
    [GAMEPLAY_DELIVERY_OUTPUT, encodeR16NativeLocalization(delivery)],
    [resolve(ROOT, "apps/web/public" + path), gameplayBytes]];
  const checked = process.argv.includes("--check");
  for (const [path, encoded] of outputs) {
    if (checked) assert(readFileSync(path, "utf8") === encoded, `generated browser catalog is stale ${path}`);
    else {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, encoded, "utf8");
    }
  }
  console.log(JSON.stringify({ checked, languages: result.languages.map(language => language.code),
    keys: result.entries.length, sourcePins: Object.keys(result.sourcePins).length, slotExceptions: result.slotExceptions.length,
    bytes: Buffer.byteLength(bytes), sha256: hash(bytes), startupKeys: startup.entries.length, gameplayKeys: gameplay.entries.length }));
}
