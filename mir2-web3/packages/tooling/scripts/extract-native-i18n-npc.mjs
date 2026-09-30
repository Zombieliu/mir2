import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { createHash } from "node:crypto";
import { assertReviewedSources } from "./native-i18n-source-guard.mjs";

const root = resolve(import.meta.dirname, "../../..");
const sourceDir = resolve(root, "packages/game-data/data/generated");
const outputDir = resolve(root, "packages/tooling/data/native-i18n");
const hash = (text) => createHash("sha256").update(text).digest("hex");
const normalize = (key) => key.replaceAll("\\", "/").replace(/\.txt$/i, "").toLowerCase();

// Match npc_script.rs: resolution precedes markup parsing. Substitute opaque
// captures here, not translated values, so usernames and script inputs survive.
export function isStaticallyUnsupportedNpcToken(token) {
  if (!token.startsWith("<$")) return false;
  const name = token.slice(2, -1).toUpperCase();
  if (["USERNAME", "DATE", "GUILDGTRENTALDAYSLEFT", "GUILDEXTENDFEE"].includes(name)) return false;
  return !/^(?:OUTPUT|CONQUESTOWNER|CONQUESTRATE|CONQUESTGOLD|CONQUESTSCHEDULE|CONQUESTGUARD|CONQUESTWALL|CONQUESTGATE)\([^)]*\)$/.test(name);
}

export function displayTemplate(source, { retainUnsupportedTokens = false } = {}) {
  const parameters = [];
  const text = source.replace(/<\$[^>]+>|%ARG\(\d+\)|%INPUTSTR|%[A-Za-z]\d/gi, (token) => {
    const name = `npc_arg${parameters.length}`;
    parameters.push({ name, source: token });
    if (retainUnsupportedTokens && isStaticallyUnsupportedNpcToken(token)) return token;
    return `{${name}}`;
  });
  return { text, parameters };
}

export function splitVisibleNpcLine(source, options = {}) {
  const { text, parameters } = displayTemplate(source, options);
  let remainder = text, body = "";
  const menus = [];
  while (true) {
    const start = remainder.indexOf("<");
    if (start < 0) break;
    const doubled = remainder[start + 1] === "<";
    const contentStart = start + (doubled ? 2 : 1);
    const closing = remainder.indexOf(doubled ? ">>" : ">", contentStart);
    if (closing < 0) break;
    let inside = remainder.slice(contentStart, closing);
    if (doubled) {
      const last = inside.lastIndexOf("/");
      if (last >= 0 && inside.slice(0, last).includes("/")) inside = inside.slice(0, last);
    }
    const split = inside.lastIndexOf("/");
    if (split >= 0) menus.push({ text: inside.slice(0, split).trim(), target: inside.slice(split + 1).trim() });
    body += remainder.slice(0, start);
    remainder = remainder.slice(closing + (doubled ? 2 : 1));
  }
  body += remainder;
  // Colour annotations are a rendering concern, never a command or parameter.
  const plain = (value) => value.replace(/\{([^{}]*(?:\{[^{}]+\}[^{}]*)*)\/[^{}\/]+\}/g, "$1").trim();
  return { body: plain(body), rawBody: body.trim(), menus: menus.map((menu) => ({ ...menu, rawText: menu.text, text: plain(menu.text) })), parameters };
}

