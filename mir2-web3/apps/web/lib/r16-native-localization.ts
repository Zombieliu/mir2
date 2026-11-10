import startup from "./generated/r16-native-localization-startup.json";
import gameplayAsset from "./generated/r16-native-localization-delivery.json";
import { fetchBootstrapResponse } from "./first-crystal-scene-request";

/** Presentation codes only. Do not forward this list to legacy protocol/WASM enums. */
export const R16_NATIVE_LANGUAGES = ["zh-CN", "zh-TW", "en", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"] as const;
export type R16NativeLanguage = typeof R16_NATIVE_LANGUAGES[number];
export type R16NativeLanguageInfo = Readonly<{
  code: R16NativeLanguage; nativeName: string; locale: string; dir: "ltr" | "rtl"; fontFamily: string;
}>;
type Entry = { key: string; values: string[]; aliases: string[]; domain: string; systemMatch?: false };
type Catalog = { languages: R16NativeLanguageInfo[]; entries: Entry[] };
type GameplayCatalog = { gameplaySha256: string; entries: Entry[] };
type NpcCatalog = GameplayCatalog & { spans: { start: number; count: number }[] };
const catalog: Catalog = startup as Catalog;
const languages = new Map<string, { index: number; info: R16NativeLanguageInfo }>(catalog.languages.map((info, index) =>
  [info.code, { index, info: Object.freeze({ ...info }) }]));
const entries = new Map(catalog.entries.map(entry => [entry.key, entry]));
let gameplayCatalog: GameplayCatalog | null = null;
let gameplayRequest: Promise<void> | null = null;
let npcReady = false;
let npcRequest: Promise<void> | null = null;

export function hasR16GameplayLocalization(): boolean {
  return gameplayCatalog !== null;
}

async function readGameplayCatalog(cache: RequestCache): Promise<GameplayCatalog> {
  if (typeof window === "undefined" || !globalThis.crypto?.subtle) {
    // An unsecured LAN development host cannot use SubtleCrypto.
    const { default: core } = await import(/* webpackChunkName: "r16-gameplay-locales" */ "./generated/r16-native-localization-gameplay.json");
    return core as GameplayCatalog;
  }
  const { path, bytes, sha256 } = gameplayAsset;
  const { response, body } = await fetchBootstrapResponse(
    path,
    { cache, bodyByteLimit: bytes },
    response => response.arrayBuffer(),
    (url, init) => fetch(url, { ...init, credentials: "omit", redirect: "error" }),
  );
  if (!response.ok || !body || body.byteLength !== bytes) {
    throw new Error("R16 gameplay localization response is incomplete.");
  }
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", body));
  const actual = Array.from(hash, byte => byte.toString(16).padStart(2, "0")).join("");
  if (actual !== sha256) throw new Error("R16 gameplay localization digest mismatch.");
  return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(body)) as GameplayCatalog;
}

/** Called after authentication. The first playable frame needs only the core catalog. */
export function loadR16GameplayLocalization(cache: RequestCache = "force-cache"): Promise<void> {
  if (gameplayCatalog) return Promise.resolve();
  gameplayRequest ??= readGameplayCatalog(cache)
    .then(core => {
      for (const entry of core.entries) entries.set(entry.key, entry);
      indexes.clear();
      gameplayCatalog = core;
    })
    .finally(() => { gameplayRequest = null; });
  return gameplayRequest;
}

export function hasR16NpcLocalization(): boolean {
  return npcReady;
}

export function loadR16NpcLocalization(): Promise<void> {
  if (npcReady) return Promise.resolve();
  npcRequest ??= loadR16GameplayLocalization()
    .then(() => import(/* webpackChunkName: "r16-npc-locales" */ "./generated/r16-native-localization-gameplay-npc.json"))
    .then(({ default: npc }) => {
      const gameplay = orderedGameplayWithNpc(npc as NpcCatalog);
      entries.clear();
      for (const entry of catalog.entries) entries.set(entry.key, entry);
      for (const entry of gameplay) entries.set(entry.key, entry);
      // Pure NPC rows cannot alter the already-built system matcher.
      indexes.delete("npc");
      npcReady = true;
    })
    .finally(() => { npcRequest = null; });
  return npcRequest;
}

function orderedGameplayWithNpc(npc: NpcCatalog): Entry[] {
  const core = gameplayCatalog;
  if (!core || npc.gameplaySha256 !== core.gameplaySha256 || !/^[a-f0-9]{64}$/.test(core.gameplaySha256)) {
    throw new Error("R16 NPC localization does not match the gameplay catalog.");
  }
  // Retain the original row order because two identical value vectors can
  // share a source template with different trim behavior.
  const ordered = new Array<Entry | undefined>(core.entries.length + npc.entries.length);
  const npcKeys = new Set<string>();
  let npcAt = 0;
  let previousEnd = 0;
  for (const { start, count } of npc.spans) {
    const end = start + count;
    if (!Number.isSafeInteger(start) || start < previousEnd || !Number.isSafeInteger(count) || count < 1 ||
        end > ordered.length || npcAt + count > npc.entries.length) {
      throw new Error("R16 NPC localization has invalid row positions.");
    }
    for (let index = start; index < end; index += 1) {
      const entry = npc.entries[npcAt++];
      if (!entry || entry.domain !== "npc" || entries.has(entry.key) || npcKeys.has(entry.key)) {
        throw new Error("R16 NPC localization has an invalid dialog entry.");
      }
      npcKeys.add(entry.key);
      ordered[index] = entry;
    }
    previousEnd = end;
  }
  if (npcAt !== npc.entries.length) throw new Error("R16 NPC localization has invalid row positions.");
  let coreAt = 0;
  return Array.from(ordered, entry => entry ?? core.entries[coreAt++]);
}

/** Unknown codes stay explicit; locale aliases and old Spanish preferences belong to the host. */
export function nativeLanguageInfo(language: string): R16NativeLanguageInfo | null {
  return languages.get(language)?.info ?? null;
}

/** Return the catalog's literal template. The caller inserts opaque arguments once. */
export function nativeText(language: string, key: string, fallback: string = key): string {
  const locale = languages.get(language);
  return locale ? entries.get(key)?.values[locale.index] ?? fallback : fallback;
}

const npcDisplayColours = new Set(["WHITE", "CORAL", "YELLOW", "LIME", "AQUA", "CYAN", "RED", "BLUE",
  "GRAY", "GREY", "SILVER", "GOLD", "ORANGE", "GREEN", "PURPLE", "PINK", "BLACK", "MAGENTA", "BROWN"]);

/** Match static script copy whose wire representation still contains Crystal
 * colour wrappers. This is not applied to captured parameters or NPC names. */
function stripNpcDisplayColours(source: string): string {
  return source.replace(/\{([^{}]*)\/([A-Za-z]+)\}/g, (whole, text: string, colour: string) =>
    npcDisplayColours.has(colour.toUpperCase()) ? text : whole);
}

/** Only the authoritative NPC script display edge may call this function.
 * After colour normalization, only an exact authored phrase may be translated;
 * normalized captures are never rematched as templates. Targets stay separate. */
export function nativeNpcDisplayText(language: string, source: string): string {
  const locale = languages.get(language);
  if (!npcReady || !locale || typeof source !== "string" || source.length > 32768) return source;
  const index = indexFor("npc");
  if (index.exact.has(source)) {
    const entry = index.exact.get(source);
    return entry ? stripNpcDisplayColours(entry.values[locale.index]) : source;
  }
  const withoutMarkup = stripNpcDisplayColours(source);
  // A standalone wire colour terminator has no captured data. Other unknown
  // prose stays opaque instead of stripping braces inside interpolated names.
  if (withoutMarkup !== source && !withoutMarkup.trim()) return withoutMarkup;
  const candidate = withoutMarkup.trim();
  // A complete static authored phrase takes precedence over a broad template
  // that could otherwise mistake a wire-only colour suffix for a parameter.
  const entry = candidate !== source ? index.exact.get(candidate) : null;
  if (entry) return stripNpcDisplayColours(entry.values[locale.index]);
  return lookup(source, locale.index, "npc", true) ?? lookup(source, locale.index, "system", true) ?? source;
}