export function extractNpc() {
  assertReviewedSources(["crystal_npc_manifest.json", "crystal_npc_info_manifest.json"]);
  const manifestBytes = readFileSync(resolve(sourceDir, "crystal_npc_manifest.json"));
  const infoBytes = readFileSync(resolve(sourceDir, "crystal_npc_info_manifest.json"));
  const manifest = JSON.parse(manifestBytes), info = JSON.parse(infoBytes);
  const scripts = new Map(manifest.scripts.map((script) => [normalize(script.script_key), script]));
  const roots = new Set(info.npcs.map((npc) => normalize(npc.script_key)));
  const reachable = new Set(roots), pending = [...roots];
  while (pending.length) {
    const script = scripts.get(pending.shift());
    for (const insert of script?.inserts ?? []) {
      const key = normalize(insert.target_path);
      if (!reachable.has(key)) { reachable.add(key); pending.push(key); }
    }
  }
  const prose = new Map(), menus = new Map();
  let occurrences = 0;
  const raw = new Set();
  const add = (map, kind, text, rawText, reference, parameters, runtime) => {
    if (!text) return;
    const key = `npc.${kind}.${hash(text).slice(0, 16)}`;
    const entry = map.get(text) ?? { key, en: text, aliases: [text], parameters: parameters.filter(({ name }) => text.includes(`{${name}}`)), references: [], runtimeVariants: [] };
    if (rawText && !entry.aliases.includes(rawText)) entry.aliases.push(rawText);
    if (runtime && runtime.en && (runtime.raw !== rawText || runtime.en !== text)) {
      const omittedParameters = parameters.filter(({ name, source }) => text.includes(`{${name}}`) && isStaticallyUnsupportedNpcToken(source)).map(({ name }) => name);
      if (omittedParameters.length) {
        const variant = { key: `${key}.runtime-empty.${hash(runtime.raw).slice(0, 8)}`, en: runtime.en, aliases: [...new Set([runtime.en, runtime.raw].filter(Boolean))], omittedParameters, reason: "npc_script.rs leaves unsupported named tokens unresolved; strip_crystal_npc_links removes their angle markup before delivery" };
        if (!entry.runtimeVariants.some(({ key: existing }) => existing === variant.key)) entry.runtimeVariants.push(variant);
      }
    }
    entry.references.push(reference); map.set(text, entry);
  };
  for (const script of manifest.scripts) {
    if (!reachable.has(normalize(script.script_key))) continue;
    for (const section of script.sections) {
      let mode = "";
      for (const [offset, rawLine] of section.lines.entries()) {
        const line = rawLine.trim();
        if (line.startsWith("#")) { mode = line.toUpperCase(); continue; }
        if (!line || line.startsWith(";") || !["#SAY", "#ELSESAY"].includes(mode)) continue;
        occurrences++; raw.add(line);
        const parsed = splitVisibleNpcLine(line);
        const runtime = splitVisibleNpcLine(line, { retainUnsupportedTokens: true });
        const reference = { script: script.script_key, label: section.label, line: section.line_number + offset };
        add(prose, "prose", parsed.body, parsed.rawBody, reference, parsed.parameters, { en: runtime.body, raw: runtime.rawBody });
        for (const [index, menu] of parsed.menus.entries()) add(menus, "menu", menu.text, menu.rawText, { ...reference, target: menu.target }, parsed.parameters, runtime.menus[index] && { en: runtime.menus[index].text, raw: runtime.menus[index].rawText });
      }
    }
  }
  const byKey = (map) => [...map.values()].sort((a, b) => a.key.localeCompare(b.key, "en"));
  const audit = { schemaVersion: 1, manifestSha256: hash(manifestBytes), npcInfoSha256: hash(infoBytes), rootScripts: roots.size, reachableScripts: reachable.size, matchedScripts: [...reachable].filter((key) => scripts.has(key)).length, referencedGmScripts: [...reachable].filter((key) => key.startsWith("gm/") && scripts.has(key)).length, excludedUnreferencedScripts: manifest.scripts.length - [...reachable].filter((key) => scripts.has(key)).length, sayLineOccurrences: occurrences, uniqueSaySourceLines: raw.size, uniqueBodyLines: prose.size, uniqueMenuLabels: menus.size, runtimeBodyVariants: [...prose.values()].reduce((sum, row) => sum + row.runtimeVariants.length, 0), runtimeMenuVariants: [...menus.values()].reduce((sum, row) => sum + row.runtimeVariants.length, 0), notes: ["Scripts referenced by the 375 NPC records and their INSERT closure are included; this includes gated GM-prefixed scripts and does not authorize their execution.", "SAY/ELSESAY text is counted, never IF/ACT commands or label/node counts.", "Menu targets and script parameters are evidence only; translated catalogues contain visible labels and opaque captures.", "Raw colour aliases match authored markup literally; no runtime operation strips colour-like text from interpolated player values.", "Statically unsupported named-token variants mirror current server removal, with only those missing values omitted from translated copy.", "The 270 unreferenced scripts are outside this NPC-record corpus; source coverage is not proof that every script page is reachable during ordinary gameplay."] };
  mkdirSync(outputDir, { recursive: true });
  for (const [name, data] of [["npc-prose-source.json", { entries: byKey(prose) }], ["npc-menu-source.json", { entries: byKey(menus) }], ["npc-source-audit.json", audit]]) writeFileSync(resolve(outputDir, name), JSON.stringify(data, null, 2) + "\n");
  return { prose: byKey(prose), menus: byKey(menus), audit };
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) console.log(JSON.stringify(extractNpc().audit, null, 2));