type Part = { kind: "literal"; value: string } | { kind: "slot"; name: string; raw: string };
type Template = { pattern: string; entry: Entry; parts: Part[]; prefix: string; suffix: string; score: number; trim: boolean };
type Index = { exact: Map<string, Entry | null>; templates: Template[]; prefixes: Map<string, number[]>; withoutPrefix: number[] };
type Domain = "system" | "npc";
const indexes = new Map<Domain, Index>();
const encoder = new TextEncoder();
const sameValues = (a: Entry, b: Entry) => a.values.length === b.values.length && a.values.every((value, index) => value === b.values[index]);

function parts(text: string): Part[] {
  const result: Part[] = []; let literalStart = 0, scan = 0;
  for (;;) {
    const start = text.indexOf("{", scan); if (start < 0) break;
    const end = text.indexOf("}", start + 1); if (end < 0) break;
    const raw = text.slice(start + 1, end);
    if (!/^[A-Za-z0-9_]+$/.test(raw) && !/^\d+:[0-9#,.*%+; PpFfNnDdXxGgEeCcRr\-]+$/.test(raw)) { scan = start + 1; continue; }
    result.push({ kind: "literal", value: text.slice(literalStart, start) }, { kind: "slot", name: raw.split(":", 1)[0], raw });
    literalStart = end + 1; scan = literalStart;
  }
  result.push({ kind: "literal", value: text.slice(literalStart) }); return result;
}

function compile(pattern: string, entry: Entry, trim: boolean): Template | null {
  const tokens = parts(pattern), literals = tokens.filter((part): part is Extract<Part, { kind: "literal" }> => part.kind === "literal");
  // Bare placeholders/separators cannot establish system ownership. Adjacent
  // opaque arguments cannot be recovered from flattened text without guessing.
  if (!tokens.some(part => part.kind === "slot") || !literals.some(part => /\p{L}/u.test(part.value))) return null;
  if (tokens.some((part, index) => part.kind === "slot" && tokens[index + 1]?.kind === "literal"
    && (tokens[index + 1] as Extract<Part, { kind: "literal" }>).value === "" && tokens[index + 2]?.kind === "slot")) return null;
  return { pattern, entry, parts: tokens, prefix: literals[0]?.value ?? "", suffix: literals.at(-1)?.value ?? "",
    score: literals.reduce((sum, part) => sum + encoder.encode(part.value).length, 0), trim };
}

function indexFor(domain: Domain): Index {
  const cached = indexes.get(domain); if (cached) return cached;
  const exact = new Map<string, Entry | null>(), patterns = new Map<string, Template | null>();
  const add = (source: string, entry: Entry) => {
    if (!source) return;
    if (parts(source).some(part => part.kind === "slot")) {
      for (const [pattern, trim] of [[source, false], ...(source.trim() !== source ? [[source.trim(), true]] : [])] as [string, boolean][]) {
        const candidate = compile(pattern, entry, trim); if (!candidate) continue;
        const previous = patterns.get(pattern);
        if (!patterns.has(pattern)) patterns.set(pattern, candidate);
        else if (previous && !sameValues(previous.entry, entry)) patterns.set(pattern, null);
      }
    } else {
      const previous = exact.get(source);
      if (!exact.has(source)) exact.set(source, entry);
      else if (previous && !sameValues(previous, entry)) exact.set(source, null);
    }
  };
  for (const entry of entries.values()) {
    if (entry.systemMatch === false || entry.domain === "keyed") continue;
    if (!(entry.domain === domain || entry.domain === "both" || domain === "system" && entry.domain === "billing")) continue;
    const sources = entry.domain === "billing" ? [entry.key, ...entry.aliases] : [...entry.values, ...entry.aliases];
    for (const source of new Set(sources)) add(source, entry);
  }
  const templates = [...patterns.values()].filter((value): value is Template => value !== null)
    .sort((a, b) => b.score - a.score || (a.pattern < b.pattern ? -1 : a.pattern > b.pattern ? 1 : 0));
  const prefixes = new Map<string, number[]>(), withoutPrefix: number[] = [];
  templates.forEach((template, index) => {
    if (!template.prefix) withoutPrefix.push(index);
    else { const prefix = Array.from(template.prefix).slice(0, 8).join(""); const group = prefixes.get(prefix) ?? []; group.push(index); prefixes.set(prefix, group); }
  });
  const result = { exact, templates, prefixes, withoutPrefix }; indexes.set(domain, result); return result;
}

function captures(template: Template, source: string): Map<string, string> | null {
  if (!source.startsWith(template.prefix) || !source.endsWith(template.suffix)) return null;
  let at = 0; const result = new Map<string, string>();
  for (let index = 0; index < template.parts.length; index++) {
    const part = template.parts[index];
    if (part.kind === "literal") { if (!source.startsWith(part.value, at)) return null; at += part.value.length; }
    else {
      const next = template.parts[index + 1], following = next?.kind === "literal" ? next.value : "";
      const end = following ? source.indexOf(following, at) : source.length; if (end < 0) return null;
      const value = source.slice(at, end);
      if (result.has(part.name) && result.get(part.name) !== value) return null;
      result.set(part.name, value); at = end;
    }
  }
  return at === source.length ? result : null;
}

function lookup(source: string, locale: number, domain: Domain, npcDisplay = false): string | null {
  const index = indexFor(domain);
  if (index.exact.has(source)) {
    const entry = index.exact.get(source);
    return entry ? (npcDisplay ? stripNpcDisplayColours(entry.values[locale]) : entry.values[locale]) : source;
  }
  const candidates = new Set(index.withoutPrefix); let prefix = "";
  for (const character of Array.from(source).slice(0, 8)) {
    prefix += character; for (const candidate of index.prefixes.get(prefix) ?? []) candidates.add(candidate);
  }
  for (const candidate of [...candidates].sort((a, b) => a - b)) {
    const template = index.templates[candidate], args = captures(template, source); if (!args) continue;
    const text = template.entry.values[locale], rendered = parts(template.trim ? text.trim() : text).map(part =>
      part.kind === "literal" ? (npcDisplay ? stripNpcDisplayColours(part.value) : part.value) : args.get(part.name) ?? `{${part.raw}}`).join("");
    return rendered;
  }
  return null;
}

/**
 * Only call at a proven system-owned presentation edge. String content cannot
 * establish provenance: a player named "Deer" must never enter this function.
 * Chat/account/character names, editable values, player mail/guild notices and
 * protocol targets remain opaque in the host. Captures are inserted once and
 * never recursively translated. Ambiguous aliases and unknown text stay exact.
 * The optional NPC domain keeps "Return" (back) separate from quest vocabulary.
 */
export function nativeSystemText(language: string, systemText: string, domain: Domain = "system"): string {
  const locale = languages.get(language);
  if (!locale || typeof systemText !== "string" || systemText.length > 16_384 ||
      !["system", "npc"].includes(domain) || (domain === "npc" && !npcReady)) return systemText;
  const translate = (source: string): string => {
    const found = lookup(source, locale.index, domain) ?? (domain === "npc" ? lookup(source, locale.index, "system") : null);
    if (found !== null) return found;
    const trimmed = source.trim();
    if (trimmed && trimmed !== source) {
      const leading = source.length - source.trimStart().length;
      return source.slice(0, leading) + translate(trimmed) + source.slice(leading + trimmed.length);
    }
    return source.includes("\n") ? source.split("\n").map(translate).join("\n") : source;
  };
  return translate(systemText);
}
